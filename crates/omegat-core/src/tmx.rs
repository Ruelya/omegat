use crate::error::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TmxEntry {
    pub source: String,
    pub translation: String,
    pub creator: Option<String>,
    pub created: Option<String>,
    pub changer: Option<String>,
    pub changed: Option<String>,
    pub note: Option<String>,
    pub default_translation: bool,
    pub file: Option<String>,
    pub id: Option<String>,
    #[serde(default)]
    pub penalty: i32,
    #[serde(default)]
    pub props: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectTmx {
    pub entries: Vec<TmxEntry>,
    by_source: HashMap<String, usize>,
}

impl ProjectTmx {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn load(path: &Path, source_lang: &str, target_lang: &str) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let raw = read_tmx_text(path)?;
        Ok(parse_tmx(&raw, source_lang, target_lang))
    }

    pub fn insert(&mut self, entry: TmxEntry) {
        if let Some(&idx) = self.by_source.get(&entry.source) {
            self.entries[idx] = entry;
        } else {
            self.by_source
                .insert(entry.source.clone(), self.entries.len());
            self.entries.push(entry);
        }
    }

    pub fn get(&self, source: &str) -> Option<&TmxEntry> {
        self.by_source.get(source).map(|&i| &self.entries[i])
    }

    pub fn get_default_translation(&self, source: &str) -> Option<&TmxEntry> {
        self.entries
            .iter()
            .find(|e| e.source == source && e.default_translation)
            .or_else(|| {
                self.get(source)
                    .filter(|e| e.default_translation || e.id.is_none())
            })
    }

    pub fn get_multiple_translation(&self, id: &str, source: &str) -> Option<&TmxEntry> {
        self.entries.iter().find(|e| {
            !e.default_translation && e.id.as_deref() == Some(id) && e.source == source
        })
    }

    pub fn set_default_translation(&mut self, source: &str, translation: &str) {
        let entry = TmxEntry {
            source: source.into(),
            translation: translation.into(),
            default_translation: true,
            ..Default::default()
        };
        if let Some(idx) = self
            .entries
            .iter()
            .position(|e| e.source == source && e.default_translation)
        {
            self.entries[idx] = entry;
            self.by_source.insert(source.into(), idx);
        } else {
            self.insert(entry);
        }
    }

    pub fn set_multiple_translation(&mut self, id: &str, source: &str, translation: &str) {
        let entry = TmxEntry {
            source: source.into(),
            translation: translation.into(),
            default_translation: false,
            id: Some(id.into()),
            ..Default::default()
        };
        if let Some(idx) = self
            .entries
            .iter()
            .position(|e| !e.default_translation && e.id.as_deref() == Some(id) && e.source == source)
        {
            self.entries[idx] = entry;
        } else {
            self.entries.push(entry);
        }
    }

    pub fn write(&self, path: &Path, source_lang: &str, target_lang: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.exists() {
            let bak = path.with_extension("tmx.bak");
            let _ = std::fs::copy(path, bak);
        }
        std::fs::write(path, self.to_xml(source_lang, target_lang))?;
        Ok(())
    }

    pub fn to_xml(&self, source_lang: &str, target_lang: &str) -> String {
        self.to_xml_level(source_lang, target_lang, "omegat")
    }

    pub fn to_xml_level(&self, source_lang: &str, target_lang: &str, level: &str) -> String {
        self.to_xml_level_ex(source_lang, target_lang, level, true)
    }

    /// Java `TMXWriter2` levels: `omegat` (internal + props), `level1` (tags stripped),
    /// `level2` (bpt/ept/ph). Target tuv carries changeid/changedate/creationid/creationdate.
    pub fn to_xml_level_ex(
        &self,
        source_lang: &str,
        target_lang: &str,
        level: &str,
        sentence_seg: bool,
    ) -> String {
        let mut body = String::new();
        for e in &self.entries {
            if e.translation.is_empty() {
                continue;
            }
            let src = match level {
                "level1" => strip_tags(&e.source),
                _ => e.source.clone(),
            };
            let tgt = match level {
                "level1" => strip_tags(&e.translation),
                _ => e.translation.clone(),
            };
            body.push_str("    <tu");
            if level == "level2" {
                if let Some(id) = e.id.as_deref().filter(|s| !s.is_empty()) {
                    body.push_str(&format!(" tuid=\"{}\"", xml_escape(id)));
                }
            }
            body.push_str(">\n");
            if level == "omegat" {
                if let Some(file) = e.file.as_deref().filter(|s| !s.is_empty()) {
                    body.push_str(&format!(
                        "      <prop type=\"file\">{}</prop>\n",
                        xml_escape(file)
                    ));
                }
                if let Some(id) = e.id.as_deref().filter(|s| !s.is_empty()) {
                    body.push_str(&format!(
                        "      <prop type=\"id\">{}</prop>\n",
                        xml_escape(id)
                    ));
                }
                for (k, v) in &e.props {
                    if k == "file" || k == "id" {
                        continue;
                    }
                    body.push_str(&format!(
                        "      <prop type=\"{}\">{}</prop>\n",
                        xml_escape(k),
                        xml_escape(v)
                    ));
                }
                if let Some(note) = &e.note {
                    if !note.is_empty() && !note.starts_with("penalty:") {
                        body.push_str(&format!("      <note>{}</note>\n", xml_escape(note)));
                    }
                }
            }
            let src_seg = if level == "level2" {
                write_level_two(&src)
            } else {
                xml_escape(&src)
            };
            let tgt_seg = if level == "level2" {
                write_level_two(&tgt)
            } else {
                xml_escape(&tgt)
            };
            let lang_attr = if level == "level1" { "lang" } else { "xml:lang" };
            body.push_str(&format!(
                "      <tuv {lang_attr}=\"{}\">\n        <seg>{}</seg>\n      </tuv>\n",
                xml_escape(source_lang),
                src_seg
            ));
            body.push_str("      <tuv");
            body.push_str(&format!(
                " {lang_attr}=\"{}\"",
                xml_escape(target_lang)
            ));
            if let Some(c) = e.changer.as_deref().filter(|s| !s.is_empty()) {
                body.push_str(&format!(" changeid=\"{}\"", xml_escape(c)));
            }
            if let Some(d) = e.changed.as_deref().filter(|s| !s.is_empty()) {
                body.push_str(&format!(" changedate=\"{}\"", xml_escape(&to_tmx_date(d))));
            }
            if let Some(c) = e.creator.as_deref().filter(|s| !s.is_empty()) {
                body.push_str(&format!(" creationid=\"{}\"", xml_escape(c)));
            }
            if let Some(d) = e.created.as_deref().filter(|s| !s.is_empty()) {
                body.push_str(&format!(" creationdate=\"{}\"", xml_escape(&to_tmx_date(d))));
            }
            body.push_str(&format!(
                ">\n        <seg>{}</seg>\n      </tuv>\n    </tu>\n",
                tgt_seg
            ));
        }
        let dtd = if level == "level1" {
            "tmx11.dtd"
        } else {
            "tmx14.dtd"
        };
        let ver = if level == "level1" { "1.1" } else { "1.4" };
        let segtype = if sentence_seg { "sentence" } else { "paragraph" };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE tmx SYSTEM "{dtd}">
<tmx version="{ver}">
  <header creationtool="OmegaT" creationtoolversion="{app}" segtype="{segtype}" o-tmf="OmegaT TMX" adminlang="EN-US" srclang="{src}" datatype="plaintext"/>
  <body>
{body}  </body>
</tmx>
"#,
            app = omegat_ipc::APP_VERSION,
            src = xml_escape(source_lang),
            body = body
        )
    }
}

/// Java `TMXWriter2.writeLevelTwo`: `<f0>` → bpt/ept, `<x0/>` → ph.
fn write_level_two(segment: &str) -> String {
    let re = Regex::new(r"<(/?)([^\s/<>\d]+)(\d+)(/?)>").unwrap();
    let mut out = String::new();
    let mut last = 0;
    for cap in re.captures_iter(segment) {
        let m = cap.get(0).unwrap();
        out.push_str(&xml_escape(&segment[last..m.start()]));
        last = m.end();
        let is_end = !cap[1].is_empty();
        let is_single = !cap[4].is_empty();
        let name = &cap[2];
        let num = &cap[3];
        let raw = xml_escape(m.as_str());
        if is_single {
            out.push_str(&format!("<ph x=\"{num}\">{raw}</ph>"));
        } else if is_end {
            let start = format!("<{name}{num}>");
            if segment.contains(&start) {
                out.push_str(&format!("<ept i=\"{num}\">{raw}</ept>"));
            } else {
                out.push_str(&format!("<it pos=\"end\" x=\"{num}\">{raw}</it>"));
            }
        } else {
            let end = format!("</{name}{num}>");
            if segment.contains(&end) {
                out.push_str(&format!("<bpt i=\"{num}\" x=\"{num}\">{raw}</bpt>"));
            } else {
                out.push_str(&format!("<it pos=\"begin\" x=\"{num}\">{raw}</it>"));
            }
        }
    }
    out.push_str(&xml_escape(&segment[last..]));
    out
}

fn unwrap_level2(seg: &str) -> String {
    let mut inner = seg.to_string();
    for tag in ["ph", "bpt", "ept", "it"] {
        let re = Regex::new(&format!(r"<{tag}\b[^>]*>(.*?)</{tag}>")).unwrap();
        inner = re.replace_all(&inner, "$1").into_owned();
    }
    html_escape::decode_html_entities(&inner).into_owned()
}

/// Java `TMXDateParser.parse`: length must be exactly 16 (`YYYYMMDDThhmmssZ`).
pub fn parse_tmx_date(s: Option<&str>) -> std::result::Result<i64, String> {
    let Some(s) = s else {
        return Err("date 'null' is null or not equal to YYYYMMDDThhmmssZ".into());
    };
    if s.len() != 16 || !s.as_bytes().get(8).is_some_and(|b| *b == b'T') || !s.ends_with('Z') {
        return Err(format!("date '{s}' is null or not equal to YYYYMMDDThhmmssZ"));
    }
    let y: i64 = s[0..4].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let mo: i64 = s[4..6].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let d: i64 = s[6..8].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let hh: i64 = s[9..11].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let mm: i64 = s[11..13].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    let ss: i64 = s[13..15].parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
    Ok(ymd_hms_to_millis(y, mo, d, hh, mm, ss))
}

/// Java `TMXDateParser.getTMXDate`.
pub fn format_tmx_date(millis: i64) -> String {
    let secs = millis.div_euclid(1000);
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

fn ymd_hms_to_millis(y: i64, mo: i64, d: i64, hh: i64, mm: i64, ss: i64) -> i64 {
    let z = days_from_civil(y, mo, d);
    ((z * 86400) + hh * 3600 + mm * 60 + ss) * 1000
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as u64 + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe as i64) - 719468
}

/// Java `MergeTest#testTimeTruncate`: TMX dates drop milliseconds (not round).
pub fn truncate_change_date_ms(ms: i64) -> i64 {
    ms - ms.rem_euclid(1000)
}

/// Java `TMXEntry.equals` / `equalsTranslation` (truncated change date).
pub fn tmx_entry_equals(a: &TmxEntry, b: &TmxEntry, compare_translation_only: bool) -> bool {
    if a.translation != b.translation {
        return false;
    }
    if compare_translation_only {
        return a.note == b.note && a.penalty == b.penalty;
    }
    let da = a.changed.as_deref().and_then(|s| parse_tmx_date(Some(s)).ok()).unwrap_or(0);
    let db = b.changed.as_deref().and_then(|s| parse_tmx_date(Some(s)).ok()).unwrap_or(0);
    truncate_change_date_ms(da) == truncate_change_date_ms(db)
}

/// Java `TmxEscapingWriterFactory.EscapeWriter` for TEXT (not attributes).
/// Woodstox walks **UTF-16 code units**; supplementary-plane emoji therefore
/// stay as a surrogate pair (neither unit is `>= 0xFFFE`).
pub fn escape_tmx_text(s: &str) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < units.len() {
        let c = units[i] as u32;
        if c < 256 && quotable_text_char(c) {
            match char::from_u32(c) {
                Some('<') => out.push_str("&lt;"),
                Some('>') => out.push_str("&gt;"),
                Some('&') => out.push_str("&amp;"),
                _ => out.push_str(&format!("&#x{c:02x};")),
            }
            i += 1;
            continue;
        }
        if c >= 0xFFFE {
            out.push_str(&format!("&#x{c:04x};"));
            i += 1;
            continue;
        }
        if (0xD800..=0xDBFF).contains(&c) && i + 1 < units.len() {
            let low = units[i + 1] as u32;
            if (0xDC00..=0xDFFF).contains(&low) {
                let cp = 0x10000 + ((c - 0xD800) << 10) + (low - 0xDC00);
                if let Some(ch) = char::from_u32(cp) {
                    out.push(ch);
                }
                i += 2;
                continue;
            }
        }
        if let Some(ch) = char::from_u32(c) {
            out.push(ch);
        }
        i += 1;
    }
    out
}

fn quotable_text_char(c: u32) -> bool {
    if c == b'\t' as u32 || c == b'\n' as u32 {
        return false;
    }
    if cfg!(windows) && c == b'\r' as u32 {
        return false;
    }
    (c < 32) || (127..160).contains(&c) || matches!(c, 0x3C | 0x3E | 0x26)
}

fn to_tmx_date(raw: &str) -> String {
    if raw.len() == 16 && raw.chars().nth(8) == Some('T') && raw.ends_with('Z') {
        return raw.to_string();
    }
    if raw.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(secs) = raw.parse::<i64>() {
            let days = secs / 86400;
            let rem = secs % 86400;
            let (y, m, d) = civil_from_days(days);
            let hh = rem / 3600;
            let mm = (rem % 3600) / 60;
            let ss = rem % 60;
            return format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z");
        }
    }
    raw.to_string()
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn decode_seg(seg_raw: String) -> String {
    if seg_raw.contains("<ph") || seg_raw.contains("<bpt") || seg_raw.contains("<ept") || seg_raw.contains("<it")
    {
        unwrap_level2(&seg_raw)
    } else {
        seg_raw
    }
}

/// `<tu` must not match `<tuv`.
fn find_tu_start(raw: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(rel) = raw[from..].find("<tu") {
        let at = from + rel;
        let after = raw.get(at + 3..).and_then(|s| s.chars().next());
        if after == Some('>') || after == Some(' ') || after == Some('\n') || after == Some('\r') || after == Some('\t')
        {
            return Some(at);
        }
        from = at + 3;
    }
    None
}

#[derive(Clone)]
struct Tuv {
    lang: String,
    text: String,
    changeid: Option<String>,
    changedate: Option<String>,
    creationid: Option<String>,
    creationdate: Option<String>,
}

fn collect_tuvs(tu: &str) -> Vec<Tuv> {
    let mut tuvs = Vec::new();
    let mut search = tu;
    while let Some(p) = search.find("<tuv") {
        let tuv = &search[p..];
        let end = tuv.find("</tuv>").unwrap_or(tuv.len());
        let block = &tuv[..end];
        let lang = attr(block, "xml:lang")
            .or_else(|| attr(block, "lang"))
            .unwrap_or_default();
        let seg = decode_seg(extract_tag(block, "seg").unwrap_or_default());
        tuvs.push(Tuv {
            lang,
            text: seg,
            changeid: attr(block, "changeid"),
            changedate: attr(block, "changedate"),
            creationid: attr(block, "creationid"),
            creationdate: attr(block, "creationdate"),
        });
        search = &search[p + 4..];
    }
    tuvs
}

/// Java `TMXReader2.readTMX` accepts `.tmx`, `.tmx.gz`, and a zip that contains a `.tmx`.
pub fn read_tmx_text(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".gz") {
        use std::io::Read;
        let file = std::fs::File::open(path)?;
        let mut decoder = flate2::read::GzDecoder::new(file);
        let mut raw = String::new();
        decoder.read_to_string(&mut raw)?;
        return Ok(raw);
    }
    if name.ends_with(".zip") {
        use std::io::Read;
        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| {
            crate::error::CoreError::InvalidProject(format!("tmx zip: {e}"))
        })?;
        for i in 0..archive.len() {
            let mut inner = archive.by_index(i).map_err(|e| {
                crate::error::CoreError::InvalidProject(format!("tmx zip entry: {e}"))
            })?;
            if inner.name().to_ascii_lowercase().ends_with(".tmx") {
                let mut raw = String::new();
                inner.read_to_string(&mut raw)?;
                return Ok(raw);
            }
        }
        return Err(crate::error::CoreError::InvalidProject(
            "zip contains no .tmx".into(),
        ));
    }
    Ok(std::fs::read_to_string(path)?)
}

/// Project TMX: one entry per TU (defaults + alternatives kept; no collapse).
pub fn parse_tmx_all(raw: &str, source_lang: &str, target_lang: &str) -> Vec<TmxEntry> {
    let src_l = source_lang.to_ascii_lowercase();
    let tgt_l = target_lang.to_ascii_lowercase();
    let mut entries = Vec::new();
    let mut rest = raw;
    while let Some(tu_start) = find_tu_start(rest) {
        let slice = &rest[tu_start..];
        let tu_end = slice.find("</tu>").unwrap_or(slice.len());
        let tu = &slice[..tu_end];
        let note = extract_tag(tu, "note");
        let tuid = attr(tu, "tuid");
        let file = extract_prop(tu, "file");
        let id = extract_prop(tu, "id").or(tuid);
        let tuvs = collect_tuvs(tu);
        let source = get_tuv_by_lang(&tuvs, &src_l).map(|t| t.text.clone());
        let target = get_tuv_by_lang(&tuvs, &tgt_l).or_else(|| {
            tuvs.iter().find(|t| {
                let ll = t.lang.to_ascii_lowercase();
                source.is_some() && !lang_matches(&ll, &src_l)
            })
        });
        if let (Some(s), Some(t)) = (source, target) {
            entries.push(TmxEntry {
                source: s,
                translation: t.text.clone(),
                note,
                default_translation: file.is_none(),
                file,
                id,
                changer: t.changeid.clone(),
                changed: t.changedate.clone(),
                creator: t.creationid.clone(),
                created: t.creationdate.clone(),
                ..Default::default()
            });
        }
        rest = &rest[tu_start + 3..];
    }
    entries
}

/// Java `ExternalTMFactory.TMXLoader`: every non-source TUV, with `foreignMatch`.
pub fn parse_external_tmx(
    raw: &str,
    source_lang: &str,
    target_lang: &str,
    keep_foreign: bool,
) -> Vec<TmxEntry> {
    let mut entries = Vec::new();
    let mut rest = raw;
    while let Some(tu_start) = find_tu_start(rest) {
        let slice = &rest[tu_start..];
        let tu_end = slice.find("</tu>").unwrap_or(slice.len());
        let tu = &slice[..tu_end];
        let note = extract_tag(tu, "note");
        let tuvs = collect_tuvs(tu);
        let Some(src) = tuvs.iter().find(|t| same_language(&t.lang, source_lang)).cloned() else {
            rest = &rest[tu_start + 3..];
            continue;
        };
        for t in &tuvs {
            if lang_equals(&t.lang, &src.lang) || lang_equals(&t.lang, source_lang) {
                continue;
            }
            let is_foreign = !same_language(&t.lang, target_lang);
            if is_foreign && !keep_foreign {
                continue;
            }
            let mut props = vec![
                ("sourceLanguage".into(), src.lang.clone()),
                ("targetLanguage".into(), t.lang.clone()),
            ];
            if is_foreign {
                props.push(("foreignMatch".into(), "true".into()));
            }
            entries.push(TmxEntry {
                source: src.text.clone(),
                translation: t.text.clone(),
                note: note.clone(),
                default_translation: true,
                props,
                changer: t.changeid.clone(),
                changed: t.changedate.clone(),
                creator: t.creationid.clone(),
                created: t.creationdate.clone(),
                ..Default::default()
            });
        }
        rest = &rest[tu_start + 3..];
    }
    entries
}

pub fn parse_tmx(raw: &str, source_lang: &str, target_lang: &str) -> ProjectTmx {
    let mut tmx = ProjectTmx::new();
    for e in parse_tmx_all(raw, source_lang, target_lang) {
        tmx.insert(e);
    }
    tmx
}

/// Java `Language.equals` via locale tag (language + country).
pub fn lang_equals(a: &str, b: &str) -> bool {
    a.replace('_', "-").eq_ignore_ascii_case(&b.replace('_', "-"))
}

/// Java `Language.isSameLanguage`.
pub fn same_language(a: &str, b: &str) -> bool {
    lang_code(a) == lang_code(b)
}

fn lang_code(s: &str) -> String {
    s.split(['-', '_']).next().unwrap_or(s).to_ascii_lowercase()
}

fn extract_prop(tu: &str, ty: &str) -> Option<String> {
    let needle = format!("<prop type=\"{ty}\">");
    let s = tu.find(&needle)? + needle.len();
    let e = tu[s..].find("</prop>")? + s;
    Some(html_escape::decode_html_entities(&tu[s..e]).into_owned())
}

/// Java `TMXReader2.getTuvByLang`: exact tag first, then same language code.
fn get_tuv_by_lang<'a>(tuvs: &'a [Tuv], lang: &str) -> Option<&'a Tuv> {
    let want = lang.replace('_', "-").to_ascii_lowercase();
    tuvs.iter()
        .find(|t| t.lang.replace('_', "-").eq_ignore_ascii_case(&want))
        .or_else(|| {
            let base = want.split('-').next().unwrap_or(&want);
            tuvs.iter().find(|t| {
                t.lang
                    .replace('_', "-")
                    .to_ascii_lowercase()
                    .split('-')
                    .next()
                    == Some(base)
            })
        })
}

fn lang_matches(a: &str, b: &str) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b
        || a.starts_with(b)
        || b.starts_with(a)
        || a.split(['-', '_']).next() == b.split(['-', '_']).next()
}

fn extract_tag(raw: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = raw.find(&open)?;
    let after = &raw[start..];
    let gt = after.find('>')? + start + 1;
    let close = format!("</{tag}>");
    let end = raw[gt..].find(&close)? + gt;
    Some(html_escape::decode_html_entities(&raw[gt..end]).into_owned())
}

fn attr(block: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let s = block.find(&key)? + key.len();
    let e = block[s..].find('"')? + s;
    Some(block[s..e].to_string())
}

pub fn is_valid_xml_char(code: u32) -> bool {
    if code < 0x20 {
        return code == 0x09 || code == 0x0A || code == 0x0D;
    }
    code <= 0xD7FF
        || (0xE000..=0xFFFD).contains(&code)
        || (0x10000..=0x10FFFF).contains(&code)
}

/// Java `StringUtil.removeXMLInvalidChars`.
pub fn remove_xml_invalid_chars(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if is_valid_xml_char(ch as u32) {
            out.push(ch);
        } else {
            out.push(' ');
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    let s = remove_xml_invalid_chars(s);
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Java `TMXWriter2.writeLevelTwo` fragment used by `TMXWriterTest#testLevel2write`.
/// Inner OmegaT tags are left unescaped so the test can `assert_eq` the
/// structural rewrite; the full XML writer still escapes via `xml_escape`.
pub fn write_level_two_fragment(segment: &str) -> String {
    let re = Regex::new(r"<(/?)([^\s/<>\d]+)(\d+)(/?)>").unwrap();
    let mut out = String::new();
    let mut last = 0;
    for cap in re.captures_iter(segment) {
        let m = cap.get(0).unwrap();
        out.push_str(&segment[last..m.start()]);
        last = m.end();
        let is_end = !cap[1].is_empty();
        let is_single = !cap[4].is_empty();
        let name = &cap[2];
        let num = &cap[3];
        let raw = m.as_str();
        if is_single {
            out.push_str(&format!("<ph x=\"{num}\">{raw}</ph>"));
        } else if is_end {
            let start = format!("<{name}{num}>");
            if segment.contains(&start) {
                out.push_str(&format!("<ept i=\"{num}\">{raw}</ept>"));
            } else {
                out.push_str(&format!("<it pos=\"end\" x=\"{num}\">{raw}</it>"));
            }
        } else {
            let end = format!("</{name}{num}>");
            if segment.contains(&end) {
                out.push_str(&format!("<bpt i=\"{num}\" x=\"{num}\">{raw}</bpt>"));
            } else {
                out.push_str(&format!("<it pos=\"begin\" x=\"{num}\">{raw}</it>"));
            }
        }
    }
    out.push_str(&segment[last..]);
    out
}

#[derive(Debug, Clone, Copy)]
pub struct TmxReadOpts {
    pub ext_level2: bool,
    pub use_slash: bool,
    pub created_by_omegat: bool,
}

impl Default for TmxReadOpts {
    fn default() -> Self {
        Self {
            ext_level2: true,
            use_slash: false,
            created_by_omegat: true,
        }
    }
}

/// Read sources with Java `TMXReader2` level-2 / slash options.
pub fn parse_tmx_sources(raw: &str, source_lang: &str, target_lang: &str, opts: TmxReadOpts) -> Vec<String> {
    let tmx = parse_tmx_opts(raw, source_lang, target_lang, opts);
    tmx.entries.into_iter().map(|e| e.source).collect()
}

pub fn parse_tmx_opts(raw: &str, source_lang: &str, target_lang: &str, opts: TmxReadOpts) -> ProjectTmx {
    let mut rewritten = raw.to_string();
    if !opts.created_by_omegat {
        rewritten = rewrite_ext_level2(&rewritten, opts.ext_level2, opts.use_slash);
    }
    parse_tmx(&rewritten, source_lang, target_lang)
}

fn rewrite_ext_level2(raw: &str, ext_level2: bool, use_slash: bool) -> String {
    let seg_re = Regex::new(r"(<seg>)(.*?)(</seg>)").unwrap();
    seg_re
        .replace_all(raw, |caps: &regex::Captures| {
            format!(
                "{}{}{}",
                &caps[1],
                rewrite_seg_level2(&caps[2], ext_level2, use_slash),
                &caps[3]
            )
        })
        .into_owned()
}

fn rewrite_seg_level2(seg: &str, ext_level2: bool, use_slash: bool) -> String {
    let re = Regex::new(r"<(ph|bpt|ept|it)\b([^>]*)>(.*?)</(?:ph|bpt|ept|it)>").unwrap();
    let mut n = 0i32;
    re.replace_all(seg, |caps: &regex::Captures| {
        if !ext_level2 {
            return String::new();
        }
        let kind = &caps[1];
        let attrs = &caps[2];
        match kind {
            "ph" => {
                let t = if use_slash {
                    format!("<a{n}/>")
                } else {
                    format!("<a{n}>")
                };
                n += 1;
                t
            }
            "bpt" => {
                let t = format!("<a{n}>");
                n += 1;
                t
            }
            "ept" => format!("</a{}>", n.saturating_sub(1)),
            "it" => {
                let end = attrs.contains("pos=\"end\"");
                let t = if end {
                    format!("</a{n}>")
                } else {
                    format!("<a{n}>")
                };
                n += 1;
                t
            }
            _ => String::new(),
        }
    })
    .into_owned()
}

fn strip_tags(s: &str) -> String {
    let re = Regex::new(r"<[^>]+>").unwrap();
    re.replace_all(s, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sample_tmx() {
        let raw = r#"<tmx><body>
        <tu><tuv lang="en"><seg>Hello</seg></tuv><tuv lang="fr"><seg>Bonjour</seg></tuv></tu>
        </body></tmx>"#;
        let tmx = parse_tmx(raw, "en", "fr");
        assert_eq!(tmx.get("Hello").unwrap().translation, "Bonjour");
    }

    #[test]
    fn java_fixture_roundtrip_and_levels() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/tmx/project_save.tmx");
        if !path.exists() {
            return;
        }
        let tmx = ProjectTmx::load(&path, "en", "fr").unwrap();
        assert!(!tmx.entries.is_empty());
        let omegat = tmx.to_xml_level("en", "fr", "omegat");
        let l1 = tmx.to_xml_level("en", "fr", "level1");
        let l2 = tmx.to_xml_level("en", "fr", "level2");
        assert!(omegat.contains("<tmx version=\"1.4\">"));
        assert!(l1.contains("<seg>"));
        assert!(l2.contains("<seg>"));
        let back = parse_tmx(&omegat, "en", "fr");
        assert_eq!(
            back.entries.len(),
            tmx.entries.iter().filter(|e| !e.translation.is_empty()).count()
        );
    }

    #[test]
    fn level1_strips_tags() {
        let mut tmx = ProjectTmx::new();
        tmx.insert(TmxEntry {
            source: "Hello <b>x</b>".into(),
            translation: "Bonjour <b>x</b>".into(),
            ..Default::default()
        });
        let xml = tmx.to_xml_level("en", "fr", "level1");
        assert!(!xml.contains("<b>"));
        assert!(xml.contains("Hello x"));
    }

    #[test]
    fn omegat_level_writes_tuv_attrs_and_props() {
        let mut tmx = ProjectTmx::new();
        tmx.insert(TmxEntry {
            source: "Hello <f0>x</f0>".into(),
            translation: "Bonjour <f0>x</f0>".into(),
            changer: Some("alice".into()),
            changed: Some("20200101T000000Z".into()),
            creator: Some("bob".into()),
            created: Some("20190101T000000Z".into()),
            note: Some("dev note".into()),
            file: Some("a.txt".into()),
            id: Some("id-1".into()),
            ..Default::default()
        });
        let omegat = tmx.to_xml_level("en-US", "fr-FR", "omegat");
        assert!(omegat.contains("changeid=\"alice\""));
        assert!(omegat.contains("creationid=\"bob\""));
        assert!(omegat.contains("changedate=\"20200101T000000Z\""));
        assert!(omegat.contains("<prop type=\"file\">a.txt</prop>"));
        assert!(omegat.contains("<prop type=\"id\">id-1</prop>"));
        assert!(omegat.contains("<note>dev note</note>"));
        let l2 = tmx.to_xml_level("en-US", "fr-FR", "level2");
        assert!(l2.contains("<bpt i=\"0\" x=\"0\">"));
        assert!(l2.contains("<ept i=\"0\">"));
        assert!(l2.contains("tuid=\"id-1\""));
        let back = parse_tmx(&omegat, "en-US", "fr-FR");
        assert_eq!(back.get("Hello <f0>x</f0>").unwrap().changer.as_deref(), Some("alice"));
    }
}
