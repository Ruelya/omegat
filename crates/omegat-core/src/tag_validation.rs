//! Java `org.omegat.core.tagvalidation.TagValidation`.

use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;

static OMEGAT_TAG_DECOMPILE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"<(/?)([a-zA-Z]+)([0-9]+)(/?)>").unwrap());
static SIMPLE_PRINTF: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"%([1-9]+\$)?([0-9]*)(\.[0-9]*)?[bcdeEfFgGinopsuxX%]").unwrap());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TagError {
    Missing,
    Extraneous,
    Order,
    Duplicate,
    Malformed,
    Orphaned,
    Whitespace,
    Unspecified,
}

impl TagError {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "MISSING",
            Self::Extraneous => "EXTRANEOUS",
            Self::Order => "ORDER",
            Self::Duplicate => "DUPLICATE",
            Self::Malformed => "MALFORMED",
            Self::Orphaned => "ORPHANED",
            Self::Whitespace => "WHITESPACE",
            Self::Unspecified => "UNSPECIFIED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagType {
    Start,
    End,
    Single,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub pos: i32,
    pub tag: String,
}

impl Tag {
    pub fn new(pos: i32, tag: impl Into<String>) -> Self {
        Self {
            pos,
            tag: tag.into(),
        }
    }

    pub fn get_type(&self) -> TagType {
        let Some(caps) = OMEGAT_TAG_DECOMPILE.captures(&self.tag) else {
            return TagType::Single;
        };
        let front = caps.get(1).map(|m| m.as_str() == "/").unwrap_or(false);
        let back = caps.get(4).map(|m| m.as_str() == "/").unwrap_or(false);
        if front && !back {
            TagType::End
        } else if !front && !back {
            TagType::Start
        } else {
            TagType::Single
        }
    }

    pub fn get_name(&self) -> String {
        let Some(caps) = OMEGAT_TAG_DECOMPILE.captures(&self.tag) else {
            return self.tag.clone();
        };
        let front = caps.get(1).map(|m| m.as_str() == "/").unwrap_or(false);
        let back = caps.get(4).map(|m| m.as_str() == "/").unwrap_or(false);
        if front && back {
            return self.tag.clone();
        }
        format!("{}{}", &caps[2], &caps[3])
    }

    pub fn paired_tag(&self) -> Option<String> {
        match self.get_type() {
            TagType::Start => Some(format!("</{}>", self.get_name())),
            TagType::End => Some(format!("<{}>", self.get_name())),
            TagType::Single => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ErrorReport {
    pub src_errors: Vec<(Tag, TagError)>,
    pub trans_errors: Vec<(Tag, TagError)>,
}

impl ErrorReport {
    fn put_src(&mut self, tag: Tag, err: TagError) {
        if !self
            .src_errors
            .iter()
            .any(|(t, _)| t.tag == tag.tag && t.pos == tag.pos)
        {
            self.src_errors.push((tag, err));
        } else {
            self.src_errors
                .retain(|(t, _)| !(t.tag == tag.tag && t.pos == tag.pos));
            self.src_errors.push((tag, err));
        }
    }

    fn put_trans(&mut self, tag: Tag, err: TagError) {
        if let Some(existing) = self
            .trans_errors
            .iter_mut()
            .find(|(t, _)| t.tag == tag.tag && t.pos == tag.pos)
        {
            existing.1 = err;
        } else {
            self.trans_errors.push((tag, err));
        }
    }

    pub fn src_map(&self) -> Vec<(String, String)> {
        let mut v: Vec<_> = self
            .src_errors
            .iter()
            .map(|(t, e)| (t.tag.clone(), e.as_str().to_string()))
            .collect();
        v.sort();
        v
    }

    pub fn trans_map(&self) -> Vec<(String, String)> {
        let mut v: Vec<_> = self
            .trans_errors
            .iter()
            .map(|(t, e)| (t.tag.clone(), e.as_str().to_string()))
            .collect();
        v.sort();
        v
    }
}

fn contains_tag(tags: &[Tag], needle: &str) -> bool {
    tags.iter().any(|t| t.tag == needle)
}

fn get_common_tags(orig: &[Tag], compare: &[Tag]) -> Vec<Tag> {
    let mut result = Vec::new();
    let mut uninspected: Vec<Tag> = compare.to_vec();
    for o in orig {
        if let Some(idx) = uninspected.iter().position(|c| c.tag == o.tag) {
            result.push(o.clone());
            uninspected.remove(idx);
        }
    }
    result
}

fn remove_tag(tags: &mut Vec<Tag>, tag: &str) -> Option<Tag> {
    if let Some(i) = tags.iter().position(|t| t.tag == tag) {
        Some(tags.remove(i))
    } else {
        None
    }
}

/// Java `TagValidation.inspectOrderedTags`.
pub fn inspect_ordered_tags(src_tags: &[Tag], loc_tags: &[Tag], loose: bool) -> ErrorReport {
    let mut report = ErrorReport::default();
    if !loose {
        let mut common_src = get_common_tags(src_tags, loc_tags);
        let mut common_loc = get_common_tags(loc_tags, src_tags);
        let mut i = 0;
        while i < common_src.len() {
            if common_loc[i].tag != common_src[i].tag {
                report.put_trans(common_loc[i].clone(), TagError::Order);
                common_src.remove(i);
                common_loc.remove(i);
            } else {
                i += 1;
            }
        }
    }

    let mut expected: Vec<Tag> = src_tags.to_vec();
    let mut stack: Vec<Tag> = Vec::new();
    for tag in loc_tags {
        if !contains_tag(src_tags, &tag.tag) {
            report.put_trans(tag.clone(), TagError::Extraneous);
            continue;
        }
        if remove_tag(&mut expected, &tag.tag).is_none() {
            report.put_trans(tag.clone(), TagError::Duplicate);
            continue;
        }
        match tag.get_type() {
            TagType::Start => {
                if let Some(end) = tag.paired_tag() {
                    if contains_tag(src_tags, &end) {
                        stack.push(tag.clone());
                    }
                }
            }
            TagType::End => {
                if stack.last().is_some_and(|t| t.get_name() == tag.get_name()) {
                    stack.pop();
                } else {
                    while let Some(last) = stack.pop() {
                        report.put_trans(last.clone(), TagError::Malformed);
                        if last.get_name() == tag.get_name() {
                            break;
                        }
                    }
                    if stack.is_empty() {
                        if let Some(pair) = tag.paired_tag() {
                            if contains_tag(src_tags, &pair) {
                                let err = if contains_tag(loc_tags, &pair) {
                                    TagError::Malformed
                                } else {
                                    TagError::Orphaned
                                };
                                report.put_trans(tag.clone(), err);
                            }
                        }
                    }
                }
            }
            TagType::Single => {}
        }
    }
    for tag in expected {
        report.put_src(tag, TagError::Missing);
    }
    while let Some(tag) = stack.pop() {
        if let Some(pair) = tag.paired_tag() {
            if contains_tag(src_tags, &pair) {
                let err = if contains_tag(loc_tags, &pair) {
                    TagError::Malformed
                } else {
                    TagError::Orphaned
                };
                report.put_trans(tag, err);
            }
        }
    }
    report
}

/// Java `TagValidation.inspectUnorderedTags`.
pub fn inspect_unordered_tags(src_tags: &[Tag], loc_tags: &[Tag]) -> ErrorReport {
    let mut report = ErrorReport::default();
    for tag in src_tags {
        if !contains_tag(loc_tags, &tag.tag) {
            report.put_src(tag.clone(), TagError::Missing);
        }
    }
    for tag in loc_tags {
        if !contains_tag(src_tags, &tag.tag) {
            report.put_trans(tag.clone(), TagError::Extraneous);
        }
    }
    report
}

fn extract_printf_vars(text: &str) -> HashMap<String, Tag> {
    let mut map = HashMap::new();
    let mut index = 1i32;
    for caps in SIMPLE_PRINTF.captures_iter(text) {
        let full = caps.get(0).unwrap();
        let variable = full.as_str();
        let swap = caps.get(1).map(|m| m.as_str());
        let last = variable.chars().last().unwrap_or('%');
        if let Some(spec) = swap {
            if spec.ends_with('$') {
                let num = &spec[..spec.len() - 1];
                map.insert(
                    format!("{num}{last}"),
                    Tag::new(full.start() as i32, variable),
                );
                continue;
            }
        }
        map.insert(
            format!("{index}{last}"),
            Tag::new(full.start() as i32, variable),
        );
        index += 1;
    }
    map
}

/// Java `TagValidation.inspectPrintfVariables` (simple check).
pub fn inspect_printf_variables(source: &str, translation: &str) -> ErrorReport {
    let mut report = ErrorReport::default();
    let src = extract_printf_vars(source);
    let loc = extract_printf_vars(translation);
    if src.keys().collect::<std::collections::BTreeSet<_>>()
        != loc.keys().collect::<std::collections::BTreeSet<_>>()
    {
        for t in src.into_values() {
            report.put_src(t, TagError::Unspecified);
        }
        for t in loc.into_values() {
            report.put_trans(t, TagError::Unspecified);
        }
    }
    report
}

/// Java `TagValidation.inspectRemovePattern`.
pub fn inspect_remove_pattern(translation: &str, pattern: &str) -> ErrorReport {
    let mut report = ErrorReport::default();
    if pattern.is_empty() {
        return report;
    }
    if let Ok(re) = Regex::new(pattern) {
        for m in re.find_iter(translation) {
            report.put_trans(Tag::new(m.start() as i32, m.as_str()), TagError::Extraneous);
        }
    }
    report
}

pub fn tags_from_strings(tags: &[&str]) -> Vec<Tag> {
    tags.iter().map(|t| Tag::new(-1, *t)).collect()
}
