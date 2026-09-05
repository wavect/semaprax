use std::fmt;
use std::fmt::Write as _;

use crate::ast::Span;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub fn is_error(self) -> bool {
        matches!(self, Severity::Error)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub path: Option<String>,
    pub span: Option<Span>,
    pub help: Option<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            path: None,
            span: Some(span),
            help: None,
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>, span: Span) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            path: None,
            span: Some(span),
            help: None,
        }
    }

    pub fn io(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            path: None,
            span: None,
            help: None,
        }
    }

    pub fn at_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn json(&self) -> String {
        let location = self.span.map_or_else(
            || "null".to_owned(),
            |span| {
                format!(
                    "{{\"line\":{},\"column\":{},\"start\":{},\"end\":{}}}",
                    span.line, span.column, span.start, span.end
                )
            },
        );
        let path = self
            .path
            .as_ref()
            .map_or_else(|| "null".to_owned(), |value| quote_json(value));
        let help = self
            .help
            .as_ref()
            .map_or_else(|| "null".to_owned(), |value| quote_json(value));
        format!(
            "{{\"code\":{},\"severity\":{},\"message\":{},\"path\":{},\"location\":{},\"help\":{}}}",
            quote_json(self.code),
            quote_json(self.severity.as_str()),
            quote_json(&self.message),
            path,
            location,
            help
        )
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}[{}]: {}",
            self.severity.as_str(),
            self.code,
            self.message
        )?;
        match (&self.path, self.span) {
            (Some(path), Some(span)) => {
                f.write_str(" at ")?;
                write_human_path(f, path)?;
                write!(f, ":{}:{}", span.line, span.column)?;
            }
            (Some(path), None) => {
                f.write_str(" at ")?;
                write_human_path(f, path)?;
            }
            (None, Some(span)) => write!(f, " at {}:{}", span.line, span.column)?,
            (None, None) => {}
        }
        if let Some(help) = &self.help {
            write!(f, "\n  help: {help}")?;
        }
        Ok(())
    }
}

fn write_human_path(f: &mut fmt::Formatter<'_>, path: &str) -> fmt::Result {
    for character in path.chars() {
        if character.is_control() {
            for escaped in character.escape_default() {
                f.write_char(escaped)?;
            }
        } else {
            f.write_char(character)?;
        }
    }
    Ok(())
}

pub fn quote_json(value: &str) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            value if value.is_control() => {
                output.push_str(&format!("\\u{:04x}", value as u32));
            }
            value => output.push(value),
        }
    }
    output.push('"');
    output.into_string()
}

#[cfg(test)]
mod tests {
    use super::{quote_json, Diagnostic};
    use crate::ast::Span;
    use crate::bounded_output::with_limit;

    fn span() -> Span {
        Span {
            line: 7,
            column: 3,
            start: 42,
            end: 51,
        }
    }

    #[test]
    fn display_renders_each_location_combination_in_a_stable_order() {
        let both = Diagnostic::error("SPX-T001", "bad thing", span()).at_path("src/a.spx");
        assert_eq!(
            both.to_string(),
            "error[SPX-T001]: bad thing at src/a.spx:7:3"
        );

        let path_only = Diagnostic::io("SPX-I001", "cannot read").at_path("src/a.spx");
        assert_eq!(
            path_only.to_string(),
            "error[SPX-I001]: cannot read at src/a.spx"
        );

        let span_only = Diagnostic::warning("SPX-W001", "smell", span());
        assert_eq!(span_only.to_string(), "warning[SPX-W001]: smell at 7:3");

        let neither = Diagnostic::io("SPX-I002", "no location");
        assert_eq!(neither.to_string(), "error[SPX-I002]: no location");

        // Help is always the last line, after the location.
        let helped = Diagnostic::error("SPX-T001", "bad thing", span())
            .at_path("src/a.spx")
            .with_help("try the other spelling");
        assert_eq!(
            helped.to_string(),
            "error[SPX-T001]: bad thing at src/a.spx:7:3\n  help: try the other spelling"
        );
    }

    #[test]
    fn displayed_paths_escape_control_bytes() {
        // A path is attacker-influenced text that lands on a terminal. Raw
        // escape or newline bytes must not survive the rendering.
        let injected =
            Diagnostic::io("SPX-I001", "cannot read").at_path("src/\u{1b}[31mred\u{7f}\nnext.spx");
        let rendered = injected.to_string();
        assert!(!rendered.contains('\u{1b}'), "{rendered}");
        assert!(!rendered.contains('\u{7f}'), "{rendered}");
        assert!(!rendered.contains('\n'), "{rendered}");
        assert_eq!(
            rendered,
            "error[SPX-I001]: cannot read at src/\\u{1b}[31mred\\u{7f}\\nnext.spx"
        );

        // The message itself is not escaped, so the escaping is specific to
        // the path rendering and cannot be assumed elsewhere.
        assert!(Diagnostic::io("SPX-I001", "a\nb")
            .to_string()
            .contains('\n'));
    }

    #[test]
    fn json_uses_bare_nulls_for_absent_fields_and_a_fixed_key_order() {
        let bare = Diagnostic::io("SPX-I002", "no location");
        assert_eq!(
            bare.json(),
            "{\"code\":\"SPX-I002\",\"severity\":\"error\",\"message\":\"no location\",\
             \"path\":null,\"location\":null,\"help\":null}"
        );

        let full = Diagnostic::warning("SPX-W001", "smell", span())
            .at_path("src/a.spx")
            .with_help("hint");
        assert_eq!(
            full.json(),
            "{\"code\":\"SPX-W001\",\"severity\":\"warning\",\"message\":\"smell\",\
             \"path\":\"src/a.spx\",\
             \"location\":{\"line\":7,\"column\":3,\"start\":42,\"end\":51},\
             \"help\":\"hint\"}"
        );

        // Message, path and help all flow through the JSON escaper, so a
        // quotation mark cannot terminate the string early.
        let hostile = Diagnostic::io("SPX-I003", "say \"hi\"\n").at_path("a\\b.spx");
        assert!(hostile
            .json()
            .contains("\"message\":\"say \\\"hi\\\"\\n\",\"path\":\"a\\\\b.spx\""));
    }

    #[test]
    fn quote_json_escapes_every_control_character() {
        assert_eq!(quote_json(""), "\"\"");
        assert_eq!(quote_json("plain"), "\"plain\"");
        assert_eq!(
            quote_json("\"\\\n\r\t"),
            "\"\\\"\\\\\\n\\r\\t\"",
            "the five short escapes must stay short"
        );

        // Everything else that is a control character becomes a four-digit
        // `\u` escape, including DEL and the C1 range that a naive
        // `is_ascii_control` check would miss.
        assert_eq!(quote_json("\u{0}"), "\"\\u0000\"");
        assert_eq!(quote_json("\u{b}"), "\"\\u000b\"");
        assert_eq!(quote_json("\u{1b}"), "\"\\u001b\"");
        assert_eq!(quote_json("\u{7f}"), "\"\\u007f\"");
        assert_eq!(quote_json("\u{85}"), "\"\\u0085\"");
        // Backspace and form feed take the long form rather than \b and \f.
        assert_eq!(quote_json("\u{8}\u{c}"), "\"\\u0008\\u000c\"");

        // Non-control non-ASCII text is emitted literally as UTF-8.
        assert_eq!(quote_json("café €"), "\"café €\"");

        // No control character survives unescaped for any code point that a
        // path or message could carry.
        for code in 0u32..0x100 {
            let Some(character) = char::from_u32(code) else {
                continue;
            };
            let quoted = quote_json(&character.to_string());
            assert!(
                !quoted.chars().any(char::is_control),
                "U+{code:04X} left a raw control character"
            );
        }
    }

    #[test]
    fn quote_json_never_exceeds_the_active_output_budget() {
        // Under an exhausted budget the escaper stops writing and the budget
        // reports the overflow. The result may be a partial string, so
        // callers must fail closed on the flag rather than parse it; what it
        // may never do is spend more than it was given.
        for limit in 0..12usize {
            let (quoted, overflowed) = with_limit(limit, || quote_json("ab\ncd"));
            assert!(quoted.len() <= limit, "{limit}: {quoted:?}");
            // `"ab\ncd"` quotes to eight bytes.
            assert_eq!(overflowed, limit < 8, "{limit}: {quoted:?}");
            if !overflowed {
                assert_eq!(quoted, "\"ab\\ncd\"");
            }
        }
    }
}
