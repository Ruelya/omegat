use crate::consts::*;
use crate::error::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryMapping {
    pub local: String,
    pub repository: String,
    pub includes: Vec<String>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositoryDef {
    pub repo_type: String,
    pub url: String,
    pub branch: Option<String>,
    pub mappings: Vec<RepositoryMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProperties {
    pub root: PathBuf,
    pub source_dir: PathBuf,
    pub target_dir: PathBuf,
    pub tm_dir: PathBuf,
    pub glossary_dir: PathBuf,
    pub glossary_file: PathBuf,
    pub dictionary_dir: PathBuf,
    pub export_tm_dir: PathBuf,
    pub export_tm_levels: String,
    pub source_lang: String,
    pub target_lang: String,
    pub source_tok: String,
    pub target_tok: String,
    pub sentence_seg: bool,
    pub support_default_translations: bool,
    pub remove_tags: bool,
    pub external_command: String,
    pub source_dir_excludes: Vec<String>,
    pub repositories: Vec<RepositoryDef>,
    pub raw_unknown: String,
}

impl ProjectProperties {
    pub fn create(root: PathBuf, source_lang: String, target_lang: String, sentence_seg: bool) -> Self {
        let source_dir = root.join(DEFAULT_SOURCE);
        let target_dir = root.join(DEFAULT_TARGET);
        let glossary_dir = root.join(DEFAULT_GLOSSARY);
        Self {
            root: root.clone(),
            source_dir,
            target_dir,
            tm_dir: root.join(DEFAULT_TM),
            glossary_dir: glossary_dir.clone(),
            glossary_file: glossary_dir.join(DEFAULT_W_GLOSSARY),
            dictionary_dir: root.join(DEFAULT_DICT),
            export_tm_dir: root.clone(),
            export_tm_levels: "omegat level1 level2".into(),
            source_lang,
            target_lang,
            source_tok: "org.omegat.tokenizer.DefaultTokenizer".into(),
            target_tok: "org.omegat.tokenizer.DefaultTokenizer".into(),
            sentence_seg,
            support_default_translations: true,
            remove_tags: false,
            external_command: String::new(),
            source_dir_excludes: DEFAULT_EXCLUDES.iter().map(|s| (*s).to_string()).collect(),
            repositories: vec![],
            raw_unknown: String::new(),
        }
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for p in [
            &self.source_dir,
            &self.target_dir,
            &self.tm_dir,
            &self.glossary_dir,
            &self.dictionary_dir,
            &self.root.join(DEFAULT_INTERNAL),
            &self.tm_dir.join(AUTO_TM),
            &self.tm_dir.join(ENFORCE_TM),
            &self.tm_dir.join(MT_TM),
            &self.tm_dir.join(TMX2SOURCE),
        ] {
            std::fs::create_dir_all(p)?;
        }
        if !self.glossary_file.exists() {
            std::fs::write(&self.glossary_file, "")?;
        }
        Ok(())
    }

    pub fn save_tmx_path(&self) -> PathBuf {
        self.root.join(DEFAULT_INTERNAL).join(STATUS_TMX)
    }

    pub fn project_file(&self) -> PathBuf {
        self.root.join(FILE_PROJECT)
    }

    pub fn write(&self) -> Result<()> {
        let xml = self.to_xml();
        std::fs::write(self.project_file(), xml)?;
        Ok(())
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(FILE_PROJECT);
        if !path.exists() {
            return Err(CoreError::InvalidProject(format!(
                "missing {}",
                FILE_PROJECT
            )));
        }
        let raw = std::fs::read_to_string(&path)?;
        parse_project_xml(root, &raw)
    }

    pub fn to_xml(&self) -> String {
        let rel = |p: &Path| -> String {
            p.strip_prefix(&self.root)
                .map(|s| s.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| p.to_string_lossy().replace('\\', "/"))
        };
        let mut excludes = String::new();
        for m in &self.source_dir_excludes {
            excludes.push_str(&format!("            <mask>{}</mask>\n", xml_escape(m)));
        }
        let mut repos = String::new();
        if !self.repositories.is_empty() {
            repos.push_str("        <repositories>\n");
            for r in &self.repositories {
                repos.push_str(&format!(
                    "            <repository type=\"{}\" url=\"{}\"{}>\n",
                    xml_escape(&r.repo_type),
                    xml_escape(&r.url),
                    r.branch
                        .as_ref()
                        .map(|b| format!(" branch=\"{}\"", xml_escape(b)))
                        .unwrap_or_default()
                ));
                for m in &r.mappings {
                    if m.includes.is_empty() && m.excludes.is_empty() {
                        repos.push_str(&format!(
                            "                <mapping local=\"{}\" repository=\"{}\"/>\n",
                            xml_escape(&m.local),
                            xml_escape(&m.repository)
                        ));
                    } else {
                        repos.push_str(&format!(
                            "                <mapping local=\"{}\" repository=\"{}\">\n",
                            xml_escape(&m.local),
                            xml_escape(&m.repository)
                        ));
                        for inc in &m.includes {
                            repos.push_str(&format!(
                                "                    <includes>{}</includes>\n",
                                xml_escape(inc)
                            ));
                        }
                        for exc in &m.excludes {
                            repos.push_str(&format!(
                                "                    <excludes>{}</excludes>\n",
                                xml_escape(exc)
                            ));
                        }
                        repos.push_str("                </mapping>\n");
                    }
                }
                repos.push_str("            </repository>\n");
            }
            repos.push_str("        </repositories>\n");
        }
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<omegat>
    <project version="{version}">
        <source_dir>{source}</source_dir>
        <source_dir_excludes>
{excludes}        </source_dir_excludes>
        <target_dir>{target}</target_dir>
        <tm_dir>{tm}</tm_dir>
        <glossary_dir>{glossary}</glossary_dir>
        <glossary_file>{gfile}</glossary_file>
        <dictionary_dir>{dict}</dictionary_dir>
        <export_tm_dir>{export}</export_tm_dir>
        <export_tm_levels>{levels}</export_tm_levels>
        <source_lang>{slang}</source_lang>
        <target_lang>{tlang}</target_lang>
        <source_tok>{stok}</source_tok>
        <target_tok>{ttok}</target_tok>
        <sentence_seg>{seg}</sentence_seg>
        <support_default_translations>{def}</support_default_translations>
        <remove_tags>{rt}</remove_tags>
        <external_command>{cmd}</external_command>
{repos}    </project>
</omegat>
"#,
            version = PROJ_VERSION,
            source = xml_escape(&rel(&self.source_dir)),
            excludes = excludes,
            target = xml_escape(&rel(&self.target_dir)),
            tm = xml_escape(&rel(&self.tm_dir)),
            glossary = xml_escape(&rel(&self.glossary_dir)),
            gfile = xml_escape(&rel(&self.glossary_file)),
            dict = xml_escape(&rel(&self.dictionary_dir)),
            export = xml_escape(&rel(&self.export_tm_dir)),
            levels = xml_escape(&self.export_tm_levels),
            slang = xml_escape(&self.source_lang),
            tlang = xml_escape(&self.target_lang),
            stok = xml_escape(&self.source_tok),
            ttok = xml_escape(&self.target_tok),
            seg = self.sentence_seg,
            def = self.support_default_translations,
            rt = self.remove_tags,
            cmd = xml_escape(&self.external_command),
            repos = repos,
        )
    }

    pub fn to_dto(&self) -> omegat_ipc::ProjectPropsDto {
        omegat_ipc::ProjectPropsDto {
            root: self.root.to_string_lossy().into(),
            source_lang: self.source_lang.clone(),
            target_lang: self.target_lang.clone(),
            sentence_seg: self.sentence_seg,
            source_dir: self.source_dir.to_string_lossy().into(),
            target_dir: self.target_dir.to_string_lossy().into(),
            tm_dir: self.tm_dir.to_string_lossy().into(),
            glossary_dir: self.glossary_dir.to_string_lossy().into(),
            glossary_file: self.glossary_file.to_string_lossy().into(),
            dictionary_dir: self.dictionary_dir.to_string_lossy().into(),
            export_tm_levels: self.export_tm_levels.clone(),
            support_default_translations: self.support_default_translations,
            remove_tags: self.remove_tags,
            has_repositories: !self.repositories.is_empty(),
        }
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn tag_text(raw: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = raw.find(&open)? + open.len();
    let end = raw[start..].find(&close)? + start;
    Some(html_escape::decode_html_entities(&raw[start..end]).into_owned())
}

fn resolve_dir(root: &Path, value: &str, default: &str) -> PathBuf {
    if value.is_empty() || value == "__DEFAULT__" {
        root.join(default)
    } else {
        let p = PathBuf::from(value);
        if p.is_absolute() {
            p
        } else {
            root.join(p)
        }
    }
}

fn parse_project_xml(root: &Path, raw: &str) -> Result<ProjectProperties> {
    let source_lang = tag_text(raw, "source_lang").unwrap_or_else(|| "en".into());
    let target_lang = tag_text(raw, "target_lang").unwrap_or_else(|| "fr".into());
    let sentence_seg = tag_text(raw, "sentence_seg")
        .map(|s| s == "true")
        .unwrap_or(true);
    let mut props = ProjectProperties::create(root.to_path_buf(), source_lang, target_lang, sentence_seg);
    if let Some(v) = tag_text(raw, "source_dir") {
        props.source_dir = resolve_dir(root, &v, DEFAULT_SOURCE);
    }
    if let Some(v) = tag_text(raw, "target_dir") {
        props.target_dir = resolve_dir(root, &v, DEFAULT_TARGET);
    }
    if let Some(v) = tag_text(raw, "tm_dir") {
        props.tm_dir = resolve_dir(root, &v, DEFAULT_TM);
    }
    if let Some(v) = tag_text(raw, "glossary_dir") {
        props.glossary_dir = resolve_dir(root, &v, DEFAULT_GLOSSARY);
    }
    if let Some(v) = tag_text(raw, "glossary_file") {
        props.glossary_file = if v == "__DEFAULT__" {
            props.glossary_dir.join(DEFAULT_W_GLOSSARY)
        } else {
            resolve_dir(root, &v, DEFAULT_W_GLOSSARY)
        };
    }
    if let Some(v) = tag_text(raw, "dictionary_dir") {
        props.dictionary_dir = resolve_dir(root, &v, DEFAULT_DICT);
    }
    if let Some(v) = tag_text(raw, "export_tm_dir") {
        props.export_tm_dir = resolve_dir(root, &v, "");
    }
    if let Some(v) = tag_text(raw, "export_tm_levels") {
        props.export_tm_levels = v;
    }
    if let Some(v) = tag_text(raw, "source_tok") {
        props.source_tok = v;
    }
    if let Some(v) = tag_text(raw, "target_tok") {
        props.target_tok = v;
    }
    if let Some(v) = tag_text(raw, "support_default_translations") {
        props.support_default_translations = v == "true";
    }
    if let Some(v) = tag_text(raw, "remove_tags") {
        props.remove_tags = v == "true";
    }
    if let Some(v) = tag_text(raw, "external_command") {
        props.external_command = v;
    }
    let mut excludes = Vec::new();
    let mut rest = raw;
    while let Some(s) = rest.find("<mask>") {
        rest = &rest[s + 6..];
        if let Some(e) = rest.find("</mask>") {
            excludes.push(html_escape::decode_html_entities(&rest[..e]).into_owned());
            rest = &rest[e + 7..];
        } else {
            break;
        }
    }
    if !excludes.is_empty() {
        props.source_dir_excludes = excludes;
    }
    if raw.contains("<repository") {
        let mut search = raw;
        while let Some(start) = search.find("<repository") {
            let slice = &search[start..];
            let end = slice.find("</repository>").unwrap_or(slice.len());
            let block = &slice[..end.min(slice.len())];
            let repo_type = attr(block, "type").unwrap_or_else(|| "git".into());
            let url = attr(block, "url").unwrap_or_default();
            let branch = attr(block, "branch");
            props.repositories.push(RepositoryDef {
                repo_type,
                url,
                branch,
                mappings: parse_mappings(block),
            });
            search = &search[start + 12..];
        }
    }
    Ok(props)
}

fn parse_mappings(block: &str) -> Vec<RepositoryMapping> {
    let mut out = Vec::new();
    let mut rest = block;
    while let Some(start) = rest.find("<mapping") {
        let slice = &rest[start..];
        let after = &slice["<mapping".len()..];
        let Some(gt) = after.find('>') else { break };
        let open = &slice[.."<mapping".len() + gt + 1];
        let self_close = after[..gt].trim_end().ends_with('/');
        let local = attr(open, "local").unwrap_or_default();
        let repository = attr(open, "repository").unwrap_or_default();
        let (inner, next) = if self_close {
            ("", &slice["<mapping".len() + gt + 1..])
        } else if let Some(end) = slice.find("</mapping>") {
            (
                &slice["<mapping".len() + gt + 1..end],
                &slice[end + "</mapping>".len()..],
            )
        } else {
            break;
        };
        out.push(RepositoryMapping {
            local,
            repository,
            includes: collect_tags(inner, "includes"),
            excludes: collect_tags(inner, "excludes"),
        });
        rest = next;
    }
    out
}

fn collect_tags(raw: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(s) = rest.find(&open) {
        rest = &rest[s + open.len()..];
        if let Some(e) = rest.find(&close) {
            out.push(html_escape::decode_html_entities(&rest[..e]).into_owned());
            rest = &rest[e + close.len()..];
        } else {
            break;
        }
    }
    out
}

fn attr(block: &str, name: &str) -> Option<String> {
    let key = format!("{name}=\"");
    let s = block.find(&key)? + key.len();
    let e = block[s..].find('"')? + s;
    Some(block[s..e].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_repository_mappings_and_filters() {
        let raw = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<omegat>
    <project version="1.0">
        <source_lang>en</source_lang>
        <target_lang>fr</target_lang>
        <repositories>
            <repository type="git" url="https://example.com/p.git" branch="main">
                <mapping local="/" repository="/">
                    <includes>**/*.tmx</includes>
                    <excludes>**/*.bak</excludes>
                </mapping>
                <mapping local="source/" repository="src/"/>
            </repository>
        </repositories>
    </project>
</omegat>
"#;
        let props = parse_project_xml(Path::new("/tmp/proj"), raw).unwrap();
        assert_eq!(props.repositories.len(), 1);
        assert_eq!(props.repositories[0].repo_type, "git");
        assert_eq!(props.repositories[0].branch.as_deref(), Some("main"));
        assert_eq!(props.repositories[0].mappings.len(), 2);
        assert_eq!(props.repositories[0].mappings[0].local, "/");
        assert_eq!(props.repositories[0].mappings[0].includes, vec!["**/*.tmx"]);
        assert_eq!(props.repositories[0].mappings[0].excludes, vec!["**/*.bak"]);
        assert_eq!(props.repositories[0].mappings[1].repository, "src/");
    }

    #[test]
    fn write_roundtrips_mapping_includes() {
        let mut props = ProjectProperties::create(PathBuf::from("/tmp/p"), "en".into(), "fr".into(), true);
        props.repositories.push(RepositoryDef {
            repo_type: "http".into(),
            url: "https://example.com/mem.tmx".into(),
            branch: None,
            mappings: vec![RepositoryMapping {
                local: "omegat/project_save.tmx".into(),
                repository: "project_save.tmx".into(),
                includes: vec![],
                excludes: vec!["*.bak".into()],
            }],
        });
        let xml = props.to_xml();
        assert!(xml.contains("type=\"http\""));
        assert!(xml.contains("<excludes>*.bak</excludes>"));
        let again = parse_project_xml(Path::new("/tmp/p"), &xml).unwrap();
        assert_eq!(again.repositories[0].mappings[0].excludes, vec!["*.bak"]);
    }
}
