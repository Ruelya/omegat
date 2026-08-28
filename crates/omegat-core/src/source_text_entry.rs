//! Java `SourceTextEntry` / `EntryKey` counterpart.

use crate::tags;
use omegat_ipc::{EntryDto, EntryKeyDto};

#[derive(Debug, Clone)]
pub struct Entry {
    pub file: String,
    pub id: String,
    pub prev: Option<String>,
    pub next: Option<String>,
    pub path: Option<String>,
    pub source: String,
    pub translation: String,
    pub note: String,
    pub comment: String,
    pub default_translation: bool,
    pub revision: u64,
    pub from_tm_exact: bool,
    pub properties: Vec<(String, String)>,
}

impl Entry {
    pub fn key(&self) -> EntryKeyDto {
        EntryKeyDto {
            file: self.file.clone(),
            source_text: self.source.clone(),
            id: (!self.id.is_empty()).then(|| self.id.clone()),
            prev: self.prev.clone(),
            next: self.next.clone(),
            path: self.path.clone(),
        }
    }

    pub fn translated(&self) -> bool {
        !self.translation.trim().is_empty()
    }

    pub fn to_dto(&self, index: usize) -> EntryDto {
        EntryDto {
            index,
            key: self.key(),
            file: self.file.clone(),
            id: self.id.clone(),
            source: self.source.clone(),
            translation: self.translation.clone(),
            note: self.note.clone(),
            comment: self.comment.clone(),
            default_translation: self.default_translation,
            revision: self.revision,
            translated: self.translated(),
            tags: tags::extract_tags(&self.source),
            properties: self.properties.clone(),
        }
    }
}
