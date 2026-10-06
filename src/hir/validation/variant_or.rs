//! Validation of or-patterns over payload-free variant cases.
use super::*;

/// Authenticates one or-pattern arm of a variant match and records the cases
/// it covers. The arm must belong to a plain value match, follow no wildcard,
/// and list only payload-free cases of `variant` not covered by an earlier arm.
pub(super) fn cover_variant_or_pattern(
    alternatives: &[ResolvedMatchPattern],
    variant: &DeclarationId,
    cases: &[ResolvedVariantCaseDeclaration],
    mode: ResolvedMatchMode,
    wildcard_seen: bool,
    covered: &mut BTreeSet<DeclarationId>,
) -> Result<(), Diagnostic> {
    if mode != ResolvedMatchMode::Value || wildcard_seen || alternatives.is_empty() {
        return Err(hir_error(
            "resolved variant or-pattern is outside a plain reachable value match",
        ));
    }
    for alternative in alternatives {
        let ResolvedMatchPattern::Variant {
            variant: pattern_variant,
            case,
            fields,
        } = alternative
        else {
            return Err(hir_error(
                "resolved variant or-pattern contains a non-case alternative",
            ));
        };
        let payload_free = cases
            .iter()
            .any(|item| item.id == *case && item.fields.is_empty());
        if pattern_variant != variant
            || !fields.is_empty()
            || !payload_free
            || !covered.insert(case.clone())
        {
            return Err(hir_error(
                "resolved variant or-pattern has a foreign, payload, or duplicate case",
            ));
        }
    }
    Ok(())
}
