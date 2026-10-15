//! Complete old region/entry receiver outcomes; only acquired directories are funded here.
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
const LIMIT: usize = 16 * 1024 * 1024;
fn program() -> ResolvedProgram {
    let checked = crate::check(
        r#"module replay.regions_entry;
@id("take") fn take(left: own Bytes,right: own Bytes)->i64 {
 let lv=bytes_as_slice(left);let rv=bytes_as_slice(right);
 if byte_len(lv)>0usize && byte_len(rv)>0usize {1}else{0}
}
@id("main") fn main()->i64 {
 let left=bytes_zeroed(1usize);let right=bytes_zeroed(1usize);take(left,right)
}"#,
        "regions-entry.spx",
    )
    .unwrap();
    crate::hir::resolve(&checked).unwrap()
}
fn unknown() -> StorageId {
    StorageId::Temporary(ExpressionId::from_owned("foreign.λ\0🙂storage".into()))
}
fn region_parity(function: &ResolvedFunction) {
    let storage = crate::cleanup_plan::replay::validate_slots(function).unwrap();
    let old = crate::cleanup_plan::replay::validate_regions(function, &storage);
    let (((), actual), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let current = validate_regions(function, &storage, &mut budget);
            assert_eq!(current, old);
            drop(current);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    let ((), actual) = measure(|| {
        let result = crate::cleanup_plan::replay::validate_regions(function, &storage);
        assert_eq!(result, old);
        drop(result);
    });
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
}
#[test]
fn complete_regions_keep_membership_uniqueness_equal_count_and_original_error_priority() {
    let program = program();
    let mut saw_nonempty = false;
    let mut saw_multiple = false;
    for function in &program.functions {
        region_parity(function);
        let mut attacks = Vec::new();
        let mut empty = function.clone();
        empty.cleanup_plan.regions.clear();
        attacks.push(empty);
        if !function.cleanup_plan.regions.is_empty() {
            let mut root = function.clone();
            root.cleanup_plan.regions[0].parent = Some(CleanupRegionId(0));
            attacks.push(root);
            let mut id = function.clone();
            id.cleanup_plan.regions[0].id = CleanupRegionId(u32::MAX);
            attacks.push(id);
            let mut exit = function.clone();
            exit.cleanup_plan.regions[0].normal_scope_end =
                crate::cleanup_plan::ExitTargetId(u32::MAX);
            attacks.push(exit);
        }
        let positions: Vec<_> = function
            .cleanup_plan
            .regions
            .iter()
            .enumerate()
            .flat_map(|(r, region)| (0..region.slots.len()).map(move |s| (r, s)))
            .collect();
        if let Some(&(r, s)) = positions.first() {
            saw_nonempty = true;
            let key = function.cleanup_plan.regions[r].slots[s].clone();
            let mut duplicate = function.clone();
            duplicate.cleanup_plan.regions[r].slots.push(key);
            attacks.push(duplicate);
            let mut foreign = function.clone();
            foreign.cleanup_plan.regions[r].slots[s] = unknown();
            attacks.push(foreign);
            let mut omitted = function.clone();
            omitted.cleanup_plan.regions[r].slots.remove(s);
            attacks.push(omitted);
            if positions.len() > 1 {
                saw_multiple = true;
                let (r2, s2) = positions[1];
                let mut same_count = function.clone();
                same_count.cleanup_plan.regions[r2].slots[s2] =
                    function.cleanup_plan.regions[r].slots[s].clone();
                attacks.push(same_count);
            }
        }
        for attack in attacks {
            let storage = crate::cleanup_plan::replay::validate_slots(&attack).unwrap();
            assert!(crate::cleanup_plan::replay::validate_regions(&attack, &storage).is_err());
            region_parity(&attack);
        }
    }
    assert!(saw_nonempty && saw_multiple);
}
#[test]
fn complete_entry_keeps_first_unknown_overlap_and_all_diagnostic_fields() {
    let program = program();
    let mut saw_owned = false;
    for function in &program.functions {
        let storage = crate::cleanup_plan::replay::validate_slots(function).unwrap();
        let leaves = crate::cleanup_plan::replay::collect_leaves(function).unwrap();
        let mut cases = vec![function.clone()];
        let mut bad_entry = function.clone();
        bad_entry.cleanup_plan.entry = BlockId(u32::MAX);
        cases.push(bad_entry);
        if let Some(place) = function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .first()
        {
            saw_owned = true;
            let mut duplicate = function.clone();
            duplicate
                .cleanup_plan
                .entry_state
                .live_owned_parameters
                .push(place.clone());
            cases.push(duplicate);
            let mut foreign = function.clone();
            foreign.cleanup_plan.entry_state.live_owned_parameters[0].storage = unknown();
            cases.push(foreign);
            let mut first_duplicate = function.clone();
            first_duplicate
                .cleanup_plan
                .entry_state
                .live_owned_parameters
                .insert(1, place.clone());
            let mut later = place.clone();
            later.storage = unknown();
            first_duplicate
                .cleanup_plan
                .entry_state
                .live_owned_parameters
                .push(later);
            cases.push(first_duplicate);
        }
        for case in cases {
            let old = crate::cleanup_plan::replay::validate_entry(&case, &storage, &leaves);
            let (((), actual), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let current = validate_entry(&case, &storage, &leaves, &mut budget);
                    assert_eq!(current, old);
                    drop(current);
                    drop(budget);
                })
            });
            assert!(!refused && first.is_none());
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        }
    }
    assert!(saw_owned);
}
fn keys(program: &ResolvedProgram) -> Vec<StorageId> {
    let value = program
        .functions
        .iter()
        .flat_map(|f| &f.params)
        .next()
        .unwrap()
        .id
        .clone();
    let mut keys = vec![StorageId::Value(value), StorageId::ProvisionalResult];
    for i in 0..97 {
        let expression = ExpressionId::from_owned(format!("storage.λ\0🙂.{i:03}"));
        keys.push(StorageId::Temporary(expression.clone()));
        keys.push(StorageId::CallArgument {
            call: expression.clone(),
            parameter_index: i,
            value_expression: expression,
        });
    }
    keys
}
#[test]
fn all_storage_variants_keep_full_original_order_duplicates_and_exact_field_differences() {
    let program = program();
    let function = &program.functions[0];
    let mut keys = keys(&program);
    let call = ExpressionId::from_owned("same.call.λ\0🙂".into());
    for index in [0, 1] {
        for value in ["a", "b"] {
            keys.push(StorageId::CallArgument {
                call: call.clone(),
                parameter_index: index,
                value_expression: ExpressionId::from_owned(format!("value.{value}.λ\0🙂")),
            });
        }
    }
    for reverse in [false, true] {
        let mut budget = ReplayBudget::unobserved();
        let mut directory =
            Directory::new(function, &mut budget, "storage fullkey controller").unwrap();
        let mut old = BTreeSet::new();
        for key in if reverse {
            keys.iter().rev().collect::<Vec<_>>()
        } else {
            keys.iter().collect::<Vec<_>>()
        } {
            assert_eq!(
                directory.insert(function, key, &mut budget).unwrap(),
                old.insert(key)
            );
            assert!(directory.balanced());
            assert_eq!(
                directory.keys().copied().collect::<Vec<_>>(),
                old.iter().copied().collect::<Vec<_>>()
            );
        }
        for key in &keys {
            assert!(!directory.insert(function, key, &mut budget).unwrap());
            assert!(directory.contains(function, &key, &mut budget).unwrap());
        }
        let foreign = unknown();
        assert_eq!(
            directory
                .contains(function, &&foreign, &mut budget)
                .unwrap(),
            old.contains(&foreign)
        );
    }
}
fn directories(
    function: &ResolvedFunction,
    keys: &[StorageId],
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let mut assigned = Directory::new(function, budget, "region assignment controller")?;
    let mut flags = Directory::new(function, budget, "entry flag controller")?;
    for (index, key) in keys.iter().enumerate() {
        assert!(assigned.insert(function, key, budget)?);
        assert!(flags.insert(function, LivenessFlagId(index as u32), budget)?);
    }
    for key in keys {
        assert!(assigned.contains(function, &key, budget)?);
        assert!(!assigned.insert(function, key, budget)?);
    }
    assert!(assigned.balanced() && flags.balanced());
    drop(flags);
    drop(assigned);
    Ok(())
}
#[test]
fn both_receiving_directory_allocations_exact_short_zero_sticky_settle_complete_drop() {
    let program = program();
    let function = &program.functions[0];
    let keys = keys(&program);
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = directories(function, &keys, &mut budget);
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = crate::test_allocation_observer::current_metrics();
                    let remaining = active_remaining();
                    drop(directories(function, &keys, &mut budget));
                    let after = crate::test_allocation_observer::current_metrics();
                    assert_eq!(
                        (before.allocations, before.allocated),
                        (after.allocations, after.allocated)
                    );
                    assert_eq!(remaining, active_remaining());
                } else {
                    result.as_ref().unwrap();
                }
                drop(result);
                drop(budget);
            })
        })
    };
    let (((), actual), refused, paid, first) = observe(LIMIT);
    assert!(!refused && paid > 0 && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= paid && actual.highwater <= paid);
    for limit in [paid, paid - 1, 0] {
        let (((), actual), refused, used, first) = observe(limit);
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        assert!(actual.allocated <= used && actual.highwater <= used);
        if limit == paid {
            assert!(!refused && used == paid && first.is_none());
        } else {
            assert!(refused && first.is_some());
        }
        if limit == 0 {
            assert_eq!(actual.allocated, 0);
        }
    }
}

#[test]
fn complete_regions_and_entry_work_exact_short_sticky_keep_the_same_payer() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| !f.cleanup_plan.entry_state.live_owned_parameters.is_empty())
        .unwrap();
    let storage = crate::cleanup_plan::replay::validate_slots(function).unwrap();
    let leaves = crate::cleanup_plan::replay::collect_leaves(function).unwrap();
    for entry in [false, true] {
        let call = |budget: &mut ReplayBudget| {
            if entry {
                validate_entry(function, &storage, &leaves, budget)
            } else {
                validate_regions(function, &storage, budget)
            }
        };
        let (required, refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            let mut budget = ReplayBudget::new();
            let initial = budget.remaining;
            call(&mut budget).unwrap();
            initial - budget.remaining
        });
        assert!(!refused && required > 0 && first.is_none());
        for work in [required, required - 1, 0] {
            let (((), actual), _, _, _) = with_limit_usage_refusal(LIMIT, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    budget.remaining = work;
                    let result = call(&mut budget);
                    if work == required {
                        assert!(result.is_ok());
                        assert_eq!(budget.remaining, 0);
                    } else {
                        assert!(result.is_err() && budget.refusal_spent);
                        let remaining = budget.remaining;
                        let retry = call(&mut budget);
                        assert!(retry.is_err());
                        assert_eq!(budget.remaining, remaining);
                        drop(retry);
                    }
                    drop(result);
                    drop(budget);
                })
            });
            assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        }
    }
}

#[test]
fn complete_region_entry_and_four_variant_source_projections_restore_full_originals() {
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(
                include_str!("../regions_entry.rs").as_bytes()
            ))
        ),
        "327237ebc01755438f25ff967fc327a3ffb4f9d6be2a2a54bad6a36803b7095b"
    );
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/region-entry-receiving-816/exact-source-inverses.json"
    )))
    .unwrap();
    let sources=[
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/region-entry-receiving-816/src__cleanup_plan__replay.rs.before.txt"))),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/lexical_replay.rs")),include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/region-entry-receiving-816/src__cleanup_plan__replay__lexical_replay.rs.before.txt"))),
        (include_str!("../../receiving.rs"),include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/region-entry-receiving-816/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt"))),
        (include_str!("../directory.rs"),include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/region-entry-receiving-816/src__cleanup_plan__replay__expression_facts__receiving__directory.rs.before.txt"))),
    ];
    assert_eq!(records.as_array().unwrap().len(), sources.len());
    for (record, (current, original)) in records.as_array().unwrap().iter().zip(sources) {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let mut current = current.to_owned();
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(current.matches(after).count(), 1);
            current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(current, original);
    }
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/region-entry-receiving-816/receiver-projections.json"
    )))
    .unwrap();
    let originals = [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/region-entry-receiving-816/regions.before.txt"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/region-entry-receiving-816/entry.before.txt"
        )),
    ];
    assert_eq!(records.as_array().unwrap().len(), originals.len());
    for (record, original) in records.as_array().unwrap().iter().zip(originals) {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        assert_eq!(record["before"].as_str().unwrap(), original);
        let after = record["after"].as_str().unwrap();
        assert_eq!(
            include_str!("../regions_entry.rs").matches(after).count(),
            1
        );
        assert_eq!(
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay.rs"
            ))
            .matches(original)
            .count(),
            1
        );
    }
}
