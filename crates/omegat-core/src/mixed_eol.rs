//! Java `org.omegat.util.MixedEolHandlingReader`.

#[derive(Debug, Clone)]
pub struct MixedEolReader {
    pub detected_eol: String,
    pub mixed: bool,
    lines: Vec<String>,
    idx: usize,
}

impl MixedEolReader {
    pub fn from_text(text: &str) -> Self {
        let (detected, mixed) = detect(text);
        let lines = split_on(&detected, text);
        Self {
            detected_eol: detected,
            mixed,
            lines,
            idx: 0,
        }
    }

    pub fn read_line(&mut self) -> Option<String> {
        if self.idx >= self.lines.len() {
            return None;
        }
        let line = self.lines[self.idx].clone();
        self.idx += 1;
        Some(line)
    }
}

fn detect(text: &str) -> (String, bool) {
    let mut crlf = 0usize;
    let mut cr = 0usize;
    let mut lf = 0usize;
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\r' && i + 1 < b.len() && b[i + 1] == b'\n' {
            crlf += 1;
            i += 2;
            continue;
        }
        if b[i] == b'\r' {
            cr += 1;
        } else if b[i] == b'\n' {
            lf += 1;
        }
        i += 1;
    }
    let kinds = [crlf > 0, cr > 0, lf > 0].iter().filter(|x| **x).count();
    let mixed = kinds > 1;
    if crlf == 0 && cr == 0 && lf == 0 {
        return (
            std::env::consts::FAMILY
                .eq("windows")
                .then_some("\r\n")
                .unwrap_or("\n")
                .into(),
            false,
        );
    }
    if crlf == 0 && cr == lf && cr > 0 {
        let sys = if cfg!(windows) { "\r\n" } else { "\n" };
        return (sys.into(), true);
    }
    let detected = if crlf >= cr && crlf >= lf {
        "\r\n"
    } else if cr >= lf {
        "\r"
    } else {
        "\n"
    };
    (detected.into(), mixed)
}

fn split_on(eol: &str, text: &str) -> Vec<String> {
    if eol == "\r\n" {
        text.split("\r\n").map(|s| s.to_string()).collect()
    } else if eol == "\r" {
        text.split('\r').map(|s| s.to_string()).collect()
    } else {
        text.split('\n').map(|s| s.to_string()).collect()
    }
}
