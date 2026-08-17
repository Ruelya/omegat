//! Java `SourceTextEntry` / `EntryKey` counterpart.

use crate::tags;
use omegat_ipc::EntryDto;

#[derive(Debug, Clone)]
pub struct Entry {
    pub file: String,
    pub id: String,
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
    pub fn translated(&self) -> bool {
        !self.translation.trim().is_empty()
    }

    pub fn to_dto(&self, index: usize) -> EntryDto {
        EntryDto {
            index,
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
