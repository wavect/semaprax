#![allow(
    clippy::duplicate_mod,
    reason = "The complete fixture intentionally compiles independently against current and predecessor owner types."
)]
//! Independent pre-change copied-prefix oracle; never runtime authority.
use super::{
    validate_control_group, BTreeMap, BTreeSet, BlockId, Diagnostic, LivenessFlagId, PathState,
    ResolvedFunction,
};
fn function() -> ResolvedFunction {
    let parsed =
        crate::check("module joins; @id(\"main\") fn main()->i64 {0}", "join.spx").unwrap();
    crate::hir::resolve(&parsed).unwrap().functions.remove(0)
}
fn state(history: &[u32]) -> PathState {
    PathState {
        renewals: BTreeMap::new(),
        live_order: history.iter().map(|flag| LivenessFlagId(*flag)).collect(),
        conditional_variants: Vec::new(),
        pending_failure: None,
        selected_failure: None,
        staged_copy_result: None,
        published: false,
        returned: None,
    }
}
fn copied_prefix(
    function: &ResolvedFunction,
    states: &BTreeSet<PathState>,
) -> Result<(), Diagnostic> {
    let mut checked = BTreeSet::new();
    for state in states {
        super::validate_join_compatibility(
            function,
            checked.iter(),
            state,
            BlockId(7),
            &mut super::ReplayBudget::new(),
        )?;
        checked.insert(state.clone());
    }
    Ok(())
}
fn same_outcome(function: &ResolvedFunction, states: &BTreeSet<PathState>) {
    let before = copied_prefix(function, states)
        .err()
        .map(|d| (d.code, d.message));
    let after = validate_control_group(function, states, BlockId(7))
        .err()
        .map(|d| (d.code, d.message));
    assert_eq!(after, before);
}
#[test]
fn borrowed_prefix_preserves_joint_history_cycle_and_every_sorted_prefix_diagnostic() {
    let function = function();
    let histories: &[&[u32]] = &[&[], &[1], &[2], &[1, 2], &[2, 3], &[3, 1], &[2, 1], &[1, 3]];
    for mask in 0..(1usize << histories.len()) {
        let states = histories
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1usize << *i) != 0)
            .map(|(_, history)| state(history))
            .collect();
        same_outcome(&function, &states);
    }
    let cycle = [state(&[1, 2]), state(&[2, 3]), state(&[3, 1])]
        .into_iter()
        .collect();
    let diagnostic = validate_control_group(&function, &cycle, BlockId(7)).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        format!("cleanup plan for function `{}` failed independent replay: cleanup join at block 7 has conflicting initialization histories", function.id)
    );
    // Every pair is compatible; omitting the full prefix would miss this cycle.
    for pair in [
        [&[1, 2][..], &[2, 3][..]],
        [&[2, 3][..], &[3, 1][..]],
        [&[1, 2][..], &[3, 1][..]],
    ] {
        let states = pair.into_iter().map(state).collect();
        validate_control_group(&function, &states, BlockId(7)).unwrap();
    }
}
#[test]
fn duplicate_states_and_incompatible_control_keep_exact_results_and_input_payloads() {
    let function = function();
    let original = state(&[4, 5]);
    let states = [original.clone(), original.clone()]
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(states.len(), 1);
    same_outcome(&function, &states);
    assert_eq!(states.first(), Some(&original));
    let mut foreign = original.clone();
    foreign.published = true;
    let states = [original.clone(), foreign].into_iter().collect();
    same_outcome(&function, &states);
    let diagnostic = validate_control_group(&function, &states, BlockId(7)).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        format!("cleanup plan for function `{}` failed independent replay: cleanup join at block 7 has incompatible control states", function.id)
    );
    assert!(states.contains(&original));
}

#[test]
fn boxed_prefix_keeps_every_history_union_and_control_disagreement() {
    let function = function();
    let histories: &[&[u32]] = &[&[], &[1], &[2], &[1, 2], &[2, 3], &[3, 1], &[2, 1], &[1, 3]];
    for mask in 0..(1usize << histories.len()) {
        let states: BTreeSet<_> = histories
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1usize << *i) != 0)
            .map(|(_, history)| state(history))
            .collect();
        let expected = copied_prefix(&function, &states).err();
        let boxed: BTreeSet<_> = states.into_iter().map(Box::new).collect();
        let actual = super::validate_control_group_with_budget(
            &function,
            &boxed,
            BlockId(7),
            &mut super::ReplayBudget::new(),
        )
        .err();
        assert_eq!(actual, expected);
    }
    let mut foreign = state(&[4, 5]);
    foreign.published = true;
    let raw: BTreeSet<_> = [state(&[4, 5]), foreign].into_iter().collect();
    let expected = copied_prefix(&function, &raw).unwrap_err();
    let boxed = raw.into_iter().map(Box::new).collect();
    let actual = super::validate_control_group_with_budget(
        &function,
        &boxed,
        BlockId(7),
        &mut super::ReplayBudget::new(),
    )
    .unwrap_err();
    assert_eq!(actual, expected);
}
