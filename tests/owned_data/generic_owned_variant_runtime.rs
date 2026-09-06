use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::cleanup::FieldLivenessShape;
use semaprax::cleanup_plan::CleanupTransition;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, hir, parse, verify, wasm};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.generic_owned_variant_runtime;

@id("generic.variant.either")
variant Either<T, U> {
  @id("generic.variant.either.empty") Empty,
  @id("generic.variant.either.left") Left {
    @id("generic.variant.either.left.value") value: T,
    @id("generic.variant.either.left.marker") marker: i64,
  },
  @id("generic.variant.either.right") Right {
    @id("generic.variant.either.right.value") value: U,
    @id("generic.variant.either.right.marker") marker: i64,
  },
}

@id("generic.variant.make-left")
fn make_left() -> Either<Bytes, i64> {
  let input = [1u8, 2u8];
  Either<Bytes, i64>::Left {
    value: bytes_copy(array_as_slice(input)), marker: 10,
  }
}

@id("generic.variant.inspect-left")
fn inspect_left(value: borrow Either<Bytes, i64>) -> i64 {
  match borrow value {
    Either::Empty {} => 0,
    Either::Left { value: left_payload, marker: left_marker } =>
      if byte_len(bytes_as_slice(left_payload)) == 2usize { left_marker } else { 0 },
    Either::Right { value: right_value, marker: right_marker } => right_value + right_marker,
  }
}

@id("generic.variant.consume-left")
fn consume_left(value: own Either<Bytes, i64>) -> i64 {
  match own value {
    Either::Empty {} => 0,
    Either::Left { value: left_payload, marker: left_marker } =>
      if byte_len(bytes_as_slice(left_payload)) == 2usize { left_marker } else { 0 },
    Either::Right { value: right_value, marker: right_marker } => right_value + right_marker,
  }
}

@id("generic.variant.make-right")
fn make_right() -> Either<i64, Bytes> {
  let input = [3u8, 4u8, 5u8];
  Either<i64, Bytes>::Right {
    value: bytes_copy(array_as_slice(input)), marker: 20,
  }
}

@id("generic.variant.inspect-right")
fn inspect_right(value: borrow Either<i64, Bytes>) -> i64 {
  match borrow value {
    Either::Empty {} => 0,
    Either::Left { value: left_value, marker: left_marker } => left_value + left_marker,
    Either::Right { value: right_payload, marker: right_marker } =>
      if byte_len(bytes_as_slice(right_payload)) == 3usize { right_marker } else { 0 },
  }
}

@id("generic.variant.consume-right")
fn consume_right(value: own Either<i64, Bytes>) -> i64 {
  match own value {
    Either::Empty {} => 0,
    Either::Left { value: left_value, marker: left_marker } => left_value + left_marker,
    Either::Right { value: right_payload, marker: right_marker } =>
      if byte_len(bytes_as_slice(right_payload)) == 3usize { right_marker } else { 0 },
  }
}

@id("generic.variant.run")
fn run() -> i64 {
  let left = make_left();
  let left_borrowed = inspect_left(left);
  let left_owned = consume_left(left);
  let right = make_right();
  let right_borrowed = inspect_right(right);
  let right_owned = consume_right(right);
  let inactive_left = consume_left(Either<Bytes, i64>::Right { value: 1, marker: 0 });
  let inactive_right = consume_right(Either<i64, Bytes>::Left { value: 1, marker: 0 });
  left_borrowed + left_owned + right_borrowed + right_owned +
    inactive_left + inactive_right - 20
}

@id("generic.variant.fail-left-match")
fn fail_left_match() -> i64 {
  let value = make_left();
  match own value {
    Either::Empty {} => 0,
    Either::Left { value: left_payload, marker: left_marker } =>
      if byte_len(bytes_as_slice(left_payload)) == 2usize {
        left_marker + 9223372036854775807
      } else { left_marker },
    Either::Right { value: right_value, marker: right_marker } => right_value + right_marker,
  }
}

@id("generic.variant.fail-right-match")
fn fail_right_match() -> i64 {
  let value = make_right();
  match own value {
    Either::Empty {} => 0,
    Either::Left { value: left_value, marker: left_marker } => left_value + left_marker,
    Either::Right { value: right_payload, marker: right_marker } =>
      if byte_len(bytes_as_slice(right_payload)) == 3usize {
        right_marker + 9223372036854775807
      } else { right_marker },
  }
}

@id("generic.variant.partial-left")
fn partial_left() -> i64 {
  let input = [6u8];
  consume_left(Either<Bytes, i64>::Left {
    value: bytes_copy(array_as_slice(input)),
    marker: 9223372036854775807 + 1,
  })
}

@id("generic.variant.partial-right")
fn partial_right() -> i64 {
  let input = [7u8];
  consume_right(Either<i64, Bytes>::Right {
    value: bytes_copy(array_as_slice(input)),
    marker: 9223372036854775807 + 1,
  })
}

@id("app.main") fn main() -> i64 { run() }
"#;

const NO_SHALLOW_COPY_SOURCE: &str = r#"
module test.generic_owned_variant_no_shallow_copy;
@id("generic.variant.copy-proof") variant Either<T, U> {
  @id("generic.variant.copy-proof.left") Left {
    @id("generic.variant.copy-proof.left.value") value: T,
  },
  @id("generic.variant.copy-proof.right") Right {
    @id("generic.variant.copy-proof.right.value") value: U,
  },
}
@id("generic.variant.copy-proof.relay")
fn relay(value: own Either<Bytes, i64>) -> Either<Bytes, i64> { value }
@id("generic.variant.copy-proof.consume")
fn consume(value: own Either<Bytes, i64>) -> i64 {
  match own value {
    Either::Left { value: payload } =>
      if byte_len(bytes_as_slice(payload)) == 0usize { 0 } else { 1 },
    Either::Right { value: scalar } => scalar,
  }
}
@id("app.main") fn main() -> i64 {
  consume(relay(Either<Bytes, i64>::Right { value: 42 }))
}
"#;

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

fn require_backends() -> bool {
    std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some()
}

fn checked() -> (semaprax::ast::Program, hir::ResolvedProgram) {
    let parsed = parse(SOURCE, Path::new("generic-owned-variant-runtime-v1.spx")).unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );
    let resolved = hir::resolve(&parsed).expect("generic owned variants resolve");
    hir::validate(&resolved).expect("generic owned variant cleanup independently replays");
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
fn concrete_generic_owned_variant_cases_replay_and_execute_on_three_engines() {
    let (parsed, resolved) = checked();
    assert_case_specific_cleanup(&resolved);
    assert_owned_variant_wasm_has_no_shallow_copy();
    run_interpreter();
    run_native(&parsed);
    run_wasm();
}

fn assert_owned_variant_wasm_has_no_shallow_copy() {
    let parsed = parse(
        NO_SHALLOW_COPY_SOURCE,
        Path::new("generic-owned-variant-no-shallow-copy.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );
    let bytes = wasm::emit_module(&parsed).unwrap();
    assert_no_memory_copy(&bytes);
}

fn assert_case_specific_cleanup(program: &hir::ResolvedProgram) {
    for (function_id, expected_live_case) in [
        (
            "generic.variant.consume-left",
            "generic.variant.either.left",
        ),
        (
            "generic.variant.consume-right",
            "generic.variant.either.right",
        ),
    ] {
        let function = function(program, function_id);
        let [parameter] = function
            .cleanup
            .entry_state
            .conditional_owned_parameters
            .as_slice()
        else {
            panic!("{function_id} must have one conditional owned parameter")
        };
        assert_eq!(parameter.variant.as_str(), "generic.variant.either");
        let expected_domain = if expected_live_case == "generic.variant.either.left" {
            [
                ("generic.variant.either.empty", 0),
                ("generic.variant.either.left", 1),
                ("generic.variant.either.right", 0),
            ]
        } else {
            [
                ("generic.variant.either.empty", 0),
                ("generic.variant.either.left", 0),
                ("generic.variant.either.right", 1),
            ]
        };
        assert_eq!(
            parameter
                .cases
                .iter()
                .map(|case| (case.case.as_str(), case.live_flags.len()))
                .collect::<Vec<_>>(),
            expected_domain
        );
        let FieldLivenessShape::Variant { cases, .. } = &function.cleanup.slots[0].shape else {
            panic!("{function_id} parameter must retain its variant shape")
        };
        let owned_case = cases
            .iter()
            .find(|case| case.case.as_str() == expected_live_case)
            .unwrap();
        assert_eq!(
            owned_case
                .fields
                .iter()
                .filter(|field| matches!(field.shape, FieldLivenessShape::Leaf { .. }))
                .count(),
            1
        );
        assert!(function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .any(|transition| matches!(transition,
                CleanupTransition::AuthenticateVariantCase { case, .. }
                    if case.as_str() == expected_live_case)));
    }

    let mut hostile_liveness = program.clone();
    let consume = hostile_liveness
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "generic.variant.consume-left")
        .unwrap();
    let live = consume.cleanup.entry_state.conditional_owned_parameters[0].cases[1].live_flags[0];
    consume.cleanup.entry_state.conditional_owned_parameters[0].cases[2]
        .live_flags
        .push(live);
    assert_eq!(
        hir::validate(&hostile_liveness).unwrap_err().code,
        "SPX-H006"
    );

    let mut hostile_transition = program.clone();
    let consume = hostile_transition
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "generic.variant.consume-left")
        .unwrap();
    let transition = consume
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find(|transition| {
            matches!(transition,
            CleanupTransition::AuthenticateVariantCase { case, .. }
                if case.as_str() == "generic.variant.either.empty")
        })
        .expect("inactive Empty case must retain explicit authentication");
    let CleanupTransition::AuthenticateVariantCase { case, .. } = transition else {
        unreachable!()
    };
    *case = hir::DeclarationId::new("generic.variant.either.right");
    assert_eq!(
        hir::validate(&hostile_transition).unwrap_err().code,
        "SPX-H006"
    );
}

fn run_interpreter() {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-generic-owned-variant-interpreter-{}-{serial}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    for (entry, succeeds) in [
        ("app.main", true),
        ("generic.variant.fail-left-match", false),
        ("generic.variant.fail-right-match", false),
        ("generic.variant.partial-left", false),
        ("generic.variant.partial-right", false),
    ] {
        for _ in 0..4 {
            let result =
                interpreter::interpret(&path, entry, &[], &InterpreterOptions::default()).unwrap();
            assert_eq!(result.returned, succeeds, "{entry}");
            interpreter::verify_envelope(&result.envelope).unwrap();
            if succeeds {
                let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
                assert_eq!(envelope["payload"]["outcome"]["value"], "42");
            }
        }
    }
    let _ = std::fs::remove_file(path);
}

fn run_native(parsed: &semaprax::ast::Program) {
    let available = Command::new("clang").arg("--version").output().is_ok();
    assert!(
        available || !require_backends(),
        "required generic-owned-variant Clang backend is unavailable"
    );
    if !available {
        return;
    }
    let generated = codegen::emit_c(parsed).unwrap();
    assert_eq!(generated, codegen::emit_c(parsed).unwrap());
    const NON_OWNING_RUNTIME_COPIES: [&str; 2] = [
        "memcpy(payload, value.ptr, (size_t)value.len);",
        "memcpy(entry->domain_storage, status.domain_id, domain_size);",
    ];
    let mut ownership_surface = generated.clone();
    for admitted in NON_OWNING_RUNTIME_COPIES {
        assert_eq!(ownership_surface.matches(admitted).count(), 1);
        ownership_surface = ownership_surface.replacen(admitted, "", 1);
    }
    let remaining_memcpy = ownership_surface
        .lines()
        .filter(|line| line.contains("memcpy("))
        .collect::<Vec<_>>();
    assert!(
        remaining_memcpy.is_empty(),
        "owned generic variants must not use shallow payload copies: {remaining_memcpy:#?}"
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
    let failures = [
        symbol("generic.variant.fail-left-match"),
        symbol("generic.variant.fail-right-match"),
        symbol("generic.variant.partial-left"),
        symbol("generic.variant.partial-right"),
    ];
    let probe = format!(
        r#"
typedef spx_status_token (*spx_i64_entry)(struct spx_context *, int64_t *);
static int spx_expect_add_overflow(
    struct spx_context *context,
    spx_i64_entry entry,
    int64_t *result
) {{
  spx_status_token token = entry(context, result);
  if (token == SPX_STATUS_SUCCESS) return 1;
  const struct spx_normalized_status *status = spx_status_resolve(context, token);
  if (status == NULL || strcmp(status->domain_id, "semaprax.arithmetic.v1") != 0) return 2;
  if (status->code != UINT32_C(1) || status->status_class != SPX_STATUS_CLASS_ARITHMETIC) return 3;
  return 0;
}}
int main(void) {{
  struct spx_status_entry entries[UINT32_C(32)];
  struct spx_context context = {{0}};
  if (!spx_context_init(&context, UINT64_C(91), entries, UINT32_C(32), NULL, NULL, NULL)) return 1;
  for (uint32_t iteration = 0; iteration < UINT32_C(4); ++iteration) {{
    int64_t result = INT64_C(0);
    if ({main}(&context, &result) != SPX_STATUS_SUCCESS || result != INT64_C(42)) return 2;
    if (spx_test_live_allocations != UINT64_C(0)) return 3;
    if (spx_expect_add_overflow(&context, {failure0}, &result) != 0) return 4;
    if (spx_expect_add_overflow(&context, {failure1}, &result) != 0) return 5;
    if (spx_expect_add_overflow(&context, {failure2}, &result) != 0) return 6;
    if (spx_expect_add_overflow(&context, {failure3}, &result) != 0) return 7;
    if (spx_test_live_allocations != UINT64_C(0)) return 8;
  }}
  return 0;
}}
"#,
        main = symbol("app.main"),
        failure0 = failures[0],
        failure1 = failures[1],
        failure2 = failures[2],
        failure3 = failures[3],
    );
    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-generic-owned-variant-native-{}-{serial}",
            std::process::id()
        ));
        let c = root.with_extension("c");
        let executable = root.with_extension(std::env::consts::EXE_EXTENSION);
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
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(&executable).status().unwrap().success());
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(executable);
    }
}

fn run_wasm() {
    let available = Command::new("node").arg("--version").output().is_ok();
    assert!(
        available || !require_backends(),
        "required generic-owned-variant Node backend is unavailable"
    );
    if !available {
        return;
    }
    for (entry, succeeds) in [
        ("run", true),
        ("fail_left_match", false),
        ("fail_right_match", false),
        ("partial_left", false),
        ("partial_right", false),
    ] {
        let source = SOURCE.replace(
            "fn main() -> i64 { run() }",
            &format!("fn main() -> i64 {{ {entry}() }}"),
        );
        let parsed = parse(
            &source,
            Path::new("generic-owned-variant-runtime-wasm-v1.spx"),
        )
        .unwrap();
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-generic-owned-variant-wasm-{}-{serial}",
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

fn assert_no_memory_copy(bytes: &[u8]) {
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                assert!(
                    !matches!(
                        operators.read().unwrap(),
                        wasmparser::Operator::MemoryCopy { .. }
                    ),
                    "owned generic variant lowering must not use memory.copy"
                );
            }
        }
    }
}
