//! Java `org.omegat.filters4.xml.openxml.OpenXmlFilter`.

use super::abstract_xliff::omegat_tag;
use super::abstract_xml::{process_xml_string_ex, StaxFilter};
use super::stax::{from_event_to_writer, QName, StaxWriter, XmlDeclStyle, XmlEvent};
use crate::{ExtractedSegment, FilterContext, Result};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

pub struct OpenXmlProc {
    remove_comments: bool,
    do_compact_tags: bool,
    source_lang: String,
    target_lang: String,
    main_para: Option<QName>,
    current_para: Vec<Vec<XmlEvent>>,
    current_buf: Option<usize>,
    tags_map: BTreeMap<String, Vec<XmlEvent>>,
    tags_count: BTreeMap<char, i32>,
    defaults_for_paragraph: Option<Vec<XmlEvent>>,
    segments: Vec<ExtractedSegment>,
    translations: BTreeMap<String, String>,
    writing: bool,
}

impl OpenXmlProc {
    pub fn new(ctx: &FilterContext, with_comments: bool, writing: bool) -> Self {
        // Java `OpenXmlFilter.doCompactTags` defaults to false. ZIP usage never
        // calls `isFileSupported` on the inner filter, so compact stays off
        // unless the option is explicitly true.
        let compact = ctx.option_flag("aggregateTags");
        Self {
            remove_comments: !with_comments,
            do_compact_tags: compact,
            source_lang: ctx.source_lang.clone(),
            target_lang: ctx.target_lang.clone(),
            main_para: None,
            current_para: Vec::new(),
            current_buf: None,
            tags_map: BTreeMap::new(),
            tags_count: BTreeMap::new(),
            defaults_for_paragraph: None,
            segments: Vec::new(),
            translations: BTreeMap::new(),
            writing,
        }
    }

    pub fn set_translations(&mut self, map: &std::collections::HashMap<String, String>) {
        self.translations.clear();
        for (k, v) in map {
            if !v.is_empty() {
                self.translations.insert(k.clone(), v.clone());
            }
        }
    }

    fn same_ns(&self, name: &QName) -> bool {
        self.main_para
            .as_ref()
            .map(|m| m.uri == name.uri)
            .unwrap_or(false)
    }

    fn push_ev(&mut self, ev: XmlEvent) {
        if let Some(i) = self.current_buf {
            self.current_para[i].push(ev);
        }
    }

    fn start_para(&mut self, ev: XmlEvent) {
        self.current_para.clear();
        self.current_para.push(vec![ev]);
        self.current_buf = Some(0);
    }

    fn start_run(&mut self, ev: XmlEvent) {
        if self.current_buf.is_none()
            || self
                .current_para
                .last()
                .map(|b| !b.is_empty())
                .unwrap_or(true)
        {
            self.current_para.push(Vec::new());
        }
        let i = self.current_para.len() - 1;
        self.current_para[i].push(ev);
        self.current_buf = Some(i);
    }

    fn flush_translation(&mut self, writer: Option<&mut StaxWriter>) {
        let src = self.build_tags();
        if let Some(writer) = writer {
            if let Some(first) = self.current_para.first() {
                for ev in first {
                    from_event_to_writer(ev, writer);
                }
            }
            if self.current_para.len() > 1 {
                let tra = self
                    .translations
                    .get(&src)
                    .cloned()
                    .unwrap_or_else(|| src.clone());
                let restored = self.restore_openxml(&tra);
                for ev in restored {
                    from_event_to_writer(&ev, writer);
                }
                if let Some(last) = self.current_para.last() {
                    for ev in last {
                        from_event_to_writer(ev, writer);
                    }
                }
            }
        }
        if !src.is_empty() {
            self.segments.push(ExtractedSegment {
                id: String::new(),
                source: src,
                existing_translation: None,
                note: None,
                comment: None,
                path: None,
                protected_parts: vec![],
            });
        }
    }

    fn build_tags(&mut self) -> String {
        self.tags_map.clear();
        for v in self.tags_count.values_mut() {
            *v = 0;
        }
        let mut res = String::new();
        self.defaults_for_paragraph = None;
        let mut i = 0;
        while i < self.current_para.len() {
            let run = self.current_para[i].clone();
            if run.is_empty() {
                i += 1;
                continue;
            }
            if run[0].local_name() == Some("r") && matches!(run[0], XmlEvent::StartElement { .. }) {
                let mut iter = 0;
                let prefix = self.find_prefix(&run, &mut iter);
                let tc = *self.tags_count.get(&prefix).unwrap_or(&0);
                if prefix == 'n' || prefix == 'd' || prefix == 'e' {
                    res.push_str(&format!("<{prefix}{tc}/>"));
                    self.tags_map.insert(format!("{prefix}{tc}"), run);
                    self.tags_count.insert(prefix, tc + 1);
                } else {
                    if prefix != '\0' {
                        res.push_str(&format!("<{prefix}{tc}>"));
                        self.tags_map
                            .insert(format!("{prefix}{tc}"), run[..iter].to_vec());
                    }
                    self.browse_run_contents(&run, &mut iter, &mut res);
                    if prefix != '\0' {
                        res.push_str(&format!("</{prefix}{tc}>"));
                        let end_from = iter.saturating_sub(1);
                        self.tags_map.insert(
                            format!("/{prefix}{tc}"),
                            run[end_from.min(run.len())..].to_vec(),
                        );
                        self.tags_count.insert(prefix, tc + 1);
                    }
                }
            } else {
                if i == 0 {
                    if run.len() > 1 && run[1].local_name() == Some("pPr") {
                        self.defaults_for_paragraph = Some(run.clone());
                        self.apply_paragraph_defaults();
                    }
                    i += 1;
                    continue;
                }
                if i + 1 == self.current_para.len() {
                    break;
                }
                if run.len() == 1 {
                    if let XmlEvent::Characters { data } = &run[0] {
                        if data.trim().is_empty() {
                            i += 1;
                            continue;
                        }
                    }
                }
                let tc = *self.tags_count.get(&'x').unwrap_or(&0);
                res.push_str(&format!("<x{tc}/>"));
                self.tags_map.insert(format!("x{tc}"), run);
                self.tags_count.insert('x', tc + 1);
            }
            i += 1;
        }
        if self.do_compact_tags {
            compact_built_tags(&mut res, &mut self.tags_map, &mut self.tags_count);
        }
        res
    }

    /// Java `buildTags` LOOP2, including in-place collapse of a start whose
    /// attributes arrived as later `ATTRIBUTE` events (or, when they did not,
    /// replacement by an attribute-less start — which is what produces the
    /// `w:szCs`/`w:t` nesting in `testParseTables`).
    fn apply_paragraph_defaults(&mut self) {
        let mut j = 1;
        while j < self.current_para.len() {
            if self.current_para[j].is_empty() {
                self.current_para.remove(j);
                j += 1; // Java `continue LOOP2` still increments
                continue;
            }
            let mut ir = 0;
            if ir >= self.current_para[j].len() {
                self.current_para.remove(j);
                j += 1;
                continue;
            }
            let ev0 = self.current_para[j][ir].clone();
            ir += 1;
            if ev0.local_name() != Some("r") || !matches!(ev0, XmlEvent::StartElement { .. }) {
                j += 1;
                continue;
            }
            if ir >= self.current_para[j].len() {
                self.defaults_for_paragraph = None;
                return;
            }
            let ev1 = self.current_para[j][ir].clone();
            ir += 1;
            if ev1.local_name() != Some("rPr") || !matches!(ev1, XmlEvent::StartElement { .. }) {
                self.defaults_for_paragraph = None;
                return;
            }
            loop {
                if ir >= self.current_para[j].len() {
                    break;
                }
                let ev = self.current_para[j][ir].clone();
                ir += 1;
                if ev.local_name() == Some("rPr") && matches!(ev, XmlEvent::EndElement { .. }) {
                    break;
                }
                if matches!(ev, XmlEvent::EndElement { .. }) {
                    continue;
                }
                if !matches!(ev, XmlEvent::StartElement { .. }) {
                    self.defaults_for_paragraph = None;
                    return;
                }
                let differ = self
                    .defaults_for_paragraph
                    .as_ref()
                    .map(|d| self.is_in_defaults(&ev, d) == 1)
                    .unwrap_or(false);
                if !differ {
                    continue;
                }
                let start_idx = ir - 1;
                let mut la = Vec::new();
                while ir < self.current_para[j].len() {
                    if let XmlEvent::Attribute { name, value } = &self.current_para[j][ir] {
                        la.push(super::stax::Attribute {
                            name: name.clone(),
                            value: value.clone(),
                        });
                        ir += 1;
                    } else {
                        break;
                    }
                }
                // Java: remove events until the iterator is back on `ev`, then
                // replace `ev` with a start that only has the collected attrs.
                if ir < self.current_para[j].len() && ir > start_idx {
                    self.current_para[j].drain(start_idx + 1..=ir);
                } else if ir > start_idx + 1 {
                    self.current_para[j].drain(start_idx + 1..ir);
                }
                let mut collapsed = ev.clone();
                if let XmlEvent::StartElement { attrs, .. } = &mut collapsed {
                    *attrs = la;
                }
                self.current_para[j][start_idx] = collapsed.clone();
                ir = start_idx + 1;
                let still = self
                    .defaults_for_paragraph
                    .as_ref()
                    .map(|d| self.is_in_defaults(&collapsed, d) == 1)
                    .unwrap_or(true);
                if still {
                    self.defaults_for_paragraph = None;
                    return;
                }
            }
            j += 1;
        }
    }

    fn is_in_defaults(&self, st: &XmlEvent, defaults: &[XmlEvent]) -> i32 {
        let Some((name, attrs, _)) = st.as_start() else {
            return 0;
        };
        for dev in defaults {
            let Some((dn, dattrs, _)) = dev.as_start() else {
                continue;
            };
            if dn.local == name.local && dn.uri == name.uri {
                let mut a: Vec<_> = attrs
                    .iter()
                    .map(|x| (x.name.local.as_str(), x.value.as_str()))
                    .collect();
                let mut b: Vec<_> = dattrs
                    .iter()
                    .map(|x| (x.name.local.as_str(), x.value.as_str()))
                    .collect();
                a.sort();
                b.sort();
                return if a == b { 2 } else { 1 };
            }
        }
        0
    }

    /// Java `findPrefix(ListIterator)` — `iter` is the cursor (`nextIndex`).
    fn find_prefix(&self, run: &[XmlEvent], iter: &mut usize) -> char {
        // wr.next(); wr.next(); // pass w:r
        if *iter >= run.len() {
            return 'e';
        }
        *iter += 1;
        if *iter >= run.len() {
            return 'e';
        }
        let mut next_i = *iter;
        *iter += 1;
        let ev = &run[next_i];
        if ev.local_name() == Some("t") && matches!(ev, XmlEvent::StartElement { .. }) {
            return '\0';
        }
        if ev.local_name() == Some("rPr") && matches!(ev, XmlEvent::StartElement { .. }) {
            let mut attrs = Vec::new();
            while *iter < run.len() {
                next_i = *iter;
                *iter += 1;
                let next = &run[next_i];
                if next.local_name() == Some("rPr") && matches!(next, XmlEvent::EndElement { .. }) {
                    break;
                }
                if !matches!(next, XmlEvent::StartElement { .. }) {
                    continue;
                }
                let skip_default = self
                    .defaults_for_paragraph
                    .as_ref()
                    .map(|d| self.is_in_defaults(next, d) > 1)
                    .unwrap_or(false);
                if skip_default {
                    continue;
                }
                if let XmlEvent::StartElement { name, .. } = next {
                    if name.local != "lang" {
                        attrs.push(name.local.clone());
                    }
                }
            }
            let mut last = run.get(next_i);
            while *iter < run.len() {
                next_i = *iter;
                *iter += 1;
                let next = &run[next_i];
                last = Some(next);
                if let XmlEvent::StartElement { name, .. } = next {
                    if self.is_text_element(name) {
                        break;
                    }
                    if name.local.starts_with("footnoteRef") {
                        return 'n';
                    }
                    match name.local.as_str() {
                        "tab" | "br" => return 'd',
                        "fldChar" | "instrText" => return 'e',
                        _ => {}
                    }
                }
            }
            if let Some(next) = last {
                if next.local_name() == Some("r") && matches!(next, XmlEvent::EndElement { .. }) {
                    return 'e';
                }
            }
            if attrs.is_empty() {
                return '\0';
            }
            if attrs.len() > 1 {
                return 'p';
            }
            match attrs[0].as_str() {
                "rStyle" => return 's',
                "rFonts" | "sz" => return 'f',
                "b" | "bCs" => return 'b',
                "i" | "iCs" => return 'i',
                "u" | "uCs" => return 'u',
                "caps" | "smallCaps" => return 'C',
                "color" => return 'c',
                "strike" | "dStrike" => return 'l',
                "vertAlign" => return 'v',
                "lang" => return '\0',
                _ => {}
            }
        }
        while *iter < run.len() {
            next_i = *iter;
            *iter += 1;
            let next = &run[next_i];
            if let XmlEvent::StartElement { name, .. } = next {
                if self.is_text_element(name) {
                    break;
                }
                if name.local.starts_with("footnoteRef") {
                    return 'n';
                }
                match name.local.as_str() {
                    "tab" | "br" => return 'd',
                    "fldChar" | "instrText" => return 'e',
                    _ => {}
                }
            } else if matches!(next, XmlEvent::Characters { .. } | XmlEvent::CData { .. }) {
                *iter -= 1; // wr.previous()
                return 'o';
            }
        }
        'e'
    }

    fn is_text_element(&self, name: &QName) -> bool {
        if name.local != "t" {
            return false;
        }
        match &self.main_para {
            Some(main) => name.uri == main.uri,
            None => true,
        }
    }

    fn browse_run_contents(&mut self, run: &[XmlEvent], iter: &mut usize, res: &mut String) {
        while *iter < run.len() {
            let next_i = *iter;
            *iter += 1;
            match &run[next_i] {
                XmlEvent::EndElement { name } if name.local == "r" => {
                    *iter -= 1; // previous(), used if prefix != 0
                    break;
                }
                XmlEvent::StartElement { name, .. } => {
                    if name.local == "t" {
                        continue;
                    }
                    let prefix_int = match name.local.as_str() {
                        "footnoteRef" => 'n',
                        "tab" | "br" => 'd',
                        "drawing" => 'g',
                        _ => 'e',
                    };
                    let idx = next_i;
                    let tag = name.local.clone();
                    while *iter < run.len() {
                        let ev = &run[*iter];
                        *iter += 1;
                        if ev.local_name() == Some(tag.as_str())
                            && matches!(ev, XmlEvent::EndElement { .. })
                        {
                            break;
                        }
                    }
                    let tc = *self.tags_count.get(&prefix_int).unwrap_or(&0);
                    let mut nlist = run[idx..*iter].to_vec();
                    if let Some(main) = &self.main_para {
                        nlist.insert(
                            0,
                            XmlEvent::StartElement {
                                name: QName::new(&main.prefix, "r", &main.uri),
                                attrs: vec![],
                                namespaces: vec![],
                            },
                        );
                        nlist.push(XmlEvent::EndElement {
                            name: QName::new(&main.prefix, "r", &main.uri),
                        });
                    }
                    res.push_str(&format!("<{prefix_int}{tc}/>"));
                    // Java browseRunContents does not increment tagsCount.
                    self.tags_map.insert(format!("{prefix_int}{tc}"), nlist);
                }
                XmlEvent::Characters { data } | XmlEvent::CData { data } => res.push_str(data),
                _ => {}
            }
        }
    }

    fn restore_openxml(&self, tra: &str) -> Vec<XmlEvent> {
        let mut res = Vec::new();
        let mut rest = tra;
        let re = omegat_tag();
        while !rest.is_empty() {
            if let Some(m) = re.find(rest) {
                if m.start() > 0 {
                    self.add_simple_run(&mut res, &rest[..m.start()]);
                }
                let cap = re.captures(m.as_str()).unwrap();
                let key = format!("{}{}", &cap[1], &cap[2]);
                if let Some(saved) = self.tags_map.get(&key) {
                    res.extend(saved.iter().cloned());
                }
                let alone = m.as_str().ends_with("/>");
                rest = &rest[m.end()..];
                if !alone {
                    if let Some(m2) = re.find(rest) {
                        self.add_characters(&mut res, &rest[..m2.start()]);
                        let cap2 = re.captures(m2.as_str()).unwrap();
                        let key2 = format!("{}{}", &cap2[1], &cap2[2]);
                        if let Some(saved) = self.tags_map.get(&key2) {
                            res.extend(saved.iter().cloned());
                        }
                        rest = &rest[m2.end()..];
                    } else {
                        self.add_characters(&mut res, rest);
                        return res;
                    }
                }
            } else {
                self.add_simple_run(&mut res, rest);
                return res;
            }
        }
        res
    }

    fn add_simple_run(&self, res: &mut Vec<XmlEvent>, text: &str) {
        let (prefix, uri) = self
            .main_para
            .as_ref()
            .map(|m| (m.prefix.clone(), m.uri.clone()))
            .unwrap_or_default();
        res.push(XmlEvent::StartElement {
            name: QName::new(&prefix, "r", &uri),
            attrs: vec![],
            namespaces: vec![],
        });
        if let Some(defaults) = &self.defaults_for_paragraph {
            // Java: these are not really defaults; repeat rPr when generating target.
            let mut in_rpr = false;
            for ev in defaults {
                if ev.local_name() == Some("rPr") && matches!(ev, XmlEvent::StartElement { .. }) {
                    in_rpr = true;
                }
                if in_rpr {
                    res.push(ev.clone());
                }
                if ev.local_name() == Some("rPr") && matches!(ev, XmlEvent::EndElement { .. }) {
                    break;
                }
            }
        }
        res.push(XmlEvent::StartElement {
            name: QName::new(&prefix, "t", &uri),
            attrs: vec![],
            namespaces: vec![],
        });
        self.add_characters(res, text);
        res.push(XmlEvent::EndElement {
            name: QName::new(&prefix, "t", &uri),
        });
        res.push(XmlEvent::EndElement {
            name: QName::new(&prefix, "r", &uri),
        });
    }

    fn add_characters(&self, res: &mut Vec<XmlEvent>, text: &str) {
        if text.trim() != text {
            if let Some(XmlEvent::StartElement { attrs, .. }) = res.last_mut() {
                if !attrs.iter().any(|a| a.name.local == "space") {
                    attrs.push(super::stax::Attribute {
                        name: QName::new("xml", "space", "http://www.w3.org/XML/1998/namespace"),
                        value: "preserve".into(),
                    });
                }
            }
        }
        res.push(XmlEvent::Characters {
            data: text.to_string(),
        });
    }

    /// Java `processStartElement` for `lang` / `themeFontLang`: emit a start
    /// with **no** attributes, then one `ATTRIBUTE` event per rewritten value.
    fn rewrite_lang_events(&self, ev: &XmlEvent) -> Vec<XmlEvent> {
        let XmlEvent::StartElement {
            name,
            attrs,
            namespaces,
        } = ev
        else {
            return vec![ev.clone()];
        };
        if name.local != "lang" && name.local != "themeFontLang" {
            return vec![ev.clone()];
        }
        if self.source_lang.is_empty() || self.target_lang.is_empty() {
            return vec![ev.clone()];
        }
        let src = self.source_lang.clone();
        let tgt = self.target_lang.clone();
        let mut out = vec![XmlEvent::StartElement {
            name: name.clone(),
            attrs: vec![],
            namespaces: namespaces.clone(),
        }];
        for a in attrs {
            let aval = a.value.clone();
            let mut pval = src.clone();
            let value = if aval.eq_ignore_ascii_case(&pval) {
                tgt.clone()
            } else {
                let aval2 = if aval.len() > 2 && aval.as_bytes().get(2) == Some(&b'-') {
                    aval[..2].to_string()
                } else {
                    aval.clone()
                };
                if pval.len() > 2 && pval.as_bytes().get(2) == Some(&b'-') {
                    pval = pval[..2].to_string();
                }
                if aval2.eq_ignore_ascii_case(&pval) {
                    tgt.clone()
                } else {
                    a.value.clone()
                }
            };
            out.push(XmlEvent::Attribute {
                name: a.name.clone(),
                value,
            });
        }
        out
    }
}

fn compact_built_tags(
    res: &mut String,
    tags_map: &mut BTreeMap<String, Vec<XmlEvent>>,
    tags_count: &mut BTreeMap<char, i32>,
) {
    static START: OnceLock<Regex> = OnceLock::new();
    static END: OnceLock<Regex> = OnceLock::new();
    let start = START.get_or_init(|| {
        Regex::new(r"((?:<[a-zA-Z]+[0-9]+/>)*)<([a-zA-Z]+[0-9]+)>((?:<[a-zA-Z]+[0-9]+/>)*)")
            .unwrap()
    });
    let end = END.get_or_init(|| {
        Regex::new(r"((?:<[a-zA-Z]+[0-9]+/>)*)<(/[a-zA-Z]+[0-9]+)>((?:<[a-zA-Z]+[0-9]+/>)*)")
            .unwrap()
    });
    compact_one(res, start, tags_map);
    compact_one(res, end, tags_map);
    let keys: Vec<(char, i32)> = tags_count.iter().map(|(k, v)| (*k, *v)).collect();
    for (key, count) in keys {
        for i in (0..count.saturating_sub(1)).rev() {
            if !res.contains(&format!("<{key}{i}")) {
                for j in (i + 1)..count {
                    if let Some(v) = tags_map.remove(&format!("{key}{j}")) {
                        tags_map.insert(format!("{key}{}", j - 1), v);
                    }
                    if let Some(v) = tags_map.remove(&format!("/{key}{j}")) {
                        tags_map.insert(format!("/{key}{}", j - 1), v);
                    }
                    let from = format!("{key}{j}");
                    let to = format!("{key}{}", j - 1);
                    *res = res.replace(&format!("<{from}>"), &format!("<{to}>"));
                    *res = res.replace(&format!("</{from}>"), &format!("</{to}>"));
                    *res = res.replace(&format!("<{from}/>"), &format!("<{to}/>"));
                }
            }
        }
    }
}

fn compact_one(res: &mut String, pattern: &Regex, tags_map: &mut BTreeMap<String, Vec<XmlEvent>>) {
    let re_tag = omegat_tag();
    let mut search_from = 0;
    loop {
        let Some(caps) = pattern.captures_at(res, search_from) else {
            break;
        };
        if !caps.get(0).unwrap().as_str().contains("/>") {
            search_from = caps.get(0).unwrap().end();
            continue;
        }
        let key = caps[2].to_string();
        let mut global = tags_map.get(&key).cloned().unwrap_or_default();
        for m in re_tag.find_iter(&caps[3]) {
            if let Some(c) = re_tag.captures(m.as_str()) {
                let k = format!("{}{}", &c[1], &c[2]);
                if let Some(v) = tags_map.get(&k) {
                    global.extend(v.iter().cloned());
                }
            }
        }
        let mut to_add = Vec::new();
        for m in re_tag.find_iter(&caps[1]) {
            if let Some(c) = re_tag.captures(m.as_str()) {
                let k = format!("{}{}", &c[1], &c[2]);
                if let Some(v) = tags_map.get(&k) {
                    to_add.extend(v.iter().cloned());
                }
            }
        }
        to_add.append(&mut global);
        tags_map.insert(key.clone(), to_add);
        let start = caps.get(0).unwrap().start();
        let end = caps.get(0).unwrap().end();
        res.replace_range(start..end, &format!("<{key}>"));
        search_from = 0;
    }
}

impl StaxFilter for OpenXmlProc {
    fn check_cursor(&mut self, _ev: &XmlEvent, _writing: bool) -> bool {
        true
    }

    fn process_start(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool {
        let Some((name, _, _)) = ev.as_start() else {
            return true;
        };
        if self.main_para.is_none() {
            self.main_para = Some(name.clone());
        }
        if let Some(main) = &self.main_para {
            if main.uri.contains("presentation") {
                // drawingml `a` prefix for p
                if let Some(a_uri) = ev.as_start().and_then(|(_, _, ns)| {
                    ns.iter().find(|(p, _)| p == "a").map(|(_, u)| u.clone())
                }) {
                    self.main_para = Some(QName::new("a", &main.local, a_uri));
                }
            }
        }
        if self.same_ns(name) {
            if name.local == "p" || name.local == "si" || name.local == "comment" {
                self.start_para(ev.clone());
                return false;
            }
            if name.local == "r" {
                self.start_run(ev.clone());
                return false;
            }
            if self.writing && (name.local == "lang" || name.local == "themeFontLang") {
                let rewritten = self.rewrite_lang_events(ev);
                if self.current_buf.is_some() {
                    for ev in rewritten {
                        self.push_ev(ev);
                    }
                } else if let Some(w) = writer {
                    for ev in rewritten {
                        from_event_to_writer(&ev, w);
                    }
                }
                return false;
            }
            if self.remove_comments
                && matches!(
                    name.local.as_str(),
                    "commentRangeStart" | "commentRangeEnd" | "commentReference"
                )
            {
                return false;
            }
            if name.local == "ins" {
                return false;
            }
            if name.local == "del" {
                self.current_para.push(Vec::new());
                self.current_buf = Some(self.current_para.len() - 1);
                return false;
            }
        }
        if self.current_buf.is_some() {
            self.push_ev(ev.clone());
            return false;
        }
        true
    }

    fn process_end(&mut self, ev: &XmlEvent, writer: Option<&mut StaxWriter>) -> bool {
        let XmlEvent::EndElement { name } = ev else {
            return true;
        };
        if self.same_ns(name) {
            if name.local == "p" || name.local == "si" || name.local == "comment" {
                self.flush_translation(writer);
                self.current_buf = None;
                return true;
            }
            if name.local == "r" {
                if self.current_buf.is_some() {
                    self.push_ev(ev.clone());
                    self.current_para.push(Vec::new());
                    self.current_buf = Some(self.current_para.len() - 1);
                }
                return false;
            }
            if self.remove_comments
                && matches!(
                    name.local.as_str(),
                    "commentRangeStart" | "commentRangeEnd" | "commentReference"
                )
            {
                return false;
            }
            if name.local == "ins" {
                return false;
            }
            if name.local == "del" {
                self.current_buf = Some(self.current_para.len().saturating_sub(1));
                return false;
            }
        }
        if self.current_buf.is_some() {
            self.push_ev(ev.clone());
            return false;
        }
        true
    }

    fn process_characters(&mut self, ev: &XmlEvent, _writer: Option<&mut StaxWriter>) -> bool {
        if self.current_buf.is_some() {
            self.push_ev(ev.clone());
            return false;
        }
        true
    }

    fn take_segments(&mut self) -> Vec<ExtractedSegment> {
        std::mem::take(&mut self.segments)
    }
}

pub fn parse_openxml_part(
    raw: &str,
    ctx: &FilterContext,
    with_comments: bool,
) -> Result<Vec<ExtractedSegment>> {
    let mut proc = OpenXmlProc::new(ctx, with_comments, false);
    let (segments, _) = process_xml_string_ex(raw, &mut proc, false, XmlDeclStyle::AbstractXml)?;
    Ok(segments)
}

pub fn write_openxml_part(
    raw: &str,
    ctx: &FilterContext,
    with_comments: bool,
    translations: &std::collections::HashMap<String, String>,
) -> Result<String> {
    let mut proc = OpenXmlProc::new(ctx, with_comments, true);
    proc.set_translations(translations);
    let (_, text) = process_xml_string_ex(raw, &mut proc, true, XmlDeclStyle::AbstractXml)?;
    Ok(text)
}
