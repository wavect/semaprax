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
    let complete_current = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/owning-corrections-940/complete-regions-entry-current.rs.txt"
    ));
    assert_eq!(include_str!("../regions_entry.rs"), complete_current);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(complete_current.as_bytes()))
        ),
        "f27fb4d28d71bb522bafb208a3da4bd8098ec61b5cca904f33d5d9858738b0e4"
    );
    let current = ci_str!("../regions_entry.rs");
    let later_helper = "helpers::validate_place(function, place, storage, leaves, budget)?";
    assert_eq!(current.matches(later_helper).count(), 1);
    let original_receiver = current.replacen(
        later_helper,
        "validate_place(function, place, storage, leaves)?",
        1,
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original_receiver.as_bytes()))
        ),
        "327237ebc01755438f25ff967fc327a3ffb4f9d6be2a2a54bad6a36803b7095b"
    );
    let records_source = region_entry_original_input(
        "docs/evidence/region-entry-receiving-816/exact-source-inverses.json",
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/region-entry-receiving-816/exact-source-inverses.json"
        )),
    );
    let records: serde_json::Value = serde_json::from_str(&records_source).unwrap();
    let sources = [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/region-entry-receiving-816/src__cleanup_plan__replay.rs.before.txt"
            )),
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/lexical_replay.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/region-entry-receiving-816/src__cleanup_plan__replay__lexical_replay.rs.before.txt"
            )),
        ),
        (
            include_str!("../../receiving.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/region-entry-receiving-816/src__cleanup_plan__replay__expression_facts__receiving.rs.before.txt"
            )),
        ),
        (
            include_str!("../directory.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/region-entry-receiving-816/src__cleanup_plan__replay__expression_facts__receiving__directory.rs.before.txt"
            )),
        ),
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
        let mut current = region_entry_original_input(record["source"].as_str().unwrap(), current);
        // The later status-prefix owner is a separate compiled child. Its
        // registration adds no directory implementation to this frozen root.
        if record["source"] == "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" {
            let registration = "\nmod status_prefix;\npub(super) use status_prefix::Owner as StatusInsertionOwner;\n";
            assert!(current.ends_with(registration));
            assert_eq!(current.matches(registration).count(), 1);
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(
                        region_entry_original_input(
                            "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs",
                            include_str!("../directory/status_prefix.rs"),
                        )
                        .as_bytes()
                    ))
                ),
                "914e1a78079f871cb37462414b1991cde6722e78b7af39cb7979e11f16f767e9"
            );
            current.truncate(current.len() - registration.len());
        }
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(current.matches(after).count(), 1);
            current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(current, original);
    }
    let records: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/region-entry-receiving-816/receiver-projections.json"
    )))
    .unwrap();
    let originals = [
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/region-entry-receiving-816/regions.before.txt"
        )),
        ci_str!(concat!(
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
        assert_eq!(original_receiver.matches(after).count(), 1);
        // The moved entry receiver explicitly qualifies its existing helper;
        // restore that recorded relocation adapter before checking the full body.
        let after = record["after"].as_str().unwrap().replace(
            "let under = validate_place(function, place, storage, leaves)?;",
            "let under = helpers::validate_place(function, place, storage, leaves, budget)?;",
        );

        assert_eq!(
            include_str!("../regions_entry.rs")
                .matches(after.as_str())
                .count(),
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

// Full physical/runtime domains stay separate from immutable source proof JSON.
fn region_entry_original_input(source: &str, current: &str) -> String {
    use sha2::{Digest, Sha256};
    // Acquire the complete exact pre-885 source before the unchanged older
    // runtime-view/hash/registration authorities below. No assertion is waived.
    let expected_entry = match source {
        "src/cleanup_plan/replay.rs" => {
            Some("cce98dd37682ccb93081bae2eeb53ea103562370461844c12d16a59cb732d31b")
        }
        "src/cleanup_plan/replay/lexical_replay.rs" => {
            Some("d0f2b7fcfc3815f0c1207c064d8bcb14d4323dff614d953988b962138ff975d0")
        }
        "src/cleanup_plan/replay/expression_facts/receiving.rs" => {
            Some("db822f51812769eadf1570bfee4b315ceadd31b689fc6efa643e5da03d7a9bb7")
        }
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => {
            Some("112b1a3f5e47c5d0c99c476c2374e7eb9dbfdc04d9220a94f6bf73b613ee294e")
        }
        _ => None,
    };
    let entry = match expected_entry {
        Some(expected) => {
            let entry = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
                source, expected, current,
            );
            assert_eq!(
                format!(
                    "{:x}",
                    crate::digest_hex::LowerHex(Sha256::digest(entry.as_bytes()))
                ),
                expected,
                "complete region-entry historical input: {source}"
            );
            entry
        }
        None => current.to_owned(),
    };
    let before_885 =
        crate::cleanup_plan::replay::expression_facts::arena::retained::tests::before_885_source(
            source, &entry,
        );
    let directory_before_lookup = region_entry_directory_before_lookup(source, &before_885);
    let current = directory_before_lookup.as_str();
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/region-status-import-view-d4-20261015/runtime-views.json"
    )))
    .unwrap();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)
        .expect("complete named region-entry input");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"].as_str().unwrap()
    );
    assert_eq!(current, row["edits"][0]["new"].as_str().unwrap());
    let original = row["edits"][0]["old"].as_str().unwrap();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        row["original_sha256"].as_str().unwrap()
    );
    original.to_owned()
}

// The existing OPT885 inverse runs first. This exact later lookup-stage inverse
// then supplies its returned bytes to every unchanged original d4 row/hook.
#[rustfmt::skip]
fn region_entry_directory_before_lookup(source: &str, current: &str) -> String {
    use sha2::{Digest, Sha256};
    if source != "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" {
        return current.to_owned();
    }
    assert_eq!(current.len(), 20833);
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))),
        "71bddf2e91cd7b6768523035ce49c111e41686dc5104adbcdb8dd7165675e464");
    let mut restored = current.to_owned();
    for added in [
        "    fn status_key(&self) -> Option<&StatusSourceId> {\n        None\n    }\n",
        "    fn status_key(&self) -> Option<&StatusSourceId> {\n        Some(self)\n    }\n",
        "    fn status_key(&self) -> Option<&StatusSourceId> {\n        T::status_key(self)\n    }\n",
        "    lookup: Option<Box<status_prefix::FrozenStatusIndex>>,\n",
        "\nstruct LookupCursor {\n    at: usize,\n    byte: usize,\n    mask: u16,\n    symbol: u16,\n    direction: bool,\n    units: usize,\n}\n// Complete initialized cursor plus receiving state, status-key Option and\n// borrowed receivers, final candidate/ordering and comparison temporaries.\nconst LOOKUP_BYTES: usize = 2 * std::mem::size_of::<LookupCursor>()\n    + 2 * std::mem::size_of::<Option<&StatusSourceId>>()\n    + 4 * std::mem::size_of::<&StatusSourceId>()\n    + std::mem::size_of::<&status_prefix::FrozenStatusIndex>()\n    + 2 * std::mem::size_of::<Option<usize>>()\n    + std::mem::size_of::<usize>()\n    + std::mem::size_of::<Ordering>();\n",
        "    #[cfg(test)]\n    pub(super) fn has_frozen_lookup_for_test(&self) -> bool {\n        self.lookup.is_some()\n    }\n\n",
        "        // The new optional field has two initialized receiving slots. Existing\n        // directory fields and reservation requests receive no credit.\n        budget.reserve_bytes(\n            function,\n            2 * std::mem::size_of::<Option<Box<status_prefix::FrozenStatusIndex>>>(),\n            phase,\n        )?;\n        budget.charge(function, 1, phase)?;\n",
        "            lookup: None,\n",
        "        if let Some(index) = &self.lookup {\n            // Binding checks read no foreign payer or cached key. A rejected\n            // receiver remains rejected even after its original owner returns.\n            if index.failed.get() {\n                return Err(Diagnostic::io(\"SPX-H006\", String::new()));\n            }\n            if !std::ptr::eq(index.function, function)\n                || !std::ptr::eq(index.budget, budget)\n                || !index.ledger.current()\n                || index.keys != self.nodes.as_ptr() as usize\n                || index.len != self.nodes.len()\n            {\n                index.failed.set(true);\n                return Err(Diagnostic::io(\"SPX-H006\", String::new()));\n            }\n            self.authenticate(function, budget)?;\n            // All traversal state is one fixed receiving frame. Loop steps\n            // update its paid fields rather than constructing a new path.\n            budget.reserve_bytes(function, LOOKUP_BYTES, self.phase)?;\n            budget.charge(function, 24, self.phase)?;\n            let Some(query) = key.status_key() else {\n                index.failed.set(true);\n                return Err(Diagnostic::io(\"SPX-H006\", String::new()));\n            };\n            let mut cursor = LookupCursor {\n                at: index.root,\n                byte: 0,\n                mask: 0,\n                symbol: 0,\n                direction: false,\n                units: 0,\n            };\n            cursor.byte = index.nodes[cursor.at].byte;\n            while cursor.byte != NIL {\n                budget.charge(function, VISIT_UNITS + 24, self.phase)?;\n                cursor.mask = index.nodes[cursor.at].mask;\n                cursor.symbol = if cursor.byte < query.expression.as_str().len() {\n                    256 | query.expression.as_str().as_bytes()[cursor.byte] as u16\n                } else if cursor.byte == query.expression.as_str().len() {\n                    match query.lane {\n                        StatusLane::OwnerAdmission => 0,\n                        StatusLane::OperationFailure => 1,\n                        StatusLane::AllocationFailure => 2,\n                        StatusLane::ContractFalse => 3,\n                    }\n                } else {\n                    0\n                };\n                cursor.direction = cursor.symbol & cursor.mask != 0;\n                cursor.at = if cursor.direction {\n                    index.nodes[cursor.at].one\n                } else {\n                    index.nodes[cursor.at].zero\n                };\n                cursor.byte = index.nodes[cursor.at].byte;\n            }\n            budget.charge(function, VISIT_UNITS + 2, self.phase)?;\n            cursor.at = index.nodes[cursor.at].zero;\n            let candidate = &self.nodes[cursor.at];\n            // The index selects only a candidate. Preserve the complete old\n            // key-comparison request and actual Ord equality, without a prefix\n            // discount, identifier parsing, hashes or numeric-ID authority.\n            cursor.units = match key\n                .comparison_units(&candidate.key)\n                .and_then(|units| units.checked_add(VISIT_UNITS))\n            {\n                Some(units) => units,\n                None => {\n                    return Err(overflow(\n                        function,\n                        budget,\n                        \"directory key comparison work overflow\",\n                    ))\n                }\n            };\n            budget.charge(function, cursor.units, self.phase)?;\n            return Ok(Ord::cmp(key, &candidate.key) == Ordering::Equal);\n        }\n",
        "        if let Some(index) = &self.lookup {\n            // Publication seals this privately owned directory. No mutation or\n            // fallback can revive a rejected read-only index.\n            index.failed.set(true);\n            return Err(Diagnostic::io(\"SPX-H006\", String::new()));\n        }\n",
        "\n#[cfg(test)]\nmod before_cursor;\n",

    ] {
        assert_eq!(restored.matches(added).count(), 1, "complete unique directory lookup-stage inverse");
        restored = restored.replacen(added, "", 1);
    }
    assert_eq!(restored.len(), 15428);
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))),
        "d161622003e500b3b9197c0f463b88eee388d9447c5a87ba75f464d3aa2a59cc");
    restored
}
