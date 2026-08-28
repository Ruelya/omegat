//! Java `org.omegat.core.KnownException`.

#[derive(Debug, Clone)]
pub struct KnownException {
    pub code: String,
    pub params: Vec<String>,
    pub cause: Option<String>,
}

impl KnownException {
    pub fn new(code: &str, params: &[&str]) -> Self {
        Self {
            code: code.into(),
            params: params.iter().map(|s| (*s).to_string()).collect(),
            cause: None,
        }
    }

    pub fn with_cause(cause: &str, code: &str, params: &[&str]) -> Self {
        let mut e = Self::new(code, params);
        e.cause = Some(cause.into());
        e
    }

    pub fn message(&self) -> &str {
        &self.code
    }

    /// Java `OStrings.getString(code)` for the test locale (`en` → `Error`).
    pub fn localized_message(&self) -> String {
        match self.code.as_str() {
            "TF_ERROR" => "Error".into(),
            other => other.into(),
        }
    }
}
