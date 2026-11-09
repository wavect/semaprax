use super::*;
use crate::cleanup_plan::replay::expression_facts::receiving::Before941StatusBacking;
use crate::cleanup_plan::replay::fact_preflight::before_941_validate_program_context;
use crate::test_allocation_observer::measure;

fn fixture() -> ResolvedProgram {
    let mut source = String::from("module held.statuses; ");
    for index in 0..12 {
        source.push_str(&format!(
            "@id(\"large{index}\") fn large{index}(x:i64)->i64 {{"
        ));
        for local in 0..128 {
            let input = if local == 0 {
                "x".to_owned()
            } else {
                format!("value{}", local - 1)
            };
            source.push_str(&format!("let value{local}={input}+1;"));
        }
        source.push_str("value127} ");
        source.push_str(&format!(
            "@id(\"small{index}\") fn small{index}(x:i64)->i64 {{x+1}} "
        ));
    }
    // Preserve all twenty-four authored workloads and add an admitted entry.
    source.push_str("@id(\"held.main\") fn main()->i64 {0}");
    let program =
        crate::hir::resolve(&crate::check(&source, "held-statuses.spx").unwrap()).unwrap();
    assert_eq!(program.functions.len(), 25);
    for index in 0..12 {
        assert_eq!(
            program.functions[2 * index]
                .cleanup_plan
                .status_sources
                .len(),
            128
        );
        assert_eq!(
            program.functions[2 * index + 1]
                .cleanup_plan
                .status_sources
                .len(),
            1
        );
    }
    assert!(program.functions[24].cleanup_plan.status_sources.is_empty());
    program
}
fn recycling_fixture() -> ResolvedProgram {
    let mut source = String::from("module held.recycle; @id(\"large\") fn large(x:i64)->i64 {");
    for local in 0..256 {
        let input = if local == 0 {
            "x".to_owned()
        } else {
            format!("value{}", local - 1)
        };
        source.push_str(&format!("let value{local}={input}+1;"));
    }
    source.push_str(
        "value255} @id(\"small\") fn small(x:i64)->i64{x+1} @id(\"main\") fn main()->i64{0}",
    );
    let program = crate::hir::resolve(&crate::check(&source, "held-recycle.spx").unwrap()).unwrap();
    assert_eq!(program.functions[0].cleanup_plan.status_sources.len(), 256);
    assert_eq!(program.functions[1].cleanup_plan.status_sources.len(), 1);
    assert!(program.functions[2].cleanup_plan.status_sources.is_empty());
    program
}
fn call(program: &ResolvedProgram, before: bool) -> Result<(), Diagnostic> {
    if before {
        before_941_validate_program_context(program)
    } else {
        crate::cleanup_plan::replay::validate_program_context(program)
    }
}
fn run(program: &ResolvedProgram, before: bool, cap: usize, floor: usize, success: bool) -> usize {
    let (((code, metrics), exhausted, paid, receipt), started, _) =
        crate::cleanup_plan::with_financed_validation_test_oracles(
            2 * (program.functions.len() + program.function_instances.len()),
            || {
                let observed = crate::bounded_output::with_limit_usage_refusal(cap, || {
                    assert!(crate::bounded_output::set_active_floor(floor));
                    let measured = measure(|| {
                        let outcome = call(program, before);
                        assert_eq!(
                            outcome.is_ok(),
                            success,
                            "before={before} cap={cap} floor={floor}: {outcome:?}"
                        );
                        let code = outcome.as_ref().err().map(|error| error.code);
                        drop(outcome);
                        code
                    });
                    let ledger = ClassifierLedger::capture();
                    if ledger.first_refusal().is_some() {
                        let first = ledger.first_refusal();
                        let remaining = crate::bounded_output::active_remaining();
                        assert!(!crate::bounded_output::reserve_active_required(0));
                        assert_eq!(ledger.first_refusal(), first);
                        assert_eq!(crate::bounded_output::active_remaining(), remaining);
                    }
                    crate::bounded_output::clear_active_floor();
                    measured
                });
                (observed, success)
            },
        );
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    assert!(metrics.allocated <= paid);
    assert_eq!(exhausted, !success);
    assert_eq!(receipt.is_some(), !success);
    assert_eq!(code, if success { None } else { Some("SPX-H006") });
    if success {
        assert!(started > 0);
    }
    paid
}
fn invoice(program: &ResolvedProgram) -> (usize, usize) {
    let before = run(program, true, usize::MAX, 0, true);
    let current = run(program, false, usize::MAX, 0, true);
    let count = program.functions.len() + program.function_instances.len();
    // Every original body runs. Only the physically different owner and
    // producer/receiver transport banks can change the complete paid invoice.
    assert_eq!(
        before + Backing::ACQUISITION_BYTES + count * Backing::PER_FUNCTION_BYTES,
        current
            + Before941StatusBacking::ACQUISITION_BYTES
            + count * Before941StatusBacking::PER_FUNCTION_BYTES,
    );
    for (old, exact) in [(true, before), (false, current)] {
        assert_eq!(run(program, old, exact, 0, true), exact);
        run(program, old, exact - 1, 0, false);
        run(program, old, exact, 1, false);
        run(program, old, 0, 0, false);
    }
    (before, current)
}
#[test]
fn complete_immediate_caller_typed_invoice_exact_short_zero_floor_sticky_and_drop() {
    let (before, current) = invoice(&fixture());
    assert!(
        current < before,
        "many physical receiver eliminations must repay the complete singleton cold owner"
    );
}
#[test]
fn cold_single_function_keeps_its_complete_owner_terminal_and_drop_bill() {
    let program = crate::hir::resolve(
        &crate::check(
            "module held.cold; @id(\"main\") fn main()->i64 {1}",
            "held-cold.spx",
        )
        .unwrap(),
    )
    .unwrap();
    invoice(&program);
}
#[test]
fn late_duplicate_status_has_all_six_original_diagnostic_fields() {
    let mut program = fixture();
    let sources = &mut program.functions[2].cleanup_plan.status_sources;
    assert!(sources.len() >= 2);
    sources[1].id = sources[0].id.clone();
    let malformed = |before| {
        let ((error, exhausted, _), started, _) =
            crate::cleanup_plan::with_financed_validation_test_oracles(
                2 * (program.functions.len() + program.function_instances.len()),
                || {
                    (
                        crate::bounded_output::with_limit_usage(usize::MAX, || {
                            call(&program, before).unwrap_err()
                        }),
                        false,
                    )
                },
            );
        assert!(!exhausted);
        // The two earlier clean functions completed both complete receivers;
        // an earlier financing/builder refusal cannot impersonate this error.
        assert!(started >= 4);
        assert!(error
            .message
            .contains("cleanup status sources do not exactly cover typed HIR failure producers"));
        error
    };
    let original = malformed(true);
    let current = malformed(false);
    assert_eq!(current, original);
}
fn acquire<'hir>(program: &'hir ResolvedProgram, budget: &mut ReplayBudget) -> Backing<'hir> {
    let first = &program.functions[0];
    budget.prepare_builder(first).unwrap();
    budget
        .reserve_bytes(
            first,
            Backing::ACQUISITION_BYTES,
            "held status test acquisition",
        )
        .unwrap();
    Backing::acquire(program, first, budget).unwrap()
}
#[test]
fn every_rebuilt_directory_retains_full_keys_and_recycles_only_empty_unindexed_backing() {
    let program = recycling_fixture();
    let (_, exhausted, _, first) =
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            let mut owner = acquire(&program, &mut budget);
            let mut previous_capacity = 0;
            let mut saw_frozen_index = false;
            for function in &program.functions {
                let mut received = None;
                assert!(owner.directory_into(&program, function, &mut budget, &mut received));
                let mut directory = received.take().unwrap();
                assert!(
                    directory.nodes.is_empty()
                        && directory.root == NIL
                        && directory.first == NIL
                        && directory.lookup.is_none()
                );
                assert!(directory.nodes.capacity() >= previous_capacity);
                {
                    let mut prefix = directory.held_prefix(function, &mut budget).unwrap();
                    for source in &function.cleanup_plan.status_sources {
                        assert!(prefix.insert(function, &source.id, &mut budget).unwrap());
                    }
                    prefix.freeze(function, &mut budget).unwrap();
                }
                assert_eq!(
                    directory.lookup.is_some(),
                    function.cleanup_plan.status_sources.len() >= 256
                );
                saw_frozen_index |= directory.lookup.is_some();
                let mut original = Directory::new(
                    function,
                    &mut budget,
                    "independent original status directory",
                )
                .unwrap();
                for source in &function.cleanup_plan.status_sources {
                    assert!(original.insert(function, &source.id, &mut budget).unwrap());
                }
                assert!(directory.balanced() && original.balanced());
                assert!(directory.iter().eq(original.iter()));
                previous_capacity = directory.nodes.capacity();
                drop(original);
                owner.recycle(directory);
                assert!(owner.empty.as_ref().unwrap().is_empty());
                assert_eq!(owner.empty.as_ref().unwrap().capacity(), previous_capacity);
                assert!(owner.complete_into(&program, function, &mut budget));
            }
            assert!(
                saw_frozen_index,
                "the complete indexed producer must actually be exercised"
            );
            assert_eq!(owner.next_function, program.functions.len());
            assert!(owner.error.is_none() && !owner.terminal_taken);
        });
    assert!(!exhausted && first.is_none());
}
#[test]
fn occupied_output_and_retries_preserve_first_error_existing_keys_and_terminal_ordinal() {
    let program = fixture();
    crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let mut owner = acquire(&program, &mut budget);
        let function = &program.functions[0];
        let mut occupied =
            Directory::new(function, &mut budget, "preoccupied test output").unwrap();
        let key = &function.cleanup_plan.status_sources[0].id;
        assert!(occupied.insert(function, key, &mut budget).unwrap());
        let pointer = occupied.nodes.as_ptr();
        let mut received = Some(occupied);
        assert!(!owner.directory_into(&program, function, &mut budget, &mut received));
        let original_error = owner.error.as_ref().unwrap().clone();
        let first = owner.ledger.first_refusal();
        let remaining = crate::bounded_output::active_remaining();
        assert!(!owner.directory_into(&program, function, &mut budget, &mut received));
        assert!(!owner.complete_into(&program, function, &mut budget));
        assert_eq!(owner.error.as_ref(), Some(&original_error));
        assert_eq!(owner.ledger.first_refusal(), first);
        assert_eq!(crate::bounded_output::active_remaining(), remaining);
        assert_eq!(owner.next_function, 0);
        let directory = received.as_ref().unwrap();
        assert_eq!(directory.nodes.as_ptr(), pointer);
        assert_eq!(directory.len(), 1);
        assert!(directory.iter().copied().eq(std::iter::once(key)));
        assert_eq!(owner.take_error(), original_error);
        assert!(owner.terminal_taken && owner.error.is_none());
        assert!(!owner.complete_into(&program, &program.functions[1], &mut budget));
        assert!(!owner.directory_into(&program, &program.functions[1], &mut budget, &mut received));
        assert_eq!(owner.next_function, 0);
        assert_eq!(owner.ledger.first_refusal(), first);
        assert_eq!(crate::bounded_output::active_remaining(), remaining);
        assert_eq!(received.as_ref().unwrap().nodes.as_ptr(), pointer);
    });
}
#[test]
fn foreign_program_budget_and_function_order_never_consume_backing_or_replace_error() {
    let program = fixture();
    let other = fixture();
    for foreign in 0..3 {
        let (_, exhausted, _, receipt) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                let mut owner = acquire(&program, &mut budget);
                let first = &program.functions[0];
                let mut received = None;
                assert!(owner.directory_into(&program, first, &mut budget, &mut received));
                let mut directory = received.take().unwrap();
                for source in &first.cleanup_plan.status_sources {
                    assert!(directory.insert(first, &source.id, &mut budget).unwrap());
                }
                owner.recycle(directory);
                assert!(owner.complete_into(&program, first, &mut budget));
                let next = &program.functions[1];
                let pointer = owner.empty.as_ref().unwrap().as_ptr();
                let capacity = owner.empty.as_ref().unwrap().capacity();
                let mut foreign_budget = ReplayBudget::new();
                let accepted = match foreign {
                    0 => owner.directory_into(&other, next, &mut budget, &mut received),
                    1 => owner.directory_into(&program, next, &mut foreign_budget, &mut received),
                    _ => owner.directory_into(
                        &program,
                        &program.functions[2],
                        &mut budget,
                        &mut received,
                    ),
                };
                assert!(!accepted && received.is_none());
                let error = owner.error.as_ref().unwrap().clone();
                let held = owner.ledger.first_refusal();
                let (accepted, foreign_exhausted, used) =
                    crate::bounded_output::with_limit_usage(1 << 20, || {
                        owner.directory_into(&program, next, &mut budget, &mut received)
                    });
                assert!(!accepted && !foreign_exhausted && used == 0);
                assert_eq!(owner.error.as_ref(), Some(&error));
                assert_eq!(owner.ledger.first_refusal(), held);
                assert_eq!(owner.next_function, 1);
                assert!(owner.empty.as_ref().unwrap().is_empty());
                assert_eq!(owner.empty.as_ref().unwrap().as_ptr(), pointer);
                assert_eq!(owner.empty.as_ref().unwrap().capacity(), capacity);
            });
        assert!(exhausted && receipt.is_some());
    }
}

struct Callback<'a> {
    program: &'a ResolvedProgram,
    witness: &'a ClassifierLedger,
    fired: std::cell::Cell<bool>,
}
impl crate::cleanup::ReplayContext for Callback<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        &self.program.declarations
    }
    fn types(&self) -> crate::hir::items::View<'_, crate::hir::ResolvedTypeDeclaration> {
        crate::hir::items::View::slice(&self.program.types)
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        &self.program.interfaces
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        crate::hir::items::View::slice(&self.program.functions)
    }
    fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
        &self.program.function_templates
    }
    fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
        &self.program.function_instances
    }
    fn full_program(&self) -> Option<&ResolvedProgram> {
        assert!(
            !self.fired.replace(true),
            "one actual complete-source callback"
        );
        let (same, exhausted, used) =
            crate::bounded_output::with_limit_usage(1 << 20, || self.witness.current());
        assert!(!same && !exhausted && used == 0);
        Some(self.program)
    }
}
#[test]
fn callback_and_foreign_active_scope_refuse_before_new_directory_payment() {
    let program = fixture();
    for callback in [false, true] {
        let (_, exhausted, _, receipt) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                let first = &program.functions[0];
                // The additional callback witness is test setup, with the same
                // complete capture/current controls as the retained original test.
                budget.prepare_builder(first).unwrap();
                budget
                    .reserve_bytes(
                        first,
                        4 * size_of::<ClassifierLedger>()
                            + ClassifierLedger::capture_into_controls()
                            + ClassifierLedger::current_receiving_controls(),
                        "held callback test complete witness",
                    )
                    .unwrap();
                let mut owner = acquire(&program, &mut budget);
                let witness = ClassifierLedger::capture();
                let mut received = None;
                if callback {
                    let context = Callback {
                        program: &program,
                        witness: &witness,
                        fired: std::cell::Cell::new(false),
                    };
                    assert!(!owner.directory_into(&context, first, &mut budget, &mut received));
                    assert!(context.fired.get());
                } else {
                    let (accepted, foreign_exhausted, used) =
                        crate::bounded_output::with_limit_usage(1 << 20, || {
                            owner.directory_into(&program, first, &mut budget, &mut received)
                        });
                    assert!(!accepted && !foreign_exhausted && used == 0);
                }
                assert!(received.is_none() && owner.empty.is_none() && owner.next_function == 0);
                let error = owner.error.as_ref().unwrap().clone();
                let held = owner.ledger.first_refusal();
                let remaining = crate::bounded_output::active_remaining();
                assert!(!owner.directory_into(&program, first, &mut budget, &mut received));
                assert_eq!(owner.error.as_ref(), Some(&error));
                assert_eq!(owner.ledger.first_refusal(), held);
                assert_eq!(crate::bounded_output::active_remaining(), remaining);
                assert_eq!(owner.take_error(), error);
                // The terminal flag is checked before even invoking full_program.
                let context = Callback {
                    program: &program,
                    witness: &witness,
                    fired: std::cell::Cell::new(false),
                };
                assert!(!owner.directory_into(&context, first, &mut budget, &mut received));
                assert!(!context.fired.get());
                assert!(owner.terminal_taken && owner.error.is_none() && received.is_none());
            });
        assert!(exhausted && receipt.is_some());
    }
}
#[test]
fn constructor_exact_short_floor_zero_and_sticky_keep_unpublished_backing() {
    let program = fixture();
    let first = &program.functions[0];
    let observe = |cap, floor| {
        crate::bounded_output::with_limit_usage_refusal(cap, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            let measured = measure(|| {
                let mut budget = ReplayBudget::new();
                let prepared = budget.prepare_builder(first).and_then(|()| {
                    budget.reserve_bytes(
                        first,
                        Backing::ACQUISITION_BYTES,
                        "held exact constructor acquisition",
                    )?;
                    Backing::acquire(&program, first, &mut budget)
                });
                let mut success = false;
                if let Ok(mut owner) = prepared {
                    let mut received = None;
                    success = owner.directory_into(&program, first, &mut budget, &mut received);
                    if !success {
                        assert!(
                            received.is_none() && owner.empty.is_none() && owner.next_function == 0
                        );
                        let receipt = owner.ledger.first_refusal();
                        let remaining = crate::bounded_output::active_remaining();
                        assert!(!owner.directory_into(&program, first, &mut budget, &mut received));
                        assert_eq!(owner.ledger.first_refusal(), receipt);
                        assert_eq!(crate::bounded_output::active_remaining(), remaining);
                        assert_eq!(owner.take_error().code, "SPX-H006");
                        assert!(owner.terminal_taken && owner.error.is_none());
                    }
                    drop(received);
                    drop(owner);
                }
                success
            });
            crate::bounded_output::clear_active_floor();
            measured
        })
    };
    let ((success, metrics), exhausted, exact, receipt) = observe(usize::MAX, 0);
    assert!(success && !exhausted && receipt.is_none());
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    for (cap, floor, success) in [
        (exact, 0, true),
        (exact - 1, 0, false),
        (exact, 1, false),
        (0, 0, false),
    ] {
        let ((actual, metrics), exhausted, paid, receipt) = observe(cap, floor);
        assert_eq!(actual, success);
        assert_eq!(exhausted, !success);
        assert_eq!(receipt.is_some(), !success);
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        assert!(metrics.allocated <= paid);
    }
}
#[test]
fn authentication_error_and_occupied_constructor_output_drop_during_unwind() {
    let program = fixture();
    let first = &program.functions[0];
    for occupied in [false, true] {
        let (((), metrics), _, paid, _) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        let mut budget = ReplayBudget::new();
                        let mut owner = acquire(&program, &mut budget);
                        let mut received = None;
                        if occupied {
                            assert!(owner.directory_into(
                                &program,
                                first,
                                &mut budget,
                                &mut received
                            ));
                            let directory = received.as_mut().unwrap();
                            for source in &first.cleanup_plan.status_sources {
                                assert!(directory.insert(first, &source.id, &mut budget).unwrap());
                            }
                            assert!(!owner.directory_into(
                                &program,
                                first,
                                &mut budget,
                                &mut received
                            ));
                        } else {
                            let mut foreign_budget = ReplayBudget::new();
                            assert!(!owner.directory_into(
                                &program,
                                first,
                                &mut foreign_budget,
                                &mut received
                            ));
                        }
                        assert!(owner.error.is_some());
                        // Resume an already-constructed zero-sized panic payload:
                        // no reporting allocator can enter the compiler window.
                        std::panic::resume_unwind(Box::new(()));
                    }));
                    assert!(caught.is_err());
                    drop(caught);
                })
            });
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        assert!(metrics.allocated <= paid);
    }
}

// Existing bounded, descriptor-first, exact-tail/digest/UTF8 reader; the new
// custody check does not introduce another physical source acquisition route.
#[path = "../../../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod held_source_reader;

const HELD_SOURCE_IMAGES: &[(&str, &str)] = &[
    (
        "src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation.rs",
        include_str!("held_invocation.rs"),
    ),
    (
        "src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation_tests.rs",
        include_str!("held_invocation_tests.rs"),
    ),
    (
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs",
        include_str!("../directory.rs"),
    ),
    (
        "src/cleanup_plan/replay/expression_facts/receiving/retained/held_status_invocation.rs",
        include_str!("../retained/held_status_invocation.rs"),
    ),
    (
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs",
        include_str!("../retained.rs"),
    ),
    (
        "src/cleanup_plan/replay/expression_facts/receiving.rs",
        include_str!("../../receiving.rs"),
    ),
    (
        "src/cleanup_plan/replay/fact_preflight/before_status_receivers.rs",
        include_str!("../../../fact_preflight/before_status_receivers.rs"),
    ),
    (
        "src/cleanup_plan/replay/fact_preflight.rs",
        include_str!("../../../fact_preflight.rs"),
    ),
];
const HELD_PRIOR_IMAGES: &[(&str, &str, &str)] = &[
    (
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs",
        "e/6036a1ba83d5532c94ccc630c88127e38cedafdeec21345841e5a32a8602711e",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/6036a1ba83d5532c94ccc630c88127e38cedafdeec21345841e5a32a8602711e"
        )),
    ),
    (
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs",
        "e/769cfe4612da4c081d0b1c85e33951297609a1d23d6875ce76736eb7ab88c558",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/769cfe4612da4c081d0b1c85e33951297609a1d23d6875ce76736eb7ab88c558"
        )),
    ),
    (
        "src/cleanup_plan/replay/expression_facts/receiving.rs",
        "e/6e30d8c90477a9ebcb0527678b43d0a49e1b8e441c6804f2f573a445c507e4b1",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/6e30d8c90477a9ebcb0527678b43d0a49e1b8e441c6804f2f573a445c507e4b1"
        )),
    ),
    (
        "src/cleanup_plan/replay/fact_preflight.rs",
        "e/46be8582ccbfb6399b36dcf5e5aecc2c695c5c6794df60434aa922f01eaa08e5",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/e/46be8582ccbfb6399b36dcf5e5aecc2c695c5c6794df60434aa922f01eaa08e5"
        )),
    ),
];
const HELD_ORIGINAL_PRODUCERS: &[(&str, &str)] = &[
    (
        "src/cleanup_plan/replay/expression_facts/receiving/directory/invocation.rs",
        include_str!("invocation.rs"),
    ),
    (
        "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_status_invocation.rs",
        include_str!("../retained/ordinary_status_invocation.rs"),
    ),
];
const HELD_PRIOR_TESTS: (&str, &str) = (
    "e/5e6d75ae92109612b22449bbb8d81fae8b2caebed98e3c888fe994897e19c11f",
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/5e6d75ae92109612b22449bbb8d81fae8b2caebed98e3c888fe994897e19c11f"
    )),
);

fn held_source_custody(supplied: &[(&str, &str)]) -> Option<Vec<Vec<u8>>> {
    use sha2::{Digest, Sha256};
    const CAP: usize = 1 << 20;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let hash = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let read = |path: &str, compiled: &str| {
        held_source_reader::read(
            &root.join(path),
            compiled.len(),
            &hash(compiled.as_bytes()),
            CAP,
        )
        .ok()
        .filter(|physical| physical == compiled)
    };
    let bound = |row: &serde_json::Value, compiled: &str| {
        row["length"].as_u64() == u64::try_from(compiled.len()).ok()
            && row["sha256"].as_str() == Some(hash(compiled.as_bytes()).as_str())
    };
    let metadata_path =
        "src/cleanup_plan/replay/expression_facts/receiving/directory/held_invocation_inverse.json";
    let metadata = include_str!("held_invocation_inverse.json");
    read(metadata_path, metadata)?;
    let packet: serde_json::Value = serde_json::from_str(metadata).ok()?;
    if packet["schema"] != "semaprax.held-status-invocation-source-custody.v1" {
        return None;
    }
    let current = packet["current"].as_array()?;
    if current.len() != HELD_SOURCE_IMAGES.len() {
        return None;
    }
    // Authenticate the entire compiled and physical eight-image boundary even
    // for a stale/foreign supplied source set. No partial source is authority.
    for ((path, compiled), row) in HELD_SOURCE_IMAGES.iter().zip(current) {
        if row["path"] != *path || !bound(&row["current"], compiled) {
            return None;
        }
        read(path, compiled)?;
    }
    if supplied != HELD_SOURCE_IMAGES {
        return None;
    }
    let originals = packet["original_producers"].as_array()?;
    if originals.len() != HELD_ORIGINAL_PRODUCERS.len() {
        return None;
    }
    for ((path, compiled), row) in HELD_ORIGINAL_PRODUCERS.iter().zip(originals) {
        if row["path"] != *path || !bound(&row["current"], compiled) {
            return None;
        }
        read(path, compiled)?;
    }
    let prior_tests = &packet["prior_test_controls"];
    if prior_tests["archive"] != HELD_PRIOR_TESTS.0
        || !bound(&prior_tests["before"], HELD_PRIOR_TESTS.1)
        || !HELD_SOURCE_IMAGES[1].1.starts_with(HELD_PRIOR_TESTS.1)
    {
        return None;
    }
    read(HELD_PRIOR_TESTS.0, HELD_PRIOR_TESTS.1)?;
    let owners = packet["owners"].as_array()?;
    if owners.len() != HELD_PRIOR_IMAGES.len() {
        return None;
    }
    let mut restored = Vec::new();
    for ((path, archive, before), row) in HELD_PRIOR_IMAGES.iter().zip(owners) {
        if row["path"] != *path || row["archive"] != *archive || !bound(&row["before"], before) {
            return None;
        }
        read(archive, before)?;
        let compiled = HELD_SOURCE_IMAGES
            .iter()
            .find(|(candidate, _)| candidate == path)?
            .1;
        if !bound(&row["current"], compiled) {
            return None;
        }
        let mut source = compiled.as_bytes().to_vec();
        let edits = row["edits"].as_array()?;
        let mut next = source.len();
        for edit in edits.iter().rev() {
            let start = usize::try_from(edit["start"].as_u64()?).ok()?;
            let end = usize::try_from(edit["end"].as_u64()?).ok()?;
            if start > end
                || end > next
                || source.get(start..end)? != edit["current"].as_str()?.as_bytes()
            {
                return None;
            }
            source.splice(
                start..end,
                edit["before"].as_str()?.as_bytes().iter().copied(),
            );
            next = start;
        }
        if source.as_slice() != before.as_bytes() {
            return None;
        }
        restored.push(source);
    }
    // Recheck physical images after all four complete inverses and archives.
    for (path, compiled) in HELD_SOURCE_IMAGES.iter().chain(HELD_ORIGINAL_PRODUCERS) {
        read(path, compiled)?;
    }
    read(metadata_path, metadata)?;
    Some(restored)
}

#[test]
fn all_eight_current_images_restore_four_complete_head_owners_and_reject_stale_or_foreign_sources()
{
    let restored = held_source_custody(HELD_SOURCE_IMAGES).unwrap();
    assert_eq!(restored.len(), HELD_PRIOR_IMAGES.len());
    for (before, (_, _, archived)) in restored.iter().zip(HELD_PRIOR_IMAGES) {
        assert_eq!(before.as_slice(), archived.as_bytes());
    }
    for index in 0..HELD_SOURCE_IMAGES.len() {
        let mut drifted = HELD_SOURCE_IMAGES[index].1.to_owned();
        drifted.push(' ');
        let mut supplied = HELD_SOURCE_IMAGES.to_vec();
        supplied[index].1 = &drifted;
        assert!(held_source_custody(&supplied).is_none());
        let mut truncated = HELD_SOURCE_IMAGES[index].1.to_owned();
        truncated.pop();
        supplied[index].1 = &truncated;
        assert!(held_source_custody(&supplied).is_none());
        supplied[index] = ("src/foreign-source.rs", HELD_SOURCE_IMAGES[index].1);
        assert!(held_source_custody(&supplied).is_none());
    }
    let mut foreign = HELD_SOURCE_IMAGES.to_vec();
    foreign.swap(0, 1);
    assert!(held_source_custody(&foreign).is_none());
    assert!(held_source_custody(&HELD_SOURCE_IMAGES[..7]).is_none());
    for (path, _, before) in HELD_PRIOR_IMAGES {
        let mut supplied = HELD_SOURCE_IMAGES.to_vec();
        let slot = supplied
            .iter_mut()
            .find(|(current, _)| current == path)
            .unwrap();
        slot.1 = before;
        assert!(
            held_source_custody(&supplied).is_none(),
            "already-old source is not current authority"
        );
    }
    assert!(held_source_custody(HELD_SOURCE_IMAGES).is_some());
}
