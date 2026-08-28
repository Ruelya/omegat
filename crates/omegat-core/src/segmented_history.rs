// SPDX-License-Identifier: GPL-3.0-or-later

//! Crash-safe, bounded hot history backed by immutable JSON segments.
//!
//! Product journals need an indefinitely durable idempotency record, but an
//! indefinitely growing NDJSON file makes every retry and every recovery scan
//! the complete past. This store keeps only a bounded, human-readable recent
//! window and hot index. Older records move to content-addressed immutable
//! segments. A replicated generational manifest contains a sparse
//! hash-prefix-to-segment index, so an exact partition lookup does not stream
//! unrelated history.
//!
//! Publication order is deliberately one-way:
//!
//! 1. fsync and rename replacement immutable segments;
//! 2. durably replace both manifest replicas;
//! 3. remove records from the replicated hot index;
//! 4. unlink predecessor/orphan segments and fsync their directory.
//!
//! A process death can therefore leave duplicates or garbage, never a missing
//! authoritative record. Opening the store coalesces exact duplicates and
//! finishes garbage collection under the caller's product lock.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const STORE_VERSION: u8 = 1;
static SEGMENT_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A history row supplies the partition used by sparse exact queries.
///
/// `relocate` is called for mutable hot rows and for rows decoded from an
/// immutable segment whose original scope moved. Implementations should only
/// rebase paths contained by `old_scope`; external paths must remain external.
pub trait SegmentedHistoryRecord: Clone + PartialEq + Serialize + DeserializeOwned {
    fn history_partition(&self) -> &str;

    fn relocate(&mut self, _old_scope: &Path, _new_scope: &Path) {}
}

#[derive(Clone, Debug)]
pub struct SegmentedHistoryOptions {
    pub recent_limit: usize,
    pub hot_limit: usize,
    pub segment_record_limit: usize,
    pub generation_segment_limit: usize,
    pub generation_record_limit: usize,
    pub partition_prefix_hex: usize,
}

impl Default for SegmentedHistoryOptions {
    fn default() -> Self {
        Self {
            recent_limit: 128,
            hot_limit: 128,
            segment_record_limit: 32,
            generation_segment_limit: 16,
            generation_record_limit: 128,
            partition_prefix_hex: 4,
        }
    }
}

impl SegmentedHistoryOptions {
    fn validate(&self) -> Result<(), String> {
        if self.recent_limit == 0
            || self.hot_limit == 0
            || self.segment_record_limit == 0
            || self.generation_segment_limit < 2
            || self.generation_record_limit == 0
            || self.partition_prefix_hex == 0
            || self.partition_prefix_hex > 64
        {
            return Err(
                "segmented history limits must be non-zero and generation limit >= 2".into(),
            );
        }
        if self.recent_limit > self.hot_limit {
            return Err("segmented history recent limit cannot exceed hot limit".into());
        }
        Ok(())
    }
}

/// On-disk names are configurable so a product can retain a bounded legacy
/// observation path while sharing the same storage implementation.
#[derive(Clone, Debug)]
pub struct SegmentedHistoryLayout {
    pub recent_file: String,
    pub hot_file: String,
    pub hot_recovery_file: String,
    pub manifest_file: String,
    pub manifest_recovery_file: String,
    pub archive_directory: String,
}

impl SegmentedHistoryLayout {
    pub fn named(name: &str) -> Self {
        Self {
            recent_file: format!("{name}-recent.ndjson"),
            hot_file: format!("{name}-hot.json"),
            hot_recovery_file: format!(".{name}-hot.recovery.json"),
            manifest_file: format!("{name}-manifest.json"),
            manifest_recovery_file: format!(".{name}-manifest.recovery.json"),
            archive_directory: format!("{name}-archive"),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        for name in [
            &self.recent_file,
            &self.hot_file,
            &self.hot_recovery_file,
            &self.manifest_file,
            &self.manifest_recovery_file,
            &self.archive_directory,
        ] {
            if name.is_empty()
                || Path::new(name).file_name().and_then(|value| value.to_str())
                    != Some(name.as_str())
            {
                return Err(format!("unsafe segmented history layout name {name}"));
            }
        }
        Ok(())
    }
}

impl Default for SegmentedHistoryLayout {
    fn default() -> Self {
        Self::named("history")
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HotIndex<T> {
    version: u8,
    scope: PathBuf,
    revision: u64,
    records: Vec<T>,
}

impl<T> HotIndex<T> {
    fn empty(scope: &Path) -> Self {
        Self {
            version: STORE_VERSION,
            scope: normalized(scope),
            revision: 0,
            records: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentDescriptor {
    pub id: u64,
    pub generation: u64,
    pub file: String,
    pub sha256: String,
    pub record_count: usize,
    pub partition_prefixes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ArchiveManifest {
    version: u8,
    scope: PathBuf,
    revision: u64,
    generation: u64,
    next_segment_id: u64,
    segments: Vec<SegmentDescriptor>,
    partition_index: BTreeMap<String, Vec<u64>>,
}

impl ArchiveManifest {
    fn empty(scope: &Path) -> Self {
        Self {
            version: STORE_VERSION,
            scope: normalized(scope),
            revision: 0,
            generation: 0,
            next_segment_id: 1,
            segments: Vec::new(),
            partition_index: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ArchiveSegment<T> {
    version: u8,
    scope: PathBuf,
    id: u64,
    generation: u64,
    records: Vec<T>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentedHistoryStatus {
    pub generation: u64,
    pub segment_count: usize,
    pub archived_records: usize,
    pub hot_records: usize,
    pub manifest_revision: u64,
    pub hot_revision: u64,
}

#[derive(Debug)]
pub struct SegmentedHistory<T: SegmentedHistoryRecord> {
    directory: PathBuf,
    scope: PathBuf,
    layout: SegmentedHistoryLayout,
    options: SegmentedHistoryOptions,
    hot: HotIndex<T>,
    manifest: ArchiveManifest,
}

impl<T: SegmentedHistoryRecord> SegmentedHistory<T> {
    pub fn has_durable_state(directory: &Path, layout: &SegmentedHistoryLayout) -> bool {
        [
            directory.join(&layout.hot_file),
            directory.join(&layout.hot_recovery_file),
            directory.join(&layout.manifest_file),
            directory.join(&layout.manifest_recovery_file),
        ]
        .iter()
        .any(|path| path.exists())
    }

    pub fn open(
        directory: &Path,
        scope: &Path,
        layout: SegmentedHistoryLayout,
        options: SegmentedHistoryOptions,
    ) -> Result<Self, String> {
        Self::open_with(directory, scope, layout, options, &mut |_| Ok(()))
    }

    pub fn open_with<F>(
        directory: &Path,
        scope: &Path,
        layout: SegmentedHistoryLayout,
        options: SegmentedHistoryOptions,
        checkpoint: &mut F,
    ) -> Result<Self, String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        options.validate()?;
        layout.validate()?;
        std::fs::create_dir_all(directory).map_err(|error| {
            format!(
                "create segmented history directory {}: {error}",
                directory.display()
            )
        })?;
        sync_parent(directory)?;
        cleanup_candidates(directory, &layout)?;
        let scope = normalized(scope);
        let mut store = Self {
            directory: directory.to_path_buf(),
            scope: scope.clone(),
            hot: read_hot::<T>(directory, &scope, &layout)?,
            manifest: ArchiveManifest::empty(&scope),
            layout,
            options,
        };
        store.manifest = store.read_manifest(checkpoint)?;
        store.coalesce_hot_archive_duplicates()?;
        store.repair_recent()?;
        Ok(store)
    }

    /// Seed a newly-created segmented store from a former append-only history.
    ///
    /// Exact duplicate rows are retained once. Calling this again after a
    /// process death is safe because each append checks hot and sparse archive
    /// candidates before publishing.
    pub fn import_legacy<F>(
        &mut self,
        records: impl IntoIterator<Item = T>,
        checkpoint: &mut F,
    ) -> Result<(), String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        for record in records {
            self.append_with(record, checkpoint)?;
        }
        Ok(())
    }

    pub fn append(&mut self, record: T) -> Result<bool, String> {
        self.append_with(record, &mut |_| Ok(()))
    }

    pub fn append_with<F>(&mut self, mut record: T, checkpoint: &mut F) -> Result<bool, String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        record.relocate(&self.scope, &self.scope);
        if self.hot.records.iter().any(|existing| existing == &record)
            || self
                .archived_for(record.history_partition())?
                .iter()
                .any(|existing| existing == &record)
        {
            return Ok(false);
        }
        self.hot.records.push(record);
        self.persist_hot()?;
        checkpoint("after_hot_append")?;
        self.compact(checkpoint)?;
        self.persist_recent()?;
        Ok(true)
    }

    pub fn records_for(&self, partition: &str) -> Result<Vec<T>, String> {
        let mut records = self.archived_for(partition)?;
        records.extend(
            self.hot
                .records
                .iter()
                .filter(|record| record.history_partition() == partition)
                .cloned(),
        );
        Ok(records)
    }

    pub fn recent(&self) -> Vec<T> {
        let start = self
            .hot
            .records
            .len()
            .saturating_sub(self.options.recent_limit);
        self.hot.records[start..].to_vec()
    }

    pub fn status(&self) -> SegmentedHistoryStatus {
        SegmentedHistoryStatus {
            generation: self.manifest.generation,
            segment_count: self.manifest.segments.len(),
            archived_records: self
                .manifest
                .segments
                .iter()
                .map(|descriptor| descriptor.record_count)
                .sum(),
            hot_records: self.hot.records.len(),
            manifest_revision: self.manifest.revision,
            hot_revision: self.hot.revision,
        }
    }

    fn recent_path(&self) -> PathBuf {
        self.directory.join(&self.layout.recent_file)
    }

    fn hot_path(&self) -> PathBuf {
        self.directory.join(&self.layout.hot_file)
    }

    fn hot_recovery_path(&self) -> PathBuf {
        self.directory.join(&self.layout.hot_recovery_file)
    }

    fn manifest_path(&self) -> PathBuf {
        self.directory.join(&self.layout.manifest_file)
    }

    fn manifest_recovery_path(&self) -> PathBuf {
        self.directory.join(&self.layout.manifest_recovery_file)
    }

    fn archive_directory(&self) -> PathBuf {
        self.directory.join(&self.layout.archive_directory)
    }

    fn partition_prefix(&self, partition: &str) -> String {
        sha256(partition.as_bytes())[..self.options.partition_prefix_hex].to_string()
    }

    fn archived_candidates(&self, partition: &str) -> Vec<&SegmentDescriptor> {
        let prefix = self.partition_prefix(partition);
        let Some(ids) = self.manifest.partition_index.get(&prefix) else {
            return Vec::new();
        };
        let ids = ids.iter().copied().collect::<BTreeSet<_>>();
        self.manifest
            .segments
            .iter()
            .filter(|descriptor| ids.contains(&descriptor.id))
            .collect()
    }

    fn archived_for(&self, partition: &str) -> Result<Vec<T>, String> {
        let mut records = Vec::new();
        for descriptor in self.archived_candidates(partition) {
            let segment = self.read_segment(descriptor)?;
            records.extend(
                segment
                    .records
                    .into_iter()
                    .filter(|record| record.history_partition() == partition),
            );
        }
        Ok(records)
    }

    fn persist_hot(&mut self) -> Result<(), String> {
        self.hot.version = STORE_VERSION;
        self.hot.scope = self.scope.clone();
        self.hot.revision = self.hot.revision.saturating_add(1);
        let bytes = serde_json::to_vec_pretty(&self.hot)
            .map_err(|error| format!("serialize segmented history hot index: {error}"))?;
        for path in [self.hot_recovery_path(), self.hot_path()] {
            crate::durable_file::replace(&path, &bytes).map_err(|error| {
                format!(
                    "publish segmented history hot replica {}: {error}",
                    path.display()
                )
            })?;
        }
        Ok(())
    }

    fn persist_manifest(&mut self) -> Result<(), String> {
        self.manifest.version = STORE_VERSION;
        self.manifest.scope = self.scope.clone();
        self.manifest.revision = self.manifest.revision.saturating_add(1);
        let bytes = serde_json::to_vec_pretty(&self.manifest)
            .map_err(|error| format!("serialize segmented history manifest: {error}"))?;
        for path in [self.manifest_recovery_path(), self.manifest_path()] {
            crate::durable_file::replace(&path, &bytes).map_err(|error| {
                format!(
                    "publish segmented history manifest replica {}: {error}",
                    path.display()
                )
            })?;
        }
        Ok(())
    }

    fn persist_recent(&self) -> Result<(), String> {
        let mut bytes = Vec::new();
        for record in self.recent() {
            serde_json::to_writer(&mut bytes, &record)
                .map_err(|error| format!("serialize segmented history recent row: {error}"))?;
            bytes.push(b'\n');
        }
        crate::durable_file::replace(&self.recent_path(), &bytes).map_err(|error| {
            format!(
                "publish segmented history recent window {}: {error}",
                self.recent_path().display()
            )
        })
    }

    fn repair_recent(&self) -> Result<(), String> {
        let path = self.recent_path();
        let disk = match std::fs::read(&path) {
            Ok(bytes) => parse_recent::<T>(&bytes, &self.scope),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => {
                return Err(format!(
                    "read segmented history recent window {}: {error}",
                    path.display()
                ))
            }
        };
        if disk.as_ref() != Some(&self.recent()) {
            self.persist_recent()?;
        }
        Ok(())
    }

    fn compact<F>(&mut self, checkpoint: &mut F) -> Result<(), String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        let archive_count = self
            .hot
            .records
            .len()
            .saturating_sub(self.options.hot_limit);
        if archive_count > 0 {
            let records = self.hot.records[..archive_count].to_vec();
            for chunk in records.chunks(self.options.segment_record_limit) {
                let id = self.manifest.next_segment_id.max(1);
                let descriptor =
                    self.stage_segment(id, self.manifest.generation, chunk, checkpoint)?;
                self.manifest.segments.push(descriptor);
                self.manifest.segments.sort_by_key(|candidate| candidate.id);
                self.manifest.next_segment_id = id.saturating_add(1);
                self.rebuild_partition_index();
                self.persist_manifest()?;
                checkpoint("after_manifest_publish")?;
            }
            self.hot.records.drain(..archive_count);
            self.persist_hot()?;
            checkpoint("after_hot_prune")?;
        }
        if self.manifest.segments.len() >= self.options.generation_segment_limit {
            self.compact_generation(checkpoint)?;
        }
        Ok(())
    }

    fn compact_generation<F>(&mut self, checkpoint: &mut F) -> Result<(), String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        if self.manifest.segments.len() < 2 {
            return Ok(());
        }
        let predecessor = self.manifest.segments.clone();
        let generation = self.manifest.generation.saturating_add(1);
        let mut next_id = self.manifest.next_segment_id.max(1);
        let mut all = Vec::new();
        for descriptor in &predecessor {
            all.extend(self.read_segment(descriptor)?.records);
        }
        let mut replacement_segments = Vec::new();
        for chunk in all.chunks(self.options.generation_record_limit) {
            let descriptor = self.stage_segment(next_id, generation, chunk, checkpoint)?;
            replacement_segments.push(descriptor);
            next_id = next_id.saturating_add(1);
        }
        let predecessor_files = predecessor
            .iter()
            .map(|descriptor| descriptor.file.clone())
            .collect::<Vec<_>>();
        self.manifest.generation = generation;
        self.manifest.next_segment_id = next_id;
        self.manifest.segments = replacement_segments;
        self.rebuild_partition_index();
        self.persist_manifest()?;
        checkpoint("after_generation_manifest_publish")?;
        self.garbage_collect(&predecessor_files, checkpoint)
    }

    fn stage_segment<F>(
        &self,
        id: u64,
        generation: u64,
        records: &[T],
        checkpoint: &mut F,
    ) -> Result<SegmentDescriptor, String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        if records.is_empty() {
            return Err("cannot stage an empty segmented history segment".into());
        }
        let segment = ArchiveSegment {
            version: STORE_VERSION,
            scope: self.scope.clone(),
            id,
            generation,
            records: records.to_vec(),
        };
        let bytes = serde_json::to_vec_pretty(&segment)
            .map_err(|error| format!("serialize segmented history segment: {error}"))?;
        let digest = sha256(&bytes);
        let file = format!("segment-g{generation:020}-{id:020}-{digest}.json");
        let directory = self.archive_directory();
        std::fs::create_dir_all(&directory).map_err(|error| {
            format!(
                "create segmented history archive {}: {error}",
                directory.display()
            )
        })?;
        sync_parent(&directory)?;
        let destination = directory.join(&file);
        if destination.exists() {
            let existing = std::fs::read(&destination).map_err(|error| {
                format!(
                    "read existing segmented history segment {}: {error}",
                    destination.display()
                )
            })?;
            if existing != bytes {
                return Err(format!(
                    "immutable segmented history segment disagrees at {}",
                    destination.display()
                ));
            }
        } else {
            let sequence = SEGMENT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let temporary = self.directory.join(format!(
                ".history-segment.{}.{sequence}.tmp",
                std::process::id()
            ));
            let result = (|| -> Result<(), String> {
                let mut candidate = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(&temporary)
                    .map_err(|error| {
                        format!(
                            "create segmented history candidate {}: {error}",
                            temporary.display()
                        )
                    })?;
                candidate.write_all(&bytes).map_err(|error| {
                    format!(
                        "write segmented history candidate {}: {error}",
                        temporary.display()
                    )
                })?;
                checkpoint("after_segment_candidate_write")?;
                candidate.sync_all().map_err(|error| {
                    format!(
                        "sync segmented history candidate {}: {error}",
                        temporary.display()
                    )
                })?;
                checkpoint("after_segment_candidate_fsync")?;
                std::fs::rename(&temporary, &destination).map_err(|error| {
                    format!(
                        "publish segmented history segment {}: {error}",
                        destination.display()
                    )
                })?;
                checkpoint("after_segment_rename")?;
                File::open(&directory)
                    .and_then(|archive| archive.sync_all())
                    .map_err(|error| {
                        format!(
                            "sync segmented history archive {}: {error}",
                            directory.display()
                        )
                    })?;
                checkpoint("after_segment_parent_fsync")
            })();
            if let Err(error) = result {
                let _ = std::fs::remove_file(&temporary);
                return Err(error);
            }
        }
        self.descriptor_for(file, &bytes)
            .map(|(descriptor, _)| descriptor)
    }

    fn descriptor_for(
        &self,
        file: String,
        bytes: &[u8],
    ) -> Result<(SegmentDescriptor, ArchiveSegment<T>), String> {
        if Path::new(&file)
            .file_name()
            .and_then(|value| value.to_str())
            != Some(file.as_str())
            || !file.starts_with("segment-g")
            || !file.ends_with(".json")
        {
            return Err(format!("unsafe segmented history segment name {file}"));
        }
        let segment: ArchiveSegment<T> = serde_json::from_slice(bytes)
            .map_err(|error| format!("parse segmented history segment {file}: {error}"))?;
        if segment.version != STORE_VERSION
            || segment.scope.as_os_str().is_empty()
            || segment.id == 0
            || segment.records.is_empty()
            || segment
                .records
                .iter()
                .any(|record| record.history_partition().is_empty())
        {
            return Err(format!("invalid segmented history segment {file}"));
        }
        let digest = sha256(bytes);
        let expected = format!(
            "segment-g{:020}-{:020}-{digest}.json",
            segment.generation, segment.id
        );
        if file != expected {
            return Err(format!(
                "segmented history segment filename digest mismatch: {file}"
            ));
        }
        let mut prefixes = segment
            .records
            .iter()
            .map(|record| self.partition_prefix(record.history_partition()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        prefixes.sort();
        Ok((
            SegmentDescriptor {
                id: segment.id,
                generation: segment.generation,
                file,
                sha256: digest,
                record_count: segment.records.len(),
                partition_prefixes: prefixes,
            },
            segment,
        ))
    }

    fn read_segment(&self, expected: &SegmentDescriptor) -> Result<ArchiveSegment<T>, String> {
        #[cfg(test)]
        SEGMENT_READS.fetch_add(1, Ordering::Relaxed);
        let path = self.archive_directory().join(&expected.file);
        let bytes = std::fs::read(&path).map_err(|error| {
            format!("read segmented history segment {}: {error}", path.display())
        })?;
        let (actual, mut segment) = self.descriptor_for(expected.file.clone(), &bytes)?;
        if &actual != expected {
            return Err(format!(
                "segmented history manifest descriptor disagrees with {}",
                path.display()
            ));
        }
        let old_scope = segment.scope.clone();
        for record in &mut segment.records {
            record.relocate(&old_scope, &self.scope);
        }
        segment.scope = self.scope.clone();
        Ok(segment)
    }

    fn rebuild_partition_index(&mut self) {
        self.manifest.partition_index = expected_partition_index(&self.manifest.segments);
    }

    fn read_manifest<F>(&mut self, checkpoint: &mut F) -> Result<ArchiveManifest, String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        let replicas = [
            read_manifest_replica(&self.manifest_path(), &self.options)?,
            read_manifest_replica(&self.manifest_recovery_path(), &self.options)?,
        ];
        let mut valid = replicas
            .iter()
            .filter_map(|(_, manifest)| manifest.as_ref())
            .cloned()
            .collect::<Vec<_>>();
        if valid.is_empty() && replicas.iter().any(|(exists, _)| *exists) {
            return Err(format!(
                "both segmented history manifest replicas are invalid in {}",
                self.directory.display()
            ));
        }
        valid.sort_by_key(|manifest| manifest.revision);
        let had_manifest = !valid.is_empty();
        let mut manifest = valid
            .last()
            .cloned()
            .unwrap_or_else(|| ArchiveManifest::empty(&self.scope));
        if valid
            .iter()
            .any(|candidate| candidate.revision == manifest.revision && candidate != &manifest)
        {
            return Err(format!(
                "segmented history manifest replicas disagree at revision {}",
                manifest.revision
            ));
        }
        for descriptor in &manifest.segments {
            if !self.archive_directory().join(&descriptor.file).is_file() {
                return Err(format!(
                    "segmented history manifest references missing segment {}",
                    descriptor.file
                ));
            }
        }

        let referenced = manifest
            .segments
            .iter()
            .map(|descriptor| descriptor.file.as_str())
            .collect::<BTreeSet<_>>();
        let mut orphans = Vec::new();
        for file in self.archive_files()? {
            if referenced.contains(file.as_str()) {
                continue;
            }
            let bytes = std::fs::read(self.archive_directory().join(&file))
                .map_err(|error| format!("read segmented history orphan {file}: {error}"))?;
            orphans.push(self.descriptor_for(file, &bytes)?.0);
        }

        // With no manifest, the lowest complete predecessor generation is the
        // only generation that was definitely published before any staged
        // replacement. Higher generations are uncommitted replacement
        // candidates and become garbage only after the reconstructed manifest
        // is durably replicated.
        if !had_manifest && !orphans.is_empty() {
            manifest.generation = orphans
                .iter()
                .map(|descriptor| descriptor.generation)
                .min()
                .expect("non-empty orphan generations");
        }

        let mut garbage = Vec::new();
        let mut changed = manifest.scope != self.scope
            || replicas
                .iter()
                .any(|(_, candidate)| candidate.as_ref() != Some(&manifest));
        manifest.scope = self.scope.clone();
        for descriptor in orphans {
            if descriptor.generation != manifest.generation {
                garbage.push(descriptor.file);
                continue;
            }
            match manifest
                .segments
                .iter()
                .find(|candidate| candidate.id == descriptor.id)
            {
                Some(existing) if existing == &descriptor => {}
                Some(_) => {
                    return Err(format!(
                        "conflicting immutable segmented history segment {} in generation {}",
                        descriptor.id, descriptor.generation
                    ))
                }
                None => {
                    manifest.segments.push(descriptor);
                    changed = true;
                }
            }
        }
        manifest.segments.sort_by_key(|descriptor| descriptor.id);
        manifest.next_segment_id = manifest
            .segments
            .last()
            .map(|descriptor| descriptor.id.saturating_add(1))
            .unwrap_or(1)
            .max(manifest.next_segment_id);
        let expected_index = expected_partition_index(&manifest.segments);
        if manifest.partition_index != expected_index {
            manifest.partition_index = expected_index;
            changed = true;
        }
        self.manifest = manifest;
        if changed || !had_manifest {
            self.persist_manifest()?;
            checkpoint("after_recovery_manifest_publish")?;
        }
        self.garbage_collect(&garbage, checkpoint)?;
        Ok(self.manifest.clone())
    }

    fn archive_files(&self) -> Result<Vec<String>, String> {
        let directory = self.archive_directory();
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => {
                return Err(format!(
                    "read segmented history archive {}: {error}",
                    directory.display()
                ))
            }
        };
        let mut files = Vec::new();
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("read segmented history archive entry: {error}"))?;
            if !entry
                .file_type()
                .map_err(|error| format!("inspect segmented history archive entry: {error}"))?
                .is_file()
            {
                continue;
            }
            let file = entry.file_name().to_string_lossy().into_owned();
            if file.starts_with("segment-g") && file.ends_with(".json") {
                files.push(file);
            }
        }
        files.sort();
        Ok(files)
    }

    fn garbage_collect<F>(&self, files: &[String], checkpoint: &mut F) -> Result<(), String>
    where
        F: FnMut(&str) -> Result<(), String>,
    {
        if files.is_empty() {
            return Ok(());
        }
        let directory = self.archive_directory();
        for file in files {
            let path = directory.join(file);
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    File::open(&directory)
                        .and_then(|archive| archive.sync_all())
                        .map_err(|error| {
                            format!(
                                "sync segmented history GC directory {}: {error}",
                                directory.display()
                            )
                        })?;
                    checkpoint("after_gc_delete")?;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(format!(
                        "remove segmented history garbage {}: {error}",
                        path.display()
                    ))
                }
            }
        }
        Ok(())
    }

    fn coalesce_hot_archive_duplicates(&mut self) -> Result<(), String> {
        if self.manifest.segments.is_empty() || self.hot.records.is_empty() {
            return Ok(());
        }
        let mut repaired = Vec::with_capacity(self.hot.records.len());
        for record in &self.hot.records {
            if self
                .archived_for(record.history_partition())?
                .iter()
                .any(|archived| archived == record)
            {
                continue;
            }
            if !repaired.iter().any(|existing| existing == record) {
                repaired.push(record.clone());
            }
        }
        if repaired != self.hot.records {
            self.hot.records = repaired;
            self.persist_hot()?;
        }
        Ok(())
    }
}

fn parse_recent<T: SegmentedHistoryRecord>(bytes: &[u8], scope: &Path) -> Option<Vec<T>> {
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        return None;
    }
    let mut records = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| line.iter().any(|byte| !byte.is_ascii_whitespace()))
    {
        let mut record = serde_json::from_slice::<T>(line).ok()?;
        record.relocate(scope, scope);
        records.push(record);
    }
    Some(records)
}

fn read_hot<T: SegmentedHistoryRecord>(
    directory: &Path,
    scope: &Path,
    layout: &SegmentedHistoryLayout,
) -> Result<HotIndex<T>, String> {
    let replicas = [
        read_hot_replica::<T>(&directory.join(&layout.hot_file), scope)?,
        read_hot_replica::<T>(&directory.join(&layout.hot_recovery_file), scope)?,
    ];
    let mut valid = replicas
        .iter()
        .filter_map(|(_, index, _)| index.as_ref())
        .cloned()
        .collect::<Vec<_>>();
    if valid.is_empty() && replicas.iter().any(|(exists, _, _)| *exists) {
        return Err(format!(
            "both segmented history hot replicas are invalid in {}",
            directory.display()
        ));
    }
    valid.sort_by_key(|index| index.revision);
    let mut selected = valid
        .last()
        .cloned()
        .unwrap_or_else(|| HotIndex::empty(scope));
    if valid
        .iter()
        .any(|candidate| candidate.revision == selected.revision && candidate != &selected)
    {
        return Err(format!(
            "segmented history hot replicas disagree at revision {}",
            selected.revision
        ));
    }
    let mut unique = Vec::with_capacity(selected.records.len());
    for record in selected.records {
        if !unique.iter().any(|existing| existing == &record) {
            unique.push(record);
        }
    }
    selected.records = unique;
    let needs_repair = replicas
        .iter()
        .any(|(_, candidate, relocated)| *relocated || candidate.as_ref() != Some(&selected));
    selected.scope = normalized(scope);
    if needs_repair && !valid.is_empty() {
        let bytes = serde_json::to_vec_pretty(&selected)
            .map_err(|error| format!("serialize repaired segmented history hot index: {error}"))?;
        for path in [
            directory.join(&layout.hot_recovery_file),
            directory.join(&layout.hot_file),
        ] {
            crate::durable_file::replace(&path, &bytes).map_err(|error| {
                format!(
                    "repair segmented history hot replica {}: {error}",
                    path.display()
                )
            })?;
        }
    }
    Ok(selected)
}

fn read_hot_replica<T: SegmentedHistoryRecord>(
    path: &Path,
    scope: &Path,
) -> Result<(bool, Option<HotIndex<T>>, bool), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((false, None, false)),
        Err(error) => {
            return Err(format!(
                "read segmented history hot replica {}: {error}",
                path.display()
            ))
        }
    };
    let Some(mut index) = serde_json::from_slice::<HotIndex<T>>(&bytes)
        .ok()
        .filter(|index| {
            index.version == STORE_VERSION
                && !index.scope.as_os_str().is_empty()
                && index
                    .records
                    .iter()
                    .all(|record| !record.history_partition().is_empty())
        })
    else {
        return Ok((true, None, false));
    };
    let old_scope = index.scope.clone();
    let relocated = normalized(&old_scope) != normalized(scope);
    if relocated {
        for record in &mut index.records {
            record.relocate(&old_scope, scope);
        }
        index.scope = normalized(scope);
    }
    Ok((true, Some(index), relocated))
}

fn read_manifest_replica(
    path: &Path,
    options: &SegmentedHistoryOptions,
) -> Result<(bool, Option<ArchiveManifest>), String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((false, None)),
        Err(error) => {
            return Err(format!(
                "read segmented history manifest replica {}: {error}",
                path.display()
            ))
        }
    };
    let manifest = serde_json::from_slice::<ArchiveManifest>(&bytes)
        .ok()
        .filter(|manifest| {
            manifest.version == STORE_VERSION
                && !manifest.scope.as_os_str().is_empty()
                && manifest.next_segment_id > 0
                && manifest
                    .segments
                    .iter()
                    .all(|descriptor| descriptor.generation == manifest.generation)
                && manifest
                    .segments
                    .iter()
                    .map(|descriptor| descriptor.id)
                    .collect::<BTreeSet<_>>()
                    .len()
                    == manifest.segments.len()
                && manifest
                    .segments
                    .iter()
                    .map(|descriptor| descriptor.file.as_str())
                    .collect::<BTreeSet<_>>()
                    .len()
                    == manifest.segments.len()
                && manifest.segments.iter().all(|descriptor| {
                    descriptor.id > 0
                        && descriptor.record_count > 0
                        && descriptor.partition_prefixes.iter().all(|prefix| {
                            prefix.len() == options.partition_prefix_hex
                                && prefix.bytes().all(|byte| byte.is_ascii_hexdigit())
                                && prefix.bytes().all(|byte| !byte.is_ascii_uppercase())
                        })
                })
                && manifest.partition_index == expected_partition_index(&manifest.segments)
        });
    Ok((true, manifest))
}

fn expected_partition_index(descriptors: &[SegmentDescriptor]) -> BTreeMap<String, Vec<u64>> {
    let mut index = BTreeMap::<String, Vec<u64>>::new();
    for descriptor in descriptors {
        for prefix in &descriptor.partition_prefixes {
            index.entry(prefix.clone()).or_default().push(descriptor.id);
        }
    }
    for ids in index.values_mut() {
        ids.sort_unstable();
        ids.dedup();
    }
    index
}

fn cleanup_candidates(directory: &Path, layout: &SegmentedHistoryLayout) -> Result<(), String> {
    let names = [
        layout.recent_file.as_str(),
        layout.hot_file.as_str(),
        layout.hot_recovery_file.as_str(),
        layout.manifest_file.as_str(),
        layout.manifest_recovery_file.as_str(),
    ];
    let mut removed = false;
    for entry in std::fs::read_dir(directory)
        .map_err(|error| format!("read segmented history directory: {error}"))?
    {
        let entry =
            entry.map_err(|error| format!("read segmented history directory entry: {error}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let durable_candidate = names
            .iter()
            .any(|target| name.starts_with(&format!(".{target}.")));
        if entry
            .file_type()
            .map_err(|error| format!("inspect segmented history candidate: {error}"))?
            .is_file()
            && name.ends_with(".tmp")
            && (durable_candidate || name.starts_with(".history-segment."))
        {
            std::fs::remove_file(entry.path()).map_err(|error| {
                format!(
                    "remove interrupted segmented history candidate {}: {error}",
                    entry.path().display()
                )
            })?;
            removed = true;
        }
    }
    if removed {
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|error| {
                format!(
                    "sync cleaned segmented history directory {}: {error}",
                    directory.display()
                )
            })?;
    }
    Ok(())
}

fn normalized(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn sync_parent(path: &Path) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("segmented history path has no parent: {}", path.display()))?;
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "sync segmented history parent {}: {error}",
                parent.display()
            )
        })
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
static SEGMENT_READS: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
    struct Record {
        partition: String,
        sequence: u64,
        root: PathBuf,
        value: String,
    }

    impl SegmentedHistoryRecord for Record {
        fn history_partition(&self) -> &str {
            &self.partition
        }

        fn relocate(&mut self, old_scope: &Path, new_scope: &Path) {
            if self.root == normalized(old_scope) {
                self.root = normalized(new_scope);
            }
        }
    }

    fn record(root: &Path, partition: &str, sequence: u64) -> Record {
        Record {
            partition: partition.into(),
            sequence,
            root: normalized(root),
            value: format!("value-{sequence}"),
        }
    }

    fn small_options() -> SegmentedHistoryOptions {
        SegmentedHistoryOptions {
            recent_limit: 3,
            hot_limit: 3,
            segment_record_limit: 1,
            generation_segment_limit: 4,
            generation_record_limit: 8,
            partition_prefix_hex: 8,
        }
    }

    #[test]
    fn bounded_recent_sparse_query_and_generation_compaction() {
        let temp = tempfile::tempdir().unwrap();
        let scope = temp.path().join("project");
        std::fs::create_dir_all(&scope).unwrap();
        let directory = scope.join("transactions");
        let layout = SegmentedHistoryLayout::default();
        let mut store =
            SegmentedHistory::open(&directory, &scope, layout.clone(), small_options()).unwrap();
        for sequence in 0..12 {
            store
                .append(record(&scope, &format!("batch-{sequence}"), sequence))
                .unwrap();
        }
        assert_eq!(store.recent().len(), 3);
        assert_eq!(
            std::fs::read_to_string(directory.join(&layout.recent_file))
                .unwrap()
                .lines()
                .count(),
            3
        );
        assert!(store.status().generation >= 1);
        SEGMENT_READS.store(0, Ordering::Relaxed);
        assert_eq!(store.records_for("batch-0").unwrap()[0].sequence, 0);
        assert_eq!(SEGMENT_READS.load(Ordering::Relaxed), 1);
        assert_eq!(store.append(record(&scope, "batch-0", 0)).unwrap(), false);
    }

    #[test]
    fn relocation_missing_segment_and_index_conflict_fail_closed() {
        let temp = tempfile::tempdir().unwrap();
        let old_scope = temp.path().join("before");
        std::fs::create_dir_all(&old_scope).unwrap();
        let layout = SegmentedHistoryLayout::default();
        let directory = old_scope.join("transactions");
        let mut store =
            SegmentedHistory::open(&directory, &old_scope, layout.clone(), small_options())
                .unwrap();
        for sequence in 0..5 {
            store
                .append(record(&old_scope, &format!("move-{sequence}"), sequence))
                .unwrap();
        }
        drop(store);
        let new_scope = temp.path().join("after");
        std::fs::rename(&old_scope, &new_scope).unwrap();
        let moved_directory = new_scope.join("transactions");
        let moved = SegmentedHistory::<Record>::open(
            &moved_directory,
            &new_scope,
            layout.clone(),
            small_options(),
        )
        .unwrap();
        assert_eq!(
            moved.records_for("move-0").unwrap()[0].root,
            normalized(&new_scope)
        );
        let descriptor = moved.manifest.segments[0].clone();
        drop(moved);
        std::fs::remove_file(
            moved_directory
                .join(&layout.archive_directory)
                .join(&descriptor.file),
        )
        .unwrap();
        let missing = SegmentedHistory::<Record>::open(
            &moved_directory,
            &new_scope,
            layout.clone(),
            small_options(),
        )
        .unwrap_err();
        assert!(missing.contains("references missing segment"));

        // Restore the immutable bytes by rebuilding in a separate store, then
        // prove same-revision manifest disagreement is rejected.
        let conflict_scope = temp.path().join("conflict");
        std::fs::create_dir_all(&conflict_scope).unwrap();
        let conflict_dir = conflict_scope.join("transactions");
        let mut store = SegmentedHistory::open(
            &conflict_dir,
            &conflict_scope,
            layout.clone(),
            small_options(),
        )
        .unwrap();
        for sequence in 0..5 {
            store
                .append(record(
                    &conflict_scope,
                    &format!("conflict-{sequence}"),
                    sequence,
                ))
                .unwrap();
        }
        let mut conflicting = store.manifest.clone();
        conflicting.next_segment_id = conflicting.next_segment_id.saturating_add(1);
        let bytes = serde_json::to_vec_pretty(&conflicting).unwrap();
        std::fs::write(conflict_dir.join(&layout.manifest_recovery_file), bytes).unwrap();
        let error = SegmentedHistory::<Record>::open(
            &conflict_dir,
            &conflict_scope,
            layout,
            small_options(),
        )
        .unwrap_err();
        assert!(error.contains("replicas disagree at revision"));
    }

    #[test]
    fn multiple_orphan_generations_recover_predecessor_then_gc_future() {
        let temp = tempfile::tempdir().unwrap();
        let scope = temp.path().join("project");
        std::fs::create_dir_all(&scope).unwrap();
        let directory = scope.join("transactions");
        let layout = SegmentedHistoryLayout::default();
        let options = small_options();
        let mut store =
            SegmentedHistory::open(&directory, &scope, layout.clone(), options.clone()).unwrap();
        let predecessor = store
            .stage_segment(1, 3, &[record(&scope, "safe", 1)], &mut |_| Ok(()))
            .unwrap();
        let future = store
            .stage_segment(2, 4, &[record(&scope, "future", 2)], &mut |_| Ok(()))
            .unwrap();
        for path in [store.manifest_path(), store.manifest_recovery_path()] {
            let _ = std::fs::remove_file(path);
        }
        drop(store);

        let recovered =
            SegmentedHistory::<Record>::open(&directory, &scope, layout.clone(), options).unwrap();
        assert_eq!(recovered.status().generation, 3);
        assert_eq!(recovered.records_for("safe").unwrap()[0].value, "value-1");
        assert!(recovered.records_for("future").unwrap().is_empty());
        assert!(directory
            .join(&layout.archive_directory)
            .join(predecessor.file)
            .is_file());
        assert!(!directory
            .join(&layout.archive_directory)
            .join(future.file)
            .exists());
    }
}
