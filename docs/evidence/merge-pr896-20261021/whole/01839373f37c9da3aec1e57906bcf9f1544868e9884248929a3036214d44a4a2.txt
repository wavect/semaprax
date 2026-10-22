use super::*;
use crate::bounded_output::classifier_string::StorageParent;
use crate::hir::{
    DeclarationId, DeclarationIndex, FunctionInstanceId, ResolvedFunctionInstance,
    ResolvedFunctionTemplate, ResolvedInterface, ResolvedTypeDeclaration,
};
use crate::test_allocation_observer::measure;
use sha2::Digest;

const CAP: usize = 64 * 1024 * 1024;
fn fixture(rows: usize, edges: bool) -> ResolvedProgram {
    let mut source = String::from("module cross.phase; @id(\"cross.add\") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {x+1} @id(\"cross.main\") fn main()->i64 {");
    for n in 0..rows {
        source.push_str(&format!("let v{n}=add(0)+3-2*4/2%3;"));
        if edges {
            source.push_str(&format!("let b{n}=if v{n}>0 {{1}} else {{2}};"));
        }
    }
    source.push_str("-1}");
    crate::hir::resolve(&crate::check(&source, "cross-phase.spx").unwrap()).unwrap()
}
fn function(program: &ResolvedProgram) -> &ResolvedFunction {
    program.functions.iter().find(|f| f.name == "main").unwrap()
}
fn queries(function: &ResolvedFunction) -> usize {
    function
        .cleanup_plan
        .edges
        .iter()
        .filter(|e| {
            matches!(
                &e.condition,
                EdgeCondition::BooleanResult(..)
                    | EdgeCondition::VariantCase { .. }
                    | EdgeCondition::ArmSelected { .. }
            )
        })
        .count()
}
fn run(
    program: &ResolvedProgram,
    old: bool,
    cap: usize,
    work: usize,
) -> (Result<(), Diagnostic>, bool, usize, usize) {
    let ((result, work), refused, paid) = crate::bounded_output::with_limit_usage(cap, || {
        let mut budget = ReplayBudget::new();
        budget.remaining = work;
        let result = if old {
            before::validate_structure_with_budget_storage(
                program,
                function(program),
                &mut budget,
                None,
                None,
            )
        } else {
            // Original C5 equations continue to exercise the independently
            // compiled complete immediate implementation.
            super::super::ordinary_block_before::validate_structure_with_budget_storage(
                program,
                function(program),
                &mut budget,
                None,
                None,
            )
        };
        (result, work - budget.remaining)
    });
    (result, refused, paid, work)
}

#[test]
fn actual_status_to_initial_edge_owner_has_full_typed_net_and_unchanged_work() {
    let program = fixture(64, true);
    let function = function(&program);
    assert!(!lexical_cert::requires_lexical(function));
    assert!(function.cleanup_plan.status_sources.len() >= retained_lookup::MIN_ROWS);
    let q = queries(function);
    assert!(
        q > 0,
        "genuine typed expression-bearing edge witness required"
    );
    let old = run(&program, true, CAP, MAX_REPLAY_WORK_UNITS);
    let current = run(&program, false, CAP, MAX_REPLAY_WORK_UNITS);
    assert_eq!(current.0, old.0);
    assert!(current.0.is_ok() && !current.1 && !old.1);
    let s = Session::OLD_SCRATCH;
    let r = Session::EDGE_QUERY_BYTES;
    assert!(s > r);
    assert_eq!(current.3, old.3, "all original work cuts remain");
    // Existing status acquisition remains fully paid; no second owner or
    // transported continuation is formed. Both old compound Results remain.
    assert_eq!(old.2.checked_sub(current.2), Some(q * (s - r)));
    eprintln!("cross-phase actual status={} edge_queries={q} old={old:?} current={current:?} S={s} R={r} M_new=0 A_new=0", function.cleanup_plan.status_sources.len());
    let exact = run(&program, false, current.2, MAX_REPLAY_WORK_UNITS);
    assert!(exact.0.is_ok() && !exact.1);
    for cap in [0, current.2 - 1] {
        let short = run(&program, false, cap, MAX_REPLAY_WORK_UNITS);
        assert!(short.0.is_err() && short.1);
    }
}

#[test]
fn dense_status_with_zero_expression_edges_keeps_the_whole_immediate_bill() {
    let program = fixture(64, false);
    assert_eq!(queries(function(&program)), 0);
    assert!(function(&program).cleanup_plan.status_sources.len() >= retained_lookup::MIN_ROWS);
    assert_eq!(
        run(&program, false, CAP, MAX_REPLAY_WORK_UNITS),
        run(&program, true, CAP, MAX_REPLAY_WORK_UNITS)
    );
    let tiny = fixture(0, false);
    assert_eq!(
        run(&tiny, false, CAP, MAX_REPLAY_WORK_UNITS),
        run(&tiny, true, CAP, MAX_REPLAY_WORK_UNITS)
    );
}

#[test]
fn schema_regions_entry_and_edge_identity_keep_first_six_field_diagnostics() {
    let original = fixture(64, true);
    for mode in 0..4 {
        let mut program = original.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.name == "main")
            .unwrap();
        match mode {
            0 => function.cleanup_plan.schema = "foreign.cleanup.schema",
            1 => function.cleanup_plan.regions[0].id.0 = u32::MAX,
            2 => function.cleanup_plan.entry.0 = u32::MAX,
            _ => function.cleanup_plan.edges[0].id.0 = u32::MAX,
        }
        let old = run(&program, true, CAP, MAX_REPLAY_WORK_UNITS);
        let current = run(&program, false, CAP, MAX_REPLAY_WORK_UNITS);
        assert!(old.0.is_err());
        assert_eq!(current, old, "complete early tuple mode={mode}");
    }
}

#[test]
fn later_unknown_expression_keeps_full_diagnostic_and_original_work_frontier() {
    let mut program = fixture(64, true);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.name == "main")
        .unwrap();
    let edge = function
        .cleanup_plan
        .edges
        .iter_mut()
        .rev()
        .find(|e| {
            matches!(
                &e.condition,
                EdgeCondition::BooleanResult(..)
                    | EdgeCondition::VariantCase { .. }
                    | EdgeCondition::ArmSelected { .. }
            )
        })
        .unwrap();
    match &mut edge.condition {
        EdgeCondition::BooleanResult(id, _)
        | EdgeCondition::VariantCase { scrutinee: id, .. }
        | EdgeCondition::ArmSelected { scrutinee: id, .. } => {
            *id = ExpressionId::from_owned("missing.λ.edge".into())
        }
        _ => unreachable!(),
    }
    let old = run(&program, true, CAP, MAX_REPLAY_WORK_UNITS);
    let current = run(&program, false, CAP, MAX_REPLAY_WORK_UNITS);
    assert!(old.0.is_err());
    assert_eq!(current.0, old.0, "all six diagnostic fields");
    assert_eq!(current.3, old.3);
}

#[test]
fn first_and_later_original_work_cuts_preserve_complete_diagnostics() {
    let program = fixture(64, true);
    let complete = run(&program, true, CAP, MAX_REPLAY_WORK_UNITS);
    assert!(complete.0.is_ok());
    for work in [0, 1, 12, 64, complete.3 / 2, complete.3 - 1, complete.3] {
        let old = run(&program, true, CAP, work);
        let current = run(&program, false, CAP, work);
        assert_eq!(current.0, old.0, "all six fields work={work}");
        assert_eq!(current.3, old.3);
    }
}

#[test]
fn whole_caller_drop_releases_actual_backing_on_success_and_refusal() {
    let program = fixture(64, true);
    // Warm only test observer state, outside either actual measured caller.
    for old in [true, false] {
        drop(run(&program, old, CAP, MAX_REPLAY_WORK_UNITS));
    }
    for cap in [0, CAP] {
        for old in [true, false] {
            let (_, metrics) = measure(|| {
                drop(run(&program, old, cap, MAX_REPLAY_WORK_UNITS));
            });
            assert!(!metrics.invalid);
            assert_eq!(
                metrics.live, 0,
                "all backing released old={old} cap={cap}: {metrics:?}"
            );
            eprintln!("cross-phase physical old={old} cap={cap} {metrics:?}");
        }
    }
}

#[test]
fn same_abi_source_has_no_early_owner_or_selector_and_consumes_budget_before_callbacks() {
    let acquired = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs",
        "df9680ce32f6df7785f786d8357cf12b43959a504a623b86aee9622c07cae7b6",
        include_str!("ordinary_cross_phase.rs"),
    );
    let source = acquired.as_str();
    let guard = source
        .find("if !prefix_owner.insert(function, &source.id, budget)?")
        .unwrap();
    let receive = source
        .find("retained status fact receiving assignment")
        .unwrap();
    let acquire = source
        .find("let mut session = Session::new_prepaid")
        .unwrap();
    assert!(guard < receive && receive < acquire);
    let start = source
        .find("let mut session = Session::new_prepaid")
        .unwrap();
    let end = source[start..]
        .find("let budget = session.budget;")
        .unwrap()
        + start;
    let closed = &source[start..end];
    for forbidden in [
        "program.full_program(",
        "renewal::validate_binding(",
        "type_needs_drop(",
        "RefCell",
        "FnMut",
        "Option<Session",
        "with_limit(",
        "ACTIVE.with",
    ] {
        assert!(
            !closed.contains(forbidden),
            "closed mutable custody admits {forbidden}"
        );
    }
    assert!(closed.contains("Session::EDGE_RECEIVING_BYTES"));
    assert!(source[end..].contains("renewal::validate_binding("));
    assert!(!source[end..].contains("session.fact"));
}

#[test]
fn exact_source_inverses_restore_every_whole_immediate_owner() {
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-expression-fact-cross-phase-20261018/inverses.json"
    )))
    .unwrap();
    for (path,current) in [
        ("src/cleanup_plan/replay.rs",include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay.rs"))),
        ("src/cleanup_plan/replay/expression_facts/arena/retained.rs",include_str!("../../arena/retained.rs")),
        ("src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs",include_str!("../../arena/retained/tests.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving.rs",include_str!("../../receiving.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs",include_str!("../retained.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs",include_str!("case_certified.rs")),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs",include_str!("case_certified/retained_lookup.rs")),
    ] {
        // Acquire the exact whole c5 caller generation before its unchanged
        // c5 -> c336 -> 847 inverse authorities. Combined CI/raw-source edits
        // are authenticated by the existing current whole-source reader first.
        let expected_c5 = match path {
            "src/cleanup_plan/replay.rs" => "3f59678dffcd84f1fe09fd3e7dae5530501ab63ded29f032ad0831bc4244ee3e",
            "src/cleanup_plan/replay/expression_facts/arena/retained.rs" => "663cc41755cb8083333cb1702352ea59478a35ff427a6bc1039c96007b2a5093",
            "src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs" => "8368b0a218ceedd2e55dcd4c9176ccd4579b6a8389a9bc6b875c906688d0799f",
            "src/cleanup_plan/replay/expression_facts/receiving.rs" => "67f00fcd1acb1f5cd3d218670af3acc80c7ba245d21a7fb2b43d9fd3867a5d44",
            "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => "8e54417f73a57f98c016451910680f81dcc0ba7da32b08c5dea2e9b91473c63b",
            "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs" => "d9826beee4de70f840309b81447450a4e0373cdee3bb4a7c1a7f8e4f14e77709",
            "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs" => "c567429125b11ddaa563be75a0bfbb93330f581279bcd68544a2033e3dc6cbfc",
            _ => unreachable!("closed seven-authority source set"),
        };
        let acquired_c5 = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
            path, expected_c5, current,
        );
        let current = acquired_c5.as_str();
        let restored_c336 = before_bare_pattern_source(path, current);
        let current = restored_c336.as_str();
        let row=records["sources"].as_array().unwrap().iter().find(|r|r["path"]==path).unwrap();
        assert_eq!(current.len(),row["after"]["length"].as_u64().unwrap()as usize);
        assert_eq!(format!("{:x}",crate::digest_hex::LowerHex(sha2::Sha256::digest(current.as_bytes()))),row["after"]["sha256"].as_str().unwrap());
        let mut restored=current.to_owned();
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after=edit["after"].as_str().unwrap();assert_eq!(restored.matches(after).count(),1);
            restored=restored.replacen(after,edit["before"].as_str().unwrap(),1);
        }
        assert_eq!(restored.len(),row["before"]["length"].as_u64().unwrap()as usize);
        assert_eq!(format!("{:x}",crate::digest_hex::LowerHex(sha2::Sha256::digest(restored.as_bytes()))),row["before"]["sha256"].as_str().unwrap());
    }
    eprintln!(
        "cross-phase retained LexicalAttempt={} Session={} S={} Re={}",
        std::mem::size_of::<case_proof::LexicalAttempt<'_, '_>>(),
        std::mem::size_of::<Session<'_, '_, '_>>(),
        Session::OLD_SCRATCH,
        Session::EDGE_QUERY_BYTES
    );
    let path = "src/cleanup_plan/replay/expression_facts/arena/retained.rs";
    let raw = include_str!("../../arena/retained.rs");
    for bad in [
        raw[..raw.len() - 1].to_owned(),
        format!("{raw}//foreign\n"),
        format!("{raw}{raw}"),
    ] {
        assert_ne!(bad, raw);
        assert!(std::panic::catch_unwind(|| before_bare_pattern_source(path, &bad)).is_err());
    }
}

// Compose only the exact independently reviewed post-c336 source delta before
// the immutable c336 whole-source controls above. Other owners stay unchanged.
fn before_bare_pattern_source(path: &str, current: &str) -> String {
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-expression-fact-cross-phase-bare-pattern-20261018/inverses.json"
    )))
    .unwrap();
    let Some(row) = records["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == path)
    else {
        return current.to_owned();
    };
    assert_eq!(
        current.len(),
        row["after"]["length"].as_u64().unwrap() as usize
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(current.as_bytes()))
        ),
        row["after"]["sha256"].as_str().unwrap()
    );
    let mut restored = current.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        restored.len(),
        row["before"]["length"].as_u64().unwrap() as usize
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(restored.as_bytes()))
        ),
        row["before"]["sha256"].as_str().unwrap()
    );
    restored
}

// The immediate control is a complete independently compiled 04 receiver.
// These tests alone select the transition successor; old C5 controls above
// retain their original status/initial-edge equation and full old receiver.
fn block_run(
    context: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    current: bool,
    cap: usize,
    work: usize,
) -> (Result<(), Diagnostic>, bool, usize, usize) {
    let _ = crate::cleanup_plan::replay::ordinary_receiving::take_census();
    let ((result, work), refused, paid) = crate::bounded_output::with_limit_usage(cap, || {
        let mut budget = ReplayBudget::new();
        budget.remaining = work;
        // The production caller establishes this allowance on the same payer.
        let result =
            match reserve_program_skeleton_work(context, std::iter::once(function), &mut budget) {
                Err(error) => Err(error),
                Ok(_) => {
                    if current {
                        validate_structure_with_budget_storage(
                            context,
                            function,
                            &mut budget,
                            None,
                            None,
                        )
                    } else {
                        super::super::ordinary_block_before::validate_structure_with_budget_storage(
                            context,
                            function,
                            &mut budget,
                            None,
                            None,
                        )
                    }
                }
            };
        (result, work - budget.remaining)
    });
    (result, refused, paid, work)
}

fn transition_counts(function: &ResolvedFunction) -> (usize, usize) {
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

#[test]
fn transition_scratch_has_complete_immediate_typed_invoice_and_unchanged_work() {
    use std::mem::size_of;
    for rows in [0, 1, 64] {
        let program = fixture(rows, true);
        let function = function(&program);
        let old = block_run(&program, function, false, CAP, MAX_REPLAY_WORK_UNITS);
        let current = block_run(&program, function, true, CAP, MAX_REPLAY_WORK_UNITS);
        assert_eq!(current.0, old.0, "all six fields rows={rows}");
        assert!(
            current.0.is_ok() && !old.1 && !current.1,
            "rows={rows}: old={old:?} current={current:?}"
        );
        assert_eq!(current.3, old.3, "every original query work debit remains");
        if function.cleanup_plan.status_sources.len() < retained_lookup::MIN_ROWS {
            assert_eq!(current, old, "ineligible route acquires no tail owner");
            continue;
        }
        let (lookups, callbacks) = transition_counts(function);
        let old_query = Session::OLD_SCRATCH
            + 2 * size_of::<Result<Option<&Option<CallFact<'_>>>, Diagnostic>>();
        let new_query = Session::into_edge_local_bytes()
            + Session::into_edge_query_bytes()
            + size_of::<Option<&Option<CallFact<'_>>>>();
        let guards = StorageParent::CURRENT_CONTROLS
            + (size_of::<usize>() + 3 * size_of::<&str>() + size_of::<bool>());
        let cold = StorageParent::ACQUISITION_CONTROLS
            + StorageParent::TERMINAL_CONTROLS
            + (size_of::<usize>() + 3 * size_of::<&str>() + size_of::<bool>());
        // No second activation, allocation, selector or status/edge credit.
        let old_tail = lookups * old_query;
        let new_tail = lookups * new_query + callbacks * guards + cold;
        assert_eq!(old.2 + new_tail, current.2 + old_tail);
        if rows == 64 {
            assert!(lookups > 0 && callbacks > 0);
            assert!(old_tail > new_tail, "complete typed positive gate: lookups={lookups} callbacks={callbacks} old_query={old_query} new_query={new_query} guards={guards} cold={cold} old_tail={old_tail} new_tail={new_tail} old={old:?} current={current:?}");
        }
        eprintln!("transition-scratch rows={rows} lookups={lookups} callbacks={callbacks} old={old:?} current={current:?} old_query={old_query} new_query={new_query} guards={guards}");
    }
}

#[test]
fn transition_scratch_exact_short_zero_floor_and_work_cuts_keep_full_refusals() {
    let program = fixture(4, true);
    let function = function(&program);
    let complete = block_run(&program, function, true, CAP, MAX_REPLAY_WORK_UNITS);
    assert!(complete.0.is_ok() && !complete.1, "complete={complete:?}");
    let exact = block_run(&program, function, true, complete.2, MAX_REPLAY_WORK_UNITS);
    assert!(exact.0.is_ok() && !exact.1, "exact={exact:?}");
    for cap in [0, complete.2 - 1] {
        let short = block_run(&program, function, true, cap, MAX_REPLAY_WORK_UNITS);
        assert!(short.0.is_err() && short.1);
        assert_eq!(short.0.as_ref().unwrap_err().code, "SPX-H006");
    }
    for work in (0..=128.min(complete.3)).chain([
        complete.3 / 3,
        complete.3 / 2,
        complete.3 - 1,
        complete.3,
    ]) {
        let old = block_run(&program, function, false, CAP, work);
        let current = block_run(&program, function, true, CAP, work);
        assert_eq!(current.0, old.0, "all six fields at work={work}");
        assert_eq!(current.3, old.3);
    }
    let ((first, retry), refused, _) = crate::bounded_output::with_limit_usage(CAP, || {
        let mut budget = ReplayBudget::new();
        let remaining = crate::bounded_output::active_remaining().unwrap();
        assert!(crate::bounded_output::set_active_floor(remaining));
        let first =
            validate_structure_with_budget_storage(&program, function, &mut budget, None, None);
        crate::bounded_output::clear_active_floor();
        let retry = budget.reserve_bytes(function, 0, "restored floor cannot clear refusal");
        (first, retry)
    });
    assert!(refused && first.is_err() && retry.is_err());
}

#[test]
fn transition_scratch_keeps_missing_and_noncall_diagnostic_priority_and_source_immutability() {
    for mode in 0..3 {
        let mut program = fixture(8, true);
        let owner = program
            .functions
            .iter_mut()
            .find(|f| f.name == "main")
            .unwrap();
        let body = owner.body.id.clone();
        let mut changed = false;
        for block in &mut owner.cleanup_plan.blocks {
            for transition in &mut block.transitions {
                match transition {
                    CleanupTransition::CallCommit { call, .. } if !changed => {
                        *call = if mode == 0 {
                            body.clone()
                        } else {
                            ExpressionId::from_owned(format!("missing.transition.{mode}"))
                        };
                        changed = true;
                    }
                    _ => {}
                }
            }
        }
        assert!(changed);
        if mode == 2 {
            owner.cleanup_plan.schema = "foreign.before-transition";
        }
        let whole = format!("{program:?}");
        let function = function(&program);
        let old = block_run(&program, function, false, CAP, MAX_REPLAY_WORK_UNITS);
        let current = block_run(&program, function, true, CAP, MAX_REPLAY_WORK_UNITS);
        assert!(old.0.is_err());
        assert_eq!(current.0, old.0, "all six fields mode={mode}");
        assert_eq!(current.3, old.3);
        assert_eq!(format!("{program:?}"), whole);
    }
}

struct CallbackContext<'a> {
    program: &'a ResolvedProgram,
    nested: &'a ResolvedProgram,
    initial_queries: usize,
    mode: u8,
    fired: std::cell::Cell<bool>,
    current: bool,
    target: Option<usize>,
    calls: std::cell::Cell<usize>,
    fired_at: std::cell::Cell<usize>,
    outer: ClassifierLedger,
}
impl crate::cleanup::ReplayContext for CallbackContext<'_> {
    fn declarations(&self) -> &DeclarationIndex {
        &self.program.declarations
    }
    fn types(&self) -> crate::hir::items::View<'_, ResolvedTypeDeclaration> {
        crate::hir::items::View::slice(&self.program.types)
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &self.program.interfaces
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        crate::hir::items::View::slice(&self.program.functions)
    }
    fn function_templates(&self) -> &[ResolvedFunctionTemplate] {
        &self.program.function_templates
    }
    fn function_instances(&self) -> &[ResolvedFunctionInstance] {
        &self.program.function_instances
    }
    fn full_program(&self) -> Option<&ResolvedProgram> {
        Some(self.program)
    }
    fn resolve_call_target(
        &self,
        callee: &DeclarationId,
        instance: Option<&FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        // The existing detached census proves this is a callback AFTER an
        // actual transition query, not an earlier schema/declaration probe.
        let census = crate::cleanup_plan::replay::ordinary_receiving::peek_census();
        assert!(!census.incomplete);
        let at = self.calls.get() + 1;
        self.calls.set(at);
        let reached = match self.target {
            Some(target) => at == target,
            None => census.reached[3] > self.initial_queries,
        };
        if reached && !self.fired.replace(true) {
            self.fired_at.set(at);
            match self.mode {
                0 => {}
                1 => {
                    let nested_function = function(self.nested);
                    let ((result, _), refused, _) = crate::bounded_output::with_limit_usage(
                        CAP,
                        || {
                            let mut budget = ReplayBudget::new();
                            let result = match reserve_program_skeleton_work(
                                self.nested,
                                std::iter::once(nested_function),
                                &mut budget,
                            ) {
                                Err(error) => Err(error),
                                Ok(_) => {
                                    if self.current {
                                        validate_structure_with_budget_storage(
                                            self.nested,
                                            nested_function,
                                            &mut budget,
                                            None,
                                            None,
                                        )
                                    } else {
                                        super::super::ordinary_block_before::validate_structure_with_budget_storage(self.nested, nested_function, &mut budget, None, None)
                                    }
                                }
                            };
                            (result, budget.remaining)
                        },
                    );
                    assert!(
                        result.is_ok() && !refused,
                        "nested current={}: result={result:?} refused={refused}",
                        self.current
                    );
                }
                2 => {
                    assert!(!crate::bounded_output::reserve_active_required(usize::MAX));
                }
                3 => {
                    let (same, _, used) = crate::bounded_output::with_limit_usage(CAP, || {
                        self.outer.storage_parent_current()
                    });
                    assert!(!same);
                    assert_eq!(used, 0, "foreign invocation buys no authority");
                }
                4 => return None,
                5 => panic!("unwind inside actual transition callback"),
                _ => unreachable!(),
            }
        }
        crate::cleanup::ReplayContext::resolve_call_target(self.program, callee, instance)
    }
}

fn callback_run(
    program: &ResolvedProgram,
    nested: &ResolvedProgram,
    current: bool,
    mode: u8,
    target: Option<usize>,
) -> ((Result<(), Diagnostic>, usize), bool, usize) {
    let _ = crate::cleanup_plan::replay::ordinary_receiving::take_census();
    crate::bounded_output::with_limit_usage(CAP, || {
        let context = CallbackContext {
            program,
            nested,
            mode,
            current,
            target,
            initial_queries: queries(function(program)),
            fired: std::cell::Cell::new(false),
            calls: std::cell::Cell::new(0),
            fired_at: std::cell::Cell::new(0),
            outer: ClassifierLedger::capture(),
        };
        let mut budget = ReplayBudget::new();
        let result = match reserve_program_skeleton_work(
            &context,
            std::iter::once(function(program)),
            &mut budget,
        ) {
            Err(error) => Err(error),
            Ok(_) => {
                if current {
                    validate_structure_with_budget_storage(
                        &context,
                        function(program),
                        &mut budget,
                        None,
                        None,
                    )
                } else {
                    super::super::ordinary_block_before::validate_structure_with_budget_storage(
                        &context,
                        function(program),
                        &mut budget,
                        None,
                        None,
                    )
                }
            }
        };
        (result, context.fired_at.get())
    })
}

#[test]
fn transition_callback_restored_nested_sticky_foreign_and_original_error_keep_custody() {
    let program = fixture(8, true);
    let nested = fixture(0, false);
    for mode in [0, 1, 2, 3, 4] {
        let current = callback_run(&program, &nested, true, mode, None);
        assert!(
            current.0 .1 > 0,
            "genuine post-query callback reached mode={mode}"
        );
        let old = callback_run(&program, &nested, false, mode, Some(current.0 .1));
        assert_eq!(old.0 .1, current.0 .1, "same original callback ordinal");
        assert_eq!(
            old.0 .0, current.0 .0,
            "complete original callback diagnostic, mode={mode}"
        );
        if mode <= 1 {
            assert!(
                current.0 .0.is_ok() && !current.1,
                "mode={mode}: old={old:?} current={current:?}"
            );
        } else {
            assert!(current.0 .0.is_err());
            if mode != 4 {
                assert_eq!(current.0 .0.unwrap_err().code, "SPX-H006");
                assert!(current.1);
            } else {
                assert!(
                    !current.1,
                    "callback semantic error keeps priority over custody"
                );
            }
        }
    }
}

#[test]
fn transition_scratch_callback_unwind_success_and_refusal_drop_all_backing() {
    // Warm only the thread panic runtime before observing real compiler owners.
    let runtime = std::panic::catch_unwind(|| panic!("885 initialize thread panic hook"));
    assert!(runtime.is_err());
    drop(runtime);
    let program = fixture(8, true);
    let nested = fixture(0, false);
    for mode in [0, 2, 3, 5] {
        let (_, metrics) = measure(|| {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                callback_run(&program, &nested, true, mode, None)
            }));
            assert_eq!(result.is_err(), mode == 5);
            drop(result);
        });
        assert!(
            !metrics.invalid && metrics.live == 0,
            "mode={mode}: {metrics:?}"
        );
    }
}

#[test]
fn immediate_transition_oracle_restores_every_original_byte_and_keeps_all_old_queries() {
    let whole = include_str!("ordinary_block_before.rs");
    let glue = "#[cfg(any())]\n#[path = \"ordinary_cross_phase_tests.rs\"]\nmod tests;";
    assert_eq!(whole.matches(glue).count(), 1);
    let restored = whole.replacen(
        glue,
        "#[path = \"ordinary_cross_phase_tests.rs\"]\nmod tests;",
        1,
    );
    assert_eq!(restored.len(), 79447);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(sha2::Sha256::digest(restored.as_bytes()))
        ),
        "e920a1dd63770419dd6bb5459cfbfd7bd5f72c7e594b6c6c0e6ecd337a5dc739"
    );
    assert!(restored.contains("drop(failure);\n"));
    assert_eq!(restored.matches("require_expression(\n").count(), 9);
    assert!(restored.contains("expressions.fact(function, call, budget)?"));
}

#[test]
fn transition_without_skeleton_preflight_keeps_complete_root_refusal_and_drop() {
    let program = fixture(0, false);
    let function = function(&program);
    for current in [false, true] {
        let (_, metrics) = measure(|| {
            let (result, refused, _) = crate::bounded_output::with_limit_usage(CAP, || {
                let mut budget = ReplayBudget::new();
                if current {
                    validate_structure_with_budget_storage(
                        &program,
                        function,
                        &mut budget,
                        None,
                        None,
                    )
                } else {
                    super::super::ordinary_block_before::validate_structure_with_budget_storage(
                        &program,
                        function,
                        &mut budget,
                        None,
                        None,
                    )
                }
            });
            assert_eq!(
                result,
                Err(replay_error(
                    function,
                    "cleanup replay work budget exhausted during HIR root path"
                )),
                "current={current}, refused={refused}"
            );
            assert!(!refused, "work refusal does not forge a byte refusal");
            drop(result);
        });
        assert!(
            !metrics.invalid && metrics.live == 0,
            "current={current}: {metrics:?}"
        );
    }
}
