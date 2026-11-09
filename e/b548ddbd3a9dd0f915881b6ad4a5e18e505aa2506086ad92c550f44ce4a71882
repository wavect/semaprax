//! Source-authored controls; the parent consolidated build must execute these.
use super::*;
use crate::bounded_output::cfg_reservation::{CfgReservation, Refused};
use crate::bounded_output::{active_remaining, set_active_floor, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

thread_local! {
    static IMMEDIATE_OLD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(super) fn use_immediate_old() -> bool {
    IMMEDIATE_OLD.with(std::cell::Cell::get)
}
struct Mode(bool);
impl Mode {
    fn select(old: bool) -> Self {
        Self(IMMEDIATE_OLD.with(|slot| slot.replace(old)))
    }
}
impl Drop for Mode {
    fn drop(&mut self) {
        IMMEDIATE_OLD.with(|slot| slot.set(self.0));
    }
}

type Inputs<'a> = (
    Cfg<'a>,
    Vec<CfgDraft<()>>,
    Vec<Option<LoanId>>,
    Vec<Vec<LoanId>>,
    Vec<Vec<u16>>,
    usize,
);

// Fixture input constructors have their OWN existing captured-owner payments.
// They are included in observed allocation and Drop, not subtracted afterward.
fn inputs<'a>(
    function: &'a ResolvedFunction,
    plan: &LoanPlan,
    owner: &CfgReservation,
) -> Result<Inputs<'a>, Diagnostic> {
    let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
    work.allocation = Some(owner);
    let cfg = build_cfg(function, &mut work)?;
    let mut drafts = cfg_directory::vector(plan.loans.len(), owner)?;
    let mut parents = cfg_directory::vector(plan.loans.len(), owner)?;
    let mut ends = cfg_directory::vector(plan.loans.len(), owner)?;
    for loan in &plan.loans {
        drafts.push(CfgDraft {
            site: loan.site.clone(),
            origin: storage::clone_place(&loan.origin, Some(owner))?,
            origin_span: function.body.span,
            parent_root: None,
            binding: None,
            start: *cfg.nodes.get(&loan.start).expect("fixture start retained"),
            seeds: (),
            cause: loan.cause.clone(),
        });
        parents.push(loan.parent);
        ends.push(storage::clone_vec(&loan.end_edges, Some(owner))?);
    }
    let mut live = cfg_directory::vector(cfg.edges.len(), owner)?;
    if plan.edges.is_empty() {
        assert!(plan.loans.is_empty());
        for _ in &cfg.edges {
            live.push(cfg_directory::vector(0, owner)?);
        }
    } else {
        assert_eq!(cfg.edges.len(), plan.edges.len());
        for (edge, pair) in plan.edges.iter().zip(&cfg.edges) {
            assert_eq!((edge.from, edge.to), *pair);
            live.push(storage::clone_vec(&edge.live, Some(owner))?);
        }
    }
    Ok((cfg, drafts, parents, live, ends, work.used))
}

fn invoke(input: Inputs<'_>, owner: &CfgReservation, old: bool) -> Result<LoanPlan, Diagnostic> {
    let (cfg, drafts, parents, live, ends, _) = input;
    if old {
        immediate_before::materialize(cfg, drafts, parents, live, ends, owner)
    } else {
        materialize(cfg, drafts, parents, live, ends, owner)
    }
}

fn physical(
    function: &ResolvedFunction,
    plan: &LoanPlan,
    old: bool,
    limit: usize,
) -> (bool, usize, Metrics) {
    let ((success, metrics), refused, debit, first) = with_limit_usage_refusal(limit, || {
        measure(|| {
            let owner = match CfgReservation::acquire() {
                Ok(owner) => owner,
                Err(_) => return false,
            };
            let result =
                inputs(function, plan, &owner).and_then(|input| invoke(input, &owner, old));
            let success = result.is_ok();
            if let Ok(actual) = &result {
                // Empty public plans early-return before creating their CFG;
                // all nonempty plans retain the full independently derived value.
                if !plan.loans.is_empty() {
                    assert_eq!(actual, plan);
                }
                for endpoint in &actual.endpoints {
                    assert!(endpoint.starts.windows(2).all(|pair| pair[0] <= pair[1]));
                    assert!(endpoint.kills.windows(2).all(|pair| pair[0] <= pair[1]));
                }
            } else {
                let reason = owner
                    .first()
                    .expect("only physical failures in this fixture");
                let before = current_metrics();
                assert_eq!(owner.reserve(0), Err(reason));
                assert_eq!(owner.reserve(1), Err(reason));
                assert_eq!(current_metrics(), before, "sticky retries allocate nothing");
            }
            drop(result);
            drop(owner);
            success
        })
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(
        metrics.allocated <= debit,
        "old={old} metrics={metrics:?} debit={debit}"
    );
    assert_eq!(success, !refused);
    assert_eq!(success, first.is_none());
    (success, debit, metrics)
}

#[test]
fn owned_rows_match_complete_immediate_plan_work_and_real_allocation_through_drop() {
    for count in [0, 1, 3, 17] {
        let (program, index) = boundary_tests::fixture_with_uses(count, true);
        let function = &program.functions[index];
        let plan = saved_planner::build_plan(&program, function).unwrap();
        let old = physical(function, &plan, true, usize::MAX);
        let current = physical(function, &plan, false, usize::MAX);
        assert!(old.0 && current.0);
        // Report whole actual fee regressions honestly. New transport funding
        // can outweigh a small clone tariff; no unconditional net-fee assertion.
        eprintln!("owned loan rows count={count}: old={old:?} current={current:?}");
        if count != 0 {
            assert!(
                current.2.allocated < old.2.allocated,
                "physical copied rows must disappear"
            );
            assert!(current.2.allocations < old.2.allocations);
        }
        with_limit_usage_refusal(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let a = inputs(function, &plan, &owner).unwrap();
            let b = inputs(function, &plan, &owner).unwrap();
            assert_eq!(a.5, b.5, "same complete CFG work; materializer grants none");
            let a = invoke(a, &owner, true).unwrap();
            let b = invoke(b, &owner, false).unwrap();
            assert_eq!(a, b);
            assert_eq!(
                crate::cache_codec::encode(&a).unwrap(),
                crate::cache_codec::encode(&b).unwrap()
            );
            assert_eq!(
                crate::graph_loan::loan_plan_json(&a),
                crate::graph_loan::loan_plan_json(&b)
            );
        });
    }
}

#[test]
fn owned_origin_and_termination_backing_move_with_original_order_dedup_and_spare_capacity() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let mut plan = saved_planner::build_plan(&program, function).unwrap();
    // An internal receiver does not reinterpret caller places. Exercise actual
    // projection backing, including shared Unicode/NUL identities, independently.
    plan.loans[0]
        .origin
        .projections
        .push(PlaceProjection::Field(crate::hir::DeclarationId::new(
            "field.é\0identity",
        )));
    with_limit_usage_refusal(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let input = inputs(function, &plan, &owner).unwrap();
        let projection_pointer = input.1[0].origin.projections.as_ptr();
        let termination_pointer = input.4[0].as_ptr();
        assert_eq!(input.4[0].len(), input.4[0].capacity());
        let current = invoke(input, &owner, false).unwrap();
        let old = invoke(inputs(function, &plan, &owner).unwrap(), &owner, true).unwrap();
        assert_eq!(current, old);
        assert_eq!(
            current.loans[0].origin.projections.as_ptr(),
            projection_pointer
        );
        assert_eq!(current.loans[0].end_edges.as_ptr(), termination_pointer);
        assert_eq!(
            owned_capacity::clone_capacity_bytes(&current),
            owned_capacity::clone_capacity_bytes(&old)
        );
        // The REAL producer's singleton rows have geometric capacity four.
        // Reproduce spare backing, then require old exact published capacity.
        let mut input = inputs(function, &plan, &owner).unwrap();
        let first_edge = input.4[0][0];
        storage::push(&mut input.4[0], first_edge, Some(&owner)).unwrap();
        assert!(input.4[0].capacity() > input.4[0].len());
        let current = invoke(input, &owner, false).unwrap();
        let mut input = inputs(function, &plan, &owner).unwrap();
        storage::push(&mut input.4[0], first_edge, Some(&owner)).unwrap();
        let old = invoke(input, &owner, true).unwrap();
        assert_eq!(current, old);
        for (a, b) in current.loans.iter().zip(&old.loans) {
            assert_eq!(a.end_edges.capacity(), a.end_edges.len());
            assert_eq!(a.end_edges.capacity(), b.end_edges.capacity());
        }
        assert_eq!(current.loans[0].end_edges.last(), Some(&first_edge));
        assert!(!current.loans[0]
            .ends
            .windows(2)
            .any(|pair| pair[0] == pair[1]));
        assert_eq!(
            owned_capacity::clone_capacity_bytes(&current),
            owned_capacity::clone_capacity_bytes(&old)
        );
        assert_eq!(owner.first(), None);
    });
}

#[test]
fn owned_rows_exact_short_zero_sticky_and_floor_keep_complete_drop() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let plan = saved_planner::build_plan(&program, function).unwrap();
    for old in [true, false] {
        let ample = physical(function, &plan, old, usize::MAX);
        for limit in [0, ample.1 - 1, ample.1, ample.1 + 1] {
            assert_eq!(physical(function, &plan, old, limit).0, limit >= ample.1);
        }
    }
    let ((), refused, _, _) = with_limit_usage_refusal(1 << 24, || {
        let owner = CfgReservation::acquire().unwrap();
        let input = inputs(function, &plan, &owner).unwrap();
        let remaining = active_remaining().unwrap();
        assert!(remaining > 0 && set_active_floor(remaining));
        let result = invoke(input, &owner, false);
        assert!(result.is_err());
        let selected = owner.first().unwrap();
        assert_eq!(owner.reserve(0), Err(selected));
        assert_eq!(active_remaining(), Some(remaining));
        drop(result);
        drop(owner);
    });
    assert!(refused);
}

#[test]
fn owned_rows_preserve_missing_end_before_malformed_liveness_and_index_panics() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let plan = saved_planner::build_plan(&program, function).unwrap();
    for old in [true, false] {
        with_limit_usage_refusal(usize::MAX, || {
            let owner = CfgReservation::acquire().unwrap();
            let mut input = inputs(function, &plan, &owner).unwrap();
            input.4[0].clear();
            input.3.clear();
            assert_eq!(
                invoke(input, &owner, old).unwrap_err(),
                error("loan has no bounded CFG termination edge")
            );
            assert_eq!(owner.first(), None);
            let mut input = inputs(function, &plan, &owner).unwrap();
            input.4.clear();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                invoke(input, &owner, old)
            }))
            .unwrap_err();
            let text = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied())
                .unwrap();
            assert!(
                text.contains("index out of bounds"),
                "owned zip must not truncate malformed rows"
            );
            drop(panic);
        });
    }
}

#[test]
fn owned_rows_reject_foreign_owner_before_new_transport_without_foreign_debit() {
    let (program, index) = boundary_tests::fixture_with_uses(1, true);
    let function = &program.functions[index];
    let plan = saved_planner::build_plan(&program, function).unwrap();
    with_limit_usage_refusal(usize::MAX, || {
        let owner = CfgReservation::acquire().unwrap();
        let input = inputs(function, &plan, &owner).unwrap();
        let ((result, metrics), refused, used, _) =
            with_limit_usage_refusal(1 << 20, || measure(|| invoke(input, &owner, false)));
        assert!(result.is_err());
        assert_eq!(owner.first(), Some(Refused::Detached));
        assert_eq!(used, 0);
        assert!(!refused, "foreign ledger is not poisoned");
        drop(result);
        assert!(!metrics.invalid);
        // Input allocations predate this observer. No new backing is created
        // before rejecting the foreign ledger, including no replacement row.
        assert_eq!(metrics.allocated, 0);
    });
}

#[test]
fn owned_rows_keep_entire_immediate_source_abi_helpers_and_typed_transport_map() {
    let original = include_str!("immediate_before.rs.txt");
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))),
        "97b1eafdd7198997927eb01d6196ef16f094e90e621ebd7e6136befe9e491844"
    );
    let compiled = include_str!("immediate_before.rs");
    let footer = "\n#[cfg(test)]\nmod before;\nmod ordered_union;\n#[cfg(test)]\nmod tests;\n";
    assert_eq!(original.strip_suffix(footer).unwrap(), compiled);
    assert!(original.contains("origin: storage::clone_place(&draft.origin, Some(payer))?"));
    assert!(original.contains("storage::clone_vec(&starts[node], Some(payer))?"));
    assert!(original.contains("storage::clone_vec(&kills[node], Some(payer))?"));
    assert!(std::mem::size_of::<DraftCold<()>>() >= std::mem::size_of::<DraftIterator<()>>());
    assert!(
        std::mem::size_of::<DraftTurn<'_, ()>>()
            >= std::mem::size_of::<Option<(usize, CfgDraft<()>)>>()
    );
    assert!(std::mem::size_of::<TakeRow<'_, u16>>() >= 3 * std::mem::size_of::<Vec<u16>>());
    assert!(
        std::mem::size_of::<EndpointTurn<'_>>() >= 2 * std::mem::size_of::<Option<Vec<LoanId>>>()
    );
    // This is the exact existing compact conversion, including its real
    // over-capacity replacement tariff; no shared-handle clone fee was removed.
    let compact = include_str!("../compact_ids.rs");
    assert!(compact.contains("ids.capacity() != ids.len()"));
    assert!(compact.contains(".array::<LoanId>(ids.len())"));
}

#[test]
fn owned_rows_malformed_unwind_releases_complete_inputs_and_partly_built_result() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    let function = &program.functions[index];
    let plan = saved_planner::build_plan(&program, function).unwrap();
    for old in [true, false] {
        with_limit_usage_refusal(usize::MAX, || {
            let (_, metrics) = measure(|| {
                let owner = CfgReservation::acquire().unwrap();
                let mut input = inputs(function, &plan, &owner).unwrap();
                // First loan is completed before the second missing row panics.
                // This retains the old natural malformed-input receiver path.
                assert!(input.4.len() > 1);
                input.4.truncate(1);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    invoke(input, &owner, old)
                }));
                assert!(result.is_err());
                drop(result);
                drop(owner);
            });
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            // Panic-runtime control allocations are not a compiler tariff claim.
        });
    }
}

#[test]
fn owned_rows_complete_funded_caller_keeps_retained_capacity_and_allocation_drop() {
    let (program, index) = boundary_tests::fixture_with_uses(3, true);
    // Reuse the admitted projected-field caller shape from the existing
    // canonical-loan storage owner, not an invented HIR-only sizing fixture.
    let ast = crate::check(
        r#"module loan.owned_rows;
@id("packet") record Packet { @id("payload") payload:Bytes, @id("sibling") sibling:Bytes, }
@id("first") fn first(packet:own Packet)->usize {
 let view=bytes_as_slice(packet.payload);
 byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)+byte_len(view)
}
@id("main") fn main()->i64 {0}
"#,
        "owned-loan-materializer.spx",
    )
    .unwrap();
    let projected = crate::hir::resolve(&ast).unwrap();
    let projected_index = projected
        .functions
        .iter()
        .position(|f| f.id.as_str() == "first")
        .unwrap();
    for (program, index) in [(program, index), (projected, projected_index)] {
        let function = &program.functions[index];
        let expected = saved_planner::build_plan(&program, function).unwrap();
        let run = |old, limit| {
            let _mode = Mode::select(old); // test-only director outside observation
            let ((retained, metrics), refused, used, first) =
                with_limit_usage_refusal(limit, || {
                    measure(|| {
                        let owner = match CfgReservation::acquire() {
                            Ok(owner) => owner,
                            Err(_) => return None,
                        };
                        let result = build_plan_with_reservation(&program, function, &owner);
                        let retained = result.as_ref().ok().map(|plan| {
                            assert_eq!(plan, &expected);
                            for loan in &plan.loans {
                                assert_eq!(loan.end_edges.capacity(), loan.end_edges.len());
                                assert_eq!(
                                    loan.origin.projections.capacity(),
                                    loan.origin.projections.len()
                                );
                            }
                            (
                                owned_capacity::clone_capacity_bytes(plan).unwrap(),
                                owned_capacity::owned_capacity_bytes(plan).unwrap(),
                            )
                        });
                        drop(result);
                        drop(owner);
                        retained
                    })
                });
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            assert!(
                metrics.allocated <= used,
                "old={old} metrics={metrics:?} paid={used}"
            );
            assert_eq!(retained.is_some(), !refused);
            assert_eq!(retained.is_none(), first.is_some());
            (retained, used, metrics)
        };
        let old = run(true, usize::MAX);
        let current = run(false, usize::MAX);
        assert_eq!(
            current.0, old.0,
            "entire retained mutable capacity equals immediate caller"
        );
        assert!(
            current.2.allocated <= old.2.allocated,
            "complete caller creates no extra backing"
        );
        if expected
            .loans
            .iter()
            .any(|loan| !loan.origin.projections.is_empty())
        {
            assert!(
                current.2.allocated < old.2.allocated,
                "actual admitted projected caller removes origin backing clone"
            );
        }
        eprintln!("whole funded loan caller old={old:?} current={current:?}");
        for old in [true, false] {
            let exact = run(old, usize::MAX).1;
            for limit in [0, exact - 1, exact] {
                assert_eq!(run(old, limit).0.is_some(), limit >= exact);
            }
        }
    }
}

// Expose only the two already executable same-ABI test stages to the complete
// canonical caller. This never enters production or creates an allowance.
pub(super) fn with_materializer_stage<T>(before_8086: bool, operation: impl FnOnce() -> T) -> T {
    let _mode = Mode::select(before_8086);
    operation()
}
