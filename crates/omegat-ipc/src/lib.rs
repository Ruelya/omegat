//! JSON-RPC 2.0 types shared by the sidecar, CLI, and Electron renderer.
//!
//! Wire format is one JSON object per line (NDJSON) on stdin/stdout.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const APP_NAME: &str = "OmegaT";
pub const APP_VERSION: &str = "6.2.0";
pub const PROTOCOL_VERSION: &str = "1.0";

/// JSON-RPC reserved and application error codes.
pub mod error_code {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const UNIMPLEMENTED: i32 = -32000;
    pub const PROJECT_NOT_OPEN: i32 = -32001;
    pub const OPTIMISTIC_LOCK: i32 = -32002;
    pub const IO: i32 = -32003;
    pub const FILTER: i32 = -32004;
    pub const TEAM_CONFLICT: i32 = -32005;
    pub const TAG_VALIDATION: i32 = -32006;
    pub const AUTH: i32 = -32007;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    pub fn ok(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Value,
}

impl RpcNotification {
    pub fn new(method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub name: String,
    pub version: String,
    pub protocol: String,
    pub rewrite: bool,
}

impl Default for VersionInfo {
    fn default() -> Self {
        Self {
            name: APP_NAME.into(),
            version: APP_VERSION.into(),
            protocol: PROTOCOL_VERSION.into(),
            rewrite: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub phase: u8,
    pub filters: Vec<String>,
    pub features: FeatureFlags,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FeatureFlags {
    pub project: bool,
    pub tmx: bool,
    pub matching: bool,
    pub glossary: bool,
    pub compile: bool,
    pub search: bool,
    pub stats: bool,
    pub gui: bool,
    pub filters_a: bool,
    pub filters_b: bool,
    pub tags: bool,
    pub spell: bool,
    pub languagetool: bool,
    pub dictionary: bool,
    pub mt: bool,
    pub autocompleter: bool,
    pub finder: bool,
    pub team: bool,
    pub aligner: bool,
    pub script: bool,
    pub i18n: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPropsDto {
    pub root: String,
    pub source_lang: String,
    pub target_lang: String,
    pub sentence_seg: bool,
    pub source_dir: String,
    pub target_dir: String,
    pub tm_dir: String,
    pub glossary_dir: String,
    pub glossary_file: String,
    pub dictionary_dir: String,
    pub export_tm_levels: String,
    pub support_default_translations: bool,
    pub remove_tags: bool,
    pub has_repositories: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProjectParams {
    pub root: String,
    pub source_lang: String,
    pub target_lang: String,
    #[serde(default = "default_true")]
    pub sentence_seg: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenProjectParams {
    pub root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryDto {
    pub index: usize,
    pub file: String,
    pub id: String,
    pub source: String,
    pub translation: String,
    pub note: String,
    pub comment: String,
    pub default_translation: bool,
    pub revision: u64,
    pub translated: bool,
    pub tags: Vec<String>,
    pub properties: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetEntryParams {
    pub index: usize,
    pub translation: String,
    pub note: Option<String>,
    pub revision: u64,
    #[serde(default = "default_true")]
    pub default_translation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchDto {
    pub source: String,
    pub translation: String,
    pub score: i32,
    pub score_no_stem: i32,
    pub adjusted_score: i32,
    pub comes_from: String,
    pub project: Option<String>,
    /// Java `NearString.attr` / `buildSimilarityData` token alignment (0=equal, 1=insert, 2=delete).
    #[serde(default)]
    pub similarity: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossaryHitDto {
    pub source: String,
    pub target: String,
    pub comment: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatCountDto {
    pub segments: usize,
    pub words: usize,
    #[serde(rename = "characters-without-spaces", default)]
    pub characters_without_spaces: usize,
    pub characters: usize,
    pub files: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileStatDto {
    pub filename: String,
    pub total: StatCountDto,
    pub remaining: StatCountDto,
    pub unique: StatCountDto,
    #[serde(rename = "unique-remaining", default)]
    pub unique_remaining: StatCountDto,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchBinDto {
    pub exact: usize,
    pub fuzzy_95: usize,
    pub fuzzy_85: usize,
    pub fuzzy_75: usize,
    pub fuzzy_50: usize,
    pub none: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsDto {
    pub files: usize,
    pub segments: usize,
    pub translated: usize,
    pub unique_segments: usize,
    pub source_words: usize,
    pub target_words: usize,
    pub source_chars: usize,
    pub target_chars: usize,
    pub match_exact: usize,
    pub match_fuzzy: usize,
    pub match_none: usize,
    #[serde(default)]
    pub total: StatCountDto,
    #[serde(default)]
    pub remaining: StatCountDto,
    #[serde(default)]
    pub unique: StatCountDto,
    #[serde(rename = "unique-remaining", default)]
    pub unique_remaining: StatCountDto,
    #[serde(default)]
    pub file_stats: Vec<FileStatDto>,
    #[serde(default)]
    pub match_bins: MatchBinDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHitDto {
    pub index: usize,
    pub file: String,
    pub field: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
}

/// Java `SearchExpression` fields used by `SearchWindowController`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    #[serde(default)]
    pub regex: bool,
    #[serde(default = "default_true")]
    pub source: bool,
    #[serde(default = "default_true")]
    pub translation: bool,
    #[serde(default)]
    pub notes: bool,
    #[serde(default)]
    pub comments: bool,
    #[serde(default)]
    pub glossary: bool,
    #[serde(default)]
    pub tmx: bool,
    #[serde(default)]
    pub case_sensitive: bool,
    #[serde(default)]
    pub whole_word: bool,
    #[serde(default)]
    pub untranslated: bool,
    /// `exact` | `keyword` | `regex` (regex also accepted via `regex: true`).
    #[serde(default)]
    pub search_type: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
    #[serde(default)]
    pub replace: Option<String>,
    #[serde(default)]
    pub preview: bool,
}

impl Default for SearchParams {
    fn default() -> Self {
        Self {
            query: String::new(),
            regex: false,
            source: true,
            translation: true,
            notes: false,
            comments: false,
            glossary: false,
            tmx: false,
            case_sensitive: false,
            whole_word: false,
            untranslated: false,
            search_type: None,
            author: None,
            date_from: None,
            date_to: None,
            replace: None,
            preview: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDto {
    pub kind: String,
    pub index: usize,
    pub file: String,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterInfoDto {
    pub id: String,
    pub name: String,
    pub masks: Vec<String>,
    pub phase: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MtSuggestionDto {
    pub engine: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompleterItemDto {
    pub kind: String,
    pub text: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictHitDto {
    pub word: String,
    pub definition: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub plugin_type: String,
    pub entry: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_roundtrip() {
        let v = VersionInfo::default();
        let s = serde_json::to_string(&v).unwrap();
        let back: VersionInfo = serde_json::from_str(&s).unwrap();
        assert_eq!(back.version, APP_VERSION);
        assert!(back.rewrite);
    }

    #[test]
    fn rpc_ok_line() {
        let r = RpcResponse::ok(Value::from(1), serde_json::json!({"ok": true}));
        let line = serde_json::to_string(&r).unwrap();
        assert!(line.contains("\"jsonrpc\":\"2.0\""));
    }
}
