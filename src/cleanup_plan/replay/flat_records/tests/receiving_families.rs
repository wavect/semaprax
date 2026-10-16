//! Whole structural owners: independent legacy receiver and paid publication.
use super::program;
use crate::cleanup_plan::replay::{
    expression_facts::errors::EngineError, structural_maps, Leaves, ReplayBudget,
};
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedFunction;
use crate::test_allocation_observer::measure;

fn current(f: &ResolvedFunction, b: &mut ReplayBudget) -> Result<Leaves, EngineError> {
    let storage = structural_maps::paid::slots(f, b)?;
    let leaves = structural_maps::paid::leaves(f, b)?;
    structural_maps::paid::regions(f, &storage, b)?;
    Ok(leaves)
}
fn legacy(f: &ResolvedFunction, b: &mut ReplayBudget) -> Result<Leaves, Diagnostic> {
    let storage = structural_maps::validate_slots(f, b)?;
    let leaves = structural_maps::collect_leaves(f, b)?;
    structural_maps::validate_regions(f, &storage, b)?;
    Ok(leaves)
}
fn same(a: &Diagnostic, b: &Diagnostic) {
    assert_eq!(a.code, b.code);
    assert_eq!(a.severity, b.severity);
    assert_eq!(a.message, b.message);
    assert_eq!(a.path, b.path);
    assert_eq!(a.span, b.span);
    assert_eq!(a.help, b.help);
}
fn semantic(error: EngineError, f: &ResolvedFunction) -> Diagnostic {
    match error {
        EngineError::Structural(receipt) => receipt.legacy(f),
        other => panic!("expected closed semantic refusal: {other:?}"),
    }
}
#[test]
fn whole_structural_families_keep_records_places_order_and_complete_refusals() {
    let p = program();
    for f in &p.functions {
        let old = legacy(f, &mut ReplayBudget::new()).unwrap();
        let mut budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let new = current(f, &mut budget).unwrap();
        assert_eq!(old.len(), new.len());
        for ((old_flag, old_leaf), (new_flag, new_leaf)) in old.iter().zip(new.iter()) {
            assert_eq!(old_flag, new_flag);
            assert_eq!(old_leaf.place, new_leaf.place);
            assert_eq!(old_leaf.lifecycle, new_leaf.lifecycle);
            assert_eq!(old.under(&old_leaf.place), new.under(&new_leaf.place));
        }
        for mask in 1..8 {
            let mut changed = f.clone();
            if mask & 1 != 0 {
                changed.cleanup_plan.regions.clear();
            }
            if mask & 2 != 0 {
                if let Some(slot) = changed.cleanup_plan.slots.first_mut() {
                    slot.storage_index = u32::MAX;
                }
            }
            if mask & 4 != 0 {
                if let Some(region) = changed.cleanup_plan.regions.first_mut() {
                    region.id.0 = u32::MAX;
                }
            }
            let old = legacy(&changed, &mut ReplayBudget::new());
            let mut budget = ReplayBudget::with_physical(&changed, usize::MAX).unwrap();
            let new = current(&changed, &mut budget);
            match (old, new) {
                (Err(old), Err(new)) => same(&old, &semantic(new, &changed)),
                (Ok(_), Ok(_)) => assert!(changed.cleanup_plan.slots.is_empty() && mask == 2),
                _ => panic!("whole old/current structural verdict differs"),
            }
        }
    }
}
#[test]
fn family_actual_ledger_exact_short_work_and_complete_drop() {
    let p = program();
    for f in &p.functions {
        let mut old = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let before = old.remaining;
        drop(legacy(f, &mut old).unwrap());
        let old_paid = old.physical.as_deref().unwrap().charged();
        let old_work = before - old.remaining;
        drop(old);
        let mut budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let setup = budget.physical.as_deref().unwrap().charged();
        let before = budget.remaining;
        drop(current(f, &mut budget).unwrap());
        let exact = budget.physical.as_deref().unwrap().charged();
        let work = before - budget.remaining;
        eprintln!("structural-family {} old_paid={old_paid} new_paid={exact} old_work={old_work} new_work={work}",f.id);
        drop(budget);
        for (limit, accepted) in [(setup, false), (exact - 1, false), (exact, true)] {
            let ((), seen) = measure(|| {
                let mut b = ReplayBudget::with_physical(f, limit).unwrap();
                let ledger = b.physical.as_deref().unwrap() as *const _;
                let text = b.refusal_text.as_ref().unwrap().as_ptr();
                let result = current(f, &mut b);
                assert_eq!(result.is_ok(), accepted);
                if let Err(EngineError::Owned(error)) = &result {
                    assert_eq!(error.code, "SPX-H006");
                    assert_eq!(error.message.as_ptr(), text);
                    let physical = b.physical.as_deref_mut().unwrap();
                    let first = physical.reserve(0).unwrap_err();
                    assert_eq!(physical.reserve(1).unwrap_err(), first);
                } else {
                    assert!(accepted);
                }
                assert_eq!(b.physical.as_deref().unwrap() as *const _, ledger);
                if accepted {
                    assert_eq!(b.physical.as_deref().unwrap().charged(), exact);
                }
                drop(result);
                drop(b);
            });
            assert!(!seen.invalid);
            assert_eq!(seen.live, 0);
            assert_eq!(seen.allocated, seen.freed);
        }
        for remaining in [0, work - 1, work] {
            let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
            b.remaining = remaining;
            assert_eq!(current(f, &mut b).is_ok(), remaining == work);
        }
    }
}
#[test]
fn lexical_u32_overflow_keeps_original_full_diagnostic() {
    use crate::cleanup_plan::replay::selected_context::child::Permit;
    fn index(f: &ResolvedFunction, b: &mut ReplayBudget, index: usize) -> Result<u32, EngineError> {
        let permit = Permit::profile::<u32>(f, b)?;
        Ok(structural_maps::paid::index!(
            f,
            permit,
            index,
            TooManySlots
        ))
    }
    let p = program();
    let f = &p.functions[0];
    for value in [0, 1, u32::MAX as usize] {
        let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        assert_eq!(
            index(f, &mut b, value).unwrap(),
            u32::try_from(value).unwrap()
        );
    }
    if usize::BITS > 32 {
        let mut b = ReplayBudget::with_physical(f, usize::MAX).unwrap();
        let old = super::super::super::u32_index(f, usize::MAX, "cleanup slot").unwrap_err();
        same(
            &old,
            &semantic(index(f, &mut b, usize::MAX).unwrap_err(), f),
        );
    }
}

#[test]
fn complete_paid_drivers_keep_independent_source_authenticated_success_and_refusal() {
    use crate::cleanup_plan::replay::{lexical_replay, scalar_replay, selected_context};
    let scalar = "module family.scalar; @id(\"family.scalar.main\") fn main()->i64 {0}";
    for (source, id, lexical) in [
        (super::SOURCE, "flat.make", true),
        (scalar, "family.scalar.main", false),
    ] {
        let fixture = crate::workspace_graph::SourceRecordReplayFixture::new(source);
        fixture.with_prepared_context(|source| {
            let f = source
                .functions()
                .iter()
                .find(|f| f.id.as_str() == id)
                .unwrap();
            for hostile in [false, true] {
                let mut changed = f.clone();
                if hostile {
                    changed.cleanup_plan.regions.clear();
                }
                // The immutable lease authenticates the original owning subject.
                // Hostile attached metadata is checked against that same source.
                // Full driver admission may refuse the cloned subject earlier;
                // both independent routes must select the same complete error.
                let subject = if hostile { &changed } else { f };
                let mut old_budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                let old_context =
                    selected_context::Context::new_paid(source, f, &mut old_budget, lexical)
                        .unwrap();
                let old = if lexical {
                    lexical_replay::before_owned_families::validate_paid(
                        &old_context,
                        subject,
                        &mut old_budget,
                    )
                } else {
                    scalar_replay::before_owned_families::validate_paid(
                        &old_context,
                        subject,
                        &mut old_budget,
                    )
                };
                let mut new_budget = ReplayBudget::with_physical(f, usize::MAX).unwrap();
                let new_context =
                    selected_context::Context::new_paid(source, f, &mut new_budget, lexical)
                        .unwrap();
                let new = if lexical {
                    lexical_replay::validate_paid(&new_context, subject, &mut new_budget)
                } else {
                    scalar_replay::validate_paid(&new_context, subject, &mut new_budget)
                };
                let render = |error| match error {
                    EngineError::Owned(error) => error,
                    EngineError::Structural(receipt) => receipt.legacy(subject),
                    other => panic!("unexpected error {other:?}"),
                };
                match (old, new) {
                    (Ok(()), Ok(())) => assert!(!hostile),
                    (Err(old), Err(new)) => {
                        assert!(hostile);
                        same(&render(old), &render(new));
                    }
                    _ => panic!("complete old/current paid driver differs"),
                }
            }
        });
    }
}
