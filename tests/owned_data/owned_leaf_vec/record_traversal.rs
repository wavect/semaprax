//! Authored borrowed-row loops exercise existing read carriers on all engines.
use super::{run_wasm, DECLARATION, SEQUENCE};
use std::sync::atomic::Ordering;
#[path = "selected_record_traversal.rs"]
mod selected_record_traversal;

fn run(body: &str, expected: i64) {
    let source = format!("{DECLARATION}{body}");
    let path = std::env::temp_dir().join(format!(
        "record-traversal-{}-{}.spx",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, &source).unwrap();
    let result =
        semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        result
            .envelope
            .contains(&format!("\"value\":\"{expected}\"")),
        "{}",
        result.envelope
    );
    super::super::scoped_vec_field_native::run(&source, 0, expected);
    run_wasm(body, 0, expected, "none");
}

#[test]
fn owned_rows_preserve_ascending_indices_and_iteration_views_across_engines() {
    run(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(3usize);
 let first=vec_push<Row>(empty,Row{priority:7,title:"é\u{0}",payload:bytes_zeroed(3usize)});
 let second=vec_push<Row>(first,Row{priority:11,title:"x",payload:bytes_zeroed(1usize)});
 let rows=vec_push<Row>(second,Row{priority:13,title:"",payload:bytes_zeroed(0usize)});
 let mut order=0; let mut good=true;
 for borrow row at index in rows {
  let text=row.title; let data=row.payload; let utf8=str_as_bytes(row.title);
  order=order*10+i64_from_usize(index)+1;
  good=good && byte_len(data)==byte_len(utf8) && str_len_bytes(text)==i64_from_usize(byte_len(utf8));
  let scalar=row.priority;
  good=good && scalar>0;
  0
 }
 let renewed=vec_sort_owned<Row>(rows);
 if good && vec_len<Row>(renewed)==3usize && order==123 {29}else{0}
}"#,
        29,
    );
}

#[test]
fn repeated_singleton_loops_expire_views_before_owner_reentry() {
    run(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:17,title:"é",payload:bytes_zeroed(2usize)});
 let mut observed=0;
 for borrow row in rows { let text=row.title; if str_len_bytes(text)==2 {observed=observed+row.priority;} 0 }
 for borrow again at position in rows { let data=again.payload; if position==0usize && byte_len(data)==2usize {observed=observed+again.priority;} 0 }
 let sorted=vec_sort_owned<Row>(rows);
 if observed==34 && vec_len<Row>(sorted)==1usize {29}else{0}
}"#,
        29,
    );
}

// Row Loan v1: one authored helper, compiled once, reads lent frozen rows and an
// ordinary resident record through the versioned borrowed-record carrier. The
// native witness aborts on any allocation after the first field read, so a row,
// record or leaf copy in the loan path cannot pass. Arguments evaluate left to
// right, contracts read the lent row, and the guarded strict call stays lazy.
const ROW_LOAN: &str = r#"@id("app.weight") fn weight(row:borrow Row, bias:i64)->i64
 requires row.priority>0
 ensures result>bias
{ row.priority+bias }
@id("app.strict") fn strict(row:borrow Row)->i64
 requires row.priority>100
{ row.priority }
@id("app.main") fn main()->i64 {
 let resident=Row{priority:5,title:"r",payload:bytes_zeroed(1usize)};
 let empty=vec_with_capacity<Row>(3usize);
 let first=vec_push<Row>(empty,Row{priority:7,title:"é\u{0}",payload:bytes_zeroed(3usize)});
 let second=vec_push<Row>(first,Row{priority:11,title:"x",payload:bytes_zeroed(1usize)});
 let rows=vec_push<Row>(second,Row{priority:13,title:"",payload:bytes_zeroed(0usize)});
 let direct=weight(resident,0);
 let mut total=0;
 for borrow row at index in rows {
  total=total+weight(row,i64_from_usize(index));
  if index==99usize && strict(row)>0 {total=0;};
  0
 }
 let renewed=vec_sort_owned<Row>(rows);
 if total==34 && direct==5 && vec_len<Row>(renewed)==3usize {29}else{0}
}"#;

#[test]
fn row_loans_reach_one_scalar_reader_without_copies_across_engines() {
    run(ROW_LOAN, 29);
}

#[test]
fn row_loan_contract_failure_is_canonical_and_settles_the_frozen_owner() {
    let failing = ROW_LOAN.replace(
        "if index==99usize && strict(row)>0 {total=0;};",
        "if index==1usize && strict(row)>0 {total=0;};",
    );
    assert_ne!(failing, ROW_LOAN);
    let source = format!("{DECLARATION}{failing}");
    let interpret_failure = |source: &str| {
        let path = std::env::temp_dir().join(format!(
            "record-row-loan-failure-{}-{}.spx",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, source).unwrap();
        for _ in 0..3 {
            let result =
                semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default())
                    .unwrap();
            semaprax::interpreter::verify_envelope(&result.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            let outcome = &envelope["payload"]["outcome"];
            assert!(!result.returned);
            assert_eq!(outcome["status"]["domain_id"], "semaprax.contract.v1");
            assert_eq!(outcome["status"]["code"], 1);
            assert!(outcome.get("value").is_none());
        }
        std::fs::remove_file(path).unwrap();
    };
    interpret_failure(&source);
    // requires-false is semaprax.contract.v1 code 1. Native selects exactly that
    // status after the lent reads began, publishes no result, allocates nothing
    // and settles the frozen owner on every reentry; the Wasm host proves the same.
    super::super::scoped_vec_field_native::run_status(&source, 1, "semaprax.contract.v1", 0);
    super::run_wasm_status(
        &failing,
        1,
        0,
        "contract-view",
        Some("semaprax.contract.v1"),
    );
    // An ordinary scalar callee uses the same checked contract and transports
    // its selected status identically; lending must not change normalization.
    let scalar = failing
        .replace(
            "fn strict(row:borrow Row)->i64\n requires row.priority>100\n{ row.priority }",
            "fn strict(value:i64)->i64\n requires value>100\n{ value }",
        )
        .replace("strict(row)", "strict(row.priority)");
    assert_ne!(scalar, failing);
    interpret_failure(&format!("{DECLARATION}{scalar}"));
    super::super::scoped_vec_field_native::run_status(
        &format!("{DECLARATION}{scalar}"),
        1,
        "semaprax.contract.v1",
        0,
    );
    super::run_wasm_status(&scalar, 1, 0, "contract-view", Some("semaprax.contract.v1"));
}

#[test]
fn hostile_host_read_refuses_inside_a_row_loan_helper_before_publication() {
    // A hostile status or forged descriptor must stop the helper at its first
    // lent read (exactly one import call); the host proves teardown settles
    // every frozen owner. An i64 field has no malformed bit pattern to test.
    for refusal in ["field-unknown-status", "field-forged-shape"] {
        run_wasm(ROW_LOAN, 0, 29, refusal);
    }
}

#[test]
fn malformed_host_row_refuses_before_scalar_or_view_publication() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:7,title:"x",payload:bytes_zeroed(1usize)});
 let mut seen=0;
 for borrow row in rows { let text=row.title; seen=seen+str_len_bytes(text); 0 }
 seen
}"#,
        0,
        1,
        "field-bad-word",
    );
}

// One scalar and two owned fields have a normative maximum of 4,096 rows.
// Construction precedes the first observed read; the reused native probe rejects
// every malloc/calloc/realloc afterward and checks complete settlement on reentry.
#[test]
fn maximum_admitted_owned_rows_preserve_views_order_and_nonallocating_reentry() {
    const SOURCE: &str = include_str!("../../fixtures/record-traversal-maximum.spx");
    // Parsing a complete module requires a function. Keep the audited record
    // prefix exact by removing only this parser anchor before formatting.
    let mut prefix = semaprax::parse(
        &format!("{DECLARATION} @id(\"maximum.prefix.anchor\") fn anchor()->i64{{0}}"),
        "maximum-row-declaration.spx",
    )
    .unwrap();
    assert_eq!(prefix.functions.len(), 1);
    assert_eq!(prefix.functions[0].stable_id, "maximum.prefix.anchor");
    prefix.functions.clear();
    let declaration = semaprax::format::canonical(&prefix);
    let body = SOURCE
        .strip_prefix(&declaration)
        .expect("exact canonical owning Row declaration, including stable fields and types");
    let expected: i64 = (0..4096).sum();
    assert_eq!(expected, 8_386_560);
    assert_eq!(format!("{declaration}{body}"), SOURCE);
    selected_record_traversal::run(SOURCE, expected);
}

// Uses the complete maximum witness's functional oracle at the unchanged Byte event cap.
#[test]
fn finite_fresh_rows_preserve_utf8_nul_and_settle_across_all_engines() {
    const SOURCE: &str = include_str!("../../fixtures/record-traversal-maximum.spx");
    let body = SOURCE.split_once("@id(\"app.main\")").unwrap().1;
    for count in [1, 8, 32] {
        let body = format!("@id(\"app.main\"){body}").replace("4096", &count.to_string());
        let body = body.replace("{ total } else { 0 }", "{ total + 17 } else { -1 }");
        run(&body, (0..count).sum::<i64>() + 17);
    }
}
#[test]
fn finite_fresh_rows_preserve_first_failure_without_result_or_leaked_owner() {
    const SOURCE: &str = include_str!("../../fixtures/record-traversal-maximum.spx");
    let body = SOURCE.split_once("@id(\"app.main\")").unwrap().1;
    let body = format!("@id(\"app.main\"){body}").replace("4096", "8");
    run_wasm(&body, 15, 0, "push-allocation");
    for mode in ["codec-push-2", "codec-push-4"] {
        run_wasm(&body, 15, 0, mode);
    }
    let terminal = format!(
        "@id(\"app.main\"){}",
        SOURCE.split_once("@id(\"app.main\")").unwrap().1
    )
    .replace("4096", "4");
    run_wasm(&terminal, 15, 0, "codec-push-4");
    run_wasm(&body, 0, 1, "field-bad-word");
}

// Complete original direct-index fixture remains a refusal control: a
// traversal field read authenticates the current iteration index.
const PREVIOUS_INDEXED_DIRECT: &str = r#"@id("app.main") fn main()->i64 {
 let rows=vec_push<Row>(vec_push<Row>(vec_with_capacity<Row>(2usize),Row{priority:7,title:"a",payload:bytes_zeroed(0usize)}),Row{priority:11,title:"b",payload:bytes_zeroed(0usize)});
 let mut good=true;
 for borrow row at index in rows {
  let current=row.title;
  if index>0usize { let previous=vec_field<Row>(rows,index-1usize,"title"); let left=str_as_bytes(previous); let right=str_as_bytes(current);
   let unequal=match byte_get(left,0usize) { Option::Some { value:a } => match byte_get(right,0usize) { Option::Some { value:b } => a!=b && a==97u8 && b==98u8, Option::None {} => false, }, Option::None {} => false, };
   good=good && unequal && byte_len(left)==1usize && byte_len(right)==1usize; }
  0
 }
 let renewed=vec_sort_owned<Row>(rows);
 if good && vec_len<Row>(renewed)==2usize {29}else{0}
}"#;

#[test]
fn owned_row_traversal_keeps_previous_indexed_string_view_scoped_to_iteration() {
    // Both byte views expire in this shared-borrow helper call within the
    // iteration. Preserve exact a/b bytes, inequality, lengths and owner reentry.
    let admitted = format!(
        r#"@id("app.previous_diff") fn previous_diff(rows:borrow Vec<Row>, index:usize, current:borrow str)->bool {{
 let previous=vec_field<Row>(rows,index-1usize,"title"); let left=str_as_bytes(previous); let right=str_as_bytes(current);
 let unequal=match byte_get(left,0usize) {{ Option::Some {{ value:a }} => match byte_get(right,0usize) {{ Option::Some {{ value:b }} => a!=b && a==97u8 && b==98u8, Option::None {{}} => false, }}, Option::None {{}} => false, }};
 unequal && byte_len(left)==1usize && byte_len(right)==1usize
}}
{}"#,
        PREVIOUS_INDEXED_DIRECT.replace(
            r#"if index>0usize { let previous=vec_field<Row>(rows,index-1usize,"title"); let left=str_as_bytes(previous); let right=str_as_bytes(current);
   let unequal=match byte_get(left,0usize) { Option::Some { value:a } => match byte_get(right,0usize) { Option::Some { value:b } => a!=b && a==97u8 && b==98u8, Option::None {} => false, }, Option::None {} => false, };
   good=good && unequal && byte_len(left)==1usize && byte_len(right)==1usize; }"#,
            "if index>0usize { good=good && previous_diff(rows,index,current); }",
        ),
    );
    assert_ne!(admitted, PREVIOUS_INDEXED_DIRECT);
    run(&admitted, 29);
}

#[test]
fn direct_previous_indexed_read_retains_the_exact_traversal_index_refusal() {
    let source = format!("{DECLARATION}{PREVIOUS_INDEXED_DIRECT}");
    let parsed = semaprax::check(&source, "previous-indexed-direct.spx").unwrap();
    let mut resolved = semaprax::hir::resolve(&parsed).unwrap();
    semaprax::hir::validate(&resolved).unwrap();
    fn forge(expression: &mut semaprax::hir::ResolvedExpr) -> bool {
        use semaprax::hir::{
            ResolvedExprKind as K, ResolvedStatement as S, VecFieldReadOrigin as O,
        };
        if let K::VecFieldRead {
            origin: O::Explicit,
            args,
            ..
        } = &expression.kind
        {
            if matches!(args.get(1).map(|arg| &arg.kind), Some(K::Binary { .. })) {
                let K::VecFieldRead { origin, .. } = &mut expression.kind else {
                    unreachable!()
                };
                *origin = O::Traversal;
                return true;
            }
        }
        match &mut expression.kind {
            K::Block { statements, tail } => {
                for statement in statements {
                    match statement {
                        S::While {
                            condition, body, ..
                        }
                        | S::LexicalWhile {
                            condition, body, ..
                        } => {
                            if forge(condition) || forge(body) {
                                return true;
                            }
                        }
                        _ => {
                            if forge(statement.value_mut()) {
                                return true;
                            }
                        }
                    }
                }
                forge(tail)
            }
            K::If {
                then_branch,
                else_branch,
                ..
            } => forge(then_branch) || forge(else_branch),
            _ => false,
        }
    }
    let main = resolved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert!(forge(&mut main.body));
    let diagnostic = semaprax::hir::validate(&resolved).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains(
        "record traversal length, index, nominal reads or whole-root freeze proof is invalid"
    ));
}

#[test]
fn ordinary_previous_row_lookup_composes_with_byte_predicate_and_lexical_exits() {
    run(
        r#"@id("app.sentinel") fn sentinel()->bool requires false { true }
@id("app.ascii") fn ascii(text:borrow str)->bool {
 let bytes=str_as_bytes(text);
 for byte in bytes {
  if byte<65u8 || byte>90u8 { return false; };
  if byte==63u8 { let unreachable=sentinel(); 0 };
  0
 }
 true
}
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(3usize);
 let first=vec_push<Row>(empty,Row{priority:7,title:"A",payload:bytes_zeroed(0usize)});
 let second=vec_push<Row>(first,Row{priority:11,title:"B",payload:bytes_zeroed(0usize)});
 let rows=vec_push<Row>(second,Row{priority:13,title:"?",payload:bytes_zeroed(0usize)});
 let mut order=0; let mut seen=0;
 for borrow row at index in rows {
  order=order*10+i64_from_usize(index)+1;
  let current=row.title;
  if index==0usize { continue; };
  let previous=vec_field<Row>(rows,index-1usize,"title");
  if ascii(current) && str_len_bytes(previous)==1 { seen=seen+1; };
  if index==2usize { break; };
  0
 }
 let renewed=vec_sort_owned<Row>(rows);
 if order==123 && seen==1 && vec_len<Row>(renewed)==3usize {29}else{0}
}"#,
        29,
    );
}

#[test]
fn ordinary_lookup_failure_inside_traversal_keeps_sticky_status_and_cleanup() {
    let body = r#"@id("app.sentinel") fn sentinel()->i64 requires false { 0 }
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:7,title:"A",payload:bytes_zeroed(0usize)});
 for borrow row in rows {
  let current=row.title;
  let previous=vec_field<Row>(rows,1usize,"title");
  let never=sentinel();
  str_len_bytes(current)+str_len_bytes(previous)+never
 }
 0
}"#;
    let source = format!("{DECLARATION}{body}");
    let path = std::env::temp_dir().join(format!(
        "field-origin-failure-{}-{}.spx",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, &source).unwrap();
    let result =
        semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    std::fs::remove_file(path).unwrap();
    semaprax::interpreter::verify_envelope(&result.envelope).unwrap();
    assert!(!result.returned);
    let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    let outcome = &wire["payload"]["outcome"];
    assert_eq!(outcome["status"]["domain_id"], "semaprax.vec.v1");
    assert_eq!(outcome["status"]["code"], 2);
    assert!(outcome.get("value").is_none());
    super::super::scoped_vec_field_native::run_status(&source, 2, "semaprax.vec.v1", 0);
    super::run_wasm_status(body, 2, 0, "vec-view", Some("semaprax.vec.v1"));
}
