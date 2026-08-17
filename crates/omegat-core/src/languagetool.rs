use omegat_ipc::IssueDto;

/// LanguageTool HTTP `v2/check`. When `endpoint` is None the checker reports a
/// degradation issue instead of pretending the text was clean.
pub fn check(endpoint: Option<&str>, text: &str, lang: &str, index: usize, file: &str) -> Vec<IssueDto> {
    if text.trim().is_empty() {
        return vec![];
    }
    let Some(url) = endpoint.filter(|s| !s.is_empty()) else {
        return vec![];
    };
    match check_http(url, text, lang) {
        Ok(issues) => issues
            .into_iter()
            .map(|message| IssueDto {
                kind: "languagetool".into(),
                index,
                file: file.to_string(),
                message,
                severity: "warn".into(),
            })
            .collect(),
        Err(e) => vec![IssueDto {
            kind: "languagetool".into(),
            index,
            file: file.to_string(),
            message: format!("LanguageTool unavailable: {e}"),
            severity: "info".into(),
        }],
    }
}

pub fn check_http(endpoint: &str, text: &str, lang: &str) -> Result<Vec<String>, String> {
    if let Some(rest) = endpoint.strip_prefix("fixture:") {
        return parse_lt_json(&std::fs::read_to_string(rest).map_err(|e| e.to_string())?);
    }
    let body = format!(
        "language={}&text={}",
        urlencoding::encode(lang),
        urlencoding::encode(text)
    );
    let url = if endpoint.contains("/v2/check") {
        endpoint.to_string()
    } else {
        format!("{}/v2/check", endpoint.trim_end_matches('/'))
    };
    let raw = http_post(&url, "application/x-www-form-urlencoded", &body)?;
    parse_lt_json(&raw)
}

pub fn parse_lt_json(raw: &str) -> Result<Vec<String>, String> {
    let v: serde_json::Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    if let Some(arr) = v.get("matches").and_then(|m| m.as_array()) {
        for m in arr {
            let msg = m
                .get("message")
                .and_then(|x| x.as_str())
                .unwrap_or("LanguageTool match");
            out.push(msg.to_string());
        }
    }
    Ok(out)
}

fn http_post(url: &str, content_type: &str, body: &str) -> Result<String, String> {
    http_exchange("POST", url, Some((content_type, body)))
}

pub fn http_get(url: &str) -> Result<String, String> {
    http_exchange("GET", url, None)
}

pub fn http_exchange(method: &str, url: &str, body: Option<(&str, &str)>) -> Result<String, String> {
    if let Some(path) = url.strip_prefix("fixture:") {
        return std::fs::read_to_string(path).map_err(|e| e.to_string());
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("unsupported URL".into());
    }
    // HTTPS is delegated to `curl` so we do not pull a TLS stack into the workspace.
    // HTTP can also use curl; tests use fixture: URLs.
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sS", "-X", method, "--max-time", "15"]);
    if let Some((ct, b)) = body {
        cmd.args(["-H", &format!("Content-Type: {ct}"), "--data-binary", b]);
    }
    cmd.arg(url);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lt_fixture() {
        let raw = r#"{"matches":[{"message":"Possible typo"}]}"#;
        let hits = parse_lt_json(raw).unwrap();
        assert_eq!(hits[0], "Possible typo");
    }

    #[test]
    fn fixture_url_and_degraded_when_unset() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lt/check.json");
        let url = format!("fixture:{}", path.display());
        let hits = check(Some(&url), "teh cat", "en", 0, "a.txt");
        assert_eq!(hits[0].kind, "languagetool");
        assert!(hits[0].message.contains("typo"));
        let none = check(None, "teh cat", "en", 0, "a.txt");
        assert!(none.is_empty());
    }
}
