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
