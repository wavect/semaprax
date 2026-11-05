use super::*;

#[derive(Clone, Copy, Debug)]
enum Receiver {
    Original,
    PaidBefore,
    Current,
}
use crate::bounded_output::{with_limit_usage_refusal, ReservationRefusal};
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let source = include_str!("../../../expression_facts.rs")
        .split_once("r#\"module replay.fact_owner;")
        .unwrap()
        .1
        .split_once("\"#,")
        .unwrap()
        .0;
    crate::hir::resolve(
        &crate::check(
            &format!("module replay.fact_owner;{source}"),
            "exit-prefix-proof.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn receive(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
    current: Receiver,
) -> Result<(), Diagnostic> {
    let statuses = validate_status_sources_retained(function, facts, budget)?;
    match current {
        Receiver::Current => validate_blocks_and_edges_retained(
            program, function, facts, storage, leaves, None, budget, &statuses,
        ),
        Receiver::PaidBefore => paid_before_blocks_and_edges_retained(
            program, function, facts, storage, leaves, None, budget, &statuses,
        ),
        Receiver::Original => exit_before::validate_blocks_and_edges_retained(
            program, function, facts, storage, leaves, None, budget, &statuses,
        ),
    }
}

// HIR/facts are existing immutable inputs. Every receiver allocation, result,
// retained token and budget drops while the actual observer is still active.
fn observe(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    current: Receiver,
    work_limit: usize,
    byte_limit: usize,
    expected: Option<&Result<(), Diagnostic>>,
) -> (bool, usize, usize, bool, Option<ReservationRefusal>, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let ((value, allocation), refused, debit, first) = with_limit_usage_refusal(byte_limit, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.remaining = work_limit;
            assert!(
                budget.pays_allocations(),
                "all3 observed routes require current paid mode"
            );
            let result = receive(
                program,
                function,
                &facts,
                &storage,
                &leaves,
                &mut budget,
                current,
            );
            if let Some(expected) = expected {
                assert_eq!(&result, expected);
            }
            let success = result.is_ok();
            if !success && work_limit < MAX_REPLAY_WORK_UNITS && byte_limit == 1 << 26 {
                assert_eq!(result.as_ref().unwrap_err().code, "SPX-H006");
                assert_eq!(budget.remaining, 0);
            }
            let used = work_limit - budget.remaining;
            drop(result);
            if !success && budget.refusal_spent {
                let remaining = budget.remaining;
                let bytes = crate::bounded_output::active_remaining();
                let retry = receive(
                    program,
                    function,
                    &facts,
                    &storage,
                    &leaves,
                    &mut budget,
                    current,
                );
                assert!(retry.is_err());
                assert_eq!(budget.remaining, remaining);
                assert_eq!(crate::bounded_output::active_remaining(), bytes);
                drop(retry);
            }
            drop(budget);
            (success, used)
        })
    });
    assert!(!allocation.invalid);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
    (
        value.0,
        value.1,
        debit,
        refused,
        first,
        allocation.allocated,
    )
}

fn original(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let mut budget = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut budget,
        )
        .unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    receive(
        program,
        function,
        &facts,
        &storage,
        &leaves,
        &mut budget,
        Receiver::Original,
    )
}

// Independently execute the actual removed AVL operations after an unchanged
// current receiver has bootstrapped the same owner. No numeric gap is fitted.
fn removed_directory_cost(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> (usize, usize, usize) {
    let mut preparation = ReplayBudget::unobserved();
    let facts = Facts::Arena(
        super::super::super::arena::build::<errors::Legacy>(
            function,
            capacity::Payer::Selected(&mut physical_storage::PhysicalReservation::new(usize::MAX)),
            &mut preparation,
        )
        .unwrap(),
    );
    let storage = validate_slots(function).unwrap();
    let leaves = collect_leaves(function).unwrap();
    let ((cost, allocation), refused, _, _) = with_limit_usage_refusal(1 << 26, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            receive(
                program,
                function,
                &facts,
                &storage,
                &leaves,
                &mut budget,
                Receiver::Current,
            )
            .unwrap();
            let work = budget.remaining;
            let bytes = crate::bounded_output::active_remaining().unwrap();
            let heap = crate::test_allocation_observer::current_metrics().allocated;
            let mut removed = Directory::<crate::cleanup_plan::ExitTargetId>::new(
                function,
                &mut budget,
                "cleanup referenced exit directory",
            )
            .unwrap();
            for block in &function.cleanup_plan.blocks {
                if let CleanupTerminator::Exit(exit) = block.terminator {
                    assert!(removed.insert(function, exit, &mut budget).unwrap());
                }
            }
            let cost = (
                work - budget.remaining,
                bytes - crate::bounded_output::active_remaining().unwrap(),
                crate::test_allocation_observer::current_metrics().allocated - heap,
            );
            drop(removed);
            drop(budget);
            cost
        })
    });
    assert!(
        !refused
            && !allocation.invalid
            && allocation.live == 0
            && allocation.allocated == allocation.freed
    );
    cost
}

#[test]
fn complete_exit_receivers_preserve_prior_owner_repeated_and_competing_six_field_errors() {
    let program = program();
    let mut saved = false;
    let mut paid_helper_work_seen = false;
    let mut repeated = false;
    let mut prior = false;
    let mut future = false;
    let mut forged_id_accepted = false;
    for function in &program.functions {
        let expected = original(&program, function);
        assert!(expected.is_ok());
        let old = observe(
            &program,
            function,
            Receiver::Original,
            MAX_REPLAY_WORK_UNITS,
            1 << 26,
            Some(&expected),
        );
        let new = observe(
            &program,
            function,
            Receiver::Current,
            MAX_REPLAY_WORK_UNITS,
            1 << 26,
            Some(&expected),
        );
        let paid = observe(
            &program,
            function,
            Receiver::PaidBefore,
            MAX_REPLAY_WORK_UNITS,
            1 << 26,
            Some(&expected),
        );
        eprintln!("{} original={old:?} paid_before={paid:?} current={new:?}; physical delta={} work delta={} debit delta={}",
            function.id.as_str(), paid.5 as i128 - old.5 as i128,
            paid.1 as i128 - old.1 as i128, paid.2 as i128 - old.2 as i128);
        assert!(old.0 && paid.0 && new.0 && !old.3 && !paid.3 && !new.3);
        assert!(old.4.is_none() && paid.4.is_none() && new.4.is_none());
        paid_helper_work_seen |= paid.1 != old.1;
        let removed = removed_directory_cost(&program, function);
        assert_eq!(paid.1, new.1.checked_add(removed.0).unwrap());
        assert_eq!(paid.2, new.2.checked_add(removed.1).unwrap());
        assert_eq!(paid.5, new.5.checked_add(removed.2).unwrap());
        eprintln!(
            "{} removed_exit_directory={removed:?}",
            function.id.as_str()
        );
        let mut no_exit = function.clone();
        no_exit.cleanup_plan.blocks.clear();
        no_exit.cleanup_plan.edges.clear();
        let expected_empty = original(&program, &no_exit);
        for current in [Receiver::Original, Receiver::PaidBefore, Receiver::Current] {
            let result = observe(
                &program,
                &no_exit,
                current,
                MAX_REPLAY_WORK_UNITS,
                1 << 26,
                Some(&expected_empty),
            );
            assert_eq!(result.0, expected_empty.is_ok());
            assert!(!result.3);
            let zero = observe(&program, &no_exit, current, MAX_REPLAY_WORK_UNITS, 0, None);
            assert!(!zero.0 && zero.3 && zero.4.is_some());
        }
        if function
            .cleanup_plan
            .blocks
            .iter()
            .any(|b| matches!(b.terminator, CleanupTerminator::Exit(_)))
        {
            assert!(new.1 < paid.1 && new.2 < paid.2 && new.5 < paid.5);
            saved = true;
        }
        let mut attacks = Vec::new();
        for (at, block) in function.cleanup_plan.blocks.iter().enumerate() {
            let CleanupTerminator::Exit(exit) = block.terminator else {
                continue;
            };
            // Current declared owner is the ordinary success path; the old
            // receiver deliberately never checks ExitTarget.id at this phase.
            let mut bad = function.clone();
            bad.cleanup_plan.exits[exit.0 as usize].id =
                crate::cleanup_plan::ExitTargetId(u32::MAX);
            assert!(original(&program, &bad).is_ok());
            forged_id_accepted = true;
            attacks.push(bad);
            let mut bad = function.clone();
            bad.cleanup_plan.blocks[at].terminator =
                CleanupTerminator::Exit(crate::cleanup_plan::ExitTargetId(u32::MAX));
            attacks.push(bad);
            let mut bad = function.clone();
            bad.cleanup_plan.exits[exit.0 as usize].from = BlockId(u32::MAX);
            attacks.push(bad);
            if at + 1 < function.cleanup_plan.blocks.len() {
                future = true;
                let mut bad = function.clone();
                bad.cleanup_plan.exits[exit.0 as usize].from =
                    function.cleanup_plan.blocks[at + 1].id;
                attacks.push(bad);
                // A real prior successful Exit(x) means repeat must outrank the
                // later wrong-owner check even though the inventory count fits.
                repeated = true;
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at + 1].terminator = CleanupTerminator::Exit(exit);
                let expected = original(&program, &bad).unwrap_err();
                assert!(expected.message.contains("unknown or repeated exit"));
                attacks.push(bad);
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at + 1].terminator = CleanupTerminator::Exit(exit);
                bad.cleanup_plan.blocks[at + 1].id = block.id;
                attacks.push(bad);
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at].id = BlockId(u32::MAX);
                bad.cleanup_plan.blocks[at + 1].terminator = CleanupTerminator::Exit(exit);
                attacks.push(bad);
            }
            if at > 0 {
                prior = true;
                // Claimed earlier owner with another exit or non-exit term is
                // not a duplicate. Preserve the distinct wrong-owner error.
                let mut bad = function.clone();
                bad.cleanup_plan.exits[exit.0 as usize].from =
                    function.cleanup_plan.blocks[at - 1].id;
                let expected = original(&program, &bad).unwrap_err();
                assert!(expected.message.contains("owned by the wrong block"));
                attacks.push(bad);
            }
            // Bounds selection must precede any malformed declared-owner probe.
            let mut bad = function.clone();
            bad.cleanup_plan.exits[exit.0 as usize].from = BlockId(u32::MAX);
            bad.cleanup_plan.blocks[at].terminator =
                CleanupTerminator::Exit(crate::cleanup_plan::ExitTargetId(u32::MAX));
            attacks.push(bad);
        }
        for hostile in attacks {
            let expected = original(&program, &hostile);
            for current in [Receiver::Original, Receiver::PaidBefore, Receiver::Current] {
                let result = observe(
                    &program,
                    &hostile,
                    current,
                    MAX_REPLAY_WORK_UNITS,
                    1 << 26,
                    Some(&expected),
                );
                assert_eq!(result.0, expected.is_ok());
                assert!(!result.3);
            }
        }
    }
    assert!(saved && repeated && prior && future && forged_id_accepted);
    assert!(
        paid_helper_work_seen,
        "fixtures must execute newly paid place helper work"
    );
}

#[test]
fn actual_exit_receivers_keep_full_work_bytes_exact_short_zero_and_sticky_drop() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| !f.cleanup_plan.exits.is_empty())
        .unwrap();
    for current in [Receiver::Original, Receiver::PaidBefore, Receiver::Current] {
        let full = observe(
            &program,
            function,
            current,
            MAX_REPLAY_WORK_UNITS,
            1 << 26,
            None,
        );
        assert!(full.0 && !full.3 && full.1 > 0 && full.2 > 0);
        for limit in [0, full.1 - 1, full.1] {
            let result = observe(&program, function, current, limit, 1 << 26, None);
            assert_eq!(result.0, limit == full.1);
            assert!(!result.3);
            if result.0 {
                assert_eq!(result.1, full.1);
            }
        }
        for limit in [0, full.2 - 1, full.2] {
            let result = observe(
                &program,
                function,
                current,
                MAX_REPLAY_WORK_UNITS,
                limit,
                None,
            );
            assert_eq!(result.0, limit == full.2);
            assert_eq!(result.3, limit != full.2);
            assert_eq!(result.4.is_some(), limit != full.2);
            if result.0 {
                assert_eq!(result.2, full.2);
            }
        }
    }
    let mut hostile = function.clone();
    let (at, exit) = hostile
        .cleanup_plan
        .blocks
        .iter()
        .enumerate()
        .find_map(|(at, b)| {
            if let CleanupTerminator::Exit(exit) = b.terminator {
                Some((at, exit))
            } else {
                None
            }
        })
        .unwrap();
    hostile.cleanup_plan.exits[exit.0 as usize].from = BlockId(u32::MAX);
    let expected = original(&program, &hostile);
    assert!(expected.is_err());
    assert_eq!(hostile.cleanup_plan.blocks[at].id.0 as usize, at);
    let full = observe(
        &program,
        &hostile,
        Receiver::Current,
        MAX_REPLAY_WORK_UNITS,
        1 << 26,
        Some(&expected),
    );
    // Full malformed route includes every new probe/control payment.
    assert!(
        !full.0
            && !full.3
            && full.1 >= exit_prefix::PROBE_UNITS
            && full.2 >= exit_prefix::CONTROL_BYTES
    );
    let exact_work = observe(
        &program,
        &hostile,
        Receiver::Current,
        full.1,
        1 << 26,
        Some(&expected),
    );
    assert!(!exact_work.0 && !exact_work.3);
    let exact_bytes = observe(
        &program,
        &hostile,
        Receiver::Current,
        MAX_REPLAY_WORK_UNITS,
        full.2,
        Some(&expected),
    );
    assert!(!exact_bytes.0 && !exact_bytes.3 && exact_bytes.4.is_none());
    for limit in [0, full.1 - 1] {
        let result = observe(&program, &hostile, Receiver::Current, limit, 1 << 26, None);
        assert!(!result.0 && !result.3);
    }
    for limit in [0, full.2 - 1] {
        let result = observe(
            &program,
            &hostile,
            Receiver::Current,
            MAX_REPLAY_WORK_UNITS,
            limit,
            None,
        );
        assert!(!result.0 && result.3 && result.4.is_some());
    }
}

#[test]
fn complete_before_exit_receiver_and_full_source_inverses_remain_exact() {
    use sha2::{Digest, Sha256};
    let old = ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/exit-prefix-reference-proof/retained.rs.before.txt"
    ));
    let rows: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/exit-prefix-reference-proof/exact-source-inverses.json"
    )))
    .unwrap();
    let fixed = [
        ("tests/hir_module_boundaries.rs", "03fde58554846398c379bbaa6f60cd1368d2573ff9fc715d340074f4c6ea56dc"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "bb72229bff1d33b7e6f03887a4022003465f077710cd643d75be9afcf4577961"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "7e0364a194df13c8fc374e4dd1ae2464609a8040ea2087429d06843a00f417c7"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "222853f211323dde7b91b7e5ba0dfebf0aeee26a4383174075718c662386bbe7"),
    ];
    let sources = [
        ("tests/hir_module_boundaries.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/tests__hir_module_boundaries.rs.before.txt"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")),
            ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/exit-prefix-reference-proof/retained.rs.before.txt"))),
    ];
    for row in rows.as_array().unwrap() {
        let fixed_hash = fixed
            .iter()
            .find(|(path, _)| *path == row["source"].as_str().unwrap())
            .unwrap()
            .1;
        assert_eq!(row["sha256"].as_str().unwrap(), fixed_hash);
        let (_, current, archive) = sources
            .iter()
            .find(|(path, _, _)| *path == row["source"].as_str().unwrap())
            .unwrap();
        let mut source = (*current).to_owned();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
            ),
            fixed_hash
        );
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(source.matches(after).count(), 1);
            source = source.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(source, *archive);
    }
    let current_tests = ci_str!("exit_tests.rs");
    let copied = current_tests
        .split_once("\n// BEGIN COMPLETE PAID EXIT COMPARISON BRIDGE\n")
        .unwrap()
        .1
        .split_once("// END COMPLETE PAID EXIT COMPARISON BRIDGE\n")
        .unwrap()
        .0;
    let copied = copied.replace(
        "fn paid_before_blocks_and_edges_retained(",
        "fn validate_blocks_and_edges_retained(",
    );
    let mut restored = copied;
    let mut substitutions = 0;
    for (before, after) in [
        (
            "validate_place(function, destination, storage, leaves)",
            "helpers::validate_place(function, destination, storage, leaves, budget)",
        ),
        (
            "validate_place(function, binding, storage, leaves)",
            "helpers::validate_place(function, binding, storage, leaves, budget)",
        ),
        (
            "validate_place(function, source, storage, leaves)",
            "helpers::validate_place(function, source, storage, leaves, budget)",
        ),
        (
            "validate_place(function, &argument.source, storage, leaves)",
            "helpers::validate_place(function, &argument.source, storage, leaves, budget)",
        ),
    ] {
        substitutions += restored.matches(after).count();
        restored = restored.replace(after, before);
    }
    assert_eq!(substitutions, 11);
    let full_before = ci_str!("exit_before.rs");
    let full_before = &full_before[full_before.find("// The retained key proof").unwrap()..];
    assert_eq!(restored, full_before);
    let start = old.find("// The retained key proof").unwrap();
    let end = old.find("\n#[cfg(test)]\nmod tests;").unwrap();
    assert_eq!(ci_str!("exit_before.rs"), format!(
        "//! Complete pre-exit-elimination receiving body, independently compiled.\nuse super::*;\n\n{}\n", &old[start..end]));
}

// BEGIN COMPLETE PAID EXIT COMPARISON BRIDGE
// The retained key proof is the sole additional borrowed argument to the complete receiver.
#[allow(clippy::too_many_arguments)]
pub(in crate::cleanup_plan::replay) fn paid_before_blocks_and_edges_retained(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    facts: &Facts<'_>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
    validated: &StatusSources<'_>,
) -> Result<(), Diagnostic> {
    if !std::ptr::eq(function, validated.function) || !std::ptr::eq(budget, validated.budget) {
        return Err(replay_error(
            function,
            "status directory belongs to another function or budget",
        ));
    }
    if !validated.ledger.current() {
        return Err(budget.budget_refusal(
            function,
            "retained status invocation owner changed",
            "",
        ));
    }
    // Authenticate the unchanged payer even though retaining the directory allocates nothing.
    budget.reserve_bytes(function, 0, "retained cleanup status directory")?;
    budget.charge(function, 0, "retained cleanup status directory")?;
    if let Facts::Tree(tree) = facts {
        return super::super::super::validate_blocks_and_edges(
            program, function, tree, storage, leaves, helpers, budget,
        );
    }
    let expressions: &dyn Lookup = facts;
    let plan = &function.cleanup_plan;
    let statuses = validated
        .ids
        .as_ref()
        .ok_or_else(|| replay_error(function, "retained status directory has no metered keys"))?;
    for (index, edge) in plan.edges.iter().enumerate() {
        if edge.id.0 != u32_index(function, index, "cleanup edge")?
            || !block_exists(function, edge.from)
            || !block_exists(function, edge.to)
        {
            return Err(replay_error(
                function,
                "cleanup edge has a non-contiguous ID or unknown endpoint",
            ));
        }
        validate_edge_condition(function, &edge.condition, expressions, statuses, budget)?;
    }

    let mut referenced_exits =
        Directory::new(function, budget, "cleanup referenced exit directory")?;
    let mut committed_calls = Directory::new(function, budget, "cleanup committed call directory")?;
    for (index, block) in plan.blocks.iter().enumerate() {
        if block.id.0 != u32_index(function, index, "cleanup block")?
            || usize::try_from(block.region.0)
                .ok()
                .is_none_or(|region| region >= plan.regions.len())
        {
            return Err(replay_error(
                function,
                "cleanup block has a non-contiguous ID or unknown region",
            ));
        }
        let mut selected = None;
        for transition in &block.transitions {
            match transition {
                CleanupTransition::Initialize { at, destination } => {
                    require_expression(function, expressions, at, budget)?;
                    helpers::validate_place(function, destination, storage, leaves, budget)?;
                }
                CleanupTransition::ReserveRenewal { at, binding } => {
                    renewal::validate_binding(program, function, at, binding, helpers, budget)?;
                    require_expression(function, expressions, at, budget)?;
                    helpers::validate_place(function, binding, storage, leaves, budget)?;
                }
                CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } => {
                    renewal::validate_binding(program, function, at, destination, helpers, budget)?;
                    require_expression(function, expressions, at, budget)?;
                    helpers::validate_place(function, source, storage, leaves, budget)?;
                    helpers::validate_place(function, destination, storage, leaves, budget)?;
                }
                CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    require_expression(function, expressions, at, budget)?;
                    let source_flags =
                        helpers::validate_place(function, source, storage, leaves, budget)?;
                    let destination_flags =
                        helpers::validate_place(function, destination, storage, leaves, budget)?;
                    let mut source_directory =
                        Directory::new(function, budget, "cleanup transfer source flag directory")?;
                    for flag in source_flags {
                        source_directory.insert(function, flag, budget)?;
                    }
                    let mut destination_directory = Directory::new(
                        function,
                        budget,
                        "cleanup transfer destination flag directory",
                    )?;
                    for flag in destination_flags {
                        destination_directory.insert(function, flag, budget)?;
                    }
                    let source_flags = source_directory;
                    let destination_flags = destination_directory;
                    if source_flags.len() != destination_flags.len()
                        || !source_flags.is_disjoint(&destination_flags, function, budget)?
                    {
                        return Err(replay_error(
                            function,
                            "cleanup transfer has incompatible structural places",
                        ));
                    }
                }
                CleanupTransition::AuthenticateVariantCase {
                    at,
                    source,
                    variant,
                    case,
                } => {
                    require_expression(function, expressions, at, budget)?;
                    helpers::validate_place(function, source, storage, leaves, budget)?;
                    let Some(cases) = program.declarations().variant_cases(variant) else {
                        return Err(replay_error(
                            function,
                            "variant authentication references a non-variant declaration",
                        ));
                    };
                    if !cases.iter().any(|candidate| candidate.id == *case) {
                        return Err(replay_error(
                            function,
                            "variant authentication references a foreign case",
                        ));
                    }
                    let slot = function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == source.storage)
                        .ok_or_else(|| {
                            replay_error(function, "variant authentication source has no slot")
                        })?;
                    if !matches!(
                        &slot.ty,
                        ResolvedType::Nominal { declaration, .. } if declaration == variant
                    ) {
                        return Err(replay_error(
                            function,
                            "variant authentication source type disagrees with its variant",
                        ));
                    }
                }
                CleanupTransition::TransferVariant {
                    at,
                    source,
                    destination,
                    variant,
                } => {
                    require_expression(function, expressions, at, budget)?;
                    let source_flags =
                        helpers::validate_place(function, source, storage, leaves, budget)?;
                    let destination_flags =
                        helpers::validate_place(function, destination, storage, leaves, budget)?;
                    if source_flags.len() != destination_flags.len()
                        || function
                            .cleanup_plan
                            .slots
                            .iter()
                            .filter(|slot| {
                                slot.storage == source.storage
                                    || slot.storage == destination.storage
                            })
                            .any(|slot| {
                                !matches!(
                                    &slot.ty,
                                    ResolvedType::Nominal { declaration, .. }
                                        if declaration == variant
                                )
                            })
                    {
                        return Err(replay_error(
                            function,
                            "variant transfer has incompatible conditional inventories",
                        ));
                    }
                }
                CleanupTransition::InitializeVariant {
                    at,
                    destination,
                    variant,
                } => {
                    require_expression(function, expressions, at, budget)?;
                    helpers::validate_place(function, destination, storage, leaves, budget)?;
                    let slot = function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == destination.storage)
                        .ok_or_else(|| {
                            replay_error(function, "variant initialization has no destination slot")
                        })?;
                    if !destination.projections.is_empty()
                        || !matches!(
                            &slot.ty,
                            ResolvedType::Nominal { declaration, .. }
                                if declaration == variant
                        )
                        || !matches!(
                            &slot.field_liveness_shape,
                            FieldLivenessShape::Variant { declaration, .. }
                                if declaration == variant
                        )
                    {
                        return Err(replay_error(
                            function,
                            "variant initialization destination disagrees with its variant",
                        ));
                    }
                }
                CleanupTransition::CallCommit { call, arguments } => {
                    let Some(Some(fact)) = expressions.fact(function, call, budget)? else {
                        return Err(replay_error(
                            function,
                            "call commit does not name a call expression",
                        ));
                    };
                    // Lexical return domains duplicate postcondition bodies, so one
                    // call legitimately commits once per proof domain; the typed
                    // skeleton comparison counts those per-domain commits exactly.
                    if !committed_calls.insert(function, call, budget)?
                        && plan.lexical_control.is_none()
                    {
                        return Err(replay_error(
                            function,
                            "call has more than one atomic commit",
                        ));
                    }
                    let params = resolved_call::resolved_call_param_view(
                        program,
                        function,
                        find_resolved_expression(function, call).ok_or_else(|| {
                            replay_error(function, "call commit expression is absent")
                        })?,
                        &fact.callee,
                        fact.instance,
                        fact.type_arguments.as_slice(),
                    )
                    .map_err(|_| {
                        replay_error(
                            function,
                            format_args!("call commit has unknown callee `{}`", fact.callee),
                        )
                    })?;
                    if params.len() != fact.arguments.len() {
                        return Err(replay_error(
                            function,
                            "call commit callee signature has inconsistent arity",
                        ));
                    }
                    if !resolved_call::owned_parameter_indices_match(
                        &params,
                        arguments,
                        function,
                        |ty| type_needs_drop(program, function, ty),
                    )? {
                        return Err(replay_error(
                            function,
                            "call commit does not contain every and only owned droppable parameter",
                        ));
                    }
                    let mut previous = None;
                    let mut consumed =
                        Directory::new(function, budget, "cleanup consumed flag directory")?;
                    for argument in arguments {
                        if previous.is_some_and(|value| value >= argument.parameter_index) {
                            return Err(replay_error(
                                function,
                                "call-commit parameters are not in strict signature order",
                            ));
                        }
                        previous = Some(argument.parameter_index);
                        let index = usize::try_from(argument.parameter_index).map_err(|_| {
                            replay_error(function, "call-commit parameter index does not fit usize")
                        })?;
                        let Some(expected_expression) = fact.arguments.get(index) else {
                            return Err(replay_error(
                                function,
                                "call-commit parameter index exceeds call arity",
                            ));
                        };
                        match &argument.source.storage {
                            StorageId::CallArgument {
                                call: storage_call,
                                parameter_index,
                                value_expression,
                            } if storage_call == call
                                && parameter_index == &argument.parameter_index
                                && value_expression == expected_expression
                                && argument.source.projections.is_empty() => {}
                            _ => {
                                return Err(replay_error(
                                    function,
                                    "call commit does not consume its matching whole argument epoch",
                                ));
                            }
                        }
                        for flag in helpers::validate_place(
                            function,
                            &argument.source,
                            storage,
                            leaves,
                            budget,
                        )? {
                            if !consumed.insert(function, flag, budget)? {
                                return Err(replay_error(
                                    function,
                                    "atomic call commit consumes overlapping argument epochs",
                                ));
                            }
                        }
                    }
                }
                CleanupTransition::SelectFailure { source } => {
                    require_status_paid(function, statuses, source, budget)?;
                    if selected.replace(source).is_some() {
                        return Err(replay_error(
                            function,
                            "cleanup block selects more than one failure status",
                        ));
                    }
                }
                CleanupTransition::StageCopyResult { source } => {
                    validate_staged_target(function, source)?;
                    match source {
                        StagedCopyResultSource::Body { expression, .. } => {
                            if expression != &function.body.id {
                                return Err(replay_error(
                                    function,
                                    "body Copy-result stage names another expression",
                                ));
                            }
                        }
                        StagedCopyResultSource::TryResidual {
                            expression,
                            operand,
                            ..
                        }
                        | StagedCopyResultSource::TryOptionNone {
                            expression,
                            operand,
                            ..
                        } => {
                            require_expression(function, expressions, expression, budget)?;
                            require_expression(function, expressions, operand, budget)?;
                        }
                    }
                }
            }
        }
        match &block.terminator {
            CleanupTerminator::Goto(edge) => {
                validate_owned_edge_unique(function, block.id, *edge)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always)
                    && !(matches!(
                        function.cleanup_plan.schema,
                        CLEANUP_PLAN_SCHEMA_V6
                            | CLEANUP_PLAN_SCHEMA_V7
                            | CLEANUP_PLAN_SCHEMA_V8
                            | CLEANUP_PLAN_SCHEMA_V9
                            | CLEANUP_PLAN_SCHEMA_V10
                            | CLEANUP_PLAN_SCHEMA_V11
                            | CLEANUP_PLAN_SCHEMA_V12
                            | CLEANUP_PLAN_SCHEMA_V13
                            | CLEANUP_PLAN_SCHEMA_V15
                            | CLEANUP_PLAN_SCHEMA_V16
                            | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17
                            | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
                            | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
                            | CLEANUP_PLAN_SCHEMA_V14
                    ) && matches!(
                        plan.edges[edge.0 as usize].condition,
                        EdgeCondition::VariantCase { matches: true, .. }
                    ))
                {
                    return Err(replay_error(function, "goto edge is conditional"));
                }
            }
            CleanupTerminator::Branch(edges) => {
                if edges.len() != 2 || edges[0] == edges[1] {
                    return Err(replay_error(
                        function,
                        "cleanup branch must own exactly two distinct edges",
                    ));
                }
                for edge in edges {
                    validate_owned_edge_unique(function, block.id, *edge)?;
                }
                validate_branch_pair(
                    function,
                    &plan.edges[edges[0].0 as usize].condition,
                    &plan.edges[edges[1].0 as usize].condition,
                )?;
            }
            CleanupTerminator::Exit(exit) => {
                if usize::try_from(exit.0)
                    .ok()
                    .is_none_or(|value| value >= plan.exits.len())
                    || !referenced_exits.insert(function, *exit, budget)?
                {
                    return Err(replay_error(
                        function,
                        "cleanup block references an unknown or repeated exit",
                    ));
                }
                if plan.exits[exit.0 as usize].from != block.id {
                    return Err(replay_error(
                        function,
                        "cleanup exit is owned by the wrong block",
                    ));
                }
            }
        }
    }
    let mut expected_calls = Directory::new(function, budget, "cleanup expected call directory")?;
    for (expression, fact) in expressions.entries_paid(function, budget)? {
        if fact.is_some() {
            expected_calls.insert(function, expression, budget)?;
        }
    }
    // Control-flow-dead calls after an abrupt exit are never lowered, so the
    // lexical profile checks commit coverage in the typed skeleton instead.
    if plan.lexical_control.is_none()
        && !committed_calls.same_keys(&expected_calls, function, budget)?
    {
        return Err(replay_error(
            function,
            "cleanup plan does not contain exactly one atomic commit for every call",
        ));
    }
    Ok(())
}

// END COMPLETE PAID EXIT COMPARISON BRIDGE
