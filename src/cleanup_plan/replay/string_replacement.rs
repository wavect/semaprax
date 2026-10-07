//! Independent v16 successful replacement history validation.
use super::*;
pub(super) fn finish(
    function: &ResolvedFunction,
    target: LivenessFlagId,
    reserved: &[LivenessFlagId],
    state: &mut PathState,
) -> Result<(), Diagnostic> {
    if !reserved.contains(&target) || !state.live_order.contains(&target) {
        return Err(replay_error(
            function,
            "String replacement lost its reserved owner",
        ));
    }
    let mut prior = reserved
        .iter()
        .copied()
        .filter(|flag| *flag != target && state.live_order.contains(flag));
    let mut fresh = Vec::new();
    let mut next = prior.next();
    for flag in state
        .live_order
        .iter()
        .copied()
        .filter(|flag| *flag != target)
    {
        if let Some(expected) = next {
            if flag != expected {
                return Err(replay_error(
                    function,
                    "String replacement changed surviving owner history",
                ));
            }
            next = prior.next();
        } else {
            if reserved.contains(&flag) {
                return Err(replay_error(
                    function,
                    "String replacement repeats prior owner history",
                ));
            }
            fresh.push(flag);
        }
    }
    if next.is_some() {
        return Err(replay_error(
            function,
            "String replacement omits surviving owner history",
        ));
    }
    let mut published = Vec::new();
    for flag in reserved {
        if *flag == target || state.live_order.contains(flag) {
            published.push(*flag);
        }
    }
    published.extend(fresh);
    state.live_order = published;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    const SOURCE: &str = r#"
module test.string_replacement_replay;
@id("renew.helper") fn rebuild(text: own string) -> string { string_concat(text,"!") }
@id("renew.literal") fn literal() -> i64 { let mut text="old"; let held="held"; text="new"; string_len(text)+string_len(held) }
@id("renew.call") fn called() -> i64 { let mut text="a"; text=rebuild(text); string_len(text) }
@id("renew.branch") fn branched() -> i64 { let mut text="a"; text=if true { rebuild(text) } else { "new" }; string_len(text) }
@id("renew.nested") fn nested() -> i64 { let mut text="a"; let mut other="b"; text={ text="x"; other="y"; string_concat(text,"!") }; string_len(text)+string_len(other) }
@id("renew.loop") fn repeated() -> i64 { let mut text="a"; let held="held"; let mut i=0; while i<3 { text=if i==1 { rebuild(text) } else { "b" }; i=i+1; 0 } string_len(text)+string_len(held) }
"#;

    #[test]
    fn string_replacement_builder_and_independent_replay_agree() {
        let parsed = crate::parse(SOURCE, Path::new("string-replacement-replay.spx")).unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        for function in program
            .functions
            .iter()
            .filter(|f| crate::string_ops::replacement::requires(f))
        {
            validate_structure(&program, function).unwrap();
            crate::cleanup_plan::build::assert_expression_lowering_oracle(
                &program,
                function,
                &function.body,
            );
        }
    }

    #[test]
    fn string_replacement_replay_rejects_missing_reservation_transfer_and_alias() {
        let program = crate::hir::resolve(
            &crate::parse(SOURCE, Path::new("string-replacement-hostile.spx")).unwrap(),
        )
        .unwrap();
        let original = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "renew.literal")
            .unwrap();
        validate_structure(&program, original).unwrap();
        for mutation in 0..4 {
            let mut function = original.clone();
            match mutation {
                0 => function.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V15,
                1 => {
                    for block in &mut function.cleanup_plan.blocks {
                        block.transitions.retain(|transition| {
                            !matches!(transition, CleanupTransition::ReserveRenewal { .. })
                        });
                    }
                }
                2 => {
                    for transition in function
                        .cleanup_plan
                        .blocks
                        .iter_mut()
                        .flat_map(|b| &mut b.transitions)
                    {
                        if let CleanupTransition::Renew {
                            at,
                            source,
                            destination,
                        } = transition
                        {
                            *transition = CleanupTransition::Transfer {
                                at: at.clone(),
                                source: source.clone(),
                                destination: destination.clone(),
                            };
                        }
                    }
                }
                3 => {
                    for transition in function
                        .cleanup_plan
                        .blocks
                        .iter_mut()
                        .flat_map(|b| &mut b.transitions)
                    {
                        if let CleanupTransition::Renew {
                            source,
                            destination,
                            ..
                        } = transition
                        {
                            *source = destination.clone();
                        }
                    }
                }
                _ => unreachable!(),
            }
            assert_eq!(
                validate_structure(&program, &function).unwrap_err().code,
                "SPX-H006",
                "mutation {mutation}"
            );
        }
    }
}
