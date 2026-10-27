//! The earlier representation retains full enumeration and weighted languages.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;

mod before;

fn fixture(decisions: usize, owned: bool) -> ResolvedProgram {
    let branches = (0..decisions)
        .map(|index| format!("let branch_{index}=if 0=={index} {{1}}else{{2}};"))
        .collect::<String>();
    let parameters = if owned { "input: own Bytes" } else { "" };
    let source = format!(
        "module moderate.paths; @id(\"moderate.paths\") fn paths({parameters})->i64 {{{branches}42}}"
    );
    // The complete original library-shaped literal is not an executable.
    // Retain its exact six-field refusal before adding a separate legal main;
    // paths remains the first function and every original replay assertion uses it.
    let diagnostics = crate::check(&source, "moderate-paths.spx").unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.code, "SPX-T105");
    assert_eq!(diagnostic.severity, crate::diagnostic::Severity::Error);
    assert_eq!(
        diagnostic.message,
        "executable module must define `fn main() -> i64`"
    );
    assert_eq!(diagnostic.path.as_deref(), Some("moderate-paths.spx"));
    let start = source.find("fn paths").unwrap();
    assert_eq!(
        diagnostic
            .span
            .as_ref()
            .map(|span| (span.start, span.end, span.line, span.column)),
        Some((start, source.len(), 1, start + 1))
    );
    assert_eq!(diagnostic.help.as_deref(), Some("a module without `fn main() -> i64` is a library module: check it through the project that owns it with `semaprax check <project-dir>`, or add `main` to run this file alone"));
    let source = format!("{source}\n@id(\"moderate.main\") fn main()->i64 {{0}}");
    crate::hir::resolve(&crate::check(&source, "moderate-paths.spx").unwrap()).unwrap()
}

fn enumerated(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<bool, Diagnostic> {
    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
    let mut hir = hir_skeleton_paths(program, function, &mut budget)?;
    let mut plan = plan_skeleton_paths(function, &mut budget)?;
    budget.reserve_bytes(
        function,
        3 * skeleton_work::enumerated_prefix::ENTRY_CALL,
        "independent moderate full-content comparison entries",
    )?;
    skeleton_work::enumerated_prefix::sort(&mut hir, function, &mut budget)?;
    skeleton_work::enumerated_prefix::sort(&mut plan, function, &mut budget)?;
    skeleton_work::enumerated_prefix::equal(&hir, &plan, function, &mut budget)
}

#[test]
fn moderate_scalar_route_keeps_owned_fallbacks_and_complete_enumerated_multisets() {
    for (decisions, owned) in [
        (8, false),
        (9, false),
        (10, false),
        (8, true),
        (9, true),
        (10, true),
    ] {
        let program = fixture(decisions, owned);
        let function = &program.functions[0];
        assert_eq!(!function.cleanup_plan.slots.is_empty(), owned);
        assert!(function.cleanup_plan.lexical_control.is_none());
        let cfg = branch_sensitive_cfg_bounds(function)
            .unwrap()
            .terminal_paths;
        let hir = hir_terminal_path_bound(function).unwrap();
        assert_eq!(cfg, 1 << decisions);
        assert_eq!(hir, cfg);
        assert!(cfg <= MAX_REPLAY_PATHS);
        assert!(enumerated(&program, function).unwrap());
        assert!(!before::selected_paid(function, cfg, hir, &mut ReplayBudget::new()).unwrap());
        assert_eq!(selected(function, cfg, hir), !owned && decisions > 8);
        assert_eq!(
            selected_paid(function, cfg, hir, &mut ReplayBudget::new()).unwrap(),
            !owned && decisions > 8
        );
        assert_eq!(
            compare(
                &program,
                function,
                &mut ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS)
            )
            .unwrap(),
            Some(true)
        );
        assert_eq!(
            validate(
                &program,
                function,
                &mut ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS)
            )
            .unwrap(),
            !owned && decisions > 8
        );
    }
}

#[test]
fn weighted_representation_cannot_hide_a_changed_boolean_route_or_its_multiplicity() {
    let program = fixture(9, false);
    let original = &program.functions[0];
    let mut changed = original.clone();
    let condition = changed
        .cleanup_plan
        .edges
        .iter_mut()
        .find_map(|edge| match &mut edge.condition {
            EdgeCondition::BooleanResult(_, value) => Some(value),
            _ => None,
        })
        .expect("fixture contains retained Boolean observations");
    *condition = !*condition;
    assert!(!enumerated(&program, &changed).unwrap());
    assert_eq!(
        compare(
            &program,
            &changed,
            &mut ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS)
        )
        .unwrap(),
        Some(false)
    );
    assert!(validate_structure(&program, &changed).is_err());
}

#[test]
fn moderate_comparison_actual_builder_keeps_exact_short_zero_and_floor_refusals() {
    use crate::bounded_output::set_active_floor;
    let program = fixture(9, false);
    let function = &program.functions[0];
    let observe = |available, floor| {
        with_limit_usage_refusal(available + floor, || {
            assert!(set_active_floor(floor));
            let mut budget = ReplayBudget::new();
            let result = compare(&program, function, &mut budget);
            if crate::bounded_output::active_refused() {
                assert!(result.is_err());
                assert!(!crate::bounded_output::reserve_active_required(0));
            }
            result
        })
    };
    let (result, refused, used, receipt) = observe(usize::MAX, 0);
    assert_eq!(result.unwrap(), Some(true));
    assert!(!refused && receipt.is_none() && used > 0);
    for floor in [0, 37] {
        let (result, refused, exact, receipt) = observe(used, floor);
        assert_eq!(result.unwrap(), Some(true));
        assert!(!refused && receipt.is_none());
        assert_eq!(exact, used);
        for available in [0, used - 1] {
            let (result, refused, actual, receipt) = observe(available, floor);
            assert!(result.is_err());
            assert!(refused);
            let receipt = receipt.unwrap();
            assert_eq!(receipt.floor, floor);
            assert!(actual <= available);
        }
    }
}

#[test]
fn original_selector_and_whole_source_receivers_remain_bound() {
    use sha2::{Digest, Sha256};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/moderate-factored-v1");
    let rows: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 3);
    for row in rows.as_array().unwrap() {
        let original =
            std::fs::read_to_string(evidence.join(row["archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
        let current = std::fs::read_to_string(root.join(row["path"].as_str().unwrap())).unwrap();
        let current = joined_input(row["path"].as_str().unwrap(), &current);
        let restore = |text: &str| -> Option<String> {
            let mut text = text.to_owned();
            for edit in row["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if text.matches(new).count() != 1 {
                    return None;
                }
                text = text.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(text)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen(&current[..1], "!", 1)).as_deref(),
            Some(original.as_str())
        );
    }
    let original = std::fs::read_to_string(evidence.join("0.before.txt")).unwrap();
    let selector = original
        .split_once("pub(super) fn selected_paid(")
        .unwrap()
        .1
        .split_once("\n#[cfg(test)]")
        .unwrap()
        .0
        .trim_end();
    let current = include_str!("moderate_tests/before.rs");
    let prefix = "use super::*;\nconst FACTORED_PATH_THRESHOLD: usize = 4_096;\n\npub(super) fn selected_paid(";
    assert_eq!(current.strip_prefix(prefix).unwrap().trim_end(), selector);
    joined_hostiles();
}

// Only this incoming historical owner receives a finite view. Production and
// complete raw source inventories continue to inspect the actual joined bytes.
const JOINED_LENGTH: usize = 319918;
const JOINED_SHA256: &str = "3c8e81fe794761cd061189a17ac6c003311920c299599c9b1cbb57b771fda258";
const JOINED_PATH: &str =
    "docs/evidence/moderate-factored-joined-custody-20261017/joined-inputs.json";
fn joined_digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
#[allow(
    clippy::assertions_on_constants,
    reason = "These assertions pin the exact control layout and resource limits."
)]
fn joined_data() -> String {
    use std::io::Read;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut file = std::fs::File::open(root.join(JOINED_PATH)).unwrap();
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert!(JOINED_LENGTH <= 2 * 1024 * 1024);
    assert_eq!(metadata.len(), JOINED_LENGTH as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(JOINED_LENGTH).unwrap();
    bytes.resize(JOINED_LENGTH, 0);
    file.read_exact(&mut bytes).unwrap();
    assert_eq!(file.read(&mut [0]).unwrap(), 0);
    assert_eq!(joined_digest(&bytes), JOINED_SHA256);
    String::from_utf8(bytes).unwrap()
}
fn joined_checked(data: &str, path: &str, current: &str) -> Option<String> {
    if data.len() != JOINED_LENGTH || joined_digest(data.as_bytes()) != JOINED_SHA256 {
        return None;
    }
    let rows: serde_json::Value = serde_json::from_str(data).ok()?;
    let rows = rows.as_array()?;
    if rows.len() != 3 || rows.iter().filter(|row| row["path"] == path).count() != 1 {
        return None;
    }
    let row = rows.iter().find(|row| row["path"] == path)?;
    if current.len() != row["current_length"].as_u64()? as usize
        || joined_digest(current.as_bytes()) != row["current_sha256"].as_str()?
    {
        return None;
    }
    let mut restored = current.to_owned();
    for edit in row["edits"].as_array()?.iter().rev() {
        let start = usize::try_from(edit["start"].as_u64()?).ok()?;
        let end = usize::try_from(edit["end"].as_u64()?).ok()?;
        if restored.get(start..end)? != edit["current"].as_str()? {
            return None;
        }
        restored.replace_range(start..end, edit["incoming"].as_str()?);
    }
    if restored.len() != row["incoming_length"].as_u64()? as usize
        || joined_digest(restored.as_bytes()) != row["incoming_sha256"].as_str()?
    {
        return None;
    }
    Some(restored)
}
fn joined_input(path: &str, current: &str) -> String {
    let checkpoint = joined_checkpoint_input(path, current);
    crate::source_reflows_merge::main7f8_source_inputs::moderate_452_input(
        "moderate-original",
        path,
        &checkpoint,
    )
}
fn joined_checkpoint_input(path: &str, current: &str) -> String {
    joined_checked(&joined_data(), path, current)
        .expect("complete joined-to-incoming source domain")
}
fn joined_hostiles() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = joined_data();
    let rows: serde_json::Value = serde_json::from_str(&data).unwrap();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let raw = std::fs::read_to_string(root.join(path)).unwrap();
        let target =
            std::fs::read_to_string(root.join(row["incoming_archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            joined_checked(&data, path, &raw).as_deref(),
            Some(target.as_str())
        );
        for wrong in [
            String::new(),
            raw[..raw.len() / 2].to_owned(),
            format!("{raw} "),
            format!("!{}", &raw[1..]),
        ] {
            assert!(joined_checked(&data, path, &wrong).is_none());
        }
        if raw != target {
            assert!(joined_checked(&data, path, &target).is_none());
        }
    }
    assert!(joined_checked(&data, "src/foreign.rs", "foreign").is_none());
    assert!(joined_checked(&data[..data.len() - 1], "src/foreign.rs", "foreign").is_none());
    assert!(joined_checked(&format!("!{}", &data[1..]), "src/foreign.rs", "foreign").is_none());
}

#[test]
fn ordinary_builder_crossover_128_keeps_reference_routes_and_complete_languages() {
    use crate::cleanup_plan::replay::allocation::{opt_in, OracleScope};
    use crate::test_allocation_observer::measure;
    const PRODUCTION_BYTES: usize = 64 * 1024 * 1024;
    // Independent, explicit test-oracle accounts. These never increase ACTIVE,
    // grant production work, refund a request, or skip either full receiver.
    const ORACLE_BYTES: usize = 1 << 29;
    const ORACLE_WORK: usize = 2 * MAX_REPLAY_WORK_UNITS;
    let mut unopted_refusals = 0;
    for decisions in [6, 7, 8, 9] {
        for owned in [false, true] {
            let program = fixture(decisions, owned);
            let function = &program.functions[0];
            let cfg = branch_sensitive_cfg_bounds(function)
                .unwrap()
                .terminal_paths;
            let hir = hir_terminal_path_bound(function).unwrap();
            assert_eq!((cfg, hir), (1 << decisions, 1 << decisions));
            assert!(enumerated(&program, function).unwrap());
            assert_eq!(selected(function, cfg, hir), !owned && decisions > 8);

            // Keep the complete unopted entry at its original production cap.
            // Its cfg(test) enumeration/factored cross-checks also pay ACTIVE;
            // a genuine first refusal is retained as a separate negative.
            let ((unopted, refused, paid, receipt, remaining), metrics) = measure(|| {
                let ((result, remaining), refused, paid, receipt) =
                    with_limit_usage_refusal(PRODUCTION_BYTES, || {
                        let mut budget = ReplayBudget::new();
                        let result = validate(&program, function, &mut budget);
                        let remaining = (budget.remaining, budget.skeleton_remaining);
                        (result, remaining)
                    });
                let result = match result {
                    Ok(selected) => {
                        assert_eq!(selected, decisions > 7);
                        assert!(!refused && receipt.is_none());
                        Ok(selected)
                    }
                    Err(error) => {
                        assert_eq!(error, replay_error(function,
                            format_args!("ordinary builder already failed")),
                            "all six unopted diagnostic fields");
                        assert!(refused && receipt.is_some());
                        assert_eq!(remaining, (0, 0));
                        drop(error);
                        Err(())
                    }
                };
                (result, refused, paid, receipt, remaining)
            });
            if unopted.is_err() {
                unopted_refusals += 1;
            }
            assert!(paid <= PRODUCTION_BYTES && !metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            eprintln!("moderate unopted decisions={decisions} owned={owned} result={unopted:?} refused={refused} paid={paid} receipt={receipt:?} work_left={remaining:?} allocation={metrics:?}");

            // The shipping selector/comparison still pays the SAME ACTIVE cap;
            // only the complete independent test cross-check uses its own
            // cumulative byte/work account, with every invocation balanced.
            let ((result, refused, paid, receipt, remaining, oracle), metrics) = measure(|| {
                let scope = OracleScope::open_with_work(ORACLE_BYTES, ORACLE_WORK).unwrap();
                let opted = opt_in(scope.lease()).unwrap();
                let ((result, remaining), refused, paid, receipt) =
                    with_limit_usage_refusal(PRODUCTION_BYTES, || {
                        let mut budget = ReplayBudget::new();
                        let result = validate(&program, function, &mut budget);
                        let remaining = (budget.remaining, budget.skeleton_remaining);
                        (result, remaining)
                    });
                drop(opted);
                let oracle = scope.finish();
                (result, refused, paid, receipt, remaining, oracle)
            });
            assert_eq!(result.unwrap(), decisions > 7);
            assert!(!refused && receipt.is_none());
            assert!(paid <= PRODUCTION_BYTES);
            assert_eq!((oracle.allowance, oracle.work_allowance), (ORACLE_BYTES, ORACLE_WORK));
            assert_eq!((oracle.started, oracle.completed), (2, 2));
            assert!(oracle.charged > 0 && oracle.charged <= ORACLE_BYTES);
            assert!(oracle.work > 0 && oracle.work <= ORACLE_WORK);
            assert!(oracle.calls > 0 && oracle.work_calls > 0);
            assert!(oracle.first_refusal.is_none() && oracle.first_work_refusal.is_none());
            assert!(oracle.physical_failure.is_none());
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
            eprintln!("moderate independently funded decisions={decisions} owned={owned} production_paid={paid} work_left={remaining:?} oracle={oracle:?} allocation={metrics:?}");
        }
    }
    assert!(unopted_refusals > 0, "retain the actual unopted ACTIVE refusal");
}

#[test]
fn ordinary_builder_crossover_keeps_complete_legacy_hostile_diagnostic() {
    let program = fixture(8, true);
    let mut changed = program.functions[0].clone();
    let condition = changed
        .cleanup_plan
        .edges
        .iter_mut()
        .find_map(|edge| match &mut edge.condition {
            EdgeCondition::BooleanResult(_, value) => Some(value),
            _ => None,
        })
        .expect("retained Boolean route");
    *condition = !*condition;
    let original = validate_structure(&program, &changed).unwrap_err();
    let (current, refused, _, receipt) =
        with_limit_usage_refusal(64 * 1024 * 1024, || validate_structure(&program, &changed));
    assert_eq!(current.unwrap_err(), original, "all six diagnostic fields");
    assert!(!refused && receipt.is_none());
}

// Complete immediate predecessor. Literal bodies and tariff expression are
// retained independently; only their lexical module gives them separate names.
#[rustfmt::skip]
mod before_cold_threshold {
use super::*;
const FITTING_REFUSAL_PROBE_CONTROLS: usize = {
    use std::cell::{Cell, Ref, RefCell};
    use std::thread::LocalKey;
    type Active = RefCell<Option<Rc<()>>>;
    type Loan = Ref<'static, Option<Rc<()>>>;
    2 * size_of::<&ReplayBudget>()
        + 2 * size_of::<bool>()
        + 2 * size_of::<&LocalKey<Active>>()
        + 4 * size_of::<&Active>()
        + size_of::<Loan>()
        + 2 * size_of::<&Loan>()
        + 3 * size_of::<&Option<Rc<()>>>()
        + 6 * size_of::<Option<Rc<()>>>()
        + 5 * size_of::<Rc<()>>()
        + 6 * size_of::<&Rc<()>>()
        + size_of::<&()>()
        + 2 * size_of::<&Cell<bool>>()
        + 4 * size_of::<bool>()
};

pub(super) const BUILDER_SELECTOR_CONTROLS: usize = {
    use std::mem::size_of;
    4 * size_of::<&ResolvedFunction>()
        + 4 * size_of::<&mut ReplayBudget>()
        + 6 * size_of::<usize>() // two path argument/formal pairs and two threshold operands
        + size_of::<[bool; 8]>() // selector six roles plus !selected argument/result
        + 2 * size_of::<Result<bool, Diagnostic>>()
        + 4 * size_of::<Result<(), Diagnostic>>()
        + 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
        + FITTING_REFUSAL_PROBE_CONTROLS
};

const BUILDER_FACTORED_PATH_THRESHOLD: usize = 128;

pub(super) fn selected_builder_paid(
    function: &ResolvedFunction,
    cfg_paths: usize,
    semantic_paths: usize,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if cfg_paths <= BUILDER_FACTORED_PATH_THRESHOLD
        && semantic_paths <= BUILDER_FACTORED_PATH_THRESHOLD
    {
        return Ok(false);
    }
    match structural_control::validate_ordinary_cfg(function, budget) {
        Ok(()) => Ok(true),
        Err(error) if budget.refusal_spent || crate::bounded_output::active_refused() => Err(error),
        Err(_) => Ok(false),
    }
}

pub(super) fn validate_with_index_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'hir>>,
    continuations: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::ReplayFactored,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::ReplayFactored,
    );
    let cfg_paths = branch_sensitive_cfg_bounds(function)?.terminal_paths;
    let semantic_paths = hir_terminal_path_bound(function)?;
    let mut selected = if let ReplayBudget {
        payer: AllocationPayer::ActiveBuilder,
        ..
    } = budget
    {
        budget.reserve_bytes(
            function,
            BUILDER_SELECTOR_CONTROLS,
            "ordinary builder representation selector receivers",
        )?;
        selected_builder_paid(
            function,
            cfg_paths,
            semantic_paths,
            &mut ReplayBudget::new(),
        )?
    } else {
        selected_paid(
            function,
            cfg_paths,
            semantic_paths,
            &mut ReplayBudget::new(),
        )?
    };
    // Keep both existing selectors above unchanged. Only the ordinary builder
    // evaluates this nonselection guard; its two bool roles were prepaid in
    // BUILDER_SELECTOR_CONTROLS before entering the initial selector.
    if let ReplayBudget {
        payer: AllocationPayer::ActiveBuilder,
        ..
    } = budget
    {
        if !selected {
            // Every fitting header read follows payment of its own bank.
            budget.reserve_bytes(
                function,
                FITTING_SELECTOR_CONTROLS,
                "fitting factored comparison selector receivers",
            )?;
            selected =
                fitting_comparison_selected_paid(function, cfg_paths, semantic_paths, budget)?;
        }
    }
    #[cfg(test)]
    let continuations = {
        let mut continuations = continuations;
        if !selected
            || (cfg_paths <= ENUMERATION_CROSS_CHECK_THRESHOLD
                && semantic_paths <= ENUMERATION_CROSS_CHECK_THRESHOLD)
        {
            oracle_hook::cross_check_with_storage(
                program,
                function,
                budget,
                continuations.as_deref_mut(),
            )?;
        }
        continuations
    };
    if !selected {
        return Ok(false);
    }
    match compare_with_index_storage(program, function, budget, helpers, continuations) {
        Ok(Some(true)) => Ok(true),
        Ok(Some(false)) => Err(skeleton_mismatch(function)),
        Err(error)
            if cfg_paths <= MAX_REPLAY_PATHS
                && semantic_paths <= MAX_REPLAY_PATHS
                && (budget.refusal_spent || crate::bounded_output::active_refused()) =>
        {
            Err(error)
        }
        verdict => {
            if cfg_paths <= MAX_REPLAY_PATHS && semantic_paths <= MAX_REPLAY_PATHS {
                // Enumeration still fits; an attempted factored comparison
                // keeps every work and physical debit already spent.
                return Ok(false);
            }
            match verdict {
                Err(error) if !error.message.contains("budget") => Err(error),
                _ => path_summary::path_budget_errors(function, cfg_paths, semantic_paths)
                    .map(|()| false),
            }
        }
    }
}
}

// The test receiver returns the same boolean/Diagnostic boundary as the actual
// lexical expansion. It never stands in for the full caller controls below.
fn cold_selector_probe(
    function: &ResolvedFunction,
    cfg_paths: usize,
    semantic_paths: usize,
    budget: &mut ReplayBudget,
    before: bool,
) -> Result<bool, Diagnostic> {
    if before {
        budget.reserve_bytes(
            function,
            before_cold_threshold::BUILDER_SELECTOR_CONTROLS,
            "ordinary builder representation selector receivers",
        )?;
        before_cold_threshold::selected_builder_paid(
            function, cfg_paths, semantic_paths, &mut ReplayBudget::new(),
        )
    } else {
        Ok(ordinary_builder_selected!(function, cfg_paths, semantic_paths, budget))
    }
}
fn cold_complete(metrics: crate::test_allocation_observer::Metrics, paid: usize) {
    assert!(!metrics.invalid && metrics.live == 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
}
#[test]
fn cold_builder_threshold_both_operands_and_hot_routes_keep_complete_invoices() {
    use crate::test_allocation_observer::measure;
    let program = fixture(0, false);
    let function = &program.functions[0];
    assert_eq!(BUILDER_SELECTOR_CONTROLS, before_cold_threshold::BUILDER_SELECTOR_CONTROLS);
    assert!(COLD_BUILDER_THRESHOLD_CONTROLS < BUILDER_SELECTOR_CONTROLS);
    for (cfg_paths, semantic_paths) in [
        (0, 0), (128, 128), (129, 0), (0, 129), (129, 129),
        (usize::MAX, 0), (0, usize::MAX),
    ] {
        let cold = cfg_paths <= 128 && semantic_paths <= 128;
        let mut observations = Vec::new();
        for before in [true, false] {
            let ((remaining, metrics), refused, paid, first) = with_limit_usage_refusal(64 << 20, || measure(|| {
                let mut budget = ReplayBudget::new();
                let result = cold_selector_probe(function, cfg_paths, semantic_paths, &mut budget, before);
                assert_eq!(result, Ok(!cold));
                let remaining = budget.remaining;
                drop(result); drop(budget); remaining
            }));
            assert!(!refused && first.is_none());
            cold_complete(metrics, paid);
            observations.push((remaining, metrics, paid));
        }
        assert_eq!(observations[0].0, observations[1].0, "selector work remains on its original temporary account");
        assert_eq!(observations[0].1.allocated, observations[1].1.allocated);
        if cold {
            assert_eq!(observations[0].2 - observations[1].2,
                BUILDER_SELECTOR_CONTROLS - COLD_BUILDER_THRESHOLD_CONTROLS);
        } else {
            assert_eq!(observations[1].2 - observations[0].2,
                COLD_BUILDER_THRESHOLD_CONTROLS + HOT_BUILDER_THRESHOLD_GATE_CONTROLS);
        }
    }
    // Inspect the actual expansion, rather than substitute an eager predicate.
    // Both path slots are already immutable usize values in the shipping caller.
    let source = include_str!("../../factored.rs");
    assert!(source.contains("if $cfg_paths <= paired::BUILDER_FACTORED_PATH_THRESHOLD\n            && $semantic_paths <= paired::BUILDER_FACTORED_PATH_THRESHOLD"));
}
#[test]
fn cold_builder_threshold_exact_short_zero_floor_and_sticky_fields_remain_owned() {
    use crate::test_allocation_observer::measure;
    let program = fixture(0, false);
    let function = &program.functions[0];
    let retained_retry = replay_error(function, "ordinary builder already failed");
    let consumed_retry = Diagnostic::io("SPX-H006", String::new());
    for paths in [(128, 128), (129, 128), (128, 129)] {
        for before in [true, false] {
            let observe = |available: usize, floor: usize| {
                with_limit_usage_refusal(available.checked_add(floor).unwrap(), || {
                    if floor != 0 { assert!(crate::bounded_output::set_active_floor(floor)); }
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        let result = cold_selector_probe(function, paths.0, paths.1, &mut budget, before);
                        let okay = result.is_ok();
                        if let Err(error) = &result {
                            assert_eq!(error.code, "SPX-H006");
                            assert_eq!(error.severity, crate::diagnostic::Severity::Error);
                            assert!(error.path.is_none() && error.span.is_none() && error.help.is_none());
                            // A hot callee can select failure on its own old
                            // temporary budget. The outer owner then publishes
                            // its retained first receipt exactly once on retry.
                            let was_selected = budget.refusal_spent;
                            let retry = cold_selector_probe(function, paths.0, paths.1, &mut budget, before).unwrap_err();
                            assert_eq!(&retry, if was_selected { &consumed_retry } else { &retained_retry }, "all six sticky fields");
                            let again = cold_selector_probe(function, paths.0, paths.1, &mut budget, before).unwrap_err();
                            assert_eq!(again, consumed_retry, "no second receipt or fallback allocation");
                            assert_eq!((budget.remaining, budget.skeleton_remaining), (0, 0));
                            drop(again); drop(retry);
                        }
                        drop(result); drop(budget); okay
                    })
                })
            };
            let ((success, metrics), refused, paid, first) = observe(64 << 20, 0);
            assert!(success && !refused && first.is_none()); cold_complete(metrics, paid);
            for floor in [0, 37] {
                let ((success, metrics), refused, exact, first) = observe(paid, floor);
                assert!(success && !refused && first.is_none()); assert_eq!(exact, paid); cold_complete(metrics, exact);
                for available in [0, paid - 1] {
                    let ((success, metrics), refused, used, first) = observe(available, floor);
                    assert!(!success && refused && first.is_some()); assert!(used <= available);
                    assert_eq!(first.unwrap().floor, floor); cold_complete(metrics, used);
                }
            }
        }
    }
    // The unchanged first bootstrap refusal has exactly the old six fields.
    let fail = |before| with_limit_usage_refusal(0, || {
        let mut budget = ReplayBudget::new();
        cold_selector_probe(function, 128, 128, &mut budget, before).unwrap_err()
    }).0;
    assert_eq!(fail(false), fail(true));
}
#[test]
fn cold_builder_threshold_expired_foreign_and_unwind_keep_current_authority_and_drop() {
    use crate::test_allocation_observer::measure;
    let program = fixture(0, false);
    let function = &program.functions[0];
    let stale = crate::cleanup_plan::replay::allocation::OracleScope::open(1 << 20).unwrap().lease();
    let (((), metrics), refused, paid, first) = with_limit_usage_refusal(64 << 20, || measure(|| {
        let mut old = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, stale);
        let mut new = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, stale);
        let a = cold_selector_probe(function, 128, 128, &mut old, true).unwrap_err();
        let b = cold_selector_probe(function, 128, 128, &mut new, false).unwrap_err();
        assert_eq!(a, b, "all six expired-payer fields without ACTIVE fallback");
        drop(a); drop(b); drop(old); drop(new);
    }));
    assert!(!refused && paid == 0 && first.is_none());
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    for before in [true, false] {
        let ((unwind, metrics), refused, paid, first) = with_limit_usage_refusal(64 << 20, || measure(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut budget = ReplayBudget::new();
                assert_eq!(cold_selector_probe(function, 129, 128, &mut budget, before), Ok(true));
                std::panic::resume_unwind(Box::new("after complete paid threshold selection"));
            })).is_err()
        }));
        assert!(unwind && !refused && first.is_none()); cold_complete(metrics, paid);
    }
}
fn cold_full_caller(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    before: bool,
) -> Result<bool, Diagnostic> {
    if before {
        before_cold_threshold::validate_with_index_storage(program, function, budget, None, None)
    } else {
        validate_with_index_storage(program, function, budget, None, None)
    }
}
#[test]
fn cold_builder_threshold_complete_old_current_languages_work_and_drop_remain_equal() {
    use crate::cleanup_plan::replay::allocation::{opt_in, OracleScope};
    use crate::test_allocation_observer::measure;
    for decisions in [0, 1, 7, 8] {
        for owned in [false, true] {
            let program = fixture(decisions, owned);
            let function = &program.functions[0];
            assert!(enumerated(&program, function).unwrap());
            let mut observations = Vec::new();
            for before in [true, false] {
                let ((remaining, metrics), refused, paid, first) = with_limit_usage_refusal(64 << 20, || measure(|| {
                    // Exact original moderate owner oracle ceilings; production
                    // ACTIVE remains64MiB and both complete comparison legs run.
                    let scope = OracleScope::open_with_work(1 << 29, 2 * MAX_REPLAY_WORK_UNITS).unwrap();
                    let opted = opt_in(scope.lease()).unwrap();
                    let mut budget = ReplayBudget::new();
                    let result = cold_full_caller(&program, function, &mut budget, before);
                    assert_eq!(result, Ok(decisions > 7));
                    let remaining = (budget.remaining, budget.skeleton_remaining);
                    drop(result); drop(budget); drop(opted);
                    let receipt = scope.finish();
                    assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none() && receipt.physical_failure.is_none());
                    assert_eq!(receipt.started, receipt.completed);
                    (remaining, receipt.charged)
                }));
                assert!(!refused && first.is_none());
                // The observer spans both fully executed algorithms; each
                // independently pays its own unchanged finite account.
                cold_complete(metrics, paid.checked_add(remaining.1).unwrap());
                observations.push((remaining.0, paid, remaining.1));
            }
            assert_eq!(observations[0].0, observations[1].0);
            assert_eq!(observations[0].2, observations[1].2, "complete independent oracle invoice");
            if decisions <= 7 {
                assert_eq!(observations[0].1 - observations[1].1,
                    BUILDER_SELECTOR_CONTROLS - COLD_BUILDER_THRESHOLD_CONTROLS);
            } else {
                assert_eq!(observations[1].1 - observations[0].1,
                    COLD_BUILDER_THRESHOLD_CONTROLS + HOT_BUILDER_THRESHOLD_GATE_CONTROLS);
            }
        }
    }
}

#[test]
fn cold_builder_threshold_full_changed_boolean_keeps_all_six_diagnostic_fields() {
    use crate::cleanup_plan::replay::allocation::{opt_in, OracleScope};
    use crate::test_allocation_observer::measure;
    for decisions in [1, 8] {
        let program = fixture(decisions, false);
        let mut changed = program.functions[0].clone();
        let condition = changed.cleanup_plan.edges.iter_mut().find_map(|edge| match &mut edge.condition {
            EdgeCondition::BooleanResult(_, value) => Some(value), _ => None,
        }).unwrap();
        *condition = !*condition;
        let expected = with_limit_usage_refusal(64 << 20, || {
            let scope = OracleScope::open_with_work(1 << 29, 2 * MAX_REPLAY_WORK_UNITS).unwrap();
            let opted = opt_in(scope.lease()).unwrap();
            let mut budget = ReplayBudget::new();
            let result = cold_full_caller(&program, &changed, &mut budget, true);
            drop(budget); drop(opted); let receipt = scope.finish();
            assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none());
            result
        });
        assert!(!expected.1 && expected.3.is_none());
        if decisions <= 7 { assert_eq!(expected.0, Ok(false), "nonselection grants no verdict"); }
        else { assert!(expected.0.is_err()); }
        for before in [true, false] {
            let ((oracle_paid, metrics), refused, paid, first) = with_limit_usage_refusal(64 << 20, || measure(|| {
                let scope = OracleScope::open_with_work(1 << 29, 2 * MAX_REPLAY_WORK_UNITS).unwrap();
                let opted = opt_in(scope.lease()).unwrap();
                let mut budget = ReplayBudget::new();
                let result = cold_full_caller(&program, &changed, &mut budget, before);
                assert_eq!(result, expected.0, "whole old/current boolean mismatch and all six fields");
                drop(result); drop(budget); drop(opted); let receipt = scope.finish();
                assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none() && receipt.physical_failure.is_none());
                assert_eq!(receipt.started, receipt.completed); receipt.charged
            }));
            assert!(!refused && first.is_none()); cold_complete(metrics, paid.checked_add(oracle_paid).unwrap());
        }
        // A cold false is not acceptance. Run the complete ordinary driver,
        // including the unchanged enumerating fallthrough and final verdict,
        // against its independent Legacy route's six-field diagnostic.
        let legacy = validate_structure(&program, &changed).unwrap_err();
        let ((oracle_paid, metrics), refused, paid, first) = with_limit_usage_refusal(64 << 20, || measure(|| {
            let scope = OracleScope::open_with_work(1 << 29, 2 * MAX_REPLAY_WORK_UNITS).unwrap();
            let opted = opt_in(scope.lease()).unwrap();
            let mut budget = ReplayBudget::new();
            let result = validate_structure_with_budget(&program, &changed, &mut budget, None);
            assert_eq!(result.as_ref().err(), Some(&legacy), "complete driver keeps every diagnostic field");
            drop(result); drop(budget); drop(opted); let receipt = scope.finish();
            assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none());
            assert_eq!(receipt.started, receipt.completed); receipt.charged
        }));
        assert!(!refused && first.is_none()); cold_complete(metrics, paid.checked_add(oracle_paid).unwrap());
    }
}

// BEGIN OPT949 COMPLETE IMMEDIATE SOURCE INVERSES
#[path = "../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod cold949_reader;
const COLD949_SOURCE_CAP: usize = 9_753_094;
type Cold949Row = (&'static str, &'static str, &'static str, &'static [(usize, usize, &'static str)]);
const COLD949_ROWS: &[Cold949Row] = &[
    ("docs/PAIRED-WEIGHTED-COMPARISON-V1.md", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/PAIRED-WEIGHTED-COMPARISON-V1.md")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/004fa1e7b90c37dae3cc4a4921faefba03d78197e89431836de4e0e98d68be94")), &[(4114,5703,"")]),
    ("docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/b595aa192e83ad59d2842be236ebfbc97413539d0faa67c6c0ad1d4c6590bd09")), &[(25233,25805,"selector. Ordinary builder dispatch prepays its selector/header/CFG/error-probe\nreceivers before testing thresholds, including on tiny nonselecting paths.\nThe existing builder crossover above 128 paths runs first; this fitting selector\n")]),
    ("src/box_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/box_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/c1c21bfdfc4ae4db27239ee21426328f42b3404c42e5a4bca55bf59117f2df79")), &[(10250,10250,"#[cfg(test)]\n")]),
    ("src/byte_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/byte_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/aac7715eed4a017553d09f3543aab57177e8400d9bffc45bfbe78e277fe42f9e")), &[(30590,30590,"#[cfg(test)]\n")]),
    ("src/cleanup_plan/replay/allocation.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/allocation.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/0d9c44752ba86a56eb75d5a77cbe97f60a7152dce52530daf9b5c06066faeda5")), &[(9894,9968,"Extra traversals reached by the independently funded trailing-active probe"),(10207,10207,"#[cfg(test)]\n"),(10227,10312,""),(11630,11647,"")]),
    ("src/cleanup_plan/replay/factored.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/a707ba025d94fa1dfcd81f44b1984ee945c3479e4e0bae6f5f5cda2dde010269")), &[(2993,6427,""),(10251,10331,"        budget.reserve_bytes(\n            function,\n            BUILDER_SELECTOR_CONTROLS,\n            \"ordinary builder representation selector receivers\",\n        )?;\n        selected_builder_paid(\n            function,\n            cfg_paths,\n            semantic_paths,\n            &mut ReplayBudget::new(),\n        )?\n"),(10496,10726,"    // Keep both existing selectors above unchanged. Only the ordinary builder\n    // evaluates this nonselection guard; its two bool roles were prepaid in\n    // BUILDER_SELECTOR_CONTROLS before entering the initial selector.\n"),(59052,59083,"mod moderate_tests;\n")]),
    ("src/cleanup_plan/replay/factored/moderate_tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/moderate_tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/4c32d91e8bc3d5567f19984cb289471e285a0ae14b42a9909ca8dba489aa8224")), &[]),
    ("src/cleanup_plan/replay/factored/paired.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/paired.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/e2d57d474e4467653c6d5ef266a11bf4d5b22f1999dfaf46fd0da5bc19e67c32")), &[(22812,22864,"const FITTING_REFUSAL_PROBE_CONTROLS: usize = {\n"),(23025,23064,"    2 * size_of::<&ReplayBudget>()\n        + 2 * size_of::<bool>()\n        + 2 * size_of::<&LocalKey<Active>>()\n"),(23415,23586,""),(27544,27607,"const BUILDER_FACTORED_PATH_THRESHOLD: usize = 128;\n")]),
    ("src/cleanup_plan/replay/trailing_active/tests.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/trailing_active/tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/12909e042372067ae367d991e6cd159805f1abd671eb155cb1c54983ae5af493")), &[(18984,19336,""),(19644,19772,"        assert_eq!(source_reader::read(&root.join(source),length,sha,2*1024*1024).unwrap(),current);\n"),(19971,20099,"        assert_eq!(source_reader::read(&root.join(source),length,sha,2*1024*1024).unwrap(),current);\n")]),
    ("src/command_io_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/command_io_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/31bee6ec1d18c786de7161c63f2fd85ddea221bf9ca366762d8a332452892239")), &[(43240,43240,"#[cfg(test)]\n")]),
    ("src/environment_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/environment_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/fc20ba1d398a69b13aae41a8057bc3953091d3b45b84702dfdca8c0cdc989af0")), &[(3068,3068,"#[cfg(test)]\n"),(3273,3273,"#[cfg(test)]\n")]),
    ("src/filesystem_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/filesystem_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/30c9508c40a210f5724a643562f48169f1e5a4278def0631c56c7692cc1041af")), &[(9915,9915,"#[cfg(test)]\n"),(10109,10109,"#[cfg(test)]\n"),(10467,10467,"#[cfg(test)]\n")]),
    ("src/hir/ids.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/ids.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/5a7fb6366d3f448c6ab0f30dc99b24a007e3c2805db082f59396d8c0ca4ee0d8")), &[(23656,23656,"#[cfg(test)]\n")]),
    ("src/hir/ids/compiler.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/ids/compiler.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/0e2cb9430e9a6ec9f73e8e1bd07d36fbec84b5299635f2883b26a835cc6796b5")), &[(6098,6098,"#[cfg(test)]\n")]),
    ("src/host_io_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/host_io_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/56f1d29f043877d59cf70f84c7bf17b35e5ee27347383d75b4e9f8348498bf1e")), &[(4417,4417,"#[cfg(test)]\n")]),
    ("src/iterator_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/iterator_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/1b18820c66c5b47e18e6ed7e2c58d4319e5ab50f0fe2c2ea1ee153c16620fa69")), &[(18877,18877,"#[cfg(test)]\n")]),
    ("src/list_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/list_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/a8e692ad890dacbfdb6b412f6cf277a9c22e09888675af9c816347c7444e86ef")), &[(12193,12193,"#[cfg(test)]\n")]),
    ("src/map_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/map_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/65bb7cb33edd6bed0e197243ba25b6cf6f7fb3f929cce6d6cdddee079990c4ba")), &[(17149,17149,"#[cfg(test)]\n")]),
    ("src/network_io_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/network_io_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/b44fb31432a28a6e846fe0bf4b371159d4e750b6a70cd76695fb90dbcd09cd65")), &[(20608,20608,"#[cfg(test)]\n")]),
    ("src/stdin_stream_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/stdin_stream_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/df37f338d309c71d599b4ada290b6fa1f681d018737344c6cb0fc7b9c8104343")), &[(17484,17484,"#[cfg(test)]\n")]),
    ("src/str_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/str_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/f681546ee58598288095926f2c76a25a26665770258eb25dea221e5c38cbbf8a")), &[(5932,5932,"#[cfg(test)]\n")]),
    ("src/vec_ops.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vec_ops.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/c880a46547d9385e12ea75f4577eef5e059fde9a189a17839951be22e278f5f2")), &[(30160,30160,"#[cfg(test)]\n")]),
];
fn cold949_sha(text: &str) -> String { joined_digest(text.as_bytes()) }
fn cold949_read(path: &std::path::Path, len: usize, sha: &str) -> Option<String> {
    cold949_reader::read(path, len, sha, COLD949_SOURCE_CAP).ok()
}
fn cold949_check(
    source: &str, compiled: &str, prior: &str,
    read: &mut impl FnMut(&std::path::Path, usize, &str) -> Option<String>,
) -> Option<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (path, text) in [
        (root.join(source), compiled),
        (root.join("e").join(cold949_sha(compiled)), compiled),
        (root.join("e").join(cold949_sha(prior)), prior),
    ] {
        if text.len() > COLD949_SOURCE_CAP { return None; }
        let actual = read(&path, text.len(), &cold949_sha(text))?;
        if actual != text { return None; }
    }
    Some(())
}
fn cold949_restore(
    source: &str, supplied: &str,
    read: &mut impl FnMut(&std::path::Path, usize, &str) -> Option<String>,
) -> Option<String> {
    let (_, compiled, prior, edits) = COLD949_ROWS.iter().find(|r| r.0 == source)?;
    if supplied != *compiled { return None; }
    cold949_check(source, compiled, prior, read)?;
    let mut restored = if source == "src/cleanup_plan/replay/factored/moderate_tests.rs" {
        let marker = "\n// Complete immediate predecessor. Literal bodies and tariff expression are\n";
        if compiled.matches(marker).count() != 1 { return None; }
        compiled.split_once(marker)?.0.to_owned()
    } else { supplied.to_owned() };
    for &(start, end, before) in edits.iter().rev() {
        if start > end || restored.get(start..end).is_none() { return None; }
        restored.replace_range(start..end, before);
        if restored.len() > COLD949_SOURCE_CAP { return None; }
    }
    if restored != *prior { return None; }
    cold949_check(source, compiled, prior, read)?;
    Some(restored)
}
#[test]
fn cold_builder_threshold_entire_old_caller_selector_tariffs_and_owner_inverses_remain_bound() {
    fn definition<'a>(source: &'a str, marker: &str) -> &'a str {
        let start = source.find(marker).unwrap();
        let end = start + source[start..].find("\n}").unwrap() + 2;
        &source[start..end]
    }
    let current = include_str!("moderate_tests.rs");
    let independent = current.split_once("mod before_cold_threshold {\nuse super::*;\n").unwrap().1
        .split_once("\n}\n\n// The test receiver").unwrap().0;
    let old_factored = COLD949_ROWS.iter().find(|r| r.0 == "src/cleanup_plan/replay/factored.rs").unwrap().2;
    let old_paired = COLD949_ROWS.iter().find(|r| r.0 == "src/cleanup_plan/replay/factored/paired.rs").unwrap().2;
    for (old, marker) in [
        (old_factored, "pub(super) fn validate_with_index_storage<'hir>("),
        (old_paired, "pub(super) fn selected_builder_paid("),
        (old_paired, "pub(super) const BUILDER_SELECTOR_CONTROLS: usize = {"),
        (old_paired, "const FITTING_REFUSAL_PROBE_CONTROLS: usize = {"),
    ] { assert_eq!(definition(independent, marker), definition(old, marker)); }
    assert_eq!(paired::BUILDER_FACTORED_PATH_THRESHOLD, 128);
    assert!(independent.contains("const BUILDER_FACTORED_PATH_THRESHOLD: usize = 128;"));
    assert_eq!(COLD949_ROWS.len(), 22);
    for &(path, compiled, prior, _) in COLD949_ROWS {
        assert_eq!(cold949_restore(path, compiled, &mut cold949_read).as_deref(), Some(prior));
        let half = compiled.char_indices().map(|(i,_)| i).take_while(|i| *i <= compiled.len()/2).last().unwrap_or(0);
        for wrong in [String::new(), compiled[..half].to_owned(), format!("{compiled} "), format!("!{}", &compiled[1..])] {
            assert!(cold949_restore(path, &wrong, &mut |_,_,_| panic!("forged current before acquisition")).is_none());
        }
        if compiled != prior {
            assert!(cold949_restore(path, prior, &mut |_,_,_| panic!("historical source cannot impersonate current")).is_none());
        }
        // Corrupt each actual physical/current-archive/prior-archive frontier,
        // both before and after the entire inverse. No gate or read is skipped.
        for corrupt_at in 0..6 {
            for truncate in [true, false] {
                let mut calls = 0;
                let mut injected = |p: &std::path::Path, len, sha: &str| {
                    let mut text = cold949_read(p, len, sha)?;
                    if calls == corrupt_at {
                        if truncate { text.pop(); } else { text.push('!'); }
                    }
                    calls += 1; Some(text)
                };
                assert!(cold949_restore(path, compiled, &mut injected).is_none());
                assert_eq!(calls, corrupt_at+1);
            }
        }
    }
    assert!(cold949_restore("src/foreign.rs", "foreign", &mut |_,_,_| panic!("unknown source has no read authority")).is_none());
}

// This is an exact historical view for the unchanged trailing-probe consumer,
// never a replacement for its live source/compiled/archive authentication.
pub(in crate::cleanup_plan::replay) fn cold949_previous_input(
    source: &str, supplied: &str, length: usize, sha: &str,
) -> String {
    assert!(supplied.len() <= 2*1024*1024 && length <= 2*1024*1024);
    let previous = cold949_restore(source, supplied, &mut cold949_read)
        .expect("actual current helper and complete immediate predecessor");
    assert_eq!(previous.len(), length);
    assert_eq!(cold949_sha(&previous), sha);
    previous
}
