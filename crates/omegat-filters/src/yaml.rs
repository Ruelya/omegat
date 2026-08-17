//! Java `org.omegat.filters2.text.yaml.YamlFilter`.
//! Only string scalars are extracted. Keys `include` / `exclude` are read.

use crate::{
    ensure_parent, read_to_string, ExtractedSegment, Filter, FilterContext, ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct YamlFilter;

impl Filter for YamlFilter {
    fn id(&self) -> &'static str {
        "yaml"
    }
    fn name(&self) -> &'static str {
        "YAML"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.yaml", "*.yml"]
    }
    fn parse(&self, path: &Path, ctx: &FilterContext) -> Result<ParsedFile> {
        let raw = read_to_string(path)?;
        let root = parse_doc(&raw)?;
        let mut segments = Vec::new();
        let mut counters = HashMap::new();
        walk(
            &root,
            None,
            ctx,
            &mut counters,
            &mut segments,
            &HashMap::new(),
        );
        Ok(ParsedFile {
            segments,
            skeleton: Some(raw),
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_to_string(source_path)?;
        let root = parse_doc(&raw)?;
        let mut counters = HashMap::new();
        let mut segments = Vec::new();
        let translated = walk(&root, None, ctx, &mut counters, &mut segments, translations);
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, emit_yaml(&translated))?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
enum Node {
    Str(String),
    Other(String),
    Map(Vec<(String, Node)>),
    Arr(Vec<Node>),
}

fn include_exclude(ctx: &FilterContext) -> (Vec<String>, Vec<String>) {
    let split = |k: &str| {
        ctx.option(k)
            .map(|s| {
                s.split(';')
                    .filter(|p| !p.is_empty())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    };
    (split("include"), split("exclude"))
}

fn path_matches(path: &str, patterns: &[String]) -> bool {
    patterns.iter().any(|p| {
        if p == path {
            return true;
        }
        let re = wildcard_to_regex(p);
        regex::Regex::new(&re)
            .map(|r| r.is_match(path))
            .unwrap_or(false)
    })
}

fn wildcard_to_regex(pattern: &str) -> String {
    let mut p = pattern;
    let mut sb = String::new();
    if let Some(rest) = p.strip_prefix("**/") {
        sb.push_str("(?:.*/)?");
        p = rest;
    } else {
        sb.push('^');
    }
    let chars: Vec<char> = p.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' if i + 1 < chars.len() && chars[i + 1] == '*' => {
                sb.push_str(".*");
                i += 2;
            }
            '*' => {
                sb.push_str("[^/]*");
                i += 1;
            }
            '[' | ']' => {
                sb.push('\\');
                sb.push(chars[i]);
                i += 1;
            }
            c if "\\.{}()^$+?|".contains(c) => {
                sb.push('\\');
                sb.push(c);
                i += 1;
            }
            c => {
                sb.push(c);
                i += 1;
            }
        }
    }
    sb.push('$');
    sb
}

fn walk(
    node: &Node,
    path: Option<&str>,
    ctx: &FilterContext,
    counters: &mut HashMap<String, usize>,
    segments: &mut Vec<ExtractedSegment>,
    translations: &HashMap<String, String>,
) -> Node {
    let (include, exclude) = include_exclude(ctx);
    match node {
        Node::Str(src) => {
            let current = path.unwrap_or("");
            if (!include.is_empty() && !path_matches(current, &include))
                || (include.is_empty() && path_matches(current, &exclude))
            {
                return Node::Str(src.clone());
            }
            let index = *counters.get(current).unwrap_or(&0);
            counters.insert(current.to_string(), index + 1);
            let id = format!("{current}_{index}");
            segments.push(ExtractedSegment {
                id: id.clone(),
                source: src.clone(),
                existing_translation: None,
                note: None,
                comment: Some(format!("name={current}")),
                path: Some(current.to_string()),
                protected_parts: vec![],
            });
            let trg = translations
                .get(&id)
                .or_else(|| translations.get(src))
                .cloned()
                .unwrap_or_else(|| src.clone());
            Node::Str(trg)
        }
        Node::Arr(items) => Node::Arr(
            items
                .iter()
                .enumerate()
                .map(|(i, item)| {
                    let next = match path {
                        None | Some("") => format!("[{i}]"),
                        Some(p) => format!("{p}[{i}]"),
                    };
                    walk(item, Some(&next), ctx, counters, segments, translations)
                })
                .collect(),
        ),
        Node::Map(pairs) => Node::Map(
            pairs
                .iter()
                .map(|(k, v)| {
                    let next = match path {
                        None | Some("") => k.clone(),
                        Some(p) => format!("{p}/{k}"),
                    };
                    (
                        k.clone(),
                        walk(v, Some(&next), ctx, counters, segments, translations),
                    )
                })
                .collect(),
        ),
        Node::Other(s) => Node::Other(s.clone()),
    }
}

fn parse_doc(raw: &str) -> Result<Node> {
    let lines: Vec<&str> = raw.lines().collect();
    let mut i = 0usize;
    if lines.first().is_some_and(|l| l.trim() == "---") {
        i = 1;
    }
    parse_value(&lines, &mut i, 0)
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ').count()
}

fn parse_value(lines: &[&str], i: &mut usize, min_indent: usize) -> Result<Node> {
    while *i < lines.len() && (lines[*i].trim().is_empty() || lines[*i].trim().starts_with('#')) {
        *i += 1;
    }
    if *i >= lines.len() {
        return Ok(Node::Map(vec![]));
    }
    let line = lines[*i];
    let ind = indent_of(line);
    if ind < min_indent {
        return Ok(Node::Map(vec![]));
    }
    let trimmed = line.trim();
    if trimmed.starts_with("- ") || trimmed == "-" {
        return parse_arr(lines, i, ind);
    }
    if trimmed.contains(':') {
        return parse_map(lines, i, ind);
    }
    *i += 1;
    Ok(scalar(trimmed))
}

fn parse_map(lines: &[&str], i: &mut usize, map_indent: usize) -> Result<Node> {
    let mut pairs = Vec::new();
    while *i < lines.len() {
        let line = lines[*i];
        if line.trim().is_empty() || line.trim().starts_with('#') {
            *i += 1;
            continue;
        }
        let ind = indent_of(line);
        if ind < map_indent {
            break;
        }
        if ind > map_indent {
            break;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("- ") {
            break;
        }
        let Some((k, rest)) = trimmed.split_once(':') else {
            break;
        };
        *i += 1;
        let rest = rest.trim();
        let val = if rest.is_empty() {
            parse_value(lines, i, map_indent + 1)?
        } else {
            scalar(rest)
        };
        pairs.push((k.to_string(), val));
    }
    Ok(Node::Map(pairs))
}

fn parse_arr(lines: &[&str], i: &mut usize, arr_indent: usize) -> Result<Node> {
    let mut items = Vec::new();
    while *i < lines.len() {
        let line = lines[*i];
        if line.trim().is_empty() {
            *i += 1;
            continue;
        }
        let ind = indent_of(line);
        if ind < arr_indent {
            break;
        }
        let trimmed = line.trim();
        if !trimmed.starts_with("- ") && trimmed != "-" {
            break;
        }
        *i += 1;
        let rest = trimmed.strip_prefix("- ").unwrap_or("").trim();
        let val = if rest.is_empty() {
            parse_value(lines, i, arr_indent + 1)?
        } else {
            scalar(rest)
        };
        items.push(val);
    }
    Ok(Node::Arr(items))
}

fn scalar(s: &str) -> Node {
    if (s.starts_with('"') && s.ends_with('"') && s.len() >= 2)
        || (s.starts_with('\'') && s.ends_with('\'') && s.len() >= 2)
    {
        return Node::Str(s[1..s.len() - 1].to_string());
    }
    if s == "true" || s == "false" || s == "null" || s.parse::<i64>().is_ok() {
        return Node::Other(s.to_string());
    }
    Node::Str(s.to_string())
}

fn emit_yaml(node: &Node) -> String {
    let mut out = String::from("---\n");
    emit(node, 0, &mut out, false);
    out
}

fn emit(node: &Node, indent: usize, out: &mut String, in_array: bool) {
    let pad = " ".repeat(indent);
    match node {
        Node::Str(s) => {
            if in_array {
                out.push_str(&pad);
                out.push_str("- ");
            }
            out.push('"');
            out.push_str(&s.replace('\\', "\\\\").replace('"', "\\\""));
            out.push_str("\"\n");
        }
        Node::Other(s) => {
            if in_array {
                out.push_str(&pad);
                out.push_str("- ");
            }
            out.push_str(s);
            out.push('\n');
        }
        Node::Map(pairs) => {
            for (k, v) in pairs {
                match v {
                    Node::Str(_) | Node::Other(_) => {
                        out.push_str(&pad);
                        out.push_str(k);
                        out.push_str(": ");
                        emit(v, indent, out, false);
                    }
                    Node::Arr(_) => {
                        out.push_str(&pad);
                        out.push_str(k);
                        out.push_str(":\n");
                        emit(v, indent, out, false);
                    }
                    Node::Map(_) => {
                        out.push_str(&pad);
                        out.push_str(k);
                        out.push_str(":\n");
                        emit(v, indent + 2, out, false);
                    }
                }
            }
        }
        Node::Arr(items) => {
            for item in items {
                match item {
                    Node::Str(_) | Node::Other(_) => emit(item, indent, out, true),
                    _ => {
                        out.push_str(&pad);
                        out.push_str("-\n");
                        emit(item, indent + 2, out, false);
                    }
                }
            }
        }
    }
}

