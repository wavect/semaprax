//! Same source, actual slot providers, no payload copies, and caller loan expiry.
use semaprax::{codegen, interpreter, wasm};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
const SOURCE: &str = include_str!("../../fixtures/string-slot-views.spx");
static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(source: &str, failure: bool, mode: &str, allocations: Option<u64>) {
    let root = std::env::temp_dir().join(format!(
        "string-slots-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("app.spx");
    std::fs::write(&path, source).unwrap();
    let ast = semaprax::check(source, &path).expect("checked String-slot source");
    for _ in 0..3 {
        let result = interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
        interpreter::verify_envelope(&result.envelope).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        if failure {
            assert!(!result.returned, "{}", result.envelope);
            assert_eq!(
                wire["payload"]["outcome"]["status"]["domain_id"],
                "semaprax.vec.v1"
            );
            assert_eq!(wire["payload"]["outcome"]["status"]["code"], 2);
        } else {
            assert!(result.returned, "{}", result.envelope);
            assert_eq!(wire["payload"]["outcome"]["value"], "42");
        }
    }
    if failure {
        super::super::nested_collection_record::failure::native_failure(
            &ast,
            &root,
            "semaprax.vec.v1",
            2,
        );
    } else {
        let mut generated = codegen::emit_c(&ast).unwrap();
        if mode == "slot-no-copy" {
            let clone = "char *spx_string_clone(const char *spx_source) {";
            assert_eq!(generated.matches(clone).count(), 1);
            generated = generated.replacen(clone, &format!("{clone}\n    abort();"), 1);
        }
        // Each lexical-control function references its terminal flag at the
        // shared epilogue; functions without lexical control declare neither.
        assert_eq!(
            generated
                .matches("(void)spx_canonical_cleanup_done;")
                .count(),
            generated
                .matches("bool spx_canonical_cleanup_done = false;")
                .count()
        );
        super::super::owned_collection_outcome::run_native_generated_with_allocation_count(
            generated,
            &root,
            allocations,
        );
    }
    let module = root.join("app.wasm");
    std::fs::write(&module, wasm::emit_module(&ast).unwrap()).unwrap();
    let output = Command::new("node")
        .arg(super::HOST)
        .arg(&module)
        .args([
            if failure { "14" } else { "0" },
            if failure { "0" } else { "42" },
            mode,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{mode}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn borrowed_and_owned_slot_views_share_nul_utf8_storage_and_expire_before_owned_calls() {
    // One Vec backing and three original String owners. Reads and aliases
    // must not add allocations; all three invocations settle every owner.
    run(SOURCE, false, "slot-no-copy", Some(4));
    // Eq/Ne reads actual String slots through the same whole-Vec loan.
    // Distinct slot roots, repeated same-slot views and an alias retain the
    // original four allocations and all caller-side last-use transfers.
    let equality = SOURCE
        .replace("str_len_bytes(selected)==4 && str_is_empty(empty)", "str_len_bytes(selected)==4 && str_is_empty(empty) && selected==vec_str_at(values,0usize) && selected!=empty && empty==vec_str_at(values,1usize)")
        .replace("answer==42 &&", "answer==42 && view==alias &&");
    assert_ne!(equality, SOURCE);
    run(&equality, false, "slot-no-copy", Some(4));
    for mode in [
        "field-bad-word",
        "field-forged-shape",
        "field-unknown-status",
    ] {
        run(SOURCE, false, mode, Some(4));
    }
    let contracts = SOURCE
        .replace("fn inspect(values:borrow Vec<string>)->i64 {", "fn inspect(values:borrow Vec<string>,early:bool)->i64 ensures str_len_bytes(vec_str_at(values,0usize))==4 {")
        .replace("repeat=repeat+1usize;\n  0", "repeat=repeat+1usize;\n  if good && early && repeat==1usize {return 42;}else{0}\n  0")
        .replace("let answer=inspect(values);", "let answer=inspect(values,true);let normal=inspect(values,false);")
        .replace("answer==42 &&", "answer==42 && normal==42 &&");
    let checked = semaprax::check(&contracts, "slot-contracts.spx").unwrap();
    let resolved = semaprax::hir::resolve(&checked).unwrap();
    let inspect = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "slots.inspect")
        .unwrap();
    let certificate = inspect
        .cleanup_plan
        .lexical_control
        .as_deref()
        .expect("return inside loop stays lexical");
    assert_eq!(certificate.function_success_exits.len(), 2);
    assert_eq!(inspect.ensures.len(), 1);
    let semaprax::hir::ResolvedExprKind::Binary { left, .. } = &inspect.ensures[0].kind else {
        panic!("exact ensures comparison")
    };
    let semaprax::hir::ResolvedExprKind::Call { args, .. } = &left.kind else {
        panic!("exact Str length operand")
    };
    let site = &args[0].id;
    assert_eq!(inspect.cleanup_plan.blocks.iter().flat_map(|block| &block.transitions)
        .filter(|transition| matches!(transition, semaprax::cleanup_plan::CleanupTransition::CallCommit { call, .. } if call==site)).count(), 2);
    // The lexical return route keeps its shared terminal flag referenced in
    // native C.
    assert!(codegen::emit_c(&checked)
        .unwrap()
        .contains("(void)spx_canonical_cleanup_done;"));
    // Ensures is authored once and emitted at both certified return contexts.
    run(&contracts, false, "slot-no-copy", Some(4));
    let skipped = SOURCE
        .replace("answer==42 &&", "answer==0 &&")
        .replace("if good && consumed==42", "if !good && consumed==42");
    run(&skipped, false, "slot-no-copy", Some(4));
    let long_str = r#"module app.slots;
@id("app.main") fn main()->i64 {
 let data=bytes_zeroed(65537usize);
 let second_data=bytes_copy(bytes_as_slice(data));
 let text=string_from_utf8(bytes_as_slice(data));
 let second_text=string_from_utf8(bytes_as_slice(second_data));
 let joined=string_concat(text,second_text);
 let empty=vec_with_capacity<string>(1usize);
 let values=vec_push<string>(empty,joined);
 let selected=vec_str_at(values,0usize);
 if str_len_bytes(selected)==131074 {42}else{0}
}
"#;
    // String payload byte extent is independent of Vec's fixed carrier charge
    // and the Slice ceiling. This is a Str view, with no Slice publication.
    // Preserve the original duplicate owning argument as a compile-time refusal.
    // The positive path copies the full payload before either String transfer.
    let duplicate_owner = long_str.replace(
        "string_concat(text,second_text)",
        "string_concat(text,text)",
    );
    let diagnostics = semaprax::check(
        &duplicate_owner,
        std::path::Path::new("duplicate-owner.spx"),
    )
    .expect_err("one String owner cannot be transferred twice");
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-O101"));
    run(long_str, false, "none", None);
    let maximum = r#"module app.slots;
@id("app.main") fn main()->i64 {
 let data=bytes_zeroed(131072usize);
 let text=string_from_utf8(bytes_as_slice(data));
 let empty=vec_with_capacity<string>(1usize);
 let values=vec_push<string>(empty,text);
 let selected=vec_str_at(values,0usize);
 let bytes=str_as_bytes(selected);
 let inline=str_as_bytes(vec_str_at(values,0usize));
 if str_len_bytes(selected)==131072 && byte_len(bytes)==131072usize && byte_len(inline)==131072usize
  && (match byte_get(bytes,131071usize) {Option::Some{value:last}=>last==0u8,Option::None{}=>false,}) {42}else{0}
}
"#;
    // Both direct and named Slice views honor the existing 128 KiB
    // boundary. Foreign roots retain their independent 64 KiB limit.
    // The named owning String passed to push retains ordinary clone semantics.
    run(maximum, false, "none", Some(4));
    let live = SOURCE.replace(
        "let consumed=consume(values);",
        "let consumed=consume(values);let late=str_len_bytes(view);",
    );
    assert!(
        semaprax::check(&live, "slot-live.spx").is_err(),
        "live whole-Vec loan must prevent transfer"
    );
}
#[test]
fn slot_index_failures_keep_vec_two_and_poisoned_result_across_reentry() {
    for index in ["3usize", "4usize", "18446744073709551615usize"] {
        let source = SOURCE.replace(
            "vec_str_at(values,0usize)",
            &format!("vec_str_at(values,{index})"),
        );
        run(&source, true, "none", None);
    }
}
