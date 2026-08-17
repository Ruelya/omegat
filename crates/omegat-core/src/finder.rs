//! External Finder: URL / command templates compatible with OmegaT finder XML.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinderItem {
    pub name: String,
    pub url: Option<String>,
    pub command: Option<String>,
    pub keystroke: Option<String>,
    pub scope: String,
}

pub fn parse_finder_xml(raw: &str) -> Vec<FinderItem> {
    let mut items = Vec::new();
    let mut rest = raw;
    while let Some(s) = rest.find("<item") {
        let slice = &rest[s..];
        let end = slice.find("</item>").unwrap_or(slice.len());
        let block = &slice[..end];
        items.push(FinderItem {
            name: tag(block, "name").unwrap_or_else(|| attr(block, "name").unwrap_or_default()),
            url: tag(block, "url").or_else(|| attr(block, "url")),
            command: tag(block, "command").or_else(|| attr(block, "command")),
            keystroke: tag(block, "keystroke").or_else(|| attr(block, "keystroke")),
            scope: tag(block, "scope").or_else(|| attr(block, "scope")).unwrap_or_else(|| "selection".into()),
        });
        rest = &rest[s + 5..];
    }
    items
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
        let u = expand(&items[0], "cat", "", "").unwrap();
        assert!(u.contains("cat"));
        let cmd = parse_finder_xml(r#"<item><name>echo</name><command>echo {sourceText}</command></item>"#);
        let exp = expand(&cmd[0], "x", "hello world", "").unwrap();
        assert!(exp.contains("hello"));
    }
}
