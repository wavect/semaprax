//! Or-patterns over payload-free variant cases.
//!
//! An arm `Status::Todo {} | Status::Doing {} => …` of a plain value match
//! over a variant resolves to one [`ResolvedMatchPattern::Or`] whose
//! alternatives are field-less [`ResolvedMatchPattern::Variant`] patterns of the
//! scrutinee's variant. Source verification has already checked every
//! alternative; resolution re-derives the exact case identities and fails
//! closed on anything else.

use crate::ast::{MatchArm, MatchPattern, Span};
use crate::diagnostic::Diagnostic;

use super::expr_nodes::ResolvedMatchPattern;
use super::ids::DeclarationId;
use super::nodes::{DeclarationKind, ResolvedMatchMode};
use super::Resolver;

/// Whether an aggregate match uses a construct only Copy-scalar scrutinees
/// admit: a guard, a literal or binding pattern, or an or-pattern that is not
/// a variant-case or-pattern.
pub(super) fn has_scalar_only_syntax(arms: &[MatchArm]) -> bool {
    arms.iter().any(|arm| {
        arm.guard.is_some()
            || matches!(
                &arm.pattern,
                MatchPattern::Literal { .. } | MatchPattern::Binding { .. }
            )
            || (matches!(&arm.pattern, MatchPattern::Or { .. }) && !arm.pattern.is_variant_or())
    })
}

impl Resolver<'_> {
    pub(super) fn resolve_variant_or_pattern(
        &self,
        matched_type: &DeclarationId,
        matched_kind: DeclarationKind,
        mode: ResolvedMatchMode,
        alternatives: &[MatchPattern],
        span: Span,
    ) -> Result<ResolvedMatchPattern, Diagnostic> {
        if matched_kind != DeclarationKind::Variant || mode != ResolvedMatchMode::Value {
            return Err(self.error(
                "SPX-H001",
                "variant or-pattern needs a plain value match over a variant",
                span,
            ));
        }
        let mut resolved = Vec::with_capacity(alternatives.len());
        for alternative in alternatives {
            let MatchPattern::Variant {
                case_name,
                fields,
                span,
                ..
            } = alternative
            else {
                return Err(self.error(
                    "SPX-H001",
                    "or-pattern alternative is not a case pattern",
                    alternative.span(),
                ));
            };
            let case = self
                .declarations
                .case_id(matched_type, case_name)
                .filter(|case| {
                    fields.is_empty()
                        && self
                            .declarations
                            .case_fields(case)
                            .is_some_and(<[_]>::is_empty)
                })
                .cloned()
                .ok_or_else(|| {
                    self.error(
                        "SPX-H001",
                        format!("unresolved payload-free case `{matched_type}::{case_name}`"),
                        *span,
                    )
                })?;
            resolved.push(ResolvedMatchPattern::Variant {
                variant: matched_type.clone(),
                case,
                fields: Vec::new(),
            });
        }
        Ok(ResolvedMatchPattern::Or(resolved))
    }
}
