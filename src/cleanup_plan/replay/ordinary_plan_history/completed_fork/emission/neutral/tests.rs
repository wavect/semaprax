//! Whole caller, original work and cache continuation; no application claim.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::tests::{fixture, seed};
use crate::test_allocation_observer::measure;

fn run(
    program: &ResolvedProgram,
    cut: usize,
    repeat: bool,
) -> (Result<(), Diagnostic>, usize, usize) {
    let function = &program.functions[0];
    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let leaf = seed(&mut work, 3, 0);
    work.budget
        .reserve_bytes(
            function,
            2 * size_of::<usize>() + size_of::<ObservationDag>() + size_of::<ExprSkeletonPath>(),
            "neutral owned fixture backing",
        )
        .unwrap();
    let root = Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
        None,
        leaf.observations.items,
    )));
    let mut suffix = skeleton_work::empty_expr_path();
    suffix.observations.prefix = Some(root.clone());
    let input = vec![skeleton_work::empty_expr_path()];
    let pointer = input.as_ptr();
    work.budget.skeleton_remaining = cut;
    let spent = work.budget.skeleton_spent;
    reset_skeleton_materializations();
    let result = (|| -> Result<(), Diagnostic> {
        let actual = sequence_skeleton_paths(input, std::slice::from_ref(&suffix), &mut work)?;
        assert_eq!(actual.len(), 1);
        assert_eq!(
            actual.as_ptr(),
            pointer,
            "both receivers reuse the actual input Vec"
        );
        assert!(Rc::ptr_eq(
            actual[0].observations.prefix.as_ref().unwrap(),
            &root
        ));
        assert!(actual[0].observations.items.is_empty());
        assert_eq!((actual[0].failed, actual[0].residual), (false, false));
        assert!(matches!(
            actual[0].continuation,
            lexical_skeleton::Continuation::Normal
        ));
        drop(actual);
        // The same classifier is used by a later ordinary emitter. Omitting
        // publication in the first call would change this continuation's work.
        if repeat {
            work.budget.reserve_bytes(
                function,
                size_of::<ExprSkeletonPath>(),
                "neutral continued fixture backing",
            )?;
            let next = vec![skeleton_work::empty_expr_path()];
            drop(sequence_skeleton_paths(
                next,
                std::slice::from_ref(&suffix),
                &mut work,
            )?);
        }
        Ok(())
    })();
    if result.is_err() {
        assert!(work
            .budget
            .reserve_bytes(function, 0, "neutral sticky retry")
            .is_err());
    }
    let work_spent = work.budget.skeleton_spent - spent;
    drop((work, suffix, root));
    drop(budget);
    (
        result,
        work_spent,
        SKELETON_MATERIALIZATIONS.with(|count| count.get()),
    )
}

fn observed(
    program: &ResolvedProgram,
    old: bool,
    cut: usize,
    repeat: bool,
    expected: &Result<(), Diagnostic>,
) -> (
    ((usize, usize), crate::test_allocation_observer::Metrics),
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
) {
    let body = || {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let (actual, spent, materializations) = run(program, cut, repeat);
                assert_eq!(&actual, expected, "all Diagnostic fields at work cut {cut}");
                drop(actual);
                (spent, materializations)
            })
        })
    };
    if old {
        before(body)
    } else {
        body()
    }
}

#[test]
fn neutral_complete_caller_preserves_cached_work_every_cut_and_whole_drop() {
    let program = fixture();
    for repeat in [false, true] {
        let old_work =
            before(|| with_limit_usage_refusal(usize::MAX, || run(&program, usize::MAX, repeat)).0)
                .1;
        let current = observed(&program, false, usize::MAX, repeat, &Ok(()));
        let old = observed(&program, true, usize::MAX, repeat, &Ok(()));
        assert_eq!(current.0 .0, old.0 .0);
        for record in [&current.0 .1, &old.0 .1] {
            assert!(!record.invalid && record.live == 0);
            assert_eq!(record.allocated, record.freed);
        }
        assert!(current.0 .1.allocated <= current.2);
        assert!(old.0 .1.allocated <= old.2);
        println!("neutral whole repeat={repeat} current={current:?} old={old:?}");
        assert!(
            current.2 < old.2,
            "entire ordinary sequence caller must earn a real byte reduction"
        );
        for cut in 0..=old_work {
            let expected =
                before(|| with_limit_usage_refusal(usize::MAX, || run(&program, cut, repeat)).0).0;
            let a = observed(&program, false, cut, repeat, &expected);
            let b = observed(&program, true, cut, repeat, &expected);
            assert_eq!(a.0 .0, b.0 .0);
            for record in [&a.0 .1, &b.0 .1] {
                assert!(!record.invalid && record.live == 0);
                assert_eq!(record.allocated, record.freed);
            }
        }
    }
}

#[test]
fn neutral_old_caller_source_is_whole_and_unchanged() {
    use sha2::{Digest, Sha256};
    let current = include_str!("../../emission.rs");
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission.rs");
    assert_eq!(std::fs::read(&path).unwrap(), current.as_bytes());
    let saved = include_str!("emission.before.rs.txt");
    assert_eq!(saved.len(), 23389);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(saved.as_bytes()))
        ),
        "f2539cb9f7408df422dd81ddf360625e2a54fcc4ca9c330288e81147ffa894eb"
    );
    let declaration = "pub(in crate::cleanup_plan::replay) mod neutral;\n";
    assert_eq!(current.matches(declaration).count(), 1);
    let mut restored = current.replacen(declaration, "", 1);
    let call = "                $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::neutral::dispatch!($prefixes, $suffixes, $work)";
    assert_eq!(restored.matches(call).count(), 1);
    restored = restored.replacen(call, r#"                match $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::emit!($prefixes, $suffixes, $work)? {
                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::Emission::Complete(paths) => return Ok(paths),
                    $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::Emission::Unsupported(paths) => paths,
                }"#, 1);
    assert_eq!(
        restored, saved,
        "entire independently executable immediate emitter, all fees and controls"
    );
    assert_eq!(std::fs::read(path).unwrap(), current.as_bytes());
}

#[test]
fn neutral_exact_short_positive_floor_and_all_refusal_owners_drop() {
    use crate::bounded_output::set_active_floor;
    let program = fixture();
    for old in [false, true] {
        let observe = |limit, floor, success| {
            let body = || {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        assert!(set_active_floor(floor));
                        let (actual, work, materializations) = run(&program, usize::MAX, false);
                        if success {
                            assert_eq!(actual, Ok(()));
                        } else {
                            assert_eq!(actual.as_ref().unwrap_err().code, "SPX-H006");
                        }
                        drop(actual);
                        (work, materializations)
                    })
                })
            };
            if old {
                before(body)
            } else {
                body()
            }
        };
        let ample = observe(usize::MAX, 0, true);
        assert!(!ample.1 && ample.3.is_none());
        let paid = ample.2;
        assert!(paid > 1);
        for (limit, floor, success) in [
            (paid, 0, true),
            (paid - 1, 0, false),
            (paid + 128, 128, true),
            (paid + 127, 128, false),
        ] {
            let actual = observe(limit, floor, success);
            assert_eq!(actual.1, !success);
            assert_eq!(actual.3.is_some(), !success);
            if success {
                assert_eq!((actual.0 .0, actual.2), (ample.0 .0, paid));
            }
            let heap = actual.0 .1;
            assert!(
                !heap.invalid && heap.live == 0,
                "old={old} floor={floor}: {actual:?}"
            );
            assert_eq!(heap.allocated, heap.freed);
        }
        println!("neutral byte frontier old={old} ample={ample:?}");
    }
}

// Complete receiver: fixture owners, cached continuation, output and budget all
// Drop in the measured invocation. No allocator warm-up or live-byte subtraction.
fn run_many(
    program: &ResolvedProgram,
    width: usize,
    cut: usize,
    repeat: bool,
    flat_tail: bool,
) -> (Result<(), Diagnostic>, usize, usize) {
    let function = &program.functions[0];
    let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
    let mut work = SkeletonWork::new(program, function, &mut budget);
    let leaf = seed(&mut work, 3, 0);
    work.budget
        .reserve_bytes(
            function,
            2 * size_of::<usize>() + size_of::<ObservationDag>() + size_of::<ExprSkeletonPath>(),
            "neutral many fixture root and input backing",
        )
        .unwrap();
    let root = Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
        None,
        leaf.observations.items,
    )));
    let mut suffixes = Vec::new();
    for index in 0..width {
        work.budget
            .reserve_vec(
                function,
                &mut suffixes,
                1,
                "neutral many fixture suffix backing",
            )
            .unwrap();
        let mut suffix = skeleton_work::empty_expr_path();
        suffix.observations.prefix = Some(root.clone());
        suffix.failed = index % 3 == 1;
        suffix.residual = index % 3 == 2;
        if flat_tail && index + 1 == width {
            suffix.observations = seed(&mut work, 1, 0).observations;
        }
        suffixes.push(suffix);
    }
    let input = vec![skeleton_work::empty_expr_path()];
    work.budget.skeleton_remaining = cut;
    let spent = work.budget.skeleton_spent;
    reset_skeleton_materializations();
    // Keep the first input uniquely owned; execute the same full caller for
    // initial and continued calls, with output checks inside this closure.
    let result = (|| -> Result<(), Diagnostic> {
        let actual = sequence_skeleton_paths(input, &suffixes, &mut work)?;
        assert_eq!(actual.len(), width);
        for (index, path) in actual.iter().enumerate() {
            assert_eq!(
                (path.failed, path.residual),
                (index % 3 == 1, index % 3 == 2)
            );
            assert!(matches!(
                path.continuation,
                lexical_skeleton::Continuation::Normal
            ));
            if !flat_tail || index + 1 != width {
                assert!(Rc::ptr_eq(
                    path.observations.prefix.as_ref().unwrap(),
                    &root
                ));
                assert!(path.observations.items.is_empty());
            }
        }
        drop(actual);
        if repeat {
            work.budget.reserve_bytes(
                function,
                size_of::<ExprSkeletonPath>(),
                "neutral many continuation input backing",
            )?;
            let input = vec![skeleton_work::empty_expr_path()];
            let actual = sequence_skeleton_paths(input, &suffixes, &mut work)?;
            assert_eq!(actual.len(), width);
            drop(actual);
        }
        Ok(())
    })();
    if result.is_err() {
        assert!(work
            .budget
            .reserve_bytes(function, 0, "neutral many sticky retry")
            .is_err());
    }
    let spent = work.budget.skeleton_spent - spent;
    drop((work, suffixes, root));
    drop(budget);
    (
        result,
        spent,
        SKELETON_MATERIALIZATIONS.with(|count| count.get()),
    )
}

#[test]
fn neutral_many_complete_caller_preserves_all_work_cuts_cache_outputs_and_drop() {
    let program = fixture();
    for (width, repeat, flat_tail) in [(2, false, false), (8, true, false), (3, true, true)] {
        let old_work = before(|| {
            with_limit_usage_refusal(usize::MAX, || {
                run_many(&program, width, usize::MAX, repeat, flat_tail)
            })
            .0
        })
        .1;
        let observe = |old, cut, expected: &Result<(), Diagnostic>| {
            let body = || {
                with_limit_usage_refusal(usize::MAX, || {
                    measure(|| {
                        let (actual, spent, materializations) =
                            run_many(&program, width, cut, repeat, flat_tail);
                        assert_eq!(&actual, expected, "width={width} cut={cut} old={old}");
                        drop(actual);
                        (spent, materializations)
                    })
                })
            };
            if old {
                before(body)
            } else {
                body()
            }
        };
        let old = observe(true, usize::MAX, &Ok(()));
        let current = observe(false, usize::MAX, &Ok(()));
        assert_eq!(current.0 .0, old.0 .0);
        for record in [&old, &current] {
            assert!(!record.1 && record.3.is_none());
            assert!(!record.0 .1.invalid && record.0 .1.live == 0);
            assert_eq!(record.0 .1.allocated, record.0 .1.freed);
            assert!(record.0 .1.allocated <= record.2);
        }
        if !flat_tail && width == 8 {
            assert!(
                current.2 < old.2,
                "complete old caller invoice, no baseline padding"
            );
        }
        eprintln!("NEUTRAL_MANY width={width} repeat={repeat} flat_tail={flat_tail} current={current:?} old={old:?}");
        for cut in 0..=old_work {
            let expected = before(|| {
                with_limit_usage_refusal(usize::MAX, || {
                    run_many(&program, width, cut, repeat, flat_tail)
                })
                .0
            })
            .0;
            let old = observe(true, cut, &expected);
            let current = observe(false, cut, &expected);
            assert_eq!(current.0 .0, old.0 .0);
            for record in [&old, &current] {
                assert!(!record.0 .1.invalid && record.0 .1.live == 0);
                assert_eq!(record.0 .1.allocated, record.0 .1.freed);
            }
        }
    }
}

#[test]
fn neutral_many_exact_short_and_positive_floor_keep_complete_drop() {
    use crate::bounded_output::set_active_floor;
    let program = fixture();
    for old in [false, true] {
        let observe = |limit, floor, success| {
            let body = || {
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        assert!(set_active_floor(floor));
                        let (actual, spent, materializations) =
                            run_many(&program, 8, usize::MAX, true, false);
                        if success {
                            assert_eq!(actual, Ok(()));
                        } else {
                            assert_eq!(actual.as_ref().unwrap_err().code, "SPX-H006");
                        }
                        drop(actual);
                        (spent, materializations)
                    })
                })
            };
            if old {
                before(body)
            } else {
                body()
            }
        };
        let ample = observe(usize::MAX, 0, true);
        assert!(!ample.1 && ample.3.is_none());
        let paid = ample.2;
        for (limit, floor, success) in [
            (paid, 0, true),
            (paid - 1, 0, false),
            (paid + 128, 128, true),
            (paid + 127, 128, false),
        ] {
            let actual = observe(limit, floor, success);
            assert_eq!(actual.1, !success);
            assert_eq!(actual.3.is_some(), !success);
            if success {
                assert_eq!((actual.0 .0, actual.2), (ample.0 .0, paid));
            }
            assert!(!actual.0 .1.invalid && actual.0 .1.live == 0);
            assert_eq!(actual.0 .1.allocated, actual.0 .1.freed);
        }
        eprintln!("NEUTRAL_MANY_BYTE_FRONTIER old={old} ample={ample:?}");
    }
}
