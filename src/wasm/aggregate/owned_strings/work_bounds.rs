use super::Cells;
use crate::hir::{DeclarationId, ExpressionId, FunctionExecutionId};

fn scope_id(index: usize) -> ExpressionId {
    ExpressionId::new(
        &FunctionExecutionId::Monomorphic(DeclarationId::new("work.bounds")),
        &format!("scope.{index}"),
    )
}

#[test]
fn materialized_scope_visits_charge_exact_limit_and_one_more() {
    let mut cells = Cells::default();
    for owner in 0..65_536 {
        cells.insert(owner).unwrap();
    }
    // Two epilogues charge 131072; actual range visits add 65536 + 65535.
    cells.scope(&scope_id(0), 0, 65_536).unwrap();
    cells.scope(&scope_id(1), 0, 65_535).unwrap();
    assert_eq!(
        cells.bounded_emission_work().into_iter().next(),
        Some(262_143)
    );

    // A separate singleton scope is another emitted visit, not deduplicated
    // merely because its owner also appears in the previous lexical scopes.
    cells.scope(&scope_id(2), 65_535, 65_536).unwrap();
    assert_eq!(
        cells.bounded_emission_work().into_iter().next(),
        Some(262_144)
    );
    cells.scope(&scope_id(3), 0, 1).unwrap();
    assert_eq!(cells.bounded_emission_work().into_iter().next(), None);
}

#[test]
fn epilogue_work_is_bounded_even_without_lexical_scopes() {
    let mut cells = Cells::default();
    assert_eq!(cells.bounded_emission_work().into_iter().next(), Some(0));
    for owner in 0..131_072 {
        cells.insert(owner).unwrap();
    }
    assert_eq!(
        cells.bounded_emission_work().into_iter().next(),
        Some(262_144)
    );
    cells.insert(131_072).unwrap();
    assert_eq!(cells.bounded_emission_work().into_iter().next(), None);
    // This is a private counter boundary, not a claim that the standalone
    // selected profile admits this many simultaneous owners.
}
