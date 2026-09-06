use std::path::Path;

use semaprax::cleanup::FieldLivenessShape;
use semaprax::cleanup_plan::{CleanupTransition, EdgeCondition, StorageId};
use semaprax::hir::{self, OwnershipMode, ResolvedExprKind, ResolvedStatement, ResolvedType};
use semaprax::{format, graph, parse, verify};

const SOURCE: &str = r#"
module test.owned_bounded_vec;

@id("app.main")
fn main() -> i64 {
    let mut values = vec_with_capacity<i64>(4usize);
    values = vec_push<i64>(values, 7);
    let length = vec_len<i64>(values);
    let capacity = vec_capacity<i64>(values);
    if length < capacity { vec_get<i64>(values, 0usize) } else { 0 }
}
"#;

fn parse_source(source: &str) -> semaprax::ast::Program {
    parse(source, Path::new("owned-bounded-vec-v1.spx")).unwrap()
}

fn error_codes(source: &str) -> Vec<&'static str> {
    verify::verify(&parse_source(source))
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn main_function_mut(program: &mut hir::ResolvedProgram) -> &mut hir::ResolvedFunction {
    program
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap()
}

#[test]
fn owned_bounded_vec_has_exact_source_hir_formatter_and_graph_identity() {
    let program = parse_source(SOURCE);
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("vec_with_capacity<i64>(4usize)"));
    assert!(canonical.contains("vec_push<i64>(values, 7)"));
    let reparsed = parse_source(&canonical);
    let resolved = hir::resolve(&reparsed).unwrap();
    let facts = resolved
        .declarations
        .type_facts(&ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("core.vec"),
            arguments: vec![ResolvedType::I64],
        })
        .unwrap();
    assert!(!facts.copy && facts.needs_drop && facts.sized && !facts.contains_resource);
    let function = resolved
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("let")
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        ..
    } = &value.kind
    else {
        panic!("call")
    };
    assert_eq!(callee.as_str(), "core.vec.with-capacity");
    assert_eq!(type_arguments, &[ResolvedType::I64]);
    assert!(instance.is_none());
    assert_eq!(value.ownership, OwnershipMode::Own);
    let rendered = graph::to_json(&reparsed).unwrap();
    assert!(rendered.contains("semaprax.prelude.v2"));
    let scalar = parse_source("module test.scalar; @id(\"app.main\") fn main()->i64{0}");
    let scalar_graph = graph::to_json(&scalar).unwrap();
    assert!(scalar_graph.contains("semaprax.prelude.v1"));
    assert!(!scalar_graph.contains("semaprax.prelude.v2"));
    assert!(!scalar_graph.contains("core.vec"));
    let declaration_only = parse_source(
        "module test.vec_type; @id(\"test.holder\") fn hold()->Vec<i64>{vec_with_capacity<i64>(0usize)} @id(\"app.main\") fn main()->i64{0}",
    );
    let declaration_graph = graph::to_json(&declaration_only).unwrap();
    assert!(declaration_graph.contains("semaprax.prelude.v2"));
    assert!(declaration_graph.contains("core.vec"));
    assert!(!scalar_graph.contains("core.vec"));
    assert_ne!(graph::revision(&scalar), graph::revision(&reparsed));
    assert!(rendered.contains("core.vec.with-capacity"));
    assert!(rendered.contains("core.vec.push"));
}

#[test]
fn vec_profile_rejects_inference_owned_elements_and_oversized_capacity() {
    assert!(
        error_codes(&SOURCE.replace("vec_with_capacity<i64>", "vec_with_capacity"))
            .contains(&"SPX-T281")
    );
    assert!(error_codes(&SOURCE.replace("<i64>", "<Bytes>")).contains(&"SPX-T281"));
    assert!(error_codes(&SOURCE.replace("4usize", "8193usize")).contains(&"SPX-T282"));

    let dynamic_capacity = SOURCE.replace(
        "let mut values = vec_with_capacity<i64>(4usize);",
        "let requested = 8193usize;\n    let mut values = vec_with_capacity<i64>(requested);",
    );
    assert!(error_codes(&dynamic_capacity).is_empty());
    hir::resolve(&parse_source(&dynamic_capacity)).unwrap();

    assert!(error_codes(
        "module test.vec_name; @id(\"test.fake\") fn vec_push()->i64{0} @id(\"app.main\") fn main()->i64{0}"
    )
    .contains(&"SPX-S113"));
    assert!(error_codes(
        "module test.vec_id; @id(\"core.vec.push\") fn fake()->i64{0} @id(\"app.main\") fn main()->i64{0}"
    )
    .contains(&"SPX-S113"));
}

#[test]
fn hir_validation_rejects_a_forged_vec_element_identity() {
    let mut resolved = hir::resolve(&parse_source(SOURCE)).unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("let")
    };
    let ResolvedExprKind::Call { type_arguments, .. } = &mut value.kind else {
        panic!("call")
    };
    type_arguments[0] = ResolvedType::Bytes;
    assert_eq!(hir::validate(&resolved).unwrap_err().code, "SPX-H006");

    let mut reserved_identity = hir::resolve(&parse_source(SOURCE)).unwrap();
    reserved_identity
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap()
        .id = hir::DeclarationId::new("core.vec.push");
    assert_eq!(
        hir::validate(&reserved_identity).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn cleanup_replay_rejects_forged_vec_lifecycle_staging_commit_and_status_order() {
    let baseline = hir::resolve(&parse_source(SOURCE)).unwrap();
    hir::validate(&baseline).unwrap();

    let mut wrong_lifecycle = baseline.clone();
    let lifecycle = main_function_mut(&mut wrong_lifecycle)
        .cleanup_plan
        .slots
        .iter_mut()
        .find_map(|slot| match &mut slot.field_liveness_shape {
            FieldLivenessShape::Leaf { lifecycle, .. } => Some(lifecycle),
            _ => None,
        })
        .expect("Vec cleanup plan has an owned leaf");
    *lifecycle = hir::DeclarationId::new("hostile.vec.drop");
    assert_eq!(
        hir::validate(&wrong_lifecycle).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_staging = baseline.clone();
    let staged = main_function_mut(&mut wrong_staging)
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find_map(|transition| match transition {
            CleanupTransition::Transfer { destination, .. }
                if matches!(destination.storage, StorageId::CallArgument { .. }) =>
            {
                Some(destination)
            }
            _ => None,
        })
        .expect("vec_push stages its owned argument");
    let StorageId::CallArgument {
        parameter_index, ..
    } = &mut staged.storage
    else {
        unreachable!()
    };
    *parameter_index = 1;
    assert_eq!(hir::validate(&wrong_staging).unwrap_err().code, "SPX-H006");

    let mut wrong_commit = baseline.clone();
    let commit = main_function_mut(&mut wrong_commit)
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find_map(|transition| match transition {
            CleanupTransition::CallCommit { arguments, .. } if !arguments.is_empty() => {
                Some(arguments)
            }
            _ => None,
        })
        .expect("vec_push has an owned call commit");
    commit[0].parameter_index = 1;
    assert_eq!(hir::validate(&wrong_commit).unwrap_err().code, "SPX-H006");

    let mut wrong_status_order = baseline;
    let edges = &mut main_function_mut(&mut wrong_status_order)
        .cleanup_plan
        .edges;
    let zero = edges
        .iter()
        .position(|edge| matches!(edge.condition, EdgeCondition::StatusZero(_)))
        .expect("Vec operation has a success edge");
    let nonzero = edges
        .iter()
        .position(|edge| matches!(edge.condition, EdgeCondition::StatusNonzero(_)))
        .expect("Vec operation has a failure edge");
    let zero_condition = edges[zero].condition.clone();
    edges[zero].condition = edges[nonzero].condition.clone();
    edges[nonzero].condition = zero_condition;
    assert_eq!(
        hir::validate(&wrong_status_order).unwrap_err().code,
        "SPX-H006"
    );
}
