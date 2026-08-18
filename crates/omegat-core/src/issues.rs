//! Java `org.omegat.gui.issues` table / provider surface.

#[derive(Debug, Clone)]
pub struct Issue {
    pub entry_num: usize,
    pub type_name: String,
    pub description: String,
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
