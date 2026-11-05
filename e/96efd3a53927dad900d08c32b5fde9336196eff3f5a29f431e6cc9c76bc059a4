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
            "edge-ownership-proof.spx",
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
        Receiver::Original => edge_before::validate_blocks_and_edges_retained(
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

// Actual removed insertions in original terminator order, after the same receiver bootstrap.
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
            let mut edges = Directory::<EdgeId>::new(
                function,
                &mut budget,
                "cleanup referenced edge directory",
            )
            .unwrap();
            let mut exits = Directory::<crate::cleanup_plan::ExitTargetId>::new(
                function,
                &mut budget,
                "cleanup referenced exit directory",
            )
            .unwrap();
            for block in &function.cleanup_plan.blocks {
                match &block.terminator {
                    CleanupTerminator::Goto(edge) => {
                        assert!(edges.insert(function, *edge, &mut budget).unwrap());
                    }
                    CleanupTerminator::Branch(branch) => {
                        for edge in branch {
                            assert!(edges.insert(function, *edge, &mut budget).unwrap());
                        }
                    }
                    CleanupTerminator::Exit(exit) => {
                        assert!(exits.insert(function, *exit, &mut budget).unwrap());
                    }
                }
            }
            let cost = (
                work - budget.remaining,
                bytes - crate::bounded_output::active_remaining().unwrap(),
                crate::test_allocation_observer::current_metrics().allocated - heap,
            );
            drop(edges);
            drop(exits);
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
fn complete_edge_receivers_preserve_full_diagnostics_order_and_drop() {
    let program = program();
    let mut saved = false;
    let mut paid_helper_work_seen = false;
    let mut branch_attacked = false;
    let mut foreign_reference_attacked = false;
    let mut foreign_block_attacked = false;
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
        eprintln!(
            "{} removed_edge_and_exit_directories={removed:?}",
            function.id.as_str()
        );
        assert_eq!(paid.1, new.1.checked_add(removed.0).unwrap());
        assert_eq!(paid.2, new.2.checked_add(removed.1).unwrap());
        assert_eq!(paid.5, new.5.checked_add(removed.2).unwrap());
        if !function.cleanup_plan.edges.is_empty() {
            assert!(new.1 < paid.1 && new.2 < paid.2 && new.5 < paid.5);
            saved = true;
        }
        let mut attacks = Vec::new();
        if !function.cleanup_plan.edges.is_empty() {
            let mut bad = function.clone();
            bad.cleanup_plan.edges[0].id = EdgeId(u32::MAX);
            attacks.push(bad);
            let mut bad = function.clone();
            bad.cleanup_plan.edges[0].from = BlockId(u32::MAX);
            attacks.push(bad);
            if let Some(other) = function
                .cleanup_plan
                .blocks
                .iter()
                .find(|b| b.id != function.cleanup_plan.edges[0].from)
            {
                let mut bad = function.clone();
                bad.cleanup_plan.edges[0].from = other.id;
                attacks.push(bad);
            }
            let mut bad = function.clone();
            bad.cleanup_plan.edges[0].to = BlockId(u32::MAX);
            attacks.push(bad);
            // Two independent faults retain the first complete original error.
            let mut bad = function.clone();
            bad.cleanup_plan.edges[0].id = EdgeId(u32::MAX);
            bad.cleanup_plan.blocks[0].id = BlockId(u32::MAX);
            attacks.push(bad);
            if let Some((at, _)) = function
                .cleanup_plan
                .blocks
                .iter()
                .enumerate()
                .find(|(_, b)| !matches!(b.terminator, CleanupTerminator::Exit(_)))
            {
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at].terminator = CleanupTerminator::Goto(EdgeId(u32::MAX));
                attacks.push(bad);
                // Equal edge inventory count; borrow another existing block's edge.
                if let Some(edge) = function
                    .cleanup_plan
                    .edges
                    .iter()
                    .find(|e| e.from != function.cleanup_plan.blocks[at].id)
                {
                    foreign_reference_attacked = true;
                    let mut bad = function.clone();
                    bad.cleanup_plan.blocks[at].terminator = CleanupTerminator::Goto(edge.id);
                    attacks.push(bad);
                }
            }
        }
        for (at, block) in function.cleanup_plan.blocks.iter().enumerate() {
            if let CleanupTerminator::Branch(edges) = &block.terminator {
                branch_attacked = true;
                assert_eq!(edges.len(), 2);
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at].terminator =
                    CleanupTerminator::Branch(vec![edges[0], edges[0]]);
                attacks.push(bad);
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at].terminator = CleanupTerminator::Branch(vec![edges[0]]);
                attacks.push(bad);
            }
            if at > 0 {
                foreign_block_attacked = true;
                let mut bad = function.clone();
                bad.cleanup_plan.blocks[at].id = function.cleanup_plan.blocks[at - 1].id;
                attacks.push(bad);
            }
        }
        for hostile in attacks {
            let expected = original(&program, &hostile);
            assert!(expected.is_err());
            for current in [Receiver::Original, Receiver::PaidBefore, Receiver::Current] {
                let result = observe(
                    &program,
                    &hostile,
                    current,
                    MAX_REPLAY_WORK_UNITS,
                    1 << 26,
                    Some(&expected),
                );
                assert!(!result.0 && !result.3);
            }
        }
    }
    assert!(saved && branch_attacked && foreign_reference_attacked && foreign_block_attacked);
    assert!(
        paid_helper_work_seen,
        "fixtures must execute newly paid place helper work"
    );
}

#[test]
fn actual_edge_receivers_keep_own_exact_short_zero_work_and_bytes() {
    let program = program();
    let function = program
        .functions
        .iter()
        .find(|f| !f.cleanup_plan.edges.is_empty())
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
}

#[test]
fn complete_before_edge_receiver_and_current_full_source_remain_exact() {
    use sha2::{Digest, Sha256};
    let old = ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/referenced-edge-ownership-proof/retained.rs.before.txt"
    ));
    let rows: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/referenced-edge-ownership-proof/exact-source-inverses.json"
    )))
    .unwrap();
    let fixed = [
        ("tests/hir_module_boundaries.rs", "5be1edee738ee12b426939fbfbc09635229d4c7a3da4f45d3c1074254e0eb0e2"),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "844628e0b1a3f3dafd563a7a9ef3396f513e664dc63e7d70237912736d6558ea"),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "52fd9fb9d542ec3916b7427610885d934f7a9e0e9987e515feea226a0bbabfdb"),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "02901e6e9fb8922ee4be72dab3899cbafaba25b942a9f62daf0979ea1fb8556d"),
    ];
    let complete_sources = [
        ("tests/hir_module_boundaries.rs", "tests__hir_module_boundaries.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/tests__hir_module_boundaries.rs.before.txt"))),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", "crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/crates__semaprax-native-rust-interop-builder__src__implementation__tests__ledger_capacity__source_inventory.rs.before.txt"))),
        ("src/cleanup_plan/replay/live_oracle/source_custody/b.rs", "src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/live_oracle/source_custody/b.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/src__cleanup_plan__replay__live_oracle__source_custody__b.rs.before.txt"))),
        ("src/cleanup_plan/replay/expression_facts/receiving/retained.rs", "retained.rs.before.txt", ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/expression_facts/receiving/retained.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/referenced-edge-ownership-proof/retained.rs.before.txt"))),
    ];
    assert_eq!(rows.as_array().unwrap().len(), complete_sources.len());
    for row in rows.as_array().unwrap() {
        let fixed_hash = fixed
            .iter()
            .find(|(path, _)| *path == row["source"].as_str().unwrap())
            .unwrap()
            .1;
        assert_eq!(row["sha256"].as_str().unwrap(), fixed_hash);
        let (_, archive_name, current, archive) = complete_sources
            .iter()
            .find(|(source, _, _, _)| *source == row["source"].as_str().unwrap())
            .unwrap();
        assert_eq!(row["archive"].as_str().unwrap(), *archive_name);
        let mut source = (*current).to_owned();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(archive.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
        for edit in row["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(source.matches(after).count(), 1);
            source = source.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(source, *archive);
    }
    let current_tests = ci_str!("edge_tests.rs");
    let copied = current_tests
        .split_once("\n// BEGIN COMPLETE PAID EDGE COMPARISON BRIDGE\n")
        .unwrap()
        .1
        .split_once("// END COMPLETE PAID EDGE COMPARISON BRIDGE\n")
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
    let full_before = ci_str!("edge_before.rs");
    let full_before = &full_before[full_before.find("// The retained key proof").unwrap()..];
    assert_eq!(restored, full_before);
    let start = old
        .find("// The retained key proof is the sole additional borrowed argument")
        .unwrap();
    let end = old.find("\n#[cfg(test)]\nmod tests;").unwrap();
    assert_eq!(ci_str!("edge_before.rs"), format!(
        "//! Complete pre-elimination receiving body, independently compiled.\nuse super::*;\n\n{}\n", &old[start..end]));
}

// BEGIN COMPLETE PAID EDGE COMPARISON BRIDGE
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

    let mut referenced_edges =
        Directory::new(function, budget, "cleanup referenced edge directory")?;
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
                validate_owned_edge_paid(function, block.id, *edge, &mut referenced_edges, budget)?;
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
                    validate_owned_edge_paid(
                        function,
                        block.id,
                        *edge,
                        &mut referenced_edges,
                        budget,
                    )?;
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

// END COMPLETE PAID EDGE COMPARISON BRIDGE
