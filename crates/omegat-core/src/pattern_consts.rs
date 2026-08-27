//! Java `org.omegat.util.PatternConsts`.

use once_cell::sync::Lazy;
use regex::Regex;

/// Java `PatternConsts.LANG_AND_COUNTRY`.
pub static LANG_AND_COUNTRY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^([A-Za-z]{1,8})(?:(?:-|_)(?:[A-Za-z]{4}(?:-|_))?([A-Za-z0-9]{1,8}))?$").unwrap()
});

pub fn lang_and_country(s: &str) -> Option<(String, Option<String>)> {
    let c = LANG_AND_COUNTRY.captures(s)?;
    Some((
        c.get(1)?.as_str().to_string(),
        c.get(2).map(|m| m.as_str().to_string()),
    ))
}
