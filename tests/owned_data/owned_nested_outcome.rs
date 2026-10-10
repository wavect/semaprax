//! Proper tagged nested success/error, exact physical settlement, and partial failure.
use semaprax::interpreter::{self, InterpreterOptions};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
const SOURCE: &str = include_str!("../fixtures/owned-nested-outcome.spx");
static NEXT: AtomicU64 = AtomicU64::new(0);
fn directory(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "nested-outcome-{}-{label}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}
fn source(body: &str, helpers: &str) -> String {
    format!(
        "{}{helpers}@id(\"app.main\") fn main()->i64 {{{body}}}",
        SOURCE.split("@id(\"app.main\")").next().unwrap()
    )
}

#[test]
fn nested_outcome_borrow_forward_move_and_error_settle_on_three_backends() {
    for (label,source) in [("success",SOURCE.to_owned()),("error-only",source("let rejected=Outcome::Error{code:42,offset:0usize,field:0};finish(forward(rejected))",""))] {
  let root=directory(label);let path=root.join("app.spx");std::fs::write(&path,&source).unwrap();let ast=semaprax::check(&source,&path).unwrap();
  for _ in 0..3 {let result=interpreter::interpret(&path,"app.main",&[],&InterpreterOptions::default()).unwrap();interpreter::verify_envelope(&result.envelope).unwrap();assert!(result.returned,"{}",result.envelope);let wire:serde_json::Value=serde_json::from_str(&result.envelope).unwrap();assert_eq!(wire["payload"]["outcome"]["value"],"42");}
  super::owned_collection_outcome::run_native(&ast,&root);super::owned_collection_outcome::run_strict_wasm(&ast,&root);std::fs::remove_dir_all(root).unwrap();
 }
}

#[test]
fn nested_outcome_partial_record_staging_callee_and_postcondition_keep_first_failure() {
    let helpers = r#"
@id("n.boom") fn boom()->i64 {9223372036854775807+1}
@id("n.consume") fn consume(value:own Outcome,marker:i64)->i64 {boom()}
@id("n.guard") fn guard(value:own Outcome)->Outcome ensures false {value}
"#;
    for (label,body) in [
  ("partial", "let items=vec_push<string>(vec_with_capacity<string>(1usize),\"kept\");let result=Outcome::Ready{value:Payload{items:items,config:Config{label:\"prefix\",seed:boom()},bytes:bytes_zeroed(3usize)}};0"),
  ("staging", "let value=make(true);consume(value,boom())"),
  ("callee", "let value=make(true);consume(value,1)"),
  ("postcondition", "let value=make(true);let unpublished=guard(value);0"),
 ] {
  let source=source(body,helpers);let root=directory(label);let path=root.join("app.spx");std::fs::write(&path,&source).unwrap();let ast=semaprax::check(&source,&path).unwrap();
  let contract=label=="postcondition";let domain=if contract{"semaprax.contract.v1"}else{"semaprax.arithmetic.v1"};let code=if contract{2}else{1};
  for _ in 0..3 {let result=interpreter::interpret(&path,"app.main",&[],&InterpreterOptions::default()).unwrap();assert!(!result.returned,"{}",result.envelope);interpreter::verify_envelope(&result.envelope).unwrap();assert!(result.envelope.contains(&format!("\"domain_id\":\"{domain}\"")));assert!(result.envelope.contains(&format!("\"code\":{code}")));}
  super::nested_collection_record::failure::native_failure(&ast,&root,domain,code);
  let wasm=semaprax::wasm::emit_module(&ast).unwrap();wasmparser::Validator::new().validate_all(&wasm).unwrap();let module=root.join("app.wasm");std::fs::write(&module,wasm).unwrap();
  let output=Command::new("node").arg("-e").arg(include_str!("owned_leaf_vec/host.js")).arg(module).args([if contract{"10"}else{"1"},"0","prefix-failure"]).output().unwrap();assert!(output.status.success(),"{label}: {}",String::from_utf8_lossy(&output.stderr));std::fs::remove_dir_all(root).unwrap();
 }
}

#[test]
fn nested_outcome_error_case_never_constructs_a_dummy_record() {
    let source = source(
        "let value=Outcome::Error{code:42,offset:0usize,field:0};finish(forward(value))",
        "",
    );
    let root = directory("allocation-free-error");
    let ast = semaprax::check(&source, "error.spx").unwrap();
    let generated = semaprax::codegen::emit_c(&ast)
        .unwrap()
        .replace("malloc(", "deny_alloc(")
        .replace("calloc(", "deny_pair(")
        .replace("realloc(", "deny_resize(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC deny_resize",
        );
    let allocator = r#"#include <stdlib.h>
static void *deny_alloc(size_t n){(void)outcome_malloc;(void)n;abort();}
static void *deny_pair(size_t n,size_t s){(void)outcome_calloc;(void)n;(void)s;abort();}
static void *deny_resize(void *p,size_t n){(void)outcome_realloc;(void)p;(void)n;abort();}
"#;
    // Reuse the ordinary physical ownership and status probe, but prohibit every
    // allocation before the function can fabricate an otherwise invisible Root.
    super::owned_collection_outcome::run_native_generated(
        format!("{allocator}\n{generated}"),
        &root,
    );
    super::owned_collection_outcome::run_strict_wasm(&ast, &root);
    std::fs::remove_dir_all(root).unwrap();
}
