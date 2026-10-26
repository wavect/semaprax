//! Owning successor controls. Authored only: no executed net/admission claim.
use super::*;
use crate::bounded_output::{self, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};

fn binding(count: usize, depth: usize, state: Availability) -> ValidationBinding {
    let moved_places = (0..count).map(|n| {
        let path = (0..depth).map(|d| PlaceProjection::Field(
            DeclarationId::new(format!("retained.partial.{n:04}.{d}"))
        )).collect();
        (path, state)
    }).collect();
    ValidationBinding {
        contract_local: false,
        mutable: true,
        ty: ResolvedType::Bytes,
        ownership: OwnershipMode::Own,
        availability: Availability::Available,
        active_loans: BTreeSet::new(),
        moved_places,
        definitely_partial: BTreeSet::new(),
    }
}
fn equal_fact(left: &ValidationBinding, right: &ValidationBinding) {
    assert_eq!(left, right);
}
fn complete(metrics: Metrics, paid: usize) {
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
}
fn seed(input: &BTreeMap<ValueId, ValidationBinding>) -> Result<Scope, Diagnostic> {
    let mut result = Scope::default();
    for (id, value) in input {
        let prepared = result.prepare_insert()?;
        charge::reserve(charge::binding(value))?;
        prepared.insert(id.clone(), value.clone())?;
    }
    Ok(result)
}
fn receive(
    input: &BTreeMap<ValueId, ValidationBinding>,
    expected: &BTreeMap<ValueId, ValidationBinding>,
    branches: bool,
    repetitions: usize,
    cow: bool,
) -> Result<(), Diagnostic> {
    let mut target = seed(input)?;
    let left = target.clone();
    let right = target.clone();
    let ids = target.ids();
    for iteration in 0..repetitions {
        if branches {
            super::super::branches(&mut target, &left, &right, &ids)?;
        } else {
            super::super::conditional(&mut target, &left, &ids)?;
        }
        for (id, original) in expected {
            let actual = target.get(id).unwrap();
            assert_eq!(actual.ty, original.ty);
            assert_eq!(actual.availability, original.availability);
            assert_eq!(actual.moved_places, original.moved_places);
            assert_eq!(actual.definitely_partial, original.definitely_partial);
            assert_eq!(actual.active_loans, original.active_loans);
            assert_eq!(actual.ownership, original.ownership);
            assert_eq!(actual.contract_local, original.contract_local);
            assert_eq!(actual.mutable, original.mutable ^ (cow && iteration % 2 == 1));
        }
        // A real retained snapshot forces the later binding/tree COW clone.
        // Its charge::edit pays actual logical payload; packing is measured,
        // rather than assuming retained and collected tree layouts are equal.
        let snapshot = target.clone();
        if cow {
            for id in input.keys() {
                let target = target.get_mut(id)?.unwrap();
                target.mutable = !target.mutable;
            }
            for id in input.keys() {
                assert_ne!(target.get(id).unwrap().mutable, snapshot.get(id).unwrap().mutable);
            }
        }
        assert!(left.iter().eq(input.iter()) && right.iter().eq(input.iter()));
        drop(snapshot);
    }
    drop(ids);
    drop(right);
    drop(left);
    drop(target);
    Ok(())
}
fn input(count: usize, depth: usize) -> BTreeMap<ValueId, ValidationBinding> {
    BTreeMap::from([(ValueId::new("retained.partial.owner".to_owned()),
        binding(count, depth, Availability::Moved))])
}
fn expected(input: &BTreeMap<ValueId, ValidationBinding>) -> BTreeMap<ValueId, ValidationBinding> {
    input.iter().map(|(id, value)| {
        let mut result = value.clone();
        result.moved_places = HirValidator::join_moved_places(value, value);
        result.definitely_partial = HirValidator::join_definitely_partial(value, value);
        (id.clone(), result)
    }).collect()
}

#[test]
fn retained_candidates_preserve_all_four_classes_empty_prefixes_duplicates_and_short_circuit() {
    for count in [0, 1, 2, 17] {
        for depth in [1, 2, 3] {
            for state in [Availability::Available, Availability::Moved, Availability::MaybeMoved] {
                let mut left = binding(count, depth, state);
                let mut right = binding(count, depth, Availability::Moved);
                let common = vec![PlaceProjection::Field(DeclarationId::new("common"))];
                left.definitely_partial.insert(Vec::new());
                left.definitely_partial.insert(common.clone());
                right.definitely_partial.insert(common.clone());
                right.definitely_partial.insert(vec![PlaceProjection::VariantField { case: DeclarationId::new("case"), field: DeclarationId::new("payload") }]);
                right.moved_places.insert(common, Availability::MaybeMoved);
                for availability in [Availability::Available, Availability::Moved, Availability::MaybeMoved] {
                    right.availability = availability;
                    let before_left = left.clone();
                    let before_right = right.clone();
                    let old = HirValidator::join_definitely_partial(&left, &right);
                    let current = HirValidator::join_definitely_partial_retained(&left, &right);
                    assert_eq!(current, old);
                    equal_fact(&left, &before_left);
                    equal_fact(&right, &before_right);
                }
            }
        }
    }
}

#[test]
fn complete_depth_one_to_three_successive_scope_and_cow_callers_earn_real_allocation_and_invoice_net() {
    for depth in [1, 2, 3] {
        let input = input(64, depth);
        let expected = expected(&input);
        for branches in [false, true] {
            for (repetitions, cow) in [(1, false), (3, true)] {
                let mut observations = Vec::new();
                for old in [true, false] {
                    let (((), metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
                        measure(|| {
                            if old { with_before(|| receive(&input, &expected, branches, repetitions, cow)) }
                            else { receive(&input, &expected, branches, repetitions, cow) }.unwrap();
                        })
                    });
                    assert!(!refused && first.is_none());
                    complete(metrics, paid);
                    observations.push((metrics, paid));
                }
                eprintln!("PARTIAL_RETAIN depth={depth} branches={branches} repeats={repetitions} cow={cow} old={:?} current={:?}", observations[0], observations[1]);
                assert!(observations[1].0.allocated < observations[0].0.allocated,
                    "complete successive/COW caller must remove actual requested backing");
                assert!(observations[1].1 < observations[0].1,
                    "all reached current controls must earn their own full-caller debit");
            }
        }
    }
}

#[test]
fn complete_small_empty_and_all_reject_costs_are_observed_without_a_universal_gain_claim() {
    for count in [0, 1, 2] {
        for state in [Availability::Available, Availability::MaybeMoved, Availability::Moved] {
            let mut input = input(count, 1);
            input.values_mut().next().unwrap().moved_places.values_mut().for_each(|v| *v = state);
            if count == 0 { input.values_mut().next().unwrap().availability = Availability::Moved; }
            let expected = expected(&input);
            for branches in [false, true] {
                for old in [true, false] {
                    let (((), metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
                        measure(|| {
                            if old { with_before(|| receive(&input, &expected, branches, 1, true)) }
                            else { receive(&input, &expected, branches, 1, true) }.unwrap();
                        })
                    });
                    assert!(!refused && first.is_none());
                    complete(metrics, paid);
                    eprintln!("PARTIAL_RETAIN_COLD count={count} state={state:?} branches={branches} old={old} paid={paid} {metrics:?}");
                }
            }
        }
    }
}

#[test]
fn exact_first_control_and_complete_short_zero_floor_sticky_frontiers_leave_inputs_immutable() {
    let left = binding(64, 2, Availability::Moved);
    let right = left.clone();
    let control = super::super::moved_direct::CONTROL + CONTROL;
    for limit in [0, control - 1] {
        let ((result, metrics), refused, _, first) = with_limit_usage_refusal(limit, || {
            measure(|| super::super::join_scratch(&left, &right))
        });
        let error = result.unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(refused);
        assert_eq!(first.unwrap().requested, control);
        assert_eq!(metrics.allocated, 0, "first positive refusal precedes candidate construction");
    }
    let input = input(64, 2);
    let expected = expected(&input);
    for branches in [false, true] {
        for old in [true, false] {
            let run = |limit, floor| with_limit_usage_refusal(limit, || {
                assert!(bounded_output::set_active_floor(floor));
                measure(|| {
                    let result = if old { with_before(|| receive(&input, &expected, branches, 2, true)) }
                        else { receive(&input, &expected, branches, 2, true) };
                    match result {
                        Ok(()) => None,
                        Err(error) => {
                            assert!(!bounded_output::reserve_active_required(0));
                            Some((error.code, error.severity, error.message, error.path, error.span, error.help))
                        }
                    }
                })
            });
            let ((ok, metrics), refused, exact, first) = run(usize::MAX, 0);
            assert!(ok.is_none() && !refused && first.is_none()); complete(metrics, exact);
            for (limit, floor, success) in [(exact, 0, true), (exact - 1, 0, false),
                (0, 0, false), (exact + 71, 71, true), (exact + 70, 71, false)] {
                let result = run(limit, floor);
                assert_eq!(result.0.0.is_none(), success);
                assert_eq!(result.1, !success);
                assert_eq!(result.3.is_none(), success);
                complete(result.0.1, result.2);
                assert_eq!(run(limit, floor), result, "all raw diagnostics and actual Drop must be deterministic");
            }
        }
    }
    equal_fact(&left, &right);
}

#[test]
fn real_many_field_hir_old_and_current_keep_complete_semantics_and_hostile_diagnostics() {
    let mut source = String::from("module retained.partial.real;\n@id(\"Packet\") record Packet {");
    for n in 0..32 { source.push_str(&format!("@id(\"field{n}\") field{n}:Bytes,")); }
    source.push_str("}\n@id(\"take\") fn take(value:own Bytes)->usize {byte_len(bytes_as_slice(value))}\n@id(\"inspect\") fn inspect(packet:own Packet,flag:bool)->i64 {");
    for n in 0..32 {
        source.push_str(&format!("let branch{n}=if flag {{let used=take(packet.field{n});0}}else{{let used=take(packet.field{n});0}};"));
    }
    source.push_str("0}\n@id(\"main\") fn main()->i64 {0}\n");
    let ast = crate::check(&source, "retained-partial-real.spx").unwrap();
    let program = with_before(|| crate::hir::resolve(&ast)).unwrap();
    let mut observations = Vec::new();
    for old in [true, false] {
        let ((result, metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || measure(|| {
            let result = if old { with_before(|| crate::hir::validate(&program)) }
                else { crate::hir::validate(&program) };
            assert!(result.is_ok()); drop(result);
        }));
        assert_eq!(result, ()); assert!(!refused && first.is_none());
        complete(metrics, paid); observations.push((metrics, paid));
    }
    eprintln!("PARTIAL_RETAIN_FULL_HIR old={:?} current={:?}", observations[0], observations[1]);
    assert!(observations[1].0.allocated < observations[0].0.allocated);
    assert!(observations[1].1 < observations[0].1);
    let mut hostile = program.clone();
    hostile.entrypoint = DeclarationId::new("missing.retained.entry");
    let old = with_before(|| crate::hir::validate(&hostile));
    assert_eq!(crate::hir::validate(&hostile), old);
}

#[test]
fn postjoin_unwind_drops_retained_nodes_snapshots_and_restores_old_selection() {
    let input = input(64, 3);
    let expected = expected(&input);
    for old in [true, false] {
        let ((unwind, metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || measure(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let invoke = || {
                    let mut target = seed(&input).unwrap();
                    let snapshot = target.clone();
                    let ids = target.ids();
                    super::super::conditional(&mut target, &snapshot, &ids).unwrap();
                    for (id, value) in &expected {
                        assert_eq!(target.get(id).unwrap().definitely_partial, value.definitely_partial);
                    }
                    assert!(snapshot.iter().eq(input.iter()));
                    panic!("after complete retained candidate publication");
                };
                if old { with_before(invoke) } else { invoke() }
            })).is_err()
        }));
        assert!(unwind && !refused && first.is_none());
        complete(metrics, paid);
        assert!(!before_selected(), "test-only predecessor selection is panic-safe");
    }
}

#[test]
fn reached_empty_jointariff_pays_exact_fixed_surcharge_before_identical_publication_and_drop() {
    let id = ValueId::new("retained.empty.owner".to_owned());
    let target = BTreeMap::from([(id.clone(), binding(0, 1, Availability::Available))]);
    let mut child = target.clone();
    child.get_mut(&id).unwrap().availability = Availability::Moved;
    for branches in [false, true] {
        let mut expected = target.clone();
        let ids = [id.clone()];
        if branches { HirValidator::join_branches(&mut expected, &child, &child, &ids); }
        else { HirValidator::join_conditional(&mut expected, &child, &ids); }
        let mut observations = Vec::new();
        for old in [true, false] {
            let (((), metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || measure(|| {
                let run = || {
                    let mut output = seed(&target).unwrap();
                    let left = seed(&child).unwrap();
                    let right = seed(&child).unwrap();
                    let ids = output.ids();
                    if branches { super::super::branches(&mut output, &left, &right, &ids).unwrap(); }
                    else { super::super::conditional(&mut output, &left, &ids).unwrap(); }
                    assert!(output.iter().eq(expected.iter()));
                    assert!(left.iter().eq(child.iter()) && right.iter().eq(child.iter()));
                    drop(ids); drop(right); drop(left); drop(output);
                };
                if old { with_before(run) } else { run() }
            }));
            assert!(!refused && first.is_none()); complete(metrics, paid);
            observations.push((metrics, paid));
        }
        assert_eq!(observations[1].1, observations[0].1 + CONTROL,
            "non-skipped empty inventories have no removed tree debit");
        assert_eq!(observations[1].0, observations[0].0,
            "fixed source ABI surcharge does not remove empty backing");
        eprintln!("PARTIAL_RETAIN_REACHED_EMPTY branches={branches} old={:?} current={:?}", observations[0], observations[1]);
    }
}

#[test]
fn paid_distinct_full_scope_inventory_states_and_partial_classes_match_whole_generic_publication() {
    for depth in [1, 2, 3] {
        for count in [1, 7, 32] {
            let id = ValueId::new("retained.distinct.owner".to_owned());
            let target = BTreeMap::from([(id.clone(), binding(count, depth, Availability::Available))]);
            let mut a = binding(count, depth, Availability::Moved);
            let mut b = binding(count, depth, Availability::MaybeMoved);
            a.active_loans.insert(LoanId(1)); b.active_loans.insert(LoanId(2));
            let variant = vec![PlaceProjection::VariantField {
                case: DeclarationId::new("retained.variant"), field: DeclarationId::new("payload"),
            }];
            a.definitely_partial.insert(variant.clone());
            b.moved_places.insert(variant, Availability::Moved);
            b.definitely_partial.insert(Vec::new());
            // Different root lengths/overlap force all four candidate classes
            // and missing/default Availability through the paid whole caller.
            if let Some(path) = b.moved_places.keys().next().cloned() { b.moved_places.remove(&path); }
            let left = BTreeMap::from([(id.clone(), a)]);
            let right = BTreeMap::from([(id.clone(), b)]);
            for branches in [false, true] {
                let mut expected = target.clone();
                let ids = [id.clone()];
                if branches { HirValidator::join_branches(&mut expected, &left, &right, &ids); }
                else { HirValidator::join_conditional(&mut expected, &left, &ids); }
                for old in [true, false] {
                    let (((), metrics), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || measure(|| {
                        let run = || {
                            let mut output = seed(&target).unwrap();
                            let left_scope = seed(&left).unwrap();
                            let right_scope = seed(&right).unwrap();
                            let ids = output.ids();
                            for step in 0..2 {
                                let snapshot = output.clone();
                                if branches { super::super::branches(&mut output, &left_scope, &right_scope, &ids).unwrap(); }
                                else { super::super::conditional(&mut output, &left_scope, &ids).unwrap(); }
                                assert!(output.iter().eq(expected.iter()), "all binding fields");
                                if step == 0 { assert!(snapshot.iter().eq(target.iter())); }
                                else { assert!(snapshot.iter().eq(expected.iter())); }
                                assert!(left_scope.iter().eq(left.iter()) && right_scope.iter().eq(right.iter()));
                                drop(snapshot);
                            }
                            drop(ids); drop(right_scope); drop(left_scope); drop(output);
                        };
                        if old { with_before(run) } else { run() }
                    }));
                    assert!(!refused && first.is_none()); complete(metrics, paid);
                }
            }
        }
    }
}

#[test]
fn whole_immediate_paid_receiver_generic_builder_and_source_inverses_are_exact() {
    use sha2::{Digest, Sha256};
    let sha = |s: &str| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(s.as_bytes())));
    let original = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/7f066dff4bbc040dd5b90645588a907173290f119f05532e73429c12209ce9b8"));
    assert_eq!(sha(original), "7f066dff4bbc040dd5b90645588a907173290f119f05532e73429c12209ce9b8");
    let mut restored = include_str!("../../merge.rs").to_owned();
    assert_eq!(restored.matches("mod moved_direct;\nmod partial_retain;").count(), 1);
    restored = restored.replacen("mod moved_direct;\nmod partial_retain;", "mod moved_direct;", 1);
    assert_eq!(restored.matches("charge::reserve(Some(moved_direct::CONTROL + partial_retain::CONTROL))?;").count(), 1);
    restored = restored.replacen("charge::reserve(Some(moved_direct::CONTROL + partial_retain::CONTROL))?;", "charge::reserve(Some(moved_direct::CONTROL))?;", 1);
    assert_eq!(restored.matches("        // The SAME owned candidate builder still copies every prefix, even\n        // duplicates and rejected paths. Retain reuses that one tree; all\n        // copied path payload and other output/COW fees remain independent.\n        let slots = node.checked_mul(count)?;").count(), 1);
    restored = restored.replacen("        // The SAME owned candidate builder still copies every prefix, even\n        // duplicates and rejected paths. Retain reuses that one tree; all\n        // copied path payload and other output/COW fees remain independent.\n        let slots = node.checked_mul(count)?;", "        // The old candidate builder copies every prefix even when that prefix\n        // is a duplicate. Its filtered output reuses vectors but builds a new\n        // set. Keep both complete tree directories and every prefix payload.\n        let slots = node.checked_mul(count)?.checked_mul(2)?;", 1);
    assert_eq!(restored.matches("HirValidator::join_definitely_partial_retained(before, after)").count(), 1);
    restored = restored.replacen("HirValidator::join_definitely_partial_retained(before, after)", "HirValidator::join_definitely_partial(before, after)", 1);
    assert_eq!(restored.matches("HirValidator::join_definitely_partial_retained(left, right)").count(), 1);
    restored = restored.replacen("HirValidator::join_definitely_partial_retained(left, right)", "HirValidator::join_definitely_partial(left, right)", 1);
    assert_eq!(restored.matches("    #[cfg(test)]\n    if partial_retain::before_selected() {\n        return partial_retain::before::conditional(target, source, ids);\n    }\n").count(), 1);
    restored = restored.replacen("    #[cfg(test)]\n    if partial_retain::before_selected() {\n        return partial_retain::before::conditional(target, source, ids);\n    }\n", "", 1);
    assert_eq!(restored.matches("    #[cfg(test)]\n    if partial_retain::before_selected() {\n        return partial_retain::before::branches(target, left, right, ids);\n    }\n").count(), 1);
    restored = restored.replacen("    #[cfg(test)]\n    if partial_retain::before_selected() {\n        return partial_retain::before::branches(target, left, right, ids);\n    }\n", "", 1);
    assert_eq!(restored, original);
    let mut prior = include_str!("before.rs").replace("pub(in crate::hir::validation) fn join_scratch(", "fn join_scratch(").replacen("use super::*;\n", "use super::*;\nmod moved_direct;\n", 1);
    prior.push_str("\n#[cfg(test)]\nmod saved;\n#[cfg(test)]\nmod tests;\n");
    assert_eq!(prior, original);
    let generic = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/branch_merge.rs"));
    assert_eq!(sha(generic), "380817c30ceea47241d76d6356f24847b45b823dcf1b75196970b5ac375aee4a");
    let current = include_str!("../partial_retain.rs");
    let start = current.find("    pub(super) fn join_definitely_partial_retained(").unwrap();
    let end = current.find("\n}\n#[cfg(test)]").unwrap() + 1;
    let restored = current[start..end].replace("fn join_definitely_partial_retained(", "fn join_definitely_partial(").replace("        candidates.retain(move |path| {\n            Self::place_availability(left, path) == Availability::Moved\n                && Self::place_availability(right, path) == Availability::Moved\n        });\n        candidates\n", "        candidates\n            .into_iter()\n            .filter(|path| {\n                Self::place_availability(left, path) == Availability::Moved\n                    && Self::place_availability(right, path) == Availability::Moved\n            })\n            .collect()\n");
    let old_start = generic.find("    pub(super) fn join_definitely_partial(").unwrap();
    assert_eq!(restored, generic[old_start..generic.rfind("\n}").unwrap()].to_owned() + "\n");
}
