//! Whole immediate current ordinary composition, including actual HIR caller.
use super::super::super::ordinary_cross_phase_before_receiving::validate_structure_with_budget_storage_before_receiving;
use super::*;
use crate::bounded_output::classifier_string::StorageParent;
use crate::cleanup_plan::replay::expression_facts::arena::retained::Session;
use crate::cleanup_plan::replay::ordinary_receiving;

#[derive(Clone, Copy)]
enum Driver {
    BeforeReceiving,
    ImmediateReceiving,
    TransitionReceiving,
    Current,
}

fn run_driver(
    program: &ResolvedProgram,
    driver: Driver,
    limit: usize,
) -> (
    Result<(), Diagnostic>,
    bool,
    usize,
    crate::cleanup_plan::replay::allocation::OracleReceipt,
) {
    // Reference-only comparison has its own finite account. The actual driver
    // remains ActiveBuilder under the exact ordinary caller limit below.
    // Cumulative reference work funds the complete pair for both functions;
    // the original per-receiver ceilings and all work debits remain intact.
    use crate::cleanup_plan::replay::allocation::{opt_in, OracleScope};
    use crate::cleanup_plan::replay::workspace_oracle::{
        reference_work_bound, ReferenceReceiptOnUnwind,
    };
    let receivers = program
        .functions
        .len()
        .checked_mul(2)
        .expect("two receivers per checked function");
    let scope = OracleScope::open_with_work(1 << 28, reference_work_bound(receivers))
        .expect("explicit independent receiving reference account");
    let trace = ReferenceReceiptOnUnwind::new(&scope, receivers);
    let selected = opt_in(scope.lease()).expect("one receiving reference account");
    let (result, refused, paid) = crate::bounded_output::with_limit_usage(limit, || {
        let mut budget = ReplayBudget::new();
        let helpers = ordinary_renewals::index(program, &mut budget)?;
        reserve_program_skeleton_work(program, program.functions.iter(), &mut budget)?;
        let mut continuations = continuation_storage::Storage::for_program(program, &mut budget)?;
        for function in &program.functions {
            match driver {
                Driver::BeforeReceiving => ordinary_receiving::with_before(|| {
                    validate_structure_with_budget_storage_before_receiving(
                        program, function, &mut budget, Some(&helpers), Some(&mut continuations),
                    )
                })?,
                Driver::ImmediateReceiving =>
                    super::super::super::ordinary_block_before::validate_structure_with_budget_storage(
                        program, function, &mut budget, Some(&helpers), Some(&mut continuations),
                    )?,
                Driver::TransitionReceiving =>
                    super::super::super::before_structure_release::validate_structure_with_budget_storage(
                        program, function, &mut budget, Some(&helpers), Some(&mut continuations),
                    )?,
                Driver::Current => validate_structure_with_budget_storage(
                    program, function, &mut budget, Some(&helpers), Some(&mut continuations),
                )?,
            }
        }
        Ok(())
    });
    drop(selected);
    drop(trace);
    let reference = scope.finish();
    assert!(reference.started <= receivers, "{reference:?}");
    eprintln!("receiving reference max_receivers={receivers} complete_receipt={reference:?}");
    assert_eq!(reference.started, reference.completed);
    assert!(
        reference.first_refusal.is_none()
            && reference.first_work_refusal.is_none()
            && reference.physical_failure.is_none(),
        "{reference:?}"
    );
    (result, refused, paid, reference)
}

fn run(
    program: &ResolvedProgram,
    old: bool,
    limit: usize,
) -> (
    Result<(), Diagnostic>,
    bool,
    usize,
    crate::cleanup_plan::replay::allocation::OracleReceipt,
) {
    run_driver(
        program,
        if old {
            Driver::BeforeReceiving
        } else {
            Driver::Current
        },
        limit,
    )
}

#[test]
fn whole_immediate_ordinary_result_exact_short_zero_and_real_nm_are_observed() {
    complete_receiving_parent_sources();
    let _complete_old_certified_rows =
        super::super::retained_lookup_before_receiving::complete_rows_before_receiving;
    for rows in [0, 1, 32, 64] {
        let program = checked_program(rows);
        let old = run(&program, true, 64 * 1024 * 1024);
        ordinary_receiving::take_census();
        // The complete immediate #892 image remains compiled independently.
        // Later transition retention (#889) and early scratch release (#903)
        // have their own real fees, so cannot enter the #892-only equation.
        let immediate = run_driver(&program, Driver::ImmediateReceiving, 64 * 1024 * 1024);
        let census = ordinary_receiving::take_census();
        // Independently compiled complete transition receiver, before #903's
        // scratch release; this arm actually owns the incoming transition ABI.
        let transition = run_driver(&program, Driver::TransitionReceiving, 64 * 1024 * 1024);
        let transition_census = ordinary_receiving::take_census();
        let current = run(&program, false, 64 * 1024 * 1024);
        ordinary_receiving::take_census();
        for (name, value) in [
            ("before", &old),
            ("immediate", &immediate),
            ("transition", &transition),
            ("current", &current),
        ] {
            assert!(
                value.3.started > 0,
                "{name} must execute its independent reference"
            );
            eprintln!(
                "OPT892_REFERENCE rows={rows} arm={name} reference={:?} ordinary_paid={}",
                value.3, value.2
            );
        }
        assert_eq!(immediate.0, old.0); // complete six-field diagnostics
        assert!(
            immediate.0.is_ok() && !immediate.1,
            "rows={rows} immediate_error={:?} immediate_refused={} immediate_paid={}",
            immediate.0.as_ref().err(),
            immediate.1,
            immediate.2
        );
        assert_eq!(current.0, old.0); // complete Result and all six error fields
        assert!(
            current.0.is_ok() && !old.1 && !current.1,
            "rows={rows} old(result,refused,paid)={old:?} current(result,refused,paid)={current:?} census={census:?}"
        );
        assert!(!census.incomplete);
        assert!(census.reached[0] > 0);
        if census.reached[1] > 0 {
            assert!(census.reached[2] > 0);
        }
        let expected = census.reached[0] as i128
            * (old_setup_invoice() as i128 - skeleton_work::receiving::acquisition_bytes() as i128)
            + census.reached[2] as i128
                * (2 * std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>()
                    as i128
                    - Session::added_status_query_bytes() as i128)
            + census.reached[3] as i128
                * (2 * std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>()
                    as i128
                    - Session::added_edge_query_bytes() as i128)
            - census.reached[1] as i128 * ordinary_receiving::Failure::acquisition_bytes() as i128;
        assert_eq!(
            old.2 as i128 - immediate.2 as i128,
            expected,
            "complete immediate same-source byte equation must include every new owner and reached query"
        );
        assert_eq!(transition.0, old.0); // complete Result and all six error fields
        assert!(
            transition.0.is_ok() && !transition.1,
            "rows={rows} transition_error={:?} transition_refused={} transition_paid={}",
            transition.0.as_ref().err(),
            transition.1,
            transition.2
        );
        assert!(!transition_census.incomplete);
        assert!(transition_census.reached[0] > 0);
        if transition_census.reached[1] > 0 {
            assert!(transition_census.reached[2] > 0);
        }
        // The retained owner now continues into the transition receiver. Count
        // those genuine HIR queries and callback guards independently rather
        // than pricing every edge/transition query as the earlier edge ABI.
        let mut transition_queries = 0usize;
        let mut transition_callbacks = 0usize;
        let mut transition_owners = 0usize;
        for function in &program.functions {
            if function.cleanup_plan.status_sources.len() >= retained_lookup::MIN_ROWS {
                let (queries, callbacks) = receiving_transition_counts(function);
                transition_queries += queries;
                transition_callbacks += callbacks;
                transition_owners += 1;
            }
        }
        assert_eq!(transition_owners, transition_census.reached[1]);
        assert!(transition_queries <= transition_census.reached[3]);
        let initial_edge_queries = transition_census.reached[3] - transition_queries;
        let callback_packet = std::mem::size_of::<usize>()
            + 3 * std::mem::size_of::<&str>()
            + std::mem::size_of::<bool>();
        let transition_expected = transition_census.reached[0] as i128
            * (old_setup_invoice() as i128 - skeleton_work::receiving::acquisition_bytes() as i128)
            + transition_census.reached[2] as i128
                * (2 * std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>()
                    as i128
                    - Session::added_status_query_bytes() as i128)
            + initial_edge_queries as i128
                * (2 * std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>()
                    as i128
                    - Session::added_edge_query_bytes() as i128)
            + transition_queries as i128
                * ((Session::OLD_SCRATCH
                    + 2 * std::mem::size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>())
                    as i128
                    - (Session::into_edge_local_bytes()
                        + Session::into_edge_query_bytes()
                        + std::mem::size_of::<Option<&Option<CallFact<'_>>>>())
                        as i128)
            - transition_callbacks as i128
                * (StorageParent::CURRENT_CONTROLS + callback_packet) as i128
            - transition_owners as i128
                * (StorageParent::ACQUISITION_CONTROLS
                    + StorageParent::TERMINAL_CONTROLS
                    + callback_packet) as i128
            - transition_census.reached[1] as i128
                * ordinary_receiving::Failure::acquisition_bytes() as i128;
        assert_eq!(
            old.2 as i128 - transition.2 as i128,
            transition_expected,
            "complete transition receiver equation must include every new owner and reached query"
        );
        eprintln!(
            "ordinary transition rows={rows} old={} transition={} census={transition_census:?} delta={transition_expected}",
            old.2, transition.2
        );
        assert!(
            run_driver(&program, Driver::TransitionReceiving, transition.2)
                .0
                .is_ok()
        );
        for limit in [0, transition.2 - 1] {
            let short = run_driver(&program, Driver::TransitionReceiving, limit);
            assert!(short.0.is_err() && short.1);
        }
        eprintln!(
            "ordinary receiving rows={rows} old={} immediate={} current={} census={census:?} immediate_delta={expected} composed_delta={}",
            old.2, immediate.2, current.2, old.2 as i128 - current.2 as i128
        );
        // This is an executable adoption gate, not an unrun performance claim.
        // Empty/short fees are recorded independently; no cross-fixture credit.
        if rows == 64 {
            assert!(
                current.2 < old.2,
                "whole current composition must improve after all new owners/ABI costs: {census:?}"
            );
        }
        assert!(
            run_driver(&program, Driver::ImmediateReceiving, immediate.2)
                .0
                .is_ok()
        );
        for limit in [0, immediate.2 - 1] {
            let short = run_driver(&program, Driver::ImmediateReceiving, limit);
            assert!(short.0.is_err() && short.1);
        }
        assert!(run(&program, false, current.2).0.is_ok());
        for limit in [0, current.2 - 1] {
            let short = run(&program, false, limit);
            assert!(short.0.is_err() && short.1);
        }
        for driver in [
            Driver::ImmediateReceiving,
            Driver::TransitionReceiving,
            Driver::Current,
        ] {
            let ((ok, heap), refused, _) =
                crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
                    crate::test_allocation_observer::measure(|| {
                        let result = run_driver(&program, driver, 64 * 1024 * 1024);
                        let ok = result.0.is_ok();
                        drop(result);
                        ok
                    })
                });
            assert!(ok && !refused);
            assert!(!heap.invalid && heap.live == 0);
            assert_eq!(heap.allocated, heap.freed);
        }
    }
}

#[test]
fn earlier_schema_and_hostile_status_keep_complete_old_first_error() {
    for mutation in 0..4 {
        let mut program = checked_program(64);
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.name == "main")
            .unwrap();
        match mutation {
            0 => function.cleanup_plan.schema = "foreign.schema",
            1 => {
                let row = function.cleanup_plan.status_sources[0].clone();
                function.cleanup_plan.status_sources.insert(1, row);
            }
            2 => {
                function.cleanup_plan.status_sources[0].id.expression =
                    ExpressionId::from_owned("foreign\0λ".into())
            }
            _ => function.cleanup_plan.status_sources[0].id.expression = function.body.id.clone(),
        }
        let before = format!("{program:?}");
        let old = run(&program, true, 64 * 1024 * 1024);
        let current = run(&program, false, 64 * 1024 * 1024);
        assert!(old.0.is_err() && current.0.is_err());
        assert_eq!(current.0, old.0);
        for driver in [Driver::ImmediateReceiving, Driver::TransitionReceiving] {
            let counterpart = run_driver(&program, driver, 64 * 1024 * 1024);
            assert!(counterpart.0.is_err());
            assert_eq!(counterpart.0, old.0); // all six fields, same first error
        }
        assert_eq!(format!("{program:?}"), before);
    }
}

#[test]
fn real_foreign_helper_index_guard_still_precedes_any_receiving_owner() {
    let program = checked_program(64);
    let other = checked_program(1);
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    let index = crate::loop_calls::byte_writer::ModuleIndex::new(&other).unwrap();
    let mut old_budget = ReplayBudget::unobserved();
    let old = validate_structure_with_budget_storage_before_receiving(
        &program,
        function,
        &mut old_budget,
        Some(&index),
        None,
    );
    ordinary_receiving::take_census();
    let mut budget = ReplayBudget::unobserved();
    let current =
        validate_structure_with_budget_storage(&program, function, &mut budget, Some(&index), None);
    assert_eq!(current, old);
    assert!(current.is_err());
    let census = ordinary_receiving::take_census();
    assert_eq!(census.reached, [0; 4]);
    assert!(!census.incomplete);
    assert_eq!(budget.remaining, old_budget.remaining);
    for driver in [Driver::ImmediateReceiving, Driver::TransitionReceiving] {
        let mut counterpart_budget = ReplayBudget::unobserved();
        let counterpart = match driver {
            Driver::ImmediateReceiving =>
                super::super::super::ordinary_block_before::validate_structure_with_budget_storage(
                    &program, function, &mut counterpart_budget, Some(&index), None,
                ),
            Driver::TransitionReceiving =>
                super::super::super::before_structure_release::validate_structure_with_budget_storage(
                    &program, function, &mut counterpart_budget, Some(&index), None,
                ),
            _ => unreachable!(),
        };
        assert_eq!(counterpart, old); // complete first diagnostic
        assert_eq!(counterpart_budget.remaining, old_budget.remaining);
        let census = ordinary_receiving::take_census();
        assert_eq!(census.reached, [0; 4]);
        assert!(!census.incomplete);
    }
}

// Complete pre-financing c907 helper, compiled with its original body.
// Its original ordinary ACTIVE invoice includes all three comparison receivers.
mod original_unopted_scalar {
    use super::*;
    #[rustfmt::skip]
fn ordinary(
    program: &ResolvedProgram,
    old: bool,
    bytes: usize,
) -> (Result<(), Diagnostic>, bool, usize) {
    let function = program.functions.iter().find(|f| f.name == "main").unwrap();
    crate::bounded_output::with_limit_usage(bytes, || {
        let mut budget = ReplayBudget::new();
        // Production reserves the complete skeleton allowance before calling this inner receiver.
        // Keep that real preflight inside the same measured payer for both independent arms.
        reserve_program_skeleton_work(program, std::iter::once(function), &mut budget)?;
        if old {
            lookup_caller_before::validate_structure_with_budget_storage(
                program,
                function,
                &mut budget,
                None,
                None,
            )
        } else {
            validate_structure_with_budget_storage(program, function, &mut budget, None, None)
        }
    })
}

    pub(super) fn run(
        program: &ResolvedProgram,
        old: bool,
        bytes: usize,
    ) -> (Result<(), Diagnostic>, bool, usize) {
        assert!(crate::cleanup_plan::replay::allocation::opted()
            .unwrap()
            .is_none());
        ordinary(program, old, bytes)
    }
}

fn old_setup_invoice() -> usize {
    use crate::hir::funded::FundedError;
    2 * std::mem::size_of::<
        Result<crate::cleanup_plan::renewal_profile::RenewalBindings<'_>, FundedError<Diagnostic>>,
    >() + 4 * std::mem::size_of::<Result<BTreeSet<ExpressionId>, FundedError<Diagnostic>>>()
        + 2 * std::mem::size_of::<Result<bool, FundedError<Diagnostic>>>()
        + 2 * std::mem::size_of::<
            Result<
                Result<crate::hir::FunctionExecutionId, crate::hir::lexical_exit::OwnerVerdict>,
                FundedError<Diagnostic>,
            >,
        >()
        + 2 * std::mem::size_of::<(
            &mut ReplayBudget,
            &ResolvedFunction,
            &crate::bounded_output::classifier_string::ClassifierLedger,
        )>()
        + std::mem::size_of::<crate::bounded_output::classifier_string::ClassifierLedger>()
        + 10 * std::mem::size_of::<&mut dyn FnMut(usize) -> Result<(), Diagnostic>>()
}

#[test]
fn exact_original_scalar_fixture_current_invoice_work_cuts_and_earlier_error_are_independent() {
    // Exact preexisting #885 scalar program AND actual no-index ordinary helper.
    let scalar = checked_program(0);
    let ordinary = original_unopted_scalar::run;
    let old = ordinary_receiving::with_before(|| ordinary(&scalar, false, 64 * 1024 * 1024));
    ordinary_receiving::take_census();
    let current = ordinary(&scalar, false, 64 * 1024 * 1024);
    let census = ordinary_receiving::take_census();
    assert_eq!(current.0, old.0);
    assert!(current.0.is_ok() && !old.1 && !current.1);
    // The ordinary receiver constructs once. Its unchanged test-only
    // cross_check independently constructs the enumerating and factored HIR
    // paths too; all three use the same active payer and remain in this invoice.
    const COMPLETE_SETUP_CALLS: usize = 3;
    assert_eq!(census.reached, [COMPLETE_SETUP_CALLS, 0, 0, 0]);
    assert!(!census.incomplete);
    assert_eq!(
        old.2 as i128 - current.2 as i128,
        COMPLETE_SETUP_CALLS as i128
            * (old_setup_invoice() as i128 - skeleton_work::receiving::acquisition_bytes() as i128)
    );
    eprintln!(
        "exact old scalar invoice old={} current={} delta={}",
        old.2,
        current.2,
        old.2 as i128 - current.2 as i128
    );
    for bytes in [0, current.2 - 1, current.2] {
        let result = ordinary(&scalar, false, bytes);
        assert_eq!(result.0.is_ok(), bytes == current.2);
        assert_eq!(result.1, bytes != current.2);
    }
    // The explicitly financed generation keeps its own independent equation:
    // only the ordinary driver is ACTIVE; the other two receivers are Oracle.
    let (funded_old, old_reference) = ordinary_receiving::with_before(|| {
        ordinary_driver_reference(&scalar, false, false, 64 * 1024 * 1024)
    });
    ordinary_receiving::take_census();
    let (funded_current, current_reference) =
        ordinary_driver_reference(&scalar, false, false, 64 * 1024 * 1024);
    let funded_census = ordinary_receiving::take_census();
    assert_eq!(funded_census.reached, [1, 0, 0, 0]);
    assert!(!funded_census.incomplete);
    assert_eq!(funded_current.0, funded_old.0);
    assert!(funded_current.0.is_ok() && !funded_current.1 && !funded_old.1);
    for receipt in [old_reference, current_reference] {
        assert_eq!((receipt.started, receipt.completed), (2, 2));
        assert!(receipt.charged > 0 && receipt.work > 0);
        assert!(receipt.charged <= receipt.allowance && receipt.work <= receipt.work_allowance);
        assert!(
            receipt.first_refusal.is_none()
                && receipt.first_work_refusal.is_none()
                && receipt.physical_failure.is_none()
        );
    }
    assert_eq!(
        funded_old.2 as i128 - funded_current.2 as i128,
        old_setup_invoice() as i128 - skeleton_work::receiving::acquisition_bytes() as i128
    );
    eprintln!("funded scalar active_count=1 old={funded_old:?} current={funded_current:?} old_reference={old_reference:?} current_reference={current_reference:?}");
    for bytes in [0, funded_current.2 - 1, funded_current.2] {
        let (result, reference) = ordinary_driver_reference(&scalar, false, false, bytes);
        assert_eq!(result.0.is_ok(), bytes == funded_current.2);
        assert_eq!(result.1, bytes != funded_current.2);
        eprintln!(
            "funded scalar exact/short bytes={bytes} actual={result:?} reference={reference:?}"
        );
    }
    let function = scalar.functions.iter().find(|f| f.name == "main").unwrap();
    let work_cut = |old: bool, units| {
        crate::bounded_output::with_limit_usage(64 * 1024 * 1024, || {
            let mut budget = ReplayBudget::new();
            reserve_program_skeleton_work(&scalar, std::iter::once(function), &mut budget).unwrap();
            budget.skeleton_remaining = units;
            let result = if old {
                ordinary_receiving::with_before(|| {
                    validate_structure_with_budget_storage_before_receiving(
                        &scalar,
                        function,
                        &mut budget,
                        None,
                        None,
                    )
                })
            } else {
                validate_structure_with_budget_storage(&scalar, function, &mut budget, None, None)
            };
            (
                result,
                budget.remaining,
                budget.skeleton_remaining,
                budget.skeleton_spent,
            )
        })
        .0
    };
    let full = work_cut(true, MAX_REPLAY_WORK_UNITS);
    assert!(full.0.is_ok());
    for units in 0..=full.3 + 1 {
        assert_eq!(
            work_cut(false, units),
            work_cut(true, units),
            "all fields/work at {units}"
        );
    }
    let mut bad = checked_program(0);
    bad.functions
        .iter_mut()
        .find(|f| f.name == "main")
        .unwrap()
        .cleanup_plan
        .schema = "foreign.schema";
    let old = ordinary_receiving::with_before(|| ordinary(&bad, false, 64 * 1024 * 1024));
    ordinary_receiving::take_census();
    let current = ordinary(&bad, false, 64 * 1024 * 1024);
    assert!(current.0.is_err());
    assert_eq!(current, old); // exact six fields, first-error precedence AND debit
    let census = ordinary_receiving::take_census();
    assert_eq!(census.reached, [0; 4]);
    assert!(!census.incomplete);
}

// Same structural census as the independent transition ABI regression.
fn receiving_transition_counts(function: &ResolvedFunction) -> (usize, usize) {
    let mut lookups = 0;
    let mut callbacks = 0;
    for block in &function.cleanup_plan.blocks {
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Initialize { .. }
                | CleanupTransition::Transfer { .. }
                | CleanupTransition::TransferVariant { .. }
                | CleanupTransition::InitializeVariant { .. } => lookups += 1,
                CleanupTransition::ReserveRenewal { .. }
                | CleanupTransition::Renew { .. }
                | CleanupTransition::AuthenticateVariantCase { .. }
                | CleanupTransition::CallCommit { .. } => {
                    lookups += 1;
                    callbacks += 1;
                }
                CleanupTransition::StageCopyResult {
                    source:
                        StagedCopyResultSource::TryResidual { .. }
                        | StagedCopyResultSource::TryOptionNone { .. },
                    ..
                } => lookups += 2,
                _ => {}
            }
        }
    }
    (lookups, callbacks)
}

// Complete literal source custody for both independently preserved merge parents.
fn complete_receiving_parent_sources() {
    use sha2::{Digest, Sha256};
    let hash = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs";
    let raw = std::fs::read(root.join(path)).unwrap();
    assert_eq!(raw.as_slice(), include_bytes!("receiving.rs").as_slice());
    let encoded = include_bytes!("receiving_predecessors.json");
    assert_eq!(std::fs::read(root.join("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving_predecessors.json")).unwrap().as_slice(), encoded.as_slice());
    let packet: serde_json::Value = serde_json::from_slice(encoded).unwrap();
    assert_eq!(
        packet["current_length"].as_u64().unwrap() as usize,
        raw.len()
    );
    assert_eq!(packet["current_sha256"].as_str().unwrap(), hash(&raw));
    // Preserve the complete prior packet and its generation-specific inverse
    // rows before acquiring this test-only financing/census source phase.
    let prior_raw = packet["full_prior_packet"].as_str().unwrap().as_bytes();
    assert_eq!(prior_raw.len(), 60223);
    assert_eq!(
        hash(prior_raw),
        "cf0855a5bb5ee4ab772464c5e36a0619446025363ca89ef3f3d39b98789601eb"
    );
    let prior: serde_json::Value = serde_json::from_slice(prior_raw).unwrap();
    let rows = packet["parents"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    for (row, old) in rows.iter().zip(prior["parents"].as_array().unwrap()) {
        assert_eq!(row["exact_prior_parent_record"], *old);
        let edits = row["edits"].as_array().unwrap();
        assert_eq!(&edits[..edits.len() - 2], old["edits"].as_array().unwrap());
        let acquisition = edits.last().unwrap();
        assert_eq!(acquisition["start"], 0);
        assert_eq!(acquisition["end"].as_u64().unwrap() as usize, raw.len());
        assert_eq!(acquisition["current"].as_str().unwrap().as_bytes(), raw);
        let immediate = acquisition["before"].as_str().unwrap().as_bytes();
        assert_eq!(immediate.len(), 24273);
        assert_eq!(
            hash(immediate),
            "fc7a3fe35db9670affa2d479fea6e0a9eadf50afed410e729c7d956124d1c7f5"
        );
        let stale_front = &edits[edits.len() - 2];
        assert_eq!(
            stale_front["current"].as_str().unwrap().as_bytes(),
            immediate
        );
        let prior_current = stale_front["before"].as_str().unwrap().as_bytes();
        assert_eq!(
            prior_current.len(),
            prior["current_length"].as_u64().unwrap() as usize
        );
        assert_eq!(
            hash(prior_current),
            prior["current_sha256"].as_str().unwrap()
        );
    }
    let unopted = &packet["original_unopted_c907"];
    assert_eq!(unopted["head"], "c90764eebdf0d1db8a35742c39b8831530de1966");
    let file = unopted["full_source_file"].as_str().unwrap();
    assert_eq!(file.len(), 15446);
    assert_eq!(
        hash(file.as_bytes()),
        "2bd12be9b146263f0c7c66f5c0cf07c3a91715a8dabb808162dca62ef5237bf2"
    );
    let helper = unopted["full_function"].as_str().unwrap();
    assert_eq!(helper.len(), 947);
    assert_eq!(
        hash(helper.as_bytes()),
        "57682d2c842c20358780ad6837343b8f2534ca7b5d15063984772e66fde412a1"
    );
    assert_eq!(file.matches(helper).count(), 1);
    assert_eq!(
        std::str::from_utf8(&raw).unwrap().matches(helper).count(),
        1
    );
    let fixed = [
        (
            "8caff6ada4d0b65bb0875ab49a874797e9f653d4",
            12916usize,
            "7fd5bf0961d3cede1a747ea6f4dc80f7598d547c012c3394a5539d9690794515",
        ),
        (
            "c90764eebdf0d1db8a35742c39b8831530de1966",
            14483usize,
            "fcd3a360eaf7681316cc2432e0165729766a29ea18ce80424cef798cfc421e38",
        ),
    ];
    for (row, (head, length, digest)) in rows.iter().zip(fixed) {
        assert_eq!(row["head"].as_str().unwrap(), head);
        assert_eq!(row["before_length"].as_u64().unwrap() as usize, length);
        assert_eq!(row["before_sha256"].as_str().unwrap(), digest);
        let mut restored = raw.clone();
        for change in row["edits"].as_array().unwrap().iter().rev() {
            let start = change["start"].as_u64().unwrap() as usize;
            let end = change["end"].as_u64().unwrap() as usize;
            assert_eq!(
                &restored[start..end],
                change["current"].as_str().unwrap().as_bytes()
            );
            drop(restored.splice(start..end, change["before"].as_str().unwrap().bytes()));
        }
        let expected = row["full_before"].as_str().unwrap().as_bytes();
        assert_eq!(restored.as_slice(), expected);
        assert_eq!(
            row["before_length"].as_u64().unwrap() as usize,
            restored.len()
        );
        assert_eq!(row["before_sha256"].as_str().unwrap(), hash(&restored));
    }
}
