//! Full collector/validator oracles; source-authored and unexecuted.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup::FieldLiveness;
use crate::cleanup_plan::CleanupSlotId;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let checked = crate::check(
        r#"module projection.backing;
@id("projection.main") fn main()->i64 {let data=bytes_zeroed(1usize);0}"#,
        "projection-backing.spx",
    )
    .unwrap();
    crate::hir::resolve(&checked).unwrap()
}
fn nested(depth: usize, flag: u32) -> FieldLivenessShape {
    let mut shape = FieldLivenessShape::Leaf {
        flag: LivenessFlagId(flag),
        lifecycle: DeclarationId::new("projection.drop"),
    };
    for _ in 0..depth {
        shape = FieldLivenessShape::Record {
            declaration: DeclarationId::new("projection.record"),
            fields: vec![FieldLiveness {
                field: DeclarationId::new("projection.field"),
                field_index: 0,
                shape,
            }],
        };
    }
    shape
}
fn function(depths: &[usize]) -> ResolvedFunction {
    let mut f = program().functions.remove(0);
    let template = f.cleanup_plan.slots[0].clone();
    f.cleanup_plan.slots = depths
        .iter()
        .enumerate()
        .map(|(i, depth)| {
            let mut slot = template.clone();
            slot.id = CleanupSlotId(i.try_into().unwrap());
            slot.storage_index = i.try_into().unwrap();
            slot.storage =
                StorageId::Temporary(ExpressionId::from_owned(format!("projection.slot.{i}")));
            slot.field_liveness_shape = nested(*depth, i.try_into().unwrap());
            slot
        })
        .collect();
    f
}
fn same(a: &Leaves, b: &Leaves) {
    assert_eq!(a.len(), b.len());
    for (flag, leaf) in a.iter() {
        assert_eq!(leaf.place, b[flag].place);
        assert_eq!(leaf.lifecycle, b[flag].lifecycle);
    }
}
fn old(f: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<Leaves, Diagnostic> {
    let _stage = Before::hold();
    collect_leaves_paid(f, budget)
}
fn compare(f: &ResolvedFunction) {
    match (
        old(f, &mut ReplayBudget::unobserved()),
        collect_leaves_paid(f, &mut ReplayBudget::unobserved()),
    ) {
        (Ok(a), Ok(b)) => {
            same(&a, &b);
            // These independently allocated lookup results are outside an
            // observed allocation/accounting scope, never free measured work.
            for leaf in a.values() {
                assert_eq!(a.under(&leaf.place), b.under(&leaf.place));
            }
        }
        (Err(a), Err(b)) => assert_eq!(a, b),
        _ => panic!("complete old/current collectors disagree"),
    }
}
#[test]
fn alternating_deep_slots_and_later_malformed_duplicate_shapes_preserve_entire_results() {
    for depths in [vec![], vec![0], vec![1], vec![65, 1, 0, 33, 2, 64, 1]] {
        compare(&function(&depths));
    }
    let full = function(&[33, 1, 65, 2]);
    let mut duplicate_leaf = full.clone();
    duplicate_leaf.cleanup_plan.slots[3].field_liveness_shape = nested(2, 0);
    let mut noncontiguous = full.clone();
    noncontiguous.cleanup_plan.slots[3].field_liveness_shape = nested(2, 17);
    let mut duplicate_field = full.clone();
    let field = FieldLiveness {
        field: DeclarationId::new("duplicate.field"),
        field_index: 0,
        shape: FieldLivenessShape::NoDrop,
    };
    let mut second = field.clone();
    second.field_index = 1;
    duplicate_field.cleanup_plan.slots[3].field_liveness_shape = FieldLivenessShape::Record {
        declaration: DeclarationId::new("duplicate.record"),
        fields: vec![field, second],
    };
    let mut malformed = full.clone();
    malformed.cleanup_plan.slots[3].field_liveness_shape = FieldLivenessShape::Record {
        declaration: DeclarationId::new("malformed.record"),
        fields: vec![FieldLiveness {
            field: DeclarationId::new("bad.field"),
            field_index: 1,
            shape: FieldLivenessShape::NoDrop,
        }],
    };
    for f in [duplicate_leaf, noncontiguous, duplicate_field, malformed] {
        assert!(old(&f, &mut ReplayBudget::unobserved()).is_err());
        compare(&f);
    }
}
fn run(f: &ResolvedFunction, before: bool, limit: usize) -> (bool, usize, usize) {
    let reference = old(f, &mut ReplayBudget::unobserved()).unwrap();
    let ((success, metrics), refused, paid, receipt) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let _stage = if before { Some(Before::hold()) } else { None };
            let mut budget = ReplayBudget::new();
            let result = collect_leaves_paid(f, &mut budget);
            let success = result.is_ok();
            if let Ok(leaves) = &result {
                same(leaves, &reference);
            }
            if let Err(first) = &result {
                let held = (
                    first.code,
                    first.severity,
                    first.message.as_ref(),
                    &first.path,
                    first.span,
                    &first.help,
                );
                let before = crate::test_allocation_observer::current_metrics();
                let retry = budget.reserve_bytes(f, 0, "projection backing sticky retry");
                assert!(retry.is_err());
                assert_eq!(crate::test_allocation_observer::current_metrics(), before);
                assert_eq!(
                    (
                        first.code,
                        first.severity,
                        first.message.as_ref(),
                        &first.path,
                        first.span,
                        &first.help
                    ),
                    held
                );
                drop(retry);
            }
            drop((result, budget));
            success
        })
    });
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    assert_eq!(refused, !success);
    assert_eq!(receipt.is_some(), !success);
    (success, paid, metrics.allocated)
}
#[test]
fn complete_old_current_invoices_report_repeated_net_and_honest_empty_cold_loss() {
    for depths in [vec![], vec![0], vec![1], vec![33; 64]] {
        let f = function(&depths);
        let before = run(&f, true, usize::MAX);
        let current = run(&f, false, usize::MAX);
        assert!(before.0 && current.0);
        if depths.len() == 64 {
            assert!(
                current.1 < before.1,
                "full collector must earn its new owner bank"
            );
            assert!(
                current.2 < before.2,
                "physically absent repeated projection growth"
            );
        }
        if depths.is_empty() {
            // A new first positive reservation also acquires the unchanged
            // actual refusal-text backing; an empty predecessor acquired none.
            assert_eq!(current.1, before.1 + ENTRY + f.id.as_str().len() + 512);
        }
        eprintln!("PROJECTION_BACKING depths={depths:?} old={before:?} current={current:?}; no result until executed");
    }
}
#[test]
fn exact_short_zero_and_sticky_cuts_drop_all_old_and_current_owners() {
    let f = function(&[33, 1, 65, 2, 33, 0]);
    for before in [true, false] {
        let full = run(&f, before, usize::MAX);
        assert!(full.0);
        let exact = run(&f, before, full.1);
        assert_eq!(exact, full);
        for limit in [0, full.1 - 1] {
            assert!(!run(&f, before, limit).0);
        }
    }
    // A later duplicate fails only after earlier deep slots have completed.
    let mut hostile = f.clone();
    hostile.cleanup_plan.slots[5].field_liveness_shape = nested(3, 0);
    let expected = old(&hostile, &mut ReplayBudget::unobserved())
        .err()
        .unwrap();
    for before in [true, false] {
        let (((), metrics), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let _stage = if before { Some(Before::hold()) } else { None };
                let mut budget = ReplayBudget::new();
                let error = collect_leaves_paid(&hostile, &mut budget).err().unwrap();
                assert_eq!(error, expected);
                drop((error, budget));
            })
        });
        assert!(!refused && !metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid);
    }
}
#[test]
fn whole_validator_old_and_current_callers_remain_independent_through_drop() {
    let p = program();
    let f = &p.functions[0];
    for before in [true, false] {
        let (((), metrics), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let _stage = if before { Some(Before::hold()) } else { None };
                let mut budget = ReplayBudget::new();
                reserve_program_skeleton_work(&p, std::iter::once(f), &mut budget).unwrap();
                validate_structure_with_budget(&p, f, &mut budget, None).unwrap();
                drop(budget);
            })
        });
        assert!(!refused && !metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
        assert!(metrics.allocated <= paid && metrics.highwater <= paid);
    }
}
#[test]
fn complete_immediate_source_and_lexical_old_collector_are_bound() {
    let current = include_str!("../../replay.rs");
    let old = prior_source(current);
    let from = old.find("fn collect_leaves_paid(").unwrap();
    let to = old[from..].find("\nfn collect_shape").unwrap() + from;
    let function = &old[from..to];
    let body = &function[function.find('{').unwrap() + 1..function.rfind('}').unwrap()];
    let module = include_str!("../projection_backing.rs");
    let from = module.find("=> {{").unwrap() + 5;
    let to = module[from..].find("    }};").unwrap() + from;
    let lexical = &module[from..to];
    let restored = lexical
        .replace("$function", "function")
        .replace("$budget", "budget");
    assert_eq!(restored.trim_end(), body.trim_end());
    let data: serde_json::Value =
        serde_json::from_str(include_str!("../projection_backing_inverse.json")).unwrap();
    let digest = |s: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(s.as_bytes()))
        )
    };
    use sha2::Digest;
    let child = include_str!("../projection_backing.rs");
    let tests = include_str!("tests.rs");
    assert_eq!(
        digest(child),
        data["new_module"]["sha256"].as_str().unwrap()
    );
    assert_eq!(digest(tests), data["new_tests"]["sha256"].as_str().unwrap());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, compiled) in [
        ("src/cleanup_plan/replay/projection_backing.rs", child),
        ("src/cleanup_plan/replay/projection_backing/tests.rs", tests),
    ] {
        assert_eq!(
            reader::read(&root.join(path), compiled.len(), &digest(compiled), 2 << 20).unwrap(),
            compiled
        );
    }
    for row in data["owners"].as_array().unwrap() {
        let path = root.join(row["source"].as_str().unwrap());
        let current = reader::read(
            &path,
            row["current"]["length"]
                .as_u64()
                .unwrap()
                .try_into()
                .unwrap(),
            row["current"]["sha256"].as_str().unwrap(),
            2 << 20,
        )
        .unwrap();
        let mut restored = current.clone();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let start: usize = edit["start"].as_u64().unwrap().try_into().unwrap();
            let end: usize = edit["end"].as_u64().unwrap().try_into().unwrap();
            assert_eq!(&restored[start..end], edit["after"].as_str().unwrap());
            restored.replace_range(start..end, edit["before"].as_str().unwrap());
        }
        assert_eq!(
            restored.len() as u64,
            row["before"]["length"].as_u64().unwrap()
        );
        assert_eq!(digest(&restored), row["before"]["sha256"].as_str().unwrap());
        assert_eq!(
            reader::read(&path, current.len(), &digest(&current), 2 << 20).unwrap(),
            current
        );
    }
}
