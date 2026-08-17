//! Java `org.omegat.filters2.latex.LatexFilter`.

use crate::misc::seg;
use crate::{ensure_parent, read_to_string, Filter, FilterContext, ParsedFile, Result};
use regex::Regex;
use std::collections::{HashMap, VecDeque};
use std::path::Path;

pub struct LatexFilter;

impl Filter for LatexFilter {
    fn id(&self) -> &'static str {
        "latex"
    }
    fn name(&self) -> &'static str {
        "LaTeX"
    }
    fn default_masks(&self) -> &'static [&'static str] {
        &["*.tex", "*.latex"]
    }
    fn parse(&self, path: &Path, _ctx: &FilterContext) -> Result<ParsedFile> {
        Ok(process(&read_to_string(path)?, None).parsed)
    }
    fn write(
        &self,
        source_path: &Path,
        dest_path: &Path,
        translations: &HashMap<String, String>,
        _ctx: &FilterContext,
    ) -> Result<()> {
        let out = process(&read_to_string(source_path)?, Some(translations)).written;
        ensure_parent(dest_path)?;
        std::fs::write(dest_path, out)?;
        Ok(())
    }
}

struct Outcome {
    parsed: ParsedFile,
    written: String,
}

struct Engine<'a> {
    translations: Option<&'a HashMap<String, String>>,
    segments: Vec<crate::ExtractedSegment>,
    written: String,
    line_break: String,
    verbatim_level: i32,
    one_arg_no_text: Vec<String>,
    one_arg_inline: Vec<String>,
    one_arg_par: Vec<String>,
    par_break: Vec<String>,
    verbatim_envs: Vec<String>,
}

fn process(raw: &str, translations: Option<&HashMap<String, String>>) -> Outcome {
    let mut eng = Engine {
        translations,
        segments: Vec::new(),
        written: String::new(),
        line_break: "\n".into(),
        verbatim_level: 0,
        one_arg_no_text: vec![
            "\\begin",
            "\\end",
            "\\cite",
            "\\label",
            "\\ref",
            "\\pageref",
            "\\pagestyle",
            "\\thispagestyle",
            "\\vspace",
            "\\hspace",
            "\\vskip",
            "\\hskip",
            "\\put",
            "\\includegraphics",
            "\\documentclass",
            "\\usepackage",
            "\\documentstyle",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        one_arg_inline: vec![
            "\\emph", "\\textbf", "\\texttt", "\\textsf", "\\textit", "\\hbox", "\\mbox", "\\vbox",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        one_arg_par: vec![
            "\\typeout",
            "\\footnote",
            "\\author",
            "\\index",
            "\\title",
            "\\Chapter",
            "\\chapter",
            "\\section*",
            "\\section",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        par_break: vec![
            "\\item",
            "\\newcommand",
            "\\renewcommand",
            "\\maketitle",
            "\\addcontentsline",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        verbatim_envs: vec![
            "verbatim",
            "verbatim*",
            "comment",
            "verbatimimport",
            "lstlisting",
            "lstlisting*",
            "minted",
            "listing",
            "listing*",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    };
    eng.run(raw);
    Outcome {
        parsed: ParsedFile {
            segments: eng.segments,
            skeleton: Some(eng.written.clone()),
        },
        written: eng.written,
    }
}

impl Engine<'_> {
    fn run(&mut self, raw: &str) {
        let mut par = String::new();
        let mut comment = String::new();
        let mut commands: Vec<String> = Vec::new();
        for (s, br) in crate::text::lines_with_breaks(raw) {
            if self.verbatim_level == 0 {
                if let Some(env) = parse_braced_command(s.trim(), "\\begin{") {
                    if self.verbatim_envs.iter().any(|e| e == &env) {
                        self.verbatim_level += 1;
                        self.written.push_str(s);
                        self.written.push_str(br);
                        continue;
                    }
                }
            }
            if self.verbatim_level > 0 {
                self.written.push_str(s);
                self.written.push_str(br);
                if let Some(env) = parse_braced_command(s.trim(), "\\end{") {
                    if self.verbatim_envs.iter().any(|e| e == &env) {
                        self.verbatim_level -= 1;
                    }
                }
                continue;
            }
            self.line_break = br.to_string();
            if self.line_break.is_empty() {
                self.line_break = "\n".into();
            }
            let chars: Vec<char> = s.chars().collect();
            let mut state = "N";
            let mut idx = 0usize;
            while idx < chars.len() {
                let cidx = chars[idx];
                let cat = find_category(cidx);
                if cat == 0 {
                    let mut cmd = String::from("\\");
                    idx += 1;
                    while idx < chars.len() {
                        let cmdc = chars[idx];
                        if find_category(cmdc) == 11 {
                            cmd.push(cmdc);
                        } else if cmd.len() == 1 {
                            cmd.push(cmdc);
                            state = "M";
                            break;
                        } else {
                            idx -= 1;
                            state = "M";
                            break;
                        }
                        idx += 1;
                    }
                    if !commands.contains(&cmd) {
                        commands.push(cmd.clone());
                    }
                    par.push_str(&cmd);
                } else if cat == 4 {
                    let out = self.process_paragraph(&commands, &par);
                    self.written.push_str(&out);
                    self.written.push('&');
                    par.clear();
                    commands.clear();
                } else if cat == 10 {
                    if state == "M" {
                        state = "S";
                        par.push(cidx);
                    }
                } else if cat == 14 {
                    comment.push(cidx);
                    idx += 1;
                    while idx < chars.len() {
                        comment.push(chars[idx]);
                        idx += 1;
                    }
                    continue;
                } else {
                    state = "M";
                    par.push(cidx);
                }
                idx += 1;
            }
            if state == "N" {
                if !par.is_empty() {
                    let out = self.process_paragraph(&commands, &par);
                    self.written.push_str(&out);
                    self.written.push_str(&self.line_break);
                    self.written.push_str(&self.line_break);
                    par.clear();
                }
                commands.clear();
                if !comment.is_empty() {
                    self.written.push_str(&comment);
                    self.written.push_str(&self.line_break);
                    comment.clear();
                }
            } else if state == "M" {
                par.push(' ');
            }
        }
        if !par.is_empty() {
            let out = self.process_paragraph(&commands, &par);
            self.written.push_str(&out);
        }
    }

    fn process_entry(&mut self, entry: &str) -> String {
        if !entry.is_empty() {
            self.segments.push(seg(self.segments.len().to_string(), entry));
        }
        if let Some(map) = self.translations {
            map.get(entry).cloned().unwrap_or_else(|| entry.to_string())
        } else {
            entry.to_string()
        }
    }

    fn process_paragraph(&mut self, commands: &[String], par: &str) -> String {
        let mut substituted: VecDeque<(String, String)> = VecDeque::new();
        let mut par = substitute_unicode(par);
        par = self.replace_par_break(&mut substituted, commands, &par);
        par = self.replace_one_arg_no_text(&mut substituted, commands, &par);
        par = self.replace_one_arg_inline(&mut substituted, commands, &par);
        par = self.replace_one_arg_par(&mut substituted, commands, &par);
        par = self.replace_unknown(&mut substituted, commands, &par);

        let find = Regex::new(r"^((?:\s*</?[nipu]\d+>\s*)*)(.*?)((?:\s*</?[nipu]\d+>\s*)*)$").unwrap();
        if let Some(m) = find.captures(&par) {
            let mut rebuilt = String::new();
            if let Some(g1) = m.get(1) {
                rebuilt.push_str(g1.as_str());
            }
            if let Some(g2) = m.get(2) {
                rebuilt.push_str(&self.process_entry(g2.as_str()));
            }
            if let Some(g3) = m.get(3) {
                rebuilt.push_str(g3.as_str());
            }
            par = rebuilt;
        }
        par = resubstitute_tex(&par);
        for (orig, placeholder) in substituted {
            par = par.replace(&placeholder, &orig);
        }
        par
    }

    fn replace_par_break(
        &mut self,
        substituted: &mut VecDeque<(String, String)>,
        commands: &[String],
        par: &str,
    ) -> String {
        let mut tmp = par.to_string();
        let mut counter = 0i32;
        for command in commands {
            if !self.par_break.contains(command) {
                continue;
            }
            let find = format!(".*({})", regex::escape(command));
            let Ok(p) = Regex::new(&find) else {
                continue;
            };
            let mut sb = String::new();
            let mut last = 0usize;
            for m in p.captures_iter(&tmp) {
                let full = m.get(0).unwrap();
                let g1 = m.get(1).unwrap();
                let content = self.process_paragraph(commands, &tmp[..g1.start()]);
                let replace = format!("<r{counter}>");
                substituted.push_front((format!("{}{}", content, format!("{}{}", self.line_break, &g1.as_str())), replace.clone()));
                sb.push_str(&tmp[last..full.start()]);
                sb.push_str(&replace);
                last = full.end();
                counter += 1;
            }
            if last == 0 && sb.is_empty() {
                continue;
            }
            sb.push_str(&tmp[last..]);
            tmp = sb;
        }
        tmp
    }

    fn replace_one_arg_no_text(
        &mut self,
        substituted: &mut VecDeque<(String, String)>,
        commands: &[String],
        par: &str,
    ) -> String {
        let mut par = par.to_string();
        let mut counter = 0i32;
        for command in commands {
            if !self.one_arg_no_text.contains(command) {
                continue;
            }
            let find = format!(
                "{}\\*?(\\[[^\\]]*\\]|\\([^\\)]*\\))?\\s*\\{{[^}}]*\\}}",
                regex::escape(command)
            );
            let Ok(p) = Regex::new(&find) else {
                continue;
            };
            let mut sb = String::new();
            let mut last = 0usize;
            for m in p.find_iter(&par) {
                let replace = format!("<n{counter}>");
                substituted.push_front((format!("{}{}", self.line_break, m.as_str()), replace.clone()));
                sb.push_str(&par[last..m.start()]);
                sb.push_str(&replace);
                last = m.end();
                counter += 1;
            }
            if last == 0 && sb.is_empty() {
                continue;
            }
            sb.push_str(&par[last..]);
            par = sb;
        }
        par
    }

    fn replace_one_arg_inline(
        &mut self,
        substituted: &mut VecDeque<(String, String)>,
        commands: &[String],
        par: &str,
    ) -> String {
        let mut par = par.to_string();
        let mut counter = 0i32;
        for command in commands {
            if !self.one_arg_inline.contains(command) {
                continue;
            }
            let find = format!("({}\\s*\\{{)([^}}]*)\\}}", regex::escape(command));
            let Ok(p) = Regex::new(&find) else {
                continue;
            };
            let mut sb = String::new();
            let mut last = 0usize;
            for m in p.captures_iter(&par) {
                let full = m.get(0).unwrap();
                let pre = format!("<i{counter}>");
                let post = format!("</i{counter}>");
                substituted.push_front((m.get(1).unwrap().as_str().to_string(), pre.clone()));
                substituted.push_front(("}".into(), post.clone()));
                sb.push_str(&par[last..full.start()]);
                sb.push_str(&pre);
                sb.push_str(&m[2]);
                sb.push_str(&post);
                last = full.end();
                counter += 1;
            }
            if last == 0 && sb.is_empty() {
                continue;
            }
            sb.push_str(&par[last..]);
            par = sb;
        }
        par
    }

    fn replace_one_arg_par(
        &mut self,
        substituted: &mut VecDeque<(String, String)>,
        commands: &[String],
        par: &str,
    ) -> String {
        let mut par = par.to_string();
        let mut counter = 0i32;
        for command in commands {
            if !self.one_arg_par.contains(command) {
                continue;
            }
            let find = format!("({}\\*?\\s*)\\{{([^}}]*)\\}}", regex::escape(command));
            let Ok(p) = Regex::new(&find) else {
                continue;
            };
            let mut sb = String::new();
            let mut last = 0usize;
            for m in p.captures_iter(&par) {
                let full = m.get(0).unwrap();
                let replace = format!("<p{counter}>");
                let inner = m.get(2).map(|g| g.as_str()).unwrap_or("");
                let content = self.process_paragraph(commands, inner);
                substituted.push_front((
                    format!("{}{}{{{}}}", self.line_break, m.get(1).unwrap().as_str(), content),
                    replace.clone(),
                ));
                sb.push_str(&par[last..full.start()]);
                sb.push_str(&replace);
                last = full.end();
                counter += 1;
            }
            if last == 0 && sb.is_empty() {
                continue;
            }
            sb.push_str(&par[last..]);
            par = sb;
        }
        par
    }

    fn replace_unknown(
        &mut self,
        substituted: &mut VecDeque<(String, String)>,
        commands: &[String],
        par: &str,
    ) -> String {
        let mut par = par.to_string();
        let mut placeholder_index = 0i32;
        let mut sorted = commands.to_vec();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.len()));
        for mut command in sorted {
            // Java: extra escape only for \\, \{, \[, \| then Pattern.compile("\\"+command).
            if command == "\\\\" || command == "\\{" || command == "\\[" || command == "\\|" {
                command = format!("\\{command}");
            }
            let find = format!("\\{command}");
            let Ok(p) = Regex::new(&find) else {
                continue;
            };
            let mut sb = String::new();
            let mut last = 0usize;
            for m in p.find_iter(&par) {
                let replace = format!("<u{placeholder_index}>");
                substituted.push_front((m.as_str().to_string(), replace.clone()));
                sb.push_str(&par[last..m.start()]);
                sb.push_str(&replace);
                last = m.end();
                placeholder_index += 1;
            }
            if last == 0 && sb.is_empty() {
                continue;
            }
            sb.push_str(&par[last..]);
            par = sb;
        }
        par
    }
}

fn find_category(c: char) -> i32 {
    match c {
        '\\' => 0,
        '{' => 1,
        '}' => 2,
        '$' => 3,
        '&' => 4,
        '\n' => 5,
        '#' => 6,
        '^' => 7,
        '_' => 8,
        '\u{0000}' => 9,
        ' ' | '\t' => 10,
        'a'..='z' | 'A'..='Z' => 11,
        '~' => 13,
        '%' => 14,
        _ => 12,
    }
}

fn parse_braced_command(line: &str, prefix: &str) -> Option<String> {
    if !line.starts_with(prefix) {
        return None;
    }
    let open = line.find('{')?;
    let close = line[open + 1..].find('}')?;
    Some(line[open + 1..open + 1 + close].trim().to_string())
}

fn substitute_unicode(par: &str) -> String {
    let mut par = Regex::new(r"\\\\").unwrap().replace_all(par, "<br0>").into_owned();
    par = Regex::new(r"\{?\\ss}?")
        .unwrap()
        .replace_all(&par, "ß")
        .into_owned();
    par = Regex::new(r"\{?\\glqq}?(\{\})?")
        .unwrap()
        .replace_all(&par, "〟")
        .into_owned();
    par = Regex::new(r"\{?\\grqq}?(\{\})?")
        .unwrap()
        .replace_all(&par, "〝")
        .into_owned();
    par = Regex::new(r"\{?\\glq}?(\{\})?")
        .unwrap()
        .replace_all(&par, "‚")
        .into_owned();
    par = Regex::new(r"\{?\\grq}?(\{\})?")
        .unwrap()
        .replace_all(&par, "‘")
        .into_owned();
    par = Regex::new(r"\\%").unwrap().replace_all(&par, "%").into_owned();
    par = Regex::new(r"\\-")
        .unwrap()
        .replace_all(&par, "\u{00ad}")
        .into_owned();
    par = Regex::new(r"\\,")
        .unwrap()
        .replace_all(&par, "\u{2009}")
        .into_owned();
    par.replace('~', "\u{00a0}")
}

fn resubstitute_tex(par: &str) -> String {
    par.replace('\u{00a0}', "~")
        .replace('\u{2009}', "\\,")
        .replace('\u{00ad}', "\\-")
        .replace('%', "\\%")
        .replace("<br0>", "\\\\")
}
