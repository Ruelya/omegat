//! Java `org.omegat.util.Language`.

#[derive(Debug, Clone)]
pub struct Language {
    language: String,
    country: String,
    tag: String,
}

impl Language {
    pub fn new(raw: Option<&str>) -> Self {
        let Some(str) = raw.filter(|s| !s.is_empty()) else {
            return Self { language: String::new(), country: String::new(), tag: String::new() };
        };
        let normalized = str.replace('_', "-");
        let parts: Vec<&str> = normalized.split('-').collect();
        let language = parts.first().unwrap_or(&"").to_ascii_lowercase();
        let country = if parts.len() >= 2 && parts[1].len() != 4 {
            parts[1].to_ascii_uppercase()
        } else if parts.len() >= 3 {
            parts[2].to_ascii_uppercase()
        } else {
            String::new()
        };
        let tag = if country.is_empty() {
            language.clone()
        } else if parts.len() >= 3 && parts[1].len() == 4 {
            format!("{language}-{}-{country}", parts[1])
        } else {
            format!("{language}-{country}")
        };
        Self { language, country, tag }
    }

    pub fn get_language(&self) -> String {
        if self.tag.is_empty() {
            String::new()
        } else {
            // Java Locale.toLanguageTag() keeps the input-like BCP47 form.
            self.tag.clone()
        }
    }

    pub fn get_locale_code(&self) -> String {
        if self.language.is_empty() {
            return String::new();
        }
        let lang = match self.language.as_str() {
            "in" => "id",
            "iw" => "he",
            "ji" => "yi",
            other => other,
        };
        if self.country.is_empty() {
            lang.to_string()
        } else {
            format!("{lang}_{}", self.country)
        }
    }

    pub fn get_language_code(&self) -> &str {
        &self.language
    }

    pub fn get_country_code(&self) -> &str {
        &self.country
    }

    pub fn is_space_delimited(&self) -> bool {
        !matches!(self.language.to_ascii_uppercase().as_str(), "ZH" | "JA" | "BO")
    }

    pub fn verify_single_lang_code(code: &str) -> bool {
        if code.contains('+') {
            return false;
        }
        let re = regex::Regex::new(r"^[A-Za-z]{1,8}(-[A-Za-z0-9]{1,8})*$").unwrap();
        re.is_match(code)
    }

    pub fn lower_case_language_from_tag(tag: &str) -> String {
        Self::new(Some(tag)).language
    }

    pub fn upper_case_country_from_tag(tag: &str) -> String {
        Self::new(Some(tag)).country
    }
}

impl PartialEq for Language {
    fn eq(&self, other: &Self) -> bool {
        self.language == other.language && self.country == other.country
    }
}

impl Eq for Language {}
