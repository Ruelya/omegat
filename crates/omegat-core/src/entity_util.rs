//! Java `org.omegat.util.html.EntityUtil`.

pub fn entities_to_chars(input: &str) -> String {
    let mut out = String::new();
    let mut rest = input;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        if let Some(semi) = after.find(';') {
            let name = &after[..semi];
            if let Some(ch) = decode_entity(name) {
                out.push_str(&ch);
                rest = &after[semi + 1..];
                continue;
            }
        }
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

fn decode_entity(name: &str) -> Option<String> {
    if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
        let n = u32::from_str_radix(hex, 16).ok()?;
        return char::from_u32(n).map(|c| c.to_string());
    }
    if let Some(dec) = name.strip_prefix('#') {
        let n = dec.parse::<u32>().ok()?;
        return char::from_u32(n).map(|c| c.to_string());
    }
    Some(
        match name {
            "lt" => "<",
            "gt" => ">",
            "amp" => "&",
            "quot" => "\"",
            "nbsp" => "\u{00A0}",
            "OElig" => "Œ",
            "oelig" => "œ",
            "Scaron" => "Š",
            "scaron" => "š",
            "Yuml" => "Ÿ",
            _ => return None,
        }
        .to_string(),
    )
}

pub fn chars_to_entities(input: &str, _encoding: &str, protected: &[&str]) -> String {
    let mut out = String::new();
    let mut i = 0;
    let chars: Vec<char> = input.chars().collect();
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        if let Some(p) = protected.iter().find(|p| rest.starts_with(*p)) {
            out.push_str(p);
            i += p.chars().count();
            continue;
        }
        match chars[i] {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\u{00A0}' => out.push_str("&nbsp;"),
            c => out.push(c),
        }
        i += 1;
    }
    out
}
