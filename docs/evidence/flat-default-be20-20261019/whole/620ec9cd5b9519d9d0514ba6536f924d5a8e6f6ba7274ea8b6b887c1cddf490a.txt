//! Source-authored owning gates; runtime qualification is intentionally pending.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let checked = crate::check(
        "module enum_prefix; @id(\"f\") fn f(a:bool)->i64 {if a {1}else{2}} @id(\"main\") fn main()->i64 {f(true)}",
        "enum-prefix.spx",
    ).unwrap();
    crate::hir::resolve(&checked).unwrap()
}
fn seed(work: &mut SkeletonWork<'_, '_>, len: usize) -> Result<ExprSkeletonPath, Diagnostic> {
    let mut path = empty_expr_path();
    for index in 0..len {
        let expression = work.function.body.id.clone();
        work.push_observation(
            &mut path,
            SkeletonObservation::Boolean {
                expression,
                value: index % 2 == 0,
            },
            "enumerated prefix seed",
        )?;
    }
    Ok(path)
}
fn inputs(
    work: &mut SkeletonWork<'_, '_>,
    len: usize,
    count: usize,
) -> Result<(Vec<ExprSkeletonPath>, Vec<ExprSkeletonPath>), Diagnostic> {
    let prefix = seed(work, len)?;
    let mut prefixes = Vec::new();
    work.push_expr_path(&mut prefixes, prefix, "enumerated prefix input")?;
    let mut suffixes = Vec::new();
    for index in 0..count {
        let mut suffix = seed(work, index % 3)?;
        suffix.failed = index == 1;
        suffix.residual = index == 2;
        suffix.owned_source = Some(CleanupPlace::whole(StorageId::ProvisionalResult));
        work.push_expr_path(&mut suffixes, suffix, "enumerated suffix input")?;
    }
    Ok((prefixes, suffixes))
}
fn assert_same(
    actual: &[ExprSkeletonPath],
    expected: &[ExprSkeletonPath],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.owned_source, expected.owned_source);
        assert_eq!(actual.failed, expected.failed);
        assert_eq!(actual.residual, expected.residual);
        assert_eq!(actual.continuation, expected.continuation);
        budget
            .reserve_bytes(
                function,
                CURSOR_STORAGE + CURSOR_CALL,
                "test cursor receivers",
            )
            .unwrap();
        let mut cursor = Cursor::new();
        cursor
            .prepare(&actual.observations, function, budget)
            .unwrap();
        for observation in &expected.observations.items {
            budget
                .reserve_bytes(function, CURSOR_CALL, "test cursor next")
                .unwrap();
            assert_eq!(
                cursor.next(function, budget).unwrap(),
                Some(observation.as_ref())
            );
        }
        budget
            .reserve_bytes(function, CURSOR_CALL, "test cursor next")
            .unwrap();
        assert_eq!(cursor.next(function, budget).unwrap(), None);
    }
}

#[test]
fn whole_old_sequence_content_states_multiplicity_and_original_skeleton_work_survive() {
    let program = program();
    let function = &program.functions[0];
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        for len in [0, 1, 4095, 4096, 8192] {
            for count in [0, 1, 2, 3, 5] {
                let mut old = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut new = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut work = SkeletonWork::new(&program, function, &mut old);
                let (prefixes, suffixes) = inputs(&mut work, len, count).unwrap();
                let start = skeleton_materializations();
                let expected =
                    before::sequence_skeleton_paths(prefixes, &suffixes, &mut work).unwrap();
                let events = skeleton_materializations() - start;
                drop((suffixes, work));
                let mut work = SkeletonWork::new(&program, function, &mut new);
                let (prefixes, suffixes) = inputs(&mut work, len, count).unwrap();
                let start = skeleton_materializations();
                let actual =
                    super::super::super::sequence_skeleton_paths(prefixes, &suffixes, &mut work)
                        .unwrap();
                assert_eq!(skeleton_materializations() - start, events);
                drop((suffixes, work));
                // New bound/cursor work is ordinary free work, never the held
                // original skeleton loan or a new quota.
                assert_eq!(new.skeleton_spent, old.skeleton_spent);
                assert_eq!(new.skeleton_remaining, old.skeleton_remaining);
                assert!(new.remaining <= old.remaining);
                assert_same(&actual, &expected, function, &mut new);
                if len >= 4096 && count >= 2 {
                    assert!(actual.iter().all(|path| path.observations.prefix.is_some()));
                }
                drop((actual, expected, new, old));
            }
        }
    });
    assert!(!refused && first.is_none());
}

#[test]
fn independently_allocated_equal_sequences_sort_by_content_and_keep_terminal_multiplicity() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let mut paths = Vec::new();
        for value in [true, false, true] {
            let mut path = seed(&mut work, 7).unwrap();
            work.push_observation(
                &mut path,
                SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value,
                },
                "independent sequence terminal observation",
            )
            .unwrap();
            work.budget
                .reserve_bytes(function, SHARE_CONTROLS, "test node receivers")
                .unwrap();
            let history = node(
                &mut work,
                ObservationDagKind::Seq(None, std::mem::take(&mut path.observations.items)),
            )
            .unwrap();
            path.observations.prefix = Some(history);
            work.push_skeleton_path(
                &mut paths,
                SkeletonPath {
                    observations: path.observations,
                    terminal: SkeletonTerminal::Success,
                },
                "independent sequence path",
            )
            .unwrap();
        }
        let mut flat = Vec::new();
        for value in [false, true, true] {
            let mut path = seed(&mut work, 7).unwrap();
            work.push_observation(
                &mut path,
                SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value,
                },
                "flat terminal observation",
            )
            .unwrap();
            work.push_skeleton_path(
                &mut flat,
                SkeletonPath {
                    observations: path.observations,
                    terminal: SkeletonTerminal::Success,
                },
                "flat comparison path",
            )
            .unwrap();
        }
        drop(work);
        sort(&mut paths, function, &mut budget).unwrap();
        flat.sort();
        assert!(equal(&paths, &flat, function, &mut budget).unwrap());
        paths[0].terminal = SkeletonTerminal::Failure;
        assert!(!equal(&paths, &flat, function, &mut budget).unwrap());
        paths[0].terminal = SkeletonTerminal::Success;
        flat.pop();
        assert!(!equal(&paths, &flat, function, &mut budget).unwrap());
    });
}

#[test]
fn sequence_and_cursor_real_byte_frontiers_preserve_six_field_diagnostics_and_full_drop() {
    let program = program();
    let function = &program.functions[0];
    let owner = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let result = (|| {
                    let mut work = SkeletonWork::with_index(&program, function, &mut budget, None)?;
                    let (prefixes, suffixes) = inputs(&mut work, 4096, 3)?;
                    let actual = super::super::super::sequence_skeleton_paths(
                        prefixes, &suffixes, &mut work,
                    )?;
                    drop(work);
                    budget.reserve_bytes(
                        function,
                        CURSOR_STORAGE + CURSOR_CALL,
                        "owner cursor receivers",
                    )?;
                    let mut cursor = Cursor::new();
                    cursor.prepare(&actual[0].observations, function, &mut budget)?;
                    loop {
                        budget.reserve_bytes(function, CURSOR_CALL, "owner cursor next")?;
                        if cursor.next(function, &mut budget)?.is_none() {
                            break;
                        }
                    }
                    Ok::<(), Diagnostic>(())
                })();
                // Keep the original budget-before-error Drop order. The full
                // Diagnostic must be freed inside the allocation observer too.
                drop(budget);
                result.map_err(DiagnosticFields::capture)
            })
        })
    };
    let ((success, full), refused, exact, _) = owner(usize::MAX);
    success.unwrap();
    assert!(!refused && !full.invalid && full.live == 0);
    assert_eq!(full.allocated, full.freed);
    for limit in [0, exact - 1, exact] {
        let ((result, heap), refused, paid, first) = owner(limit);
        assert!(!heap.invalid && heap.live == 0);
        assert_eq!(heap.allocated, heap.freed);
        assert!(heap.allocated <= paid);
        assert_eq!(result.is_ok(), limit == exact);
        if let Err(error) = result {
            assert!(refused && first.is_some());
            assert_eq!(error.code, "SPX-H006");
            let ((again, _), _, _, _) = owner(limit);
            let again = again.unwrap_err();
            assert_eq!(error.code, again.code);
            assert_eq!(error.severity, again.severity);
            assert_eq!(error.message, again.message);
            assert_eq!(error.path, again.path);
            assert_eq!(error.span, again.span);
            assert_eq!(error.help, again.help);
        }
    }
}

#[test]
fn shared_hir_content_is_consumed_by_both_flat_and_immutable_plan_comparators() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut expected =
            super::super::super::hir_skeleton_paths(&program, function, &mut budget).unwrap();
        // Independently node each complete expected path; no shared pointer
        // identity may become authority for a content-equivalent PLAN path.
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        for path in &mut expected {
            work.budget
                .reserve_bytes(function, SHARE_CONTROLS, "test HIR node receiver")
                .unwrap();
            path.observations.prefix = Some(
                node(
                    &mut work,
                    ObservationDagKind::Seq(
                        path.observations.prefix.take(),
                        std::mem::take(&mut path.observations.items),
                    ),
                )
                .unwrap(),
            );
        }
        drop(work);
        sort(&mut expected, function, &mut budget).unwrap();
        let mut actual = super::super::super::plan_skeleton_paths(function, &mut budget).unwrap();
        actual.sort();
        assert!(equal(&expected, &actual, function, &mut budget).unwrap());
        let history =
            super::super::super::ordinary_plan_history::produce(function, &mut budget).unwrap();
        super::super::super::ordinary_plan_history::compare(
            function,
            &mut budget,
            history,
            &expected,
        )
        .unwrap();
    });
}

#[test]
fn first_original_work_failure_and_sticky_floor_settlement_are_not_replaced() {
    let program = program();
    let function = &program.functions[0];
    for old in [true, false] {
        let (error, _, _, _) = with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let (prefixes, suffixes) = inputs(&mut work, 4096, 3).unwrap();
            work.budget.skeleton_remaining = 0;
            let error = if old {
                before::sequence_skeleton_paths(prefixes, &suffixes, &mut work)
            } else {
                super::super::super::sequence_skeleton_paths(prefixes, &suffixes, &mut work)
            }
            .err()
            .unwrap();
            assert!(error.message.contains("work"));
            error
        });
        assert_eq!(error.code, "SPX-H006");
    }
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        budget.prepare_builder(function).unwrap();
        assert!(crate::bounded_output::set_active_floor(
            crate::bounded_output::active_remaining().unwrap()
        ));
        let first = budget
            .reserve_bytes(function, SHARE_CONTROLS, "enumerated HIR floor")
            .unwrap_err();
        let retry = budget
            .reserve_bytes(function, 0, "enumerated HIR sticky retry")
            .unwrap_err();
        // The original complete first Diagnostic stays selected. Retrying a
        // consumed refusal owner returns only its stable code and empty text,
        // without replacing the already selected Diagnostic or allocating.
        assert_eq!(first.code, retry.code);
        assert!(retry.message.is_empty());
        assert!(first.message.contains("enumerated HIR floor"));
    });
    assert!(refused);
}

#[test]
fn complete_owned_contract_residual_corpus_keeps_independent_flat_hir_and_plan_outputs() {
    let source = r#"module enum_owned;
@id("choose") fn choose(left:own Bytes,right:own Bytes,gate:bool)->Bytes {
 if gate {left}else{right}
}
@id("fallible") fn fallible(value:own Result<Bytes,Bytes>)->Result<Bytes,Bytes> {
 let payload=value?;
 Result<Bytes,Bytes>::Ok {value:payload}
}
@id("contract") fn contract(value:i64)->i64
 requires value>=0
 ensures result>=0
 {value}
@id("main") fn main()->i64 {0}
"#;
    let checked = crate::check(source, "enum-owned.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    with_limit_usage_refusal(usize::MAX, || {
        for function in &program.functions {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut expected =
                super::super::super::hir_skeleton_paths(&program, function, &mut budget).unwrap();
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            for path in &mut expected {
                work.budget
                    .reserve_bytes(function, SHARE_CONTROLS, "owned HIR sequence receiving")
                    .unwrap();
                path.observations.prefix = Some(
                    node(
                        &mut work,
                        ObservationDagKind::Seq(
                            path.observations.prefix.take(),
                            std::mem::take(&mut path.observations.items),
                        ),
                    )
                    .unwrap(),
                );
            }
            drop(work);
            sort(&mut expected, function, &mut budget).unwrap();
            let mut flat = super::super::super::plan_skeleton_paths(function, &mut budget).unwrap();
            flat.sort();
            assert!(equal(&expected, &flat, function, &mut budget).unwrap());
            let history =
                super::super::super::ordinary_plan_history::produce(function, &mut budget).unwrap();
            super::super::super::ordinary_plan_history::compare(
                function,
                &mut budget,
                history,
                &expected,
            )
            .unwrap();
        }
    });
}

#[test]
fn complete_frozen_sequence_and_comparison_bodies_are_independently_compiled_verbatim() {
    let frozen = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay.rs.before.txt"
    ));
    let start = frozen.find("fn sequence_skeleton_paths(\n").unwrap();
    let end = frozen[start..]
        .find("\nuse path_summary::has_active_paths;")
        .unwrap()
        + start;
    let compare_start = frozen
        .find("fn validate_typed_control_skeleton_with_index")
        .unwrap();
    let compare_end = frozen[compare_start..]
        .find("\n#[cfg(test)]\nfn hir_skeleton_paths")
        .unwrap()
        + compare_start;
    let expected = format!("//! Complete frozen36 ordinary sequence receiver, independently compiled.\nuse super::super::*;\n\npub(super) {}\n#[allow(dead_code)]\npub(super) {}", &frozen[start..end], &frozen[compare_start..compare_end]);
    assert_eq!(include_str!("before.rs"), expected);
    let plan = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/ordinary-hir-prefix-877-v1/src__cleanup_plan__replay__ordinary_plan_history.rs.before.txt"));
    let start = plan.find("pub(super) fn compare(").unwrap();
    assert_eq!(include_str!("../../ordinary_plan_history/before_hir_compare.rs"),
        format!("//! Complete frozen36 PLAN/HIR comparator, independently compiled.\nuse super::*;\n\n{}", &plan[start..]));
}

#[test]
fn whole_sequence_owner_reports_paid_allocated_peak_and_complete_drop_for_small_and_long_fanout() {
    let program = program();
    let function = &program.functions[0];
    let run = |old, len, count| {
        with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
                let mut work = SkeletonWork::new(&program, function, &mut budget);
                let (prefixes, suffixes) = inputs(&mut work, len, count).unwrap();
                let result = if old {
                    before::sequence_skeleton_paths(prefixes, &suffixes, &mut work)
                } else {
                    super::super::super::sequence_skeleton_paths(prefixes, &suffixes, &mut work)
                }
                .unwrap();
                let paths = result.len();
                drop((result, suffixes, work));
                drop(budget);
                paths
            })
        })
    };
    for len in [1, 4096, 8192] {
        let ((old, old_heap), old_refused, old_paid, _) = run(true, len, 5);
        let ((new, new_heap), new_refused, new_paid, _) = run(false, len, 5);
        assert_eq!(new, old);
        assert!(!old_refused && !new_refused);
        for heap in [&old_heap, &new_heap] {
            assert!(!heap.invalid && heap.live == 0);
            assert_eq!(heap.allocated, heap.freed);
        }
        if len == 1 {
            assert_eq!(old_paid, new_paid);
            assert_eq!(old_heap.allocated, new_heap.allocated);
        } else {
            assert!(new_heap.allocated < old_heap.allocated);
        }
        eprintln!("OPT877 sequence-only len={len} old_paid={old_paid} new_paid={new_paid} old_heap={old_heap:?} new_heap={new_heap:?}; comparator costs are additional and not included");
    }
}

#[test]
fn original_oracle_retains_full_old_accounts_without_entering_shared_storage() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let run = |old| {
            let scope = allocation::OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
            let invocation = allocation::enter(&program, function, scope.lease()).unwrap();
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(usize::MAX, scope.lease());
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let (prefixes, suffixes) = inputs(&mut work, 8192, 5).unwrap();
            let paths = if old {
                before::sequence_skeleton_paths(prefixes, &suffixes, &mut work)
            } else {
                super::super::super::sequence_skeleton_paths(prefixes, &suffixes, &mut work)
            }
            .unwrap();
            assert!(paths.iter().all(|path| path.observations.prefix.is_none()));
            drop((paths, suffixes, work));
            drop(budget);
            drop(invocation);
            let receipt = scope.receipt();
            drop(scope);
            receipt
        };
        assert_eq!(run(true), run(false));
    });
}

#[test]
fn repeated_long_tail_freeze_retains_all_earlier_segment_contents() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let mut prefix = seed(&mut work, 4096).unwrap();
        work.budget
            .reserve_bytes(function, SHARE_CONTROLS, "repeated node receiver")
            .unwrap();
        prefix.observations.prefix = Some(
            node(
                &mut work,
                ObservationDagKind::Seq(None, std::mem::take(&mut prefix.observations.items)),
            )
            .unwrap(),
        );
        for index in 0..4096 {
            work.push_observation(
                &mut prefix,
                SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value: index % 2 == 0,
                },
                "repeated shared tail",
            )
            .unwrap();
        }
        let mut prefixes = Vec::new();
        work.push_expr_path(&mut prefixes, prefix, "repeated input")
            .unwrap();
        let mut suffixes = Vec::new();
        for _ in 0..2 {
            let suffix = seed(&mut work, 1).unwrap();
            work.push_expr_path(&mut suffixes, suffix, "repeated suffix")
                .unwrap();
        }
        let actual =
            super::super::super::sequence_skeleton_paths(prefixes, &suffixes, &mut work).unwrap();
        let mut expected = Vec::new();
        for _ in 0..2 {
            let mut flat = seed(&mut work, 8192).unwrap();
            work.push_observation(
                &mut flat,
                SkeletonObservation::Boolean {
                    expression: function.body.id.clone(),
                    value: true,
                },
                "repeated flat suffix",
            )
            .unwrap();
            work.push_expr_path(&mut expected, flat, "repeated flat output")
                .unwrap();
        }
        drop(work);
        assert_same(&actual, &expected, function, &mut budget);
    });
}

#[test]
fn sequence_marker_never_authenticates_foreign_observation_contents_or_lent_work() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let mut left = seed(&mut work, 1).unwrap();
        let mut right = seed(&mut work, 1).unwrap();
        // Different exact source identities, identical representation marker.
        right.observations.items.clear();
        work.push_observation(
            &mut right,
            SkeletonObservation::Boolean {
                expression: program.functions[1].body.id.clone(),
                value: true,
            },
            "foreign observation",
        )
        .unwrap();
        for path in [&mut left, &mut right] {
            work.budget
                .reserve_bytes(function, SHARE_CONTROLS, "foreign sequence receiver")
                .unwrap();
            path.observations.prefix = Some(
                node(
                    &mut work,
                    ObservationDagKind::Seq(None, std::mem::take(&mut path.observations.items)),
                )
                .unwrap(),
            );
        }
        let left = SkeletonPath {
            observations: left.observations,
            terminal: SkeletonTerminal::Success,
        };
        let right = SkeletonPath {
            observations: right.observations,
            terminal: SkeletonTerminal::Success,
        };
        drop(work);
        budget
            .reserve_bytes(function, ORDER_CALL, "foreign content order call")
            .unwrap();
        assert_ne!(
            order(&left, &right, function, &mut budget).unwrap(),
            Ordering::Equal
        );
    });
    for (lent, merged) in [(true, false), (false, true)] {
        with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            work.budget
                .reserve_bytes(function, SHARE_CONTROLS, "inadmissible sequence receiver")
                .unwrap();
            work.budget.lent_skeleton = lent;
            work.budget.merge_paths = merged;
            assert!(node(&mut work, ObservationDagKind::Seq(None, Vec::new())).is_err());
            assert_eq!(work.budget.skeleton_spent, 0);
        });
    }
}

#[test]
fn shared_suffix_concatenation_and_prepend_keep_complete_order_with_free_metadata_work() {
    let program = program();
    let function = &program.functions[0];
    with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::with_skeleton_limit(usize::MAX);
        let mut work = SkeletonWork::new(&program, function, &mut budget);
        let mut head = seed(&mut work, 2).unwrap();
        let mut suffix = seed(&mut work, 2).unwrap();
        let mut expected = seed(&mut work, 4).unwrap();
        for path in [&mut head, &mut suffix] {
            work.budget
                .reserve_bytes(function, SHARE_CONTROLS, "shared concatenation node")
                .unwrap();
            path.observations.prefix = Some(
                node(
                    &mut work,
                    ObservationDagKind::Seq(None, std::mem::take(&mut path.observations.items)),
                )
                .unwrap(),
            );
        }
        head.observations = work
            .extend_owned_observations(
                head.observations,
                &suffix.observations,
                "shared concatenation extension",
            )
            .unwrap();
        for path in [&mut head, &mut expected] {
            let expression = function.body.id.clone();
            work.insert_front_observation(
                &mut path.observations,
                SkeletonObservation::Boolean {
                    expression,
                    value: false,
                },
                "shared prepend operation",
            )
            .unwrap();
        }
        drop(work);
        assert_same(&[head], &[expected], function, &mut budget);
    });
}

#[test]
fn whole_frozen_sequence_and_plan_comparator_run_independently_in_original_scope() {
    let program = program();
    with_limit_usage_refusal(usize::MAX, || {
        for function in &program.functions {
            let mut old = ReplayBudget::with_skeleton_limit(usize::MAX);
            let mut new = ReplayBudget::with_skeleton_limit(usize::MAX);
            with_original(|| {
                before::validate_typed_control_skeleton_with_index(
                    &program, function, &mut old, None, None,
                )
            })
            .unwrap();
            assert!(!original_selected());
            super::super::super::validate_typed_control_skeleton_with_index(
                &program, function, &mut new, None, None,
            )
            .unwrap();
            assert_eq!(new.skeleton_spent, old.skeleton_spent);
            assert_eq!(new.skeleton_remaining, old.skeleton_remaining);
        }
    });
}

// Observation-only transport: keep every field byte, not an owning refusal
// String beyond measure(). This fixture's prepaid refusal text is bounded by
// its short function IDs plus the existing 512-byte refusal backing contract.
// The reporting bound is checked, never truncated or used as compiler funding.
#[derive(Debug, Eq, PartialEq)]
struct DiagnosticText {
    bytes: [u8; 1024],
    len: usize,
}
impl DiagnosticText {
    fn capture(value: &str) -> Self {
        let mut bytes = [0; 1024];
        assert!(
            value.len() <= bytes.len(),
            "complete fixture Diagnostic must fit reporting storage"
        );
        bytes[..value.len()].copy_from_slice(value.as_bytes());
        Self {
            bytes,
            len: value.len(),
        }
    }
}
#[derive(Debug, Eq, PartialEq)]
struct DiagnosticFields {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: DiagnosticText,
    path: Option<DiagnosticText>,
    span: Option<crate::ast::Span>,
    help: Option<DiagnosticText>,
}
impl DiagnosticFields {
    fn capture(error: Diagnostic) -> Self {
        Self {
            code: error.code,
            severity: error.severity,
            message: DiagnosticText::capture(&error.message),
            path: error.path.as_deref().map(DiagnosticText::capture),
            span: error.span,
            help: error.help.as_deref().map(DiagnosticText::capture),
        }
        // The original owning Diagnostic drops here, before measure returns.
    }
}

#[test]
fn oracle_entry_still_refuses_missing_and_same_id_foreign_invocation_without_active_fallback() {
    let program = program();
    let function = &program.functions[0];
    let foreign = function.clone();
    assert_eq!(foreign.id, function.id);
    for entered_foreign in [false, true] {
        let ((), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
            let active = crate::bounded_output::active_remaining();
            let scope = allocation::OracleScope::open_with_work(usize::MAX, usize::MAX).unwrap();
            let invocation = if entered_foreign {
                Some(allocation::enter(&program, &foreign, scope.lease()).unwrap())
            } else {
                None
            };
            let mut budget = ReplayBudget::with_skeleton_limit_oracle(usize::MAX, scope.lease());
            let error = SkeletonWork::with_index(&program, function, &mut budget, None)
                .err()
                .unwrap();
            assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
            assert!(budget.refusal_spent);
            assert_eq!((budget.remaining, budget.skeleton_remaining), (0, 0));
            let selected = scope.receipt();
            assert_eq!((selected.charged, selected.work), (0, 0));
            assert!(selected.physical_failure.is_some());
            let retry = budget
                .reserve_bytes(function, 0, "oracle invocation sticky retry")
                .unwrap_err();
            assert_eq!(retry, error);
            let retried = scope.receipt();
            assert_eq!(
                (retried.charged, retried.work, retried.physical_failure),
                (selected.charged, selected.work, selected.physical_failure)
            );
            assert_eq!(crate::bounded_output::active_remaining(), active);
            drop((retry, error));
            drop(budget);
            drop(invocation);
            drop(scope);
        });
        assert!(!refused && first.is_none());
        assert_eq!(paid, 0);
    }
}

// Numeric diagnostic observation only. These copies preserve every original
// receiver statement; exact rename/hook inverses are retained in the packet.
// No capture spends work/bytes, changes a result or selects a compiler route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ComparisonCheckpoint {
    ordinary_remaining: usize,
    skeleton_remaining: usize,
    skeleton_spent: usize,
    physical_remaining: usize,
    heap: crate::test_allocation_observer::Metrics,
}
#[derive(Clone, Copy, Debug)]
struct ComparisonBoundaryReport {
    immutable_plan: bool,
    before: ComparisonCheckpoint,
    after: Option<ComparisonCheckpoint>,
}
thread_local! {
    static COMPARISON_BOUNDARY_REPORT: std::cell::RefCell<Option<ComparisonBoundaryReport>> = const { std::cell::RefCell::new(None) };
}
fn comparison_checkpoint(budget: &ReplayBudget) -> ComparisonCheckpoint {
    ComparisonCheckpoint {
        ordinary_remaining: budget.remaining,
        skeleton_remaining: budget.skeleton_remaining,
        skeleton_spent: budget.skeleton_spent,
        physical_remaining: crate::bounded_output::active_remaining().unwrap(),
        heap: crate::test_allocation_observer::current_metrics(),
    }
}
fn comparison_report_begin(budget: &ReplayBudget, immutable_plan: bool) {
    let before = comparison_checkpoint(budget);
    COMPARISON_BOUNDARY_REPORT.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(
            slot.is_none(),
            "one complete comparison receiver per report"
        );
        *slot = Some(ComparisonBoundaryReport {
            immutable_plan,
            before,
            after: None,
        });
    });
}
fn comparison_report_end(budget: &ReplayBudget) {
    let after = comparison_checkpoint(budget);
    COMPARISON_BOUNDARY_REPORT.with(|slot| {
        let mut slot = slot.borrow_mut();
        let report = slot.as_mut().expect("real comparison entry observed");
        assert!(report.after.is_none());
        report.after = Some(after);
    });
}

#[allow(unused_imports)]
// Preserve the complete independently compiled historical receiver imports.
use crate::cleanup_plan::replay::*;
fn comparison_report_old_receiver<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    mut continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    if function.cleanup_plan.lexical_control.is_none()
        && factored::validate_with_index_storage(
            program,
            function,
            budget,
            helpers,
            continuations.as_deref_mut(),
        )?
    {
        return Ok(());
    }
    let mut expected =
        hir_skeleton_paths_with_index_storage(program, function, budget, helpers, continuations)?;
    // Match the existing received reference, with no new payer/tag/flag carrier.
    if let ReplayBudget {
        payer: AllocationPayer::ActiveBuilder,
        ..
    } = budget
    {
        budget.reserve_bytes(
            function,
            ordinary_plan_history::SELECT_CALL_CONTROLS,
            "ordinary plan history selector call receivers",
        )?;
        if ordinary_plan_history::select(function, budget)? {
            budget.reserve_bytes(
                function,
                ordinary_plan_history::PRODUCE_CALL_CONTROLS,
                "ordinary plan history producer call receivers",
            )?;
            let actual = ordinary_plan_history::produce(function, budget)?;
            comparison_report_begin(budget, true);
            expected.sort();
            budget.reserve_bytes(
                function,
                ordinary_plan_history::COMPARE_CALL_CONTROLS,
                "ordinary plan history owned comparison call receivers",
            )?;
            let observed_result =
                ordinary_plan_history::compare(function, budget, actual, &expected);
            comparison_report_end(budget);
            return observed_result;
        }
    }
    let mut actual = plan_skeleton_paths(function, budget)?;
    comparison_report_begin(budget, false);
    expected.sort();
    actual.sort();
    if actual != expected {
        if std::env::var_os("SPX_PROBE_SKELETON").is_some() {
            for (index, path) in expected.iter().enumerate() {
                eprintln!("EXP[{index}] {path:?}");
            }
            for (index, path) in actual.iter().enumerate() {
                eprintln!("ACT[{index}] {path:?}");
            }
        }
        comparison_report_end(budget);
        return Err(replay_error(
            function,
            "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
        ));
    }
    comparison_report_end(budget);
    Ok(())
}

fn comparison_report_current_receiver<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    mut continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);
    if function.cleanup_plan.lexical_control.is_none()
        && factored::validate_with_index_storage(
            program,
            function,
            budget,
            helpers,
            continuations.as_deref_mut(),
        )?
    {
        return Ok(());
    }
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::EnumeratedHIR);
    let mut expected =
        hir_skeleton_paths_with_index_storage(program, function, budget, helpers, continuations)?;
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::OtherReplay);
    // Match the existing received reference, with no new payer/tag/flag carrier.
    if let ReplayBudget {
        payer: AllocationPayer::ActiveBuilder,
        ..
    } = budget
    {
        budget.reserve_bytes(
            function,
            ordinary_plan_history::SELECT_CALL_CONTROLS,
            "ordinary plan history selector call receivers",
        )?;
        if ordinary_plan_history::select(function, budget)? {
            #[cfg(test)]
            let _debit_phase = debit_routes::phase(debit_routes::Phase::HistoryPLAN);
            budget.reserve_bytes(
                function,
                ordinary_plan_history::PRODUCE_CALL_CONTROLS,
                "ordinary plan history producer call receivers",
            )?;
            let actual = ordinary_plan_history::produce(function, budget)?;
            comparison_report_begin(budget, true);
            #[cfg(test)]
            let _debit_phase = debit_routes::phase(debit_routes::Phase::PathComparison);
            budget.reserve_bytes(
                function,
                skeleton_work::enumerated_prefix::ENTRY_CALL,
                "enumerated HIR sort entry receivers",
            )?;
            skeleton_work::enumerated_prefix::sort(&mut expected, function, budget)?;
            budget.reserve_bytes(
                function,
                ordinary_plan_history::COMPARE_CALL_CONTROLS,
                "ordinary plan history owned comparison call receivers",
            )?;
            let observed_result =
                ordinary_plan_history::compare(function, budget, actual, &expected);
            comparison_report_end(budget);
            return observed_result;
        }
    }
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::FlatPLAN);
    let mut actual = plan_skeleton_paths(function, budget)?;
    comparison_report_begin(budget, false);
    #[cfg(test)]
    let _debit_phase = debit_routes::phase(debit_routes::Phase::PathComparison);
    if match budget {
        ReplayBudget {
            payer: AllocationPayer::ActiveBuilder,
            admission: AdmissionMode::Ordinary,
            lent_skeleton: false,
            ..
        } => {
            budget.reserve_bytes(
                function,
                2 * skeleton_work::enumerated_prefix::ENTRY_CALL,
                "enumerated HIR sort and equality entry receivers",
            )?;
            skeleton_work::enumerated_prefix::sort(&mut expected, function, budget)?;
            actual.sort();
            !skeleton_work::enumerated_prefix::equal(&expected, &actual, function, budget)?
        }
        _ => {
            expected.sort();
            actual.sort();
            actual != expected
        }
    } {
        if std::env::var_os("SPX_PROBE_SKELETON").is_some() {
            for (index, path) in expected.iter().enumerate() {
                eprintln!("EXP[{index}] {path:?}");
            }
            for (index, path) in actual.iter().enumerate() {
                eprintln!("ACT[{index}] {path:?}");
            }
        }
        comparison_report_end(budget);
        return Err(replay_error(
            function,
            "cleanup CFG decision or ownership-event sequence disagrees with typed HIR",
        ));
    }
    comparison_report_end(budget);
    Ok(())
}

#[test]
fn complete_comparison_boundaries_report_real_old_current_work_paid_and_heap_after_drop() {
    // Same existing three-branch fixture as ordinary_plan_history::tests;
    // independent program storage is built for EVERY measured arm.
    let source = "module history; @id(\"f\") fn f(a:bool,b:bool,c:bool)->i64 {let n=if a {1}else{2};let m=if b {n+1}else{n+2};if c {m+1}else{m+2}} @id(\"main\") fn main()->i64 {f(true,false,true)}";
    let run = |ordinal: usize, old: bool, hooked: bool| {
        let checked = crate::check(source, "history.spx").unwrap();
        let canonical = crate::format::canonical(&checked);
        let checked = crate::check(&canonical, "history.spx").unwrap();
        assert_eq!(crate::format::canonical(&checked), canonical);
        let program = crate::hir::resolve(&checked).unwrap();
        let function = &program.functions[ordinal];
        COMPARISON_BOUNDARY_REPORT.with(|slot| *slot.borrow_mut() = None);
        let ((outcome, heap), refused, paid, receipt) = with_limit_usage_refusal(1 << 26, || {
            measure(|| {
                let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                let result = if old {
                    with_original(|| {
                        if hooked {
                            comparison_report_old_receiver(
                                &program,
                                function,
                                &mut budget,
                                None,
                                None,
                            )
                        } else {
                            before::validate_typed_control_skeleton_with_index(
                                &program,
                                function,
                                &mut budget,
                                None,
                                None,
                            )
                        }
                    })
                } else if hooked {
                    comparison_report_current_receiver(&program, function, &mut budget, None, None)
                } else {
                    validate_typed_control_skeleton_with_index(
                        &program,
                        function,
                        &mut budget,
                        None,
                        None,
                    )
                };
                let remaining = (
                    budget.remaining,
                    budget.skeleton_remaining,
                    budget.skeleton_spent,
                );
                drop(budget);
                let status = result.map_err(DiagnosticFields::capture);
                let after_drop = crate::test_allocation_observer::current_metrics();
                let remaining_physical = crate::bounded_output::active_remaining().unwrap();
                (status, remaining, after_drop, remaining_physical)
            })
        });
        assert!(!refused && receipt.is_none());
        assert!(outcome.0.is_ok());
        assert!(!heap.invalid);
        assert_eq!(heap.live, 0);
        assert_eq!(heap.allocated, heap.freed);
        let report = COMPARISON_BOUNDARY_REPORT.with(|slot| slot.borrow_mut().take());
        (outcome, heap, paid, report)
    };
    let mut observed_routes = [false; 2];
    for ordinal in 0..2 {
        for old in [true, false] {
            let (plain, _, plain_paid, no_report) = run(ordinal, old, false);
            assert!(no_report.is_none());
            let (hooked, outer_heap, hooked_paid, report) = run(ordinal, old, true);
            // Captures add no compiler work, physical debit or status change.
            assert_eq!(
                hooked.0, plain.0,
                "all six fields if a first Diagnostic exists"
            );
            assert_eq!(hooked.1, plain.1);
            assert_eq!(hooked_paid, plain_paid);
            let report = report.expect("fixture must reach a real comparison boundary");
            observed_routes[usize::from(report.immutable_plan)] = true;
            let end = report.after.unwrap();
            let work = report
                .before
                .ordinary_remaining
                .checked_sub(end.ordinary_remaining)
                .unwrap();
            let skeleton = report
                .before
                .skeleton_remaining
                .checked_sub(end.skeleton_remaining)
                .unwrap();
            assert_eq!(
                end.skeleton_spent
                    .checked_sub(report.before.skeleton_spent)
                    .unwrap(),
                skeleton
            );
            let comparison_paid = report
                .before
                .physical_remaining
                .checked_sub(hooked.3)
                .unwrap();
            let allocated = hooked
                .2
                .allocated
                .checked_sub(report.before.heap.allocated)
                .unwrap();
            let freed = hooked
                .2
                .freed
                .checked_sub(report.before.heap.freed)
                .unwrap();
            assert_eq!(
                report.before.heap.live.checked_add(allocated).unwrap(),
                freed + hooked.2.live
            );
            eprintln!("OPT877 comparison-boundary ordinal={ordinal} old={old} immutable_plan={} before={:?} after={end:?} ordinary_work={work} skeleton_work={skeleton} paid_through_drop={comparison_paid} allocated_through_drop={allocated} freed_through_drop={freed} live_after_drop={} outer_heap={outer_heap:?}; boundary includes sort/materialization/comparison and receiver Drop; setup excluded from deltas; outer highwater is not phase peak", report.immutable_plan, report.before, hooked.2.live);
        }
    }
    assert!(
        observed_routes.into_iter().all(|seen| seen),
        "both real flat and immutable PLAN receiver boundaries required"
    );
}
