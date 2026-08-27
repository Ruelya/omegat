//! Java `RealProject` — open / save / compile / apply TM / tag validation.

use crate::cancellation::CancellationToken;
use crate::consts::*;
use crate::error::{CoreError, Result};
use crate::external_tm::folder_is;
use crate::glossary::{self, GlossaryEntry};
use crate::last_segment;
use crate::matching;
use crate::prefs::Preferences;
use crate::properties::ProjectProperties;
pub use crate::source_text_entry::Entry;
use crate::spell::SpellChecker;
use crate::tags;
use crate::tmx::{ProjectTmx, TmxEntry};
use crate::{align, dict, mt, search, stats};
use fs2::FileExt;
use omegat_filters::{FilterContext, FilterRegistry};
use omegat_ipc::*;
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct ProjectSession {
    pub props: ProjectProperties,
    pub entries: Vec<Entry>,
    pub tmx: ProjectTmx,
    pub external_tm: Vec<(TmxEntry, String)>,
    pub glossary: Vec<GlossaryEntry>,
    pub prefs: Preferences,
    pub spell: SpellChecker,
    pub mt_cache: mt::MtCache,
    pub filters: FilterRegistry,
    pub last_index: usize,
    _lock: Option<File>,
    dirty: bool,
}

impl ProjectSession {
    pub fn create(params: &CreateProjectParams, prefs: Preferences) -> Result<Self> {
        let root = PathBuf::from(&params.root);
        std::fs::create_dir_all(&root)?;
        let props = ProjectProperties::create(
            root,
            params.source_lang.clone(),
            params.target_lang.clone(),
            params.sentence_seg,
        );
        props.ensure_dirs()?;
        props.write()?;
        Self::open_props(props, prefs, FilterRegistry::new())
    }

    pub fn create_with_filters(
        params: &CreateProjectParams,
        prefs: Preferences,
        filters: FilterRegistry,
    ) -> Result<Self> {
        let root = PathBuf::from(&params.root);
        std::fs::create_dir_all(&root)?;
        let props = ProjectProperties::create(
            root,
            params.source_lang.clone(),
            params.target_lang.clone(),
            params.sentence_seg,
        );
        props.ensure_dirs()?;
        props.write()?;
        Self::open_props(props, prefs, filters)
    }

    pub fn open(root: &Path, prefs: Preferences) -> Result<Self> {
        let props = ProjectProperties::load(root)?;
        Self::open_props(props, prefs, FilterRegistry::new())
    }

    pub fn open_with_filters(
        root: &Path,
        prefs: Preferences,
        filters: FilterRegistry,
    ) -> Result<Self> {
        let props = ProjectProperties::load(root)?;
        Self::open_props(props, prefs, filters)
    }

    fn open_props(
        props: ProjectProperties,
        prefs: Preferences,
        filters: FilterRegistry,
    ) -> Result<Self> {
        props.ensure_dirs()?;
        let lock_path = props.root.join(DEFAULT_INTERNAL).join(".lock");
        let lock_file = File::create(&lock_path)?;
        lock_file.try_lock_exclusive().map_err(|_| {
            CoreError::InvalidProject("project is locked by another process".into())
        })?;

        let tmx = ProjectTmx::load(
            &props.save_tmx_path(),
            &props.source_lang,
            &props.target_lang,
        )?;
        let external_tm = crate::external_tm::load_external_tm(&props);
        let glossary = glossary::load_glossary(&props.glossary_file);
        let backend = match prefs.spell_backend.as_str() {
            "lucene" => crate::spell::SpellBackend::Lucene,
            "morfologik" => crate::spell::SpellBackend::Morfologik,
            _ => crate::spell::SpellBackend::Hunspell,
        };
        let dest = prefs.config_dir.join("spell").join("hunspell");
        let _ = crate::spell::ensure_lang(&props.target_lang, &dest);
        let spell = SpellChecker::load_backend(&props.root, &prefs.config_dir, backend);
        let mut session = Self {
            props,
            entries: vec![],
            tmx,
            external_tm,
            glossary,
            prefs,
            spell,
            mt_cache: mt::MtCache::default(),
            filters,
            last_index: 0,
            _lock: Some(lock_file),
            dirty: false,
        };
        session.reload_sources()?;
        session.apply_memory();
        session.last_index = last_segment::load_last_index(&session.props.root);
        Ok(session)
    }

    pub fn reload(&mut self) -> Result<()> {
        self.reload_sources()?;
        self.apply_memory();
        Ok(())
    }

    /// Reload every project-owned input after a filesystem or team update.
    ///
    /// Ordinary reload keeps the in-memory TM/glossary objects because it is
    /// used after local editor commits. External refresh must instead adopt
    /// the on-disk project file, project TM, external TMs, and glossary before
    /// rebuilding source entries.
    pub fn refresh_external(&mut self) -> Result<()> {
        let root = self.props.root.clone();
        let props = ProjectProperties::load(&root)?;
        props.ensure_dirs()?;
        let tmx = ProjectTmx::load(
            &props.save_tmx_path(),
            &props.source_lang,
            &props.target_lang,
        )?;
        let external_tm = crate::external_tm::load_external_tm(&props);
        let glossary = glossary::load_glossary(&props.glossary_file);
        self.props = props;
        self.tmx = tmx;
        self.external_tm = external_tm;
        self.glossary = glossary;
        self.reload_sources()?;
        self.apply_memory();
        Ok(())
    }

    fn filter_ctx(&self) -> FilterContext {
        let mut options = self.prefs.filter_context.clone();
        if self.prefs.remove_tags {
            options.insert("remove_tags".into(), "true".into());
        }
        for opts in self.prefs.filter_options.values() {
            for (k, v) in opts {
                options.insert(k.clone(), v.clone());
            }
        }
        FilterContext {
            source_lang: self.props.source_lang.clone(),
            target_lang: self.props.target_lang.clone(),
            in_encoding: None,
            out_encoding: None,
            remove_tags: self.props.remove_tags || self.prefs.remove_tags,
            remove_spaces_nonseg: true,
            options,
        }
    }

    fn reload_sources(&mut self) -> Result<()> {
        self.entries.clear();
        let ctx = self.filter_ctx();
        let excludes = build_excludes(&self.props.source_dir_excludes);
        for file in walk_sources(&self.props.source_dir, &excludes) {
            let rel = file
                .strip_prefix(&self.props.source_dir)
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");
            let Some(filter) = self.filters.for_path(&file) else {
                continue;
            };
            let parsed = filter.parse(&file, &ctx)?;
            let mut file_entries = Vec::new();
            for seg in parsed.segments {
                let custom = (!self.prefs.srx_path.is_empty())
                    .then(|| std::fs::read_to_string(&self.prefs.srx_path).ok())
                    .flatten()
                    .map(|raw| crate::segment::parse_srx(&raw, &self.props.source_lang));
                for sentence in crate::segment::split_sentences_lang(
                    &seg.source,
                    self.props.sentence_seg,
                    &self.props.source_lang,
                    custom.as_ref(),
                ) {
                    if sentence.trim().is_empty() {
                        continue;
                    }
                    file_entries.push(Entry {
                        file: rel.clone(),
                        id: seg.id.clone(),
                        prev: None,
                        next: None,
                        path: seg.path.clone(),
                        source: sentence,
                        translation: seg.existing_translation.clone().unwrap_or_default(),
                        note: seg.note.clone().unwrap_or_default(),
                        comment: seg.comment.clone().unwrap_or_default(),
                        default_translation: true,
                        revision: 1,
                        from_tm_exact: false,
                        properties: vec![],
                    });
                }
            }
            let sources = file_entries
                .iter()
                .map(|entry| entry.source.clone())
                .collect::<Vec<_>>();
            for (index, entry) in file_entries.iter_mut().enumerate() {
                entry.prev = Some(
                    index
                        .checked_sub(1)
                        .and_then(|previous| sources.get(previous))
                        .cloned()
                        .unwrap_or_default(),
                );
                entry.next = Some(sources.get(index + 1).cloned().unwrap_or_default());
            }
            self.entries.extend(file_entries);
        }
        Ok(())
    }

    fn apply_memory(&mut self) {
        let mut enforce = HashMap::new();
        let mut auto = HashMap::new();
        let mut other_lang = HashMap::new();
        for (hit, origin) in &self.external_tm {
            let rel = origin.replace('\\', "/");
            if folder_is(&rel, ENFORCE_TM) {
                enforce.insert(hit.source.clone(), hit.clone());
            } else if folder_is(&rel, AUTO_TM) {
                auto.insert(hit.source.clone(), hit.clone());
            } else if folder_is(&rel, TMX2SOURCE) {
                other_lang.insert(hit.source.clone(), hit.translation.clone());
            }
        }
        for e in &mut self.entries {
            if let Some(alt) = other_lang.get(&e.source) {
                e.properties.push(("tmx2source".into(), alt.clone()));
            }
            if let Some(hit) = enforce.get(&e.source) {
                e.translation = hit.translation.clone();
                e.note = hit.note.clone().unwrap_or_default();
                e.from_tm_exact = true;
                e.properties.push(("tm".into(), ENFORCE_TM.into()));
                apply_tm_meta(e, hit);
                continue;
            }
            if e.translation.is_empty() {
                if let Some(hit) = self.tmx.get_translation_for_key(&e.key()) {
                    e.translation = hit.translation.clone();
                    e.note = hit.note.clone().unwrap_or_default();
                    e.default_translation = hit.default_translation;
                    e.from_tm_exact = true;
                    apply_tm_meta(e, hit);
                } else if let Some(hit) = auto.get(&e.source) {
                    e.translation = hit.translation.clone();
                    e.from_tm_exact = true;
                    e.properties.push(("tm".into(), AUTO_TM.into()));
                    apply_tm_meta(e, hit);
                }
            }
        }
    }

    pub fn save(&mut self) -> Result<()> {
        self.sync_tmx_from_entries();
        self.tmx.write(
            &self.props.save_tmx_path(),
            &self.props.source_lang,
            &self.props.target_lang,
        )?;
        self.props.write()?;
        last_segment::save_last_index(&self.props.root, self.last_index)?;
        self.dirty = false;
        Ok(())
    }

    fn sync_tmx_from_entries(&mut self) {
        for e in &self.entries {
            if e.translated() {
                self.tmx.insert(TmxEntry {
                    source: e.source.clone(),
                    translation: e.translation.clone(),
                    note: if e.note.is_empty() {
                        None
                    } else {
                        Some(e.note.clone())
                    },
                    default_translation: e.default_translation,
                    file: (!e.default_translation).then(|| e.file.clone()),
                    id: (!e.default_translation).then(|| e.id.clone()),
                    prev: (!e.default_translation).then(|| e.prev.clone()).flatten(),
                    next: (!e.default_translation).then(|| e.next.clone()).flatten(),
                    path: (!e.default_translation).then(|| e.path.clone()).flatten(),
                    changer: Some("omegat-rewrite".into()),
                    changed: Some(now_iso()),
                    ..Default::default()
                });
            }
        }
    }

    pub fn set_entry(&mut self, params: &SetEntryParams) -> Result<SetEntryResult> {
        let current = self
            .entries
            .get(params.index)
            .cloned()
            .ok_or_else(|| CoreError::InvalidProject("entry out of range".into()))?;
        if current.revision != params.revision {
            return Err(CoreError::OptimisticLock(params.index));
        }
        if params.key.as_ref().is_some_and(|key| key != &current.key()) {
            return Err(CoreError::InvalidProject(format!(
                "entry key changed at index {}",
                params.index
            )));
        }
        if !params.translation.trim().is_empty() {
            let mode = self.prefs.tag_validation.as_str();
            if mode == "abort" || mode == "warn" {
                let errs = tags::validate(&current.source, &params.translation);
                if !errs.is_empty() && mode == "abort" {
                    return Err(CoreError::TagValidation(format!(
                        "TAG_VALIDATION: segment {} failed tag validation",
                        params.index
                    )));
                }
            }
        }

        let note = params.note.clone().unwrap_or(current.note.clone());
        let changed = now_iso();
        let tmx_entry = TmxEntry {
            source: current.source.clone(),
            translation: params.translation.clone(),
            note: (!note.is_empty()).then(|| note.clone()),
            default_translation: params.default_translation,
            file: (!params.default_translation).then(|| current.file.clone()),
            id: (!params.default_translation).then(|| current.id.clone()),
            prev: (!params.default_translation)
                .then(|| current.prev.clone())
                .flatten(),
            next: (!params.default_translation)
                .then(|| current.next.clone())
                .flatten(),
            path: (!params.default_translation)
                .then(|| current.path.clone())
                .flatten(),
            changer: Some("omegat-rewrite".into()),
            changed: Some(changed.clone()),
            ..Default::default()
        };

        if params.default_translation {
            if !current.default_translation {
                self.tmx
                    .remove_occurrence_translation_for_key(&current.key());
            }
            self.tmx.insert(tmx_entry);
        } else {
            self.tmx.insert(tmx_entry);
        }

        let mut updated = Vec::new();
        for (index, entry) in self.entries.iter_mut().enumerate() {
            let affected = if params.default_translation {
                entry.source == current.source
                    && (entry.default_translation || index == params.index)
            } else {
                index == params.index
            };
            if !affected {
                continue;
            }
            entry.translation = params.translation.clone();
            entry.note = note.clone();
            entry.default_translation = params.default_translation;
            entry.revision += 1;
            entry.from_tm_exact = false;
            upsert_prop(entry, "changeid", "omegat-rewrite");
            upsert_prop(entry, "changedate", &changed);
            updated.push(entry.to_dto(index));
        }

        self.dirty = true;
        self.last_index = params.index;
        let entry = updated
            .iter()
            .find(|entry| entry.index == params.index)
            .cloned()
            .ok_or_else(|| CoreError::InvalidProject("entry update was not applied".into()))?;
        Ok(SetEntryResult { entry, updated })
    }

    pub fn matches_for(&self, index: usize) -> Vec<MatchDto> {
        let Some(e) = self.entries.get(index) else {
            return vec![];
        };
        matching::find_matches_threshold(
            &e.source,
            &self.tmx.entries,
            &self.external_tm,
            &self.props.source_lang,
            self.prefs.fuzzy_threshold,
            crate::consts::MAX_NEAR_STRINGS,
        )
        .into_iter()
        .map(|m| {
            let mut dto = m.to_dto();
            dto.similarity =
                matching::similarity_data(&e.source, &m.source, &self.props.source_lang);
            dto
        })
        .collect()
    }

    pub fn glossary_for(&self, index: usize) -> Vec<GlossaryHitDto> {
        let Some(e) = self.entries.get(index) else {
            return vec![];
        };
        let ignore_case = self.prefs.glossary_ignore_case;
        let use_stem = self.prefs.glossary_stem;
        glossary::lookup_opts_lang(
            &self.glossary,
            &e.source,
            ignore_case,
            use_stem,
            &self.props.target_lang,
        )
    }

    pub fn compile(&mut self, source_pattern: Option<&str>) -> Result<usize> {
        if self.prefs.tag_validation == "abort" {
            let bad = self
                .issues()
                .iter()
                .filter(|i| i.kind == "tag" && i.severity == "error")
                .count();
            if bad > 0 {
                return Err(CoreError::TagValidation(format!(
                    "TAG_VALIDATION: {bad} tag errors"
                )));
            }
        }
        self.save()?;
        let ctx = self.filter_ctx();
        let mut by_file: HashMap<String, Vec<&Entry>> = HashMap::new();
        for e in &self.entries {
            if let Some(pat) = source_pattern {
                if !e.file.contains(pat) {
                    continue;
                }
            }
            by_file.entry(e.file.clone()).or_default().push(e);
        }
        let mut n = 0;
        for (rel, segs) in by_file {
            let src = self.props.source_dir.join(&rel);
            let dest = self.props.target_dir.join(&rel);
            let Some(filter) = self.filters.for_path(&src) else {
                continue;
            };
            let parsed = filter.parse(&src, &ctx)?;
            let mut map = HashMap::new();
            for (i, p) in parsed.segments.iter().enumerate() {
                let trans = segs
                    .iter()
                    .filter(|e| {
                        e.source == p.source
                            || (!p.id.is_empty()
                                && (e.id == p.id || e.id.starts_with(&format!("{}:", p.id))))
                    })
                    .map(|e| e.translation.as_str())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let trans = if trans.is_empty() {
                    segs.get(i)
                        .map(|e| e.translation.clone())
                        .unwrap_or_default()
                } else {
                    trans
                };
                if trans.is_empty() {
                    continue;
                }
                map.insert(p.id.clone(), trans.clone());
                map.insert(i.to_string(), trans.clone());
                if !p.source.is_empty() {
                    map.insert(p.source.clone(), trans);
                }
            }
            filter.write(&src, &dest, &map, &ctx)?;
            n += 1;
        }
        self.export_tm_levels()?;
        Ok(n)
    }

    fn export_tm_levels(&self) -> Result<()> {
        let levels = self.props.export_tm_levels.to_ascii_lowercase();
        let stem = self
            .props
            .root
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("project");
        let dir = if self.props.export_tm_dir.as_os_str().is_empty() {
            &self.props.root
        } else {
            &self.props.export_tm_dir
        };
        for level in ["omegat", "level1", "level2"] {
            if levels.contains(level) {
                let path = dir.join(format!("{stem}-{level}.tmx"));
                std::fs::write(
                    path,
                    self.tmx
                        .to_xml_level(&self.props.source_lang, &self.props.target_lang, level),
                )?;
            }
        }
        Ok(())
    }

    pub fn stats(&self) -> StatsDto {
        stats::compute_with_memory(
            &self.entries,
            &self.tmx.entries,
            &self.props.source_lang,
            &self.props.target_lang,
        )
    }

    pub fn search(&self, params: &SearchParams) -> Vec<SearchHitDto> {
        search::search(&self.entries, params)
    }

    pub fn search_cancellable(
        &self,
        params: &SearchParams,
        cancellation: &CancellationToken,
    ) -> Option<Vec<SearchHitDto>> {
        search::search_cancellable(&self.entries, params, cancellation)
    }

    pub fn search_replace(&mut self, params: &SearchParams) -> usize {
        let n = search::replace(&mut self.entries, params);
        if n > 0 {
            self.dirty = true;
        }
        n
    }

    pub fn issues(&self) -> Vec<IssueDto> {
        let mut all = Vec::new();
        let lt = (!self.prefs.languagetool_url.is_empty())
            .then_some(self.prefs.languagetool_url.as_str());
        if lt.filter(|s| !s.is_empty()).is_none() {
            all.push(IssueDto {
                kind: "languagetool".into(),
                index: 0,
                file: String::new(),
                message: crate::languagetool::UNCONFIGURED_MESSAGE.into(),
                severity: "info".into(),
            });
        }
        for (i, e) in self.entries.iter().enumerate() {
            all.extend(tags::issues_for(i, &e.file, &e.source, &e.translation));
            if lt.filter(|s| !s.is_empty()).is_some() {
                all.extend(crate::languagetool::check(
                    lt,
                    &e.translation,
                    &self.props.target_lang,
                    i,
                    &e.file,
                ));
            }
            for w in self.spell.unknown_in(&e.translation) {
                all.push(IssueDto {
                    kind: "spell".into(),
                    index: i,
                    file: e.file.clone(),
                    message: format!("Unknown word: {w}"),
                    severity: "info".into(),
                });
            }
            if e.translated() {
                for g in glossary::lookup(&self.glossary, &e.source) {
                    if !e
                        .translation
                        .to_lowercase()
                        .contains(&g.target.to_lowercase())
                    {
                        all.push(IssueDto {
                            kind: "glossary".into(),
                            index: i,
                            file: e.file.clone(),
                            message: format!(
                                "Glossary term '{}' not used (expected '{}')",
                                g.source, g.target
                            ),
                            severity: "warn".into(),
                        });
                    }
                }
            }
        }
        all
    }

    pub fn mt(&self, index: usize, engine: &str) -> Result<MtSuggestionDto> {
        self.mt_cancellable(index, engine, &CancellationToken::default())
    }

    pub fn mt_cancellable(
        &self,
        index: usize,
        engine: &str,
        cancellation: &CancellationToken,
    ) -> Result<MtSuggestionDto> {
        let e = self
            .entries
            .get(index)
            .ok_or(CoreError::InvalidProject("entry".into()))?;
        mt::translate_with_creds_cancellable(
            engine,
            &e.source,
            &self.props.source_lang,
            &self.props.target_lang,
            &self.mt_cache,
            &mt::MtCreds::from_prefs(&self.prefs),
            cancellation,
        )
        .map_err(CoreError::Filter)
    }

    pub fn dict(&self, word: &str) -> Vec<DictHitDto> {
        dict::lookup_opts(
            &self.props.dictionary_dir,
            word,
            self.prefs.dictionary_fuzzy_matching,
        )
    }

    pub fn dict_cancellable(
        &self,
        word: &str,
        cancellation: &CancellationToken,
    ) -> Option<Vec<DictHitDto>> {
        dict::lookup_opts_cancellable(
            &self.props.dictionary_dir,
            word,
            self.prefs.dictionary_fuzzy_matching,
            cancellation,
        )
    }

    pub fn completer(
        &self,
        index: usize,
        prefix: &str,
        draft: Option<&str>,
    ) -> Vec<CompleterItemDto> {
        let mut items = Vec::new();
        if let Some(e) = self.entries.get(index) {
            if self.prefs.completer_glossary {
                for g in glossary::lookup(&self.glossary, &e.source) {
                    if g.target.to_lowercase().starts_with(&prefix.to_lowercase())
                        || prefix.is_empty()
                    {
                        items.push(CompleterItemDto {
                            kind: "glossary".into(),
                            text: g.target,
                            detail: g.source,
                        });
                    }
                }
            }
            if self.prefs.completer_tags {
                for t in tags::extract_tags(&e.source) {
                    items.push(CompleterItemDto {
                        kind: "tag".into(),
                        text: t,
                        detail: "source tag".into(),
                    });
                }
            }
            if self.prefs.completer_autotext && !self.prefs.autotext.is_empty() {
                for pair in self.prefs.autotext.split(';') {
                    if let Some((k, v)) = pair.split_once('=') {
                        if k.starts_with(prefix) || prefix.is_empty() {
                            items.push(CompleterItemDto {
                                kind: "autotext".into(),
                                text: v.to_string(),
                                detail: k.to_string(),
                            });
                        }
                    }
                }
            }
            if self.prefs.completer_chartable {
                for ch in self.prefs.chartable.chars() {
                    items.push(CompleterItemDto {
                        kind: "charset".into(),
                        text: ch.to_string(),
                        detail: "chartable".into(),
                    });
                }
            }
        }
        let translations: Vec<&str> = self
            .entries
            .iter()
            .map(|e| e.translation.as_str())
            .collect();
        if self.prefs.history_completion {
            items.extend(crate::completer::history_complete(&translations, prefix));
        }
        if self.prefs.history_prediction {
            let model = crate::completer::train_predictor(&translations);
            items.extend(crate::completer::history_predict(
                &model,
                draft.unwrap_or(prefix),
            ));
        }
        items.truncate(40);
        items
    }

    pub fn align(&self, source: &Path, target: &Path, dest: &Path) -> Result<()> {
        let tmx = align::align_files(
            source,
            target,
            &self.props.source_lang,
            &self.props.target_lang,
        )?;
        align::write_aligned_tmx(&tmx, dest, &self.props.source_lang, &self.props.target_lang)
    }

    pub fn capabilities() -> Capabilities {
        Capabilities {
            phase: 8,
            filters: FilterRegistry::new()
                .info()
                .into_iter()
                .map(|f| f.id.to_string())
                .collect(),
            features: FeatureFlags {
                project: true,
                tmx: true,
                matching: true,
                glossary: true,
                compile: true,
                search: true,
                stats: true,
                gui: true,
                filters_a: true,
                filters_b: true,
                tags: true,
                spell: true,
                languagetool: true,
                dictionary: true,
                mt: true,
                autocompleter: true,
                finder: true,
                team: true,
                aligner: true,
                script: true,
                i18n: true,
            },
        }
    }
}

fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

fn apply_tm_meta(e: &mut Entry, hit: &TmxEntry) {
    if let Some(c) = &hit.changer {
        upsert_prop(e, "changeid", c);
    }
    if let Some(d) = &hit.changed {
        upsert_prop(e, "changedate", d);
    }
}

fn upsert_prop(e: &mut Entry, key: &str, value: &str) {
    if let Some((_, v)) = e.properties.iter_mut().find(|(k, _)| k == key) {
        *v = value.to_string();
    } else {
        e.properties.push((key.to_string(), value.to_string()));
    }
}

fn build_excludes(masks: &[String]) -> globset::GlobSet {
    let mut b = globset::GlobSetBuilder::new();
    for m in masks {
        if let Ok(g) = globset::Glob::new(m) {
            b.add(g);
        }
    }
    b.build()
        .unwrap_or_else(|_| globset::GlobSetBuilder::new().build().unwrap())
}

fn walk_sources(root: &Path, excludes: &globset::GlobSet) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !root.exists() {
        return files;
    }
    for ent in walkdir::WalkDir::new(root).into_iter().flatten() {
        if !ent.file_type().is_file() {
            continue;
        }
        let rel = ent.path().strip_prefix(root).unwrap_or(ent.path());
        if excludes.is_match(rel) {
            continue;
        }
        files.push(ent.path().to_path_buf());
    }
    files.sort();
    files
}

impl Drop for ProjectSession {
    fn drop(&mut self) {
        if let Some(f) = self._lock.take() {
            let _ = FileExt::unlock(&f);
        }
    }
}
