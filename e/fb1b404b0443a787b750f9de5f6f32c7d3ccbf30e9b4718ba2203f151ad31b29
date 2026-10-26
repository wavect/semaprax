//! Borrowed first-site presentation. Output belongs to the ordinary caller;
//! this module acquires no String, report slot, ledger or filename authority.
use super::*;
use crate::bounded_output::{active_limit, ReservationSite};
use std::fmt::Write as _;

struct Help<'a> {
    base: Option<&'a str>,
    site: ReservationSite,
}
impl fmt::Display for Help<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(base) = self.base {
            out.write_str(base)?;
        }
        write!(
            out,
            " Reservation caller: {}:{}:{}.",
            self.site.file(),
            self.site.line(),
            self.site.column()
        )
    }
}
fn write_one(
    diagnostic: &Diagnostic,
    site: ReservationSite,
    output: &mut dyn fmt::Write,
    json: bool,
) -> fmt::Result {
    if active_limit().is_some() {
        return diagnostic.write_to(output, json);
    }
    let help = Help {
        base: diagnostic.help.as_deref(),
        site,
    };
    if json {
        diagnostic.write_json_with_help(output, &help)
    } else {
        diagnostic.write_to(output, false)?;
        if diagnostic.help.is_none() {
            output.write_str("\n  help:")?;
        }
        write!(output, "{}", Help { base: None, site })
    }
}
pub(super) fn write_lines(
    report: &[Diagnostic],
    site: ReservationSite,
    output: &mut impl fmt::Write,
    json: bool,
) -> fmt::Result {
    for (index, diagnostic) in report.iter().enumerate() {
        if index == 0 {
            write_one(diagnostic, site, output, json)?;
        } else {
            diagnostic.write_to(output, json)?;
        }
        output.write_char('\n')?;
    }
    Ok(())
}
pub(super) fn display(
    report: &[Diagnostic],
    site: ReservationSite,
    output: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    for (index, diagnostic) in report.iter().enumerate() {
        if index != 0 {
            output.write_char('\n')?;
        }
        if index == 0 {
            write_one(diagnostic, site, output, false)?;
        } else {
            diagnostic.write_to(output, false)?;
        }
    }
    Ok(())
}

// Static report/formatting carrier census only, not a tariff for rendering
// execution. Caller-owned output still funds its own sink and traversal. There is
// no guessed maximum filename length or post-refusal owned text acquisition.
pub(super) fn controls() -> usize {
    use std::mem::size_of;
    size_of::<super::Owner>()
        + 4 * size_of::<WorkspaceReportFailure>()
        + 4 * size_of::<Option<ReservationSite>>()
        + 2 * size_of::<ReservationSite>()
        + 2 * size_of::<Option<crate::bounded_output::ReservationRefusal>>()
        + 4 * size_of::<&WorkspaceReportFailure>()
        + 2 * size_of::<&[Diagnostic]>()
        + 2 * size_of::<Help<'_>>()
        + 2 * size_of::<&Help<'_>>()
        + 4 * size_of::<&Diagnostic>()
        + 4 * size_of::<&mut dyn fmt::Write>()
        + 4 * size_of::<fmt::Result>()
        + 3 * size_of::<fmt::Arguments<'_>>()
        + size_of::<&'static std::panic::Location<'static>>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<u32>()
        + 4 * size_of::<bool>()
        + crate::diagnostic::writer::reservation_site_controls()
}
#[cfg(test)]
mod tests;
