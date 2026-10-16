use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
// These direct receivers start after skeleton preflight. Supply its explicit
// work allowance; new() intentionally leaves that allowance unset. Allocation
// zero/exact/short accounts below keep their original limits and assertions.

fn program() -> ResolvedProgram {
    let source = r#"
module compact_staged_sources;
@id("result_path") fn result_path(value: Result<i64, i64>) -> Result<i64, i64> {
    let payload = value?;
    Result<i64, i64>::Ok { value: payload }
}
@id("option_path") fn option_path(value: Option<i64>) -> Option<i64> {
    let payload = value?;
    Option<i64>::Some { value: payload }
}
@id("main") fn main() -> i64 { 0 }
"#;
    let checked = crate::check(source, "compact-staged-source.spx").unwrap();
    let canonical = crate::format::canonical(&checked);
    let checked = crate::check(&canonical, "compact-staged-source.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    crate::hir::resolve(&checked).unwrap()
}

fn sources(program: &ResolvedProgram) -> Vec<&StagedCopyResultSource> {
    program
        .functions
        .iter()
        .flat_map(|function| &function.cleanup_plan.blocks)
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::StageCopyResult { source } => Some(source),
            _ => None,
        })
        .collect()
}

#[test]
fn all_canonical_fields_order_and_debug_survive_compaction() {
    let program = program();
    let sources = sources(&program);
    assert!(sources
        .iter()
        .any(|source| matches!(source, StagedCopyResultSource::Body { .. })));
    assert!(sources
        .iter()
        .any(|source| matches!(source, StagedCopyResultSource::TryResidual { .. })));
    assert!(sources
        .iter()
        .any(|source| matches!(source, StagedCopyResultSource::TryOptionNone { .. })));
    let (compact, refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let function = &program.functions[0];
        let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
        let mut compact = Vec::new();
        for source in &sources {
            let owned =
                skeleton_clone(&mut budget, function, *source, "canonical test source").unwrap();
            budget
                .reserve_vec(function, &mut compact, 1, "compact test sources")
                .unwrap();
            budget
                .reserve_bytes(
                    function,
                    CopySource::QUERY_CONTROLS,
                    "compact staged-source caller query controls",
                )
                .unwrap();
            match &owned {
                StagedCopyResultSource::Body { .. } => budget
                    .reserve_bytes(
                        function,
                        CopySource::BODY_CONTROLS,
                        "compact staged-source body receiving controls",
                    )
                    .unwrap(),
                _ => budget
                    .reserve_bytes(
                        function,
                        CopySource::RESIDUAL_CONTROLS,
                        "compact staged-source residual receiving controls",
                    )
                    .unwrap(),
            }
            compact.push(CopySource::new(function, &mut budget, owned).unwrap());
        }
        compact
    });
    assert!(!refused && first.is_none());
    for (index, value) in compact.iter().enumerate() {
        assert!(value.matches_canonical(sources[index]));
        assert_eq!(format!("{value:?}"), format!("{:?}", sources[index]));
        assert_eq!(format!("{value:#?}"), format!("{:#?}", sources[index]));
        for (other, canonical) in compact.iter().zip(&sources) {
            assert_eq!(value == other, sources[index] == *canonical);
            assert_eq!(value.cmp(other), sources[index].cmp(canonical));
        }
    }
}

#[test]
fn constructor_and_rc_pay_all_backing_with_zero_exact_short_and_drop() {
    let program = program();
    let function = &program.functions[0];
    for source in sources(&program) {
        let observe = |limit| {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let result = (|| {
                        // Retain the entire original owned-source clone and census.
                        let owned =
                            skeleton_clone(&mut budget, function, source, "test source clone")?;
                        prepare!(function, &mut budget, &owned);
                        let value = CopySource::new(function, &mut budget, owned)?;
                        assert!(value.matches_canonical(source));
                        let mut items: Vec<std::rc::Rc<SkeletonObservation>> = Vec::new();
                        skeleton_push(
                            &mut budget,
                            function,
                            &mut items,
                            SkeletonObservation::StageCopyResult(value),
                            "test observation",
                        )?;
                        assert!(
                            matches!(items[0].as_ref(), SkeletonObservation::StageCopyResult(value)
                    if value.matches_canonical(source))
                        );
                        drop(items);
                        Ok::<_, Diagnostic>(())
                    })();
                    let accepted = result.is_ok();
                    drop(result);
                    drop(budget);
                    accepted
                })
            })
        };
        let ((accepted, allocation), refused, exact, first) = observe(usize::MAX);
        assert!(accepted && !refused && first.is_none());
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
        assert!(allocation.allocated <= exact);
        for limit in [0, exact - 1, exact] {
            let ((accepted, allocation), refused, paid, first) = observe(limit);
            assert_eq!(accepted, limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), limit != exact);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            assert!(allocation.allocated <= paid);
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}

#[test]
fn complete_original_plan_paths_keep_every_field_work_and_materialization() {
    use std::mem::{align_of, size_of};
    assert_eq!(size_of::<before::Observations>(), size_of::<Observations>());
    assert_eq!(
        align_of::<before::Observations>(),
        align_of::<Observations>()
    );
    assert_eq!(size_of::<before::SkeletonPath>(), size_of::<SkeletonPath>());
    assert_eq!(
        align_of::<before::SkeletonPath>(),
        align_of::<SkeletonPath>()
    );
    assert!(
        physical_clone::rc_backing_bytes::<SkeletonObservation>()
            < physical_clone::rc_backing_bytes::<before::SkeletonObservation>()
    );
    let program = program();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            for function in &program.functions {
                let mut old = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let mut current = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let before = skeleton_materializations();
                let expected = original_paths(function, &mut old).unwrap();
                let old_count = skeleton_materializations() - before;
                let before = skeleton_materializations();
                let actual = plan_skeleton_paths(function, &mut current).unwrap();
                assert_eq!(skeleton_materializations() - before, old_count);
                assert!(paths_match(&actual, &expected));
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
    assert!(allocation.allocated <= paid);
}
