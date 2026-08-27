//! External Finder: URL / command templates compatible with OmegaT finder XML.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinderItem {
    pub name: String,
    pub url: Option<String>,
    pub urls: Vec<String>,
    pub command: Option<String>,
    pub commands: Vec<String>,
    pub keystroke: Option<String>,
    pub scope: String,
    pub nopopup: bool,
    pub ascii_only: bool,
    pub non_ascii_only: bool,
}

pub fn parse_finder_xml(raw: &str) -> Vec<FinderItem> {
    let mut items = Vec::new();
    let mut rest = raw;
    while let Some(s) = find_item_tag(rest) {
        let slice = &rest[s..];
        let end = slice.find("</item>").unwrap_or(slice.len());
        let block = &slice[..end];
        let urls = tags(block, "url");
        let commands = tags(block, "command");
        let ascii_only = block.contains("target=\"ascii_only\"") && !block.contains("target=\"both\"");
        let non_ascii_only = block.contains("target=\"non_ascii_only\"");
        items.push(FinderItem {
            name: unescape_xml(&tag(block, "name").unwrap_or_else(|| attr(block, "name").unwrap_or_default())),
            url: urls.first().cloned(),
            urls,
            command: commands.first().cloned(),
            commands,
            keystroke: tag(block, "keystroke").or_else(|| attr(block, "keystroke")),
            scope: tag(block, "scope").or_else(|| attr(block, "scope")).unwrap_or_else(|| "selection".into()),
            nopopup: block.contains("nopopup=\"true\""),
            ascii_only,
            non_ascii_only,
        });
        rest = &rest[s + 5..];
    }
    items
}

/// `<item` but not the `<items>` wrapper.
fn find_item_tag(raw: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(rel) = raw[start..].find("<item") {
        let abs = start + rel;
        let after = raw.get(abs + 5..).and_then(|s| s.chars().next())?;
        if after == '>' || after.is_whitespace() {
            return Some(abs);
        }
        start = abs + 5;
    }
    None
}

fn tags(raw: &str, name: &str) -> Vec<String> {
    let mut out = Vec::new();
    let open = format!("<{name}");
    let close = format!("</{name}>");
    let mut rest = raw;
    while let Some(s) = rest.find(&open) {
        let after = &rest[s..];
        if let Some(gt) = after.find('>') {
            if let Some(end) = after[gt + 1..].find(&close) {
                out.push(unescape_xml(&after[gt + 1..gt + 1 + end]));
                rest = &after[gt + 1 + end + close.len()..];
                continue;
            }
        }
        break;
    }
    out
}

fn unescape_xml(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
}

pub fn expand(item: &FinderItem, selection: &str, source: &str, target: &str) -> Option<String> {
    let mut t = item.url.clone().or(item.command.clone())?;
    t = t.replace("{selection}", &urlencoding::encode(selection));
    t = t.replace("{sourceText}", &urlencoding::encode(source));
    t = t.replace("{targetText}", &urlencoding::encode(target));
    t = t.replace("{source}", &urlencoding::encode(source));
    t = t.replace("{target}", &urlencoding::encode(target));
    Some(t)
}

fn tag(raw: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let start = raw.find(&open)? + open.len();
    let close = format!("</{name}>");
    let end = raw[start..].find(&close)? + start;
    Some(raw[start..end].trim().to_string())
}

fn attr(raw: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let s = raw.find(&key)? + key.len();
    let e = raw[s..].find('"')? + s;
    Some(raw[s..e].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_finder_xml() {
        let xml = r#"<items><item><name>Wiktionary</name><url>https://en.wiktionary.org/wiki/{selection}</url><scope>selection</scope></item></items>"#;
        let items = parse_finder_xml(xml);
        assert_eq!(items[0].name, "Wiktionary");
        assert_eq!(items[0].urls.len(), 1);
        let u = expand(&items[0], "cat", "", "").unwrap();
        assert!(u.contains("cat"));
        let cmd = parse_finder_xml(r#"<item><name>echo</name><command>echo {sourceText}</command></item>"#);
        let exp = expand(&cmd[0], "x", "hello world", "").unwrap();
        assert!(exp.contains("hello"));
    }
}
