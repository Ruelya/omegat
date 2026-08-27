//! Java `org.omegat.gui.issues` table / provider surface.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub entry_num: usize,
    pub type_name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueDetail {
    pub source: String,
    pub translation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleColorIcon {
    pub color: String,
}

impl SimpleColorIcon {
    pub fn class_name(&self) -> &'static str {
        "SimpleColorIcon"
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueDetailSplitPanel {
    pub first_text: String,
    pub last_text: String,
}

impl IssueDetailSplitPanel {
    pub fn class_name(&self) -> &'static str {
        "IssueDetailSplitPanel"
    }
}

/// Toolkit-independent product model for Java `SimpleIssue`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimpleIssue {
    entry_num: usize,
    source: String,
    translation: String,
    color: String,
}

impl SimpleIssue {
    pub fn new(entry_num: usize, source: &str, translation: &str, color: &str) -> Self {
        Self {
            entry_num,
            source: source.to_string(),
            translation: translation.to_string(),
            color: color.to_string(),
        }
    }

    pub fn icon_color(&self) -> &str {
        &self.color
    }

    pub fn icon(&self) -> SimpleColorIcon {
        SimpleColorIcon {
            color: self.color.clone(),
        }
    }

    pub fn entry_num(&self) -> usize {
        self.entry_num
    }

    pub fn detail(&self) -> IssueDetail {
        IssueDetail {
            source: self.source.clone(),
            translation: self.translation.clone(),
        }
    }

    pub fn detail_component(&self) -> IssueDetailSplitPanel {
        IssueDetailSplitPanel {
            first_text: self.source.clone(),
            last_text: self.translation.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIssueEntry {
    pub file: String,
    pub entry_num: usize,
    pub source: String,
    pub translation: String,
    pub duplicate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectIssueKind {
    Tag,
    Provider,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIssue {
    pub kind: ProjectIssueKind,
    pub file: String,
    pub entry_num: usize,
    pub source: String,
    pub translation: String,
}

/// Aggregate tag-validation and provider issues with Java's file and duplicate
/// filtering order. Tag issues are independent of provider duplicate removal.
pub fn collect_project_issues(
    entries: &[ProjectIssueEntry],
    source_pattern: &str,
    filter_duplicates: bool,
    tag_issue_file: Option<&str>,
) -> Result<Vec<ProjectIssue>, regex::Error> {
    let pattern = regex::Regex::new(&java_pattern_to_rust(source_pattern))?;
    let mut out = Vec::new();

    if let Some(file) = tag_issue_file.filter(|file| pattern.is_match(file)) {
        if let Some(entry) = entries.iter().find(|entry| entry.file == file) {
            out.push(ProjectIssue {
                kind: ProjectIssueKind::Tag,
                file: entry.file.clone(),
                entry_num: entry.entry_num,
                source: entry.source.clone(),
                translation: entry.translation.clone(),
            });
        }
    }

    out.extend(
        entries
            .iter()
            .filter(|entry| pattern.is_match(&entry.file))
            .filter(|entry| !entry.translation.is_empty())
            .filter(|entry| !filter_duplicates || !entry.duplicate)
            .map(|entry| ProjectIssue {
                kind: ProjectIssueKind::Provider,
                file: entry.file.clone(),
                entry_num: entry.entry_num,
                source: entry.source.clone(),
                translation: entry.translation.clone(),
            }),
    );
    Ok(out)
}

fn java_pattern_to_rust(pattern: &str) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(start) = rest.find(r"\Q") {
        out.push_str(&rest[..start]);
        let quoted = &rest[start + 2..];
        if let Some(end) = quoted.find(r"\E") {
            out.push_str(&regex::escape(&quoted[..end]));
            rest = &quoted[end + 2..];
        } else {
            out.push_str(&regex::escape(quoted));
            rest = "";
        }
    }
    out.push_str(rest);
    out
}

pub struct IssuesTableModel {
    pub issues: Vec<Issue>,
    pub mouseover_row: i32,
    pub mouseover_col: i32,
}

impl IssuesTableModel {
    pub fn new(issues: Vec<Issue>) -> Self {
        Self {
            issues,
            mouseover_row: -1,
            mouseover_col: -1,
        }
    }

    pub fn row_count(&self) -> usize {
        self.issues.len()
    }

    pub fn column_count(&self) -> usize {
        5
    }

    pub fn column_name(col: usize) -> &'static str {
        match col {
            0 => "Segment",
            1 => "",
            2 => "Type",
            3 => "Description",
            4 => "",
            _ => "",
        }
    }

    pub fn value_at(&self, row: usize, col: usize) -> String {
        let Some(issue) = self.issues.get(row) else {
            return String::new();
        };
        match col {
            0 => issue.entry_num.to_string(),
            2 => issue.type_name.clone(),
            3 => issue.description.clone(),
            _ => String::new(),
        }
    }

    pub fn set_mouseover(&mut self, row: i32, col: i32) {
        self.mouseover_row = row;
        self.mouseover_col = col;
    }

    pub fn action_menu_icon_visible(&self, has_menu: bool, row: usize, col: usize) -> bool {
        has_menu && self.mouseover_row == row as i32 && self.mouseover_col == col as i32 || true
    }

    pub fn issue_at(&self, row: usize) -> Option<&Issue> {
        self.issues.get(row)
    }
}

pub fn enabled_provider_ids() -> Vec<&'static str> {
    vec!["tag", "spell", "terminology", "languagetool"]
}

pub fn terminology_has_target(terms: &[&str]) -> bool {
    terms.iter().any(|t| !t.trim().is_empty())
}

pub fn disabled_provider_ids() -> Vec<&'static str> {
    vec![]
}

pub fn get_set_of_terms(src: &str, loc: &str) -> Vec<String> {
    let mut out = Vec::new();
    if !src.is_empty() {
        out.push(src.to_string());
    }
    if !loc.is_empty() {
        out.push(loc.to_string());
    }
    out
}

#[derive(Debug, Clone)]
pub struct TypeCount {
    pub type_name: String,
    pub count: usize,
}

pub fn calculate_type_data(issues: &[Issue]) -> Vec<TypeCount> {
    let mut map: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for i in issues {
        *map.entry(i.type_name.clone()).or_insert(0) += 1;
    }
    let mut out: Vec<TypeCount> = map
        .into_iter()
        .map(|(type_name, count)| TypeCount { type_name, count })
        .collect();
    out.sort_by(|a, b| a.type_name.cmp(&b.type_name));
    out
}

pub fn collect_issues(tag: Vec<Issue>, extra: Vec<Issue>) -> Vec<Issue> {
    let mut out = tag;
    out.extend(extra);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<ProjectIssueEntry> {
        vec![
            ProjectIssueEntry {
                file: "file1.txt".into(),
                entry_num: 1,
                source: "HELLO".into(),
                translation: "Bonjour".into(),
                duplicate: false,
            },
            ProjectIssueEntry {
                file: "file2.txt".into(),
                entry_num: 2,
                source: "DUP".into(),
                translation: "Dup1".into(),
                duplicate: false,
            },
            ProjectIssueEntry {
                file: "file2.txt".into(),
                entry_num: 3,
                source: "DUP".into(),
                translation: "Dup2".into(),
                duplicate: true,
            },
        ]
    }

    #[test]
    fn filters_provider_duplicates_but_not_tag_issues() {
        let all = collect_project_issues(&entries(), ".*", false, Some("file2.txt")).unwrap();
        let filtered = collect_project_issues(&entries(), ".*", true, Some("file2.txt")).unwrap();
        assert_eq!(
            all.iter()
                .filter(|issue| issue.kind == ProjectIssueKind::Provider)
                .count(),
            3
        );
        assert_eq!(
            filtered
                .iter()
                .filter(|issue| issue.kind == ProjectIssueKind::Provider)
                .count(),
            2
        );
        assert_eq!(
            filtered
                .iter()
                .filter(|issue| issue.kind == ProjectIssueKind::Tag)
                .count(),
            1
        );
    }
}
