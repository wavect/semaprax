//! Current selected structural family against the complete immediate predecessor.
use super::*;
use crate::cleanup_plan::replay::{
    lexical_replay, primitive_fixture, scalar_replay, selected_context,
};
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
const LEXICAL: &str = r#"module structural.selected;
@id("probe") fn probe(flag:bool)->i64 {
 let key=str"x";let view=str_as_bytes(key);let mut next=0;
 while next<3 {next=next+1;if flag {continue;};if next==2 {return if byte_len(view)==1usize {7}else{0};};0}
 9
}
@id("main") fn main()->i64 {probe(false)}
"#;
const SCALAR: &str = r#"module selected.schema.family;
@id("family.one") fn one(n:i64)->i64 {n+1}
@id("family.main") fn main()->i64 {one(3)}
@id("family.fill") fn fill(data:own Bytes)->Bytes {data}
@id("family.bytes") fn bytes()->i64 {
 let data=fill(bytes_zeroed(1usize));
 if byte_len(bytes_as_slice(data))==1usize {7}else{0}
}
"#;
fn program(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::check(source, "selected-control-family.spx").unwrap()).unwrap()
}
fn render(error: Error, function: &ResolvedFunction) -> Diagnostic {
    match error {
        Error::Owned(error) => error,
        Error::Structural(receipt) => receipt.legacy(function),
        Error::Fact(receipt) => receipt.legacy(function),
        Error::Failed => panic!("fresh invocation cannot already have failed"),
    }
}
fn owner_cost(
    source: &dyn ReplayContext,
    function: &ResolvedFunction,
    old: bool,
) -> (Result<(), Diagnostic>, usize, usize) {
    let lexical = function.cleanup_plan.lexical_control.is_some();
    let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let initial = budget.remaining;
    let context = match selected_context::Context::new_paid(source, function, &mut budget, lexical)
    {
        Ok(context) => context,
        Err(error) => {
            return (
                Err(render(error, function)),
                budget.physical.as_ref().unwrap().charged(),
                initial - budget.remaining,
            )
        }
    };
    let result = match (old, lexical) {
        (true, true) => lexical_replay::before_selected_control::validate_before_selected_control(
            &context,
            function,
            &mut budget,
        ),
        (true, false) => scalar_replay::before_selected_control::validate_before_selected_control(
            &context,
            function,
            &mut budget,
        ),
        (false, true) => lexical_replay::validate_paid(&context, function, &mut budget),
        (false, false) => scalar_replay::validate_paid(&context, function, &mut budget),
    };
    // Detailed rendering is an explicit legacy observation after paid replay.
    (
        result.map_err(|e| render(e, function)),
        budget.physical.as_ref().unwrap().charged(),
        initial - budget.remaining,
    )
}
fn owner(
    source: &dyn ReplayContext,
    function: &ResolvedFunction,
    old: bool,
) -> Result<(), Diagnostic> {
    owner_cost(source, function, old).0
}

fn same_owner(program: &ResolvedProgram, id: &str, expected_success: bool) {
    let source = primitive_fixture::Context::new(program);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == id)
        .unwrap();
    let mut outcomes = Vec::new();
    for old in [true, false] {
        let ((result, bytes, work), observed) = measure(|| owner_cost(&source, function, old));
        assert!(!observed.invalid);
        eprintln!(
            "selected-control {} old={old} physical={bytes} work={work}",
            function.id
        );
        // Returned detailed diagnostic owns text until the observation drops it.
        outcomes.push(result);
    }
    assert_eq!(
        outcomes[0], outcomes[1],
        "all six fields/first refusal {id}"
    );
    assert_eq!(
        outcomes[1].is_ok(),
        expected_success,
        "{id}: {:?}",
        outcomes[1]
    );
    let (_, observed) = measure(|| {
        for old in [true, false] {
            drop(owner(&source, function, old));
        }
    });
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}
mod boundary;
mod children;
mod nominal;
mod whole;

mod renewal;
