use super::nested_owned::{
    graph_schema_includes_loans, graph_schema_includes_modern_composite_facts,
    graph_schema_includes_projected_provenance, reject_nested_native_flags,
};
use std::path::Path;

const DESTRUCTURE_SOURCE: &str = r#"
module test.graph_nested_destructure;
@id("graph.leaf") record Leaf { @id("graph.leaf.payload") payload: Bytes, }
@id("graph.outer") record Outer { @id("graph.outer.leaf") leaf: Leaf, }
@id("graph.take") fn take(value: own Outer) -> i64 {
  match own value { Outer { leaf: Leaf { payload } } => 0, }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn destructure_program(source: &str) -> crate::hir::ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(source, Path::new("graph-nested-destructure.spx"))
            .expect("nested destructure source parses"),
    )
    .expect("nested destructure source resolves")
}

#[test]
fn nested_cleanup_versions_are_closed_and_legacy_selection_is_unchanged() {
    assert!(reject_nested_native_flags(false, false).is_ok());
    assert!(reject_nested_native_flags(false, true).is_ok());
    assert!(reject_nested_native_flags(true, false).is_ok());
    assert!(reject_nested_native_flags(true, true).is_err());

    for schema in [
        "semaprax.graph.v26",
        "semaprax.graph.v27",
        "semaprax.graph.v28",
        "semaprax.graph.v29",
        "semaprax.graph.v30",
        "semaprax.graph.v31",
    ] {
        assert!(graph_schema_includes_modern_composite_facts(schema));
    }
    assert!(!graph_schema_includes_loans("semaprax.graph.v26"));
    assert!(graph_schema_includes_loans("semaprax.graph.v27"));
    assert!(!graph_schema_includes_loans("semaprax.graph.v28"));
    assert!(graph_schema_includes_loans("semaprax.graph.v29"));
    assert!(!graph_schema_includes_loans("semaprax.graph.v30"));
    assert!(graph_schema_includes_loans("semaprax.graph.v31"));
    assert!(!graph_schema_includes_projected_provenance(
        "semaprax.graph.v26"
    ));
    assert!(graph_schema_includes_projected_provenance(
        "semaprax.graph.v27"
    ));
    assert!(!graph_schema_includes_projected_provenance(
        "semaprax.graph.v28"
    ));
    assert!(graph_schema_includes_projected_provenance(
        "semaprax.graph.v29"
    ));
    assert!(!graph_schema_includes_projected_provenance(
        "semaprax.graph.v30"
    ));
    assert!(graph_schema_includes_projected_provenance(
        "semaprax.graph.v31"
    ));
    for schema in [
        "semaprax.graph.v25",
        "semaprax.graph.v30 ",
        "semaprax.graph.v27 ",
    ] {
        assert!(!graph_schema_includes_modern_composite_facts(schema));
        assert!(!graph_schema_includes_loans(schema));
        assert!(!graph_schema_includes_projected_provenance(schema));
    }
}

#[test]
fn v28_and_v29_are_selected_only_for_exact_nested_destructure_compositions() {
    let plain = destructure_program(DESTRUCTURE_SOURCE);
    assert_eq!(super::graph_schema(&plain).unwrap(), "semaprax.graph.v28");
    let json = super::to_hir_json(&plain, "sha256:test").unwrap();
    assert!(json.contains("\"schema\":\"semaprax.graph.v28\""));
    assert!(json.contains("\"field\":\"graph.outer.leaf\""));
    assert!(json.contains("\"field\":\"graph.leaf.payload\""));

    let with_loan = DESTRUCTURE_SOURCE.replace(
        "@id(\"app.main\")",
        r#"@id("graph.inspect") fn inspect(value: own Outer) -> usize {
  let view = bytes_as_slice(value.leaf.payload);
  byte_len(view)
}
@id("app.main")"#,
    );
    let mut with_loan = destructure_program(&with_loan);
    assert_eq!(
        super::graph_schema(&with_loan).unwrap(),
        "semaprax.graph.v29"
    );
    let json = super::to_hir_json(&with_loan, "sha256:test").unwrap();
    assert!(json.contains("\"schema\":\"semaprax.graph.v29\""));
    assert!(json.contains("graph.outer.leaf"));
    assert!(json.contains("graph.leaf.payload"));

    for function in &mut with_loan.functions {
        for loan in &mut function.loan_plan.loans {
            loan.origin.projections.clear();
        }
    }
    assert_eq!(
        super::graph_schema(&with_loan).unwrap_err().code,
        "SPX-G410"
    );
}

#[test]
fn one_valid_nested_loan_cannot_mask_an_invalid_sibling_loan() {
    let source = DESTRUCTURE_SOURCE.replace(
        "@id(\"app.main\")",
        r#"@id("graph.inspect") fn inspect(value: own Outer) -> usize {
  let view = bytes_as_slice(value.leaf.payload);
  byte_len(view)
}
@id("app.main")"#,
    );
    let program = destructure_program(&source);
    assert_eq!(super::graph_schema(&program).unwrap(), "semaprax.graph.v29");
    assert_eq!(
        super::graph_schema_from_parts_and_instances(
            &program.interfaces,
            &program.types,
            &program.functions,
            &program.function_templates,
            &program.function_instances,
        )
        .unwrap(),
        "semaprax.graph.v29"
    );

    let inspect = program
        .functions
        .iter()
        .position(|function| function.id.as_str() == "graph.inspect")
        .expect("loan-bearing fixture function exists");
    let valid = program.functions[inspect].loan_plan.loans[0].clone();

    let mut short = program.clone();
    let mut invalid = valid.clone();
    invalid.origin.projections.clear();
    short.functions[inspect].loan_plan.loans.push(invalid);
    assert_eq!(super::graph_schema(&short).unwrap_err().code, "SPX-G410");
    assert_eq!(
        super::graph_schema_from_parts_and_instances(
            &short.interfaces,
            &short.types,
            &short.functions,
            &short.function_templates,
            &short.function_instances,
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );

    let mut foreign = program;
    let mut invalid = valid;
    invalid.origin.root = foreign.functions[inspect].result_id.clone();
    foreign.functions[inspect].loan_plan.loans.push(invalid);
    assert_eq!(super::graph_schema(&foreign).unwrap_err().code, "SPX-G410");
    assert_eq!(
        super::graph_schema_from_parts_and_instances(
            &foreign.interfaces,
            &foreign.types,
            &foreign.functions,
            &foreign.function_templates,
            &foreign.function_instances,
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );

    let two_roots = r#"
module test.graph_nested_two_roots;
@id("graph.two.leaf") record Leaf { @id("graph.two.leaf.payload") payload: Bytes, }
@id("graph.two.pair") record Pair {
  @id("graph.two.pair.left") left: Leaf,
  @id("graph.two.pair.right") right: Leaf,
}
@id("graph.two.take") fn take(value: own Pair) -> i64 {
  match own value {
    Pair { left: Leaf { payload: left }, right: Leaf { payload: right } } => 0,
  }
}
@id("graph.two.inspect") fn inspect(value: own Pair) -> usize {
  let view = bytes_as_slice(value.left.payload);
  byte_len(view)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let mut two_roots = destructure_program(two_roots);
    assert_eq!(
        super::graph_schema(&two_roots).unwrap(),
        "semaprax.graph.v29"
    );
    let inspect = two_roots
        .functions
        .iter()
        .position(|function| function.id.as_str() == "graph.two.inspect")
        .expect("two-root loan-bearing fixture function exists");
    let mut unattached = two_roots.functions[inspect].loan_plan.loans[0].clone();
    unattached.id = crate::loan_plan::LoanId(1);
    unattached.origin.projections[0] =
        crate::hir::PlaceProjection::Field(crate::hir::DeclarationId::new("graph.two.pair.right"));
    two_roots.functions[inspect]
        .loan_plan
        .loans
        .push(unattached);
    assert_eq!(
        super::graph_schema(&two_roots).unwrap_err().code,
        "SPX-G410",
        "a valid loan must not authenticate a structurally valid but unattached sibling",
    );
    assert_eq!(
        super::graph_schema_from_parts_and_instances(
            &two_roots.interfaces,
            &two_roots.types,
            &two_roots.functions,
            &two_roots.function_templates,
            &two_roots.function_instances,
        )
        .unwrap(),
        "semaprax.graph.v29",
        "the parts-only classifier is limited to exact structural evidence",
    );
}

#[test]
fn native_v25_cannot_mask_v28_or_v29() {
    let mut program = destructure_program(DESTRUCTURE_SOURCE);
    let native = crate::hir::resolve(
        &crate::parse(
            r#"module test.native;
@id("native.host") interface Host permits {} {
  @id("native.ping") import rust fn ping(value:i64)->i64 effects {} failure infallible;
}
@id("app.main") fn main()->i64 {0}"#,
            Path::new("graph-nested-destructure-native.spx"),
        )
        .unwrap(),
    )
    .unwrap();
    program.interfaces = native.interfaces;
    assert_eq!(super::graph_schema(&program).unwrap_err().code, "SPX-G410");
}

#[test]
fn nested_update_selects_v30_or_universally_authenticated_v31() {
    let source = r#"
module test.graph_nested_update;
@id("update.leaf") record Leaf { @id("update.leaf.payload") payload: Bytes, }
@id("update.pair") record Pair {
  @id("update.pair.left") left: Leaf,
  @id("update.pair.right") right: Leaf,
}
@id("update.apply") fn apply(value: own Pair, replacement: own Leaf) -> Pair {
  value with { left: replacement }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let plain = destructure_program(source);
    assert_eq!(super::graph_schema(&plain).unwrap(), "semaprax.graph.v30");
    let with_loan = source.replace(
        "@id(\"app.main\")",
        r#"@id("update.inspect") fn inspect(value: own Pair) -> usize {
  let view = bytes_as_slice(value.right.payload);
  byte_len(view)
}
@id("app.main")"#,
    );
    let mut with_loan = destructure_program(&with_loan);
    assert_eq!(
        super::graph_schema(&with_loan).unwrap(),
        "semaprax.graph.v31"
    );
    let function = with_loan
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "update.inspect")
        .expect("loan-bearing update companion exists");
    let mut invalid = function.loan_plan.loans[0].clone();
    invalid.origin.projections.clear();
    function.loan_plan.loans.push(invalid);
    assert_eq!(
        super::graph_schema(&with_loan).unwrap_err().code,
        "SPX-G410"
    );
}
