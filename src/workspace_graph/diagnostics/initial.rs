//! Initial constructor failures retain four complete-field decisions, never a
//! partially formatted String. Decisions debit the REAL original parent once.
//! The initial inline carrier/context controls are UNFUNDED caller-stack
//! storage: an unacquired owner cannot certify them or any later output.
use super::*;
use crate::bounded_output::{format_acceptance as acceptance, ReservationRefusal, ReservationSite};
use std::fmt::{self, Write};
mod context;
use context::Context;

#[derive(Clone, Copy)]
enum Help {
    None,
    Base,
    Evidence,
    Combined,
}
#[derive(Clone, Copy)]
pub(crate) enum Message {
    Builder {
        complete: bool,
        maximum: usize,
    },
    Fixed(&'static str),
    Change {
        complete: bool,
        maximum: usize,
    },
    Structural {
        complete: bool,
        field: &'static str,
        maximum: usize,
    },
    Operations {
        field: &'static str,
        maximum: usize,
    },
}
pub(crate) struct Captured {
    message: Message,
    context: Option<Context>,
    help: Help,
    first: Option<ReservationRefusal>,
    site: Option<ReservationSite>,
}
fn evidence(out: &mut dyn Write, first: Option<ReservationRefusal>) -> fmt::Result {
    match first {
        Some(first) => write!(out,
            "resolved-core phase first sticky ledger refusal: requested {} bytes with {} remaining and {} reserved floor. Authenticated internal stage: {}. These are cumulative reservation bytes, not a retained-memory forecast or process RSS; the exact inner operation is unknown.",
            first.requested, first.remaining, first.floor,
            first.stage.map_or("unknown", |stage| stage.label())),
        None => out.write_str("resolved-core phase refused; the first reservation and exact inner operation are unknown."),
    }
}
struct Evidence(Option<ReservationRefusal>);
impl fmt::Display for Evidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        evidence(f, self.0)
    }
}
impl Captured {
    pub(in crate::workspace_graph) fn limit(maximum: usize) -> Self {
        Self::capture(maximum, None, None, false)
    }
    pub(in crate::workspace_graph) fn live(
        maximum: usize,
        first: Option<ReservationRefusal>,
        site: Option<ReservationSite>,
    ) -> Self {
        Self::capture(maximum, first, site, true)
    }
    fn capture(
        maximum: usize,
        first: Option<ReservationRefusal>,
        site: Option<ReservationSite>,
        live: bool,
    ) -> Self {
        let field = "builder_bytes";
        let message = acceptance::formatted(format_args!(
            "Workspace Semantic Graph `{field}` exceeds {maximum}"
        ));
        let base = acceptance::formatted(format_args!("{}", BorrowedBuilderHelp(maximum)));
        let mut help = if base.complete && base.bytes != 0 {
            Help::Base
        } else {
            Help::None
        };
        if live {
            let suffix = acceptance::formatted(format_args!("{}", Evidence(first)));
            if suffix.complete && suffix.bytes != 0 {
                if matches!(help, Help::Base) {
                    let combined = acceptance::joined(base.bytes, suffix.bytes);
                    if combined.complete && combined.bytes != 0 {
                        help = Help::Combined;
                    }
                } else {
                    help = Help::Evidence;
                }
            }
        }
        let site = if live && crate::bounded_output::active_remaining().is_none() && first.is_some()
        {
            site
        } else {
            None
        };
        // Move only after every original field/counter reservation has run.
        Self {
            message: Message::Builder {
                complete: message.complete,
                maximum,
            },
            context: Some(Context::take()),
            help,
            first,
            site,
        }
    }
    pub(in crate::workspace_graph) fn fixed(message: &'static str) -> Self {
        Self {
            message: Message::Fixed(message),
            context: None,
            help: Help::None,
            first: None,
            site: None,
        }
    }
    pub(crate) fn code(&self) -> &'static str {
        match self.message {
            Message::Builder { .. } => "SPX-G171",
            Message::Fixed(_) => "SPX-G173",
            Message::Change { .. } => "SPX-G183",
            Message::Structural { .. } => "SPX-G191",
            Message::Operations { .. } => "SPX-G199",
        }
    }
    pub(crate) fn write_message(&self, out: &mut dyn Write) -> fmt::Result {
        match self.message {
            Message::Builder {
                complete: true,
                maximum,
            } => write!(
                out,
                "Workspace Semantic Graph `builder_bytes` exceeds {maximum}"
            ),
            Message::Fixed(message) => out.write_str(message),
            Message::Change {
                complete: true,
                maximum,
            } => write!(
                out,
                "Semantic Workspace Change `total_analysis_builder_bytes` exceeds {maximum}"
            ),
            Message::Structural {
                complete: true,
                field,
                maximum,
            } => write!(
                out,
                "Semantic Workspace Structural Change limit exceeded: {field} maximum {maximum}"
            ),
            Message::Operations { field, maximum } => write!(
                out,
                "Semantic Workspace Operations exceeds {field} maximum {maximum}"
            ),
            _ => Ok(()),
        }
    }
    pub(crate) fn has_help(&self) -> bool {
        !matches!(self.help, Help::None)
    }
    pub(crate) fn write_help(&self, out: &mut dyn Write) -> fmt::Result {
        let maximum = match self.message {
            Message::Builder { maximum, .. } => maximum,
            _ => return Ok(()),
        };
        match self.help {
            Help::None => return Ok(()),
            Help::Base => self
                .context
                .as_ref()
                .expect("held base help")
                .write_help(maximum, out)?,
            Help::Evidence => evidence(out, self.first)?,
            Help::Combined => {
                self.context
                    .as_ref()
                    .expect("held base help")
                    .write_help(maximum, out)?;
                out.write_str(" ")?;
                evidence(out, self.first)?;
            }
        }
        if let Some(site) = self.site {
            write!(
                out,
                " Reservation caller: {}:{}:{}.",
                site.file(),
                site.line(),
                site.column()
            )?;
        }
        Ok(())
    }
    /// Explicit ordinary caller IO allocation. No TLS or original-payer access.
    pub(crate) fn materialize_unfunded(&self) -> Diagnostic {
        let mut message = String::new();
        self.write_message(&mut message)
            .expect("writing owned String");
        let mut diagnostic = Diagnostic::io(self.code(), message);
        if self.has_help() {
            let mut help = String::new();
            self.write_help(&mut help).expect("writing owned String");
            diagnostic.help = Some(help);
        }
        diagnostic
    }
    pub(crate) fn write_human(&self, out: &mut dyn Write) -> fmt::Result {
        write!(out, "error[{}]: ", self.code())?;
        self.write_message(out)?;
        if self.has_help() {
            out.write_str("\n  help: ")?;
            self.write_help(out)?;
        }
        Ok(())
    }
}

impl Captured {
    pub(crate) fn write_json(&self, out: &mut impl fmt::Write) -> fmt::Result {
        // Captured fields never re-query their original payer or TLS. Bounded
        // JSON quoting remains the ordinary CURRENT output policy, exactly as
        // Diagnostic::write_json; materialization is explicit caller IO here.
        if crate::bounded_output::active_limit().is_some() {
            return self.materialize_unfunded().write_json(out);
        }
        write!(
            out,
            "{{\"code\":\"{}\",\"severity\":\"error\",\"message\":\"",
            self.code()
        )?;
        self.write_message(&mut Escaped(out))?;
        out.write_str("\",\"path\":null,\"location\":null,\"help\":")?;
        if self.has_help() {
            out.write_char('"')?;
            self.write_help(&mut Escaped(out))?;
            out.write_char('"')?;
        } else {
            out.write_str("null")?;
        }
        out.write_char('}')
    }
}
struct Escaped<'a, W: fmt::Write>(&'a mut W);
impl<W: fmt::Write> fmt::Write for Escaped<'_, W> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        for value in value.chars() {
            match value {
                '"' => self.0.write_str("\\\"")?,
                '\\' => self.0.write_str("\\\\")?,
                '\n' => self.0.write_str("\\n")?,
                '\r' => self.0.write_str("\\r")?,
                '\t' => self.0.write_str("\\t")?,
                value if value.is_control() => write!(self.0, "\\u{:04x}", value as u32)?,
                value => self.0.write_char(value)?,
            }
        }
        Ok(())
    }
}

impl Captured {
    fn forget_help(&mut self) {
        self.context = None;
        self.help = Help::None;
        self.first = None;
        self.site = None;
    }
    pub(crate) fn map_change(&mut self, internal: usize, maximum: usize) {
        // Legacy ALWAYS formats expected first, even when the code differs.
        let expected = acceptance::formatted(format_args!(
            "Workspace Semantic Graph `change_builder_bytes` exceeds {internal}"
        ));
        // Initial Builder uses builder_bytes; complete intended strings never
        // match change_builder_bytes. Two failed empty Strings DO match.
        if matches!(
            self.message,
            Message::Builder {
                complete: false,
                ..
            }
        ) && !expected.complete
        {
            let field = "total_analysis_builder_bytes";
            let replacement = acceptance::formatted(format_args!(
                "Semantic Workspace Change `{field}` exceeds {maximum}"
            ));
            self.forget_help();
            self.message = Message::Change {
                complete: replacement.complete,
                maximum,
            };
        }
    }
    pub(crate) fn map_structural(
        &mut self,
        internal: usize,
        candidate_maximum: usize,
        analysis_maximum: usize,
    ) {
        let expected = acceptance::formatted(format_args!(
            "Workspace Semantic Graph `change_builder_bytes` exceeds {internal}"
        ));
        if matches!(
            self.message,
            Message::Builder {
                complete: false,
                ..
            }
        ) && !expected.complete
        {
            let (field, maximum) = if internal == candidate_maximum {
                ("candidate_graph_builder_bytes", candidate_maximum)
            } else {
                ("analysis_builder_bytes", analysis_maximum)
            };
            let replacement = acceptance::formatted(format_args!(
                "Semantic Workspace Structural Change limit exceeded: {field} maximum {maximum}"
            ));
            self.forget_help();
            self.message = Message::Structural {
                complete: replacement.complete,
                field,
                maximum,
            };
        }
    }
    pub(crate) fn map_operations(
        &mut self,
        internal: usize,
        candidate_maximum: usize,
        operations_maximum: usize,
    ) {
        // Both expected and replacement in the original operations mapper use
        // ordinary format!, not budgeted_format. Empty refused G171 cannot
        // match those nonempty strings; do not invent the other mappers' rule.
        if matches!(self.message, Message::Builder { complete: true, maximum } if maximum == internal)
        {
            let (field, maximum) = if internal == candidate_maximum {
                ("candidate_graph_builder_bytes", candidate_maximum)
            } else {
                ("operations_builder_bytes", operations_maximum)
            };
            self.forget_help();
            self.message = Message::Operations { field, maximum };
        }
    }
}

#[cfg(test)]
mod tests;
