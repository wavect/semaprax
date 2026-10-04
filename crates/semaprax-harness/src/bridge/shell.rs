//! Conservative single-command tokenizer and read-only RTK hook detection.

/// Split a Bash command string into argv only when it is one plain command:
/// whitespace-separated words with optional `'..'` / `".."` quoting and no
/// expansion, globbing, redirection, pipeline or list syntax. `Err` carries why.
pub fn tokenize(cmd: &str) -> Result<Vec<String>, String> {
    let mut args: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut have = false;
    let mut quote: Option<char> = None;
    for c in cmd.chars() {
        if c == '\n' || c == '\r' || c == '\0' {
            return Err("multi-line or control characters".into());
        }
        match quote {
            Some('\'') => {
                if c == '\'' {
                    quote = None;
                } else {
                    cur.push(c);
                }
            }
            Some(_) => {
                if c == '"' {
                    quote = None;
                } else if matches!(c, '$' | '`' | '\\' | '!') {
                    return Err(format!("expansion character `{c}` inside double quotes"));
                } else {
                    cur.push(c);
                }
            }
            None => match c {
                '\'' | '"' => {
                    quote = Some(c);
                    have = true;
                }
                c if c.is_whitespace() => {
                    if have {
                        args.push(std::mem::take(&mut cur));
                        have = false;
                    }
                }
                '$' | '`' | '\\' | '*' | '?' | '[' | ']' | '~' | '{' | '}' | '(' | ')' | '<'
                | '>' | '|' | '&' | ';' | '#' | '!' => {
                    return Err(format!("shell syntax `{c}`"));
                }
                _ => {
                    cur.push(c);
                    have = true;
                }
            },
        }
    }
    if quote.is_some() {
        return Err("unterminated quote".into());
    }
    if have {
        args.push(cur);
    }
    if args.is_empty() {
        return Err("empty command".into());
    }
    if args[0].contains('=') {
        return Err("leading environment assignment".into());
    }
    Ok(args)
}

/// POSIX single-quote a word.
pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn rtk_word(w: &str) -> bool {
    let w = w.trim_matches(|c| c == '"' || c == '\'');
    w == "rtk"
        || w == "rtk.exe"
        || w.ends_with("/rtk")
        || w.ends_with("\\rtk")
        || w.ends_with("/rtk.exe")
        || w.ends_with("\\rtk.exe")
}

fn command_matches(c: &str) -> bool {
    if c.contains("rtk-rewrite.sh")
        || c.contains("rtk-rewrite.json")
        || c.contains("rtk-hook-gemini.sh")
    {
        return true;
    }
    let w: Vec<&str> = c.split_whitespace().collect();
    w.windows(3).any(|t| {
        rtk_word(t[0]) && t[1] == "hook" && t[2].starts_with(|ch: char| ch.is_ascii_lowercase())
    })
}

fn commands(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::Object(m) => {
            if let Some(serde_json::Value::String(s)) = m.get("command") {
                out.push(s.clone());
            }
            m.values().for_each(|x| commands(x, out));
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| commands(x, out)),
        _ => {}
    }
}

/// True when the settings text registers an RTK rewrite hook. Port of
/// `packages/semaprax-harness-adapters/rtk/hook_detect.py`; reads text only.
pub fn detect_rtk_hook(settings_text: &str) -> bool {
    match serde_json::from_str::<serde_json::Value>(settings_text) {
        Ok(v) => {
            let mut c = Vec::new();
            commands(&v, &mut c);
            c.iter().any(|s| command_matches(s))
        }
        Err(_) => command_matches(settings_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_plain_commands_only() {
        assert_eq!(
            tokenize("git log --oneline -3").unwrap(),
            ["git", "log", "--oneline", "-3"]
        );
        assert_eq!(
            tokenize("echo 'a b' \"c d\"").unwrap(),
            ["echo", "a b", "c d"]
        );
        for bad in [
            "ls | wc",
            "echo $HOME",
            "ls *.rs",
            "a && b",
            "x=1 ls",
            "echo `id`",
            "ls > f",
            "echo 'a",
            "a\nb",
        ] {
            assert!(tokenize(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn detects_rtk_like_the_python_adapter() {
        let s = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"/usr/local/bin/rtk hook claude"}]}]}}"#;
        assert!(detect_rtk_hook(s));
        assert!(detect_rtk_hook(
            r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"~/.claude/hooks/rtk-rewrite.sh"}]}]}}"#
        ));
        assert!(detect_rtk_hook("rtk hook vibe"));
        assert!(!detect_rtk_hook(
            r#"{"hooks":{"PreToolUse":[{"hooks":[{"command":"echo rtk"}]}]}}"#
        ));
        assert!(!detect_rtk_hook("{}"));
    }

    #[test]
    fn quoting_round_trips() {
        assert_eq!(quote("a'b"), "'a'\\''b'");
    }
}
