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
        3
    }

    pub fn column_name(col: usize) -> &'static str {
        match col {
            0 => "Segment",
            1 => "Type",
            2 => "Description",
            _ => "",
        }
    }

    pub fn value_at(&self, row: usize, col: usize) -> String {
        let Some(issue) = self.issues.get(row) else {
            return String::new();
        };
        match col {
            0 => issue.entry_num.to_string(),
            1 => issue.type_name.clone(),
            2 => issue.description.clone(),
            _ => String::new(),
        }
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
