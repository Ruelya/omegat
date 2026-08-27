//! Plugin registry: `omegat-plugin.toml` + cdylib ABI.
//!
//! Host calls `omegat_plugin_register` so a plugin can register Filter / MT /
//! Tokenizer / Marker implementations. `omegat_plugin_abi` remains for discovery.

use omegat_filters::{
    ExtractedSegment, Filter, FilterContext, FilterError, FilterRegistry, ParsedFile,
    Result as FilterResult,
};
use omegat_ipc::PluginManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::io::{Read, Write};
use std::os::raw::{c_char, c_int, c_void};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("unknown plugin type: {0}")]
    UnknownType(String),
    #[error("plugin not found: {0}")]
    NotFound(String),
    #[error("invalid manifest: {0}")]
    Manifest(String),
    #[error("duplicate plugin marker: {0}")]
    DuplicateMarker(String),
    #[error("plugin marker {plugin} failed: {message}")]
    MarkerExecution { plugin: String, message: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    Filter,
    Tokenizer,
    Marker,
    Mt,
    Glossary,
    Dictionary,
    Theme,
    Repository,
    Spell,
    Language,
    Misc,
}

impl PluginType {
    pub fn parse(s: &str) -> Result<Self, PluginError> {
        match s.to_ascii_lowercase().as_str() {
            "filter" => Ok(Self::Filter),
            "tokenizer" => Ok(Self::Tokenizer),
            "marker" => Ok(Self::Marker),
            "mt" | "machinetranslator" => Ok(Self::Mt),
            "glossary" => Ok(Self::Glossary),
            "dictionary" => Ok(Self::Dictionary),
            "theme" => Ok(Self::Theme),
            "repository" => Ok(Self::Repository),
            "spell" | "spellcheck" => Ok(Self::Spell),
            "language" => Ok(Self::Language),
            "misc" | "miscellaneous" => Ok(Self::Misc),
            other => Err(PluginError::UnknownType(other.into())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToml {
    pub plugin: PluginManifest,
}

type ParseFn = extern "C" fn(*const c_char, *mut c_char, c_int) -> c_int;
type WriteFn = extern "C" fn(*const c_char, *const c_char, *const c_char) -> c_int;
type MarkFn = extern "C" fn(*const c_char, *mut c_char, c_int) -> c_int;

#[repr(C)]
struct OmegatPluginHost {
    ctx: *mut c_void,
    register_filter: Option<
        extern "C" fn(
            ctx: *mut c_void,
            id: *const c_char,
            name: *const c_char,
            masks: *const c_char,
            parse: ParseFn,
            write: WriteFn,
        ),
    >,
    register_mt: Option<extern "C" fn(ctx: *mut c_void, id: *const c_char, name: *const c_char)>,
    register_tokenizer:
        Option<extern "C" fn(ctx: *mut c_void, id: *const c_char, name: *const c_char)>,
    // ABI fields are append-only. Older plugins see the unchanged prefix.
    register_marker: Option<
        extern "C" fn(ctx: *mut c_void, id: *const c_char, name: *const c_char, mark: MarkFn),
    >,
}

struct Registration {
    filters: Vec<DynamicFilter>,
    mt: Vec<(String, String)>,
    tokenizers: Vec<(String, String)>,
    markers: Vec<DynamicMarker>,
}

fn cstr<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        return "";
    }
    unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("")
}

fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn leak_masks(spec: &str) -> &'static [&'static str] {
    let v: Vec<&'static str> = spec
        .split(|c| c == ',' || c == ';')
        .map(|m| m.trim())
        .filter(|m| !m.is_empty())
        .map(leak_str)
        .collect();
    Box::leak(v.into_boxed_slice())
}

extern "C" fn host_register_filter(
    ctx: *mut c_void,
    id: *const c_char,
    name: *const c_char,
    masks: *const c_char,
    parse: ParseFn,
    write: WriteFn,
) {
    if ctx.is_null() {
        return;
    }
    let reg = unsafe { &mut *(ctx as *mut Registration) };
    reg.filters.push(DynamicFilter {
        id: leak_str(cstr(id)),
        name: leak_str(cstr(name)),
        masks: leak_masks(cstr(masks)),
        parse_fn: parse,
        write_fn: write,
    });
}

extern "C" fn host_register_mt(ctx: *mut c_void, id: *const c_char, name: *const c_char) {
    if ctx.is_null() {
        return;
    }
    let reg = unsafe { &mut *(ctx as *mut Registration) };
    reg.mt.push((cstr(id).to_string(), cstr(name).to_string()));
}

extern "C" fn host_register_tokenizer(ctx: *mut c_void, id: *const c_char, name: *const c_char) {
    if ctx.is_null() {
        return;
    }
    let reg = unsafe { &mut *(ctx as *mut Registration) };
    reg.tokenizers
        .push((cstr(id).to_string(), cstr(name).to_string()));
}

extern "C" fn host_register_marker(
    ctx: *mut c_void,
    id: *const c_char,
    name: *const c_char,
    mark: MarkFn,
) {
    if ctx.is_null() {
        return;
    }
    let reg = unsafe { &mut *(ctx as *mut Registration) };
    reg.markers.push(DynamicMarker {
        plugin_id: String::new(),
        id: cstr(id).to_string(),
        name: cstr(name).to_string(),
        mark_fn: mark,
        library_path: None,
    });
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMarkerInfo {
    pub plugin_id: String,
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PluginEntryPart {
    Source,
    Translation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginMark {
    pub start_offset: usize,
    pub end_offset: usize,
    pub painter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub painter_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tooltip_text: Option<String>,
    pub entry_part: PluginEntryPart,
}

#[derive(Debug, Serialize, Deserialize)]
struct PluginMarks {
    marks: Vec<PluginMark>,
}

struct DynamicMarker {
    plugin_id: String,
    id: String,
    name: String,
    mark_fn: MarkFn,
    library_path: Option<PathBuf>,
}

impl DynamicMarker {
    fn info(&self) -> PluginMarkerInfo {
        PluginMarkerInfo {
            plugin_id: self.plugin_id.clone(),
            id: self.id.clone(),
            name: self.name.clone(),
        }
    }

    fn marks(&self, input: &serde_json::Value) -> Result<Vec<PluginMark>, PluginError> {
        let json = serde_json::to_string(input).map_err(|e| self.error(e.to_string()))?;
        let input = CString::new(json).map_err(|e| self.error(e.to_string()))?;
        let mut buf = vec![0u8; 1 << 20];
        let n = (self.mark_fn)(
            input.as_ptr(),
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as c_int,
        );
        if n < 0 || n as usize > buf.len() {
            return Err(self.error(format!("callback returned invalid length {n}")));
        }
        let raw = std::str::from_utf8(&buf[..n as usize])
            .map_err(|e| self.error(format!("output is not UTF-8: {e}")))?;
        self.parse_and_validate(raw, input.as_bytes())
    }

    fn marks_isolated(
        &self,
        executable: &Path,
        input: &serde_json::Value,
    ) -> Result<Vec<PluginMark>, PluginError> {
        let Some(library_path) = &self.library_path else {
            return self.marks(input);
        };
        let mut child = Command::new(executable)
            .arg("--plugin-marker-worker")
            .arg(library_path)
            .arg(&self.id)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| self.error(format!("cannot start isolated worker: {error}")))?;
        let serialized =
            serde_json::to_vec(input).map_err(|error| self.error(error.to_string()))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&serialized)
                .map_err(|error| self.error(format!("cannot write isolated worker input: {error}")))?;
        }
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| self.error("isolated worker stdout unavailable"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| self.error("isolated worker stderr unavailable"))?;
        let stdout_reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            let mut stdout = stdout;
            stdout.read_to_end(&mut output).map(|_| output)
        });
        let stderr_reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            let mut stderr = stderr;
            stderr.read_to_end(&mut output).map(|_| output)
        });
        let started = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() < Duration::from_secs(5) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(self.error("isolated worker timed out"));
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    return Err(self.error(format!("cannot wait for isolated worker: {error}")));
                }
            }
        };
        let stdout = stdout_reader
            .join()
            .map_err(|_| self.error("isolated worker stdout reader panicked"))?
            .map_err(|error| self.error(format!("cannot read isolated worker output: {error}")))?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| self.error("isolated worker stderr reader panicked"))?
            .map_err(|error| self.error(format!("cannot read isolated worker error: {error}")))?;
        if !status.success() {
            let detail = String::from_utf8_lossy(&stderr);
            return Err(self.error(format!(
                "isolated worker exited {status}{}",
                if detail.trim().is_empty() {
                    String::new()
                } else {
                    format!(": {}", detail.trim())
                }
            )));
        }
        let raw = std::str::from_utf8(&stdout)
            .map_err(|error| self.error(format!("worker output is not UTF-8: {error}")))?;
        self.parse_and_validate(raw, &serialized)
    }

    fn parse_and_validate(
        &self,
        raw: &str,
        input_json: &[u8],
    ) -> Result<Vec<PluginMark>, PluginError> {
        let output: PluginMarks = serde_json::from_str(raw)
            .map_err(|e| self.error(format!("invalid marks JSON: {e}")))?;
        let input: serde_json::Value = serde_json::from_slice(input_json)
            .map_err(|error| self.error(format!("invalid marker input JSON: {error}")))?;
        let source_len = input_text_utf16_len(&input, "source_text");
        let translation_len = input_text_utf16_len(&input, "translation_text");
        for mark in &output.marks {
            let limit = match mark.entry_part {
                PluginEntryPart::Source => source_len,
                PluginEntryPart::Translation => translation_len,
            };
            if mark.painter.trim().is_empty() {
                return Err(self.error("mark painter is empty"));
            }
            if mark.start_offset >= mark.end_offset || mark.end_offset > limit {
                return Err(self.error(format!(
                    "invalid {:?} UTF-16 span {}..{} for length {limit}",
                    mark.entry_part, mark.start_offset, mark.end_offset
                )));
            }
        }
        Ok(output.marks)
    }

    fn error(&self, message: impl Into<String>) -> PluginError {
        PluginError::MarkerExecution {
            plugin: self.id.clone(),
            message: message.into(),
        }
    }
}

fn input_text_utf16_len(input: &serde_json::Value, key: &str) -> usize {
    input
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .encode_utf16()
        .count()
}

/// Filter whose parse/write live in a loaded cdylib.
#[derive(Clone)]
pub struct DynamicFilter {
    id: &'static str,
    name: &'static str,
    masks: &'static [&'static str],
    parse_fn: ParseFn,
    write_fn: WriteFn,
}

impl Filter for DynamicFilter {
    fn id(&self) -> &'static str {
        self.id
    }
    fn name(&self) -> &'static str {
        self.name
    }
    fn default_masks(&self) -> &'static [&'static str] {
        self.masks
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> FilterResult<ParsedFile> {
        let c_path =
            CString::new(path.to_string_lossy().as_bytes()).map_err(|e| FilterError::Parse {
                format: self.id.to_string(),
                message: e.to_string(),
            })?;
        let mut buf = vec![0u8; 1 << 20];
        let n = (self.parse_fn)(
            c_path.as_ptr(),
            buf.as_mut_ptr() as *mut c_char,
            buf.len() as c_int,
        );
        if n < 0 {
            return Err(FilterError::Parse {
                format: self.id.to_string(),
                message: "plugin parse failed".into(),
            });
        }
        let raw = std::str::from_utf8(&buf[..n as usize]).map_err(|e| FilterError::Parse {
            format: self.id.to_string(),
            message: e.to_string(),
        })?;
        let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| FilterError::Parse {
            format: self.id.to_string(),
            message: e.to_string(),
        })?;
        let mut segments = Vec::new();
        if let Some(arr) = v.get("segments").and_then(|s| s.as_array()) {
            for (i, item) in arr.iter().enumerate() {
                segments.push(ExtractedSegment {
                    id: item
                        .get("id")
                        .and_then(|x| x.as_str())
                        .unwrap_or(&i.to_string())
                        .to_string(),
                    source: item
                        .get("source")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    existing_translation: None,
                    note: item
                        .get("note")
                        .and_then(|x| x.as_str())
                        .map(|s| s.to_string()),
                    comment: None,
                    path: None,
                    protected_parts: vec![],
                });
            }
        }
        Ok(ParsedFile {
            segments,
            skeleton: None,
        })
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> FilterResult<()> {
        let src = CString::new(source_path.to_string_lossy().as_bytes()).map_err(|e| {
            FilterError::Parse {
                format: self.id.to_string(),
                message: e.to_string(),
            }
        })?;
        let dest = CString::new(dest_path.to_string_lossy().as_bytes()).map_err(|e| {
            FilterError::Parse {
                format: self.id.to_string(),
                message: e.to_string(),
            }
        })?;
        let json = serde_json::to_string(translations).unwrap_or_else(|_| "{}".into());
        let c_json = CString::new(json).map_err(|e| FilterError::Parse {
            format: self.id.to_string(),
            message: e.to_string(),
        })?;
        let rc = (self.write_fn)(src.as_ptr(), dest.as_ptr(), c_json.as_ptr());
        if rc != 0 {
            return Err(FilterError::Parse {
                format: self.id.to_string(),
                message: "plugin write failed".into(),
            });
        }
        Ok(())
    }
}

fn load_dynamic_registration(
    library_path: &Path,
) -> Result<(libloading::Library, Registration), PluginError> {
    // Safety: plugins are trusted local cdylibs explicitly named by a manifest.
    let library = unsafe { libloading::Library::new(library_path) }
        .map_err(|error| PluginError::Manifest(error.to_string()))?;
    type AbiFn = unsafe extern "C" fn() -> *const c_char;
    type RegisterFn = unsafe extern "C" fn(*const OmegatPluginHost);
    if let Ok(symbol) = unsafe { library.get::<AbiFn>(b"omegat_plugin_abi\0") } {
        let pointer = unsafe { symbol() };
        if !pointer.is_null() {
            let _ = unsafe { CStr::from_ptr(pointer) }.to_string_lossy();
        }
    }
    let mut pending = Registration {
        filters: Vec::new(),
        mt: Vec::new(),
        tokenizers: Vec::new(),
        markers: Vec::new(),
    };
    if let Ok(symbol) = unsafe { library.get::<RegisterFn>(b"omegat_plugin_register\0") } {
        let host = OmegatPluginHost {
            ctx: &mut pending as *mut Registration as *mut c_void,
            register_filter: Some(host_register_filter),
            register_mt: Some(host_register_mt),
            register_tokenizer: Some(host_register_tokenizer),
            register_marker: Some(host_register_marker),
        };
        unsafe { symbol(&host) };
    }
    Ok((library, pending))
}

/// Execute one registered Marker callback inside the current helper process.
///
/// Product callers use [`PluginRegistry::enable_marker_isolation`] so this is
/// reached only after the sidecar has spawned itself in worker mode.
pub fn run_marker_worker(
    library_path: &Path,
    marker_id: &str,
    input: &serde_json::Value,
) -> Result<Vec<PluginMark>, PluginError> {
    let (_library, pending) = load_dynamic_registration(library_path)?;
    pending
        .markers
        .iter()
        .find(|marker| marker.id == marker_id)
        .ok_or_else(|| PluginError::NotFound(marker_id.to_string()))?
        .marks(input)
}

pub struct PluginRegistry {
    by_type: HashMap<PluginType, Vec<PluginManifest>>,
    dyn_filters: Vec<DynamicFilter>,
    mt: Vec<(String, String)>,
    tokenizers: Vec<(String, String)>,
    markers: Vec<DynamicMarker>,
    marker_worker_executable: Option<PathBuf>,
    _libs: Vec<libloading::Library>,
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            by_type: HashMap::new(),
            dyn_filters: Vec::new(),
            mt: Vec::new(),
            tokenizers: Vec::new(),
            markers: Vec::new(),
            marker_worker_executable: None,
            _libs: Vec::new(),
        };
        reg.register_builtin();
        reg
    }

    pub fn register(&mut self, manifest: PluginManifest) -> Result<(), PluginError> {
        let ty = PluginType::parse(&manifest.plugin_type)?;
        self.by_type.entry(ty).or_default().push(manifest);
        Ok(())
    }

    pub fn list(&self, ty: Option<PluginType>) -> Vec<PluginManifest> {
        match ty {
            Some(t) => self.by_type.get(&t).cloned().unwrap_or_default(),
            None => self.by_type.values().flatten().cloned().collect(),
        }
    }

    pub fn extra_filters(&self) -> Vec<Box<dyn Filter>> {
        self.dyn_filters
            .iter()
            .map(|f| Box::new(f.clone()) as Box<dyn Filter>)
            .collect()
    }

    pub fn registered_mt(&self) -> &[(String, String)] {
        &self.mt
    }

    pub fn registered_tokenizers(&self) -> &[(String, String)] {
        &self.tokenizers
    }

    pub fn registered_markers(&self) -> Vec<PluginMarkerInfo> {
        self.markers.iter().map(DynamicMarker::info).collect()
    }

    pub fn enable_marker_isolation(&mut self, executable: impl Into<PathBuf>) {
        self.marker_worker_executable = Some(executable.into());
    }

    pub fn marker_marks(
        &self,
        id: &str,
        input: &serde_json::Value,
    ) -> Result<Vec<PluginMark>, PluginError> {
        let marker = self
            .markers
            .iter()
            .find(|marker| marker.id == id)
            .ok_or_else(|| PluginError::NotFound(id.to_string()))?;
        match &self.marker_worker_executable {
            Some(executable) => marker.marks_isolated(executable, input),
            None => marker.marks(input),
        }
    }

    pub fn filter_registry(&self) -> FilterRegistry {
        let mut reg = FilterRegistry::new();
        for f in self.extra_filters() {
            reg.register(f);
        }
        reg
    }

    /// Load every `omegat-plugin.toml` under `dir` and `dlopen` the `entry` cdylib.
    pub fn load_dir(&mut self, dir: &Path) -> Result<Vec<String>, PluginError> {
        let mut loaded = Vec::new();
        if !dir.exists() {
            return Ok(loaded);
        }
        let rd = std::fs::read_dir(dir).map_err(|e| PluginError::Manifest(e.to_string()))?;
        for ent in rd.flatten() {
            let p = ent.path();
            let manifest_path = if p.is_dir() {
                let toml = p.join("omegat-plugin.toml");
                let json = p.join("omegat-plugin.json");
                if toml.exists() {
                    toml
                } else {
                    json
                }
            } else if p.file_name().and_then(|s| s.to_str()) == Some("omegat-plugin.toml") {
                p
            } else {
                continue;
            };
            if !manifest_path.exists() {
                continue;
            }
            let raw = std::fs::read_to_string(&manifest_path)
                .map_err(|e| PluginError::Manifest(e.to_string()))?;
            let m = Self::parse_toml(&raw)?;
            if m.entry != "builtin" && !m.entry.is_empty() {
                let lib = manifest_path.parent().unwrap_or(dir).join(&m.entry);
                if lib.exists() {
                    let (dynlib, mut pending) = load_dynamic_registration(&lib)?;
                    for marker in &pending.markers {
                        if marker.id.trim().is_empty() || marker.name.trim().is_empty() {
                            return Err(PluginError::Manifest(
                                "plugin marker id and name are required".into(),
                            ));
                        }
                        if self.markers.iter().any(|loaded| loaded.id == marker.id)
                            || pending
                                .markers
                                .iter()
                                .filter(|candidate| candidate.id == marker.id)
                                .count()
                                > 1
                        {
                            return Err(PluginError::DuplicateMarker(marker.id.clone()));
                        }
                    }
                    for marker in &mut pending.markers {
                        marker.plugin_id.clone_from(&m.id);
                        marker.library_path = Some(lib.clone());
                    }
                    self.dyn_filters.extend(pending.filters);
                    self.mt.extend(pending.mt);
                    self.tokenizers.extend(pending.tokenizers);
                    self.markers.extend(pending.markers);
                    self._libs.push(dynlib);
                    loaded.push(m.id.clone());
                }
            }
            self.register(m)?;
        }
        Ok(loaded)
    }

    pub fn load_default_dirs(&mut self, config_dir: &Path) {
        let _ = self.load_dir(&config_dir.join("plugins"));
        let _ = self.load_dir(Path::new("plugins"));
        if let Ok(dir) = std::env::var("OMEGAT_PLUGINS_DIR") {
            let _ = self.load_dir(Path::new(&dir));
        }
    }

    pub fn parse_toml(src: &str) -> Result<PluginManifest, PluginError> {
        if let Ok(parsed) = serde_json::from_str::<PluginToml>(src) {
            PluginType::parse(&parsed.plugin.plugin_type)?;
            return Ok(parsed.plugin);
        }
        let mut id = String::new();
        let mut name = String::new();
        let mut version = String::new();
        let mut plugin_type = String::new();
        let mut entry = String::new();
        for line in src.lines() {
            let line = line.trim();
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().trim_matches('"').to_string();
                match k.trim() {
                    "id" => id = v,
                    "name" => name = v,
                    "version" => version = v,
                    "plugin_type" => plugin_type = v,
                    "entry" => entry = v,
                    _ => {}
                }
            }
        }
        if id.is_empty() {
            return Err(PluginError::Manifest("missing id".into()));
        }
        PluginType::parse(&plugin_type)?;
        Ok(PluginManifest {
            id,
            name,
            version,
            plugin_type,
            entry,
        })
    }

    fn register_builtin(&mut self) {
        let builtins = [
            ("core-filters", "Built-in file filters", "filter"),
            (
                "core-tokenizer",
                "Unicode / language tokenizers",
                "tokenizer",
            ),
            ("core-mt", "Machine translation connectors", "mt"),
            ("core-spell", "Spell checker backends", "spell"),
            ("core-dictionary", "StarDict / Lingvo DSL", "dictionary"),
            ("core-theme", "Ink / stone themes", "theme"),
            ("core-repository", "Git / SVN / HTTP / file", "repository"),
            ("core-script", "JavaScript event scripts", "misc"),
        ];
        for (id, name, ty) in builtins {
            let _ = self.register(PluginManifest {
                id: id.into(),
                name: name.into(),
                version: omegat_ipc::APP_VERSION.into(),
                plugin_type: ty.into(),
                entry: "builtin".into(),
            });
        }
    }
}

pub fn example_cdylib_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "omegat_example_plugin.dll"
    } else if cfg!(target_os = "macos") {
        "libomegat_example_plugin.dylib"
    } else {
        "libomegat_example_plugin.so"
    }
}

pub fn example_cdylib_path() -> std::path::PathBuf {
    let mut p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("../../target/debug");
    p.push(example_cdylib_name());
    if !p.exists() {
        let mut alt = std::path::PathBuf::from("target/debug");
        alt.push(example_cdylib_name());
        if alt.exists() {
            return alt;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_registry_has_filter_type() {
        let reg = PluginRegistry::new();
        assert!(!reg.list(Some(PluginType::Filter)).is_empty());
    }

    #[test]
    fn parse_manifest_toml() {
        let src = r#"
[plugin]
id = "demo"
name = "Demo"
version = "1.0.0"
plugin_type = "filter"
entry = "libdemo.so"
"#;
        let m = PluginRegistry::parse_toml(src).unwrap();
        assert_eq!(m.id, "demo");
    }

    #[test]
    fn load_dir_registers_toml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("omegat-plugin.toml"),
            r#"
id = "demo"
name = "Demo"
version = "1.0.0"
plugin_type = "filter"
entry = "builtin"
"#,
        )
        .unwrap();
        let mut reg = PluginRegistry::new();
        let loaded = reg.load_dir(dir.path()).unwrap();
        assert!(reg.list(None).iter().any(|p| p.id == "demo"));
        let _ = loaded;
    }

    fn ensure_example_cdylib() -> std::path::PathBuf {
        let lib = example_cdylib_path();
        if !lib.exists() {
            let status = std::process::Command::new("cargo")
                .args(["build", "-p", "omegat-example-plugin"])
                .status()
                .expect("cargo build example plugin");
            assert!(status.success(), "failed to build omegat-example-plugin");
        }
        let lib = example_cdylib_path();
        assert!(lib.exists(), "missing {}", lib.display());
        lib
    }

    #[test]
    fn example_plugin_registers_filter_and_parses_fixture() {
        let lib = ensure_example_cdylib();
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join(lib.file_name().unwrap());
        std::fs::copy(&lib, &dest).unwrap();
        std::fs::write(
            dir.path().join("omegat-plugin.toml"),
            format!(
                "id = \"example\"\nname = \"Example Filter\"\nversion = \"1.0.0\"\nplugin_type = \"filter\"\nentry = \"{}\"\n",
                dest.file_name().unwrap().to_string_lossy()
            ),
        )
        .unwrap();
        let mut reg = PluginRegistry::new();
        let loaded = reg.load_dir(dir.path()).unwrap();
        assert!(loaded.contains(&"example".to_string()));
        assert!(reg
            .list(Some(PluginType::Filter))
            .iter()
            .any(|p| p.id == "example"));
        let filters = reg.extra_filters();
        let filter = filters
            .iter()
            .find(|f| f.id() == "example")
            .expect("registered example filter");
        let fixture =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/plugin/sample.example");
        let parsed = filter.parse(&fixture, &FilterContext::default()).unwrap();
        assert_eq!(parsed.segments.len(), 2);
        assert_eq!(parsed.segments[0].source, "Hello from plugin");
        assert_eq!(parsed.segments[1].source, "Second line");
        let out = dir.path().join("out.example");
        let mut tr = HashMap::new();
        tr.insert("0".into(), "Bonjour depuis le greffon".into());
        tr.insert("1".into(), "Deuxieme ligne".into());
        filter
            .write(&fixture, &out, &tr, &FilterContext::default())
            .unwrap();
        let written = std::fs::read_to_string(&out).unwrap();
        assert!(written.contains("Bonjour depuis le greffon"));
        assert!(written.contains("Deuxieme ligne"));
        let list = reg.filter_registry();
        assert!(list.by_id("example").is_some());
        assert!(list.for_path(&fixture).map(|f| f.id()) == Some("example"));
        assert_eq!(
            reg.registered_markers(),
            vec![PluginMarkerInfo {
                plugin_id: "example".into(),
                id: "example.native-marker".into(),
                name: "org.omegat.example.NativePluginMarker".into(),
            }]
        );
        let marks = reg
            .marker_marks(
                "example.native-marker",
                &serde_json::json!({
                    "entry_key": {
                        "file": "source/sample.example",
                        "source_text": "Hello from plugin",
                        "id": "0",
                        "prev": "",
                        "next": "Second line",
                        "path": null
                    },
                    "source_text": "Hello from plugin",
                    "translation_text": "😀 plugin and plugin",
                    "is_active": true
                }),
            )
            .unwrap();
        assert_eq!(
            marks,
            vec![
                PluginMark {
                    start_offset: 3,
                    end_offset: 9,
                    painter: "native-plugin".into(),
                    painter_color: Some("#7c3aed".into()),
                    tooltip_text: Some("Example marker in source/sample.example".into()),
                    entry_part: PluginEntryPart::Translation,
                },
                PluginMark {
                    start_offset: 14,
                    end_offset: 20,
                    painter: "native-plugin".into(),
                    painter_color: Some("#7c3aed".into()),
                    tooltip_text: Some("Example marker in source/sample.example".into()),
                    entry_part: PluginEntryPart::Translation,
                },
            ]
        );
    }
}
