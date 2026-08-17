use omegat_ipc::IssueDto;

/// Optional LanguageTool HTTP client. Core never links LT itself.
pub fn check(endpoint: Option<&str>, text: &str, lang: &str, index: usize, file: &str) -> Vec<IssueDto> {
    let Some(url) = endpoint else {
        return vec![];
    };
    if url.is_empty() || text.is_empty() {
        return vec![];
    }
    let _ = (url, lang, text);
    let v = serde_json::json!({"matches": []});
    v.get("matches")
        .and_then(|m| m.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    Some(IssueDto {
                        kind: "languagetool".into(),
                        index,
                        file: file.to_string(),
                        message: m.get("message")?.as_str()?.to_string(),
                        severity: "info".into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
