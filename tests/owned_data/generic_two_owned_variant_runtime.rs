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
module test.generic_two_owned_variant_runtime;

@id("generic.two-owned.either")
variant Either<T, U> {
  @id("generic.two-owned.either.left") Left {
    @id("generic.two-owned.either.left.value") value: T,
    @id("generic.two-owned.either.left.marker") marker: i64,
  },
  @id("generic.two-owned.either.right") Right {
    @id("generic.two-owned.either.right.value") value: U,
    @id("generic.two-owned.either.right.marker") marker: i64,
  },
}

@id("generic.two-owned.make-left")
fn make_left() -> Either<Bytes, Bytes> {
  let input = [1u8, 2u8];
  Either<Bytes, Bytes>::Left {
    value: bytes_copy(array_as_slice(input)), marker: 10,
  }
}

@id("generic.two-owned.make-right")
fn make_right() -> Either<Bytes, Bytes> {
  let input = [3u8, 4u8, 5u8];
  Either<Bytes, Bytes>::Right {
    value: bytes_copy(array_as_slice(input)), marker: 20,
  }
}

@id("generic.two-owned.inspect")
fn inspect(value: borrow Either<Bytes, Bytes>) -> i64 {
  match borrow value {
    Either::Left { value: payload, marker } =>
      if byte_len(bytes_as_slice(payload)) == 2usize { marker } else { 0 },
    Either::Right { value: payload, marker } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { marker } else { 0 },
  }
}

@id("generic.two-owned.consume")
fn consume(value: own Either<Bytes, Bytes>) -> i64 {
  match own value {
    Either::Left { value: payload, marker } =>
      if byte_len(bytes_as_slice(payload)) == 2usize { marker } else { 0 },
    Either::Right { value: payload, marker } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { marker } else { 0 },
  }
}

@id("generic.two-owned.identity")
fn identity(value: own Either<Bytes, Bytes>) -> Either<Bytes, Bytes> { value }

@id("generic.two-owned.forward")
fn forward(value: own Either<Bytes, Bytes>) -> Either<Bytes, Bytes> {
  identity(value)
}

@id("generic.two-owned.guarded")
fn guarded(value: own Either<Bytes, Bytes>) -> i64
requires false
{
  consume(value)
}

@id("generic.two-owned.run")
fn run() -> i64 {
  let left = make_left();
  let left_borrowed = inspect(left);
  let left_owned = consume(forward(left));
  let right = make_right();
  let right_borrowed = inspect(right);
  let right_owned = consume(forward(right));
  left_borrowed + left_owned + right_borrowed + right_owned - 18
}

@id("generic.two-owned.fail-left-arm")
fn fail_left_arm() -> i64 {
  match own make_left() {
    Either::Left { value: payload, marker } =>
      if byte_len(bytes_as_slice(payload)) == 2usize {
        marker + 9223372036854775807
      } else { marker },
    Either::Right { value: payload, marker } =>
      if byte_len(bytes_as_slice(payload)) == 3usize { marker } else { 0 },
  }
}

@id("generic.two-owned.fail-right-arm")
fn fail_right_arm() -> i64 {
  match own make_right() {
    Either::Left { value: payload, marker } =>
      if byte_len(bytes_as_slice(payload)) == 2usize { marker } else { 0 },
    Either::Right { value: payload, marker } =>
      if byte_len(bytes_as_slice(payload)) == 3usize {
        marker + 9223372036854775807
      } else { marker },
  }
}

@id("generic.two-owned.partial-left")
fn partial_left() -> i64 {
  let input = [6u8];
  consume(Either<Bytes, Bytes>::Left {
    value: bytes_copy(array_as_slice(input)),
    marker: 9223372036854775807 + 1,
  })
}

@id("generic.two-owned.partial-right")
fn partial_right() -> i64 {
  let input = [7u8];
  consume(Either<Bytes, Bytes>::Right {
    value: bytes_copy(array_as_slice(input)),
    marker: 9223372036854775807 + 1,
  })
}

@id("app.main") fn main() -> i64 { run() }
"#;

const NO_SHALLOW_COPY_SOURCE: &str = r#"
module test.generic_two_owned_variant_no_shallow_copy;
@id("generic.two-owned.copy-proof") variant Either<T, U> {
  @id("generic.two-owned.copy-proof.left") Left {
    @id("generic.two-owned.copy-proof.left.value") value: T,
  },
  @id("generic.two-owned.copy-proof.right") Right {
    @id("generic.two-owned.copy-proof.right.value") value: U,
  },
}
@id("generic.two-owned.copy-proof.identity")
fn identity(value: own Either<Bytes, Bytes>) -> Either<Bytes, Bytes> { value }
@id("generic.two-owned.copy-proof.consume")
fn consume(value: own Either<Bytes, Bytes>) -> i64 {
  match own value {
    Either::Left { value: payload } =>
      if byte_len(bytes_as_slice(payload)) == 0usize { 0 } else { 1 },
    Either::Right { value: payload } =>
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
        panic!("generic variant symbol requires a nominal type")
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
        Path::new("generic-two-owned-variant-runtime-v1.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics.iter().all(|item| !item.severity.is_error()),
        "{diagnostics:?}"
    );
    let resolved = hir::resolve(&parsed).expect("two-owned generic variant must resolve");
    hir::validate(&resolved).expect("two-owned generic variant cleanup must replay");
    (parsed, resolved)
}

fn function<'a>(program: &'a hir::ResolvedProgram, id: &str) -> &'a hir::ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == id)
        .unwrap_or_else(|| panic!("missing function {id}"))
}

#[test]
fn authored_two_owned_branch_variant_replays_and_executes_on_all_engines() {
    let (parsed, resolved) = checked();
    assert_conditional_cleanup(&resolved);
    assert_no_shallow_wasm_copy();
    run_interpreter();
    run_native(&parsed, &resolved);
    run_wasm();
}

fn assert_conditional_cleanup(program: &hir::ResolvedProgram) {
    let consume = function(program, "generic.two-owned.consume");
    let [parameter] = consume
        .cleanup
        .entry_state
        .conditional_owned_parameters
        .as_slice()
    else {
        panic!("consume must have one conditional owner")
    };
    assert_eq!(parameter.variant.as_str(), "generic.two-owned.either");
    assert_eq!(
        parameter
            .cases
            .iter()
            .map(|case| (case.case.as_str(), case.live_flags.len()))
            .collect::<Vec<_>>(),
        [
            ("generic.two-owned.either.left", 1),
            ("generic.two-owned.either.right", 1),
        ]
    );
    let FieldLivenessShape::Variant { cases, .. } = &consume.cleanup.slots[0].shape else {
        panic!("consume parameter must retain its variant liveness shape")
    };
    assert_eq!(cases.len(), 2);
    for (index, case_id, field_id) in [
        (
            0,
            "generic.two-owned.either.left",
            "generic.two-owned.either.left.value",
        ),
        (
            1,
            "generic.two-owned.either.right",
            "generic.two-owned.either.right.value",
        ),
    ] {
        assert_eq!(cases[index].case.as_str(), case_id);
        assert_eq!(cases[index].fields[0].field.as_str(), field_id);
        let flag = parameter.cases[index].live_flags[0];
        assert_eq!(
            consume.cleanup.flags[flag.0 as usize]
                .place
                .projections
                .iter()
                .map(|projection| projection.as_str())
                .collect::<Vec<_>>(),
            [case_id, field_id]
        );
    }
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
    assert_eq!(
        authenticated,
        [
            "generic.two-owned.either.left",
            "generic.two-owned.either.right"
        ]
    );

    for id in ["generic.two-owned.identity", "generic.two-owned.forward"] {
        assert!(
            function(program, id)
                .cleanup_plan
                .blocks
                .iter()
                .flat_map(|block| &block.transitions)
                .any(|transition| matches!(transition,
                CleanupTransition::TransferVariant { variant, .. }
                    if variant.as_str() == "generic.two-owned.either")),
            "{id} must dynamically transfer the active owner"
        );
    }
    assert!(function(program, "generic.two-owned.inspect")
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .all(|transition| !matches!(transition, CleanupTransition::TransferVariant { .. })));

    for (active_index, inactive_index) in [(0, 1), (1, 0)] {
        let mut hostile = program.clone();
        let consume = hostile
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "generic.two-owned.consume")
            .unwrap();
        let foreign = consume.cleanup.entry_state.conditional_owned_parameters[0].cases
            [active_index]
            .live_flags[0];
        consume.cleanup.entry_state.conditional_owned_parameters[0].cases[inactive_index]
            .live_flags
            .push(foreign);
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }

    for (from, to) in [
        (
            "generic.two-owned.either.left",
            "generic.two-owned.either.right",
        ),
        (
            "generic.two-owned.either.right",
            "generic.two-owned.either.left",
        ),
    ] {
        let mut hostile = program.clone();
        let consume = hostile
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "generic.two-owned.consume")
            .unwrap();
        let transition = consume
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
            .find(|transition| {
                matches!(transition,
                CleanupTransition::AuthenticateVariantCase { case, .. }
                    if case.as_str() == from)
            })
            .unwrap();
        let CleanupTransition::AuthenticateVariantCase { case, .. } = transition else {
            unreachable!()
        };
        *case = hir::DeclarationId::new(to);
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }

    for (from, to) in [
        (
            "generic.two-owned.either.left",
            "generic.two-owned.either.right",
        ),
        (
            "generic.two-owned.either.right",
            "generic.two-owned.either.left",
        ),
    ] {
        let mut hostile = program.clone();
        let guarded = hostile
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "generic.two-owned.guarded")
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
            .expect("each owner case must have conditional settlement");
        action.active_case.as_mut().unwrap().case = hir::DeclarationId::new(to);
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }
}

fn run_interpreter() {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-generic-two-owned-interpreter-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    for (entry, succeeds) in [
        ("app.main", true),
        ("generic.two-owned.fail-left-arm", false),
        ("generic.two-owned.fail-right-arm", false),
        ("generic.two-owned.partial-left", false),
        ("generic.two-owned.partial-right", false),
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
        "required generic two-owned Clang backend is unavailable"
    );
    if !available {
        return;
    }
    let generated = codegen::emit_c(parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(parsed).unwrap());
    assert_native_result_tags_publish_after_owned_payloads(&generated);
    const NON_OWNING_COPIES: [&str; 2] = [
        "memcpy(payload, value.ptr, (size_t)value.len);",
        "memcpy(entry->domain_storage, status.domain_id, domain_size);",
    ];
    let mut ownership_surface = generated.clone();
    for admitted in NON_OWNING_COPIES {
        assert_eq!(ownership_surface.matches(admitted).count(), 1);
        ownership_surface = ownership_surface.replacen(admitted, "", 1);
    }
    assert!(
        ownership_surface
            .lines()
            .all(|line| !line.contains("memcpy(")),
        "owned variant carriers must never be shallow-copied"
    );
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
static void *spx_test_malloc(size_t size) {
  void *allocation = malloc(size);
  if (allocation != NULL) spx_test_live_allocations += UINT64_C(1);
  return allocation;
}
static void spx_test_free(void *allocation) {
  if (allocation != NULL) {
    if (spx_test_live_allocations == UINT64_C(0)) abort();
    spx_test_live_allocations -= UINT64_C(1);
  }
  free(allocation);
}
"#;
    let probe = format!(
        r#"
typedef spx_status_token (*spx_i64_entry)(struct spx_context *, int64_t *);
static int spx_expect_overflow(struct spx_context *context, spx_i64_entry entry) {{
  int64_t result = INT64_C(0);
  spx_status_token token = entry(context, &result);
  if (token == SPX_STATUS_SUCCESS) return 1;
  const struct spx_normalized_status *status = spx_status_resolve(context, token);
  if (status == NULL || strcmp(status->domain_id, "semaprax.arithmetic.v1") != 0) return 2;
  if (status->code != UINT32_C(1) || status->status_class != SPX_STATUS_CLASS_ARITHMETIC) return 3;
  return 0;
}}
int main(void) {{
  struct spx_status_entry entries[UINT32_C(32)];
  struct spx_context context = {{0}};
  if (!spx_context_init(&context, UINT64_C(191), entries, UINT32_C(32), NULL, NULL, NULL)) return 1;
  for (uint32_t iteration = 0; iteration < UINT32_C(4); ++iteration) {{
    int64_t result = INT64_C(0);
    if ({main}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(42)) return 2;
    if (spx_test_live_allocations != UINT64_C(0)) return 3;
    if (spx_expect_overflow(&context, {failure0}) != 0) return 4;
    if (spx_expect_overflow(&context, {failure1}) != 0) return 5;
    if (spx_expect_overflow(&context, {failure2}) != 0) return 6;
    if (spx_expect_overflow(&context, {failure3}) != 0) return 7;
    if (spx_test_live_allocations != UINT64_C(0)) return 8;
  }}
  return 0;
}}
"#,
        main = symbol("app.main"),
        failure0 = symbol("generic.two-owned.fail-left-arm"),
        failure1 = symbol("generic.two-owned.fail-right-arm"),
        failure2 = symbol("generic.two-owned.partial-left"),
        failure3 = symbol("generic.two-owned.partial-right"),
    );
    let variant_ty = &function(resolved, "generic.two-owned.consume").params[0].ty;
    let invalid_probe = format!(
        r#"
int main(void) {{
  struct spx_status_entry entries[UINT32_C(8)];
  struct spx_context context = {{0}};
  if (!spx_context_init(&context, UINT64_C(192), entries, UINT32_C(8), NULL, NULL, NULL)) return 1;
  struct {variant} invalid = {{0}};
  invalid.spx_tag = UINT32_MAX;
  int64_t result = INT64_C(0);
  (void){consume}(&context, &invalid, &result);
  return 0;
}}
"#,
        variant = variant_symbol(variant_ty),
        consume = symbol("generic.two-owned.consume"),
    );
    for optimization in ["-O0", "-O2"] {
        compile_native(
            &format!("{allocator}\n{tracked}"),
            &probe,
            optimization,
            true,
        );
        compile_native(&generated, &invalid_probe, optimization, false);
    }
}

fn assert_native_result_tags_publish_after_owned_payloads(generated: &str) {
    let tag_publish = "(*spx_result_out).spx_tag = spx_result.spx_tag;";
    let publications = generated.match_indices(tag_publish).collect::<Vec<_>>();
    assert_eq!(
        publications.len(),
        4,
        "make_left, make_right, identity, and forward must each publish one variant result"
    );
    for (offset, _) in publications {
        let prefix = &generated[..offset];
        let shell = prefix
            .rfind("memset((uint8_t *)&((*spx_result_out)) + sizeof(((*spx_result_out)).spx_tag), 0, sizeof((*spx_result_out)) - sizeof(((*spx_result_out)).spx_tag));")
            .expect("variant result publication must initialize a private output shell");
        let payload = prefix[shell..]
            .rfind("spx_bytes_move(")
            .map(|relative| shell + relative)
            .expect("variant result publication must materialize its selected owned payload");
        assert!(
            shell < payload && payload < offset,
            "the output tag must be the final publication after payload materialization"
        );
    }
}

fn compile_native(source: &str, probe: &str, optimization: &str, succeeds: bool) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "semaprax-generic-two-owned-native-{}-{serial}",
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
        "{optimization}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let executed = Command::new(&executable).status().unwrap();
    assert_eq!(executed.success(), succeeds, "{optimization}");
    let _ = std::fs::remove_file(c);
    let _ = std::fs::remove_file(executable);
}

fn run_wasm() {
    let available = Command::new("node").arg("--version").output().is_ok();
    assert!(
        available || !require_backends(),
        "required generic two-owned Node backend is unavailable"
    );
    if !available {
        return;
    }
    for (entry, succeeds) in [
        ("run", true),
        ("fail_left_arm", false),
        ("fail_right_arm", false),
        ("partial_left", false),
        ("partial_right", false),
    ] {
        let source = SOURCE.replace(
            "fn main() -> i64 { run() }",
            &format!("fn main() -> i64 {{ {entry}() }}"),
        );
        let parsed = parse(
            &source,
            Path::new("generic-two-owned-variant-runtime-wasm-v1.spx"),
        )
        .unwrap();
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-generic-two-owned-wasm-{}-{serial}",
            std::process::id()
        ));
        wasm::build_web(&parsed, &root).unwrap();
        std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
        let expectation = if succeeds {
            "if(instance.exports.semaprax_main()!==42n)throw Error('wrong value');"
        } else {
            "let failed=false;try{instance.exports.semaprax_main();}catch(error){const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.arithmetic.v1'||status.code!==1||error.message!=='SEMAPRAX checked arithmetic failure: addition overflow')throw error;failed=true;}if(!failed)throw Error('missing failure');"
        };
        std::fs::write(
            root.join("probe.mjs"),
            format!(
                r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const bytes=await readFile('./app.wasm');
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
}

fn assert_no_shallow_wasm_copy() {
    let parsed = parse(
        NO_SHALLOW_COPY_SOURCE,
        Path::new("generic-two-owned-variant-no-shallow-copy.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(diagnostics.iter().all(|item| !item.severity.is_error()));
    let bytes = wasm::emit_module(&parsed).unwrap();
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                assert!(
                    !matches!(
                        operators.read().unwrap(),
                        wasmparser::Operator::MemoryCopy { .. }
                    ),
                    "two-owned generic variant lowering must not use memory.copy"
                );
            }
        }
    }
}
