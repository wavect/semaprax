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
    derive_weighted(program, layouts, roots, |plan| Ok(plan.frame_size))
}

/// Every authenticated Bytes cleanup leaf and private String owner cell may
/// retain an owner for one activation. String cells are also used for physical
/// settlement; unlike Strings, Bytes use the resource CleanupPlan authority.
/// Add simultaneous activations, not sibling calls or loop iterations. One
/// extra slot covers the synchronous mint-to-initialize/result handoff.
/// This is conservative storage accounting, not exact liveness or heap bytes.
pub(super) fn arena_capacity(
    program: &ResolvedProgram,
    layouts: &VariantLayoutCache,
    roots: &[DeclarationId],
) -> Result<u32, Diagnostic> {
    let extents = derive_weighted(program, layouts, roots, |plan| {
        let count = plan
            .cleanup_place_flags
            .len()
            .checked_add(plan.owned_strings.owners.len())
            .ok_or_else(|| error("owned-data arena cleanup inventory overflows"))?;
        u32::try_from(count).map_err(|_| error("owned-data arena cleanup inventory overflows"))
    })?;
    let maximum = roots.iter().try_fold(0, |maximum, root| {
        extents
            .get(root)
            .map(|extent| maximum.max(*extent))
            .ok_or_else(|| error("owned-data arena root is absent"))
    })?;
    checked_capacity(maximum)
}

fn checked_capacity(maximum: u32) -> Result<u32, Diagnostic> {
    maximum
        .checked_add(1)
        .filter(|capacity| *capacity <= 0x7fff_ffff)
        .ok_or_else(|| error("owned-data arena exceeds token representability"))
}

fn derive_weighted(
    program: &ResolvedProgram,
    layouts: &VariantLayoutCache,
    roots: &[DeclarationId],
    weight: impl Fn(&FunctionPlan) -> Result<u32, Diagnostic>,
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
                weight(&FunctionPlan::build(program, function, layouts)?)?,
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

pub(super) fn longest_paths(
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

    #[test]
    fn arena_capacity_has_one_transient_slot_and_checked_token_boundary() {
        assert_eq!(checked_capacity(0).unwrap(), 1);
        assert_eq!(checked_capacity(17).unwrap(), 18);
        assert_eq!(checked_capacity(0x7fff_fffe).unwrap(), 0x7fff_ffff);
        for rejected in [0x7fff_ffff, u32::MAX] {
            let diagnostic = checked_capacity(rejected).unwrap_err();
            assert_eq!(
                diagnostic.message,
                "owned-data arena exceeds token representability"
            );
        }
    }

    #[test]
    fn arena_counts_cleanup_leaves_not_frame_bytes_or_unrelated_functions() {
        let source = r#"module test.arena;
@id("arena.leaf") fn leaf(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }
@id("arena.root") fn root(input: borrow Slice<u8>) -> Bytes {
    let kept = bytes_copy(input);
    let returned = leaf(input);
    let count = byte_len(bytes_as_slice(kept));
    returned
}
@id("arena.unrelated") fn unrelated(value: i64) -> i64 { unrelated(value) }
@id("arena.main") fn main() -> i64 { 0 }
"#;
        let program = crate::hir::resolve(&crate::check(source, "arena.spx").unwrap()).unwrap();
        let layouts =
            VariantLayoutCache::build(&program, crate::variant_layout::VariantTarget::Wasm32)
                .unwrap();
        let weight = |name: &str| {
            let function = program
                .functions
                .iter()
                .find(|function| function.id == id(name))
                .unwrap();
            FunctionPlan::build(&program, function, &layouts)
                .unwrap()
                .cleanup_place_flags
                .len() as u32
        };
        assert!(weight("arena.root") >= 2);
        assert!(weight("arena.leaf") >= 1);
        assert_eq!(
            arena_capacity(&program, &layouts, &[id("arena.root")]).unwrap(),
            1 + weight("arena.root") + weight("arena.leaf")
        );
        assert!(arena_capacity(&program, &layouts, &[id("arena.unrelated")]).is_err());
        assert!(arena_capacity(&program, &layouts, &[id("missing")]).is_err());
    }

    fn id(value: &str) -> DeclarationId {
        DeclarationId::new(value)
    }

    #[test]
    fn string_owner_cells_count_even_when_resource_cleanup_inventory_is_empty() {
        let source = r#"module test.string_capacity;
@id("s.sink") fn sink(value: string) -> string { "done" }
@id("s.root") fn root() -> string { sink("argument") }
@id("s.main") fn main() -> i64 { 0 }
"#;
        let program =
            crate::hir::resolve(&crate::check(source, "string-capacity.spx").unwrap()).unwrap();
        let layouts =
            VariantLayoutCache::build(&program, crate::variant_layout::VariantTarget::Wasm32)
                .unwrap();
        let mut count = 1;
        for name in ["s.root", "s.sink"] {
            let function = program
                .functions
                .iter()
                .find(|function| function.id == id(name))
                .unwrap();
            let plan = FunctionPlan::build(&program, function, &layouts).unwrap();
            assert!(plan.cleanup_place_flags.is_empty());
            assert!(!plan.owned_strings.owners.is_empty());
            count += u32::try_from(plan.owned_strings.owners.len()).unwrap();
        }
        assert_eq!(
            arena_capacity(&program, &layouts, &[id("s.root")]).unwrap(),
            count
        );
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
