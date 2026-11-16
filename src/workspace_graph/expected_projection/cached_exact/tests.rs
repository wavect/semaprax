use super::{summed, Retention};
use crate::ast::Program;
fn programs() -> Vec<Program> {
    ["one", "two"]
        .into_iter()
        .map(|module| {
            crate::parse(
                &format!("module {module}; @id(\"{module}.main\") fn main()->i64 {{1+2}}"),
                std::path::Path::new(&format!("{module}.spx")),
            )
            .unwrap()
        })
        .collect()
}
struct Restore(usize);
impl Drop for Restore {
    fn drop(&mut self) {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
    }
}
fn limit(bytes: usize) -> Restore {
    Restore(crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(bytes)))
}
#[test]
fn semantic_cache_charges_two_exact_hir_bundles_without_duplicating_arc_carriers() {
    let programs = programs();
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let parsed = summed(&programs, &authored, Retention::Parsed).unwrap();
    let semantic = summed(&programs, &authored, Retention::Semantic).unwrap();
    let shared = summed(&programs, &authored, Retention::SharedProject).unwrap();
    assert_eq!(shared, parsed);
    // Independently known AST: two modules, each Block/Binary/Int/Int. Only the
    // Binary and two Int roots omit their four proved-absent storage footprints;
    // as block tail and operands they also omit their six auxiliary carriers.
    let extra = 8 * crate::workspace_graph::HIR_EXPR_FIXED_BUNDLE
        - 6 * (super::super::cost::EXACT_ABSENT_ROOT_STORAGE
            + super::super::cost::EXACT_ROOT_AUXILIARY_CARRIERS);
    assert_eq!(semantic.0 - parsed.0, extra);
    assert_eq!(semantic.1 - parsed.1, extra);
    let root = crate::hir::DeclarationId::new("one.main");
    let id =
        crate::hir::ExpressionId::new(&crate::hir::FunctionExecutionId::Monomorphic(root), "body");
    let cloned = id.clone();
    assert_eq!(id.as_str().as_ptr(), cloned.as_str().as_ptr());
    let mut malformed = super::super::StructuralCost::new();
    malformed.exact_expressions = 1;
    assert!(super::fixed_upper(
        0,
        &malformed,
        &super::super::StructuralCost::new(),
        Retention::Semantic
    )
    .is_err());
    malformed.exact_expressions = usize::MAX;
    assert!(super::fixed_upper(
        usize::MAX,
        &malformed,
        &super::super::StructuralCost::new(),
        Retention::Semantic
    )
    .is_err());
}
#[test]
fn cached_exact_forecast_uses_all_module_sums_and_exact_one_short_limit() {
    let programs = programs();
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    for retention in [
        Retention::Parsed,
        Retention::Semantic,
        Retention::SharedProject,
    ] {
        let full = summed(&programs, &authored, retention).unwrap();
        let parts: Vec<_> = programs
            .iter()
            .map(|program| summed(std::slice::from_ref(program), &authored, retention).unwrap())
            .collect();
        assert_eq!(full.0, parts.iter().map(|part| part.0).sum::<usize>());
        assert_eq!(full.1, parts.iter().map(|part| part.1).sum::<usize>());
        {
            let _restore = limit(full.1);
            assert_eq!(summed(&programs, &authored, retention).unwrap(), full);
        }
        {
            let _restore = limit(full.1 - 1);
            assert!(summed(&programs, &authored, retention)
                .unwrap_err()
                .iter()
                .any(|error| error.code == "SPX-G171"));
        }
        {
            let _restore = limit(parts.iter().map(|part| part.1).max().unwrap());
            assert!(summed(&programs, &authored, retention).is_err());
        }
    }
}
#[test]
fn cached_exact_preserves_old_fitting_receipts_and_never_selects_peak_or_filter() {
    let programs = programs();
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let old = super::super::initial_core_prebound(&programs, &authored, false).unwrap();
    for retention in [Retention::Parsed, Retention::Semantic] {
        assert_eq!(
            super::initial(&programs, &authored, false, retention).unwrap(),
            old
        );
        let mut mode = 4;
        let exact = summed(&programs, &authored, retention).unwrap();
        assert_eq!(
            super::next(&programs, &authored, usize::MAX, &mut mode, retention).unwrap(),
            exact
        );
        assert_eq!(mode, 7);
        assert!(super::next(&programs, &authored, usize::MAX, &mut mode, retention).is_err());
    }
}
// Literal, unary (checked `-`, `!`) and binary (checked arithmetic, comparison
// and lazy `&&`/`||`) roots over owned children: a String call and literal.
const ABSENCE: &str = r#"module exact.absence;
@id("exact.absence.text") fn text()->string { "owned" }
@id("exact.absence.main") fn main()->i64 {
 let word=text();
 let wide=-(7/2)+(1-3)*2%5;
 let gate=!(wide<0) && (wide>=0 || wide!=3);
 if gate && string_len(word)==5 {wide}else{0}
}
"#;
fn census(body: &str, inline: bool) -> [usize; 3] {
    let source = format!("module census; @id(\"census.main\") fn main() -> i64 {{ {body} }}");
    let program = crate::parse(&source, std::path::Path::new("census.spx")).unwrap();
    let mut cost = super::super::StructuralCost::raw_ast(true).with_inline_values(inline);
    super::super::declaration_cost::ast_expr_cost(&program.functions[0].body, &mut cost).unwrap();
    assert!(cost.exact_absent_roots.iter().sum::<usize>() <= cost.exact_expressions);
    cost.exact_absent_roots
}
#[test]
fn absent_root_storage_is_exactly_four_footprints_and_bundle_keeps_every_other_term() {
    use std::mem::size_of;
    let storage = size_of::<crate::cleanup::CleanupStorageSlot>()
        + size_of::<crate::cleanup::CleanupFlag>()
        + size_of::<crate::cleanup::CleanupPlace>()
        + size_of::<crate::cleanup_plan::CleanupSlot>();
    assert_eq!(super::super::cost::EXACT_ABSENT_ROOT_STORAGE, storage);
    // HIR carriers, entry states, failure/lazy transitions, blocks, edges,
    // regions and loan reachability all remain charged for every root.
    let retained = size_of::<crate::hir::ResolvedExpr>()
        + size_of::<crate::hir::ResolvedBinding>()
        + size_of::<crate::hir::ResolvedStatement>()
        + size_of::<crate::hir::ResolvedFieldInitializer>()
        + size_of::<crate::hir::ResolvedMatchArm>()
        + size_of::<crate::hir::ResolvedMatchPatternField>()
        + size_of::<crate::hir::ResolvedRecordMatchPatternField>()
        + size_of::<crate::cleanup::CleanupEntryState>()
        + size_of::<crate::cleanup_plan::CleanupEntryState>()
        + size_of::<crate::cleanup_plan::CleanupTransition>()
        + size_of::<crate::cleanup_plan::CleanupBlock>()
        + size_of::<crate::cleanup_plan::CleanupEdge>()
        + size_of::<crate::cleanup_plan::CleanupRegion>()
        + crate::loan_plan::REACHABILITY_BYTES_PER_EXPRESSION;
    assert_eq!(
        crate::workspace_graph::HIR_EXPR_FIXED_BUNDLE - storage,
        retained
    );
}
#[test]
fn fixed_upper_omits_absent_storage_per_retained_copy_and_rejects_malformed_census() {
    use super::super::StructuralCost;
    let footprint = crate::workspace_graph::HIR_FIXED_EXPANSION_FACTOR
        * std::mem::size_of::<crate::ast::Expr>();
    let bundle = crate::workspace_graph::HIR_EXPR_FIXED_BUNDLE;
    let storage = super::super::cost::EXACT_ABSENT_ROOT_STORAGE;
    let upper = 1 << 24;
    let mut raw = StructuralCost::new();
    raw.exact_expressions = 5;
    raw.exact_absent_roots = [1, 1, 1];
    // A runtime census is never consulted: synthetic bodies keep full bundles.
    let mut runtime = StructuralCost::new();
    runtime.exact_expressions = 2;
    runtime.exact_absent_roots = [2, 0, 0];
    let mut plain = StructuralCost::new();
    plain.exact_expressions = 5;
    for (retention, copies) in [(Retention::Parsed, 1), (Retention::Semantic, 2)] {
        assert_eq!(
            super::fixed_upper(upper, &raw, &runtime, retention).unwrap(),
            upper - 7 * footprint + copies * (7 * bundle - 3 * storage)
        );
        assert_eq!(
            super::fixed_upper(upper, &plain, &runtime, retention).unwrap(),
            upper - 7 * footprint + copies * 7 * bundle
        );
    }
    // Two of the three storage roots are also carrier-free: each retained copy
    // omits exactly two more carrier sets, never a runtime root's.
    let carriers = super::super::cost::EXACT_ROOT_AUXILIARY_CARRIERS;
    raw.exact_carrier_free_roots = 2;
    for (retention, copies) in [(Retention::Parsed, 1), (Retention::Semantic, 2)] {
        assert_eq!(
            super::fixed_upper(upper, &raw, &runtime, retention).unwrap(),
            upper - 7 * footprint + copies * (7 * bundle - 3 * storage - 2 * carriers)
        );
    }
    // The other two raw expressions as free `if` roots: each copy omits two
    // more carrier sets but no storage. A hostile runtime count is ignored.
    raw.exact_carrier_free_ifs = 2;
    runtime.exact_carrier_free_ifs = usize::MAX;
    for (retention, copies) in [(Retention::Parsed, 1), (Retention::Semantic, 2)] {
        assert_eq!(
            super::fixed_upper(upper, &raw, &runtime, retention).unwrap(),
            upper - 7 * footprint + copies * (7 * bundle - 3 * storage - 4 * carriers)
        );
    }
    // Three `if` roots beside three storage roots exceed five expressions.
    raw.exact_carrier_free_ifs = 3;
    for retention in [Retention::Parsed, Retention::Semantic] {
        assert!(super::fixed_upper(upper, &raw, &runtime, retention).is_err());
    }
    raw.exact_carrier_free_ifs = 0;
    raw.exact_carrier_free_roots = 4;
    for retention in [Retention::Parsed, Retention::Semantic] {
        assert!(super::fixed_upper(upper, &raw, &runtime, retention).is_err());
    }
    raw.exact_carrier_free_roots = 0;
    for malformed in [[3, 2, 1], [0, 0, 6], [usize::MAX, 1, 0]] {
        raw.exact_absent_roots = malformed;
        for retention in [Retention::Parsed, Retention::Semantic] {
            assert!(super::fixed_upper(upper, &raw, &runtime, retention)
                .unwrap_err()
                .iter()
                .all(|error| error.code == "SPX-G171"));
        }
    }
}
#[test]
fn census_counts_only_literal_unary_and_binary_roots_and_keeps_owned_children() {
    use crate::ast::{BinaryOp, Expr, ExprKind, Span, UnaryOp};
    assert_eq!(census("1", true), [1, 0, 0]);
    assert_eq!(census("-(7 / 2) + (1 - 3)", true), [4, 1, 3]);
    // Calls, variables, strings, blocks and `if` roots keep their storage;
    // their literal/unary/binary descendants are each counted once.
    let hostile = "let a = text(); if a == \"s\" && !flag { -(7 / 2) } else { (1 - 3) * f(2) }";
    assert_eq!(census(hostile, true), [5, 2, 5]);
    for body in ["1", "-(7 / 2) + (1 - 3)", hostile] {
        assert_eq!(census(body, false), [0, 0, 0], "legacy accumulators");
    }
    let leaf = || {
        Box::new(Expr {
            kind: ExprKind::Var("owned".to_owned()),
            span: Span::default(),
        })
    };
    let one = |kind: ExprKind| {
        let mut cost = super::super::StructuralCost::raw_ast(true).with_inline_values(true);
        cost.account_exact_absent_storage(&kind).unwrap();
        cost.exact_absent_roots
    };
    for op in [UnaryOp::Neg, UnaryOp::Not] {
        assert_eq!(one(ExprKind::Unary { op, value: leaf() }), [0, 1, 0]);
    }
    for op in [
        BinaryOp::Div,
        BinaryOp::Rem,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::Eq,
    ] {
        let kind = ExprKind::Binary {
            op,
            left: leaf(),
            right: leaf(),
        };
        assert_eq!(one(kind), [0, 0, 1]);
    }
    for kind in [
        ExprKind::String("owned".to_owned()),
        ExprKind::BorrowedUtf8Literal("view".to_owned()),
        ExprKind::ArrayU8(vec![1]),
        ExprKind::Var("owned".to_owned()),
        ExprKind::Call {
            name: "bytes_copy".to_owned(),
            type_arguments: vec![],
            args: vec![],
        },
        ExprKind::Block {
            statements: vec![],
            tail: leaf(),
        },
        ExprKind::If {
            condition: leaf(),
            then_branch: leaf(),
            else_branch: leaf(),
        },
    ] {
        assert_eq!(one(kind), [0, 0, 0]);
    }
}
#[test]
fn resolved_inventory_and_plan_hold_no_storage_for_any_censused_root() {
    use crate::cleanup::CleanupStorageOrigin;
    use crate::cleanup_plan::{CheckedOperation, StatusProducer, StorageId};
    use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind};
    let checked = crate::check(ABSENCE, "exact-absence.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&program).unwrap();
    let (mut census, mut lazy, mut owned) = ([0usize; 3], 0usize, 0usize);
    for function in &program.functions {
        let mut pending: Vec<&ResolvedExpr> = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect();
        let temporaries: Vec<_> = function
            .cleanup
            .slots
            .iter()
            .filter_map(|slot| match &slot.origin {
                CleanupStorageOrigin::Temporary { expression } => Some(expression),
                _ => None,
            })
            .collect();
        let planned: Vec<_> = function
            .cleanup_plan
            .slots
            .iter()
            .filter_map(|slot| match &slot.storage {
                StorageId::Temporary(expression)
                | StorageId::CallArgument {
                    value_expression: expression,
                    ..
                } => Some(expression),
                _ => None,
            })
            .collect();
        while let Some(expression) = pending.pop() {
            let root = match &expression.kind {
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_) => Some(0),
                ResolvedExprKind::Unary { value, .. } => {
                    pending.push(&**value);
                    Some(1)
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    lazy += usize::from(matches!(
                        op,
                        crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or
                    ));
                    pending.extend([&**left, &**right]);
                    Some(2)
                }
                ResolvedExprKind::Block { statements, tail } => {
                    for statement in statements {
                        pending.extend(
                            (0..statement.child_count()).filter_map(|index| statement.child(index)),
                        );
                    }
                    pending.push(&**tail);
                    None
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.extend([&**condition, &**then_branch, &**else_branch]);
                    None
                }
                ResolvedExprKind::Call { args, .. } => {
                    pending.extend(args);
                    None
                }
                _ => None,
            };
            let Some(root) = root else {
                owned += usize::from(temporaries.contains(&&expression.id));
                continue;
            };
            census[root] += 1;
            assert_eq!(
                expression.ownership,
                OwnershipMode::Value,
                "{}",
                expression.id
            );
            assert!(!temporaries.contains(&&expression.id), "{}", expression.id);
            assert!(!planned.contains(&&expression.id), "{}", expression.id);
        }
    }
    // Non-vacuous: every censused kind, both lazy operators, and an owned
    // String child holding real temporary storage were all observed.
    assert!(
        census[0] >= 11 && census[1] >= 2 && census[2] >= 12,
        "{census:?}"
    );
    assert!(lazy >= 3);
    assert!(owned >= 1);
    let main = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "exact.absence.main")
        .unwrap();
    for operation in [
        CheckedOperation::Neg,
        CheckedOperation::Add,
        CheckedOperation::Sub,
        CheckedOperation::Mul,
        CheckedOperation::Div,
        CheckedOperation::Rem,
    ] {
        assert!(main
            .cleanup_plan
            .status_sources
            .iter()
            .any(|source| matches!(
                &source.producer,
                StatusProducer::CheckedArithmetic { operation: actual, .. } if *actual == operation
            )));
    }
}
#[test]
fn absence_fixture_admits_exact_limit_and_refuses_one_short() {
    let programs = vec![crate::parse(ABSENCE, std::path::Path::new("exact-absence.spx")).unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let parsed = summed(&programs, &authored, Retention::Parsed).unwrap();
    let semantic = summed(&programs, &authored, Retention::Semantic).unwrap();
    // The second retained copy still charges every root's remaining bundle.
    assert!(semantic.0 > parsed.0);
    for (retention, full) in [(Retention::Parsed, parsed), (Retention::Semantic, semantic)] {
        {
            let _restore = limit(full.1);
            assert_eq!(summed(&programs, &authored, retention).unwrap(), full);
        }
        {
            let _restore = limit(full.1 - 1);
            assert!(summed(&programs, &authored, retention)
                .unwrap_err()
                .iter()
                .any(|error| error.code == "SPX-G171"));
        }
    }
}
