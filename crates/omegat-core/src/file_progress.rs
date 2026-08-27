//! Java `ProjectFilesListController` progress helpers.

#[derive(Debug, Clone, Copy)]
pub struct FileProgress {
    pub translated: usize,
    pub total: usize,
}

impl FileProgress {
    pub fn new(translated: usize, total: usize) -> Self {
        Self { translated, total }
    }
}

pub fn format_progress_percent(translated: usize, total: usize) -> String {
    if total == 0 || translated == 0 {
        return "0%".into();
    }
    if translated == total {
        return "100.0%".into();
    }
    let pct = (translated as f64) * 100.0 / (total as f64);
    format!("{pct:.1}%")
}

pub fn compare_file_progress(a: FileProgress, b: FileProgress) -> i32 {
    let ar = if a.total == 0 {
        0.0
    } else {
        a.translated as f64 / a.total as f64
    };
    let br = if b.total == 0 {
        0.0
    } else {
        b.translated as f64 / b.total as f64
    };
    if ar < br {
        -1
    } else if ar > br {
        1
    } else {
        (a.total as i32) - (b.total as i32)
    }
}

pub fn progress_color(p: FileProgress) -> (u8, u8, u8) {
    if p.total == 0 || p.translated == 0 {
        (240, 184, 180)
    } else if p.translated == p.total {
        (184, 204, 240)
    } else {
        (183, 215, 183)
    }
}

pub fn progress_fill_width(p: FileProgress, max: usize) -> usize {
    if p.total == 0 {
        0
    } else if p.translated == 0 {
        3
    } else if p.translated == p.total {
        max
    } else {
        ((p.translated as f64 / p.total as f64) * max as f64).round() as usize
    }
}

pub fn calculate_file_progress(
    entries: usize,
    unique_translated: usize,
    unique_total: usize,
) -> FileProgress {
    let _ = entries;
    FileProgress::new(unique_translated, unique_total)
}

/// Java `updateProgressColumn`: hide leaves filename only (1); restore adds progress (2).
pub fn update_progress_column(show_progress: bool) -> (usize, usize) {
    if show_progress {
        (1, 2)
    } else {
        (1, 1)
    }
}

/// Java `syncTotalColumnsToFileColumns`: file column model indexes plus trailing margin (6).
pub fn sync_total_columns(file_order: &[i32]) -> Vec<i32> {
    let mut out: Vec<i32> = file_order.to_vec();
    if !out.contains(&6) {
        out.push(6);
    }
    out
}
