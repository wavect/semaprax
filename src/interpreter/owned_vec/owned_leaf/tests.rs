use crate::interpreter::*;
use std::collections::BTreeMap;

#[test]
fn second_owned_leaf_clone_failure_retains_first_selected_budget_status() {
    let source = r#"module leaf.clone_failure;
@id("pair") record Pair { @id("pair.left") left:string, @id("pair.right") right:string, }
@id("entry") fn main()->i64 {
 let pair=Pair{left:"a",right:"b"};
 let rows=vec_push<Pair>(vec_with_capacity<Pair>(1usize),pair);
 let clone=vec_clone_at<Pair>(rows,0usize);
 0
}"#;
    let ast = crate::check(source, "clone-failure.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let entry = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "entry")
        .unwrap();
    let admitted = program
        .functions
        .iter()
        .map(|f| (f.id.as_str(), f))
        .collect::<BTreeMap<_, _>>();
    let (outcome, _, _, usage) = evaluate_resolved_entry_with_utf8_budget(
        entry,
        &[],
        &admitted,
        &program,
        1_000_000,
        false,
        Utf8MaterializationBudget::Fixed {
            used_materializations: 0,
            used_bytes: MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES - 3,
        },
    );
    assert!(
        matches!(outcome,Err(Flow::Utf8MaterializationLimitExceeded{attempted_materializations:4,attempted_bytes}) if attempted_bytes==MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES+1)
    );
    assert_eq!(usage, (3, MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES));
}

#[test]
fn clone_out_returns_independent_bytes_before_original_carrier_is_cleared() {
    let source = r#"module leaf.clone_independent;
@id("row") record Row { @id("row.bytes") bytes:Bytes, @id("row.text") text:string, }
@id("entry") fn main()->i64 {
 let input=[9u8];
 let row=Row{bytes:bytes_copy(array_as_slice(input)),text:"independent"};
 let mut rows=vec_push<Row>(vec_with_capacity<Row>(1usize),row);
 let cloned=vec_clone_at<Row>(rows,0usize);
 rows=vec_clear<Row>(rows);
 match own cloned { Row{bytes,text}=>{
   let n={let view=bytes_as_slice(bytes);byte_len(view)};
   if n==1usize && text=="independent" && vec_len<Row>(rows)==0usize {0}else{1}
 }, }
}"#;
    let ast = crate::check(source, "clone-independent.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let outcome = evaluate_resolved_zero_arg_i64(&program, "entry", 1_000_000).unwrap();
    assert!(
        matches!(outcome.outcome, ResolvedEvaluationOutcome::ReturnedI64(0)),
        "{:?}",
        outcome.outcome
    );
    let bytes = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "entry")
        .unwrap();
    let summaries = crate::hir::analyze_byte_data_capacity(&program).unwrap();
    let summary = summaries.function(bytes.id.as_str()).unwrap();
    assert_eq!(summary.bytes_copy_sites, 2);
}
