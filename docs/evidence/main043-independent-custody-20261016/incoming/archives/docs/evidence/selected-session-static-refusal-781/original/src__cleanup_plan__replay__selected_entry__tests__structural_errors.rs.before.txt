//! Narrow held-Session structural subphase; not full-driver acceptance/funding.
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};
const FORM: ReportForm = ReportForm::new("SPX-T781", "typed structural refusal");
fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(
        "module refusal.storage; @id(\"main\") fn main()->i64 {let a=bytes_zeroed(1usize);let b=bytes_zeroed(1usize);0}", "storage.spx"
    ).unwrap()).unwrap()
}
fn session<'a>(program: &'a ResolvedProgram, limit: usize) -> Session<'a> {
    let function = &program.functions[0];
    Acquired::try_acquire(PhysicalReservation::new(limit), FORM)
        .ok()
        .unwrap()
        .prepare(function)
        .ok()
        .unwrap()
        .into_paid_session(program, function)
        .ok()
        .unwrap()
}
#[test]
fn held_session_structural_refusal_keeps_paid_report_exact_short_sticky_and_drop() {
    let program = program();
    let function = &program.functions[0];
    assert!(function.cleanup_plan.slots.len() >= 2);
    let mut duplicate = function.clone();
    duplicate.cleanup_plan.slots.truncate(2);
    duplicate.cleanup_plan.slots[1].storage = duplicate.cleanup_plan.slots[0].storage.clone();
    // Constructor baseline is observed separately; only the added single unique
    // B-tree key reservation is an independent literal subphase layout census.
    let (_, refused, base) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let owner = session(&program, usize::MAX);
        let n = owner.usage().1;
        drop(owner);
        n
    });
    assert!(!refused);
    // Independent pinned B=6 internal node fields: parent, two u16 fields,
    // eleven StorageId keys, zero-sized values, and twelve child pointers.
    let alignment = std::mem::align_of::<StorageId>().max(std::mem::align_of::<usize>());
    let round = |n: usize| n.div_ceil(alignment) * alignment;
    let extra = round(std::mem::size_of::<Option<std::ptr::NonNull<()>>>())
        + 2 * round(std::mem::size_of::<u16>())
        + round(11 * std::mem::size_of::<StorageId>())
        + round(12 * std::mem::size_of::<usize>());
    assert_eq!(
        extra,
        physical_storage::btree_node_upper::<StorageId, ()>().unwrap()
    );
    for (limit, semantic) in [
        (base, false),
        (base + extra - 1, false),
        (base + extra, true),
    ] {
        let (_, overflowed, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let (_, observed) = measure(|| {
                let mut selected = session(&program, limit);
                let report = selected.report.as_ref().unwrap().backing();
                let ledger = selected.budget.physical.as_deref().unwrap() as *const _;
                let error = structural_maps::validate_slots_core::<
                    expression_facts::errors::EngineError,
                >(&duplicate, &mut selected.budget)
                .err()
                .unwrap();
                assert_eq!(
                    matches!(&error, expression_facts::errors::EngineError::Structural(_)),
                    semantic
                );
                if semantic {
                    assert_eq!(selected.usage().1, base + extra);
                }
                selected.failed = true;
                let before = current_metrics();
                assert!(matches!(
                    selected.validate_scalar_paid(function),
                    Err(expression_facts::errors::EngineError::Failed)
                ));
                let mut refusal = selected.into_failed_preparation().refuse_engine(error);
                assert_eq!(refusal.report().unwrap().as_ptr(), report.0);
                assert_eq!(refusal.report().unwrap()[0].message.as_ptr(), report.1);
                assert_eq!(refusal.ledger_mut().unwrap() as *const _, ledger);
                if semantic {
                    let receipt = refusal.structural_receipt().unwrap();
                    assert_eq!(
                        receipt.source().as_str().as_ptr(),
                        duplicate.body.id.as_str().as_ptr()
                    );
                    assert!(refusal.receipt().is_none());
                } else {
                    assert_eq!(refusal.receipt().unwrap().code, "SPX-H006");
                }
                drop(refusal.into_report(FORM).ok().unwrap());
                assert_eq!(current_metrics().allocations, before.allocations);
            });
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        });
        assert!(!overflowed);
    }
    // Original first-error order: malformed slot wins before the later duplicate.
    duplicate.cleanup_plan.slots[0].storage_index = 99;
    let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
    let old = structural_maps::validate_slots(&duplicate, &mut budget).unwrap_err();
    assert!(old.message.contains("slot at index 0"));
    let mut budget = ReplayBudget::with_physical(function, usize::MAX).unwrap();
    let error = structural_maps::validate_slots_core::<expression_facts::errors::EngineError>(
        &duplicate,
        &mut budget,
    )
    .unwrap_err();
    let expression_facts::errors::EngineError::Structural(receipt) = error else {
        panic!("typed slot receipt")
    };
    let rendered = receipt.legacy(&duplicate);
    assert_eq!(rendered.code, old.code);
    assert_eq!(rendered.message, old.message);
}

#[test]
fn status_atoms_preserve_ordinary_selected_text_and_full_driver_earlier_priority() {
    let program = super::program();
    let original = super::members(&program)[0];
    assert!(!original.cleanup_plan.status_sources.is_empty());
    let facts = expression_facts::expression_facts(original).unwrap();
    // Saved pre-migration identity stage, independently constructs legacy text.
    let saved = |function: &ResolvedFunction| {
        let mut ids = BTreeSet::new();
        for source in &function.cleanup_plan.status_sources {
            if !ids.insert(source.id.clone()) {
                return replay_error(
                    function,
                    format!("status source for `{}` is repeated", source.id.expression),
                );
            }
            if !facts.contains_key(&source.id.expression) {
                return replay_error(
                    function,
                    format!(
                        "status source references unknown expression `{}`",
                        source.id.expression
                    ),
                );
            }
        }
        panic!("identity hostile must refuse")
    };
    for duplicate in [false, true] {
        let mut forged = original.clone();
        forged.cleanup_plan.status_sources.truncate(1);
        if duplicate {
            forged
                .cleanup_plan
                .status_sources
                .push(forged.cleanup_plan.status_sources[0].clone());
        } else {
            forged.cleanup_plan.status_sources[0].id.expression = program
                .functions
                .iter()
                .find(|f| f.id.as_str() == "main")
                .unwrap()
                .body
                .id
                .clone();
        }
        let old = saved(&forged);
        let ordinary = validate_status_sources(&forged, &facts).unwrap_err();
        assert_eq!(ordinary.code, old.code);
        assert_eq!(ordinary.message, old.message);
        assert!(old.message.contains(if duplicate {
            "is repeated"
        } else {
            "unknown expression"
        }));
        let mut budget = ReplayBudget::with_physical(original, usize::MAX).unwrap();
        let selected =
            status_validation::validate_status_sources(&forged, &facts, &mut budget).unwrap_err();
        assert_eq!(selected.code, old.code);
        assert_eq!(selected.message, old.message);
        let mut budget = ReplayBudget::with_physical(original, usize::MAX).unwrap();
        let error = status_validation::validate_core::<expression_facts::errors::EngineError>(
            &forged,
            &facts,
            &mut budget,
        )
        .unwrap_err();
        let expression_facts::errors::EngineError::Structural(receipt) = error else {
            panic!("typed status")
        };
        let rendered = receipt.legacy(&forged);
        assert_eq!(rendered.code, old.code);
        assert_eq!(rendered.message, old.message);
        // Full replay still fails at the original earlier exact inventory stage.
        // A detached function is deliberately not made current by the receipt.
        let mut old = session(&program, usize::MAX);
        let mut paid = session(&program, usize::MAX);
        let legacy = old.validate(&forged).unwrap_err();
        let error = paid.validate_paid(&forged).unwrap_err();
        let expression_facts::errors::EngineError::Owned(actual) = error else {
            panic!("earlier current-member error remains owned")
        };
        assert_eq!(actual.code, legacy.code);
        assert_eq!(actual.message, legacy.message);
    }
}
