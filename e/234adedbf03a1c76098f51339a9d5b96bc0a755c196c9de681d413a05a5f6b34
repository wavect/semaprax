//! Diagnostic output borrows caller storage; this is not a prepaid report owner.
//! Bounded logical output keeps the complete legacy JSON formatting route.
use super::Diagnostic;
use std::fmt;
use std::io;

impl Diagnostic {
    /// Write the exact compatibility JSON without owning an intermediate String
    /// on the ordinary unbounded output route. The caller owns output storage.
    pub fn write_json(&self, output: &mut (impl fmt::Write + ?Sized)) -> fmt::Result {
        if crate::bounded_output::active_limit().is_some() {
            // Legacy quoting pays path/help before code/severity/message. Its
            // partial writes and sticky/floor receipts cannot be reordered by
            // streaming fields. This explicit compatibility route still owns
            // its original Strings and does not certify their physical funding.
            return output.write_str(&self.json());
        }
        output.write_str("{\"code\":")?;
        quoted(output, self.code)?;
        output.write_str(",\"severity\":")?;
        quoted(output, self.severity.as_str())?;
        output.write_str(",\"message\":")?;
        quoted(output, &self.message)?;
        output.write_str(",\"path\":")?;
        match &self.path {
            Some(path) => quoted(output, path)?,
            None => output.write_str("null")?,
        }
        output.write_str(",\"location\":")?;
        match self.span {
            Some(span) => write!(
                output,
                "{{\"line\":{},\"column\":{},\"start\":{},\"end\":{}}}",
                span.line, span.column, span.start, span.end
            )?,
            None => output.write_str("null")?,
        }
        output.write_str(",\"help\":")?;
        match &self.help {
            Some(help) => quoted(output, help)?,
            None => output.write_str("null")?,
        }
        output.write_char('}')
    }

    /// Borrow every diagnostic field, using its original human Display contract.
    pub fn write_to(&self, output: &mut (impl fmt::Write + ?Sized), json: bool) -> fmt::Result {
        if json {
            self.write_json(output)
        } else {
            write!(output, "{self}")
        }
    }
}

fn quoted(output: &mut (impl fmt::Write + ?Sized), value: &str) -> fmt::Result {
    output.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => output.write_str("\\\"")?,
            '\\' => output.write_str("\\\\")?,
            '\n' => output.write_str("\\n")?,
            '\r' => output.write_str("\\r")?,
            '\t' => output.write_str("\\t")?,
            value if value.is_control() => write!(output, "\\u{:04x}", value as u32)?,
            value => output.write_char(value)?,
        }
    }
    output.write_char('"')
}

/// Preserve ordered diagnostics and exactly one final newline per diagnostic.
/// A failed writer stops here; callers retain their original compiler status.
pub fn write_lines(
    output: &mut (impl fmt::Write + ?Sized),
    diagnostics: &[Diagnostic],
    json: bool,
) -> fmt::Result {
    for diagnostic in diagnostics {
        diagnostic.write_to(output, json)?;
        output.write_char('\n')?;
    }
    Ok(())
}

/// Adapter to the caller's existing byte stream. No output buffer is acquired.
/// The first IO failure remains available; subsequent writes cannot replace it.
pub struct IoWriter<'a, W: io::Write + ?Sized> {
    output: &'a mut W,
    error: Option<io::Error>,
}
impl<'a, W: io::Write + ?Sized> IoWriter<'a, W> {
    pub fn new(output: &'a mut W) -> Self {
        Self {
            output,
            error: None,
        }
    }
    pub fn error(&self) -> Option<&io::Error> {
        self.error.as_ref()
    }
}
impl<W: io::Write + ?Sized> fmt::Write for IoWriter<'_, W> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.error.is_some() {
            return Err(fmt::Error);
        }
        match self.output.write_all(value.as_bytes()) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.error = Some(error);
                Err(fmt::Error)
            }
        }
    }
}

#[cfg(test)]
mod tests;
