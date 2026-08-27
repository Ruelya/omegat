//! Java `org.omegat.util.StaticUtils`.

/// Java `StaticUtils.parseCLICommand`.
pub fn parse_cli_command(cmd: &str) -> Vec<String> {
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return vec![String::new()];
    }
    let mut arg = String::new();
    let mut result = Vec::new();
    let chars: Vec<char> = cmd.chars().collect();
    let mut i = 0;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        if Some(c) == quote {
            quote = None;
            i += 1;
            continue;
        }
        if quote.is_none() && (c == '"' || c == '\'') {
            quote = Some(c);
            i += 1;
            continue;
        }
        if c == '\\' && i + 1 < chars.len() {
            let next = chars[i + 1];
            if (quote.is_none() && next.is_whitespace()) || (quote == Some('"') && next == '"') {
                arg.push(next);
                i += 2;
                continue;
            }
            arg.push(c);
            i += 1;
            continue;
        }
        if c.is_whitespace() && quote.is_none() {
            if !arg.is_empty() {
                result.push(std::mem::take(&mut arg));
            }
            i += 1;
            continue;
        }
        arg.push(c);
        i += 1;
    }
    if !arg.is_empty() {
        result.push(arg);
    }
    result
}

pub fn glob_to_regex(text: &str, space_match_nbsp: bool) -> String {
    crate::search::glob_to_regex(text, space_match_nbsp)
}

pub fn glob_matches(glob: &str, text: &str, space_match_nbsp: bool) -> bool {
    let pat = format!("^{}$", glob_to_regex(glob, space_match_nbsp));
    regex::Regex::new(&pat).map(|r| r.is_match(text)).unwrap_or(false)
}
