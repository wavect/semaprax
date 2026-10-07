use crate::diagnostic::Diagnostic;

use super::ResolvedFunction;

pub(super) fn uses_authored_type(function: &ResolvedFunction, declaration: &str) -> Diagnostic {
    let mut diagnostic = super::link_error(format!(
        "workspace function `{}` uses authored type `{declaration}`, which is outside the Useful Data linker profile",
        function.id
    ));
    let span = function.body.span;
    if span.start < span.end && span.line != 0 && span.column != 0 {
        diagnostic.span = Some(span);
    }
    diagnostic
}
