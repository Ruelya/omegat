use crate::{
    ensure_parent, read_to_string, ExtractedSegment, Filter, FilterContext, FilterError,
    ParsedFile, Result,
};
use std::collections::HashMap;
use std::path::Path;

pub struct CsvFilter;

impl Filter for CsvFilter {
    fn id(&self) -> &'static str {
        "csv"
    }
    fn name(&self) -> &'static str {
        "CSV / TSV"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.csv", "*.tsv"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        let raw = read_to_string(path)?;
        let delim = if path.extension().and_then(|e| e.to_str()) == Some("tsv") {
            b'\t'
        } else {
            b','
        };
        parse_csv(&raw, delim)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let raw = read_to_string(source_path)?;
        let delim = if source_path.extension().and_then(|e| e.to_str()) == Some("tsv") {
            b'\t'
        } else {
            b','
        };
        let mut rdr = csv::ReaderBuilder::new()
            .delimiter(delim)
            .has_headers(false)
            .from_reader(raw.as_bytes());
        let mut wtr = csv::WriterBuilder::new()
            .delimiter(delim)
            .from_writer(Vec::new());
        let mut row_i = 0usize;
        for rec in rdr.records() {
            let rec = rec.map_err(|e| FilterError::Parse {
                format: "csv".into(),
                message: e.to_string(),
            })?;
            let mut out = rec.iter().map(|s| s.to_string()).collect::<Vec<_>>();
            if let Some(cell) = out.last_mut() {
                let id = format!("{row_i}");
                if let Some(t) = translations.get(&id) {
                    *cell = t.clone();
                }
            }
            wtr.write_record(&out).map_err(|e| FilterError::Parse {
                format: "csv".into(),
                message: e.to_string(),
            })?;
            row_i += 1;
        }
        let bytes = wtr.into_inner().map_err(|e| FilterError::Parse {
            format: "csv".into(),
            message: e.to_string(),
        })?;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, bytes)?;
        Ok(())
    }
}

fn parse_csv(raw: &str, delim: u8) -> Result<ParsedFile> {
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .from_reader(raw.as_bytes());
    let mut segments = Vec::new();
    for (i, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| FilterError::Parse {
            format: "csv".into(),
            message: e.to_string(),
        })?;
        let source = rec.iter().last().unwrap_or("").to_string();
        if source.trim().is_empty() {
            continue;
        }
        let key = rec.iter().next().unwrap_or("").to_string();
        segments.push(ExtractedSegment {
            id: i.to_string(),
            source,
            existing_translation: None,
            note: None,
            comment: None,
            path: Some(key),
            protected_parts: vec![],
        });
    }
    Ok(ParsedFile {
        segments,
        skeleton: Some(raw.to_string()),
    })
}
