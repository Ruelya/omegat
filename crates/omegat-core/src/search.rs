use crate::session::Entry;
use omegat_ipc::{SearchHitDto, SearchParams};
use regex::Regex;

pub fn search(entries: &[Entry], params: &SearchParams) -> Vec<SearchHitDto> {
    let mut hits = Vec::new();
    let re = if params.regex {
        Regex::new(&params.query).ok()
    } else {
        None
    };
    let q = params.query.to_lowercase();
    for (index, e) in entries.iter().enumerate() {
        if params.source && matches_field(&e.source, &q, re.as_ref()) {
            hits.push(SearchHitDto {
                index,
                file: e.file.clone(),
                field: "source".into(),
                text: e.source.clone(),
            });
        }
        if params.translation && matches_field(&e.translation, &q, re.as_ref()) {
            hits.push(SearchHitDto {
                index,
                file: e.file.clone(),
                field: "translation".into(),
                text: e.translation.clone(),
            });
        }
    }
    hits
}

fn matches_field(text: &str, q: &str, re: Option<&Regex>) -> bool {
    if q.is_empty() {
        return false;
    }
    if let Some(re) = re {
        re.is_match(text)
    } else {
        text.to_lowercase().contains(q)
    }
}

pub fn replace(entries: &mut [Entry], params: &SearchParams) -> usize {
    let Some(repl) = &params.replace else {
        return 0;
    };
    let mut n = 0;
    if params.regex {
        if let Ok(re) = Regex::new(&params.query) {
            for e in entries.iter_mut() {
                if params.translation && re.is_match(&e.translation) {
                    e.translation = re.replace_all(&e.translation, repl.as_str()).into_owned();
                    e.revision += 1;
                    n += 1;
                }
            }
        }
    } else {
        for e in entries.iter_mut() {
            if params.translation && e.translation.contains(&params.query) {
                e.translation = e.translation.replace(&params.query, repl);
                e.revision += 1;
                n += 1;
            }
        }
    }
    n
}
