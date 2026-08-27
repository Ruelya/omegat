//! Java `org.omegat.core.matching.LevenshteinDistance`.

/// Java `LevenshteinDistance.MAX_N`.
pub const MAX_N: usize = 1000;

/// Token-level Levenshtein distance. `None` inputs are rejected like Java.
pub fn compute(
    source: Option<&[String]>,
    target: Option<&[String]>,
) -> Result<usize, &'static str> {
    let source = source.ok_or("LD_NULL_ARRAYS_ERROR")?;
    let target = target.ok_or("LD_NULL_ARRAYS_ERROR")?;
    Ok(compute_tokens(source, target))
}

pub fn compute_tokens(source: &[String], target: &[String]) -> usize {
    let mut source_len = source.len();
    let mut target_len = target.len();
    if source_len == 0 {
        return target_len;
    }
    if target_len == 0 {
        return source_len;
    }
    if source_len > MAX_N {
        source_len = MAX_N;
    }
    if target_len > MAX_N {
        target_len = MAX_N;
    }
    let source = &source[..source_len];
    let target = &target[..target_len];
    let mut previous: Vec<usize> = (0..=source_len).collect();
    let mut current = vec![0; source_len + 1];
    for (j, t) in target.iter().enumerate() {
        current[0] = j + 1;
        for (i, s) in source.iter().enumerate() {
            let cost = if s == t { 0 } else { 1 };
            current[i + 1] = (current[i] + 1)
                .min(previous[i + 1] + 1)
                .min(previous[i] + cost);
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[source_len]
}

pub fn token_levenshtein(a: &[String], b: &[String]) -> usize {
    compute_tokens(a, b)
}

/// Java `FuzzyMatcher.calcSimilarity`.
pub fn token_similarity(a: &[String], b: &[String]) -> i32 {
    if a.is_empty() && b.is_empty() {
        return 0;
    }
    let max = a.len().max(b.len());
    let ld = compute_tokens(a, b);
    ((max - ld) * 100 / max) as i32
}

pub fn char_levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<String> = a.chars().map(|c| c.to_string()).collect();
    let b: Vec<String> = b.chars().map(|c| c.to_string()).collect();
    compute_tokens(&a, &b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(words: &[&str]) -> Vec<String> {
        words.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn identical_is_zero() {
        assert_eq!(
            compute_tokens(&toks(&["test", "example"]), &toks(&["test", "example"])),
            0
        );
    }

    #[test]
    fn empty_is_other_len() {
        assert_eq!(compute_tokens(&toks(&["alpha", "beta"]), &[]), 2);
        assert_eq!(
            compute_tokens(&[], &toks(&["gamma", "delta", "epsilon"])),
            3
        );
    }

    #[test]
    fn null_rejected() {
        assert!(compute(None, Some(&toks(&["null"]))).is_err());
        assert!(compute(Some(&toks(&["null"])), None).is_err());
    }
}
