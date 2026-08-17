//! mALIGNa-style bitext alignment (Java `org.omegat.gui.align.Aligner`).
//!
//! HEAPWISE: filter-extract (or whole text) → SRX → length HMM.
//! PARSEWISE: both sides use the same filter, then each index pair is SRX'd and aligned.
//! ID: pair units that share a filter id (Resource Bundle keys).
//! Viterbi is a min-cost path; Forward-Backward is a posterior / soft path — not an alias.

use crate::error::Result;
use crate::segment::split_sentences_lang;
use crate::tmx::{ProjectTmx, TmxEntry};
use omegat_filters::{FilterContext, FilterRegistry};
use std::collections::HashMap;
use std::path::Path;

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
    let mut out: Vec<(String, String)> = pairs.to_vec();
    if out.is_empty() {
        return out;
    }
    let i = index.min(out.len() - 1);
    match action {
        "merge" if i + 1 < out.len() => {
            let (s2, t2) = out.remove(i + 1);
            out[i].0 = join_bitext(&out[i].0, &s2);
            out[i].1 = join_bitext(&out[i].1, &t2);
        }
        "split" => {
            let (s, t) = out[i].clone();
            let (s1, s2) = split_once(&s);
            let (t1, t2) = split_once(&t);
            out[i] = (s1, t1);
            out.insert(i + 1, (s2, t2));
        }
        "up" if i > 0 => out.swap(i - 1, i),
        "down" if i + 1 < out.len() => out.swap(i, i + 1),
        _ => {}
    }
    out
}

pub fn write_aligned_tmx(tmx: &ProjectTmx, dest: &Path, src_lang: &str, tgt_lang: &str) -> Result<()> {
    tmx.write(dest, src_lang, tgt_lang)
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

    #[test]
    fn heapwise_matches_java_aligner_fixture() {
        let tmx = align_files_cfg(
            &fixture("heapSource.txt"),
            &fixture("heapTarget.txt"),
            "en",
            "ja",
            &heap_cfg(),
        )
        .unwrap();
        assert_eq!(tmx.entries.len(), 4);
        assert_eq!(tmx.entries[0].source, "This is sentence one.");
        assert_eq!(tmx.entries[0].translation, "これが1つ目のセンテンス。");
        assert_eq!(tmx.entries[1].source, "Short sentence.");
        assert_eq!(tmx.entries[1].translation, "短い文。");
        assert!(tmx.entries[2].source.contains("very long sentence"));
        assert!(tmx.entries[2].source.contains("Where shall it end?"));
        assert!(tmx.entries[2].translation.contains("長蛇"));
        assert_eq!(tmx.entries[3].source, "No one knows.");
        assert_eq!(tmx.entries[3].translation, "誰も知らない。");
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
        assert_eq!(tmx.entries.len(), 4);
        assert_eq!(tmx.entries[0].source, "This is sentence one.");
        assert_eq!(tmx.entries[1].source, "Short sentence.");
        assert_eq!(tmx.entries[2].source, "And then this is a very, very, very long sentence.");
        assert_eq!(tmx.entries[3].source, "Where shall it end? No one knows.");
        assert_eq!(tmx.entries[3].translation, "誰も知らない。");
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
        assert_eq!(tmx.entries.len(), 4);
        assert_eq!(tmx.entries[0].source, "This is sentence one.");
        assert_eq!(tmx.entries[3].source, "Where shall it end?");
        assert_eq!(tmx.entries[3].translation, "誰も知らない。");
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
    fn id_mode_zips_lines() {
        let cfg = AlignConfig {
            mode: AlignMode::Id,
            ..AlignConfig::default()
        };
        let tmx = align_text("a\nb\n", "A\nB\n", "en", "fr", &cfg);
        assert_eq!(tmx.entries.len(), 2);
    }
}
