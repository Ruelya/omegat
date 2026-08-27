use omegat_ipc::IssueDto;
use regex::Regex;
use once_cell::sync::Lazy;

static OMEGAT_TAG: Lazy<Regex> = Lazy::new(|| Regex::new(r"</?[a-zA-Z]+[0-9]+/?>").unwrap());
static OMEGAT_TAG_DECOMPILE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<(/?)([a-zA-Z]+)([0-9]+)(/?)>").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    Start,
    End,
    Single,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub pos: usize,
    pub tag: String,
}

impl Tag {
    pub fn new(pos: usize, tag: impl Into<String>) -> Self {
        Self {
            pos,
            tag: tag.into(),
        }
    }

    /// Java `TagUtil.Tag.getType` via `PatternConsts.OMEGAT_TAG_DECOMPILE`.
    pub fn tag_type(&self) -> TagType {
        let Some(c) = OMEGAT_TAG_DECOMPILE.captures(&self.tag) else {
            return TagType::Single;
        };
        let front = c.get(1).map(|m| m.as_str() == "/").unwrap_or(false);
        let back = c.get(4).map(|m| m.as_str() == "/").unwrap_or(false);
        if front && !back {
            TagType::End
        } else if !front && !back {
            TagType::Start
        } else {
            TagType::Single
        }
    }

    pub fn name(&self) -> String {
        let Some(c) = OMEGAT_TAG_DECOMPILE.captures(&self.tag) else {
            return self.tag.clone();
        };
        let front = c.get(1).map(|m| m.as_str() == "/").unwrap_or(false);
        let back = c.get(4).map(|m| m.as_str() == "/").unwrap_or(false);
        if front && back {
            return self.tag.clone();
        }
        format!("{}{}", &c[2], &c[3])
    }

    pub fn paired_tag(&self) -> Option<String> {
        match self.tag_type() {
            TagType::Start => Some(format!("</{}>", self.name())),
            TagType::End => Some(format!("<{}>", self.name())),
            TagType::Single => None,
        }
    }
}

/// Java `TagUtil.buildTagList`.
pub fn build_tag_list(str: &str, protected: &[&str]) -> Vec<Tag> {
    let mut out = Vec::new();
    if protected.is_empty() {
        for m in OMEGAT_TAG.find_iter(str) {
            out.push(Tag::new(m.start(), m.as_str()));
        }
        return out;
    }
    let mut from = 0;
    for p in protected {
        if p.is_empty() {
            continue;
        }
        if let Some(rel) = str[from..].find(p) {
            let pos = from + rel;
            out.push(Tag::new(pos, *p));
            from = pos + p.len();
        }
    }
    out.sort_by_key(|t| t.pos);
    out
}

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
    let src = crate::tag_validation::tags_from_strings(
        &extract_tags(source).iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    let loc = crate::tag_validation::tags_from_strings(
        &extract_tags(target).iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    let report = crate::tag_validation::inspect_ordered_tags(&src, &loc, false);
    let mut errors: Vec<TagErrorKind> = report
        .src_errors
        .iter()
        .chain(report.trans_errors.iter())
        .filter_map(|(_, e)| match e {
            crate::tag_validation::TagError::Missing => Some(TagErrorKind::Missing),
            crate::tag_validation::TagError::Extraneous => Some(TagErrorKind::Extraneous),
            crate::tag_validation::TagError::Order => Some(TagErrorKind::Order),
            crate::tag_validation::TagError::Duplicate => Some(TagErrorKind::Duplicate),
            crate::tag_validation::TagError::Malformed => Some(TagErrorKind::Malformed),
            crate::tag_validation::TagError::Orphaned => Some(TagErrorKind::Orphaned),
            crate::tag_validation::TagError::Whitespace => Some(TagErrorKind::Whitespace),
            crate::tag_validation::TagError::Unspecified => None,
        })
        .collect();
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
    let src_tags = crate::tag_validation::tags_from_strings(
        &extract_tags(source).iter().map(|s| s.as_str()).collect::<Vec<_>>(),
    );
    let mut out = target.to_string();
    for t in extract_tags(target) {
        if !src_tags.iter().any(|s| s.tag == t) {
            crate::tag_repair::fix_extraneous(&mut out, &crate::tag_validation::Tag::new(-1, t));
        }
    }
    for t in &src_tags {
        if !out.contains(t.tag.as_str()) {
            crate::tag_repair::fix_missing(&src_tags, &mut out, t);
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

    #[test]
    fn all_kinds_and_repair() {
        assert!(validate("a {1} b {2}", "a {2} b {1}").contains(&TagErrorKind::Order));
        assert!(validate("a {1}", "a {1} {9}").contains(&TagErrorKind::Extraneous));
        assert!(validate("  hi", "hi").contains(&TagErrorKind::Whitespace));
        assert!(validate("<b0>x</b0>", "<b0>x").contains(&TagErrorKind::Orphaned));
        let fixed = repair("Hello <b0>x</b0>", "Hello x <i0>");
        assert!(fixed.contains("<b0>"));
        assert!(!fixed.contains("<i0>"));
    }
}
