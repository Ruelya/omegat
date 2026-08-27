//! Java `org.omegat.util.BiDiUtils`.

use crate::language::Language;

pub const BIDI_LRE: &str = "\u{202a}";
pub const BIDI_RLE: &str = "\u{202b}";
pub const BIDI_PDF: &str = "\u{202c}";
pub const BIDI_LRM: &str = "\u{200e}";
pub const BIDI_RLM: &str = "\u{200f}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    AllLtr,
    AllRtl,
    Differ,
}

const RTL: &[&str] = &["AR", "HE", "FA", "UR", "YI", "IW", "JI"];

pub fn is_rtl(code: &str) -> bool {
    let lang = Language::new(Some(code));
    RTL.iter()
        .any(|r| lang.get_language_code().eq_ignore_ascii_case(r))
}

pub fn is_locale_rtl(locale: &str) -> bool {
    is_rtl(locale)
}

pub fn add_ltr_bidi_around(text: &str) -> String {
    format!("{BIDI_LRE}{text}{BIDI_PDF}")
}

pub fn add_rtl_bidi_around(text: &str) -> String {
    format!("{BIDI_RLE}{text}{BIDI_PDF}")
}

pub fn orientation_type(source: Option<&str>, target: Option<&str>, locale: &str) -> Orientation {
    match (source, target) {
        (None, None) => {
            if is_locale_rtl(locale) {
                Orientation::AllRtl
            } else {
                Orientation::AllLtr
            }
        }
        (Some(src), Some(tgt)) => {
            let s = is_rtl(src);
            let t = is_rtl(tgt);
            if s && t {
                Orientation::AllRtl
            } else if !s && !t {
                Orientation::AllLtr
            } else {
                Orientation::Differ
            }
        }
        _ => Orientation::Differ,
    }
}

pub fn is_mixed_orientation(o: Orientation) -> bool {
    o == Orientation::Differ
}
