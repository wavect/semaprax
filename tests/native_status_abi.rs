use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, parse};

const STATUS_MATRIX: &str = r#"
module test.native_status_abi;

@id("case.success")
fn success() -> i64 { 42 }

@id("case.requires")
fn requires_false() -> i64 requires false { 11 }

@id("case.ensures")
fn ensures_false() -> i64 ensures false { 12 }

@id("case.add")
fn add_overflow() -> i64 { 9223372036854775807 + 1 }

@id("case.sub")
fn sub_overflow() -> i64 { -9223372036854775807 - 2 }

@id("case.mul")
fn mul_overflow() -> i64 { 9223372036854775807 * 2 }

@id("case.div.zero")
fn division_by_zero() -> i64 { 7 / 0 }

@id("case.div.overflow")
fn division_overflow() -> i64 { (-9223372036854775807 - 1) / -1 }

@id("case.rem.zero")
fn remainder_by_zero() -> i64 { 7 % 0 }

@id("case.rem.overflow")
fn remainder_overflow() -> i64 { (-9223372036854775807 - 1) % -1 }

@id("case.neg")
fn negation_overflow() -> i64 { -(-9223372036854775807 - 1) }

@id("case.first")
fn first() -> i64 requires false { 1 }

@id("case.second")
fn second() -> i64 ensures false { 2 }

@id("case.sum")
fn sum(left: i64, right: i64) -> i64 { left + right }

@id("case.nested")
fn nested() -> i64 { sum(first(), second()) }

@id("app.main")
fn main() -> i64 { success() }
"#;

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

fn c_symbol(declaration_id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in declaration_id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

fn compiler_is_available() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}

fn native_newline() -> &'static str {
    if cfg!(windows) {
        "\r\n"
    } else {
        "\n"
    }
}

fn compile_and_run_entry_wrapper(source: &str, label: &str) -> std::process::Output {
    let program = parse(source, Path::new("native-entry-wrapper.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    let test_id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-native-wrapper-{label}-{}-{test_id}",
        std::process::id()
    );
    let source_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable_path =
        std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source_path, generated).unwrap();

    let compiled = Command::new("clang")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&source_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "generated {label} wrapper did not compile: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let executed = Command::new(&executable_path).output().unwrap();
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&executable_path);
    executed
}

#[test]
fn native_entry_wrapper_preserves_contract_failure_compatibility() {
    if !compiler_is_available() {
        return;
    }

    let executed = compile_and_run_entry_wrapper(
        r#"
module test.native_requires_wrapper;
@id("app.main")
fn main() -> i64 requires false { 42 }
"#,
        "requires",
    );

    assert_eq!(executed.status.code(), Some(70));
    assert!(executed.stdout.is_empty());
    assert_eq!(
        String::from_utf8(executed.stderr).unwrap(),
        format!(
            "SEMAPRAX contract failure: requires in main: false{}",
            native_newline()
        )
    );
}

#[test]
fn native_entry_wrapper_preserves_arithmetic_failure_compatibility() {
    if !compiler_is_available() {
        return;
    }

    let executed = compile_and_run_entry_wrapper(
        r#"
module test.native_division_wrapper;
@id("app.main")
fn main() -> i64 { 7 / 0 }
"#,
        "division",
    );

    assert_eq!(executed.status.code(), Some(71));
    assert!(executed.stdout.is_empty());
    assert_eq!(
        String::from_utf8(executed.stderr).unwrap(),
        format!(
            "SEMAPRAX checked arithmetic failure: invalid division{}",
            native_newline()
        )
    );
}

#[test]
fn native_status_arena_exhaustion_never_returns_a_language_token() {
    if !compiler_is_available() {
        return;
    }

    let source = r#"
module test.native_status_exhaustion;
@id("app.main")
fn main() -> i64 requires false { 42 }
"#;
    let program = parse(source, Path::new("native-status-exhaustion.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(1)];
    struct spx_context context;
    if (!spx_context_init(
        &context, UINT64_C(303), entries, UINT32_C(1), NULL, NULL, NULL
    )) return 80;

    spx_status_token occupied = SPX_STATUS_SUCCESS;
    if (!spx_status_record_adapter(
        &context,
        "test.prefilled.v1",
        UINT32_C(9),
        SPX_STATUS_CLASS_ADAPTER,
        SPX_RETRYABILITY_UNKNOWN,
        &occupied
    )) return 81;
    if (occupied != UINT32_C(1) || spx_status_resolve(&context, occupied) == NULL) return 82;

    int64_t result_out = -INT64_C(777777777777777777);
    spx_status_token returned = {main_symbol}(&context, &result_out);
    if (returned != SPX_STATUS_SUCCESS && spx_status_resolve(&context, returned) == NULL) {{
        fputs("generated function returned an unresolvable status token\n", stderr);
        return 83;
    }}
    fputs("generated function returned through the language status channel\n", stderr);
    return 84;
}}
"#,
        main_symbol = c_symbol("app.main"),
    );

    let test_id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-native-status-exhaustion-{}-{test_id}",
        std::process::id()
    );
    let source_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable_path =
        std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source_path, format!("{generated}\n{probe}")).unwrap();

    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&source_path)
        .arg("-o")
        .arg(&executable_path)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "generated arena-exhaustion probe did not compile: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let executed = Command::new(&executable_path).output().unwrap();
    let _ = std::fs::remove_file(&source_path);
    let _ = std::fs::remove_file(&executable_path);
    assert!(!executed.status.success());
    assert!(executed.stdout.is_empty());
    assert_eq!(
        String::from_utf8(executed.stderr).unwrap(),
        format!(
            "SEMAPRAX native runtime invariant failure: status arena exhaustion{}",
            native_newline()
        )
    );
}

#[test]
fn native_scalar_status_out_abi_preserves_poison_and_exact_failures() {
    if !compiler_is_available() {
        return;
    }

    let program = parse(STATUS_MATRIX, Path::new("native-status-abi.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    let probe = format!(
        r#"
typedef spx_status_token (*spx_test_case)(struct spx_context *, int64_t *);

static int spx_test_success(spx_test_case test_case) {{
    struct spx_status_entry records[UINT32_C(2)];
    struct spx_context context;
    if (!spx_context_init(
        &context, UINT64_C(101), records, UINT32_C(2), NULL, NULL, NULL
    )) return 10;
    int64_t result = -INT64_C(777777777777777777);
    spx_status_token token = test_case(&context, &result);
    if (token != SPX_STATUS_SUCCESS) return 11;
    if (result != INT64_C(42)) return 12;
    if (context.status_arena.length != UINT32_C(0)) return 13;
    if (spx_status_resolve(&context, token) != NULL) return 14;
    return 0;
}}

static int spx_test_failure(
    const char *label,
    spx_test_case test_case,
    const char *expected_domain,
    uint32_t expected_code,
    spx_status_class expected_class
) {{
    struct spx_status_entry records[UINT32_C(2)];
    struct spx_context context;
    if (!spx_context_init(
        &context, UINT64_C(202), records, UINT32_C(2), NULL, NULL, NULL
    )) return 20;
    const int64_t poison = -INT64_C(777777777777777777);
    int64_t result = poison;
    spx_status_token token = test_case(&context, &result);
    if (token != UINT32_C(1)) {{ fprintf(stderr, "%s: token\n", label); return 21; }}
    if (result != poison) {{ fprintf(stderr, "%s: result_out\n", label); return 22; }}
    if (context.status_arena.length != UINT32_C(1)) {{
        fprintf(stderr, "%s: arena length\n", label);
        return 23;
    }}
    const struct spx_normalized_status *status = spx_status_resolve(&context, token);
    if (status == NULL) {{ fprintf(stderr, "%s: resolve\n", label); return 24; }}
    if (strcmp(status->schema, SPX_STATUS_SCHEMA_V1) != 0) {{
        fprintf(stderr, "%s: schema\n", label);
        return 25;
    }}
    if (strcmp(status->domain_id, expected_domain) != 0) {{
        fprintf(stderr, "%s: domain\n", label);
        return 26;
    }}
    if (status->code != expected_code) {{ fprintf(stderr, "%s: code\n", label); return 27; }}
    if (status->status_class != expected_class) {{
        fprintf(stderr, "%s: class\n", label);
        return 28;
    }}
    if (status->retryability != SPX_RETRYABILITY_FALSE) {{
        fprintf(stderr, "%s: retryability\n", label);
        return 29;
    }}
    return 0;
}}

int main(void) {{
    int result = spx_test_success({success});
    if (result != 0) return result;

#define SPX_TEST_FAILURE(label, function, domain, code, status_class) \
    do {{ \
        result = spx_test_failure(label, function, domain, code, status_class); \
        if (result != 0) return result; \
    }} while (false)

    SPX_TEST_FAILURE(
        "requires", {requires}, "semaprax.contract.v1",
        SPX_STATUS_CONTRACT_REQUIRES_FALSE, SPX_STATUS_CLASS_CONTRACT
    );
    SPX_TEST_FAILURE(
        "ensures", {ensures}, "semaprax.contract.v1",
        SPX_STATUS_CONTRACT_ENSURES_FALSE, SPX_STATUS_CLASS_CONTRACT
    );
    SPX_TEST_FAILURE(
        "add", {add}, "semaprax.arithmetic.v1",
        SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, SPX_STATUS_CLASS_ARITHMETIC
    );
    SPX_TEST_FAILURE(
        "sub", {sub}, "semaprax.arithmetic.v1",
        SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, SPX_STATUS_CLASS_ARITHMETIC
    );
    SPX_TEST_FAILURE(
        "mul", {mul}, "semaprax.arithmetic.v1",
        SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, SPX_STATUS_CLASS_ARITHMETIC
    );
    SPX_TEST_FAILURE(
        "division by zero", {div_zero}, "semaprax.arithmetic.v1",
        SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, SPX_STATUS_CLASS_ARITHMETIC
    );
    SPX_TEST_FAILURE(
        "division overflow", {div_overflow}, "semaprax.arithmetic.v1",
        SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW, SPX_STATUS_CLASS_ARITHMETIC
    );
    SPX_TEST_FAILURE(
        "remainder by zero", {rem_zero}, "semaprax.arithmetic.v1",
        SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, SPX_STATUS_CLASS_ARITHMETIC
    );
    SPX_TEST_FAILURE(
        "remainder overflow", {rem_overflow}, "semaprax.arithmetic.v1",
        SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW, SPX_STATUS_CLASS_ARITHMETIC
    );
    SPX_TEST_FAILURE(
        "negation overflow", {neg}, "semaprax.arithmetic.v1",
        SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, SPX_STATUS_CLASS_ARITHMETIC
    );

    /* The first argument fails. A propagated token must not be re-recorded,
       and the second argument must not execute. Arena length one proves both. */
    SPX_TEST_FAILURE(
        "nested left-to-right", {nested}, "semaprax.contract.v1",
        SPX_STATUS_CONTRACT_REQUIRES_FALSE, SPX_STATUS_CLASS_CONTRACT
    );
    return 0;
}}
"#,
        success = c_symbol("case.success"),
        requires = c_symbol("case.requires"),
        ensures = c_symbol("case.ensures"),
        add = c_symbol("case.add"),
        sub = c_symbol("case.sub"),
        mul = c_symbol("case.mul"),
        div_zero = c_symbol("case.div.zero"),
        div_overflow = c_symbol("case.div.overflow"),
        rem_zero = c_symbol("case.rem.zero"),
        rem_overflow = c_symbol("case.rem.overflow"),
        neg = c_symbol("case.neg"),
        nested = c_symbol("case.nested"),
    );

    let test_id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-native-status-{}-{test_id}", std::process::id());
    let source_path = std::env::temp_dir().join(format!("{stem}.c"));
    std::fs::write(&source_path, format!("{generated}\n{probe}")).unwrap();

    let mut configurations = vec![("o0", vec!["-O0"]), ("o2", vec!["-O2"])];
    if cfg!(unix) {
        configurations.push((
            "sanitized",
            vec![
                "-O1",
                "-fno-omit-frame-pointer",
                "-fsanitize=address,undefined",
            ],
        ));
    }
    for (label, flags) in configurations {
        let executable_path =
            std::env::temp_dir().join(format!("{stem}-{label}{}", std::env::consts::EXE_SUFFIX));
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .args(flags)
            .arg(&source_path)
            .arg("-o")
            .arg(&executable_path)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "generated status/out C did not compile for {label}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );

        let executed = Command::new(&executable_path).output().unwrap();
        let _ = std::fs::remove_file(&executable_path);
        assert!(
            executed.status.success(),
            "native status/out {label} probe failed with {:?}: {}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
    let _ = std::fs::remove_file(&source_path);
}

#[test]
fn native_resource_execution_remains_fail_closed_until_cleanup_lowering() {
    let source = r#"
module test.native_resource_gate;
@id("resource.handle")
resource Handle {
    @id("resource.handle.drop")
    drop trivial;
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("native-resource-gate.spx")).unwrap();
    let diagnostic = codegen::emit_c(&program).unwrap_err();

    assert_eq!(diagnostic.code, "SPX-B104");
    assert_eq!(
        diagnostic.message,
        "native resource lowering requires lifecycle declarations and the verified cleanup ABI"
    );
}
