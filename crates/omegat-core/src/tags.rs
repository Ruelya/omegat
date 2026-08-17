use omegat_ipc::IssueDto;
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagErrorKind {
    Missing,
    Extraneous,
    Order,
    Duplicate,
    Malformed,
    Orphaned,
    Whitespace,
}

impl TagErrorKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "MISSING",
            Self::Extraneous => "EXTRANEOUS",
            Self::Order => "ORDER",
            Self::Duplicate => "DUPLICATE",
            Self::Malformed => "MALFORMED",
            Self::Orphaned => "ORPHANED",
            Self::Whitespace => "WHITESPACE",
        }
    }
}

pub fn extract_tags(text: &str) -> Vec<String> {
    let re = Regex::new(r"<[^>]+>|\{[0-9]+\}").unwrap();
    re.find_iter(text).map(|m| m.as_str().to_string()).collect()
}

pub fn validate(source: &str, target: &str) -> Vec<TagErrorKind> {
    let mut errors = Vec::new();
    let src = extract_tags(source);
    let tgt = extract_tags(target);
    for t in &src {
        let sc = src.iter().filter(|x| *x == t).count();
        let tc = tgt.iter().filter(|x| *x == t).count();
        if tc == 0 {
            errors.push(TagErrorKind::Missing);
        } else if tc > sc {
            errors.push(TagErrorKind::Duplicate);
        }
    }
    for t in &tgt {
        if !src.contains(t) {
            if t.starts_with('<') && !t.contains('>') {
                errors.push(TagErrorKind::Malformed);
            } else {
                errors.push(TagErrorKind::Extraneous);
            }
        }
    }
    let src_seq: Vec<_> = src.iter().filter(|t| tgt.contains(t)).cloned().collect();
    let tgt_seq: Vec<_> = tgt.iter().filter(|t| src.contains(t)).cloned().collect();
    if src_seq != tgt_seq && !src_seq.is_empty() {
        errors.push(TagErrorKind::Order);
    }
    if source.starts_with(' ') != target.starts_with(' ')
        || source.ends_with(' ') != target.ends_with(' ')
    {
        errors.push(TagErrorKind::Whitespace);
    }
    errors.sort_by_key(|e| e.as_str());
    errors.dedup();
    errors
}

pub fn repair(source: &str, target: &str) -> String {
    let src_tags = extract_tags(source);
    let mut out = target.to_string();
    for t in extract_tags(target) {
        if !src_tags.contains(&t) {
            out = out.replace(&t, "");
        }
    }
    for t in &src_tags {
        if !out.contains(t.as_str()) {
            out.push_str(t);
        }
    }
    out
}

pub fn issues_for(index: usize, file: &str, source: &str, target: &str) -> Vec<IssueDto> {
    if target.is_empty() {
        return vec![];
    }
    validate(source, target)
        .into_iter()
        .map(|k| IssueDto {
            kind: "tag".into(),
            index,
            file: file.to_string(),
            message: format!("Tag {}", k.as_str()),
            severity: if matches!(k, TagErrorKind::Missing | TagErrorKind::Malformed) {
                "error".into()
            } else {
                "warn".into()
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing() {
        let e = validate("Hello <b>x</b>", "Hello x");
        assert!(e.contains(&TagErrorKind::Missing));
    }
}
