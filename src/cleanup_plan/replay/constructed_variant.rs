use super::*;

pub(super) fn materialize_constructed_variant(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    state: &mut PathState,
    source: &CleanupPlace,
    variant: &DeclarationId,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
) -> Result<(), Diagnostic> {
    let StorageId::Temporary(expression) = &source.storage else {
        return Err(replay_error(
            function,
            "variant transfer has no authenticated conditional source state",
        ));
    };
    let expression = find_resolved_expression(function, expression).ok_or_else(|| {
        replay_error(
            function,
            "variant transfer source temporary has no typed-HIR expression",
        )
    })?;
    let ResolvedExprKind::ConstructVariant {
        variant: constructed_variant,
        case,
        ..
    } = &expression.kind
    else {
        return Err(replay_error(
            function,
            "variant transfer lacks a conditional or constructed source",
        ));
    };
    if constructed_variant != variant
        || program
            .declarations
            .variant_cases(variant)
            .is_none_or(|cases| !cases.iter().any(|candidate| candidate.id == *case))
    {
        return Err(replay_error(
            function,
            "constructed variant transfer has unauthenticated case metadata",
        ));
    }
    let all_flags = validate_place(function, source, storage, leaves)?;
    let prefix = source
        .projections
        .iter()
        .chain(std::iter::once(case))
        .cloned()
        .collect::<Vec<_>>();
    let selected = all_flags
        .iter()
        .filter(|flag| leaves[flag].place.projections.starts_with(&prefix))
        .copied()
        .collect::<Vec<_>>();
    if selected.iter().any(|flag| !state.live_order.contains(flag))
        || all_flags.iter().any(|flag| {
            state.live_order.contains(flag) && !leaves[flag].place.projections.starts_with(&prefix)
        })
    {
        return Err(replay_error(
            function,
            "constructed variant transfer has incomplete or inactive payload liveness",
        ));
    }
    let selected_set = selected.iter().copied().collect::<BTreeSet<_>>();
    state.live_order.retain(|flag| !selected_set.contains(flag));
    state.conditional_variants.push(ReplayConditionalVariant {
        root: source.clone(),
        variant: variant.clone(),
        // Rebuild every tag, but retain cleanup obligations only for the
        // authenticated constructor. Match edges derive their own exact
        // payload inventories after tag authentication.
        cases: program
            .declarations
            .variant_cases(variant)
            .ok_or_else(|| replay_error(function, "constructed owning variant has no case domain"))?
            .iter()
            .map(|candidate| {
                let prefix = source.projected(candidate.id.clone()).projections;
                (
                    candidate.id.clone(),
                    all_flags
                        .iter()
                        .filter(|flag| {
                            candidate.id == *case
                                && leaves[flag].place.projections.starts_with(&prefix)
                        })
                        .copied()
                        .collect(),
                )
            })
            .collect(),
    });
    Ok(())
}
