use super::{contains, contains_or_false};
use crate::cleanup_plan::replay::{
    expression_facts::errors::EngineError, lexical_cert, selected_context::child::Permit,
    ReplayBudget,
};
use crate::hir::ResolvedFunction;
fn query(
    f: &ResolvedFunction,
    region: crate::cleanup_plan::CleanupRegionId,
    budget: &mut ReplayBudget,
) -> Result<bool, EngineError> {
    let permit = Permit::acquire::<bool>(f, budget)?;
    Ok(contains!(permit, f, f.cleanup_plan.entry, region, true))
}
#[test]
fn exact_borrowed_queries_keep_unknown_cycle_and_intentional_or_false() {
    let p = crate::hir::resolve(
        &crate::check(
            "module chains; @id(\"main\") fn main()->i64 {0}",
            "chains.spx",
        )
        .unwrap(),
    )
    .unwrap();
    let f = &p.functions[0];
    let region = f.cleanup_plan.blocks[f.cleanup_plan.entry.0 as usize].region;
    for mask in 0..3 {
        let mut changed = f.clone();
        match mask {
            1 => changed.cleanup_plan.regions[region.0 as usize].parent = Some(region),
            2 => {
                changed.cleanup_plan.regions[region.0 as usize].parent =
                    Some(crate::cleanup_plan::CleanupRegionId(u32::MAX))
            }
            _ => {}
        }
        let old = lexical_cert::storage::chain_contains(
            &changed,
            changed.cleanup_plan.entry,
            region,
            true,
        );
        let mut b = ReplayBudget::with_physical(&changed, usize::MAX).unwrap();
        let new = query(&changed, region, &mut b);
        match (old, new) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(EngineError::Structural(e))) => assert_eq!(a, e.legacy(&changed)),
            _ => panic!("borrowed chain differs"),
        }
        assert_eq!(
            contains_or_false!(&changed, changed.cleanup_plan.entry, region, true),
            mask == 0
        );
    }
    assert_eq!(
        contains_or_false!(f, f.cleanup_plan.entry, region, false),
        lexical_cert::storage::chain_contains(f, f.cleanup_plan.entry, region, false).unwrap()
    );
}
