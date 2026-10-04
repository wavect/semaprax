//! Host-side guarantees on a model view: bounded display, raw rendering with a
//! stderr section, redaction before a provider sees data, and the critical-line
//! check that stops a lossy view from passing as complete.

/// Neutralize benign mentions so `0 failed` and `... ok` do not count.
fn neutral(line: &str) -> String {
    let mut l = line.to_lowercase();
    for p in [
        "0 failed",
        "0 errors",
        "0 error",
        "0 failures",
        "no errors",
        "no failures",
        "without errors",
        "0 panicked",
    ] {
        l = l.replace(p, "");
    }
    l
}

pub fn is_critical(line: &str) -> bool {
    let t = line.trim_end();
    if t.ends_with("... ok") || t.ends_with("... ignored") {
        return false;
    }
    let l = neutral(t);
    l.contains("error") || l.contains("fail") || l.contains("panic")
}

pub fn redact(text: &str, patterns: &[String]) -> String {
    if patterns.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let lower = line.to_lowercase();
        if patterns.iter().any(|p| lower.contains(p.as_str())) {
            out.push_str("[redacted by retention policy]");
            if line.ends_with('\n') {
                out.push('\n');
            }
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Lossy UTF-8 decode and the number of undecodable sequences.
pub fn decode(bytes: &[u8]) -> (String, u64) {
    let bad = bytes
        .utf8_chunks()
        .filter(|c| !c.invalid().is_empty())
        .count() as u64;
    (String::from_utf8_lossy(bytes).into_owned(), bad)
}

/// Raw display: stdout, then a labelled stderr section.
pub fn raw_text(stdout: &str, stderr: &str) -> String {
    if stderr.is_empty() {
        stdout.to_string()
    } else {
        format!(
            "{stdout}{}[stderr]\n{stderr}",
            if stdout.is_empty() || stdout.ends_with('\n') {
                ""
            } else {
                "\n"
            }
        )
    }
}

fn clip(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut e = max;
    while !s.is_char_boundary(e) {
        e -= 1;
    }
    &s[..e]
}

/// Critical raw lines (stdout then stderr) that `shown` does not contain.
pub fn missing_critical(raw: &str, shown: &str) -> Vec<String> {
    raw.lines()
        .filter(|l| is_critical(l) && !shown.contains(l.trim()))
        .map(String::from)
        .collect()
}

/// Bound `text` to `cap` bytes keeping head and tail. Returns the bounded text
/// and whether anything was cut.
pub fn bound(text: &str, cap: usize) -> (String, bool) {
    if text.len() <= cap {
        return (text.to_string(), false);
    }
    let head = clip(text, cap * 6 / 10);
    let tail_start = {
        let mut s = text.len() - cap * 3 / 10;
        while !text.is_char_boundary(s) {
            s += 1;
        }
        s
    };
    (
        format!(
            "{head}\n[... {} bytes omitted ...]\n{}",
            tail_start - head.len(),
            &text[tail_start..]
        ),
        true,
    )
}

/// Append up to 20 missing critical lines (bounded) so they are never only
/// implied.
pub fn append_critical(text: &mut String, missing: &[String]) {
    if missing.is_empty() {
        return;
    }
    text.push_str(&format!(
        "\n[host: {} critical line(s) were not in the view; first {}:]\n",
        missing.len(),
        missing.len().min(20)
    ));
    let mut budget = 4096usize;
    for l in missing.iter().take(20) {
        let c = clip(l, 240.min(budget));
        text.push_str(c);
        text.push('\n');
        budget = budget.saturating_sub(c.len() + 1);
        if budget == 0 {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn critical_rules() {
        assert!(is_critical("thread 'x' panicked at src/lib.rs"));
        assert!(is_critical("test a ... FAILED"));
        assert!(!is_critical("test result: ok. 3 passed; 0 failed"));
        assert!(!is_critical("test error_paths ... ok"));
    }

    #[test]
    fn redaction_replaces_lines() {
        let r = redact("a\nTOKEN=abc\nb\n", &["token=".into()]);
        assert_eq!(r, "a\n[redacted by retention policy]\nb\n");
    }
}
