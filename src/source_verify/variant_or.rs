//! Or-patterns over payload-free variant cases.
//!
//! `Status::Todo {} | Status::Doing {} => …` is admitted in a plain value
//! `match` over a variant when every alternative names a payload-free case of
//! the scrutinee's own variant. Each alternative covers its case for
//! exhaustiveness exactly like a one-case arm. The iterative verifier and the
//! recursive oracle both call [`check_variant_or_pattern`], so their
//! diagnostics stay byte-identical.

use super::diagnostics::error;
use crate::ast::{MatchMode, MatchPattern, Program, Span, VariantCaseDeclaration};
use crate::diagnostic::Diagnostic;

/// Help attached to every refutable-construct rejection over a record or
/// variant scrutinee, naming the forms that are admitted there.
pub(super) const AGGREGATE_REFUTABLE_HELP: &str =
    "over a record or variant scrutinee, arms admit case patterns, `_`, and `|` between \
     payload-free cases of the scrutinee's variant (`Status::Todo {} | Status::Doing {} => ...`); \
     literal patterns, bindings, and guards need an i64/i32/u8/char/bool scrutinee";

/// The scrutinee facts and arm-sequence state one or-pattern arm is checked
/// against.
pub(super) struct VariantOrContext<'a> {
    pub(super) variant_name: Option<&'a str>,
    pub(super) declared_cases: Option<&'a [VariantCaseDeclaration]>,
    pub(super) mode: MatchMode,
    pub(super) wildcard_seen: bool,
}

/// Checks one or-pattern arm of a variant match. `cover` records a case as
/// covered and reports whether it was new.
pub(super) fn check_variant_or_pattern<'p>(
    program: &Program,
    alternatives: &'p [MatchPattern],
    span: Span,
    context: &VariantOrContext<'_>,
    cover: &mut dyn FnMut(&'p str) -> bool,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !alternatives
        .iter()
        .all(|alternative| matches!(alternative, MatchPattern::Variant { .. }))
    {
        diagnostics.push(
            error(
                program,
                "SPX-T254",
                "or-pattern alternatives over a variant scrutinee must all be payload-free \
                 cases of its type",
                span,
            )
            .with_help(AGGREGATE_REFUTABLE_HELP),
        );
        return;
    }
    if context.mode != MatchMode::Value {
        diagnostics.push(error(
            program,
            "SPX-O117",
            "or-patterns are admitted only in a plain value `match`; explicit ownership \
             variant matches need one case pattern per arm",
            span,
        ));
    }
    for alternative in alternatives {
        let MatchPattern::Variant {
            type_name,
            case_name,
            fields,
            span,
            ..
        } = alternative
        else {
            continue;
        };
        let compatible = context.variant_name == Some(type_name.as_str());
        let declared_case = compatible
            .then_some(context.declared_cases)
            .flatten()
            .and_then(|cases| cases.iter().find(|case| case.name == *case_name));
        let Some(declared_case) = declared_case else {
            let diagnostic = error(
                program,
                "SPX-M103",
                format!(
                    "pattern `{type_name}::{case_name}` is incompatible with the match scrutinee"
                ),
                *span,
            );
            diagnostics.push(
                match compatible
                    .then_some(context.declared_cases)
                    .flatten()
                    .and_then(|cases| super::hints::nearest_variant_case_name(case_name, cases))
                {
                    Some(nearest) => diagnostic
                        .with_help(format!("did you mean `{type_name}::{nearest} {{ ... }}`?")),
                    None => diagnostic,
                },
            );
            continue;
        };
        if context.wildcard_seen || !cover(case_name) {
            diagnostics.push(error(
                program,
                "SPX-M102",
                format!("unreachable duplicate case `{type_name}::{case_name}`"),
                *span,
            ));
        }
        if !fields.is_empty() || !declared_case.fields.is_empty() {
            diagnostics.push(
                error(
                    program,
                    "SPX-M105",
                    format!(
                        "or-pattern alternative `{type_name}::{case_name}` must be a \
                         payload-free case without bindings"
                    ),
                    *span,
                )
                .with_help("match a case that carries a payload in an arm of its own"),
            );
        }
    }
}
