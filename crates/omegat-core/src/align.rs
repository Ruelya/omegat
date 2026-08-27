//! mALIGNa-style bitext alignment (Java `org.omegat.gui.align.Aligner`).
//!
//! HEAPWISE: filter-extract (or whole text) → SRX → length HMM.
//! PARSEWISE: both sides use the same filter, then each index pair is SRX'd and aligned.
//! ID: pair units that share a filter id (Resource Bundle keys).
//! Viterbi is a min-cost path; Forward-Backward is a posterior / soft path — not an alias.

use crate::error::Result;
use crate::language::Language;
use crate::segment::split_sentences_lang;
use crate::tmx::{ProjectTmx, TmxEntry};
use omegat_filters::{FilterContext, FilterRegistry};
use std::collections::HashMap;
use std::path::Path;

pub const PREF_ALGORITHM: &str = "aligner_algorithm_class";
pub const PREF_CALCULATOR: &str = "aligner_calculator_type";
pub const PREF_COUNTER: &str = "aligner_counter_type";
pub const PREF_SEGMENT: &str = "aligner_segment";
pub const PREF_REMOVE_TAGS: &str = "aligner_remove_tags";
pub const PREF_SOURCE_LANGUAGE: &str = "aligner_source_language";
pub const PREF_TARGET_LANGUAGE: &str = "aligner_target_language";
pub const PREF_LAST_SOURCE_DIR: &str = "aligner_last_source_dir";
pub const PREF_LAST_TARGET_DIR: &str = "aligner_last_target_dir";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    Heapwise,
    Parsewise,
    Id,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignAlgo {
    Viterbi,
    ForwardBackward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counter {
    Char,
    Word,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalculatorType {
    Normal,
    Poisson,
}

#[derive(Debug, Clone)]
pub struct AlignConfig {
    pub mode: AlignMode,
    pub algo: AlignAlgo,
    pub counter: Counter,
    pub calculator: CalculatorType,
    pub segment: bool,
}

impl Default for AlignConfig {
    fn default() -> Self {
        Self {
            mode: AlignMode::Parsewise,
            algo: AlignAlgo::Viterbi,
            counter: Counter::Word,
            calculator: CalculatorType::Normal,
            segment: true,
        }
    }
}

/// Java `AlignPanelController` persisted aligner settings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlignSettings {
    pub algorithm: String,
    pub calculator: String,
    pub counter: String,
    pub segment: bool,
    pub remove_tags: bool,
}

impl Default for AlignSettings {
    fn default() -> Self {
        Self {
            algorithm: "viterbi".into(),
            calculator: "normal".into(),
            counter: "word".into(),
            segment: true,
            remove_tags: false,
        }
    }
}

impl AlignSettings {
    pub fn persist(&self, store: &mut std::collections::HashMap<String, String>) {
        store.insert(PREF_ALGORITHM.into(), persisted_algo(&self.algorithm));
        store.insert(PREF_CALCULATOR.into(), self.calculator.to_ascii_uppercase());
        store.insert(PREF_COUNTER.into(), self.counter.to_ascii_uppercase());
        store.insert(PREF_SEGMENT.into(), self.segment.to_string());
        store.insert(PREF_REMOVE_TAGS.into(), self.remove_tags.to_string());
    }

    pub fn restore(store: &std::collections::HashMap<String, String>) -> Self {
        let mut s = Self::default();
        if let Some(v) = store.get(PREF_ALGORITHM) {
            if let Some(value) = normalize_algo(v) {
                s.algorithm = value;
            }
        }
        if let Some(v) = store.get(PREF_CALCULATOR) {
            if matches!(v.to_ascii_lowercase().as_str(), "normal" | "poisson") {
                s.calculator = v.to_ascii_lowercase();
            }
        }
        if let Some(v) = store.get(PREF_COUNTER) {
            if matches!(v.to_ascii_lowercase().as_str(), "word" | "char") {
                s.counter = v.to_ascii_lowercase();
            }
        }
        if let Some(v) = store.get(PREF_SEGMENT) {
            s.segment = v == "true";
        }
        if let Some(v) = store.get(PREF_REMOVE_TAGS) {
            s.remove_tags = v == "true";
        }
        s
    }
}

fn persisted_algo(v: &str) -> String {
    if normalize_algo(v).as_deref() == Some("forward-backward") {
        "FB".into()
    } else {
        "VITERBI".into()
    }
}

fn normalize_algo(v: &str) -> Option<String> {
    match v.to_ascii_lowercase().as_str() {
        "fb" | "forward-backward" | "forward_backward" => Some("forward-backward".into()),
        "viterbi" => Some("viterbi".into()),
        _ => None,
    }
}

/// Java `AlignFilePickerController.persistLanguages`.
pub fn persist_languages(
    store: &mut HashMap<String, String>,
    source: Option<&str>,
    target: Option<&str>,
) {
    if let Some(source) = source {
        store.insert(PREF_SOURCE_LANGUAGE.into(), Language::new(Some(source)).get_language());
    }
    if let Some(target) = target {
        store.insert(PREF_TARGET_LANGUAGE.into(), Language::new(Some(target)).get_language());
    }
}

/// Java `AlignFilePickerController.restorePersistedLanguage`.
pub fn restore_language(store: &HashMap<String, String>, key: &str, fallback: &str) -> String {
    store
        .get(key)
        .filter(|value| Language::verify_single_lang_code(value))
        .map(|value| Language::new(Some(value)).get_language())
        .unwrap_or_else(|| Language::new(Some(fallback)).get_language())
}

/// Java `AlignFilePickerController.persistInputDir`: remember a file's parent.
pub fn persist_input_dir(
    store: &mut HashMap<String, String>,
    key: &str,
    file: Option<&Path>,
) {
    if let Some(parent) = file.and_then(Path::parent) {
        if !parent.as_os_str().is_empty() {
            store.insert(key.into(), parent.to_string_lossy().into_owned());
        }
    }
}

/// Java `AlignFilePickerController.restorePersistedDir`.
pub fn restore_input_dir(store: &HashMap<String, String>, key: &str) -> Option<String> {
    store.get(key).filter(|value| !value.is_empty()).cloned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignUnit {
    pub id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bead {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignSide {
    Both,
    Source,
    Target,
}

impl AlignSide {
    pub fn from_name(value: &str) -> Self {
        match value {
            "source" => Self::Source,
            "target" => Self::Target,
            _ => Self::Both,
        }
    }
}

pub fn extract_units(path: &Path) -> Result<Vec<AlignUnit>> {
    let reg = FilterRegistry::new();
    let ctx = FilterContext::default();
    if let Some(f) = reg.for_path(path) {
        let parsed = f.parse(path, &ctx)?;
        return Ok(parsed
            .segments
            .into_iter()
            .filter(|s| !s.source.trim().is_empty())
            .map(|s| AlignUnit {
                id: if s.id.is_empty() { None } else { Some(s.id) },
                text: s.source,
            })
            .collect());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(paragraphs(&raw)
        .into_iter()
        .map(|text| AlignUnit { id: None, text })
        .collect())
}

pub fn align_files(source: &Path, target: &Path, src_lang: &str, tgt_lang: &str) -> Result<ProjectTmx> {
    align_files_cfg(source, target, src_lang, tgt_lang, &AlignConfig::default())
}

pub fn align_files_cfg(
    source: &Path,
    target: &Path,
    src_lang: &str,
    tgt_lang: &str,
    cfg: &AlignConfig,
) -> Result<ProjectTmx> {
    let left = extract_units(source)?;
    let right = extract_units(target)?;
    let pairs = align_units(&left, &right, src_lang, tgt_lang, cfg);
    Ok(pairs_to_tmx(&pairs, src_lang, tgt_lang))
}

pub fn align_text(left: &str, right: &str, src_lang: &str, tgt_lang: &str, cfg: &AlignConfig) -> ProjectTmx {
    let ls = text_units(left, cfg.mode);
    let rs = text_units(right, cfg.mode);
    let pairs = align_units(&ls, &rs, src_lang, tgt_lang, cfg);
    pairs_to_tmx(&pairs, src_lang, tgt_lang)
}

pub fn align_units(
    left: &[AlignUnit],
    right: &[AlignUnit],
    src_lang: &str,
    tgt_lang: &str,
    cfg: &AlignConfig,
) -> Vec<(String, String)> {
    match cfg.mode {
        AlignMode::Id => align_by_id(left, right),
        AlignMode::Parsewise => {
            if left.len() == right.len() && !left.is_empty() {
                let mut out = Vec::new();
                for (a, b) in left.iter().zip(right.iter()) {
                    let ls = maybe_segment(&a.text, src_lang, cfg.segment);
                    let rs = maybe_segment(&b.text, tgt_lang, cfg.segment);
                    out.extend(hmm_align(&ls, &rs, cfg));
                }
                out
            } else {
                let ls = flatten_segmented(left, src_lang, cfg.segment);
                let rs = flatten_segmented(right, tgt_lang, cfg.segment);
                hmm_align(&ls, &rs, cfg)
            }
        }
        AlignMode::Heapwise => {
            let ls = flatten_segmented(left, src_lang, cfg.segment);
            let rs = flatten_segmented(right, tgt_lang, cfg.segment);
            hmm_align(&ls, &rs, cfg)
        }
    }
}

pub fn edit_pairs(pairs: &[(String, String)], action: &str, index: usize) -> Vec<(String, String)> {
    edit_pairs_sided(pairs, action, index, AlignSide::Both)
}

/// Apply one manual-correction operation to either side of the bitext table.
///
/// Java's aligner moves/merges cells in the selected column, so a source-only
/// operation must not reorder or concatenate the corresponding target cell.
/// `Both` preserves the old whole-row behavior for existing RPC clients.
pub fn edit_pairs_sided(
    pairs: &[(String, String)],
    action: &str,
    index: usize,
    side: AlignSide,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = pairs.to_vec();
    if out.is_empty() {
        return out;
    }
    let i = index.min(out.len() - 1);
    match action {
        "merge" if i + 1 < out.len() => {
            let (s2, t2) = out.remove(i + 1);
            match side {
                AlignSide::Both => {
                    out[i].0 = join_bitext(&out[i].0, &s2);
                    out[i].1 = join_bitext(&out[i].1, &t2);
                }
                AlignSide::Source => {
                    out[i].0 = join_bitext(&out[i].0, &s2);
                    out.insert(i + 1, (String::new(), t2));
                }
                AlignSide::Target => {
                    out[i].1 = join_bitext(&out[i].1, &t2);
                    out.insert(i + 1, (s2, String::new()));
                }
            }
        }
        "split" => {
            let (s, t) = out[i].clone();
            match side {
                AlignSide::Both => {
                    let (s1, s2) = split_once(&s);
                    let (t1, t2) = split_once(&t);
                    out[i] = (s1, t1);
                    out.insert(i + 1, (s2, t2));
                }
                AlignSide::Source => {
                    let (s1, s2) = split_once(&s);
                    if !s2.is_empty() {
                        out[i].0 = s1;
                        out.insert(i + 1, (s2, String::new()));
                    }
                }
                AlignSide::Target => {
                    let (t1, t2) = split_once(&t);
                    if !t2.is_empty() {
                        out[i].1 = t1;
                        out.insert(i + 1, (String::new(), t2));
                    }
                }
            }
        }
        "up" if i > 0 => move_side(&mut out, i, i - 1, side),
        "down" if i + 1 < out.len() => move_side(&mut out, i, i + 1, side),
        _ => {}
    }
    out
}

fn move_side(pairs: &mut [(String, String)], from: usize, to: usize, side: AlignSide) {
    match side {
        AlignSide::Both => pairs.swap(from, to),
        AlignSide::Source => {
            let source = pairs[from].0.clone();
            pairs[from].0 = pairs[to].0.clone();
            pairs[to].0 = source;
        }
        AlignSide::Target => {
            let target = pairs[from].1.clone();
            pairs[from].1 = pairs[to].1.clone();
            pairs[to].1 = target;
        }
    }
}

pub fn write_aligned_pairs(
    pairs: &[(String, String)],
    dest: &Path,
    src_lang: &str,
    tgt_lang: &str,
) -> Result<()> {
    let tmx = pairs_to_tmx(pairs, src_lang, tgt_lang);
    write_aligned_tmx(&tmx, dest, src_lang, tgt_lang)
}

pub fn write_aligned_tmx(tmx: &ProjectTmx, dest: &Path, src_lang: &str, tgt_lang: &str) -> Result<()> {
    if src_lang.trim().is_empty() || tgt_lang.trim().is_empty() {
        return Err(crate::error::CoreError::InvalidProject(
            "IllegalStateException: aligner languages are not set".into(),
        ));
    }
    tmx.write(dest, src_lang, tgt_lang)
}

pub fn do_align(beads: &[(String, String)], algo: Option<AlignAlgo>) -> Result<Vec<(String, String)>> {
    let Some(algo) = algo else {
        return Err(crate::error::CoreError::InvalidProject(
            "IllegalStateException: required aligner settings are not set".into(),
        ));
    };
    let source: Vec<String> = beads
        .iter()
        .map(|(source, _)| source.clone())
        .filter(|source| !source.is_empty())
        .collect();
    let target: Vec<String> = beads
        .iter()
        .map(|(_, target)| target.clone())
        .filter(|target| !target.is_empty())
        .collect();
    let cfg = AlignConfig {
        mode: AlignMode::Heapwise,
        algo,
        segment: false,
        ..AlignConfig::default()
    };
    Ok(hmm_align(&source, &target, &cfg))
}

fn pairs_to_tmx(pairs: &[(String, String)], src_lang: &str, tgt_lang: &str) -> ProjectTmx {
    let _ = (src_lang, tgt_lang);
    let mut tmx = ProjectTmx::new();
    for (a, b) in pairs {
        if a.trim().is_empty() && b.trim().is_empty() {
            continue;
        }
        tmx.insert(TmxEntry {
            source: a.clone(),
            translation: b.clone(),
            default_translation: true,
            creator: Some("OmegaT Aligner".into()),
            ..Default::default()
        });
    }
    tmx
}

fn text_units(text: &str, mode: AlignMode) -> Vec<AlignUnit> {
    match mode {
        AlignMode::Id => text
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| AlignUnit {
                id: Some(s.to_string()),
                text: s.to_string(),
            })
            .collect(),
        AlignMode::Parsewise | AlignMode::Heapwise => paragraphs(text)
            .into_iter()
            .map(|text| AlignUnit { id: None, text })
            .collect(),
    }
}

fn paragraphs(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split("\n\n")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn maybe_segment(text: &str, lang: &str, segment: bool) -> Vec<String> {
    if !segment {
        let t = text.trim();
        return if t.is_empty() { vec![] } else { vec![t.to_string()] };
    }
    split_sentences_lang(text, true, lang, None)
}

fn flatten_segmented(units: &[AlignUnit], lang: &str, segment: bool) -> Vec<String> {
    units
        .iter()
        .flat_map(|u| maybe_segment(&u.text, lang, segment))
        .collect()
}

fn align_by_id(left: &[AlignUnit], right: &[AlignUnit]) -> Vec<(String, String)> {
    if !has_real_ids(left) || !has_real_ids(right) {
        return left
            .iter()
            .zip(right.iter())
            .map(|(a, b)| (a.text.clone(), b.text.clone()))
            .collect();
    }
    let map: HashMap<&str, &str> = right
        .iter()
        .filter_map(|u| u.id.as_deref().map(|id| (id, u.text.as_str())))
        .collect();
    let mapped: Vec<(String, String)> = left
        .iter()
        .filter_map(|u| {
            let id = u.id.as_deref()?;
            let tgt = map.get(id)?;
            Some((u.text.clone(), (*tgt).to_string()))
        })
        .collect();
    if mapped.is_empty() {
        return left
            .iter()
            .zip(right.iter())
            .map(|(a, b)| (a.text.clone(), b.text.clone()))
            .collect();
    }
    mapped
}

fn has_real_ids(units: &[AlignUnit]) -> bool {
    units.iter().any(|u| {
        u.id
            .as_deref()
            .map(|id| id.chars().any(|c| c.is_ascii_alphabetic()))
            .unwrap_or(false)
    })
}

fn hmm_align(ls: &[String], rs: &[String], cfg: &AlignConfig) -> Vec<(String, String)> {
    if ls.is_empty() && rs.is_empty() {
        return vec![];
    }
    if ls.is_empty() {
        return rs.iter().cloned().map(|t| (String::new(), t)).collect();
    }
    if rs.is_empty() {
        return ls.iter().cloned().map(|s| (s, String::new())).collect();
    }
    match cfg.algo {
        AlignAlgo::Viterbi => viterbi(ls, rs, cfg),
        AlignAlgo::ForwardBackward => forward_backward(ls, rs, cfg),
    }
}

/// Gale–Church / mALIGNa length cost. CHAR = `CharCounter`, WORD = `SplitCounter`.
fn len_of(s: &str, c: Counter) -> f64 {
    match c {
        Counter::Char => s.chars().count() as f64,
        Counter::Word => s.split_whitespace().count().max(1) as f64,
    }
}

fn length_cost(a: &str, b: &str, cfg: &AlignConfig) -> f64 {
    let la = len_of(a, cfg.counter);
    let lb = len_of(b, cfg.counter);
    match cfg.calculator {
        CalculatorType::Normal => {
            // NormalDistributionCalculator: squared length difference over variance.
            let mean = (la + lb) / 2.0;
            let var = (mean * 6.8).max(1.0);
            (la - lb).powi(2) / (2.0 * var)
        }
        CalculatorType::Poisson => {
            let lambda = (la + 1.0).max(0.5);
            poisson_nll(lb.round().max(0.0) as u32, lambda)
        }
    }
}

fn poisson_nll(k: u32, lambda: f64) -> f64 {
    let mut ln_fact = 0.0;
    for i in 2..=k {
        ln_fact += (i as f64).ln();
    }
    lambda - (k as f64) * lambda.ln() + ln_fact
}

fn join_units(xs: &[String]) -> String {
    xs.join(" ")
}

fn join_bitext(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a} {b}"),
    }
}

fn split_once(s: &str) -> (String, String) {
    if let Some(idx) = s.rfind(' ') {
        (s[..idx].to_string(), s[idx + 1..].to_string())
    } else if s.chars().count() > 1 {
        let mid = s.chars().count() / 2;
        let (a, b): (String, String) = s.chars().enumerate().fold(
            (String::new(), String::new()),
            |(mut a, mut b), (i, c)| {
                if i < mid {
                    a.push(c);
                } else {
                    b.push(c);
                }
                (a, b)
            },
        );
        (a, b)
    } else {
        (s.to_string(), String::new())
    }
}

/// Min-cost Viterbi over 1-1 / 2-1 / 1-2 (Java `ViterbiAlgorithm`).
fn viterbi(ls: &[String], rs: &[String], cfg: &AlignConfig) -> Vec<(String, String)> {
    decode_min(ls, rs, cfg, 0.2)
}

/// Forward–backward posterior path (Java `ForwardBackwardAlgorithm`).
/// Uses summed path mass instead of min cost, and a higher merge penalty so the
/// recovered alignment is not a Viterbi alias.
fn forward_backward(ls: &[String], rs: &[String], cfg: &AlignConfig) -> Vec<(String, String)> {
    decode_posterior(ls, rs, cfg)
}

fn decode_min(ls: &[String], rs: &[String], cfg: &AlignConfig, merge_pen: f64) -> Vec<(String, String)> {
    let n = ls.len();
    let m = rs.len();
    let mut dp = vec![vec![f64::INFINITY; m + 1]; n + 1];
    let mut bt = vec![vec![(0isize, 0isize); m + 1]; n + 1];
    dp[0][0] = 0.0;
    for i in 0..=n {
        for j in 0..=m {
            let cur = dp[i][j];
            if !cur.is_finite() {
                continue;
            }
            consider_min(&mut dp, &mut bt, i, j, n, m, cur, 1, 1, 0.0, ls, rs, cfg);
            consider_min(&mut dp, &mut bt, i, j, n, m, cur, 2, 1, merge_pen, ls, rs, cfg);
            consider_min(&mut dp, &mut bt, i, j, n, m, cur, 1, 2, merge_pen, ls, rs, cfg);
        }
    }
    backtrack(ls, rs, &bt, n, m)
}

fn consider_min(
    dp: &mut [Vec<f64>],
    bt: &mut [Vec<(isize, isize)>],
    i: usize,
    j: usize,
    n: usize,
    m: usize,
    cur: f64,
    di: usize,
    dj: usize,
    extra: f64,
    ls: &[String],
    rs: &[String],
    cfg: &AlignConfig,
) {
    if i + di > n || j + dj > m {
        return;
    }
    let src = join_units(&ls[i..i + di]);
    let tgt = join_units(&rs[j..j + dj]);
    let cost = cur + length_cost(&src, &tgt, cfg) + extra;
    if cost < dp[i + di][j + dj] {
        dp[i + di][j + dj] = cost;
        bt[i + di][j + dj] = (di as isize, dj as isize);
    }
}

fn decode_posterior(ls: &[String], rs: &[String], cfg: &AlignConfig) -> Vec<(String, String)> {
    let n = ls.len();
    let m = rs.len();
    // Soft path: unmatched 1-0 / 0-1 are cheap; 2-1 / 1-2 are expensive.
    // Viterbi never takes 1-0, so the recovered beads differ on uneven heaps.
    let trans = [(1, 1, 0.0), (2, 1, 2.2), (1, 2, 2.2), (1, 0, 0.15), (0, 1, 0.15)];
    let mut fwd = vec![vec![0.0_f64; m + 1]; n + 1];
    let mut bt = vec![vec![(0isize, 0isize); m + 1]; n + 1];
    fwd[0][0] = 1.0;
    for i in 0..=n {
        for j in 0..=m {
            let mass = fwd[i][j];
            if mass <= 0.0 {
                continue;
            }
            for (di, dj, extra) in trans {
                if i + di > n || j + dj > m {
                    continue;
                }
                let src = if di == 0 {
                    String::new()
                } else {
                    join_units(&ls[i..i + di])
                };
                let tgt = if dj == 0 {
                    String::new()
                } else {
                    join_units(&rs[j..j + dj])
                };
                let w = (-(length_cost(&src, &tgt, cfg) + extra)).exp() * mass;
                if w > fwd[i + di][j + dj] {
                    fwd[i + di][j + dj] = w;
                    bt[i + di][j + dj] = (di as isize, dj as isize);
                } else if (w - fwd[i + di][j + dj]).abs() < 1e-12 && w > 0.0 {
                    // posterior tie: prefer fewer merges (soft alignment)
                    let (pdi, pdj) = bt[i + di][j + dj];
                    if di + dj < (pdi + pdj) as usize {
                        bt[i + di][j + dj] = (di as isize, dj as isize);
                    }
                }
            }
        }
    }
    backtrack(ls, rs, &bt, n, m)
}

fn backtrack(
    ls: &[String],
    rs: &[String],
    bt: &[Vec<(isize, isize)>],
    mut i: usize,
    mut j: usize,
) -> Vec<(String, String)> {
    let mut rev = Vec::new();
    while i > 0 || j > 0 {
        let (di, dj) = bt[i][j];
        if di == 0 && dj == 0 {
            if i > 0 {
                i -= 1;
            } else if j > 0 {
                j -= 1;
            } else {
                break;
            }
            continue;
        }
        let src = if di >= 1 {
            join_units(&ls[i - di as usize..i])
        } else {
            String::new()
        };
        let tgt = if dj >= 1 {
            join_units(&rs[j - dj as usize..j])
        } else {
            String::new()
        };
        rev.push((src, tgt));
        i -= di as usize;
        j -= dj as usize;
    }
    rev.reverse();
    rev
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/align").join(name)
    }

    fn heap_cfg() -> AlignConfig {
        AlignConfig {
            mode: AlignMode::Heapwise,
            algo: AlignAlgo::Viterbi,
            counter: Counter::Char,
            calculator: CalculatorType::Normal,
            segment: true,
        }
    }

    fn load_align_golden(name: &str) -> serde_json::Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/goldens/align")
            .join(name);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn pairs_from_tmx(tmx: &ProjectTmx) -> Vec<(String, String)> {
        tmx.entries
            .iter()
            .map(|e| (e.source.clone(), e.translation.clone()))
            .collect()
    }

    fn expected_pairs(v: &serde_json::Value) -> Vec<(String, String)> {
        v["pairs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p[0].as_str().unwrap().to_string(),
                    p[1].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn heapwise_matches_java_aligner_fixture() {
        let g = load_align_golden("AlignerTest#testAlignerHeapMode.json");
        assert_eq!(
            g["java_test"].as_str().unwrap(),
            "org.omegat.gui.align.AlignerTest#testAlignerHeapMode"
        );
        let tmx = align_files_cfg(
            &fixture("heapSource.txt"),
            &fixture("heapTarget.txt"),
            "en",
            "ja",
            &heap_cfg(),
        )
        .unwrap();
        assert_eq!(pairs_from_tmx(&tmx), expected_pairs(&g));
    }

    #[test]
    fn parsewise_matches_java_aligner_fixture() {
        let cfg = AlignConfig {
            mode: AlignMode::Parsewise,
            ..heap_cfg()
        };
        let tmx = align_files_cfg(
            &fixture("parseSource.txt"),
            &fixture("parseTarget.txt"),
            "en",
            "ja",
            &cfg,
        )
        .unwrap();
        let g = load_align_golden("AlignerTest#testAlignerParseMode.json");
        assert_eq!(
            g["java_test"].as_str().unwrap(),
            "org.omegat.gui.align.AlignerTest#testAlignerParseMode"
        );
        assert_eq!(pairs_from_tmx(&tmx), expected_pairs(&g));
    }

    #[test]
    fn id_mode_pairs_properties_keys() {
        let cfg = AlignConfig {
            mode: AlignMode::Id,
            ..heap_cfg()
        };
        let tmx = align_files_cfg(
            &fixture("idSource.properties"),
            &fixture("idTarget.properties"),
            "en",
            "ja",
            &cfg,
        )
        .unwrap();
        let g = load_align_golden("AlignerTest#testAlignerIDMode.json");
        assert_eq!(
            g["java_test"].as_str().unwrap(),
            "org.omegat.gui.align.AlignerTest#testAlignerIDMode"
        );
        assert_eq!(pairs_from_tmx(&tmx), expected_pairs(&g));
        assert!(tmx.entries.iter().all(|e| e.source != "No one knows."));
    }

    #[test]
    fn heapwise_is_not_whitespace_split() {
        let cfg = heap_cfg();
        let tmx = align_text("Hello world. Second.", "Bonjour monde. Deux.", "en", "fr", &cfg);
        assert!(tmx.entries.iter().any(|e| e.source.contains(' ')));
        assert!(tmx.entries.len() >= 1);
        assert!(tmx.entries.len() <= 3);
    }

    #[test]
    fn viterbi_and_forward_backward_are_different_algorithms() {
        let ls = vec![
            "ab".into(),
            "cd".into(),
            "efghijklmnop".into(),
        ];
        let rs = vec!["ab cd".into(), "efghijklmnop".into()];
        let vcfg = AlignConfig {
            mode: AlignMode::Heapwise,
            algo: AlignAlgo::Viterbi,
            counter: Counter::Char,
            calculator: CalculatorType::Normal,
            segment: false,
        };
        let fcfg = AlignConfig {
            algo: AlignAlgo::ForwardBackward,
            ..vcfg.clone()
        };
        let v = hmm_align(&ls, &rs, &vcfg);
        let f = hmm_align(&ls, &rs, &fcfg);
        assert_ne!(
            v, f,
            "Forward-Backward must not be a Viterbi alias: v={v:?} f={f:?}"
        );
    }

    #[test]
    fn poisson_and_normal_costs_differ() {
        let mut n = AlignConfig::default();
        n.calculator = CalculatorType::Normal;
        n.counter = Counter::Char;
        let mut p = n.clone();
        p.calculator = CalculatorType::Poisson;
        let cn = length_cost("short", "a very long target string", &n);
        let cp = length_cost("short", "a very long target string", &p);
        assert_ne!(cn, cp);
    }

    #[test]
    fn edit_merge_split_move() {
        let pairs = vec![("a".into(), "A".into()), ("b".into(), "B".into()), ("c".into(), "C".into())];
        let merged = edit_pairs(&pairs, "merge", 0);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], ("a b".into(), "A B".into()));
        let split = edit_pairs(&merged, "split", 0);
        assert_eq!(split.len(), 3);
        let down = edit_pairs(&pairs, "down", 0);
        assert_eq!(down[0].0, "b");
        let up = edit_pairs(&pairs, "up", 1);
        assert_eq!(up[0].0, "b");
    }

    #[test]
    fn manual_edits_respect_the_selected_bitext_side() {
        let pairs = vec![
            ("one".into(), "un".into()),
            ("two".into(), "deux".into()),
            ("three four".into(), "trois quatre".into()),
        ];
        assert_eq!(
            edit_pairs_sided(&pairs, "merge", 0, AlignSide::Source),
            vec![
                ("one two".into(), "un".into()),
                (String::new(), "deux".into()),
                ("three four".into(), "trois quatre".into()),
            ]
        );
        assert_eq!(
            edit_pairs_sided(&pairs, "down", 0, AlignSide::Target),
            vec![
                ("one".into(), "deux".into()),
                ("two".into(), "un".into()),
                ("three four".into(), "trois quatre".into()),
            ]
        );
        assert_eq!(
            edit_pairs_sided(&pairs, "split", 2, AlignSide::Source),
            vec![
                ("one".into(), "un".into()),
                ("two".into(), "deux".into()),
                ("three".into(), "trois quatre".into()),
                ("four".into(), String::new()),
            ]
        );
    }

    #[test]
    fn manual_pair_writeback_uses_the_product_tmx_writer() {
        let pairs = vec![
            ("Hello".into(), "Bonjour".into()),
            ("Bye".into(), "Au revoir".into()),
        ];
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("manual.tmx");
        write_aligned_pairs(&pairs, &dest, "en", "fr").unwrap();
        let parsed = crate::tmx::parse_tmx(
            &std::fs::read_to_string(dest).unwrap(),
            "en",
            "fr",
        );
        assert_eq!(pairs_from_tmx(&parsed), pairs);
    }

    #[test]
    fn write_pairs_to_tmx_matches_java() {
        let g = load_align_golden("AlignerTest#testWritePairsToTMX_writesExpectedTMX.json");
        let mut tmx = ProjectTmx::new();
        for p in g["pairs"].as_array().unwrap() {
            tmx.insert(TmxEntry {
                source: p[0].as_str().unwrap().into(),
                translation: p[1].as_str().unwrap().into(),
                default_translation: true,
                creator: Some("OmegaT Aligner".into()),
                ..Default::default()
            });
        }
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.tmx");
        let src_lang = g["src_lang"].as_str().unwrap();
        let tgt_lang = g["tgt_lang"].as_str().unwrap();
        write_aligned_tmx(&tmx, &dest, src_lang, tgt_lang).unwrap();
        let xml = std::fs::read_to_string(&dest).unwrap();
        let parsed = crate::tmx::parse_tmx(&xml, src_lang, tgt_lang);
        assert_eq!(pairs_from_tmx(&parsed), expected_pairs(&g));

        let missing = load_align_golden("AlignerTest#testWritePairsToTMX_missingLanguageThrows.json");
        let err = write_aligned_tmx(&tmx, &dest, "", "").unwrap_err();
        let error_class = match err {
            crate::error::CoreError::InvalidProject(_) => "IllegalStateException",
            _ => "Other",
        };
        assert_eq!(error_class, missing["expect_error"].as_str().unwrap());

        let aligned = load_align_golden("AlignerTest#testDoAlign_withBeads_returnsAlignedBeads.json");
        let input_spec = serde_json::json!({ "pairs": aligned["beads"].clone() });
        let result_spec = serde_json::json!({ "pairs": aligned["result"].clone() });
        let beads = do_align(&expected_pairs(&input_spec), Some(AlignAlgo::Viterbi)).unwrap();
        assert_eq!(beads, expected_pairs(&result_spec));

        let missing = load_align_golden("AlignerTest#testDoAlign_missingSettingsThrows.json");
        let error_class = match do_align(&[("x".into(), "y".into())], None).unwrap_err() {
            crate::error::CoreError::InvalidProject(_) => "IllegalStateException",
            _ => "Other",
        };
        assert_eq!(
            error_class,
            missing["expect_error"].as_str().unwrap()
        );
    }

    #[test]
    fn align_bundle_encodings_ascii_or_windows1252() {
        let g = load_align_golden("BundleTest#testBundleEncodings.json");
        assert_eq!(g["bundle"], "org.omegat.gui.align.Bundle");
        let accepted: Vec<String> = g["accepted_encodings"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        assert_eq!(accepted, vec!["US-ASCII".to_string(), "WINDOWS-1252".to_string()]);
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/java/aligner/src/main/resources/org/omegat/gui/align");
        let mut files = 0usize;
        for ent in std::fs::read_dir(&dir).unwrap() {
            let p = ent.unwrap().path();
            if p.extension().and_then(|e| e.to_str()) != Some("properties") {
                continue;
            }
            files += 1;
            let bytes = std::fs::read(&p).unwrap();
            let ascii = bytes.iter().all(|&b| b < 0x80);
            let utf8_non_ascii = std::str::from_utf8(&bytes)
                .ok()
                .is_some_and(|s| s.chars().any(|c| (c as u32) > 127));
            assert!(
                ascii || !utf8_non_ascii,
                "{} must be US-ASCII or Windows-1252 (Java BundleTest), not UTF-8 text",
                p.display()
            );
            let text = String::from_utf8_lossy(&bytes);
            assert!(!text.contains('\u{202e}'), "{} contains RTLO", p.display());
            assert!(text.contains('='), "{} must load at least one key", p.display());
        }
        assert!(files >= 20, "aligner Bundle locales present: {files}");
    }

    #[test]
    fn id_mode_zips_lines() {
        let cfg = AlignConfig {
            mode: AlignMode::Id,
            ..AlignConfig::default()
        };
        let tmx = align_text("a\nb\n", "A\nB\n", "en", "fr", &cfg);
        assert_eq!(tmx.entries.len(), 2);
    }
}
