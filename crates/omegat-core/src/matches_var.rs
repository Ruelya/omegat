//! Java `org.omegat.gui.matches.MatchesVarExpansion`.

use crate::bidi::{self, Orientation};

#[derive(Debug, Clone, Default)]
pub struct MatchVars {
    pub id: i32,
    pub source_text: String,
    pub target_text: String,
    pub source_language: String,
    pub target_language: String,
    pub project_source_lang: String,
    pub project_target_lang: String,
    pub project_source_lang_code: String,
    pub project_target_lang_code: String,
    pub creation_id: String,
    pub changed_id: String,
    pub file_name: String,
    pub file_path: String,
    pub score: i32,
    pub no_stem_score: i32,
    pub adjusted_score: i32,
    pub fuzzy_flag: String,
}

pub fn expand_variables(template: &str, vars: &MatchVars) -> String {
    template
        .replace("${targetText}", &vars.target_text)
        .replace("${sourceLanguage}", &vars.source_language)
        .replace("${targetLanguage}", &vars.target_language)
        .replace("${projectSourceLang}", &vars.project_source_lang)
        .replace("${projectTargetLang}", &vars.project_target_lang)
        .replace("${projectSourceLangCode}", &vars.project_source_lang_code)
        .replace("${projectTargetLangCode}", &vars.project_target_lang_code)
        .replace("${creationId}", &vars.creation_id)
        .replace("${changedId}", &vars.changed_id)
        .replace("${initialCreationId}", &vars.creation_id)
        .replace("${fuzzyFlag}", &vars.fuzzy_flag)
        .replace("${fileName}", &vars.file_name)
        .replace("${filePath}", &vars.file_path)
        .replace("${score}", &vars.score.to_string())
        .replace("${noStemScore}", &vars.no_stem_score.to_string())
        .replace("${adjustedScore}", &vars.adjusted_score.to_string())
}

pub fn apply(template: &str, vars: &MatchVars) -> String {
    expand_variables(template, vars)
        .replace("${id}", &vars.id.to_string())
        .replace("${sourceText}", &vars.source_text)
}

pub fn apply_bidi(
    template: &str,
    vars: &MatchVars,
    source_lang: &str,
    target_lang: &str,
    locale: &str,
) -> String {
    let mut v = vars.clone();
    match bidi::orientation_type(Some(source_lang), Some(target_lang), locale) {
        Orientation::Differ => {
            if bidi::is_rtl(source_lang) {
                v.source_text = bidi::add_rtl_bidi_around(&v.source_text);
            } else {
                v.source_text = bidi::add_ltr_bidi_around(&v.source_text);
            }
            if bidi::is_rtl(target_lang) {
                v.target_text = bidi::add_rtl_bidi_around(&v.target_text);
            } else {
                v.target_text = bidi::add_ltr_bidi_around(&v.target_text);
            }
        }
        _ => {}
    }
    apply(template, &v)
}

pub fn mock_near_string() -> MatchVars {
    MatchVars {
        id: 2,
        source_text: "mock source text".into(),
        target_text: "mock target text".into(),
        source_language: "mock source language".into(),
        target_language: "mock target language".into(),
        project_source_lang: "pl".into(),
        project_target_lang: "pl".into(),
        project_source_lang_code: "pl".into(),
        project_target_lang_code: "pl".into(),
        creation_id: "mock creator".into(),
        changed_id: "mock modifier".into(),
        file_name: "mock testing project".into(),
        file_path: "mock testing project".into(),
        score: 20,
        no_stem_score: 40,
        adjusted_score: 60,
        fuzzy_flag: String::new(),
    }
}
