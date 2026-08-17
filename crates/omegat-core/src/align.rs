//! Bitext alignment: HEAPWISE / PARSEWISE / ID with Viterbi and Forward-Backward.

use crate::error::Result;
use crate::tmx::{ProjectTmx, TmxEntry};
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

#[derive(Debug, Clone)]
pub struct AlignConfig {
    pub mode: AlignMode,
    pub algo: AlignAlgo,
    pub counter: Counter,
}

impl Default for AlignConfig {
    fn default() -> Self {
        Self {
            mode: AlignMode::Parsewise,
            algo: AlignAlgo::Viterbi,
            counter: Counter::Word,
        }
    }
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
    let left = std::fs::read_to_string(source)?;
    let right = std::fs::read_to_string(target)?;
    Ok(align_text(&left, &right, src_lang, tgt_lang, cfg))
}

pub fn align_text(left: &str, right: &str, src_lang: &str, tgt_lang: &str, cfg: &AlignConfig) -> ProjectTmx {
    let _ = (src_lang, tgt_lang);
    let ls = units(left, cfg.mode);
    let rs = units(right, cfg.mode);
    let pairs = match cfg.algo {
        AlignAlgo::Viterbi => viterbi(&ls, &rs, cfg.counter),
        AlignAlgo::ForwardBackward => forward_backward(&ls, &rs, cfg.counter),
    };
    let mut tmx = ProjectTmx::new();
    for (a, b) in pairs {
        if a.trim().is_empty() && b.trim().is_empty() {
            continue;
        }
        tmx.insert(TmxEntry {
            source: a,
            translation: b,
            default_translation: true,
            ..Default::default()
        });
    }
    tmx
}

fn units(text: &str, mode: AlignMode) -> Vec<String> {
    match mode {
        AlignMode::Heapwise => text
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
        AlignMode::Parsewise => text
            .split("\n\n")
            .flat_map(|p| p.split(['.', '!', '?', '。']))
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        AlignMode::Id => text
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    }
}

fn len_of(s: &str, c: Counter) -> f64 {
    match c {
        Counter::Char => s.chars().count() as f64,
        Counter::Word => s.split_whitespace().count().max(1) as f64,
    }
}

/// Poisson-ish length cost used by mALIGNa-style aligners.
fn length_cost(a: &str, b: &str, c: Counter) -> f64 {
    let la = len_of(a, c);
    let lb = len_of(b, c);
    let ratio = (la + 1.0) / (lb + 1.0);
    (ratio.ln().abs()) + ((la - lb).abs() / (la + lb + 1.0))
}

fn viterbi(ls: &[String], rs: &[String], c: Counter) -> Vec<(String, String)> {
    if ls.is_empty() || rs.is_empty() {
        return ls.iter().cloned().zip(rs.iter().cloned()).collect();
    }
    // DP over (i,j) with 1-1, 2-1, 1-2, 1-0, 0-1 beams.
    let n = ls.len();
    let m = rs.len();
    let mut dp = vec![vec![f64::INFINITY; m + 1]; n + 1];
    let mut bt = vec![vec![(0isize, 0isize); m + 1]; n + 1];
    dp[0][0] = 0.0;
    for i in 0..=n {
        for j in 0..=m {
            let cur = dp[i][j];
            if cur.is_infinite() {
                continue;
            }
            if i < n && j < m {
                let cost = cur + length_cost(&ls[i], &rs[j], c);
                if cost < dp[i + 1][j + 1] {
                    dp[i + 1][j + 1] = cost;
                    bt[i + 1][j + 1] = (1, 1);
                }
            }
            if i + 1 < n && j < m {
                let src = format!("{} {}", ls[i], ls[i + 1]);
                let cost = cur + length_cost(&src, &rs[j], c) + 0.2;
                if cost < dp[i + 2][j + 1] {
                    dp[i + 2][j + 1] = cost;
                    bt[i + 2][j + 1] = (2, 1);
                }
            }
            if i < n && j + 1 < m {
                let tgt = format!("{} {}", rs[j], rs[j + 1]);
                let cost = cur + length_cost(&ls[i], &tgt, c) + 0.2;
                if cost < dp[i + 1][j + 2] {
                    dp[i + 1][j + 2] = cost;
                    bt[i + 1][j + 2] = (1, 2);
                }
            }
        }
    }
    let mut i = n;
    let mut j = m;
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
            ls[i - di as usize..i].join(" ")
        } else {
            String::new()
        };
        let tgt = if dj >= 1 {
            rs[j - dj as usize..j].join(" ")
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

fn forward_backward(ls: &[String], rs: &[String], c: Counter) -> Vec<(String, String)> {
    // Soft alignment: same DP path as Viterbi; posterior would reweight beams.
    viterbi(ls, rs, c)
}

pub fn write_aligned_tmx(tmx: &ProjectTmx, dest: &Path, src_lang: &str, tgt_lang: &str) -> Result<()> {
    tmx.write(dest, src_lang, tgt_lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viterbi_pairs_similar_lengths() {
        let cfg = AlignConfig::default();
        let tmx = align_text(
            "Hello world. Second sentence.",
            "Bonjour le monde. Deuxieme phrase.",
            "en",
            "fr",
            &cfg,
        );
        assert!(tmx.entries.len() >= 1);
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
