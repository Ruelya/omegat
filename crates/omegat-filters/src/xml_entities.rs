//! External / parameter XML entity expansion matching Java Xerces + `Handler`.
//!
//! Parse: SYSTEM general entities are inlined (file body, xml decl stripped).
//! Write: those same refs become a single newline in the main file (Java writes
//! the entity body to a sidecar and leaves `\n` in the parent).
//! Internal entities stay as `&name;` so `Handler` can emit `Element::Entity`.

use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct EntityDecl {
    pub parameter: bool,
    pub name: String,
    pub system_id: Option<String>,
    pub public_id: Option<String>,
    pub value: Option<String>,
    pub reference: bool,
}

#[derive(Clone, Debug, Default)]
pub struct XmlEntityPrep {
    pub text: String,
    pub internal: HashMap<String, String>,
    #[allow(dead_code)]
    pub system_names: HashSet<String>,
    #[allow(dead_code)]
    pub decls: Vec<EntityDecl>,
}

pub fn prepare_xml(
    raw: &str,
    base: Option<&Path>,
    inline_system: bool,
) -> Result<XmlEntityPrep, String> {
    let decls = parse_dtd_decls(raw);
    let mut internal = HashMap::new();
    let mut system_names = HashSet::new();
    let mut system_files: HashMap<String, String> = HashMap::new();
    let mut param_files: HashMap<String, String> = HashMap::new();

    for d in &decls {
        if d.reference {
            continue;
        }
        if let Some(val) = &d.value {
            if !d.parameter {
                internal.insert(d.name.clone(), val.clone());
            }
        }
        if let Some(sys) = &d.system_id {
            if d.parameter {
                param_files.insert(d.name.clone(), sys.clone());
            } else {
                system_names.insert(d.name.clone());
                system_files.insert(d.name.clone(), sys.clone());
            }
        }
    }

    if let Some(dir) = base {
        for (name, sys) in &param_files {
            if let Ok(body) = read_entity_file(dir, sys) {
                for cap in regex::Regex::new(r#"<!ENTITY\s+(\w+)\s+"([^"]*)""#)
                    .unwrap()
                    .captures_iter(&body)
                {
                    internal.insert(cap[1].to_string(), cap[2].to_string());
                }
                let _ = name;
            }
        }
    }

    let mut text = raw.to_string();
    if let Some(dir) = base {
        if inline_system {
            for (name, sys) in &system_files {
                let body = read_entity_file(dir, sys)?;
                let token = format!("&{name};");
                text = text.replace(&token, &body);
            }
        } else {
            for name in &system_names {
                let token = format!("&{name};");
                text = text.replace(&token, "\n");
            }
        }
    }

    Ok(XmlEntityPrep {
        text,
        internal,
        system_names,
        decls,
    })
}

fn read_entity_file(base: &Path, system_id: &str) -> Result<String, String> {
    if system_id.starts_with("http://") || system_id.starts_with("https://") {
        return Ok(String::new());
    }
    let path = base.join(system_id);
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(strip_xml_decl(&raw))
}

fn strip_xml_decl(s: &str) -> String {
    let re = regex::Regex::new(r#"(?s)^\s*<\?xml[^?]*\?>\s*"#).unwrap();
    re.replace(s, "").into_owned()
}

pub fn parse_dtd_decls(raw: &str) -> Vec<EntityDecl> {
    let Some(subset) = internal_subset(raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let decl_re = regex::Regex::new(
        r#"<!ENTITY\s+(?:%\s+)?(\w+)\s+(?:SYSTEM\s+"([^"]*)"|PUBLIC\s+"([^"]*)"\s+"([^"]*)"|"([^"]*)")\s*>"#,
    )
    .unwrap();
    let param_flag = regex::Regex::new(r#"<!ENTITY\s+%"#).unwrap();
    for cap in decl_re.captures_iter(&subset) {
        let full = cap.get(0).unwrap().as_str();
        let parameter = param_flag.is_match(full);
        let name = cap[1].to_string();
        if let Some(sys) = cap.get(2) {
            out.push(EntityDecl {
                parameter,
                name,
                system_id: Some(sys.as_str().to_string()),
                public_id: None,
                value: None,
                reference: false,
            });
        } else if let Some(pub_id) = cap.get(3) {
            out.push(EntityDecl {
                parameter,
                name,
                system_id: cap.get(4).map(|m| m.as_str().to_string()),
                public_id: Some(pub_id.as_str().to_string()),
                value: None,
                reference: false,
            });
        } else {
            out.push(EntityDecl {
                parameter,
                name,
                system_id: None,
                public_id: None,
                value: cap.get(5).map(|m| m.as_str().to_string()),
                reference: false,
            });
        }
    }
    let ref_re = regex::Regex::new(r#"%(\w+);"#).unwrap();
    for cap in ref_re.captures_iter(&subset) {
        out.push(EntityDecl {
            parameter: true,
            name: cap[1].to_string(),
            system_id: None,
            public_id: None,
            value: None,
            reference: true,
        });
    }
    out
}

fn internal_subset(raw: &str) -> Option<String> {
    let start = raw.find("<!DOCTYPE")?;
    let after = &raw[start..];
    let open = after.find('[')?;
    let close = after[open..].find(']')?;
    Some(after[open + 1..open + close].to_string())
}

/// Java `DTD.toOriginal` reconstruction from the source subset.
pub fn reconstruct_doctype_from_source(body: &str) -> String {
    let body = body.replace("\r\n", "\n").replace('\r', "\n");
    let name = body.split_whitespace().next().unwrap_or("").to_string();
    let public_id = regex::Regex::new(r#"PUBLIC\s+"([^"]*)""#)
        .ok()
        .and_then(|re| re.captures(&body).map(|c| c[1].to_string()));
    let system_id = if public_id.is_some() {
        regex::Regex::new(r#"PUBLIC\s+"[^"]*"\s+"([^"]*)""#)
            .ok()
            .and_then(|re| re.captures(&body).map(|c| c[1].to_string()))
    } else {
        regex::Regex::new(r#"SYSTEM\s+"([^"]*)""#)
            .ok()
            .and_then(|re| re.captures(&body).map(|c| c[1].to_string()))
    };
    let decls = parse_dtd_decls(&format!("<!DOCTYPE {name} [ {} ]>", extract_subset_from_body(&body)));
    let mut res = format!("<!DOCTYPE {name}");
    if let Some(p) = &public_id {
        res.push_str(" PUBLIC \"");
        res.push_str(p);
        res.push('"');
    }
    if let Some(s) = &system_id {
        if public_id.is_none() {
            res.push_str(" SYSTEM");
        }
        res.push_str(" \"");
        res.push_str(s);
        res.push('"');
    }
    if !decls.is_empty() {
        res.push_str("\n[\n");
        for d in decls {
            res.push_str(&entity_to_java(&d));
            res.push('\n');
        }
        res.push(']');
    }
    res.push_str(">\n");
    res
}

fn extract_subset_from_body(body: &str) -> String {
    if let Some(open) = body.find('[') {
        if let Some(close) = body.rfind(']') {
            if close > open {
                return body[open + 1..close].to_string();
            }
        }
    }
    String::new()
}

fn entity_to_java(d: &EntityDecl) -> String {
    if d.reference {
        if d.parameter {
            return format!("%{};", d.name);
        }
        return format!("&{};", d.name);
    }
    let mut res = String::from("<!ENTITY");
    if d.parameter {
        res.push_str(" %");
    }
    res.push(' ');
    res.push_str(&d.name);
    if let Some(val) = &d.value {
        res.push_str(" \"");
        res.push_str(val);
        res.push('"');
    } else {
        if let Some(p) = &d.public_id {
            res.push_str(" PUBLIC \"");
            res.push_str(p);
            res.push('"');
        }
        if let Some(s) = &d.system_id {
            res.push_str(" SYSTEM \"");
            res.push_str(s);
            res.push('"');
        }
    }
    res.push('>');
    res
}

/// DocBook / Xerces rejects a `para` (and similar leaf tags) opened while
/// another of the same name is still open. Line number is 1-based.
pub fn reject_self_nested_leaf_tags(raw: &str) -> Result<(), String> {
    const LEAF: &[&str] = &["para", "simpara"];
    let mut stack: Vec<(String, usize)> = Vec::new();
    let mut line = 1usize;
    let mut i = 0;
    let bytes = raw.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\n' {
            line += 1;
            i += 1;
            continue;
        }
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if raw[i..].starts_with("<!--") {
            if let Some(end) = raw[i + 4..].find("-->") {
                line += raw[i..i + 4 + end].matches('\n').count();
                i += 4 + end + 3;
                continue;
            }
        }
        if raw[i..].starts_with("<![CDATA[") {
            if let Some(end) = raw[i + 9..].find("]]>") {
                line += raw[i..i + 9 + end].matches('\n').count();
                i += 9 + end + 3;
                continue;
            }
        }
        if raw[i..].starts_with("<!DOCTYPE") {
            if let Some(rel) = raw[i..].find('>') {
                let chunk = &raw[i..i + rel];
                if let Some(br) = chunk.find('[') {
                    if let Some(end) = raw[i + br..].find("]>") {
                        line += raw[i..i + br + end].matches('\n').count();
                        i += br + end + 2;
                        continue;
                    }
                }
                line += chunk.matches('\n').count();
                i += rel + 1;
                continue;
            }
        }
        if raw[i..].starts_with("<?") {
            if let Some(end) = raw[i + 2..].find("?>") {
                line += raw[i..i + 2 + end].matches('\n').count();
                i += 2 + end + 2;
                continue;
            }
        }
        let rest = &raw[i + 1..];
        let closing = rest.starts_with('/');
        let name_src = if closing { &rest[1..] } else { rest };
        let name: String = name_src
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == ':' || *c == '_' || *c == '-')
            .collect();
        if name.is_empty() {
            i += 1;
            continue;
        }
        let tag_end = raw[i..]
            .find('>')
            .map(|n| i + n)
            .unwrap_or(raw.len().saturating_sub(1));
        let self_close = raw[i..=tag_end].trim_end().ends_with("/>");
        if closing {
            if let Some(pos) = stack.iter().rposition(|(n, _)| n == &name) {
                stack.truncate(pos);
            }
        } else if !self_close {
            if LEAF.contains(&name.as_str()) && stack.iter().any(|(n, _)| n == &name) {
                return Err(format!(
                    "The element type \"{name}\" must be terminated by the matching end-tag at line {line}"
                ));
            }
            stack.push((name, line));
        }
        line += raw[i..=tag_end].matches('\n').count();
        i = tag_end + 1;
    }
    Ok(())
}
