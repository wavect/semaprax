//! Private shadow-stack exclusion for the closed owned-data export profiles.
//!
//! Reuse the lowerer's exact per-function frame sizes and the shared validated
//! HIR call index. Sibling calls reuse stack space; only simultaneous frames
//! on a call path are added. No backend-specific expression walk guesses calls.

use std::collections::{BTreeMap, BTreeSet};

use super::{error, Diagnostic, FunctionPlan, ResolvedProgram, VariantLayoutCache};
use crate::hir::DeclarationId;

pub(super) fn derive(
    program: &ResolvedProgram,
    layouts: &VariantLayoutCache,
    roots: &[DeclarationId],
) -> Result<BTreeMap<DeclarationId, u32>, Diagnostic> {
    if program.functions.len() > crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
        return Err(error(
            "owned-data stack inventory is outside the monomorphic bound",
        ));
    }
    let calls = crate::call_index::PersistentCallIndex::build(program)?;
    // Public admission is selected-closure-only. Unrelated recursive or generic
    // declarations must not acquire a new rejection through this analysis.
    let mut reachable = BTreeSet::new();
    let mut pending = roots.to_vec();
    while let Some(owner) = pending.pop() {
        if !reachable.insert(owner.clone()) {
            continue;
        }
        if reachable.len() > crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
            return Err(error("owned-data stack closure exceeds its bound"));
        }
        let callees = calls
            .calls_by_owner()
            .get(&owner)
            .ok_or_else(|| error("owned-data stack root is absent"))?;
        pending.extend(
            callees
                .iter()
                .filter(|callee| !reachable.contains(*callee))
                .cloned(),
        );
    }
    let frames = program
        .functions
        .iter()
        .filter(|function| reachable.contains(&function.id))
        .map(|function| {
            Ok((
                function.id.clone(),
                FunctionPlan::build(program, function, layouts)?.frame_size,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Diagnostic>>()?;
    let selected_calls = calls
        .calls_by_owner()
        .iter()
        .filter(|(owner, _)| reachable.contains(*owner))
        .map(|(owner, callees)| (owner.clone(), callees.clone()))
        .collect();
    longest_paths(&frames, &selected_calls)
}

fn longest_paths(
    frames: &BTreeMap<DeclarationId, u32>,
    calls: &BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
) -> Result<BTreeMap<DeclarationId, u32>, Diagnostic> {
    if frames.len() != calls.len()
        || calls.iter().any(|(owner, callees)| {
            !frames.contains_key(owner) || callees.iter().any(|callee| !frames.contains_key(callee))
        })
    {
        return Err(error("owned-data stack call inventory is incomplete"));
    }
    let mut extents: BTreeMap<DeclarationId, u32> = BTreeMap::new();
    // At most one pass per admitted function. This avoids recursion and fails
    // closed if no leaf can be resolved (including a self-edge or cycle).
    while extents.len() != frames.len() {
        let before = extents.len();
        for (owner, frame) in frames {
            if extents.contains_key(owner) {
                continue;
            }
            let callees = &calls[owner];
            if callees.iter().any(|callee| !extents.contains_key(callee)) {
                continue;
            }
            let child = callees
                .iter()
                .map(|callee| extents[callee])
                .max()
                .unwrap_or(0);
            let extent = frame
                .checked_add(child)
                .ok_or_else(|| error("owned-data stack extent overflows"))?;
            extents.insert(owner.clone(), extent);
        }
        if extents.len() == before {
            return Err(error("owned-data stack call graph is cyclic"));
        }
    }
    Ok(extents)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod runtime;

    fn id(value: &str) -> DeclarationId {
        DeclarationId::new(value)
    }

    #[test]
    fn extent_adds_nested_frames_but_not_sibling_frames() {
        let frames = BTreeMap::from([(id("a"), 8), (id("b"), 16), (id("c"), 32)]);
        let mut calls = BTreeMap::from([
            (id("a"), BTreeSet::from([id("b"), id("c")])),
            (id("b"), BTreeSet::new()),
            (id("c"), BTreeSet::new()),
        ]);
        assert_eq!(longest_paths(&frames, &calls).unwrap()[&id("a")], 40);
        calls.get_mut(&id("b")).unwrap().insert(id("c"));
        assert_eq!(longest_paths(&frames, &calls).unwrap()[&id("a")], 56);
    }

    #[test]
    fn missing_cyclic_and_overflowing_extents_are_never_zero_fallbacks() {
        let frames = BTreeMap::from([(id("a"), u32::MAX), (id("b"), 1)]);
        let mut calls = BTreeMap::from([
            (id("a"), BTreeSet::from([id("b")])),
            (id("b"), BTreeSet::new()),
        ]);
        assert!(longest_paths(&frames, &calls).is_err());
        calls.get_mut(&id("b")).unwrap().insert(id("a"));
        assert!(longest_paths(&frames, &calls).is_err());
        calls.get_mut(&id("b")).unwrap().insert(id("missing"));
        assert!(longest_paths(&frames, &calls).is_err());
        calls.remove(&id("b"));
        assert!(longest_paths(&frames, &calls).is_err());
    }

    #[test]
    fn unrelated_recursive_declaration_does_not_change_selected_extent() {
        let source = r#"module test.stack_selection;
@id("stack.selected") fn selected() -> i64 { 7 }
@id("stack.unrelated") fn unrelated(value: i64) -> i64 { unrelated(value) }
@id("stack.main") fn main() -> i64 { 0 }
"#;
        let program =
            crate::hir::resolve(&crate::check(source, "stack-selection.spx").unwrap()).unwrap();
        let layouts =
            VariantLayoutCache::build(&program, crate::variant_layout::VariantTarget::Wasm32)
                .unwrap();
        let selected = derive(&program, &layouts, &[id("stack.selected")]).unwrap();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[&id("stack.selected")], 0);
        assert!(derive(&program, &layouts, &[id("stack.unrelated")]).is_err());
    }
}
