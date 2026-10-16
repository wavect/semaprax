use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use std::cell::Cell;
use std::rc::Rc;
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
    let source="module branch_copy; @id(\"f\") fn f(a:bool,b:bool)->i64 {let n=if a {1}else{2};if b {n+1}else{n+2}} @id(\"main\") fn main()->i64 {f(true,false)}";
    let checked = crate::check(source, "branch-copy.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "branch-copy.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}

#[test]
fn full_plan_enumerator_keeps_every_original_path_work_and_materialization() {
    let program = program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::new();
                let mut current = ReplayBudget::new();
                assert!(old.pays_allocations() && current.pays_allocations());
                let before = skeleton_materializations();
                let expected =
                    with_original(|| compact_stage::original_paths(function, &mut old)).unwrap();
                let old_count = skeleton_materializations() - before;
                let before = skeleton_materializations();
                let actual = plan_skeleton_paths(function, &mut current).unwrap();
                assert_eq!(skeleton_materializations() - before, old_count);
                assert!(compact_stage::paths_match(&actual, &expected));
                assert_eq!(current.remaining, old.remaining);
                assert_eq!(current.skeleton_remaining, old.skeleton_remaining);
                drop(expected);
                drop(actual);
                drop(old);
                drop(current);
            }
        })
    });
    assert!(!refused && first.is_none());
    assert!(!allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    assert!(allocation.allocated <= paid);
}

#[test]
fn clone_and_push_keep_capacity_contents_work_and_drop_with_less_backing() {
    let program = program();
    let function = &program.functions[0];
    for width in [0, 1, 2, 8, 32, 64] {
        for pushes in [1, 3] {
            let values = (0..width).map(Rc::new).collect::<Vec<_>>();
            let next = Rc::new(999usize);
            let observe = |original, limit| {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let before = skeleton_materializations();
                        let result = (|| {
                            let mut copy = if original {
                                skeleton_clone(&mut budget, function, &values, "branch clone")?
                            } else {
                                for_push(&mut budget, function, &values, "branch clone")?
                            };
                            for _ in 0..pushes {
                                skeleton_push(
                                    &mut budget,
                                    function,
                                    &mut copy,
                                    Rc::clone(&next),
                                    "branch push",
                                )?;
                            }
                            assert_eq!(copy.len(), width + pushes);
                            assert!(copy[..width]
                                .iter()
                                .zip(&values)
                                .all(|(a, b)| Rc::ptr_eq(a, b)));
                            assert!(copy[width..].iter().all(|value| Rc::ptr_eq(value, &next)));
                            let capacity = copy.capacity();
                            drop(copy);
                            Ok::<_, Diagnostic>(capacity)
                        })();
                        let capacity = result.as_ref().ok().copied();
                        let work = budget.skeleton_remaining;
                        let materialized = skeleton_materializations() - before;
                        drop(result);
                        drop(budget);
                        (capacity, work, materialized)
                    })
                })
            };
            let ((old, old_allocation), refused, old_paid, first) = observe(true, usize::MAX);
            assert!(!refused && first.is_none());
            assert!(old.0.is_some());
            assert!(!old_allocation.invalid && old_allocation.live == 0);
            assert_eq!(old_allocation.allocated, old_allocation.freed);
            assert!(old_allocation.allocated <= old_paid);
            let ((actual, allocation), refused, exact, first) = observe(false, usize::MAX);
            assert!(!refused && first.is_none());
            assert_eq!(actual, old);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= exact);
            assert_eq!(old_paid - exact, width * std::mem::size_of::<Rc<usize>>());
            assert_eq!(
                old_allocation.allocated - allocation.allocated,
                width * std::mem::size_of::<Rc<usize>>()
            );
            for limit in [0, exact - 1, exact] {
                let ((outcome, allocation), refused, paid, first) = observe(false, limit);
                assert!(!allocation.invalid && allocation.live == 0);
                assert_eq!(allocation.allocated, allocation.freed);
                assert!(allocation.allocated <= paid);
                assert_eq!(outcome.0.is_some(), limit == exact);
                if limit == exact {
                    assert!(!refused && first.is_none());
                    assert_eq!(outcome, actual);
                    assert_eq!(paid, exact);
                } else {
                    assert!(refused && first.is_some());
                    if limit == 0 {
                        assert_eq!(allocation.allocated, 0);
                    }
                }
            }
            assert_eq!(Rc::strong_count(&next), 1);
            assert!(values.iter().all(|value| Rc::strong_count(value) == 1));
        }
    }
}

#[test]
fn whole_original_enumerator_and_clone_push_helpers_remain_byte_exact() {
    use sha2::{Digest, Sha256};
    let root = compact_stage::source_custody::before_root(include_str!("../../replay.rs"));
    let packets = [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-clone-capacity-v1/plan_skeleton_paths.before.rs.txt"
            )),
            "fc722eea9f308a5ccae38965e33eeee30f426173c6fea9cb8a364238049ee2e8",
            true,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-clone-capacity-v1/skeleton_clone.before.rs.txt"
            )),
            "8e797ebf316c2951cb3eb66caf2a06c08859b63b4923bcbcc2c37b03cabef97e",
            false,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/branch-clone-capacity-v1/skeleton_push.before.rs.txt"
            )),
            "4fe08ee24ce1cef9aa4802256e2b59c280705157bb7bd44d432c8fb683649023",
            false,
        ),
    ];
    for (original, hash, changed) in packets {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            hash
        );
        let current = if changed {
            assert_eq!(
                original.matches("let mut branch = skeleton_clone(").count(),
                1
            );
            original.replacen(
                "let mut branch = skeleton_clone(",
                "let mut branch = branch_clone::for_push(",
                1,
            )
        } else {
            original.to_owned()
        };
        assert_eq!(root.matches(&current).count(), 1);
    }
}
