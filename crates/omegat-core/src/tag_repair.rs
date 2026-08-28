//! Java `org.omegat.core.tagvalidation.TagRepair`.

use crate::tag_validation::Tag;

/// Java `TagRepair.fixExtraneous`.
pub fn fix_extraneous(text: &mut String, tag: &Tag) {
    let tag_end = tag.pos + tag.tag.len() as i32;
    if tag.pos > 0
        && tag_end < text.len() as i32
        && text
            .get(tag.pos as usize..tag_end as usize)
            .is_some_and(|s| s == tag.tag)
    {
        text.replace_range(tag.pos as usize..tag_end as usize, "");
    } else if let Some(i) = text.find(&tag.tag) {
        text.replace_range(i..i + tag.tag.len(), "");
    }
}

fn tag_index(tags: &[Tag], tag: &Tag) -> i32 {
    tags.iter()
        .position(|t| t.tag == tag.tag)
        .map(|i| i as i32)
        .unwrap_or(-1)
}

/// Java `TagRepair.fixMissing`.
pub fn fix_missing(tags: &[Tag], text: &mut String, tag: &Tag) {
    let index = tag_index(tags, tag);
    let prev = if index > 0 {
        tags.get((index - 1) as usize)
    } else {
        None
    };
    let next = if index >= 0 && (index as usize) + 1 < tags.len() {
        tags.get(index as usize + 1)
    } else {
        None
    };
    if let Some(prev) = prev {
        if let Some(pos) = text.find(&prev.tag) {
            let at = pos + prev.tag.len();
            text.insert_str(at, &tag.tag);
            return;
        }
    }
    if let Some(next) = next {
        if let Some(pos) = text.find(&next.tag) {
            text.insert_str(pos, &tag.tag);
            return;
        }
    }
    text.push_str(&tag.tag);
}

/// Java `TagRepair.fixMalformed`.
pub fn fix_malformed(tags: &[Tag], text: &mut String, tag: &Tag) {
    fix_extraneous(text, tag);
    fix_missing(tags, text, tag);
}

/// Java `TagRepair.fixWhitespace` (PO leading/trailing newline only).
pub fn fix_whitespace(text: &mut String, source: &str) {
    if source.starts_with('\n') && !text.starts_with('\n') {
        text.insert(0, '\n');
    } else if !source.starts_with('\n') && text.starts_with('\n') {
        text.remove(0);
    }
    if source.ends_with('\n') && !text.ends_with('\n') {
        text.push('\n');
    } else if !source.ends_with('\n') && text.ends_with('\n') {
        text.pop();
    }
}
