//! Primitive-only harness for the byte-identical old protocol owner.
//! No global current validator is called or charged as historical evidence.
use super::*;
use crate::test_allocation_observer::measure;
type CurrentBacking<'hir> = super::super::Backing<'hir>;

fn fixture() -> ResolvedProgram {
    let mut source = String::from("module old.held; @id(\"old.large\") fn large(x:i64)->i64 {");
    for index in 0..256 {
        let input = if index == 0 { "x".to_owned() } else { format!("v{}", index - 1) };
        source.push_str(&format!("let v{index}={input}+1;"));
    }
    source.push_str("v255} @id(\"old.small\") fn small(x:i64)->i64{x+1} @id(\"old.main\") fn main()->i64{0}");
    let program = crate::hir::resolve(&crate::check(&source, "old-held.spx").unwrap()).unwrap();
    assert_eq!(program.functions.len(), 3);
    assert_eq!(program.functions[0].cleanup_plan.status_sources.len(), 256);
    assert_eq!(program.functions[1].cleanup_plan.status_sources.len(), 1);
    assert!(program.functions[2].cleanup_plan.status_sources.is_empty());
    program
}
fn acquire<'hir>(program: &'hir ResolvedProgram, budget: &mut ReplayBudget) -> Result<Backing<'hir>, Diagnostic> {
    let first = &program.functions[0];
    budget.prepare_builder(first)?;
    budget.reserve_bytes(first, Backing::ACQUISITION_BYTES, "old held primitive acquisition")?;
    Backing::acquire(program, first, budget)
}

// The same actual primitive sequence runs either actual type. The old branch
// uses Backing from this owner, never a current global call or borrowed tariff.
macro_rules! primitive_sequence {
    ($program:ident, $backing:ident) => {{
        let mut budget = ReplayBudget::new();
        let first = &$program.functions[0];
        budget.prepare_builder(first)?;
        budget.reserve_bytes(first, $backing::ACQUISITION_BYTES, "held primitive acquisition")?;
        let mut owner = $backing::acquire($program, first, &mut budget)?;
        let mut previous_pointer = std::ptr::null();
        let mut previous_capacity = 0;
        for function in &$program.functions {
            let mut output = None;
            if !owner.directory_into($program, function, &mut budget, &mut output) {
                return Err(owner.take_error());
            }
            let mut directory = output.take().unwrap();
            assert!(directory.nodes.is_empty() && directory.lookup.is_none());
            assert_eq!(directory.root, NIL);
            assert_eq!(directory.first, NIL);
            if previous_capacity != 0 {
                assert_eq!(directory.nodes.as_ptr(), previous_pointer);
                assert_eq!(directory.nodes.capacity(), previous_capacity);
            }
            {
                let mut prefix = directory.held_prefix(function, &mut budget)?;
                for source in &function.cleanup_plan.status_sources {
                    assert!(prefix.insert(function, &source.id, &mut budget)?);
                }
                prefix.freeze(function, &mut budget)?;
            }
            let mut independent = Directory::new(function, &mut budget, "independent old primitive directory")?;
            for source in &function.cleanup_plan.status_sources {
                assert!(independent.insert(function, &source.id, &mut budget)?);
            }
            assert!(directory.balanced() && independent.balanced());
            assert!(directory.iter().eq(independent.iter()));
            previous_pointer = directory.nodes.as_ptr();
            previous_capacity = directory.nodes.capacity();
            drop(independent);
            owner.recycle(directory);
            assert!(owner.empty.as_ref().unwrap().is_empty());
            assert_eq!(owner.empty.as_ref().unwrap().as_ptr(), previous_pointer);
            assert_eq!(owner.empty.as_ref().unwrap().capacity(), previous_capacity);
            if !owner.complete_into($program, function, &mut budget) {
                return Err(owner.take_error());
            }
        }
        assert_eq!(owner.next_function, $program.functions.len());
        assert!(owner.error.is_none() && !owner.terminal_taken);
        Ok(())
    }};
}
fn old_sequence(program: &ResolvedProgram) -> Result<(), Diagnostic> { primitive_sequence!(program, Backing) }
fn current_sequence(program: &ResolvedProgram) -> Result<(), Diagnostic> { primitive_sequence!(program, CurrentBacking) }
fn run(program: &ResolvedProgram, old: bool, cap: usize, floor: usize, success: bool) -> usize {
    let ((ok, metrics), exhausted, paid, receipt) = crate::bounded_output::with_limit_usage_refusal(cap, || {
        assert!(crate::bounded_output::set_active_floor(floor));
        let measured = measure(|| {
            let result = if old { old_sequence(program) } else { current_sequence(program) };
            assert_eq!(result.is_ok(), success, "old={old} cap={cap}: {result:?}");
            if let Err(error) = &result { assert_eq!(error.code, "SPX-H006"); }
            let ok = result.is_ok();
            drop(result);
            ok
        });
        crate::bounded_output::clear_active_floor();
        measured
    });
    assert_eq!(ok, success);
    assert_eq!(exhausted, !success);
    assert_eq!(receipt.is_some(), !success);
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    assert!(metrics.allocated <= paid);
    paid
}
#[test]
fn actual_old_and_current_primitives_keep_directory_backing_ordinals_full_drop_and_typed_tariffs() {
    let program = fixture();
    let old = run(&program, true, usize::MAX, 0, true);
    let current = run(&program, false, usize::MAX, 0, true);
    let count = program.functions.len();
    assert_eq!(old + CurrentBacking::ACQUISITION_BYTES + count * CurrentBacking::PER_FUNCTION_BYTES,
        current + Backing::ACQUISITION_BYTES + count * Backing::PER_FUNCTION_BYTES);
    for (old, exact) in [(true, old), (false, current)] {
        assert_eq!(run(&program, old, exact, 0, true), exact);
        run(&program, old, exact - 1, 0, false);
        run(&program, old, exact, 1, false);
        run(&program, old, 0, 0, false);
    }
}
#[test]
fn original_authenticate_context_and_completion_are_executed_on_the_old_owner() {
    let program = fixture();
    let (_, exhausted, _, receipt) = crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let mut owner = acquire(&program, &mut budget).unwrap();
        let first = &program.functions[0];
        assert!(owner.authenticate_into(first, &mut budget));
        assert!(owner.context_into(&program, first, &mut budget));
        assert!(owner.complete_into(&program, first, &mut budget));
        assert_eq!(owner.next_function, 1);
        assert!(owner.error.is_none());
    });
    assert!(!exhausted && receipt.is_none());
}
#[test]
fn old_foreign_order_and_occupied_failures_keep_raw_diagnostics_without_current_receipts() {
    let program = fixture();
    let foreign = fixture();
    for attack in 0..4 {
        let (_, exhausted, _, receipt) = crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            let mut owner = acquire(&program, &mut budget).unwrap();
            let first = &program.functions[0];
            let mut foreign_budget = ReplayBudget::new();
            let mut output = None;
            if attack == 3 {
                let mut occupied = Directory::new(first, &mut budget, "old occupied primitive slot").unwrap();
                assert!(occupied.insert(first, &first.cleanup_plan.status_sources[0].id, &mut budget).unwrap());
                output = Some(occupied);
            }
            let pointer = output.as_ref().map(|d| d.nodes.as_ptr());
            let accepted = match attack {
                0 => owner.directory_into(&foreign, first, &mut budget, &mut output),
                1 => owner.directory_into(&program, first, &mut foreign_budget, &mut output),
                2 => owner.directory_into(&program, &program.functions[1], &mut budget, &mut output),
                _ => owner.directory_into(&program, first, &mut budget, &mut output),
            };
            assert!(!accepted && owner.next_function == 0);
            assert_eq!(output.as_ref().map(|d| d.nodes.as_ptr()), pointer);
            if let Some(directory) = output.as_ref() { assert_eq!(directory.len(), 1); }
            let reason = match attack {
                0 => "status backing belongs to another typed program",
                1 => "status backing belongs to another invocation",
                2 => "status backing function order differs from held program",
                _ => "status directory receiving slot is not empty",
            };
            let function = if attack == 2 { &program.functions[1] } else { first };
            let expected = Diagnostic {
                code: "SPX-H006", severity: crate::diagnostic::Severity::Error,
                message: if attack == 1 { String::new().into() } else {
                    format!("cleanup plan for function `{}` failed independent replay: {reason}", function.id).into()
                },
                path: None, span: None, help: None,
            };
            assert_eq!(owner.error.as_ref(), Some(&expected));
            assert!(owner.ledger.first_refusal().is_none());
            assert!(!owner.authenticate_into(first, &mut budget));
            assert!(!owner.complete_into(&program, first, &mut budget));
            assert_eq!(owner.error.as_ref(), Some(&expected));
            assert_eq!(owner.take_error(), expected);
            assert!(owner.terminal_taken && owner.error.is_none());
            assert!(!owner.authenticate_into(first, &mut budget));
            assert_eq!(owner.next_function, 0);
        });
        assert!(!exhausted && receipt.is_none(), "old raw refusal must not impersonate the successor receipt");
    }
}
#[test]
fn byte_identical_old_owner_and_original_shared_test_archive_remain_whole() {
    use sha2::{Digest, Sha256};
    let hash = |raw: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)));
    let owner = include_bytes!("owner.rs");
    assert_eq!(owner.len(), 12925);
    assert_eq!(hash(owner), "1f1c63aee7d304e6c44d2decc2b72c285621d0977257d7a33a62fbb65023eb59");
    let current_tests = include_bytes!("../held_invocation_tests.rs");
    let archived = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/623b7cc93e6b6a81ffd7933978ec91c0126c4891d68c8f43919481b978936226"));
    assert_eq!(current_tests.len(), 34683);
    assert_eq!(hash(current_tests), "623b7cc93e6b6a81ffd7933978ec91c0126c4891d68c8f43919481b978936226");
    assert_eq!(current_tests.as_slice(), archived.as_slice());
}
