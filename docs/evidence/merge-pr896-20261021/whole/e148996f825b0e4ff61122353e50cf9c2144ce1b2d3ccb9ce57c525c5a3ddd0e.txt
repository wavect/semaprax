use super::*;
use crate::cleanup_plan::replay::inventory_coverage::paid::receive_observed as receive;
use std::mem::size_of;
type Roots<'a> = std::iter::Chain<
    std::iter::Chain<std::slice::Iter<'a, ResolvedExpr>, std::iter::Once<&'a ResolvedExpr>>,
    std::slice::Iter<'a, ResolvedExpr>,
>;
type Caller<'a> = (
    &'a ResolvedFunction,
    &'a ExpressionId,
    &'a mut ReplayBudget,
    Result<Option<&'a ResolvedExpr>, Error>,
);
type Formal<'a> = (
    &'a ResolvedFunction,
    &'a ExpressionId,
    &'a mut ReplayBudget,
    Result<Option<&'a ResolvedExpr>, Error>,
    Vec<&'a ResolvedExpr>,
    Roots<'a>,
    usize,
    Option<&'a ResolvedExpr>,
    Diagnostic,
    Error,
    Diagnostic,
    Error,
);
type Next<'a> = (
    &'a mut Roots<'a>,
    Option<&'a ResolvedExpr>,
    &'a ResolvedExpr,
);
type Pop<'a> = (
    &'a mut Vec<&'a ResolvedExpr>,
    Option<&'a ResolvedExpr>,
    &'a ResolvedExpr,
);
type ChildCaller<'a> = (
    &'a ResolvedFunction,
    &'a ResolvedExpr,
    usize,
    &'a mut ReplayBudget,
    Result<Option<&'a ResolvedExpr>, Error>,
    &'a ResolvedExpr,
);
type ChildFormal<'a> = (
    &'a ResolvedFunction,
    &'a ResolvedExpr,
    usize,
    &'a mut ReplayBudget,
    Result<Option<&'a ResolvedExpr>, Error>,
    Option<&'a ResolvedExpr>,
    Diagnostic,
    Error,
    Diagnostic,
    Error,
);
type Affine<'a> = (
    &'a ResolvedExpr,
    Option<(&'static DeclarationId, &'a [ResolvedExpr])>,
    &'a [ResolvedExpr],
);
#[test]
fn literal_leaf_lookup_exact_known_unknown_short_zero_and_terminal_none() {
    let program = program("module selected.lookup; @id(\"app.main\") fn main()->i64 {7}");
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block { statements, tail } = &function.body.kind else {
        panic!("genuine resolved function block")
    };
    assert!(statements.is_empty());
    assert!(matches!(tail.kind, ResolvedExprKind::Int(_)));
    let absent = ExpressionId::from_owned("missing.λ\0.lookup".to_owned());
    let initial = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let setup = initial.physical.as_ref().unwrap().charged();
    for known in [true, false] {
        let id = if known { &function.body.id } else { &absent };
        let expected =
            crate::cleanup_plan::replay::transition::find_resolved_expression(function, id);
        let request = size_of::<Caller<'_>>()
            + size_of::<Formal<'_>>()
            + size_of::<&ResolvedExpr>()
            + 2 * size_of::<Next<'_>>()
            + size_of::<Pop<'_>>()
            + size_of::<bool>()
            + if known {
                0
            } else {
                // Complete empty Block -> literal -> terminal reverse-stack
                // traversal. Two block child probes (Some/None) and one
                // literal child None preserve the original table exactly.
                3 * (size_of::<ChildCaller<'_>>()
                    + size_of::<ChildFormal<'_>>()
                    + size_of::<Affine<'_>>())
                    + 2 * size_of::<(usize, std::slice::Iter<'_, crate::hir::ResolvedStatement>)>()
                    + 2 * size_of::<(
                        &mut std::slice::Iter<'_, crate::hir::ResolvedStatement>,
                        Option<&crate::hir::ResolvedStatement>,
                        &crate::hir::ResolvedStatement,
                    )>()
                    + 2 * size_of::<Pop<'_>>()
                    + size_of::<bool>()
                    + 2 * size_of::<usize>()
            };
        let work = 4
            + 12
            + 2 * 3
            + 3
            + 1
            + if known {
                0
            } else {
                3 * (6 + 10 + 3) + 2 * 2 + 2 * 3 + 2 * 3 + 1 + 2
            };
        for (limit, units, complete) in [
            (setup + request, work, true),
            (setup + request - 1, work, false),
            (setup + request, work - 1, false),
            (setup + request, 0, false),
            (setup, work, false),
        ] {
            let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
            budget.remaining = units;
            let actual = receive!(
                budget,
                function,
                Caller<'_>,
                4,
                "selected test lookup caller receiving",
                super::super::lookup::find(function, id, &mut budget)
            );
            if complete {
                let actual = actual.unwrap();
                assert_eq!(
                    actual.map(std::ptr::from_ref),
                    expected.map(std::ptr::from_ref)
                );
                assert_eq!(budget.remaining, 0);
                assert_eq!(budget.physical.as_ref().unwrap().charged(), setup + request);
            } else {
                assert!(matches!(actual, Err(Error::Owned(_))));
                let ledger = budget.physical.as_deref_mut().unwrap();
                let first = ledger.reserve(0).unwrap_err();
                assert_eq!(ledger.reserve(1).unwrap_err(), first);
            }
        }
    }
}
#[test]
fn full_reverse_stack_lookup_matches_independent_old_for_every_genuine_node_and_drops() {
    let program = program(SCALAR);
    for function in &program.functions {
        let mut nodes = Vec::new();
        let mut stack = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(node) = stack.pop() {
            nodes.push(node);
            let mut index = 0usize;
            while let Some(child) =
                crate::cleanup_plan::replay::expression_children::replay_expression_child(
                    node, index,
                )
            {
                stack.push(child);
                index += 1;
            }
        }
        let (_, observed) = measure(|| {
            for node in &nodes {
                let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
                let expected = crate::cleanup_plan::replay::transition::find_resolved_expression(
                    function, &node.id,
                );
                let current = receive!(
                    budget,
                    function,
                    Caller<'_>,
                    4,
                    "selected test full lookup caller receiving",
                    super::super::lookup::find(function, &node.id, &mut budget)
                )
                .unwrap();
                assert_eq!(
                    current.map(std::ptr::from_ref),
                    expected.map(std::ptr::from_ref)
                );
            }
        });
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
