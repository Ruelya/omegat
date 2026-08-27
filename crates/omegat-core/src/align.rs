//! mALIGNa-style bitext alignment (Java `org.omegat.gui.align.Aligner`).
//!
//! HEAPWISE: filter-extract (or whole text) → SRX → length HMM.
//! PARSEWISE: both sides use the same filter, then each index pair is SRX'd and aligned.
//! ID: pair units that share a filter id (Resource Bundle keys).
//! Viterbi is a min-cost path; Forward-Backward is a posterior / soft path — not an alias.

use crate::cancellation::CancellationToken;
use crate::error::{CoreError, Result};
use crate::language::Language;
use crate::segment::split_sentences_lang;
use crate::tmx::{ProjectTmx, TmxEntry};
use omegat_filters::{FilterContext, FilterRegistry};
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static ALIGN_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub const PREF_ALGORITHM: &str = "aligner_algorithm_class";
pub const PREF_CALCULATOR: &str = "aligner_calculator_type";
pub const PREF_COUNTER: &str = "aligner_counter_type";
pub const PREF_SEGMENT: &str = "aligner_segment";
pub const PREF_REMOVE_TAGS: &str = "aligner_remove_tags";
pub const PREF_SOURCE_LANGUAGE: &str = "aligner_source_language";
pub const PREF_TARGET_LANGUAGE: &str = "aligner_target_language";
pub const PREF_LAST_SOURCE_DIR: &str = "aligner_last_source_dir";
pub const PREF_LAST_TARGET_DIR: &str = "aligner_last_target_dir";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignMode {
    Heapwise,
    Parsewise,
    Id,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignAlgo {
    Viterbi,
    ForwardBackward,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Counter {
    Char,
    Word,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalculatorType {
    Normal,
    Poisson,
}

#[derive(Debug, Clone)]
pub struct AlignConfig {
    pub mode: AlignMode,
    pub algo: AlignAlgo,
    pub counter: Counter,
    pub calculator: CalculatorType,
    pub segment: bool,
}

impl Default for AlignConfig {
    fn default() -> Self {
        Self {
            mode: AlignMode::Parsewise,
            algo: AlignAlgo::Viterbi,
            counter: Counter::Word,
            calculator: CalculatorType::Normal,
            segment: true,
        }
    }
}

/// Java `AlignPanelController` persisted aligner settings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AlignSettings {
    pub algorithm: String,
    pub calculator: String,
    pub counter: String,
    pub segment: bool,
    pub remove_tags: bool,
}

impl Default for AlignSettings {
    fn default() -> Self {
        Self {
            algorithm: "viterbi".into(),
            calculator: "normal".into(),
            counter: "word".into(),
            segment: true,
            remove_tags: false,
        }
    }
}

impl AlignSettings {
    pub fn persist(&self, store: &mut std::collections::HashMap<String, String>) {
        store.insert(PREF_ALGORITHM.into(), persisted_algo(&self.algorithm));
        store.insert(PREF_CALCULATOR.into(), self.calculator.to_ascii_uppercase());
        store.insert(PREF_COUNTER.into(), self.counter.to_ascii_uppercase());
        store.insert(PREF_SEGMENT.into(), self.segment.to_string());
        store.insert(PREF_REMOVE_TAGS.into(), self.remove_tags.to_string());
    }

    pub fn restore(store: &std::collections::HashMap<String, String>) -> Self {
        let mut s = Self::default();
        if let Some(v) = store.get(PREF_ALGORITHM) {
            if let Some(value) = normalize_algo(v) {
                s.algorithm = value;
            }
        }
        if let Some(v) = store.get(PREF_CALCULATOR) {
            if matches!(v.to_ascii_lowercase().as_str(), "normal" | "poisson") {
                s.calculator = v.to_ascii_lowercase();
            }
        }
        if let Some(v) = store.get(PREF_COUNTER) {
            if matches!(v.to_ascii_lowercase().as_str(), "word" | "char") {
                s.counter = v.to_ascii_lowercase();
            }
        }
        if let Some(v) = store.get(PREF_SEGMENT) {
            s.segment = v == "true";
        }
        if let Some(v) = store.get(PREF_REMOVE_TAGS) {
            s.remove_tags = v == "true";
        }
        s
    }
}

fn persisted_algo(v: &str) -> String {
    if normalize_algo(v).as_deref() == Some("forward-backward") {
        "FB".into()
    } else {
        "VITERBI".into()
    }
}

fn normalize_algo(v: &str) -> Option<String> {
    match v.to_ascii_lowercase().as_str() {
        "fb" | "forward-backward" | "forward_backward" => Some("forward-backward".into()),
        "viterbi" => Some("viterbi".into()),
        _ => None,
    }
}

/// Java `AlignFilePickerController.persistLanguages`.
pub fn persist_languages(
    store: &mut HashMap<String, String>,
    source: Option<&str>,
    target: Option<&str>,
) {
    if let Some(source) = source {
        store.insert(
            PREF_SOURCE_LANGUAGE.into(),
            Language::new(Some(source)).get_language(),
        );
    }
    if let Some(target) = target {
        store.insert(
            PREF_TARGET_LANGUAGE.into(),
            Language::new(Some(target)).get_language(),
        );
    }
}

/// Java `AlignFilePickerController.restorePersistedLanguage`.
pub fn restore_language(store: &HashMap<String, String>, key: &str, fallback: &str) -> String {
    store
        .get(key)
        .filter(|value| Language::verify_single_lang_code(value))
        .map(|value| Language::new(Some(value)).get_language())
        .unwrap_or_else(|| Language::new(Some(fallback)).get_language())
}

/// Java `AlignFilePickerController.persistInputDir`: remember a file's parent.
pub fn persist_input_dir(store: &mut HashMap<String, String>, key: &str, file: Option<&Path>) {
    if let Some(parent) = file.and_then(Path::parent) {
        if !parent.as_os_str().is_empty() {
            store.insert(key.into(), parent.to_string_lossy().into_owned());
        }
    }
}

/// Java `AlignFilePickerController.restorePersistedDir`.
pub fn restore_input_dir(store: &HashMap<String, String>, key: &str) -> Option<String> {
    store.get(key).filter(|value| !value.is_empty()).cloned()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignUnit {
    pub id: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Bead {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignSide {
    Both,
    Source,
    Target,
}

impl AlignSide {
    pub fn from_name(value: &str) -> Self {
        match value {
            "source" => Self::Source,
            "target" => Self::Target,
            _ => Self::Both,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BeadStatus {
    #[default]
    Default,
    Accepted,
    NeedsReview,
}

/// Mutable manual-alignment state corresponding to Java `MutableBead`.
///
/// Lines stay separate until TMX output, so a visual cell split does not lose
/// its language-aware join semantics. `None` is retained because Java's table
/// model uses null cells to represent the short side of an unbalanced bead.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MutableBead {
    pub score: f32,
    pub source_lines: Vec<Option<String>>,
    pub target_lines: Vec<Option<String>>,
    pub enabled: bool,
    pub status: BeadStatus,
}

impl MutableBead {
    pub fn from_lines(
        score: f32,
        source_lines: Vec<Option<String>>,
        target_lines: Vec<Option<String>>,
    ) -> Self {
        let equal = source_lines == target_lines;
        Self {
            score,
            source_lines,
            target_lines,
            enabled: !equal,
            status: if equal {
                BeadStatus::Accepted
            } else {
                BeadStatus::Default
            },
        }
    }

    pub fn new(source: impl Into<String>, target: impl Into<String>) -> Self {
        Self::from_lines(
            f32::MAX,
            vec![Some(source.into())],
            vec![Some(target.into())],
        )
    }

    pub fn empty() -> Self {
        Self {
            score: f32::MAX,
            source_lines: Vec::new(),
            target_lines: Vec::new(),
            enabled: true,
            status: BeadStatus::Default,
        }
    }

    pub fn is_balanced(&self) -> bool {
        self.source_lines.len() == self.target_lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.source_lines.is_empty() && self.target_lines.is_empty()
    }

    pub fn source_text(&self, language: &str) -> String {
        join_bead_lines(language, &self.source_lines)
    }

    pub fn target_text(&self, language: &str) -> String {
        join_bead_lines(language, &self.target_lines)
    }
}

fn join_bead_lines(language: &str, lines: &[Option<String>]) -> String {
    let delimiter = if Language::new(Some(language)).is_space_delimited() {
        " "
    } else {
        ""
    };
    lines
        .iter()
        .map(|line| line.as_deref().unwrap_or("null"))
        .collect::<Vec<_>>()
        .join(delimiter)
}

pub fn beads_to_pairs(
    beads: &[MutableBead],
    source_language: &str,
    target_language: &str,
) -> Vec<(String, String)> {
    beads
        .iter()
        .filter(|bead| bead.enabled)
        .map(|bead| {
            (
                bead.source_text(source_language),
                bead.target_text(target_language),
            )
        })
        .collect()
}

pub fn merge_beads(beads: &[MutableBead], index: usize, side: AlignSide) -> Vec<MutableBead> {
    let mut out = beads.to_vec();
    if index + 1 >= out.len() {
        return out;
    }
    match side {
        AlignSide::Both => {
            let next = out.remove(index + 1);
            out[index].source_lines.extend(next.source_lines);
            out[index].target_lines.extend(next.target_lines);
            out[index].status = BeadStatus::Default;
        }
        AlignSide::Source => {
            let lines = std::mem::take(&mut out[index + 1].source_lines);
            out[index].source_lines.extend(lines);
            out[index].status = BeadStatus::Default;
            out[index + 1].status = BeadStatus::Default;
        }
        AlignSide::Target => {
            let lines = std::mem::take(&mut out[index + 1].target_lines);
            out[index].target_lines.extend(lines);
            out[index].status = BeadStatus::Default;
            out[index + 1].status = BeadStatus::Default;
        }
    }
    out.retain(|bead| !bead.is_empty());
    out
}

pub fn move_bead_side(
    beads: &[MutableBead],
    from: usize,
    to: usize,
    side: AlignSide,
) -> Vec<MutableBead> {
    let mut out = beads.to_vec();
    if from >= out.len() || to >= out.len() || from == to {
        return out;
    }
    match side {
        AlignSide::Both => out.swap(from, to),
        AlignSide::Source => {
            let lines = out[from].source_lines.clone();
            out[from].source_lines = out[to].source_lines.clone();
            out[to].source_lines = lines;
            out[from].status = BeadStatus::Default;
            out[to].status = BeadStatus::Default;
        }
        AlignSide::Target => {
            let lines = out[from].target_lines.clone();
            out[from].target_lines = out[to].target_lines.clone();
            out[to].target_lines = lines;
            out[from].status = BeadStatus::Default;
            out[to].status = BeadStatus::Default;
        }
    }
    out
}

pub fn split_bead_line(
    beads: &[MutableBead],
    index: usize,
    side: AlignSide,
    line_index: usize,
    parts: &[String],
) -> Vec<MutableBead> {
    let mut out = beads.to_vec();
    let Some(bead) = out.get_mut(index) else {
        return out;
    };
    let lines = match side {
        AlignSide::Source => &mut bead.source_lines,
        AlignSide::Target => &mut bead.target_lines,
        AlignSide::Both => return out,
    };
    if line_index >= lines.len() || parts.len() < 2 {
        return out;
    }
    lines.splice(line_index..=line_index, parts.iter().cloned().map(Some));
    bead.status = BeadStatus::Default;
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BeadRow {
    pub bead_index: usize,
    pub row_in_bead: usize,
    pub source_line_index: Option<usize>,
    pub target_line_index: Option<usize>,
}

/// Flatten mutable beads into the visual rows used by the manual aligner.
///
/// A bead occupies the maximum of its source/target line counts. The shorter
/// side is represented by `None`, which lets selection APIs address complete
/// row spans without flattening the underlying `MutableBead` state.
pub fn bead_rows(beads: &[MutableBead]) -> Vec<BeadRow> {
    let mut rows = Vec::new();
    for (bead_index, bead) in beads.iter().enumerate() {
        let count = bead.source_lines.len().max(bead.target_lines.len());
        for row_in_bead in 0..count {
            rows.push(BeadRow {
                bead_index,
                row_in_bead,
                source_line_index: (row_in_bead < bead.source_lines.len()).then_some(row_in_bead),
                target_line_index: (row_in_bead < bead.target_lines.len()).then_some(row_in_bead),
            });
        }
    }
    rows
}

fn line_locations(
    beads: &[MutableBead],
    start_row: usize,
    end_row: usize,
    side: AlignSide,
) -> Vec<(usize, usize)> {
    if matches!(side, AlignSide::Both) {
        return Vec::new();
    }
    let rows = bead_rows(beads);
    if rows.is_empty() {
        return Vec::new();
    }
    let low = start_row.min(end_row).min(rows.len() - 1);
    let high = start_row.max(end_row).min(rows.len() - 1);
    rows[low..=high]
        .iter()
        .filter_map(|row| {
            let line_index = match side {
                AlignSide::Source => row.source_line_index,
                AlignSide::Target => row.target_line_index,
                AlignSide::Both => None,
            }?;
            let lines = match side {
                AlignSide::Source => &beads[row.bead_index].source_lines,
                AlignSide::Target => &beads[row.bead_index].target_lines,
                AlignSide::Both => unreachable!(),
            };
            lines
                .get(line_index)
                .and_then(Option::as_ref)
                .map(|_| (row.bead_index, line_index))
        })
        .collect()
}

fn lines_mut(bead: &mut MutableBead, side: AlignSide) -> &mut Vec<Option<String>> {
    match side {
        AlignSide::Source => &mut bead.source_lines,
        AlignSide::Target => &mut bead.target_lines,
        AlignSide::Both => unreachable!(),
    }
}

/// Replace every non-empty cell in a visual row span with the supplied lines.
///
/// The replacement is anchored at the first selected cell. All touched beads
/// lose accepted/review state, matching Java's destructive table edits.
pub fn replace_bead_row_span(
    beads: &[MutableBead],
    start_row: usize,
    end_row: usize,
    side: AlignSide,
    replacement: Vec<Option<String>>,
) -> Vec<MutableBead> {
    let locations = line_locations(beads, start_row, end_row, side);
    let Some(&(anchor_bead, anchor_line)) = locations.first() else {
        return beads.to_vec();
    };
    let mut out = beads.to_vec();
    let mut touched: Vec<usize> = locations.iter().map(|(bead, _)| *bead).collect();
    touched.sort_unstable();
    touched.dedup();
    for &(bead_index, line_index) in locations.iter().rev() {
        lines_mut(&mut out[bead_index], side).remove(line_index);
    }
    lines_mut(&mut out[anchor_bead], side).splice(anchor_line..anchor_line, replacement);
    for bead_index in touched {
        out[bead_index].status = BeadStatus::Default;
    }
    out.retain(|bead| !bead.is_empty());
    out
}

pub fn merge_bead_row_span(
    beads: &[MutableBead],
    start_row: usize,
    end_row: usize,
    side: AlignSide,
    language: &str,
) -> Vec<MutableBead> {
    let locations = line_locations(beads, start_row, end_row, side);
    if locations.len() < 2 {
        return beads.to_vec();
    }
    let selected: Vec<Option<String>> = locations
        .iter()
        .map(|&(bead_index, line_index)| {
            let lines = match side {
                AlignSide::Source => &beads[bead_index].source_lines,
                AlignSide::Target => &beads[bead_index].target_lines,
                AlignSide::Both => unreachable!(),
            };
            lines[line_index].clone()
        })
        .collect();
    let merged = join_bead_lines(language, &selected);
    replace_bead_row_span(beads, start_row, end_row, side, vec![Some(merged)])
}

/// Move a complete selected row span to the adjacent bead.
pub fn move_bead_row_span(
    beads: &[MutableBead],
    start_row: usize,
    end_row: usize,
    side: AlignSide,
    direction: isize,
) -> Vec<MutableBead> {
    if direction == 0 {
        return beads.to_vec();
    }
    let mut locations = line_locations(beads, start_row, end_row, side);
    let (Some(&(first_bead, _)), Some(&(last_bead, _))) = (locations.first(), locations.last())
    else {
        return beads.to_vec();
    };
    let values: Vec<Option<String>> = locations
        .iter()
        .map(|&(bead_index, line_index)| {
            let lines = match side {
                AlignSide::Source => &beads[bead_index].source_lines,
                AlignSide::Target => &beads[bead_index].target_lines,
                AlignSide::Both => unreachable!(),
            };
            lines[line_index].clone()
        })
        .collect();
    let mut out = beads.to_vec();
    let target_bead = if direction < 0 {
        if first_bead == 0 {
            out.insert(0, MutableBead::empty());
            for (bead_index, _) in &mut locations {
                *bead_index += 1;
            }
            0
        } else {
            first_bead - 1
        }
    } else if last_bead + 1 >= out.len() {
        out.push(MutableBead::empty());
        out.len() - 1
    } else {
        last_bead + 1
    };
    let mut touched: Vec<usize> = locations.iter().map(|(bead, _)| *bead).collect();
    for &(bead_index, line_index) in locations.iter().rev() {
        lines_mut(&mut out[bead_index], side).remove(line_index);
    }
    let target = lines_mut(&mut out[target_bead], side);
    if direction < 0 {
        target.extend(values);
    } else {
        target.splice(0..0, values);
    }
    touched.push(target_bead);
    touched.sort_unstable();
    touched.dedup();
    for bead_index in touched {
        out[bead_index].status = BeadStatus::Default;
    }
    out.retain(|bead| !bead.is_empty());
    out
}

fn real_line_rows(
    beads: &[MutableBead],
    start_row: usize,
    end_row: usize,
    side: AlignSide,
) -> Vec<(usize, usize, usize)> {
    if matches!(side, AlignSide::Both) {
        return Vec::new();
    }
    let rows = bead_rows(beads);
    if rows.is_empty() {
        return Vec::new();
    }
    let low = start_row.min(end_row).min(rows.len() - 1);
    let high = start_row.max(end_row).min(rows.len() - 1);
    rows[low..=high]
        .iter()
        .enumerate()
        .filter_map(|(offset, row)| {
            let line_index = match side {
                AlignSide::Source => row.source_line_index,
                AlignSide::Target => row.target_line_index,
                AlignSide::Both => None,
            }?;
            let line = match side {
                AlignSide::Source => &beads[row.bead_index].source_lines,
                AlignSide::Target => &beads[row.bead_index].target_lines,
                AlignSide::Both => unreachable!(),
            }
            .get(line_index)?;
            line.as_ref()
                .map(|_| (low + offset, row.bead_index, line_index))
        })
        .collect()
}

/// Apply Java `AlignTransferHandler.canImport` rules to a visual row span.
///
/// A drag has exactly one source/target column, ignores empty cells, must leave
/// the selected span, and may only move an edge line into a different bead.
pub fn can_move_bead_row_span_to(
    beads: &[MutableBead],
    start_row: usize,
    end_row: usize,
    side: AlignSide,
    target_row: isize,
) -> bool {
    let real_rows = real_line_rows(beads, start_row, end_row, side);
    let (Some(&(first_row, first_bead, first_line)), Some(&(last_row, last_bead, last_line))) =
        (real_rows.first(), real_rows.last())
    else {
        return false;
    };
    let rows = bead_rows(beads);
    let (boundary_row, boundary_bead, boundary_line, moving_up) = if target_row < first_row as isize
    {
        (first_row, first_bead, first_line, true)
    } else if target_row > last_row as isize {
        (last_row, last_bead, last_line, false)
    } else {
        return false;
    };
    let side_lines = match side {
        AlignSide::Source => &beads[boundary_bead].source_lines,
        AlignSide::Target => &beads[boundary_bead].target_lines,
        AlignSide::Both => return false,
    };
    let opposite_lines = match side {
        AlignSide::Source => &beads[boundary_bead].target_lines,
        AlignSide::Target => &beads[boundary_bead].source_lines,
        AlignSide::Both => unreachable!(),
    };
    let movable =
        if (boundary_row == 0 && moving_up) || (boundary_row + 1 == rows.len() && !moving_up) {
            !opposite_lines.is_empty()
        } else if moving_up {
            boundary_line == 0
        } else {
            boundary_line + 1 == side_lines.len()
        };
    if !movable {
        return false;
    }
    if let Ok(target) = usize::try_from(target_row) {
        if let Some(target) = rows.get(target) {
            return target.bead_index != boundary_bead;
        }
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BeadRowSelection {
    pub anchor_row: usize,
    pub focus_row: usize,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MoveBeadRowSpanResult {
    pub beads: Vec<MutableBead>,
    pub selection: Option<BeadRowSelection>,
}

/// Move selected source/target cells into the bead at an arbitrary drop row.
///
/// Insertion order follows Java `BeadTableModel.move`: moving upward appends
/// cells, while moving downward repeatedly inserts at index zero.
pub fn move_bead_row_span_to(
    beads: &[MutableBead],
    start_row: usize,
    end_row: usize,
    side: AlignSide,
    target_row: isize,
) -> Vec<MutableBead> {
    move_bead_row_span_to_with_selection(beads, start_row, end_row, side, target_row).beads
}

/// Move a row span and return the moved cells' new anchor/focus rows.
///
/// Java's `moveRows` clears the JTable selection before changing the model,
/// then restores it from the exact moved `String` instances. Returning the
/// rows from the product mutation avoids content matching (which is ambiguous
/// for duplicate lines) and lets non-Swing clients preserve the same lead
/// direction after a move.
pub fn move_bead_row_span_to_with_selection(
    beads: &[MutableBead],
    start_row: usize,
    end_row: usize,
    side: AlignSide,
    target_row: isize,
) -> MoveBeadRowSpanResult {
    if !can_move_bead_row_span_to(beads, start_row, end_row, side, target_row) {
        return MoveBeadRowSpanResult {
            beads: beads.to_vec(),
            selection: None,
        };
    }
    let rows = bead_rows(beads);
    let locations = real_line_rows(beads, start_row, end_row, side);
    let moving_up = target_row < locations[0].0 as isize;
    let mut out = beads.to_vec();
    let target_bead = if target_row < 0 {
        out.insert(0, MutableBead::empty());
        0
    } else if target_row as usize >= rows.len() {
        out.push(MutableBead::empty());
        out.len() - 1
    } else {
        rows[target_row as usize].bead_index
    };
    let shifted = usize::from(target_row < 0);
    let locations: Vec<(usize, usize)> = locations
        .into_iter()
        .map(|(_, bead, line)| (bead + shifted, line))
        .collect();
    let values: Vec<Option<String>> = locations
        .iter()
        .map(|&(bead, line)| lines_mut(&mut out[bead], side)[line].clone())
        .collect();
    let mut touched: Vec<usize> = locations.iter().map(|(bead, _)| *bead).collect();
    for &(bead, line) in locations.iter().rev() {
        lines_mut(&mut out[bead], side).remove(line);
    }
    let target = lines_mut(&mut out[target_bead], side);
    let insertion_start = target.len();
    if moving_up {
        target.extend(values);
    } else {
        for value in values {
            target.insert(0, value);
        }
    }
    touched.push(target_bead);
    touched.sort_unstable();
    touched.dedup();
    for bead in touched {
        out[bead].status = BeadStatus::Default;
    }
    let moved_count = locations.len();
    let first_line = if moving_up {
        insertion_start
    } else {
        moved_count - 1
    };
    let last_line = if moving_up {
        insertion_start + moved_count - 1
    } else {
        0
    };
    let target_after_retain = out[..target_bead]
        .iter()
        .filter(|bead| !bead.is_empty())
        .count();
    out.retain(|bead| !bead.is_empty());
    let first_row = out[..target_after_retain]
        .iter()
        .map(|bead| bead.source_lines.len().max(bead.target_lines.len()))
        .sum::<usize>();
    MoveBeadRowSpanResult {
        beads: out,
        selection: Some(BeadRowSelection {
            anchor_row: first_row + first_line,
            focus_row: first_row + last_line,
        }),
    }
}

pub fn set_bead_status(
    beads: &[MutableBead],
    indexes: &[usize],
    status: BeadStatus,
) -> Vec<MutableBead> {
    let mut out = beads.to_vec();
    for &index in indexes {
        if let Some(bead) = out.get_mut(index) {
            bead.status = status;
        }
    }
    out
}

/// Select the first visual row of the bead after a completed status operation.
///
/// Java `AlignPanelController.setStatus` advances from the last selected row
/// to `nextBeadFromRow`, preserving the selected source/target column.
pub fn selection_after_bead_status(
    beads: &[MutableBead],
    indexes: &[usize],
) -> Option<BeadRowSelection> {
    let next_bead = indexes.iter().copied().max()?.checked_add(1)?;
    if next_bead >= beads.len() {
        return None;
    }
    let next_row = beads[..next_bead]
        .iter()
        .map(|bead| bead.source_lines.len().max(bead.target_lines.len()))
        .sum();
    Some(BeadRowSelection {
        anchor_row: next_row,
        focus_row: next_row,
    })
}

pub fn set_beads_enabled(
    beads: &[MutableBead],
    indexes: Option<&[usize]>,
    enabled: bool,
) -> Vec<MutableBead> {
    let mut out = beads.to_vec();
    if let Some(indexes) = indexes {
        for &index in indexes {
            if let Some(bead) = out.get_mut(index) {
                bead.enabled = enabled;
            }
        }
    } else {
        for bead in &mut out {
            bead.enabled = enabled;
        }
    }
    out
}

/// Toggle every selected bead once, preserving mixed selections just like
/// Java `BeadTableModel.toggleBeadsAtRows`.
pub fn toggle_beads_enabled(beads: &[MutableBead], indexes: &[usize]) -> Vec<MutableBead> {
    let mut out = beads.to_vec();
    let mut unique = indexes.to_vec();
    unique.sort_unstable();
    unique.dedup();
    for index in unique {
        if let Some(bead) = out.get_mut(index) {
            bead.enabled = !bead.enabled;
        }
    }
    out
}

pub fn pinpoint_align(
    beads: &[MutableBead],
    first: (usize, AlignSide),
    second: (usize, AlignSide),
) -> Vec<MutableBead> {
    if first.0 == second.0
        || first.1 == second.1
        || matches!(first.1, AlignSide::Both)
        || matches!(second.1, AlignSide::Both)
    {
        return beads.to_vec();
    }
    let (low, high, relocate) = if first.0 < second.0 {
        (first.0, second.0, first.1)
    } else {
        (second.0, first.0, second.1)
    };
    if high >= beads.len() {
        return beads.to_vec();
    }
    let mut out = beads.to_vec();
    let mut relocated = Vec::new();
    for bead in &mut out[low..=high] {
        let lines = match relocate {
            AlignSide::Source => &mut bead.source_lines,
            AlignSide::Target => &mut bead.target_lines,
            AlignSide::Both => unreachable!(),
        };
        relocated.append(lines);
        bead.status = BeadStatus::Default;
    }
    match relocate {
        AlignSide::Source => out[high].source_lines = relocated,
        AlignSide::Target => out[high].target_lines = relocated,
        AlignSide::Both => unreachable!(),
    }
    out[high].status = BeadStatus::Accepted;
    out.retain(|bead| !bead.is_empty());
    out
}

/// Pinpoint-align exact visual rows instead of relocating whole endpoint beads.
pub fn pinpoint_align_rows(
    beads: &[MutableBead],
    first: (usize, AlignSide),
    second: (usize, AlignSide),
) -> Vec<MutableBead> {
    if first.0 == second.0
        || first.1 == second.1
        || matches!(first.1, AlignSide::Both)
        || matches!(second.1, AlignSide::Both)
    {
        return beads.to_vec();
    }
    let rows = bead_rows(beads);
    if first.0 >= rows.len() || second.0 >= rows.len() {
        return beads.to_vec();
    }
    let (low, high, relocate) = if first.0 < second.0 {
        (first.0, second.0, first.1)
    } else {
        (second.0, first.0, second.1)
    };
    let locations = line_locations(beads, low, high, relocate);
    if locations.is_empty() {
        return beads.to_vec();
    }
    let values: Vec<Option<String>> = locations
        .iter()
        .map(|&(bead_index, line_index)| {
            let lines = match relocate {
                AlignSide::Source => &beads[bead_index].source_lines,
                AlignSide::Target => &beads[bead_index].target_lines,
                AlignSide::Both => unreachable!(),
            };
            lines[line_index].clone()
        })
        .collect();
    let target_bead = rows[high].bead_index;
    let mut out = beads.to_vec();
    let mut touched: Vec<usize> = locations.iter().map(|(bead, _)| *bead).collect();
    for &(bead_index, line_index) in locations.iter().rev() {
        lines_mut(&mut out[bead_index], relocate).remove(line_index);
    }
    lines_mut(&mut out[target_bead], relocate).extend(values);
    touched.push(target_bead);
    touched.sort_unstable();
    touched.dedup();
    for bead_index in touched {
        out[bead_index].status = BeadStatus::Default;
    }
    out[target_bead].status = BeadStatus::Accepted;
    out.retain(|bead| !bead.is_empty());
    out
}

pub fn realign_pending(beads: &[MutableBead], algo: AlignAlgo) -> Result<Vec<MutableBead>> {
    fn flush(
        pending: &mut Vec<(String, String)>,
        output: &mut Vec<MutableBead>,
        algo: AlignAlgo,
    ) -> Result<()> {
        if pending.is_empty() {
            return Ok(());
        }
        output.extend(
            do_align(pending, Some(algo))?
                .into_iter()
                .map(|(source, target)| MutableBead::new(source, target)),
        );
        pending.clear();
        Ok(())
    }

    let mut output = Vec::new();
    let mut pending = Vec::new();
    for bead in beads {
        if bead.status == BeadStatus::Accepted {
            flush(&mut pending, &mut output, algo)?;
            output.push(bead.clone());
            continue;
        }
        let count = bead.source_lines.len().max(bead.target_lines.len());
        for index in 0..count {
            pending.push((
                bead.source_lines
                    .get(index)
                    .and_then(Option::as_deref)
                    .unwrap_or("")
                    .to_string(),
                bead.target_lines
                    .get(index)
                    .and_then(Option::as_deref)
                    .unwrap_or("")
                    .to_string(),
            ));
        }
    }
    flush(&mut pending, &mut output, algo)?;
    Ok(output)
}

pub fn extract_units(path: &Path) -> Result<Vec<AlignUnit>> {
    extract_units_cancellable(path, &CancellationToken::default())
}

pub fn extract_units_cancellable(
    path: &Path,
    cancellation: &CancellationToken,
) -> Result<Vec<AlignUnit>> {
    if cancellation.is_cancelled() {
        return Err(CoreError::Cancelled);
    }
    let reg = FilterRegistry::new();
    let ctx = FilterContext::default();
    if let Some(f) = reg.for_path(path) {
        let parsed = f.parse_cancellable(path, &ctx, &|| cancellation.is_cancelled())?;
        let units = parsed
            .segments
            .into_iter()
            .filter(|s| !s.source.trim().is_empty())
            .map(|s| AlignUnit {
                id: if s.id.is_empty() { None } else { Some(s.id) },
                text: s.source,
            })
            .collect();
        return if cancellation.is_cancelled() {
            Err(CoreError::Cancelled)
        } else {
            Ok(units)
        };
    }
    let raw =
        omegat_filters::read_to_string_cancellable(path, &|| cancellation.is_cancelled())?;
    let units = paragraphs(&raw)
        .into_iter()
        .map(|text| AlignUnit { id: None, text })
        .collect();
    if cancellation.is_cancelled() {
        Err(CoreError::Cancelled)
    } else {
        Ok(units)
    }
}

pub fn align_files(
    source: &Path,
    target: &Path,
    src_lang: &str,
    tgt_lang: &str,
) -> Result<ProjectTmx> {
    align_files_cfg(source, target, src_lang, tgt_lang, &AlignConfig::default())
}

pub fn align_files_cfg(
    source: &Path,
    target: &Path,
    src_lang: &str,
    tgt_lang: &str,
    cfg: &AlignConfig,
) -> Result<ProjectTmx> {
    align_files_cfg_cancellable(
        source,
        target,
        src_lang,
        tgt_lang,
        cfg,
        &CancellationToken::default(),
    )
}

pub fn align_files_cfg_cancellable(
    source: &Path,
    target: &Path,
    src_lang: &str,
    tgt_lang: &str,
    cfg: &AlignConfig,
    cancellation: &CancellationToken,
) -> Result<ProjectTmx> {
    let left = extract_units_cancellable(source, cancellation)?;
    let right = extract_units_cancellable(target, cancellation)?;
    let pairs = align_units_cancellable(&left, &right, src_lang, tgt_lang, cfg, cancellation)?;
    if cancellation.is_cancelled() {
        return Err(CoreError::Cancelled);
    }
    Ok(pairs_to_tmx(&pairs, src_lang, tgt_lang))
}

pub fn align_text(
    left: &str,
    right: &str,
    src_lang: &str,
    tgt_lang: &str,
    cfg: &AlignConfig,
) -> ProjectTmx {
    let ls = text_units(left, cfg.mode);
    let rs = text_units(right, cfg.mode);
    let pairs = align_units(&ls, &rs, src_lang, tgt_lang, cfg);
    pairs_to_tmx(&pairs, src_lang, tgt_lang)
}

pub fn align_units(
    left: &[AlignUnit],
    right: &[AlignUnit],
    src_lang: &str,
    tgt_lang: &str,
    cfg: &AlignConfig,
) -> Vec<(String, String)> {
    align_units_cancellable(
        left,
        right,
        src_lang,
        tgt_lang,
        cfg,
        &CancellationToken::default(),
    )
    .expect("default cancellation token cannot be cancelled")
}

pub fn align_units_cancellable(
    left: &[AlignUnit],
    right: &[AlignUnit],
    src_lang: &str,
    tgt_lang: &str,
    cfg: &AlignConfig,
    cancellation: &CancellationToken,
) -> Result<Vec<(String, String)>> {
    if cancellation.is_cancelled() {
        return Err(CoreError::Cancelled);
    }
    match cfg.mode {
        AlignMode::Id => Ok(align_by_id(left, right)),
        AlignMode::Parsewise => {
            if left.len() == right.len() && !left.is_empty() {
                let mut out = Vec::new();
                for (a, b) in left.iter().zip(right.iter()) {
                    if cancellation.is_cancelled() {
                        return Err(CoreError::Cancelled);
                    }
                    let ls = maybe_segment(&a.text, src_lang, cfg.segment);
                    let rs = maybe_segment(&b.text, tgt_lang, cfg.segment);
                    out.extend(hmm_align_cancellable(&ls, &rs, cfg, cancellation)?);
                }
                Ok(out)
            } else {
                let ls = flatten_segmented(left, src_lang, cfg.segment);
                let rs = flatten_segmented(right, tgt_lang, cfg.segment);
                hmm_align_cancellable(&ls, &rs, cfg, cancellation)
            }
        }
        AlignMode::Heapwise => {
            let ls = flatten_segmented(left, src_lang, cfg.segment);
            let rs = flatten_segmented(right, tgt_lang, cfg.segment);
            hmm_align_cancellable(&ls, &rs, cfg, cancellation)
        }
    }
}

pub fn edit_pairs(pairs: &[(String, String)], action: &str, index: usize) -> Vec<(String, String)> {
    edit_pairs_sided(pairs, action, index, AlignSide::Both)
}

/// Apply one manual-correction operation to either side of the bitext table.
///
/// Java's aligner moves/merges cells in the selected column, so a source-only
/// operation must not reorder or concatenate the corresponding target cell.
/// `Both` preserves the old whole-row behavior for existing RPC clients.
pub fn edit_pairs_sided(
    pairs: &[(String, String)],
    action: &str,
    index: usize,
    side: AlignSide,
) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = pairs.to_vec();
    if out.is_empty() {
        return out;
    }
    let i = index.min(out.len() - 1);
    match action {
        "merge" if i + 1 < out.len() => {
            let (s2, t2) = out.remove(i + 1);
            match side {
                AlignSide::Both => {
                    out[i].0 = join_bitext(&out[i].0, &s2);
                    out[i].1 = join_bitext(&out[i].1, &t2);
                }
                AlignSide::Source => {
                    out[i].0 = join_bitext(&out[i].0, &s2);
                    out.insert(i + 1, (String::new(), t2));
                }
                AlignSide::Target => {
                    out[i].1 = join_bitext(&out[i].1, &t2);
                    out.insert(i + 1, (s2, String::new()));
                }
            }
        }
        "split" => {
            let (s, t) = out[i].clone();
            match side {
                AlignSide::Both => {
                    let (s1, s2) = split_once(&s);
                    let (t1, t2) = split_once(&t);
                    out[i] = (s1, t1);
                    out.insert(i + 1, (s2, t2));
                }
                AlignSide::Source => {
                    let (s1, s2) = split_once(&s);
                    if !s2.is_empty() {
                        out[i].0 = s1;
                        out.insert(i + 1, (s2, String::new()));
                    }
                }
                AlignSide::Target => {
                    let (t1, t2) = split_once(&t);
                    if !t2.is_empty() {
                        out[i].1 = t1;
                        out.insert(i + 1, (String::new(), t2));
                    }
                }
            }
        }
        "up" if i > 0 => move_side(&mut out, i, i - 1, side),
        "down" if i + 1 < out.len() => move_side(&mut out, i, i + 1, side),
        _ => {}
    }
    out
}

fn move_side(pairs: &mut [(String, String)], from: usize, to: usize, side: AlignSide) {
    match side {
        AlignSide::Both => pairs.swap(from, to),
        AlignSide::Source => {
            let source = pairs[from].0.clone();
            pairs[from].0 = pairs[to].0.clone();
            pairs[to].0 = source;
        }
        AlignSide::Target => {
            let target = pairs[from].1.clone();
            pairs[from].1 = pairs[to].1.clone();
            pairs[to].1 = target;
        }
    }
}

pub fn write_aligned_pairs(
    pairs: &[(String, String)],
    dest: &Path,
    src_lang: &str,
    tgt_lang: &str,
) -> Result<()> {
    let tmx = pairs_to_tmx(pairs, src_lang, tgt_lang);
    write_aligned_tmx(&tmx, dest, src_lang, tgt_lang)
}

pub fn write_aligned_tmx(
    tmx: &ProjectTmx,
    dest: &Path,
    src_lang: &str,
    tgt_lang: &str,
) -> Result<()> {
    if src_lang.trim().is_empty() || tgt_lang.trim().is_empty() {
        return Err(crate::error::CoreError::InvalidProject(
            "IllegalStateException: aligner languages are not set".into(),
        ));
    }
    tmx.write(dest, src_lang, tgt_lang)
}

pub fn write_aligned_tmx_cancellable(
    tmx: &ProjectTmx,
    dest: &Path,
    src_lang: &str,
    tgt_lang: &str,
    cancellation: &CancellationToken,
) -> Result<()> {
    if src_lang.trim().is_empty() || tgt_lang.trim().is_empty() {
        return Err(crate::error::CoreError::InvalidProject(
            "IllegalStateException: aligner languages are not set".into(),
        ));
    }
    if cancellation.is_cancelled() {
        return Err(CoreError::Cancelled);
    }
    let id = ALIGN_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("align.tmx");
    let staged = dest.with_file_name(format!(
        ".{name}.omegat-align-staged-{}-{id}",
        std::process::id()
    ));
    let backup = dest.with_file_name(format!(
        ".{name}.omegat-align-backup-{}-{id}",
        std::process::id()
    ));
    tmx.write(&staged, src_lang, tgt_lang)?;
    if cancellation.checkpoint("align.write") {
        let _ = std::fs::remove_file(staged);
        return Err(CoreError::Cancelled);
    }
    let had_destination = dest.exists();
    if had_destination {
        if let Err(error) = std::fs::rename(dest, &backup) {
            let _ = std::fs::remove_file(staged);
            return Err(error.into());
        }
    }
    if let Err(error) = std::fs::rename(&staged, dest) {
        if had_destination {
            let _ = std::fs::rename(&backup, dest);
        }
        let _ = std::fs::remove_file(staged);
        return Err(error.into());
    }
    if had_destination {
        let _ = std::fs::remove_file(backup);
    }
    Ok(())
}

pub fn do_align(
    beads: &[(String, String)],
    algo: Option<AlignAlgo>,
) -> Result<Vec<(String, String)>> {
    let Some(algo) = algo else {
        return Err(crate::error::CoreError::InvalidProject(
            "IllegalStateException: required aligner settings are not set".into(),
        ));
    };
    let source: Vec<String> = beads
        .iter()
        .map(|(source, _)| source.clone())
        .filter(|source| !source.is_empty())
        .collect();
    let target: Vec<String> = beads
        .iter()
        .map(|(_, target)| target.clone())
        .filter(|target| !target.is_empty())
        .collect();
    let cfg = AlignConfig {
        mode: AlignMode::Heapwise,
        algo,
        segment: false,
        ..AlignConfig::default()
    };
    Ok(hmm_align(&source, &target, &cfg))
}

fn pairs_to_tmx(pairs: &[(String, String)], src_lang: &str, tgt_lang: &str) -> ProjectTmx {
    let _ = (src_lang, tgt_lang);
    let mut tmx = ProjectTmx::new();
    for (a, b) in pairs {
        if a.trim().is_empty() && b.trim().is_empty() {
            continue;
        }
        tmx.insert(TmxEntry {
            source: a.clone(),
            translation: b.clone(),
            default_translation: true,
            creator: Some("OmegaT Aligner".into()),
            ..Default::default()
        });
    }
    tmx
}

fn text_units(text: &str, mode: AlignMode) -> Vec<AlignUnit> {
    match mode {
        AlignMode::Id => text
            .lines()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| AlignUnit {
                id: Some(s.to_string()),
                text: s.to_string(),
            })
            .collect(),
        AlignMode::Parsewise | AlignMode::Heapwise => paragraphs(text)
            .into_iter()
            .map(|text| AlignUnit { id: None, text })
            .collect(),
    }
}

fn paragraphs(text: &str) -> Vec<String> {
    text.replace("\r\n", "\n")
        .split("\n\n")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn maybe_segment(text: &str, lang: &str, segment: bool) -> Vec<String> {
    if !segment {
        let t = text.trim();
        return if t.is_empty() {
            vec![]
        } else {
            vec![t.to_string()]
        };
    }
    split_sentences_lang(text, true, lang, None)
}

fn flatten_segmented(units: &[AlignUnit], lang: &str, segment: bool) -> Vec<String> {
    units
        .iter()
        .flat_map(|u| maybe_segment(&u.text, lang, segment))
        .collect()
}

fn align_by_id(left: &[AlignUnit], right: &[AlignUnit]) -> Vec<(String, String)> {
    if !has_real_ids(left) || !has_real_ids(right) {
        return left
            .iter()
            .zip(right.iter())
            .map(|(a, b)| (a.text.clone(), b.text.clone()))
            .collect();
    }
    let map: HashMap<&str, &str> = right
        .iter()
        .filter_map(|u| u.id.as_deref().map(|id| (id, u.text.as_str())))
        .collect();
    let mapped: Vec<(String, String)> = left
        .iter()
        .filter_map(|u| {
            let id = u.id.as_deref()?;
            let tgt = map.get(id)?;
            Some((u.text.clone(), (*tgt).to_string()))
        })
        .collect();
    if mapped.is_empty() {
        return left
            .iter()
            .zip(right.iter())
            .map(|(a, b)| (a.text.clone(), b.text.clone()))
            .collect();
    }
    mapped
}

fn has_real_ids(units: &[AlignUnit]) -> bool {
    units.iter().any(|u| {
        u.id.as_deref()
            .map(|id| id.chars().any(|c| c.is_ascii_alphabetic()))
            .unwrap_or(false)
    })
}

fn hmm_align(ls: &[String], rs: &[String], cfg: &AlignConfig) -> Vec<(String, String)> {
    hmm_align_cancellable(ls, rs, cfg, &CancellationToken::default())
        .expect("default cancellation token cannot be cancelled")
}

fn hmm_align_cancellable(
    ls: &[String],
    rs: &[String],
    cfg: &AlignConfig,
    cancellation: &CancellationToken,
) -> Result<Vec<(String, String)>> {
    if cancellation.is_cancelled() {
        return Err(CoreError::Cancelled);
    }
    if ls.is_empty() && rs.is_empty() {
        return Ok(vec![]);
    }
    if ls.is_empty() {
        return Ok(rs.iter().cloned().map(|t| (String::new(), t)).collect());
    }
    if rs.is_empty() {
        return Ok(ls.iter().cloned().map(|s| (s, String::new())).collect());
    }
    match cfg.algo {
        AlignAlgo::Viterbi => decode_min_cancellable(ls, rs, cfg, 0.2, cancellation),
        AlignAlgo::ForwardBackward => decode_posterior_cancellable(ls, rs, cfg, cancellation),
    }
}

/// Gale–Church / mALIGNa length cost. CHAR = `CharCounter`, WORD = `SplitCounter`.
fn len_of(s: &str, c: Counter) -> f64 {
    match c {
        Counter::Char => s.chars().count() as f64,
        Counter::Word => s.split_whitespace().count().max(1) as f64,
    }
}

fn length_cost(a: &str, b: &str, cfg: &AlignConfig) -> f64 {
    let la = len_of(a, cfg.counter);
    let lb = len_of(b, cfg.counter);
    match cfg.calculator {
        CalculatorType::Normal => {
            // NormalDistributionCalculator: squared length difference over variance.
            let mean = (la + lb) / 2.0;
            let var = (mean * 6.8).max(1.0);
            (la - lb).powi(2) / (2.0 * var)
        }
        CalculatorType::Poisson => {
            let lambda = (la + 1.0).max(0.5);
            poisson_nll(lb.round().max(0.0) as u32, lambda)
        }
    }
}

fn poisson_nll(k: u32, lambda: f64) -> f64 {
    let mut ln_fact = 0.0;
    for i in 2..=k {
        ln_fact += (i as f64).ln();
    }
    lambda - (k as f64) * lambda.ln() + ln_fact
}

fn join_units(xs: &[String]) -> String {
    xs.join(" ")
}

fn join_bitext(a: &str, b: &str) -> String {
    match (a.is_empty(), b.is_empty()) {
        (true, _) => b.to_string(),
        (_, true) => a.to_string(),
        _ => format!("{a} {b}"),
    }
}

fn split_once(s: &str) -> (String, String) {
    if let Some(idx) = s.rfind(' ') {
        (s[..idx].to_string(), s[idx + 1..].to_string())
    } else if s.chars().count() > 1 {
        let mid = s.chars().count() / 2;
        let (a, b): (String, String) =
            s.chars()
                .enumerate()
                .fold((String::new(), String::new()), |(mut a, mut b), (i, c)| {
                    if i < mid {
                        a.push(c);
                    } else {
                        b.push(c);
                    }
                    (a, b)
                });
        (a, b)
    } else {
        (s.to_string(), String::new())
    }
}

/// Min-cost Viterbi over 1-1 / 2-1 / 1-2 (Java `ViterbiAlgorithm`).
fn viterbi(ls: &[String], rs: &[String], cfg: &AlignConfig) -> Vec<(String, String)> {
    decode_min(ls, rs, cfg, 0.2)
}

/// Forward–backward posterior path (Java `ForwardBackwardAlgorithm`).
/// Uses summed path mass instead of min cost, and a higher merge penalty so the
/// recovered alignment is not a Viterbi alias.
fn forward_backward(ls: &[String], rs: &[String], cfg: &AlignConfig) -> Vec<(String, String)> {
    decode_posterior(ls, rs, cfg)
}

fn decode_min(
    ls: &[String],
    rs: &[String],
    cfg: &AlignConfig,
    merge_pen: f64,
) -> Vec<(String, String)> {
    decode_min_cancellable(
        ls,
        rs,
        cfg,
        merge_pen,
        &CancellationToken::default(),
    )
    .expect("default cancellation token cannot be cancelled")
}

fn decode_min_cancellable(
    ls: &[String],
    rs: &[String],
    cfg: &AlignConfig,
    merge_pen: f64,
    cancellation: &CancellationToken,
) -> Result<Vec<(String, String)>> {
    let n = ls.len();
    let m = rs.len();
    let mut dp = vec![vec![f64::INFINITY; m + 1]; n + 1];
    let mut bt = vec![vec![(0isize, 0isize); m + 1]; n + 1];
    dp[0][0] = 0.0;
    for i in 0..=n {
        if cancellation.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        for j in 0..=m {
            let cur = dp[i][j];
            if !cur.is_finite() {
                continue;
            }
            consider_min(&mut dp, &mut bt, i, j, n, m, cur, 1, 1, 0.0, ls, rs, cfg);
            consider_min(
                &mut dp, &mut bt, i, j, n, m, cur, 2, 1, merge_pen, ls, rs, cfg,
            );
            consider_min(
                &mut dp, &mut bt, i, j, n, m, cur, 1, 2, merge_pen, ls, rs, cfg,
            );
        }
        if cancellation.checkpoint("align.decode") {
            return Err(CoreError::Cancelled);
        }
    }
    if cancellation.is_cancelled() {
        Err(CoreError::Cancelled)
    } else {
        Ok(backtrack(ls, rs, &bt, n, m))
    }
}

fn consider_min(
    dp: &mut [Vec<f64>],
    bt: &mut [Vec<(isize, isize)>],
    i: usize,
    j: usize,
    n: usize,
    m: usize,
    cur: f64,
    di: usize,
    dj: usize,
    extra: f64,
    ls: &[String],
    rs: &[String],
    cfg: &AlignConfig,
) {
    if i + di > n || j + dj > m {
        return;
    }
    let src = join_units(&ls[i..i + di]);
    let tgt = join_units(&rs[j..j + dj]);
    let cost = cur + length_cost(&src, &tgt, cfg) + extra;
    if cost < dp[i + di][j + dj] {
        dp[i + di][j + dj] = cost;
        bt[i + di][j + dj] = (di as isize, dj as isize);
    }
}

fn decode_posterior(ls: &[String], rs: &[String], cfg: &AlignConfig) -> Vec<(String, String)> {
    decode_posterior_cancellable(
        ls,
        rs,
        cfg,
        &CancellationToken::default(),
    )
    .expect("default cancellation token cannot be cancelled")
}

fn decode_posterior_cancellable(
    ls: &[String],
    rs: &[String],
    cfg: &AlignConfig,
    cancellation: &CancellationToken,
) -> Result<Vec<(String, String)>> {
    let n = ls.len();
    let m = rs.len();
    // Soft path: unmatched 1-0 / 0-1 are cheap; 2-1 / 1-2 are expensive.
    // Viterbi never takes 1-0, so the recovered beads differ on uneven heaps.
    let trans = [
        (1, 1, 0.0),
        (2, 1, 2.2),
        (1, 2, 2.2),
        (1, 0, 0.15),
        (0, 1, 0.15),
    ];
    let mut fwd = vec![vec![0.0_f64; m + 1]; n + 1];
    let mut bt = vec![vec![(0isize, 0isize); m + 1]; n + 1];
    fwd[0][0] = 1.0;
    for i in 0..=n {
        if cancellation.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        for j in 0..=m {
            let mass = fwd[i][j];
            if mass <= 0.0 {
                continue;
            }
            for (di, dj, extra) in trans {
                if i + di > n || j + dj > m {
                    continue;
                }
                let src = if di == 0 {
                    String::new()
                } else {
                    join_units(&ls[i..i + di])
                };
                let tgt = if dj == 0 {
                    String::new()
                } else {
                    join_units(&rs[j..j + dj])
                };
                let w = (-(length_cost(&src, &tgt, cfg) + extra)).exp() * mass;
                if w > fwd[i + di][j + dj] {
                    fwd[i + di][j + dj] = w;
                    bt[i + di][j + dj] = (di as isize, dj as isize);
                } else if (w - fwd[i + di][j + dj]).abs() < 1e-12 && w > 0.0 {
                    // posterior tie: prefer fewer merges (soft alignment)
                    let (pdi, pdj) = bt[i + di][j + dj];
                    if di + dj < (pdi + pdj) as usize {
                        bt[i + di][j + dj] = (di as isize, dj as isize);
                    }
                }
            }
        }
        if cancellation.checkpoint("align.decode") {
            return Err(CoreError::Cancelled);
        }
    }
    if cancellation.is_cancelled() {
        Err(CoreError::Cancelled)
    } else {
        Ok(backtrack(ls, rs, &bt, n, m))
    }
}

fn backtrack(
    ls: &[String],
    rs: &[String],
    bt: &[Vec<(isize, isize)>],
    mut i: usize,
    mut j: usize,
) -> Vec<(String, String)> {
    let mut rev = Vec::new();
    while i > 0 || j > 0 {
        let (di, dj) = bt[i][j];
        if di == 0 && dj == 0 {
            if i > 0 {
                i -= 1;
            } else if j > 0 {
                j -= 1;
            } else {
                break;
            }
            continue;
        }
        let src = if di >= 1 {
            join_units(&ls[i - di as usize..i])
        } else {
            String::new()
        };
        let tgt = if dj >= 1 {
            join_units(&rs[j - dj as usize..j])
        } else {
            String::new()
        };
        rev.push((src, tgt));
        i -= di as usize;
        j -= dj as usize;
    }
    rev.reverse();
    rev
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/align")
            .join(name)
    }

    fn heap_cfg() -> AlignConfig {
        AlignConfig {
            mode: AlignMode::Heapwise,
            algo: AlignAlgo::Viterbi,
            counter: Counter::Char,
            calculator: CalculatorType::Normal,
            segment: true,
        }
    }

    fn load_align_golden(name: &str) -> serde_json::Value {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/goldens/align")
            .join(name);
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn pairs_from_tmx(tmx: &ProjectTmx) -> Vec<(String, String)> {
        tmx.entries
            .iter()
            .map(|e| (e.source.clone(), e.translation.clone()))
            .collect()
    }

    fn expected_pairs(v: &serde_json::Value) -> Vec<(String, String)> {
        v["pairs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                (
                    p[0].as_str().unwrap().to_string(),
                    p[1].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn heapwise_matches_java_aligner_fixture() {
        let g = load_align_golden("AlignerTest#testAlignerHeapMode.json");
        assert_eq!(
            g["java_test"].as_str().unwrap(),
            "org.omegat.gui.align.AlignerTest#testAlignerHeapMode"
        );
        let tmx = align_files_cfg(
            &fixture("heapSource.txt"),
            &fixture("heapTarget.txt"),
            "en",
            "ja",
            &heap_cfg(),
        )
        .unwrap();
        assert_eq!(pairs_from_tmx(&tmx), expected_pairs(&g));
    }

    #[test]
    fn parsewise_matches_java_aligner_fixture() {
        let cfg = AlignConfig {
            mode: AlignMode::Parsewise,
            ..heap_cfg()
        };
        let tmx = align_files_cfg(
            &fixture("parseSource.txt"),
            &fixture("parseTarget.txt"),
            "en",
            "ja",
            &cfg,
        )
        .unwrap();
        let g = load_align_golden("AlignerTest#testAlignerParseMode.json");
        assert_eq!(
            g["java_test"].as_str().unwrap(),
            "org.omegat.gui.align.AlignerTest#testAlignerParseMode"
        );
        assert_eq!(pairs_from_tmx(&tmx), expected_pairs(&g));
    }

    #[test]
    fn id_mode_pairs_properties_keys() {
        let cfg = AlignConfig {
            mode: AlignMode::Id,
            ..heap_cfg()
        };
        let tmx = align_files_cfg(
            &fixture("idSource.properties"),
            &fixture("idTarget.properties"),
            "en",
            "ja",
            &cfg,
        )
        .unwrap();
        let g = load_align_golden("AlignerTest#testAlignerIDMode.json");
        assert_eq!(
            g["java_test"].as_str().unwrap(),
            "org.omegat.gui.align.AlignerTest#testAlignerIDMode"
        );
        assert_eq!(pairs_from_tmx(&tmx), expected_pairs(&g));
        assert!(tmx.entries.iter().all(|e| e.source != "No one knows."));
    }

    #[test]
    fn heapwise_is_not_whitespace_split() {
        let cfg = heap_cfg();
        let tmx = align_text(
            "Hello world. Second.",
            "Bonjour monde. Deux.",
            "en",
            "fr",
            &cfg,
        );
        assert!(tmx.entries.iter().any(|e| e.source.contains(' ')));
        assert!(tmx.entries.len() >= 1);
        assert!(tmx.entries.len() <= 3);
    }

    #[test]
    fn viterbi_and_forward_backward_are_different_algorithms() {
        let ls = vec!["ab".into(), "cd".into(), "efghijklmnop".into()];
        let rs = vec!["ab cd".into(), "efghijklmnop".into()];
        let vcfg = AlignConfig {
            mode: AlignMode::Heapwise,
            algo: AlignAlgo::Viterbi,
            counter: Counter::Char,
            calculator: CalculatorType::Normal,
            segment: false,
        };
        let fcfg = AlignConfig {
            algo: AlignAlgo::ForwardBackward,
            ..vcfg.clone()
        };
        let v = hmm_align(&ls, &rs, &vcfg);
        let f = hmm_align(&ls, &rs, &fcfg);
        assert_ne!(
            v, f,
            "Forward-Backward must not be a Viterbi alias: v={v:?} f={f:?}"
        );
    }

    #[test]
    fn poisson_and_normal_costs_differ() {
        let mut n = AlignConfig::default();
        n.calculator = CalculatorType::Normal;
        n.counter = Counter::Char;
        let mut p = n.clone();
        p.calculator = CalculatorType::Poisson;
        let cn = length_cost("short", "a very long target string", &n);
        let cp = length_cost("short", "a very long target string", &p);
        assert_ne!(cn, cp);
    }

    #[test]
    fn edit_merge_split_move() {
        let pairs = vec![
            ("a".into(), "A".into()),
            ("b".into(), "B".into()),
            ("c".into(), "C".into()),
        ];
        let merged = edit_pairs(&pairs, "merge", 0);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0], ("a b".into(), "A B".into()));
        let split = edit_pairs(&merged, "split", 0);
        assert_eq!(split.len(), 3);
        let down = edit_pairs(&pairs, "down", 0);
        assert_eq!(down[0].0, "b");
        let up = edit_pairs(&pairs, "up", 1);
        assert_eq!(up[0].0, "b");
    }

    #[test]
    fn manual_edits_respect_the_selected_bitext_side() {
        let pairs = vec![
            ("one".into(), "un".into()),
            ("two".into(), "deux".into()),
            ("three four".into(), "trois quatre".into()),
        ];
        assert_eq!(
            edit_pairs_sided(&pairs, "merge", 0, AlignSide::Source),
            vec![
                ("one two".into(), "un".into()),
                (String::new(), "deux".into()),
                ("three four".into(), "trois quatre".into()),
            ]
        );
        assert_eq!(
            edit_pairs_sided(&pairs, "down", 0, AlignSide::Target),
            vec![
                ("one".into(), "deux".into()),
                ("two".into(), "un".into()),
                ("three four".into(), "trois quatre".into()),
            ]
        );
        assert_eq!(
            edit_pairs_sided(&pairs, "split", 2, AlignSide::Source),
            vec![
                ("one".into(), "un".into()),
                ("two".into(), "deux".into()),
                ("three".into(), "trois quatre".into()),
                ("four".into(), String::new()),
            ]
        );
    }

    #[test]
    fn mutable_bead_matches_java_state_and_language_joining() {
        let equal = MutableBead::new("同文", "同文");
        assert_eq!(equal.enabled, false);
        assert_eq!(equal.status, BeadStatus::Accepted);
        assert_eq!(equal.is_balanced(), true);

        let mut empty = MutableBead::empty();
        assert_eq!(empty.enabled, true);
        assert_eq!(empty.status, BeadStatus::Default);
        assert_eq!(empty.is_empty(), true);
        empty.source_lines = vec![Some("一".into()), Some("二".into())];
        empty.target_lines = vec![Some("one".into()), None, Some("two".into())];
        assert_eq!(empty.source_text("ja"), "一二");
        assert_eq!(empty.target_text("en"), "one null two");
        assert_eq!(empty.is_balanced(), false);
    }

    #[test]
    fn mutable_bead_review_split_pinpoint_and_output_are_stateful() {
        let beads = vec![
            MutableBead::new("one", "un"),
            MutableBead::from_lines(
                1.5,
                vec![Some("two words".into())],
                vec![Some("deux".into()), Some("mots".into())],
            ),
            MutableBead::new("three", "trois"),
        ];
        let reviewed = set_bead_status(&beads, &[1], BeadStatus::NeedsReview);
        assert_eq!(reviewed[1].status, BeadStatus::NeedsReview);
        assert_eq!(
            selection_after_bead_status(&reviewed, &[0, 1, 1]),
            Some(BeadRowSelection {
                anchor_row: 3,
                focus_row: 3,
            })
        );
        assert_eq!(selection_after_bead_status(&reviewed, &[2]), None);
        let mixed = set_beads_enabled(&reviewed, Some(&[1]), false);
        let toggled = toggle_beads_enabled(&mixed, &[0, 1, 1]);
        assert_eq!(
            toggled.iter().map(|bead| bead.enabled).collect::<Vec<_>>(),
            vec![false, true, true]
        );
        let split = split_bead_line(
            &reviewed,
            1,
            AlignSide::Source,
            0,
            &["two".into(), "words".into()],
        );
        assert_eq!(
            split[1].source_lines,
            vec![Some("two".into()), Some("words".into())]
        );
        assert_eq!(split[1].status, BeadStatus::Default);

        let pinpointed = pinpoint_align(&split, (0, AlignSide::Source), (2, AlignSide::Target));
        assert_eq!(pinpointed.len(), 3);
        assert_eq!(pinpointed[0].source_lines, Vec::<Option<String>>::new());
        assert_eq!(pinpointed[0].target_lines, vec![Some("un".into())]);
        assert_eq!(pinpointed[1].source_lines, Vec::<Option<String>>::new());
        assert_eq!(
            pinpointed[2].source_lines,
            vec![
                Some("one".into()),
                Some("two".into()),
                Some("words".into()),
                Some("three".into())
            ]
        );
        assert_eq!(pinpointed[2].target_lines, vec![Some("trois".into())]);
        assert_eq!(pinpointed[2].status, BeadStatus::Accepted);

        let disabled = set_beads_enabled(&pinpointed, None, false);
        assert_eq!(beads_to_pairs(&disabled, "en", "fr"), Vec::new());
        let enabled = set_beads_enabled(&disabled, None, true);
        assert_eq!(
            beads_to_pairs(&enabled, "en", "fr"),
            vec![
                (String::new(), "un".into()),
                (String::new(), "deux mots".into()),
                ("one two words three".into(), "trois".into())
            ]
        );
    }

    #[test]
    fn mutable_bead_visual_row_spans_edit_exact_lines() {
        let beads = vec![
            MutableBead::from_lines(
                1.0,
                vec![Some("s0a".into()), Some("s0b".into())],
                vec![Some("t0".into())],
            ),
            MutableBead::from_lines(
                2.0,
                vec![Some("s1".into())],
                vec![Some("t1a".into()), Some("t1b".into())],
            ),
            MutableBead::new("s2", "t2"),
        ];
        assert_eq!(
            bead_rows(&beads),
            vec![
                BeadRow {
                    bead_index: 0,
                    row_in_bead: 0,
                    source_line_index: Some(0),
                    target_line_index: Some(0),
                },
                BeadRow {
                    bead_index: 0,
                    row_in_bead: 1,
                    source_line_index: Some(1),
                    target_line_index: None,
                },
                BeadRow {
                    bead_index: 1,
                    row_in_bead: 0,
                    source_line_index: Some(0),
                    target_line_index: Some(0),
                },
                BeadRow {
                    bead_index: 1,
                    row_in_bead: 1,
                    source_line_index: None,
                    target_line_index: Some(1),
                },
                BeadRow {
                    bead_index: 2,
                    row_in_bead: 0,
                    source_line_index: Some(0),
                    target_line_index: Some(0),
                },
            ]
        );

        let merged = merge_bead_row_span(&beads, 1, 3, AlignSide::Source, "en");
        assert_eq!(
            merged[0].source_lines,
            vec![Some("s0a".into()), Some("s0b s1".into())]
        );
        assert_eq!(merged[1].source_lines, Vec::<Option<String>>::new());
        assert_eq!(
            merged[1].target_lines,
            vec![Some("t1a".into()), Some("t1b".into())]
        );

        let replaced = replace_bead_row_span(
            &beads,
            0,
            3,
            AlignSide::Target,
            vec![Some("left".into()), Some("right".into())],
        );
        assert_eq!(
            replaced[0].target_lines,
            vec![Some("left".into()), Some("right".into())]
        );
        assert_eq!(replaced[1].target_lines, Vec::<Option<String>>::new());
        assert_eq!(replaced[1].source_lines, vec![Some("s1".into())]);

        let moved = move_bead_row_span(&beads, 1, 2, AlignSide::Source, 1);
        assert_eq!(moved[0].source_lines, vec![Some("s0a".into())]);
        assert_eq!(moved[1].source_lines, Vec::<Option<String>>::new());
        assert_eq!(
            moved[2].source_lines,
            vec![Some("s0b".into()), Some("s1".into()), Some("s2".into())]
        );

        let pinpointed =
            pinpoint_align_rows(&beads, (1, AlignSide::Source), (3, AlignSide::Target));
        assert_eq!(pinpointed[0].source_lines, vec![Some("s0a".into())]);
        assert_eq!(
            pinpointed[1].source_lines,
            vec![Some("s0b".into()), Some("s1".into())]
        );
        assert_eq!(pinpointed[1].status, BeadStatus::Accepted);
        assert_eq!(pinpointed[2].source_lines, vec![Some("s2".into())]);
    }

    #[test]
    fn table_drop_moves_only_java_eligible_real_cells_to_target_bead() {
        let mut first = MutableBead::from_lines(
            1.0,
            vec![Some("a".into()), Some("b".into())],
            vec![Some("A".into())],
        );
        first.status = BeadStatus::Accepted;
        let mut second = MutableBead::from_lines(
            2.0,
            vec![Some("c".into())],
            vec![Some("C".into()), Some("D".into())],
        );
        second.status = BeadStatus::NeedsReview;
        let beads = vec![first, second, MutableBead::new("e", "E")];

        assert_eq!(
            can_move_bead_row_span_to(&beads, 1, 2, AlignSide::Source, 4),
            true
        );
        assert_eq!(
            can_move_bead_row_span_to(&beads, 0, 0, AlignSide::Source, 1),
            false,
            "the drop target is in the same bead"
        );
        assert_eq!(
            can_move_bead_row_span_to(&beads, 0, 0, AlignSide::Source, 4),
            false,
            "a non-edge line cannot cross a bead"
        );
        assert_eq!(
            can_move_bead_row_span_to(&beads, 3, 3, AlignSide::Source, 4),
            false,
            "a nullable visual cell is not transferable"
        );

        let moved_result = move_bead_row_span_to_with_selection(&beads, 1, 2, AlignSide::Source, 4);
        assert_eq!(
            moved_result.selection,
            Some(BeadRowSelection {
                anchor_row: 4,
                focus_row: 3,
            }),
            "the JTable lead follows the original first/last line identities"
        );
        let moved = moved_result.beads;
        assert_eq!(moved[0].source_lines, vec![Some("a".into())]);
        assert_eq!(moved[0].target_lines, vec![Some("A".into())]);
        assert_eq!(moved[1].source_lines, Vec::<Option<String>>::new());
        assert_eq!(
            moved[1].target_lines,
            vec![Some("C".into()), Some("D".into())]
        );
        assert_eq!(
            moved[2].source_lines,
            vec![Some("c".into()), Some("b".into()), Some("e".into())],
            "downward Java drops repeatedly insert at index zero"
        );
        assert_eq!(
            moved.iter().map(|bead| bead.status).collect::<Vec<_>>(),
            vec![
                BeadStatus::Default,
                BeadStatus::Default,
                BeadStatus::Default
            ]
        );

        assert_eq!(
            can_move_bead_row_span_to(&beads, 2, 2, AlignSide::Target, 0),
            true
        );
        let upward_result =
            move_bead_row_span_to_with_selection(&beads, 2, 2, AlignSide::Target, 0);
        assert_eq!(
            upward_result.selection,
            Some(BeadRowSelection {
                anchor_row: 1,
                focus_row: 1,
            })
        );
        let upward = upward_result.beads;
        assert_eq!(
            upward[0].target_lines,
            vec![Some("A".into()), Some("C".into())]
        );
        assert_eq!(upward[1].target_lines, vec![Some("D".into())]);

        let new_top_result =
            move_bead_row_span_to_with_selection(&beads, 0, 0, AlignSide::Source, -1);
        assert_eq!(
            new_top_result.selection,
            Some(BeadRowSelection {
                anchor_row: 0,
                focus_row: 0,
            })
        );
        let new_top = new_top_result.beads;
        assert_eq!(new_top[0].source_lines, vec![Some("a".into())]);
        assert_eq!(new_top[0].target_lines, Vec::<Option<String>>::new());
        assert_eq!(new_top[1].source_lines, vec![Some("b".into())]);
        assert_eq!(new_top[1].target_lines, vec![Some("A".into())]);

        let new_bottom = move_bead_row_span_to_with_selection(
            &beads,
            4,
            4,
            AlignSide::Target,
            bead_rows(&beads).len() as isize,
        );
        assert_eq!(
            new_bottom.selection,
            Some(BeadRowSelection {
                anchor_row: 5,
                focus_row: 5,
            })
        );
        assert_eq!(
            new_bottom.beads.last().unwrap().target_lines,
            vec![Some("E".into())]
        );
    }

    #[test]
    fn accepted_beads_are_barriers_when_realigning_pending() {
        let mut accepted = MutableBead::new("fixed", "fixe");
        accepted.status = BeadStatus::Accepted;
        let beads = vec![
            MutableBead::new("a", "A"),
            accepted.clone(),
            MutableBead::new("bb", "BB"),
        ];
        let result = realign_pending(&beads, AlignAlgo::Viterbi).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result[1], accepted);
        assert_eq!(result[0].source_text("en"), "a");
        assert_eq!(result[2].target_text("en"), "BB");
    }

    #[test]
    fn manual_pair_writeback_uses_the_product_tmx_writer() {
        let pairs = vec![
            ("Hello".into(), "Bonjour".into()),
            ("Bye".into(), "Au revoir".into()),
        ];
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("manual.tmx");
        write_aligned_pairs(&pairs, &dest, "en", "fr").unwrap();
        let parsed = crate::tmx::parse_tmx(&std::fs::read_to_string(dest).unwrap(), "en", "fr");
        assert_eq!(pairs_from_tmx(&parsed), pairs);
    }

    #[test]
    fn write_pairs_to_tmx_matches_java() {
        let g = load_align_golden("AlignerTest#testWritePairsToTMX_writesExpectedTMX.json");
        let mut tmx = ProjectTmx::new();
        for p in g["pairs"].as_array().unwrap() {
            tmx.insert(TmxEntry {
                source: p[0].as_str().unwrap().into(),
                translation: p[1].as_str().unwrap().into(),
                default_translation: true,
                creator: Some("OmegaT Aligner".into()),
                ..Default::default()
            });
        }
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.tmx");
        let src_lang = g["src_lang"].as_str().unwrap();
        let tgt_lang = g["tgt_lang"].as_str().unwrap();
        write_aligned_tmx(&tmx, &dest, src_lang, tgt_lang).unwrap();
        let xml = std::fs::read_to_string(&dest).unwrap();
        let parsed = crate::tmx::parse_tmx(&xml, src_lang, tgt_lang);
        assert_eq!(pairs_from_tmx(&parsed), expected_pairs(&g));

        let missing =
            load_align_golden("AlignerTest#testWritePairsToTMX_missingLanguageThrows.json");
        let err = write_aligned_tmx(&tmx, &dest, "", "").unwrap_err();
        let error_class = match err {
            crate::error::CoreError::InvalidProject(_) => "IllegalStateException",
            _ => "Other",
        };
        assert_eq!(error_class, missing["expect_error"].as_str().unwrap());

        let aligned =
            load_align_golden("AlignerTest#testDoAlign_withBeads_returnsAlignedBeads.json");
        let input_spec = serde_json::json!({ "pairs": aligned["beads"].clone() });
        let result_spec = serde_json::json!({ "pairs": aligned["result"].clone() });
        let beads = do_align(&expected_pairs(&input_spec), Some(AlignAlgo::Viterbi)).unwrap();
        assert_eq!(beads, expected_pairs(&result_spec));

        let missing = load_align_golden("AlignerTest#testDoAlign_missingSettingsThrows.json");
        let error_class = match do_align(&[("x".into(), "y".into())], None).unwrap_err() {
            crate::error::CoreError::InvalidProject(_) => "IllegalStateException",
            _ => "Other",
        };
        assert_eq!(error_class, missing["expect_error"].as_str().unwrap());
    }

    #[test]
    fn align_bundle_encodings_ascii_or_windows1252() {
        let g = load_align_golden("BundleTest#testBundleEncodings.json");
        assert_eq!(g["bundle"], "org.omegat.gui.align.Bundle");
        let accepted: Vec<String> = g["accepted_encodings"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
        assert_eq!(
            accepted,
            vec!["US-ASCII".to_string(), "WINDOWS-1252".to_string()]
        );
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../reference/java/aligner/src/main/resources/org/omegat/gui/align");
        let mut files = 0usize;
        for ent in std::fs::read_dir(&dir).unwrap() {
            let p = ent.unwrap().path();
            if p.extension().and_then(|e| e.to_str()) != Some("properties") {
                continue;
            }
            files += 1;
            let bytes = std::fs::read(&p).unwrap();
            let ascii = bytes.iter().all(|&b| b < 0x80);
            let utf8_non_ascii = std::str::from_utf8(&bytes)
                .ok()
                .is_some_and(|s| s.chars().any(|c| (c as u32) > 127));
            assert!(
                ascii || !utf8_non_ascii,
                "{} must be US-ASCII or Windows-1252 (Java BundleTest), not UTF-8 text",
                p.display()
            );
            let text = String::from_utf8_lossy(&bytes);
            assert!(!text.contains('\u{202e}'), "{} contains RTLO", p.display());
            assert!(
                text.contains('='),
                "{} must load at least one key",
                p.display()
            );
        }
        assert!(files >= 20, "aligner Bundle locales present: {files}");
    }

    #[test]
    fn id_mode_zips_lines() {
        let cfg = AlignConfig {
            mode: AlignMode::Id,
            ..AlignConfig::default()
        };
        let tmx = align_text("a\nb\n", "A\nB\n", "en", "fr", &cfg);
        assert_eq!(tmx.entries.len(), 2);
    }
}
