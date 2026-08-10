use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, parse, wasm};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.variant_backends;
@id("choice.type")
variant Choice {
    @id("choice.none") None,
    @id("choice.number") Number {
        @id("choice.number.value") value: i64,
    },
    @id("choice.flag") Flag {
        @id("choice.flag.enabled") enabled: bool,
    },
    @id("choice.pair") Pair {
        @id("choice.pair.first") first: i64,
        @id("choice.pair.second") second: i64,
    },
}
@id("choice.make")
fn make(value: i64) -> Choice { Choice::Number { value: value } }
@id("choice.select")
fn select(choice: Choice) -> i64 {
    match choice {
        Choice::None {} => 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.as_bool")
fn as_bool(choice: Choice) -> bool {
    match choice {
        Choice::None {} => false,
        Choice::Number { value: number } => number == 42,
        Choice::Flag { enabled: flag } => flag,
        Choice::Pair { first: left, second: right } => left == right,
    }
}
@id("choice.selected")
fn selected_only() -> i64 {
    match Choice::Number { value: 42 } {
        Choice::None {} => 9223372036854775807 + 1,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => 1 / 0,
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.selected_failure")
fn selected_failure() -> i64 {
    match Choice::Flag { enabled: true } {
        Choice::None {} => 1 / 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => 9223372036854775807 + 1,
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.construct_order")
fn construct_order() -> i64 {
    match Choice::Pair { second: 1 / 0, first: 9223372036854775807 + 1 } {
        Choice::None {} => 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.scrutinee")
fn failing_scrutinee() -> Choice requires false { Choice::None {} }
@id("choice.aggregate_failure")
fn aggregate_failure() -> Choice {
    Choice::Number { value: 9223372036854775807 + 1 }
}
@id("choice.scrutinee_first")
fn scrutinee_first() -> i64 {
    match failing_scrutinee() {
        Choice::None {} => 9223372036854775807 + 1,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.post")
fn post(choice: Choice) -> i64 ensures false { select(choice) }
@id("app.main")
fn main() -> i64 { select(make(42)) }
"#;

fn hex_identity(value: &str) -> String {
    let mut output = String::with_capacity(value.len() * 2);
    for byte in value.bytes() {
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn native_copy_variants_match_once_select_one_arm_and_preserve_poison_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("variant-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());

    let variant = format!("spx_variant_{}", hex_identity("choice.type"));
    let payload = format!("spx_case_{}", hex_identity("choice.number"));
    let value = format!("spx_field_{}", hex_identity("choice.number.value"));
    assert!(generated.contains(&format!(
        "_Static_assert(sizeof(struct {variant}) == UINT32_C(24)"
    )));
    assert!(generated.contains(&format!(
        "_Static_assert(offsetof(struct {variant}, spx_payload) == UINT32_C(8)"
    )));
    assert!(generated.contains("memset(&"));
    assert!(generated.contains("spx_runtime_invariant_failure(\"invalid variant tag\")"));

    let symbol = |id: &str| format!("spx_decl_{}", hex_identity(id));
    let probe = format!(
        r#"
#include <string.h>
static int spx_test_poison(const unsigned char *bytes, size_t length) {{
    for (size_t index = 0; index < length; index += 1) {{
        if (bytes[index] != UINT8_C(165)) return 0;
    }}
    return 1;
}}
static int spx_test_status(const struct spx_context *context, spx_status_token token, const char *domain, uint32_t code) {{
    const struct spx_normalized_status *status = spx_status_resolve(context, token);
    return status != NULL && strcmp(status->domain_id, domain) == 0 && status->code == code;
}}
int main(int argc, char **argv) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(88), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    struct {variant} input = {{0}};
    input.spx_tag = UINT32_C(1);
    input.spx_payload.{payload}.{value} = INT64_C(42);
    int64_t scalar = INT64_C(0x2525252525252525);
    if (argc > 1) {{
        (void)argv;
        input.spx_tag = UINT32_MAX;
        (void){select}(&context, &input, &scalar);
        return 90;
    }}
    if ({select}(&context, &input, &scalar) != SPX_STATUS_SUCCESS || scalar != INT64_C(42)) return 11;
    bool flag = false;
    if ({as_bool}(&context, &input, &flag) != SPX_STATUS_SUCCESS || !flag) return 12;
    scalar = INT64_C(0x2525252525252525);
    if ({selected}(&context, &scalar) != SPX_STATUS_SUCCESS || scalar != INT64_C(42)) return 13;
    scalar = INT64_C(0x2525252525252525);
    spx_status_token status = {selected_failure}(&context, &scalar);
    if (!spx_test_status(&context, status, "semaprax.arithmetic.v1", UINT32_C(1)) ||
        scalar != INT64_C(0x2525252525252525)) return 14;
    status = {construct_order}(&context, &scalar);
    if (!spx_test_status(&context, status, "semaprax.arithmetic.v1", UINT32_C(4)) ||
        scalar != INT64_C(0x2525252525252525)) return 15;
    status = {scrutinee_first}(&context, &scalar);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1)) ||
        scalar != INT64_C(0x2525252525252525)) return 16;
    status = {post}(&context, &input, &scalar);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(2)) ||
        scalar != INT64_C(0x2525252525252525)) return 17;
    struct {variant} aggregate_output;
    memset(&aggregate_output, 0xa5, sizeof(aggregate_output));
    status = {failing_scrutinee}(&context, &aggregate_output);
    if (!spx_test_status(&context, status, "semaprax.contract.v1", UINT32_C(1)) ||
        !spx_test_poison((const unsigned char *)&aggregate_output, sizeof(aggregate_output))) return 18;
    memset(&aggregate_output, 0xa5, sizeof(aggregate_output));
    status = {aggregate_failure}(&context, &aggregate_output);
    if (!spx_test_status(&context, status, "semaprax.arithmetic.v1", UINT32_C(1)) ||
        !spx_test_poison((const unsigned char *)&aggregate_output, sizeof(aggregate_output))) return 19;
    if ({make}(&context, INT64_C(42), &aggregate_output) != SPX_STATUS_SUCCESS) return 18;
    if (aggregate_output.spx_tag != UINT32_C(1) ||
        aggregate_output.spx_payload.{payload}.{value} != INT64_C(42)) return 19;
    return 0;
}}
"#,
        select = symbol("choice.select"),
        as_bool = symbol("choice.as_bool"),
        selected = symbol("choice.selected"),
        selected_failure = symbol("choice.selected_failure"),
        construct_order = symbol("choice.construct_order"),
        scrutinee_first = symbol("choice.scrutinee_first"),
        post = symbol("choice.post"),
        failing_scrutinee = symbol("choice.scrutinee"),
        aggregate_failure = symbol("choice.aggregate_failure"),
        make = symbol("choice.make"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-variant-native-{}-{id}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "variant C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        assert!(
            executed.status.success(),
            "variant C failed at {optimization}: status={:?} stderr={}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
        let invalid = Command::new(&executable)
            .arg("invalid-tag")
            .output()
            .unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            !invalid.status.success(),
            "invalid native variant tag did not fail closed"
        );
        assert!(String::from_utf8_lossy(&invalid.stderr).contains("invalid variant tag"));
    }
}

#[test]
fn public_native_and_wasm_variant_results_are_equivalent() {
    if !command_available("node") {
        return;
    }
    let program = parse(SOURCE, Path::new("variant-equivalence.spx")).unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-variant-equivalence-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    std::fs::write(
        &script_path,
        r#"import { readFile } from "node:fs/promises";
const fail = (name) => () => { throw new Error(`unexpected host import ${name}`); };
const bytes = await readFile(process.argv[2]);
const { instance } = await WebAssembly.instantiate(bytes, { env: {
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
} });
if (instance.exports.semaprax_main() !== 42n) throw new Error("variant backend result mismatch");
console.log("variant-equivalence-v1-ok");
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node public variant failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "variant-equivalence-v1-ok"
    );
}
