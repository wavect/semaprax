use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::cleanup::FieldLivenessShape;
use semaprax::cleanup_plan::CleanupTransition;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, hir, parse, verify, wasm};
use sha2::{Digest as _, Sha256};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.compiler_owned_two_owned_result_runtime;

@id("result.two-owned.make-ok")
fn make_ok() -> Result<Bytes, Bytes> {
  let input = [1u8, 2u8];
  Result<Bytes, Bytes>::Ok { value: bytes_copy(array_as_slice(input)) }
}

@id("result.two-owned.make-err")
fn make_err() -> Result<Bytes, Bytes> {
  let input = [3u8, 4u8, 5u8];
  Result<Bytes, Bytes>::Err { error: bytes_copy(array_as_slice(input)) }
}

@id("result.two-owned.inspect")
fn inspect(value: borrow Result<Bytes, Bytes>) -> i64 {
  match borrow value {
    Result::Ok { value: payload } =>
      if byte_len(bytes_as_slice(payload)) == 2usize { 2 } else { 0 },
    Result::Err { error: payload } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { 3 } else { 0 },
  }
}

@id("result.two-owned.consume")
fn consume(value: own Result<Bytes, Bytes>) -> i64 {
  match own value {
    Result::Ok { value: payload } =>
      if byte_len(bytes_as_slice(payload)) == 2usize { 2 } else { 0 },
    Result::Err { error: payload } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { 3 } else { 0 },
  }
}

@id("result.two-owned.identity")
fn identity(value: own Result<Bytes, Bytes>) -> Result<Bytes, Bytes> { value }

@id("result.two-owned.forward")
fn forward(value: own Result<Bytes, Bytes>) -> Result<Bytes, Bytes> {
  identity(value)
}

@id("result.two-owned.accept-pair")
fn accept_pair(value: own Result<Bytes, Bytes>, marker: i64) -> i64 {
  consume(value) + marker
}

@id("result.two-owned.guarded")
fn guarded(value: own Result<Bytes, Bytes>) -> i64
requires false
{
  consume(value)
}

@id("result.two-owned.run")
fn run() -> i64 {
  let ok = make_ok();
  let ok_borrowed = inspect(ok);
  let ok_owned = consume(forward(ok));
  let err = make_err();
  let err_borrowed = inspect(err);
  let err_owned = consume(forward(err));
  ok_borrowed + ok_owned + err_borrowed + err_owned + 32
}

@id("result.two-owned.fail-ok-arm")
fn fail_ok_arm() -> i64 {
  match own make_ok() {
    Result::Ok { value: payload } =>
      if byte_len(bytes_as_slice(payload)) == 2usize { 9223372036854775807 + 1 } else { 0 },
    Result::Err { error: payload } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { 3 } else { 0 },
  }
}

@id("result.two-owned.fail-err-arm")
fn fail_err_arm() -> i64 {
  match own make_err() {
    Result::Ok { value: payload } =>
      if byte_len(bytes_as_slice(payload)) == 2usize { 2 } else { 0 },
    Result::Err { error: payload } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { 9223372036854775807 + 1 } else { 0 },
  }
}

@id("result.two-owned.partial-ok-call")
fn partial_ok_call() -> i64 {
  accept_pair(make_ok(), 9223372036854775807 + 1)
}

@id("result.two-owned.partial-err-call")
fn partial_err_call() -> i64 {
  accept_pair(make_err(), 9223372036854775807 + 1)
}

@id("app.main") fn main() -> i64 { run() }
"#;

const NO_SHALLOW_COPY_SOURCE: &str = r#"
module test.compiler_owned_two_owned_result_no_shallow_copy;
@id("result.copy-proof.identity")
fn identity(value: own Result<Bytes, Bytes>) -> Result<Bytes, Bytes> { value }
@id("result.copy-proof.consume")
fn consume(value: own Result<Bytes, Bytes>) -> i64 {
  match own value {
    Result::Ok { value: payload } =>
      if byte_len(bytes_as_slice(payload)) == 0usize { 0 } else { 1 },
    Result::Err { error: payload } =>
      if byte_len(bytes_as_slice(payload)) == 0usize { 0 } else { 1 },
  }
}
@id("app.main") fn main() -> i64 { 42 }
"#;

fn symbol(id: &str) -> String {
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

fn variant_symbol(ty: &hir::ResolvedType) -> String {
    let hir::ResolvedType::Nominal { declaration, .. } = ty else {
        panic!("Result symbol requires nominal type")
    };
    let mut result = symbol(declaration.as_str()).replacen("spx_decl_", "spx_variant_", 1);
    let mut digest = Sha256::new();
    digest.update(b"semaprax.native-variant-instance.v1\0");
    digest.update(ty.identity_key().as_bytes());
    result.push_str("_inst_");
    for byte in digest.finalize() {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

fn require_backends() -> bool {
    std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some()
}

fn checked() -> (semaprax::ast::Program, hir::ResolvedProgram) {
    let parsed = parse(
        SOURCE,
        Path::new("compiler-owned-two-owned-result-runtime.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics.iter().all(|item| !item.severity.is_error()),
        "{diagnostics:?}"
    );
    let resolved = hir::resolve(&parsed).expect("Result<Bytes,Bytes> must resolve internally");
    hir::validate(&resolved).expect("Result<Bytes,Bytes> cleanup must replay");
    (parsed, resolved)
}

fn function<'a>(program: &'a hir::ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap()
}

#[test]
fn compiler_owned_two_owned_result_executes_on_all_internal_engines() {
    let (parsed, resolved) = checked();
    assert_conditional_cleanup(&resolved);
    assert_no_shallow_wasm_copy();
    run_interpreter();
    run_native(&parsed, &resolved);
    run_wasm();
}

fn assert_conditional_cleanup(program: &hir::ResolvedProgram) {
    let consume = function(program, "result.two-owned.consume");
    let [parameter] = consume
        .cleanup
        .entry_state
        .conditional_owned_parameters
        .as_slice()
    else {
        panic!("consume must have one conditional owned parameter")
    };
    assert_eq!(parameter.variant.as_str(), "core.result");
    assert_eq!(
        parameter
            .cases
            .iter()
            .map(|case| (case.case.as_str(), case.live_flags.len()))
            .collect::<Vec<_>>(),
        [("core.result.ok", 1), ("core.result.err", 1)]
    );
    let FieldLivenessShape::Variant { cases, .. } = &consume.cleanup.slots[0].shape else {
        panic!("Result parameter must retain conditional shape")
    };
    assert_eq!(cases[0].fields[0].field.as_str(), "core.result.ok.value");
    assert_eq!(cases[1].fields[0].field.as_str(), "core.result.err.error");
    let authenticated = consume
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| match transition {
            CleanupTransition::AuthenticateVariantCase { case, .. } => Some(case.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(authenticated, ["core.result.ok", "core.result.err"]);
    for id in ["result.two-owned.identity", "result.two-owned.forward"] {
        assert!(function(program, id).cleanup_plan.blocks.iter().flat_map(|block| &block.transitions).any(|transition| matches!(transition, CleanupTransition::TransferVariant { variant, .. } if variant.as_str() == "core.result")));
    }
    assert!(function(program, "result.two-owned.inspect")
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .all(|transition| !matches!(transition, CleanupTransition::TransferVariant { .. })));

    for (active, inactive) in [(0, 1), (1, 0)] {
        let mut hostile = program.clone();
        let consume = hostile
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "result.two-owned.consume")
            .unwrap();
        let foreign =
            consume.cleanup.entry_state.conditional_owned_parameters[0].cases[active].live_flags[0];
        consume.cleanup.entry_state.conditional_owned_parameters[0].cases[inactive]
            .live_flags
            .push(foreign);
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }
    for (from, to) in [
        ("core.result.ok", "core.result.err"),
        ("core.result.err", "core.result.ok"),
    ] {
        let mut hostile = program.clone();
        let consume = hostile
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "result.two-owned.consume")
            .unwrap();
        let transition = consume.cleanup_plan.blocks.iter_mut().flat_map(|block| &mut block.transitions).find(|transition| matches!(transition, CleanupTransition::AuthenticateVariantCase { case, .. } if case.as_str() == from)).unwrap();
        let CleanupTransition::AuthenticateVariantCase { case, .. } = transition else {
            unreachable!()
        };
        *case = hir::DeclarationId::new(to);
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");

        let mut hostile = program.clone();
        let guarded = hostile
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "result.two-owned.guarded")
            .unwrap();
        let action = guarded
            .cleanup_plan
            .exits
            .iter_mut()
            .flat_map(|exit| &mut exit.finalize_in_order)
            .find(|action| {
                action
                    .active_case
                    .as_ref()
                    .is_some_and(|guard| guard.case.as_str() == from)
            })
            .unwrap();
        action.active_case.as_mut().unwrap().case = hir::DeclarationId::new(to);
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }
}

fn run_interpreter() {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-result-two-owned-interpreter-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    for (entry, succeeds) in [
        ("app.main", true),
        ("result.two-owned.fail-ok-arm", false),
        ("result.two-owned.fail-err-arm", false),
        ("result.two-owned.partial-ok-call", false),
        ("result.two-owned.partial-err-call", false),
    ] {
        for _ in 0..4 {
            let result =
                interpreter::interpret(&path, entry, &[], &InterpreterOptions::default()).unwrap();
            assert_eq!(result.returned, succeeds, "{entry}");
            interpreter::verify_envelope(&result.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            if succeeds {
                assert_eq!(envelope["payload"]["outcome"]["value"], "42");
            } else {
                assert_eq!(
                    envelope["payload"]["outcome"]["status"]["domain_id"],
                    "semaprax.arithmetic.v1"
                );
                assert_eq!(envelope["payload"]["outcome"]["status"]["code"], 1);
            }
        }
    }
    let _ = std::fs::remove_file(path);
}

fn run_native(parsed: &semaprax::ast::Program, resolved: &hir::ResolvedProgram) {
    let available = Command::new("clang").arg("--version").output().is_ok();
    assert!(
        available || !require_backends(),
        "required Result Clang backend is unavailable"
    );
    if !available {
        return;
    }
    let generated = codegen::emit_c(parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(parsed).unwrap());
    assert_tag_last(&generated);
    let mut ownership_surface = generated.clone();
    for admitted in [
        "memcpy(payload, value.ptr, (size_t)value.len);",
        "memcpy(entry->domain_storage, status.domain_id, domain_size);",
    ] {
        assert_eq!(ownership_surface.matches(admitted).count(), 1);
        ownership_surface = ownership_surface.replacen(admitted, "", 1);
    }
    assert!(ownership_surface
        .lines()
        .all(|line| !line.contains("memcpy(")));
    let tracked = generated
        .replace(
            "uint8_t *payload = (uint8_t *)malloc(",
            "uint8_t *payload = (uint8_t *)spx_test_malloc(",
        )
        .replace("free(value->ptr);", "spx_test_free(value->ptr);");
    let allocator = r#"
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
static uint64_t spx_test_live_allocations = UINT64_C(0);
static void *spx_test_malloc(size_t size) { void *p = malloc(size); if (p != NULL) ++spx_test_live_allocations; return p; }
static void spx_test_free(void *p) { if (p != NULL) { if (spx_test_live_allocations == 0) abort(); --spx_test_live_allocations; } free(p); }
"#;
    let probe = format!(
        r#"
typedef spx_status_token (*entry_fn)(struct spx_context *, int64_t *);
static int overflow(struct spx_context *ctx, entry_fn entry) {{
  int64_t value=0; spx_status_token token=entry(ctx,&value); if(token==SPX_STATUS_SUCCESS)return 1;
  const struct spx_normalized_status *status=spx_status_resolve(ctx,token);
  return status==NULL||strcmp(status->domain_id,"semaprax.arithmetic.v1")!=0||status->code!=UINT32_C(1)||status->status_class!=SPX_STATUS_CLASS_ARITHMETIC;
}}
int main(void) {{
  struct spx_status_entry entries[UINT32_C(32)]; struct spx_context ctx={{0}};
  if(!spx_context_init(&ctx,UINT64_C(291),entries,UINT32_C(32),NULL,NULL,NULL))return 1;
  for(uint32_t i=0;i<UINT32_C(4);++i){{ int64_t value=0;
    if({main}(&ctx,&value)!=SPX_STATUS_SUCCESS||value!=INT64_C(42))return 2;
    if(spx_test_live_allocations!=0)return 3;
    if(overflow(&ctx,{f0})||overflow(&ctx,{f1})||overflow(&ctx,{f2})||overflow(&ctx,{f3}))return 4;
    if(spx_test_live_allocations!=0)return 5;
  }} return 0;
}}
"#,
        main = symbol("app.main"),
        f0 = symbol("result.two-owned.fail-ok-arm"),
        f1 = symbol("result.two-owned.fail-err-arm"),
        f2 = symbol("result.two-owned.partial-ok-call"),
        f3 = symbol("result.two-owned.partial-err-call")
    );
    let ty = &function(resolved, "result.two-owned.consume").params[0].ty;
    let invalid = format!(
        r#"
int main(void) {{ struct spx_status_entry entries[UINT32_C(8)]; struct spx_context ctx={{0}};
  if(!spx_context_init(&ctx,UINT64_C(292),entries,UINT32_C(8),NULL,NULL,NULL))return 1;
  struct {variant} hostile={{0}}; hostile.spx_tag=UINT32_MAX; int64_t out=0;
  (void){consume}(&ctx,&hostile,&out); return 0;
}}
"#,
        variant = variant_symbol(ty),
        consume = symbol("result.two-owned.consume")
    );
    for optimization in ["-O0", "-O2"] {
        compile_native(
            &format!("{allocator}\n{tracked}"),
            &probe,
            optimization,
            true,
        );
        compile_native(&generated, &invalid, optimization, false);
    }
}

fn assert_tag_last(generated: &str) {
    let publish = "(*spx_result_out).spx_tag = spx_result.spx_tag;";
    let offsets = generated
        .match_indices(publish)
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    assert_eq!(offsets.len(), 4);
    for offset in offsets {
        let prefix = &generated[..offset];
        let shell = prefix.rfind("memset((uint8_t *)&((*spx_result_out)) + sizeof(((*spx_result_out)).spx_tag), 0, sizeof((*spx_result_out)) - sizeof(((*spx_result_out)).spx_tag));").unwrap();
        let payload = prefix[shell..]
            .rfind("spx_bytes_move(")
            .map(|at| shell + at)
            .unwrap();
        assert!(shell < payload && payload < offset);
    }
}

fn compile_native(source: &str, probe: &str, optimization: &str, succeeds: bool) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-result-two-owned-native-{}-{serial}",
        std::process::id()
    ));
    let c = root.with_extension("c");
    let executable = root.with_extension(std::env::consts::EXE_EXTENSION);
    std::fs::write(&c, format!("{source}\n{probe}")).unwrap();
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
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        Command::new(&executable).status().unwrap().success(),
        succeeds
    );
    let _ = std::fs::remove_file(c);
    let _ = std::fs::remove_file(executable);
}

fn run_wasm() {
    let available = Command::new("node").arg("--version").output().is_ok();
    assert!(
        available || !require_backends(),
        "required Result Node backend is unavailable"
    );
    if !available {
        return;
    }
    for (entry, succeeds) in [
        ("run", true),
        ("fail_ok_arm", false),
        ("fail_err_arm", false),
        ("partial_ok_call", false),
        ("partial_err_call", false),
    ] {
        let source = SOURCE.replace(
            "fn main() -> i64 { run() }",
            &format!("fn main() -> i64 {{ {entry}() }}"),
        );
        let parsed = parse(
            &source,
            Path::new("compiler-owned-two-owned-result-wasm.spx"),
        )
        .unwrap();
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-result-two-owned-wasm-{}-{serial}",
            std::process::id()
        ));
        wasm::build_web(&parsed, &root).unwrap();
        std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        let expectation = if succeeds {
            "if(instance.exports.semaprax_main()!==42n)throw Error('wrong value');"
        } else {
            "let failed=false;try{instance.exports.semaprax_main();}catch(error){const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.arithmetic.v1'||status.code!==1||error.message!=='SEMAPRAX checked arithmetic failure: addition overflow')throw error;failed=true;}if(!failed)throw Error('missing failure');"
        };
        std::fs::write(root.join("probe.mjs"), format!(r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const bytes=await readFile('./app.wasm'); const {{instance}}=await instantiateBytes(bytes,{{maxOwnedByteEntries:1}});
for(let i=0;i<4;i+=1){{{expectation}}}
"#)).unwrap();
        let output = Command::new("node")
            .arg("probe.mjs")
            .current_dir(&root)
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(root);
        assert!(
            output.status.success(),
            "{entry}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

fn assert_no_shallow_wasm_copy() {
    let parsed = parse(
        NO_SHALLOW_COPY_SOURCE,
        Path::new("compiler-owned-two-owned-result-no-copy.spx"),
    )
    .unwrap();
    assert!(verify::verify(&parsed)
        .iter()
        .all(|item| !item.severity.is_error()));
    let bytes = wasm::emit_module(&parsed).unwrap();
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                assert!(!matches!(
                    operators.read().unwrap(),
                    wasmparser::Operator::MemoryCopy { .. }
                ));
            }
        }
    }
}
