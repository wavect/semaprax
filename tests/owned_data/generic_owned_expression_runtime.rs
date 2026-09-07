use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::{
    self, DeclarationId, PlaceProjection, ResolvedExprKind, ResolvedMatchPattern, ResolvedType,
};
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn backends_required() -> bool {
    std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some()
}

const PREFIX: &str = r#"
module test.generic_owned_expression_runtime;
@id("generic.expression.pair") record Pair<T, U> {
  @id("generic.expression.pair.payload") payload: T,
  @id("generic.expression.pair.marker") marker: U,
}
@id("generic.expression.compose")
fn compose<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {
  let observed = value.marker;
  let updated = value with {
    marker: observed,
  };
  match own updated {
    Pair { payload: payload, marker: marker } =>
      Pair<Bytes, T> { payload: payload, marker: marker },
  }
}
@id("generic.expression.borrow-update")
fn borrow_update<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {
  let observed = match borrow value {
    Pair { payload: _, marker: marker } => marker,
  };
  value with { marker: observed }
}
@id("generic.expression.unreachable")
fn unreachable<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {
  let observed = value.marker;
  let updated = value with { marker: observed };
  match own updated {
    Pair { payload: payload, marker: marker } =>
      Pair<Bytes, T> { payload: payload, marker: marker },
  }
}
@id("generic.expression.fail-update")
fn fail_update<T>(value: own Pair<Bytes, T>, divisor: i64) -> Pair<Bytes, T> {
  let marker = value.marker;
  value with {
    marker: if 1 / divisor == 1 { marker } else { marker },
  }
}
@id("generic.expression.fail-rebuild")
fn fail_rebuild<T>(value: own Pair<Bytes, T>, divisor: i64) -> Pair<Bytes, T> {
  match own value {
    Pair { payload: payload, marker: marker } => Pair<Bytes, T> {
      payload: payload,
      marker: if 1 / divisor == 1 { marker } else { marker },
    },
  }
}
"#;

fn run_function(name: &str, ty: &str, value: &str) -> String {
    format!(
        r#"
@id("generic.expression.inspect-{name}")
fn inspect_{name}(value: borrow Pair<Bytes, {ty}>) -> i64 {{
  match borrow value {{
    Pair {{ payload: payload, marker: marker }} =>
      if marker == {value} && byte_len(bytes_as_slice(payload)) == 1usize {{ 1 }} else {{ 0 }},
  }}
}}
@id("generic.expression.run-{name}") fn run_{name}() -> i64 {{
  let input = [1u8];
  let value = Pair<Bytes, {ty}> {{
    payload: bytes_copy(array_as_slice(input)), marker: {value},
  }};
  let inspected_owner = borrow_update<{ty}>(value);
  let composed = compose<{ty}>(inspected_owner);
  let inspected = inspect_{name}(composed);
  match own composed {{
    Pair {{ payload: payload, marker: marker }} =>
      if marker == {value} && byte_len(bytes_as_slice(payload)) == 1usize {{ inspected }} else {{ 0 }},
  }}
}}
"#
    )
}

fn source(entry: &str) -> String {
    let mut source = String::from(PREFIX);
    for (name, ty, value) in [
        ("i64", "i64", "7"),
        ("i32", "i32", "7i32"),
        ("u8", "u8", "7u8"),
        ("usize", "usize", "7usize"),
        ("char", "char", "'x'"),
        ("f32", "f32", "1.5f32"),
        ("f64", "f64", "1.5f64"),
        ("bool", "bool", "true"),
    ] {
        source.push_str(&run_function(name, ty, value));
    }
    source.push_str(
        r#"
@id("generic.expression.success") fn success() -> i64 {
  run_i64() + run_i32() + run_u8() + run_usize()
    + run_char() + run_f32() + run_f64() + run_bool()
}
@id("generic.expression.update-failure") fn update_failure() -> i64 {
  let input = [2u8];
  let value = Pair<Bytes, bool> {
    payload: bytes_copy(array_as_slice(input)), marker: true,
  };
  let composed = fail_update<bool>(value, 0);
  match own composed { Pair { payload: payload, marker: _ } =>
    if byte_len(bytes_as_slice(payload)) == 1usize { 0 } else { 0 }, }
}
@id("generic.expression.rebuild-failure") fn rebuild_failure() -> i64 {
  let input = [3u8];
  let value = Pair<Bytes, i64> {
    payload: bytes_copy(array_as_slice(input)), marker: 7,
  };
  let composed = fail_rebuild<i64>(value, 0);
  match own composed { Pair { payload: payload, marker: _ } =>
    if byte_len(bytes_as_slice(payload)) == 1usize { 0 } else { 0 }, }
}
"#,
    );
    let call = match entry {
        "success" => "success()",
        "update-failure" => "update_failure()",
        "rebuild-failure" => "rebuild_failure()",
        _ => panic!("unknown generic expression entry `{entry}`"),
    };
    writeln!(source, "@id(\"app.main\") fn main() -> i64 {{ {call} }}").unwrap();
    source
}

fn checked(entry: &str) -> (String, semaprax::ast::Program, hir::ResolvedProgram) {
    let source = source(entry);
    let parsed = parse(
        &source,
        Path::new("generic-owned-expression-composition-v1.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{entry}: {diagnostics:?}"
    );
    let resolved = hir::resolve(&parsed).expect("generic owned expressions resolve");
    hir::validate(&resolved).expect("generic owned expressions independently replay");
    (source, parsed, resolved)
}

fn symbol(id: &str) -> String {
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

#[test]
fn generic_owned_expression_composition_matches_all_internal_backends() {
    let clang_available = Command::new("clang").arg("--version").output().is_ok();
    let node_available = Command::new("node").arg("--version").output().is_ok();
    assert!(
        clang_available || !backends_required(),
        "required generic-expression Clang backend is unavailable"
    );
    assert!(
        node_available || !backends_required(),
        "required generic-expression Node backend is unavailable"
    );

    for (entry, expected) in [
        ("success", Some(8)),
        ("update-failure", None),
        ("rebuild-failure", None),
    ] {
        let (source, parsed, _) = checked(entry);
        run_interpreter(entry, &source, expected);
        if clang_available {
            run_native(entry, &parsed, expected);
        }
        if node_available {
            run_wasm(entry, &parsed, expected);
        }
    }
}

fn run_interpreter(entry: &str, source: &str, expected: Option<i64>) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-expression-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    for _ in 0..4 {
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        interpreter::verify_envelope(&result.envelope).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        match expected {
            Some(value) => {
                assert!(result.returned, "{entry}");
                assert_eq!(envelope["payload"]["outcome"]["value"], value.to_string());
            }
            None => {
                assert!(!result.returned, "{entry}");
                assert_eq!(
                    envelope["payload"]["outcome"]["status"]["domain_id"],
                    "semaprax.arithmetic.v1"
                );
                assert_eq!(envelope["payload"]["outcome"]["status"]["code"], 4);
            }
        }
    }
    let _ = std::fs::remove_file(path);
}

fn run_native(entry: &str, parsed: &semaprax::ast::Program, expected: Option<i64>) {
    let generated = codegen::emit_c(parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(parsed).unwrap());
    let mut shallow_copy_surface = generated.clone();
    for admitted in [
        "memcpy(payload, value.ptr, (size_t)value.len);",
        "memcpy(entry->domain_storage, status.domain_id, domain_size);",
    ] {
        assert_eq!(shallow_copy_surface.matches(admitted).count(), 1, "{entry}");
        shallow_copy_surface = shallow_copy_surface.replacen(admitted, "", 1);
    }
    assert!(!shallow_copy_surface.contains("memcpy("), "{entry}");
    let tracked = generated
        .replace(
            "uint8_t *payload = (uint8_t *)malloc(",
            "uint8_t *payload = (uint8_t *)spx_test_malloc(",
        )
        .replace("free(value->ptr);", "spx_test_free(value->ptr);");
    let outcome = match expected {
        Some(value) => format!("status != SPX_STATUS_SUCCESS || result != INT64_C({value})"),
        None => "status == SPX_STATUS_SUCCESS || result != INT64_C(0x2525252525252525) || spx_status_resolve(&context, status) == NULL || strcmp(spx_status_resolve(&context, status)->domain_id, \"semaprax.arithmetic.v1\") != 0 || spx_status_resolve(&context, status)->code != UINT32_C(4)".to_owned(),
    };
    let probe = format!(
        r#"
int main(void) {{
  struct spx_status_entry entries[UINT32_C(32)];
  struct spx_context context = {{0}};
  if (!spx_context_init(&context, UINT64_C(23), entries, UINT32_C(32), NULL, NULL, NULL)) return 1;
  for (uint32_t i = 0; i < UINT32_C(4); ++i) {{
    int64_t result = INT64_C(0x2525252525252525);
    uint32_t before = context.status_arena.length;
    spx_status_token status = {main}(&context, &result);
    if ({outcome}) return 2;
    if (spx_test_live_allocations != UINT64_C(0)) return 3;
    if (status == SPX_STATUS_SUCCESS && context.status_arena.length != before) return 4;
    if (status != SPX_STATUS_SUCCESS && context.status_arena.length != before + UINT32_C(1)) return 5;
  }}
  return 0;
}}
"#,
        main = symbol("app.main")
    );
    let allocator = r#"
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static uint64_t spx_test_live_allocations = UINT64_C(0);
static void *spx_test_malloc(size_t size) {
  void *allocation = malloc(size);
  if (allocation != NULL) spx_test_live_allocations += UINT64_C(1);
  return allocation;
}
static void spx_test_free(void *allocation) {
  if (allocation != NULL) {
    if (spx_test_live_allocations == UINT64_C(0)) abort();
    spx_test_live_allocations -= UINT64_C(1);
    free(allocation);
  }
}
"#;
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "semaprax-generic-owned-expression-native-{}-{serial}",
            std::process::id()
        ));
        let c = base.with_extension("c");
        let executable = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
        let output = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg("-DSPX_NO_ENTRY_WRAPPER")
            .arg(&c)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{entry} {optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(&executable).status().unwrap().success());
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(executable);
    }
}

fn wasm_bulk_memory_counts(bytes: &[u8]) -> (usize, usize) {
    let mut copies = 0;
    let mut grows = 0;
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                match operators.read().unwrap() {
                    wasmparser::Operator::MemoryCopy { .. } => copies += 1,
                    wasmparser::Operator::MemoryGrow { .. } => grows += 1,
                    _ => {}
                }
            }
        }
    }
    (copies, grows)
}

fn run_wasm(entry: &str, parsed: &semaprax::ast::Program, expected: Option<i64>) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-expression-wasm-{}-{serial}",
        std::process::id()
    ));
    wasm::build_web(parsed, &root).unwrap();
    let core = std::fs::read(root.join("app.wasm")).unwrap();
    let (copies, grows) = wasm_bulk_memory_counts(&core);
    assert_eq!(grows, 0, "{entry}: owned expressions must not grow memory");
    let mut baseline_source = source(entry);
    for (_, ty, _) in [
        ("i64", "i64", "7"),
        ("i32", "i32", "7i32"),
        ("u8", "u8", "7u8"),
        ("usize", "usize", "7usize"),
        ("char", "char", "'x'"),
        ("f32", "f32", "1.5f32"),
        ("f64", "f64", "1.5f64"),
        ("bool", "bool", "true"),
    ] {
        baseline_source = baseline_source.replace(
            &format!(
                "  let inspected_owner = borrow_update<{ty}>(value);\n  let composed = compose<{ty}>(inspected_owner);"
            ),
            "  let composed = value;",
        );
    }
    baseline_source = baseline_source
        .replace(
            "let composed = fail_update<bool>(value, 0);",
            "let composed = value;",
        )
        .replace(
            "let composed = fail_rebuild<i64>(value, 0);",
            "let composed = value;",
        );
    let baseline = parse(
        &baseline_source,
        Path::new("generic-owned-expression-composition-wasm-baseline-v1.spx"),
    )
    .unwrap();
    assert!(verify::verify(&baseline)
        .iter()
        .all(|diagnostic| !diagnostic.severity.is_error()));
    let baseline_root = root.with_extension("baseline");
    wasm::build_web(&baseline, &baseline_root).unwrap();
    let baseline_core = std::fs::read(baseline_root.join("app.wasm")).unwrap();
    let (baseline_copies, baseline_grows) = wasm_bulk_memory_counts(&baseline_core);
    assert_eq!(baseline_grows, 0);
    assert_eq!(
        copies, baseline_copies,
        "{entry}: generic owned expression lowering added memory.copy"
    );
    let _ = std::fs::remove_dir_all(baseline_root);
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    let expectation = match expected {
        Some(value) => format!(
            "if(instance.exports.semaprax_main()!=={value}n)throw Error('wrong value');"
        ),
        None => "let failed=false;try{instance.exports.semaprax_main();}catch(error){const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.arithmetic.v1'||status.code!==4)throw error;failed=true;}if(!failed)throw Error('missing failure');".to_owned(),
    };
    std::fs::write(
        root.join("probe.mjs"),
        format!(
            r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const bytes=await readFile('./app.wasm');
let zeroRejected=false;
try{{await instantiateBytes(bytes,{{maxOwnedByteEntries:0}});}}catch(error){{zeroRejected=error instanceof RangeError&&String(error).includes('owned-byte-entry limit');}}
if(!zeroRejected)throw Error('zero owned-byte capacity was accepted');
const {{instance}}=await instantiateBytes(bytes,{{maxOwnedByteEntries:1}});
for(let i=0;i<4;i+=1){{{expectation}}}
"#
        ),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(root);
    assert!(
        output.status.success(),
        "{entry}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn native_and_wasm_reject_tampered_generic_expression_proofs() {
    let (_, _, resolved) = checked("success");
    assert!(resolved
        .function_instances
        .iter()
        .all(|instance| instance.template.as_str() != "generic.expression.unreachable"));
    let expected = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.expression.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::Bool],
    };
    let assert_rejected = |hostile: &hir::ResolvedProgram| {
        assert_eq!(hir::validate(hostile).unwrap_err().code, "SPX-H006");
        assert_eq!(codegen::emit_hir_c(hostile).unwrap_err().code, "SPX-H006");
        assert_eq!(
            wasm::emit_resolved_module(hostile).unwrap_err().code,
            "SPX-H006"
        );
    };

    let mut hostile_template = resolved.clone();
    let template = hostile_template
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "generic.expression.unreachable")
        .expect("unreachable composition template");
    let ResolvedExprKind::Block { statements, .. } = &mut template.body.kind else {
        panic!("composition template body remains a block")
    };
    let update = statements
        .iter_mut()
        .map(hir::ResolvedStatement::value_mut)
        .find(|value| matches!(value.kind, ResolvedExprKind::UpdateRecord { .. }))
        .expect("template generic update");
    let ResolvedExprKind::UpdateRecord { record, .. } = &mut update.kind else {
        unreachable!()
    };
    *record = DeclarationId::new("generic.expression.missing-record");
    assert_rejected(&hostile_template);

    let mut hostile_template_projection = resolved.clone();
    let template = hostile_template_projection
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "generic.expression.unreachable")
        .expect("unreachable composition template");
    let ResolvedExprKind::Block { statements, .. } = &mut template.body.kind else {
        panic!("unreachable composition template body remains a block")
    };
    let projection = statements
        .iter_mut()
        .map(hir::ResolvedStatement::value_mut)
        .find_map(|value| match &mut value.kind {
            ResolvedExprKind::Place(place) if !place.projections.is_empty() => Some(place),
            _ => None,
        })
        .expect("unreachable template marker projection");
    projection.projections[0] =
        PlaceProjection::Field(DeclarationId::new("generic.expression.pair.missing"));
    assert_rejected(&hostile_template_projection);

    let mut hostile_template_match = resolved.clone();
    let template = hostile_template_match
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "generic.expression.unreachable")
        .expect("unreachable composition template");
    let ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
        panic!("unreachable composition template body remains a block")
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("unreachable composition template tail remains a match")
    };
    let ResolvedMatchPattern::Record {
        instance: pattern_instance,
        ..
    } = &mut arms[0].pattern
    else {
        panic!("unreachable template arm remains a record pattern")
    };
    *pattern_instance = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.expression.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::I64],
    };
    assert_rejected(&hostile_template_match);

    let mut hostile_template_constructor = resolved.clone();
    let template = hostile_template_constructor
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "generic.expression.unreachable")
        .expect("unreachable composition template");
    let ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
        panic!("unreachable composition template body remains a block")
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("unreachable composition template tail remains a match")
    };
    let ResolvedExprKind::ConstructRecord { fields, .. } = &mut arms[0].value.kind else {
        panic!("unreachable template arm remains a record constructor")
    };
    fields.swap(0, 1);
    assert_rejected(&hostile_template_constructor);

    let mut hostile_template_binding = resolved.clone();
    let template = hostile_template_binding
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "generic.expression.unreachable")
        .expect("unreachable composition template");
    let ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
        panic!("unreachable composition template body remains a block")
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("unreachable composition template tail remains a match")
    };
    let ResolvedMatchPattern::Record { fields, .. } = &mut arms[0].pattern else {
        panic!("unreachable template arm remains a record pattern")
    };
    let hir::ResolvedRecordMatchFieldPattern::Binding(binding) = &mut fields[1].pattern else {
        panic!("unreachable template marker remains bound")
    };
    binding.ty = ResolvedType::I64;
    assert_rejected(&hostile_template_binding);

    let mut hostile_projection = resolved.clone();
    let instance = hostile_projection
        .function_instances
        .iter_mut()
        .find(|instance| {
            instance.template.as_str() == "generic.expression.compose"
                && instance.type_arguments == [ResolvedType::Bool]
        })
        .expect("bool composition instance");
    assert_eq!(instance.function.return_type, expected);
    let ResolvedExprKind::Block { statements, .. } = &mut instance.function.body.kind else {
        panic!("composition body remains a block")
    };
    let projection = statements
        .iter_mut()
        .map(hir::ResolvedStatement::value_mut)
        .find_map(|value| match &mut value.kind {
            ResolvedExprKind::Place(place) if !place.projections.is_empty() => Some(place),
            _ => None,
        })
        .expect("materialized marker projection");
    projection.projections[0] =
        PlaceProjection::Field(DeclarationId::new("generic.expression.pair.missing"));
    assert_rejected(&hostile_projection);

    let mut hostile_update_type = resolved.clone();
    let instance = hostile_update_type
        .function_instances
        .iter_mut()
        .find(|instance| {
            instance.template.as_str() == "generic.expression.compose"
                && instance.type_arguments == [ResolvedType::Bool]
        })
        .expect("bool composition instance");
    let ResolvedExprKind::Block { statements, .. } = &mut instance.function.body.kind else {
        panic!("composition body remains a block")
    };
    let update = statements
        .iter_mut()
        .map(hir::ResolvedStatement::value_mut)
        .find(|value| matches!(value.kind, ResolvedExprKind::UpdateRecord { .. }))
        .expect("materialized generic update");
    update.ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.expression.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::I64],
    };
    assert_rejected(&hostile_update_type);

    let mut hostile_match = resolved.clone();
    let instance = hostile_match
        .function_instances
        .iter_mut()
        .find(|instance| {
            instance.template.as_str() == "generic.expression.compose"
                && instance.type_arguments == [ResolvedType::Bool]
        })
        .expect("bool composition instance");
    let ResolvedExprKind::Block { tail, .. } = &mut instance.function.body.kind else {
        panic!("composition body remains a block")
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("composition tail remains a match")
    };
    let ResolvedMatchPattern::Record {
        instance: pattern_instance,
        ..
    } = &mut arms[0].pattern
    else {
        panic!("composition arm remains a record pattern")
    };
    *pattern_instance = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.expression.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::I64],
    };
    assert_rejected(&hostile_match);

    let mut hostile_loan = resolved.clone();
    let instance = hostile_loan
        .function_instances
        .iter_mut()
        .find(|instance| {
            instance.template.as_str() == "generic.expression.borrow-update"
                && instance.type_arguments == [ResolvedType::Bool]
        })
        .expect("bool borrow-update instance");
    assert!(!instance.function.loan_plan.loans.is_empty());
    instance.function.loan_plan.loans.clear();
    assert_rejected(&hostile_loan);

    let mut hostile_constructor = resolved;
    let instance = hostile_constructor
        .function_instances
        .iter_mut()
        .find(|instance| {
            instance.template.as_str() == "generic.expression.compose"
                && instance.type_arguments == [ResolvedType::Bool]
        })
        .expect("bool composition instance");
    let ResolvedExprKind::Block { tail, .. } = &mut instance.function.body.kind else {
        panic!("composition body remains a block")
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        panic!("composition tail remains a match")
    };
    let ResolvedExprKind::ConstructRecord { fields, .. } = &mut arms[0].value.kind else {
        panic!("composition arm remains a record constructor")
    };
    fields[0].field = DeclarationId::new("generic.expression.pair.missing");
    assert_rejected(&hostile_constructor);
}

#[test]
fn generic_owned_expression_source_gate_rejects_non_exact_composition() {
    let bodies = [
        "Pair<Bytes, T> { payload: value.payload, marker: value.marker }",
        "if true { value with { marker: value.marker } } else { value }",
        "{ let nested = value with { marker: value.marker }; nested }",
        "value.payload",
        "match borrow value { Pair { payload: _, marker: marker } => 0, }",
        "match borrow value { Pair { payload: payload, marker: marker } => marker, }",
        "match own value { Pair { payload: payload, marker: marker } => Pair<Bytes, T> { payload: payload, marker: payload }, }",
    ];
    for (index, body) in bodies.into_iter().enumerate() {
        let source = format!(
            r#"module test.generic_owned_expression_source_negative;
@id("negative.pair") record Pair<T, U> {{
  @id("negative.pair.payload") payload: T,
  @id("negative.pair.marker") marker: U,
}}
@id("negative.relay.{index}")
fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> {{ {body} }}
@id("negative.main") fn main() -> i64 {{ 0 }}
"#
        );
        let parsed = parse(
            &source,
            Path::new("generic-owned-expression-source-negative.spx"),
        )
        .expect("negative generic composition parses");
        let diagnostics = verify::verify(&parsed);
        if index == 0 {
            // Direct field-wise reconstruction is now admitted by generic owned
            // record composition (v2); it should not be rejected.
            assert!(
                diagnostics.iter().all(|d| !d.severity.is_error()),
                "case {index} should be admitted after composition: {diagnostics:?}"
            );
        } else {
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == "SPX-T226"),
                "case {index} escaped the exact source gate: {diagnostics:?}"
            );
        }
    }
}
