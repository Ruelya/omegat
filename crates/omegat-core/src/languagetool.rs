use crate::cancellation::CancellationToken;
use crate::language::Language;
use omegat_ipc::IssueDto;
use std::io::Read;
use std::process::Stdio;
use std::thread;
use std::time::Duration;

/// Java `LanguageToolNativeBridge.getLTLanguage` class names.
pub fn lt_language_class(code: &str) -> Option<&'static str> {
    let lang = Language::new(Some(code));
    match (lang.get_language_code(), lang.get_country_code()) {
        ("en", "US") => Some("org.languagetool.language.AmericanEnglish"),
        ("en", "CA") => Some("org.languagetool.language.CanadianEnglish"),
        ("en", _) => Some("org.languagetool.language.English"),
        ("be", _) => Some("org.languagetool.language.Belarusian"),
        ("fr", _) => Some("org.languagetool.language.French"),
        _ => None,
    }
}

pub fn default_bridge_type() -> &'static str {
    "http"
}

pub const UNCONFIGURED_MESSAGE: &str =
    "LanguageTool is not configured. Set languagetool_url to an HTTP v2/check endpoint. The embedded LT JAR is not used.";

/// LanguageTool HTTP `v2/check`. When `endpoint` is None the checker reports a
/// degradation issue instead of pretending the text was clean.
pub fn check(endpoint: Option<&str>, text: &str, lang: &str, index: usize, file: &str) -> Vec<IssueDto> {
    check_cancellable(
        endpoint,
        text,
        lang,
        index,
        file,
        &CancellationToken::default(),
    )
    .unwrap_or_default()
}

pub fn check_cancellable(
    endpoint: Option<&str>,
    text: &str,
    lang: &str,
    index: usize,
    file: &str,
    cancellation: &CancellationToken,
) -> Option<Vec<IssueDto>> {
    if cancellation.is_cancelled() {
        return None;
    }
    let Some(url) = endpoint.filter(|s| !s.is_empty()) else {
        return Some(vec![IssueDto {
            kind: "languagetool".into(),
            index,
            file: file.to_string(),
            message: UNCONFIGURED_MESSAGE.into(),
            severity: "info".into(),
        }]);
    };
    if text.trim().is_empty() {
        return Some(vec![]);
    }
    match check_http_cancellable(url, text, lang, cancellation) {
        Ok(issues) => Some(issues
            .into_iter()
            .map(|m| IssueDto {
                kind: "languagetool".into(),
                index,
                file: file.to_string(),
                message: m,
                severity: "warn".into(),
            })
            .collect()),
        Err(_) if cancellation.is_cancelled() => None,
        Err(e) => Some(vec![IssueDto {
            kind: "languagetool".into(),
            index,
            file: file.to_string(),
            message: format!("LanguageTool unavailable: {e}"),
            severity: "info".into(),
        }]),
    }
}

pub fn check_http(endpoint: &str, text: &str, lang: &str) -> Result<Vec<String>, String> {
    check_http_cancellable(endpoint, text, lang, &CancellationToken::default())
}

pub fn check_http_cancellable(
    endpoint: &str,
    text: &str,
    lang: &str,
    cancellation: &CancellationToken,
) -> Result<Vec<String>, String> {
    if cancellation.is_cancelled() {
        return Err("request cancelled".into());
    }
    if let Some(rest) = endpoint.strip_prefix("fixture:") {
        let raw = std::fs::read_to_string(rest).map_err(|e| e.to_string())?;
        if cancellation.is_cancelled() {
            return Err("request cancelled".into());
        }
        return parse_lt_json(&raw);
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
    let raw = http_exchange_cancellable(
        "POST",
        &url,
        Some(("application/x-www-form-urlencoded", &body)),
        cancellation,
    )?;
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
            let rule = m
                .pointer("/rule/id")
                .and_then(|x| x.as_str())
                .or_else(|| m.get("rule").and_then(|x| x.as_str()))
                .unwrap_or("-");
            let offset = m.get("offset").and_then(|x| x.as_i64()).unwrap_or(0);
            out.push(format!("{msg} [{rule}] @{offset}"));
        }
    }
    Ok(out)
}

pub fn http_get(url: &str) -> Result<String, String> {
    http_exchange("GET", url, None)
}

pub fn http_exchange(method: &str, url: &str, body: Option<(&str, &str)>) -> Result<String, String> {
    http_exchange_cancellable(
        method,
        url,
        body,
        &CancellationToken::default(),
    )
}

/// Run curl while allowing the NDJSON request that owns it to terminate the
/// process. Reader threads drain both pipes so a large response cannot block
/// the child before `try_wait` observes its exit.
pub fn http_exchange_cancellable(
    method: &str,
    url: &str,
    body: Option<(&str, &str)>,
    cancellation: &CancellationToken,
) -> Result<String, String> {
    if cancellation.is_cancelled() {
        return Err("request cancelled".into());
    }
    if let Some(path) = url.strip_prefix("fixture:") {
        let result = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        return if cancellation.is_cancelled() {
            Err("request cancelled".into())
        } else {
            Ok(result)
        };
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("unsupported URL".into());
    }
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-sS", "-X", method, "--max-time", "15"]);
    if let Some((ct, b)) = body {
        cmd.args(["-H", &format!("Content-Type: {ct}"), "--data-binary", b]);
    }
    let mut child = cmd
        .arg(url)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("curl stdout unavailable")?;
    let mut stderr = child.stderr.take().ok_or("curl stderr unavailable")?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.read_to_end(&mut bytes);
        bytes
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let status = loop {
        if cancellation.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            return Err("request cancelled".into());
        }
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) => break status,
            None => thread::sleep(Duration::from_millis(10)),
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| "curl stdout reader failed".to_string())?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| "curl stderr reader failed".to_string())?;
    if !status.success() {
        return Err(String::from_utf8_lossy(&stderr).into_owned());
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lt_fixture_fields() {
        let raw = r#"{"matches":[{"message":"Possible typo","offset":0,"length":4,"rule":{"id":"MORFOLOGIK_RULE_EN_US"}}]}"#;
        let hits = parse_lt_json(raw).unwrap();
        assert!(hits[0].contains("Possible typo"));
        assert!(hits[0].contains("MORFOLOGIK_RULE_EN_US"));
        assert!(hits[0].contains("@0"));
    }

    #[test]
    fn fixture_url_and_degraded_when_unset() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lt/check.json");
        let url = format!("fixture:{}", path.display());
        let hits = check(Some(&url), "teh cat", "en", 0, "a.txt");
        assert_eq!(hits[0].kind, "languagetool");
        assert!(hits[0].message.contains("typo"));
        let none = check(None, "teh cat", "en", 0, "a.txt");
        assert_eq!(none.len(), 1);
        assert_eq!(none[0].severity, "info");
        assert!(none[0].message.contains("not configured"));
    }

    #[test]
    fn cancelled_check_never_becomes_a_degraded_issue() {
        let cancellation = CancellationToken::default();
        cancellation.cancel();
        assert!(check_cancellable(
            Some("http://127.0.0.1:9/v2/check"),
            "teh cat",
            "en",
            0,
            "a.txt",
            &cancellation,
        )
        .is_none());
    }
}
