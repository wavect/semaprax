use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::cleanup::FieldLivenessShape;
use semaprax::cleanup_plan::CleanupTransition;
use semaprax::hir::{self, DeclarationId, ResolvedExprKind, ResolvedType};
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const SOURCE_PREFIX: &str = r#"
module test.concrete_generic_update;
@id("generic.update.pair") record Pair<T, U> {
  @id("generic.update.pair.payload") payload: T,
  @id("generic.update.pair.marker") marker: U,
}
@id("generic.update.consume") fn consume(packet: own Pair<Bytes, u8>) -> i64 {
  match own packet {
    Pair { payload: payload, marker: _ } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { 42 } else { 0 },
  }
}
@id("generic.update.success") fn success() -> i64 {
  let first = [1u8, 2u8];
  let replacement = [3u8, 4u8, 5u8];
  let packet = Pair<Bytes, u8> {
    payload: bytes_copy(array_as_slice(first)), marker: 1u8,
  };
  let retained = packet with { marker: 7u8 };
  let updated = retained with {
    payload: bytes_copy(array_as_slice(replacement)), marker: 42u8,
  };
  consume(updated)
}
@id("generic.update.construct-failure") fn construct_failure() -> i64 {
  let input = [9u8, 8u8];
  let packet = Pair<Bytes, u8> {
    payload: bytes_copy(array_as_slice(input)), marker: 1u8 / 0u8,
  };
  consume(packet)
}
@id("generic.update.update-failure") fn update_failure() -> i64 {
  let input = [1u8, 2u8];
  let replacement = [7u8, 8u8, 9u8];
  let packet = Pair<Bytes, u8> {
    payload: bytes_copy(array_as_slice(input)), marker: 1u8,
  };
  let updated = packet with {
    payload: bytes_copy(array_as_slice(replacement)), marker: 1u8 / 0u8,
  };
  consume(updated)
}
"#;

fn source(main: &str) -> String {
    let callable = match main {
        "generic.update.success" => "success",
        "generic.update.construct-failure" => "construct_failure",
        "generic.update.update-failure" => "update_failure",
        _ => panic!("unknown generic update fixture entry `{main}`"),
    };
    format!("{SOURCE_PREFIX}\n@id(\"app.main\") fn main() -> i64 {{ {callable}() }}\n")
}

fn admitted(main: &str) -> (String, semaprax::ast::Program, hir::ResolvedProgram) {
    let source = source(main);
    let parsed = parse(&source, Path::new("concrete-generic-update-v1.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );
    let program = hir::resolve(&parsed).expect("concrete generic update resolves");
    (source, parsed, program)
}

fn function<'a>(program: &'a hir::ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

fn symbol(id: &str) -> String {
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

#[test]
fn generic_update_hir_cleanup_replay_and_hostile_mutations_are_exact() {
    let (_, _, program) = admitted("generic.update.success");
    let update = function(&program, "generic.update.update-failure");
    let instance = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.update.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::U8],
    };
    assert!(update.cleanup.slots.iter().any(|slot| {
        slot.ty == instance
            && matches!(
                &slot.shape,
                FieldLivenessShape::Record { fields, .. }
                    if matches!(fields[0].shape, FieldLivenessShape::Leaf { .. })
                        && matches!(fields[1].shape, FieldLivenessShape::NoDrop)
            )
    }));
    let ResolvedExprKind::Block { statements, .. } = &update.body.kind else {
        panic!("body remains a block")
    };
    let update_expr = statements
        .iter()
        .map(hir::ResolvedStatement::value)
        .find(|expression| matches!(expression.kind, ResolvedExprKind::UpdateRecord { .. }))
        .unwrap();
    let replacement = match &update_expr.kind {
        ResolvedExprKind::UpdateRecord { fields, .. } => &fields[0].value,
        _ => unreachable!(),
    };
    assert!(update.cleanup_plan.blocks.iter().any(|block| {
        block.transitions.iter().any(|transition| matches!(
            transition,
            CleanupTransition::Transfer { at, destination, .. }
                if at == &replacement.id
                    && destination.projections == [DeclarationId::new("generic.update.pair.payload")]
        ))
    }));
    hir::validate(&program).expect("canonical plan independently replays");

    let mut hostile_hir = program.clone();
    let function = hostile_hir
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "generic.update.update-failure")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("body remains a block")
    };
    let update = statements
        .iter_mut()
        .map(hir::ResolvedStatement::value_mut)
        .find(|expression| matches!(expression.kind, ResolvedExprKind::UpdateRecord { .. }))
        .expect("update statement");
    update.ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.update.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::Bool],
    };
    assert_eq!(hir::validate(&hostile_hir).unwrap_err().code, "SPX-H006");

    let mut hostile_leaf = program.clone();
    let function = hostile_leaf
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "generic.update.update-failure")
        .unwrap();
    let inventory_slot = function
        .cleanup
        .slots
        .iter_mut()
        .find(|slot| slot.ty == instance)
        .unwrap();
    let plan_slot = function
        .cleanup_plan
        .slots
        .iter_mut()
        .find(|slot| slot.ty == instance)
        .unwrap();
    let FieldLivenessShape::Record {
        fields: inventory_fields,
        ..
    } = &mut inventory_slot.shape
    else {
        unreachable!()
    };
    let FieldLivenessShape::Record {
        fields: plan_fields,
        ..
    } = &mut plan_slot.field_liveness_shape
    else {
        unreachable!()
    };
    let inventory_leaf = inventory_fields[0].shape.clone();
    let plan_leaf = plan_fields[0].shape.clone();
    inventory_fields[1].shape = inventory_leaf;
    plan_fields[1].shape = plan_leaf;
    assert_eq!(hir::validate(&hostile_leaf).unwrap_err().code, "SPX-H006");

    let mut hostile_order = program.clone();
    let function = hostile_order
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "generic.update.update-failure")
        .unwrap();
    let inventory_slot = function
        .cleanup
        .slots
        .iter_mut()
        .find(|slot| slot.ty == instance)
        .unwrap();
    let plan_slot = function
        .cleanup_plan
        .slots
        .iter_mut()
        .find(|slot| slot.ty == instance)
        .unwrap();
    let FieldLivenessShape::Record {
        fields: inventory_fields,
        ..
    } = &mut inventory_slot.shape
    else {
        unreachable!()
    };
    let FieldLivenessShape::Record {
        fields: plan_fields,
        ..
    } = &mut plan_slot.field_liveness_shape
    else {
        unreachable!()
    };
    inventory_fields.swap(0, 1);
    plan_fields.swap(0, 1);
    assert_eq!(hir::validate(&hostile_order).unwrap_err().code, "SPX-H006");
}

#[test]
fn generic_update_and_both_partial_failures_settle_on_three_engines() {
    for (entry, succeeds) in [
        ("generic.update.success", true),
        ("generic.update.construct-failure", false),
        ("generic.update.update-failure", false),
    ] {
        let (_, parsed, _) = admitted(entry);
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "semaprax-concrete-generic-update-{}-{serial}.spx",
            std::process::id()
        ));
        std::fs::write(&path, source(entry)).unwrap();
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        assert_eq!(result.returned, succeeds, "{entry}");
        interpreter::verify_envelope(&result.envelope).unwrap();
        if succeeds {
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert_eq!(envelope["payload"]["outcome"]["value"], "42");
        }
        let _ = std::fs::remove_file(path);

        if Command::new("clang").arg("--version").output().is_ok() {
            run_native(&parsed, succeeds);
        }
        if Command::new("node").arg("--version").output().is_ok() {
            run_wasm(&parsed, succeeds);
        }
    }
}

fn run_native(parsed: &semaprax::ast::Program, succeeds: bool) {
    let generated = codegen::emit_c(parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(parsed).unwrap());
    let tracked = generated
        .replace(
            "uint8_t *payload = (uint8_t *)malloc(",
            "uint8_t *payload = (uint8_t *)spx_test_malloc(",
        )
        .replace("free(value->ptr);", "spx_test_free(value->ptr);");
    let status_check = if succeeds {
        "status != SPX_STATUS_SUCCESS || result != INT64_C(42)"
    } else {
        "status == SPX_STATUS_SUCCESS"
    };
    let probe = format!(
        r#"
int main(void) {{
  struct spx_status_entry entries[UINT32_C(16)];
  struct spx_context context = {{0}};
  if (!spx_context_init(&context, UINT64_C(9), entries, UINT32_C(16), NULL, NULL, NULL)) return 1;
  for (uint32_t i = 0; i < UINT32_C(4); ++i) {{
    int64_t result = INT64_C(0);
    spx_status_token status = {main}(&context, &result);
    if ({status_check}) return 2;
    if (spx_test_live_allocations != UINT64_C(0)) return 3;
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
static uint64_t spx_test_live_allocations = UINT64_C(0);
static void *spx_test_malloc(size_t size) {
  void *allocation = malloc(size);
  if (allocation != NULL) spx_test_live_allocations += UINT64_C(1);
  return allocation;
}
static void spx_test_free(void *allocation) {
  if (allocation != NULL) { spx_test_live_allocations -= UINT64_C(1); free(allocation); }
}
"#;
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir().join(format!(
            "semaprax-concrete-generic-update-native-{}-{serial}",
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
            "{}: {}",
            optimization,
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(&executable).status().unwrap().success());
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(executable);
    }
}

fn run_wasm(parsed: &semaprax::ast::Program, succeeds: bool) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-concrete-generic-update-wasm-{}-{serial}",
        std::process::id()
    ));
    wasm::build_web(parsed, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    let expectation = if succeeds {
        "if(instance.exports.semaprax_main()!==42n)throw Error('wrong value');"
    } else {
        "let failed=false;try{instance.exports.semaprax_main();}catch(_){failed=true;}if(!failed)throw Error('missing failure');"
    };
    std::fs::write(
        root.join("probe.mjs"),
        format!(
            r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes}} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const {{instance}}=await instantiateBytes(bytes,{{maxOwnedByteEntries:3}});
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
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
