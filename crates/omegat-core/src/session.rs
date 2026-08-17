use crate::consts::*;
use crate::error::{CoreError, Result};
use crate::glossary::{self, GlossaryEntry};
use crate::matching;
use crate::prefs::Preferences;
use crate::properties::ProjectProperties;
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

#[derive(Debug, Clone)]
pub struct Entry {
    pub file: String,
    pub id: String,
    pub source: String,
    pub translation: String,
    pub note: String,
    pub comment: String,
    pub default_translation: bool,
    pub revision: u64,
    pub from_tm_exact: bool,
    pub properties: Vec<(String, String)>,
}

impl Entry {
    pub fn translated(&self) -> bool {
        !self.translation.trim().is_empty()
    }

    pub fn to_dto(&self, index: usize) -> EntryDto {
        EntryDto {
            index,
            file: self.file.clone(),
            id: self.id.clone(),
            source: self.source.clone(),
            translation: self.translation.clone(),
            note: self.note.clone(),
            comment: self.comment.clone(),
            default_translation: self.default_translation,
            revision: self.revision,
            translated: self.translated(),
            tags: tags::extract_tags(&self.source),
            properties: self.properties.clone(),
        }
    }
}

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
        Self::open_props(props, prefs)
    }

    pub fn open(root: &Path, prefs: Preferences) -> Result<Self> {
        let props = ProjectProperties::load(root)?;
        Self::open_props(props, prefs)
    }

    fn open_props(props: ProjectProperties, prefs: Preferences) -> Result<Self> {
        props.ensure_dirs()?;
        let lock_path = props.root.join(DEFAULT_INTERNAL).join(".lock");
        let lock_file = File::create(&lock_path)?;
        lock_file.try_lock_exclusive().map_err(|_| {
            CoreError::InvalidProject("project is locked by another process".into())
        })?;

        let tmx = ProjectTmx::load(&props.save_tmx_path(), &props.source_lang, &props.target_lang)?;
        let external_tm = load_external_tm(&props);
        let glossary = glossary::load_glossary(&props.glossary_file);
        let backend = match prefs.extra.get("spell_backend").map(|s| s.as_str()) {
            Some("lucene") => crate::spell::SpellBackend::Lucene,
            Some("morfologik") => crate::spell::SpellBackend::Morfologik,
            _ => crate::spell::SpellBackend::Hunspell,
        };
        let spell = SpellChecker::load_backend(&props.root, &prefs.config_dir, backend);
        let filters = FilterRegistry::new();
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
        session.load_last_entry();
        Ok(session)
    }

    pub fn reload(&mut self) -> Result<()> {
        self.reload_sources()?;
        self.apply_memory();
        Ok(())
    }

    fn filter_ctx(&self) -> FilterContext {
        let mut options = HashMap::new();
        for (k, v) in &self.prefs.extra {
            if let Some(rest) = k.strip_prefix("filter.") {
                if let Some((_, opt)) = rest.split_once('.') {
                    options.insert(opt.to_string(), v.clone());
                }
            } else if matches!(
                k.as_str(),
                "segmentOn" | "skipHeader" | "monolingualFormat" | "remove_tags" | "preserve_spaces"
            ) {
                options.insert(k.clone(), v.clone());
            }
        }
        FilterContext {
            source_lang: self.props.source_lang.clone(),
            target_lang: self.props.target_lang.clone(),
            remove_tags: self.props.remove_tags,
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
            let nsegs = parsed.segments.len();
            for (i, seg) in parsed.segments.into_iter().enumerate() {
                let custom = self.prefs.extra.get("srx_path").and_then(|p| {
                    let raw = std::fs::read_to_string(p).ok()?;
                    Some(crate::segment::parse_srx(&raw, &self.props.source_lang))
                });
                for sentence in crate::segment::split_sentences_lang(
                    &seg.source,
                    self.props.sentence_seg,
                    &self.props.source_lang,
                    custom.as_ref(),
                ) {
                    if sentence.trim().is_empty() {
                        continue;
                    }
                    self.entries.push(Entry {
                        file: rel.clone(),
                        id: if nsegs == 1 && i == 0 {
                            seg.id.clone()
                        } else {
                            format!("{}:{}", seg.id, self.entries.len())
                        },
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
                if let Some(hit) = self.tmx.get(&e.source) {
                    e.translation = hit.translation.clone();
                    e.note = hit.note.clone().unwrap_or_default();
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

    fn load_last_entry(&mut self) {
        let path = self.props.root.join(DEFAULT_INTERNAL).join(LAST_ENTRY);
        if let Ok(raw) = std::fs::read_to_string(path) {
            for line in raw.lines() {
                if let Some(v) = line.strip_prefix("last_entry=") {
                    self.last_index = v.parse().unwrap_or(0);
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
        let last = self.props.root.join(DEFAULT_INTERNAL).join(LAST_ENTRY);
        std::fs::write(last, format!("last_entry={}\n", self.last_index))?;
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
                    file: Some(e.file.clone()),
                    id: Some(e.id.clone()),
                    changer: Some("omegat-rewrite".into()),
                    changed: Some(now_iso()),
                    ..Default::default()
                });
            }
        }
    }

    pub fn set_entry(&mut self, params: &SetEntryParams) -> Result<EntryDto> {
        let e = self
            .entries
            .get_mut(params.index)
            .ok_or_else(|| CoreError::InvalidProject("entry out of range".into()))?;
        if e.revision != params.revision {
            return Err(CoreError::OptimisticLock(params.index));
        }
        if self.prefs.extra.get("tag_validation").map(|s| s.as_str()) == Some("abort")
            && !params.translation.trim().is_empty()
        {
            let errs = tags::validate(&e.source, &params.translation);
            if errs.iter().any(|k| {
                matches!(
                    k,
                    tags::TagErrorKind::Missing | tags::TagErrorKind::Malformed
                )
            }) {
                return Err(CoreError::TagValidation(format!(
                    "segment {} failed tag validation",
                    params.index
                )));
            }
        }
        e.translation = params.translation.clone();
        if let Some(n) = &params.note {
            e.note = n.clone();
        }
        e.default_translation = params.default_translation;
        e.revision += 1;
        e.from_tm_exact = false;
        upsert_prop(e, "changeid", "omegat-rewrite");
        upsert_prop(e, "changedate", &now_iso());
        self.dirty = true;
        self.last_index = params.index;
        Ok(e.to_dto(params.index))
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
            dto.similarity = matching::similarity_data(&e.source, &m.source, &self.props.source_lang);
            dto
        })
        .collect()
    }

    pub fn glossary_for(&self, index: usize) -> Vec<GlossaryHitDto> {
        let Some(e) = self.entries.get(index) else {
            return vec![];
        };
        let ignore_case = self.prefs.extra.get("glossary_ignore_case").map(|s| s != "false").unwrap_or(true);
        let use_stem = self.prefs.extra.get("glossary_stem").map(|s| s != "false").unwrap_or(true);
        glossary::lookup_opts(&self.glossary, &e.source, ignore_case, use_stem)
    }

    pub fn compile(&mut self, source_pattern: Option<&str>) -> Result<usize> {
        if self.prefs.extra.get("tag_validation").map(|s| s.as_str()) == Some("abort") {
            let bad = self
                .issues()
                .iter()
                .filter(|i| i.kind == "tag" && i.severity == "error")
                .count();
            if bad > 0 {
                return Err(CoreError::TagValidation(format!("{bad} tag errors")));
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
                    .filter(|e| e.source == p.source || e.id == p.id || e.id.starts_with(&format!("{}:", p.id)))
                    .map(|e| e.translation.as_str())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ");
                let trans = if trans.is_empty() {
                    segs.get(i).map(|e| e.translation.clone()).unwrap_or_default()
                } else {
                    trans
                };
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
        stats::compute(&self.entries, &self.props.source_lang, &self.props.target_lang)
    }

    pub fn search(&self, params: &SearchParams) -> Vec<SearchHitDto> {
        search::search(&self.entries, params)
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
        let lt = self.prefs.extra.get("languagetool_url").map(|s| s.as_str());
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
                    if !e.translation.to_lowercase().contains(&g.target.to_lowercase()) {
                        all.push(IssueDto {
                            kind: "glossary".into(),
                            index: i,
                            file: e.file.clone(),
                            message: format!("Glossary term '{}' not used (expected '{}')", g.source, g.target),
                            severity: "warn".into(),
                        });
                    }
                }
            }
        }
        all
    }

    pub fn mt(&self, index: usize, engine: &str) -> Result<MtSuggestionDto> {
        let e = self
            .entries
            .get(index)
            .ok_or(CoreError::InvalidProject("entry".into()))?;
        mt::translate(
            engine,
            &e.source,
            &self.props.source_lang,
            &self.props.target_lang,
            &self.mt_cache,
        )
        .map_err(CoreError::Filter)
    }

    pub fn dict(&self, word: &str) -> Vec<DictHitDto> {
        dict::lookup(&self.props.dictionary_dir, word)
    }

    pub fn completer(&self, index: usize, prefix: &str) -> Vec<CompleterItemDto> {
        let mut items = Vec::new();
        if let Some(e) = self.entries.get(index) {
            for g in glossary::lookup(&self.glossary, &e.source) {
                if g.target.to_lowercase().starts_with(&prefix.to_lowercase()) || prefix.is_empty() {
                    items.push(CompleterItemDto {
                        kind: "glossary".into(),
                        text: g.target,
                        detail: g.source,
                    });
                }
            }
            for t in tags::extract_tags(&e.source) {
                items.push(CompleterItemDto {
                    kind: "tag".into(),
                    text: t,
                    detail: "source tag".into(),
                });
            }
            if let Some(at) = self.prefs.extra.get("autotext") {
                for pair in at.split(';') {
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
            for (ch, name) in [
                ("\u{00a0}", "NBSP"),
                ("\u{2014}", "em dash"),
                ("\u{2026}", "ellipsis"),
                ("\u{00ab}", "guillemet"),
                ("\u{00bb}", "guillemet"),
            ] {
                if prefix.is_empty() || name.starts_with(prefix) {
                    items.push(CompleterItemDto {
                        kind: "charset".into(),
                        text: ch.to_string(),
                        detail: name.into(),
                    });
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        for e in &self.entries {
            for w in e.translation.split_whitespace() {
                if w.to_lowercase().starts_with(&prefix.to_lowercase()) && seen.insert(w.to_string()) {
                    items.push(CompleterItemDto {
                        kind: "history".into(),
                        text: w.to_string(),
                        detail: "history".into(),
                    });
                }
            }
        }
        items.truncate(20);
        items
    }

    pub fn align(&self, source: &Path, target: &Path, dest: &Path) -> Result<()> {
        let tmx = align::align_files(source, target, &self.props.source_lang, &self.props.target_lang)?;
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
    b.build().unwrap_or_else(|_| globset::GlobSetBuilder::new().build().unwrap())
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

fn penalty_from_origin(origin: &str) -> i32 {
    origin
        .replace('\\', "/")
        .split('/')
        .find_map(|p| p.strip_prefix("penalty-")?.parse().ok())
        .unwrap_or(0)
}

fn folder_is(origin: &str, name: &str) -> bool {
    let o = origin.replace('\\', "/");
    o == name
        || o.starts_with(&format!("{name}/"))
        || o.contains(&format!("/{name}/"))
}

fn load_external_tm(props: &ProjectProperties) -> Vec<(TmxEntry, String)> {
    let mut out = Vec::new();
    if !props.tm_dir.exists() {
        return out;
    }
    for ent in walkdir::WalkDir::new(&props.tm_dir).into_iter().flatten() {
        if ent.path().extension().and_then(|e| e.to_str()) != Some("tmx") {
            continue;
        }
        let origin = ent
            .path()
            .strip_prefix(&props.tm_dir)
            .unwrap_or(ent.path())
            .to_string_lossy()
            .replace('\\', "/");
        let langs = if folder_is(&origin, TMX2SOURCE) {
            let stem = ent
                .path()
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&props.target_lang);
            (props.source_lang.as_str(), stem)
        } else {
            (props.source_lang.as_str(), props.target_lang.as_str())
        };
        if let Ok(tmx) = ProjectTmx::load(ent.path(), langs.0, langs.1) {
            let penalty = penalty_from_origin(&origin);
            for mut e in tmx.entries {
                e.penalty = penalty;
                if penalty > 0 {
                    e.note = Some(format!("penalty:{penalty}"));
                }
                out.push((e, origin.clone()));
            }
        }
    }
    out
}
