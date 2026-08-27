//! Java `org.omegat.util.InlineTagHandler`.

use crate::xml_engine::TagType;
use std::collections::{HashMap, VecDeque};

#[derive(Default)]
pub struct InlineTagHandler {
    pair_tags: HashMap<String, i32>,
    shortcut_letters: HashMap<String, i32>,
    paired_other: HashMap<String, VecDeque<i32>>,
    current_i: Option<String>,
    current_pos: Option<String>,
    tag_index: i32,
}

impl InlineTagHandler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_bpt(&mut self, attrs: &[Option<String>]) {
        self.current_i = nvl(attrs);
        self.pair_tags
            .insert(self.current_i.clone().unwrap_or_default(), self.tag_index);
        self.tag_index += 1;
    }

    pub fn start_ept(&mut self, attrs: &[Option<String>]) {
        self.current_i = nvl(attrs);
    }

    pub fn start_other(&mut self) {
        self.current_i = None;
        self.current_pos = None;
    }

    pub fn set_tag_shortcut_letter(&mut self, letter: i32) {
        if letter != 0 {
            if let Some(i) = &self.current_i {
                self.shortcut_letters.insert(i.clone(), letter);
            }
        }
    }

    pub fn get_tag_shortcut_letter(&self) -> i32 {
        self.current_i
            .as_ref()
            .and_then(|i| self.shortcut_letters.get(i).copied())
            .unwrap_or(0)
    }

    pub fn end_bpt(&self) -> i32 {
        self.current_i
            .as_ref()
            .and_then(|i| self.pair_tags.get(i).copied())
            .unwrap_or(0)
    }

    pub fn end_ept(&self) -> i32 {
        self.end_bpt()
    }

    pub fn end_other(&mut self) -> i32 {
        let result = self.tag_index;
        self.tag_index += 1;
        result
    }

    pub fn set_current_pos(&mut self, pos: Option<String>) {
        self.current_pos = pos;
    }

    pub fn current_pos(&self) -> Option<&str> {
        self.current_pos.as_deref()
    }

    pub fn paired(&mut self, tag_name: &str, typ: TagType) -> i32 {
        match typ {
            TagType::Begin => {
                let result = self.tag_index;
                self.paired_other
                    .entry(tag_name.to_string())
                    .or_default()
                    .push_front(result);
                self.tag_index += 1;
                result
            }
            TagType::End => {
                if let Some(idx) = self
                    .paired_other
                    .get_mut(tag_name)
                    .and_then(|q| q.pop_front())
                {
                    idx
                } else {
                    let result = self.tag_index;
                    self.tag_index += 1;
                    result
                }
            }
            TagType::Alone => {
                let result = self.tag_index;
                self.tag_index += 1;
                result
            }
        }
    }
}

fn nvl(attrs: &[Option<String>]) -> Option<String> {
    attrs.iter().flatten().find(|s| !s.is_empty()).cloned()
}
