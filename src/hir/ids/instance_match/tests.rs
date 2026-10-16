use super::instance_matches;
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal_site};
use crate::hir::{DeclarationId, FunctionInstanceId, ResolvedType};
use crate::test_allocation_observer::measure;
use std::cell::Cell;

thread_local! { static ARGUMENTS: Cell<usize> = const { Cell::new(0) }; }
pub(in crate::hir) fn record_argument() {
    ARGUMENTS.with(|n| n.set(n.get() + 1));
}
fn count() -> usize {
    ARGUMENTS.with(|n| n.replace(0))
}

thread_local! { static LEGACY: Cell<bool> = const { Cell::new(false) }; }
pub(in crate::hir) fn legacy_selected() -> bool {
    LEGACY.with(Cell::get)
}
fn with_legacy<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            LEGACY.with(|s| s.set(self.0));
        }
    }
    let restore = Restore(LEGACY.with(|s| s.replace(true)));
    let value = body();
    drop(restore);
    value
}

fn forms() -> Vec<ResolvedType> {
    use ResolvedType::*;
    vec![
        Never,
        OnceFunction,
        OnceFunctionI64,
        OnceFunctionI64Pair,
        MutFunctionI64,
        Unit,
        I64,
        I32,
        Char,
        U8,
        Usize,
        ArrayU8(0),
        ArrayU8(u32::MAX),
        F32,
        F64,
        Bool,
        String,
        Bytes,
        Str,
        SliceU8,
        StringMap,
        TypeParameter {
            owner: DeclarationId::new("parameter:λ\0"),
            index: u32::MAX,
        },
        Nominal {
            declaration: DeclarationId::new("nominal:函数"),
            arguments: vec![],
        },
        Nominal {
            declaration: DeclarationId::new("nested:λ"),
            arguments: vec![I64, Bytes],
        },
        Function {
            parameters: vec![I64, Bytes],
            result: Box::new(Bool),
        },
        Function {
            parameters: vec![],
            result: Box::new(Unit),
        },
    ]
}

#[test]
fn streamed_instance_matches_complete_encoder_all_forms_and_forged_bytes() {
    let all = forms();
    for name in ["", "template:λ\0", "0123456789", "函数:template"] {
        let template = DeclarationId::new(name);
        for args in [vec![], all.clone(), all.iter().cloned().rev().collect()] {
            let original = FunctionInstanceId::derive(&template, &args);
            assert!(instance_matches!(&original, &template, &args));
            for (offset, ch) in original.as_str().char_indices() {
                let truncated = FunctionInstanceId(original.as_str()[..offset].to_owned());
                assert!(!instance_matches!(&truncated, &template, &args));
                let mut bytes = original.as_str().to_owned();
                bytes.replace_range(offset..offset + ch.len_utf8(), "!");
                let forged = FunctionInstanceId(bytes);
                assert!(!instance_matches!(&forged, &template, &args));
            }
            for extra in [
                format!("{}:", original.as_str()),
                format!(":{}", original.as_str()),
            ] {
                let forged = FunctionInstanceId(extra);
                assert!(!instance_matches!(&forged, &template, &args));
            }
            if args.len() > 1 {
                let mut reordered = args.clone();
                reordered.swap(0, 1);
                assert!(!instance_matches!(&original, &template, &reordered));
            }
        }
    }
    // Cross independent decimal-width boundaries in both header lengths and
    // argument counts without parsing or trusting the retained spelling.
    for width in [9, 10, 99, 100] {
        let template = DeclarationId::new("λ".repeat(width));
        let args = vec![ResolvedType::I64; width];
        let original = FunctionInstanceId::derive(&template, &args);
        assert!(instance_matches!(&original, &template, &args));
        assert!(!instance_matches!(&original, &template, &args[..width - 1]));
    }
    count();
}

#[test]
fn streamed_instance_exact_short_zero_sticky_floor_keep_all_key_producers_and_drop() {
    let template = DeclarationId::new("template:λ");
    let args = forms();
    let original = FunctionInstanceId::derive(&template, &args);
    count();
    let ((accepted, overflowed, bill, first, site), metrics) = measure(|| {
        with_limit_usage_refusal_site(usize::MAX, || {
            instance_matches!(&original, &template, &args)
        })
    });
    assert!(accepted && !overflowed);
    assert!(first.is_none() && site.is_none());
    assert_eq!(count(), args.len());
    assert_eq!(metrics.live, 0);
    assert!(!metrics.invalid);
    for limit in [0, bill - 1, bill] {
        let ((accepted, overflowed, paid, first, site), observed) = measure(|| {
            with_limit_usage_refusal_site(limit, || instance_matches!(&original, &template, &args))
        });
        assert_eq!(accepted, limit == bill);
        assert_eq!(overflowed, limit != bill);
        assert_eq!(count(), args.len());
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
        if limit == bill {
            assert_eq!(paid, bill);
            assert!(first.is_none() && site.is_none());
        } else {
            assert!(first.is_some() && site.is_some());
        }
    }
    let wrong = FunctionInstanceId("wrong-header".to_owned());
    let (_, overflowed, _, first, _) = with_limit_usage_refusal_site(bill, || {
        assert!(!instance_matches!(&wrong, &template, &args));
    });
    assert!(!overflowed);
    assert!(first.is_none());
    assert_eq!(count(), args.len());
    let ((before, after, accepted), overflowed, _, first, _) =
        with_limit_usage_refusal_site(bill, || {
            assert!(!reserve_active_required(usize::MAX));
            let before = crate::bounded_output::active_refused();
            let accepted = instance_matches!(&original, &template, &args);
            (before, crate::bounded_output::active_refused(), accepted)
        });
    assert!(before && after && overflowed && !accepted);
    assert!(first.is_some());
    assert_eq!(count(), args.len());
    let (accepted, overflowed, _, first, _) = with_limit_usage_refusal_site(bill, || {
        assert!(crate::bounded_output::set_active_floor(1));
        instance_matches!(&original, &template, &args)
    });
    assert!(!accepted && overflowed);
    assert_eq!(first.unwrap().floor, 1);
    assert_eq!(count(), args.len());
    // Reserved-floor truncation is distinct from absolute exhaustion: the
    // complete child still fits remaining bytes but would invade the floor.
    // Use the actual successful comparison bill, not a guessed byte cap.
    let child = ResolvedType::Nominal {
        declaration: DeclarationId::new("x".repeat(8192)),
        arguments: vec![],
    };
    let child_bytes = child.identity_key().len();
    let compound = vec![ResolvedType::Nominal {
        declaration: DeclarationId::new("N"),
        arguments: vec![child],
    }];
    let canonical = FunctionInstanceId::derive(&template, &compound);
    let (accepted, overflowed, complete_bill, _, _) =
        with_limit_usage_refusal_site(usize::MAX, || {
            instance_matches!(&canonical, &template, &compound)
        });
    assert!(accepted && !overflowed);
    let truncated = format!("nominal:1:N:1:{child_bytes}:");
    let forged = FunctionInstanceId(format!(
        "semaprax.function-instance.v1:{}:{}:1:{}:{}",
        template.as_str().len(),
        template,
        truncated.len(),
        truncated,
    ));
    let ((accepted, retried), overflowed, _, first, site) =
        with_limit_usage_refusal_site(complete_bill, || {
            assert!(crate::bounded_output::set_active_floor(child_bytes));
            let accepted = instance_matches!(&forged, &template, &compound);
            let retried = instance_matches!(&forged, &template, &compound);
            (accepted, retried)
        });
    assert!(
        !accepted && !retried && overflowed,
        "truncated nested key is never identity authority"
    );
    let first = first.unwrap();
    assert_eq!(first.requested, child_bytes);
    assert_eq!(first.floor, child_bytes);
    assert!(first.remaining >= child_bytes);
    assert_eq!(first.stage, None);
    let site = site.unwrap();
    assert!(site.file().ends_with("bounded_output/checked_string.rs"));
    assert!(site.line() > 0);
    count();
    let zero = Vec::<ResolvedType>::new();
    let small = vec![ResolvedType::I64];
    for (shape, arguments) in [
        ("zero", zero.as_slice()),
        ("small", small.as_slice()),
        ("long-compound", compound.as_slice()),
    ] {
        let retained = FunctionInstanceId::derive(&template, arguments);
        let (((), old_overflowed, old_paid, _, _), old_heap) = measure(|| {
            with_limit_usage_refusal_site(usize::MAX, || {
                drop(FunctionInstanceId::derive(&template, arguments))
            })
        });
        let ((matched, overflowed, paid, first, site), heap) = measure(|| {
            with_limit_usage_refusal_site(usize::MAX, || {
                instance_matches!(&retained, &template, arguments)
            })
        });
        assert!(matched && !overflowed && !old_overflowed);
        assert!(first.is_none() && site.is_none());
        assert_eq!(heap.live, 0);
        assert!(!heap.invalid);
        assert_eq!(old_heap.live, 0);
        assert!(!old_heap.invalid);
        assert_eq!(heap.allocated, heap.freed);
        assert_eq!(old_heap.allocated, old_heap.freed);
        eprintln!("instance shape={shape} old_paid={old_paid} current_paid={paid} old={old_heap:?} current={heap:?}");
    }
    count();
    // Independent complete encoder remains live; report costs without claiming
    // a universal budget/heap saving or that identity_key scratch is eliminated.
    let (((), old_overflowed, old_bill, _, _), old_metrics) = measure(|| {
        with_limit_usage_refusal_site(usize::MAX, || {
            drop(FunctionInstanceId::derive(&template, &args))
        })
    });
    assert!(!old_overflowed);
    assert_eq!(old_metrics.live, 0);
    assert!(!old_metrics.invalid);
    eprintln!("instance comparison old_bill={old_bill} new_bill={bill} old={old_metrics:?} new={metrics:?}");
}

#[test]
fn streamed_instance_validator_preserves_duplicate_before_canonical_refusal() {
    let source = r#"module identity.instances;
@id("identity.keep") fn keep<T>(value:T)->T {value}
@id("identity.main") fn main()->i64 {keep<i64>(42)}
"#;
    let ast = crate::check(source, "instance-comparison.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    assert!(!program.function_instances.is_empty());
    crate::hir::validate(&program).unwrap();
    let frozen = crate::graph::to_hir_json(&program, &crate::graph::revision(&ast)).unwrap();
    assert_eq!(
        frozen,
        crate::graph::to_hir_json(&program, &crate::graph::revision(&ast)).unwrap()
    );
    let mut forged = program.clone();
    forged.function_instances[0].id.0.push(':');
    let expected = with_legacy(|| crate::hir::validate(&forged)).unwrap_err();
    let error = crate::hir::validate(&forged).unwrap_err();
    assert_eq!(error, expected);
    assert_eq!(error.code, "SPX-H006");
    let mut duplicate = program.clone();
    duplicate
        .function_instances
        .push(duplicate.function_instances[0].clone());
    let expected = with_legacy(|| crate::hir::validate(&duplicate)).unwrap_err();
    assert_eq!(crate::hir::validate(&duplicate).unwrap_err(), expected);
    // Isolate the original insert-before-comparison predicate: a duplicate
    // rejects without reaching the new reservation or another key producer.
    let instance = &program.function_instances[0];
    let mut seen = std::collections::BTreeSet::new();
    assert!(seen.insert(instance.id.clone()));
    count();
    let (rejected, overflowed, paid, first, site) = with_limit_usage_refusal_site(0, || {
        !seen.insert(instance.id.clone())
            || !instance_matches!(&instance.id, &instance.template, &instance.type_arguments)
    });
    assert!(rejected && !overflowed);
    assert_eq!(paid, 0);
    assert!(first.is_none() && site.is_none());
    assert_eq!(count(), 0);
    assert_eq!(
        frozen,
        with_legacy(|| crate::graph::to_hir_json(&program, &crate::graph::revision(&ast))).unwrap()
    );
}
