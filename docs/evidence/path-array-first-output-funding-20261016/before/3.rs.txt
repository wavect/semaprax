use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::cell::Cell;

thread_local! { static ORIGINAL: Cell<bool> = const { Cell::new(false) }; }
pub(super) fn original_selected() -> bool {
    ORIGINAL.with(Cell::get)
}
fn with_original<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ORIGINAL.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(ORIGINAL.with(|flag| flag.replace(true)));
    run()
}
fn program() -> ResolvedProgram {
    let source = "module path_arrays; @id(\"condition\") fn condition()->bool {true} @id(\"f\") fn f(a:bool,b:bool,c:bool,d:bool)->i64 {let x=if a {1}else{2};let y=if b {3}else{4};let z=if c {5}else{6};let w=if d {7}else{8};x+y+z+w} @id(\"main\") fn main()->i64 {f(true,false,true,false)}";
    let checked = crate::check(source, "path-arrays.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "path-arrays.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}
fn seed(count: usize, work: &mut SkeletonWork<'_, '_>) -> Vec<ExprSkeletonPath> {
    let mut paths = Vec::new();
    for n in 0..count {
        let mut path = empty_expr_path();
        path.failed = n % 3 == 0;
        path.residual = n % 3 == 1;
        work.push_expr_path(&mut paths, path, "path array input")
            .unwrap();
    }
    paths
}
fn same(actual: &[ExprSkeletonPath], expected: &[ExprSkeletonPath]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.observations, expected.observations);
        assert_eq!(actual.owned_source, expected.owned_source);
        assert_eq!(actual.failed, expected.failed);
        assert_eq!(actual.residual, expected.residual);
        assert_eq!(actual.continuation, expected.continuation);
    }
}

#[test]
fn four_complete_original_receivers_keep_all_path_fields_order_and_work() {
    let program = program();
    let function = &program.functions[0];
    let (((), allocations), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for count in [0, 1, 7, 8, 9, 16, 17] {
                for receiver in 0..4 {
                    let mut old_budget = ReplayBudget::new();
                    let mut budget = ReplayBudget::new();
                    let mut old_work = SkeletonWork::new(&program, function, &mut old_budget);
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let old_input = seed(count, &mut old_work);
                    let input = seed(count, &mut work);
                    let run = |original, input, work: &mut SkeletonWork<'_, '_>| match receiver {
                        0 => {
                            let mut target = Vec::new();
                            if original {
                                before::append_expr_paths(&mut target, input, work, "append")?;
                            } else {
                                append_expr_paths(&mut target, input, work, "append")?;
                            }
                            Ok(target)
                        }
                        1 => {
                            let suffixes = seed(1, work);
                            if original {
                                before::sequence_skeleton_paths(input, &suffixes, work)
                            } else {
                                sequence_skeleton_paths(input, &suffixes, work)
                            }
                        }
                        2 => {
                            let source = StatusSourceId {
                                expression: work
                                    .clone_owned(&function.body.id, "status test input")?,
                                lane: StatusLane::OperationFailure,
                            };
                            if original {
                                before::split_status_paths(input, source, work)
                            } else {
                                split_status_paths(input, source, work)
                            }
                        }
                        _ => {
                            if original {
                                before::split_contract(input, &function.body, work)
                            } else {
                                split_contract(input, &function.body, work)
                            }
                        }
                    };
                    let before = skeleton_materializations();
                    let expected = run(true, old_input, &mut old_work).unwrap();
                    let old_materializations = skeleton_materializations() - before;
                    let before = skeleton_materializations();
                    let actual = run(false, input, &mut work).unwrap();
                    assert_eq!(skeleton_materializations() - before, old_materializations);
                    same(&actual, &expected);
                    assert_eq!(actual.capacity(), expected.capacity());
                    assert_eq!(work.budget.remaining, old_work.budget.remaining);
                    assert_eq!(
                        work.budget.skeleton_remaining,
                        old_work.budget.skeleton_remaining
                    );
                    drop(actual);
                    drop(expected);
                    drop(work);
                    drop(old_work);
                    drop(budget);
                    drop(old_budget);
                }
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocations.invalid && allocations.live == 0);
    assert_eq!(allocations.allocated, allocations.freed);
    assert!(allocations.allocated <= paid);
}

#[test]
fn append_pays_complete_backing_and_drop_with_exact_short_and_zero_controls() {
    let program = program();
    let function = &program.functions[0];
    for count in [8usize, 9, 16, 17, 64] {
        let observe = |original, limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let before = skeleton_materializations();
                    let result = (|| {
                        let mut work =
                            SkeletonWork::with_index(&program, function, &mut budget, None)?;
                        let mut input = Vec::new();
                        // Input construction belongs to this measured account too.
                        for _ in 0..count {
                            work.push_expr_path(&mut input, empty_expr_path(), "seed")?;
                        }
                        let mut target = Vec::new();
                        if original {
                            before::append_expr_paths(&mut target, input, &mut work, "append")?;
                        } else {
                            append_expr_paths(&mut target, input, &mut work, "append")?;
                        }
                        for _ in 0..3 {
                            work.push_expr_path(&mut target, empty_expr_path(), "following push")?;
                        }
                        assert_eq!(target.len(), count + 3);
                        let capacity = target.capacity();
                        drop(target);
                        Ok::<_, Diagnostic>(capacity)
                    })();
                    let output = result.as_ref().ok().copied();
                    let work = budget.remaining;
                    let materialized = skeleton_materializations() - before;
                    drop(result);
                    drop(budget);
                    (output, work, materialized)
                })
            })
        };
        let ((expected, old_allocation), old_refusal, old_paid, first) = observe(true, usize::MAX);
        assert!(!old_refusal && first.is_none());
        let ((actual, allocation), refused, exact, first) = observe(false, usize::MAX);
        assert!(!refused && first.is_none());
        assert_eq!(actual, expected);
        let removed = (count.next_power_of_two() - 1) * std::mem::size_of::<ExprSkeletonPath>();
        assert_eq!(old_allocation.allocated - allocation.allocated, removed);
        assert_eq!(old_paid - exact, removed - receiving_bytes());
        for limit in [0, exact - 1, exact] {
            let ((outcome, allocation), refused, paid, first) = observe(false, limit);
            assert_eq!(outcome.0.is_some(), limit == exact);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= paid);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), limit != exact);
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn complete_hir_enumeration_keeps_paths_work_materializations_and_legacy() {
    let program = program();
    for paid in [false, true] {
        let (((), allocation), refused, reserved, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    for function in &program.functions {
                        let mut old = if paid {
                            ReplayBudget::new()
                        } else {
                            ReplayBudget::unobserved()
                        };
                        let mut current = if paid {
                            ReplayBudget::new()
                        } else {
                            ReplayBudget::unobserved()
                        };
                        let before = skeleton_materializations();
                        let expected =
                            with_original(|| hir_skeleton_paths(&program, function, &mut old))
                                .unwrap();
                        let old_count = skeleton_materializations() - before;
                        let before = skeleton_materializations();
                        let actual = hir_skeleton_paths(&program, function, &mut current).unwrap();
                        assert_eq!(skeleton_materializations() - before, old_count);
                        assert_eq!(actual, expected);
                        assert_eq!(current.remaining, old.remaining);
                        assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                        drop(actual);
                        drop(expected);
                        drop(current);
                        drop(old);
                    }
                })
            });
        assert!(!refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        if paid {
            assert!(allocation.allocated <= reserved);
        }
    }
}

#[test]
fn complete_old_functions_are_compiled_and_only_capacity_hooks_change() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/minimum-path-array-v1/functions.json"
    )))
    .unwrap();
    let root = include_str!("../../replay.rs");
    let compiled = include_str!("before.rs");
    for record in records.as_array().unwrap() {
        let name = record["name"].as_str().unwrap();
        let body = record["body"].as_str().unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(body.as_bytes()))
            ),
            record["sha256"]
        );
        assert!(compiled.contains(&body.replacen(
            &format!("fn {name}("),
            &format!("pub(super) fn {name}("),
            1
        )));
        let mut current = body.to_owned();
        match name {
            "sequence_skeleton_paths" => current = current.replacen("    let mut combined = Vec::new();\n", "    let mut combined = Vec::new();\n    if !suffixes.is_empty() {\n        path_array::minimum(\n            &mut combined,\n            prefixes.len(),\n            work,\n            \"sequenced path capacity\",\n        )?;\n    }\n", 1),
            "append_expr_paths" => current = current.replacen("    for path in paths {", "    path_array::minimum(target, paths.len(), work, phase)?;\n    for path in paths {", 1),
            "split_status_paths" => current = current.replacen("    let mut results = Vec::new();\n", "    let mut results = Vec::new();\n    path_array::minimum(&mut results, paths.len(), work, \"status path capacity\")?;\n", 1),
            "split_contract" => current = current.replacen("    let mut results = Vec::new();\n", "    let mut results = Vec::new();\n    path_array::minimum(&mut results, paths.len(), work, \"contract path capacity\")?;\n", 1),
            _ => panic!("unregistered receiver"),
        }
        assert_eq!(root.matches(&current).count(), 1, "{name}");
    }
}
