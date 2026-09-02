use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-cxx-shim-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

const VALID_SOURCE: &str = r#"
module test.probe;

@id("probe.double")
fn double(value: i64) -> i64
requires value >= 0
ensures result == value + value
{
value + value
}

@id("probe.flag")
fn flag(enabled: bool) -> bool { enabled }

@id("app.main")
fn main() -> i64
ensures result == 42
{
if flag(double(21) == 42) { 42 } else { 0 }
}
"#;

fn double_options() -> CxxShimOptions {
    CxxShimOptions::new(vec!["probe.double".to_owned()], DEFAULT_MAX_BYTES).expect("valid options")
}

#[test]
fn options_reject_out_of_bounds_values() {
    assert!(CxxShimOptions::new(Vec::new(), DEFAULT_MAX_BYTES).is_err());
    assert!(
        CxxShimOptions::new(vec!["a".to_owned(); MAX_FUNCTIONS + 1], DEFAULT_MAX_BYTES).is_err()
    );
    assert!(CxxShimOptions::new(vec![String::new()], DEFAULT_MAX_BYTES).is_err());
    assert!(CxxShimOptions::new(vec!["x".to_owned(), "x".to_owned()], DEFAULT_MAX_BYTES).is_err());
    assert!(CxxShimOptions::new(vec!["x".to_owned()], 512).is_err());
    assert!(CxxShimOptions::new(vec!["x".to_owned()], graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
    assert!(CxxShimOptions::new(vec!["x".to_owned()], graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
}

#[test]
fn include_guard_is_deterministic_and_identity_sensitive() {
    let first = include_guard(&["math.add".to_owned()]);
    assert_eq!(first, include_guard(&["math.add".to_owned()]));
    assert_eq!(first.len(), "SPX_CXX_SHIM_".len() + 32);
    assert!(first.starts_with("SPX_CXX_SHIM_"));
    assert_ne!(first, include_guard(&["math.sum".to_owned()]));
    assert_ne!(
        first,
        include_guard(&["math.add".to_owned(), "math.sum".to_owned()])
    );
}

#[test]
fn symbols_match_the_native_hex_encoding() {
    assert_eq!(cxx_function_symbol("app.main"), "spx_decl_6170702e6d61696e");
}

#[test]
fn hygiene_rejects_comment_hostile_text() {
    assert!(hygiene_check("plain".to_owned()).is_ok());
    assert!(hygiene_check("result == left + right".to_owned()).is_ok());
    assert!(hygiene_check("terminates */ here".to_owned()).is_err());
    assert!(hygiene_check("line\nbreak".to_owned()).is_err());
    assert!(hygiene_check("carriage\rreturn".to_owned()).is_err());
}

#[test]
fn golden_fragment_has_expected_shape_and_is_deterministic() {
    let path = write_temp(VALID_SOURCE);
    let first = fragment_text(&path, &double_options()).expect("fragment");
    let second = fragment_text(&path, &double_options()).expect("fragment");
    assert_eq!(first, second);
    assert!(first.starts_with("/*\n"));
    assert!(first.contains("#ifndef SPX_CXX_SHIM_"));
    assert!(first.contains("#include <stdbool.h>\n#include <stdint.h>\n"));
    assert!(first.contains("extern \"C\" {\n"));
    assert!(first.contains(" * stable-id: probe.double\n"));
    assert!(first.contains(" * requires: value >= 0\n"));
    assert!(first.contains(" * ensures: result == value + value\n"));
    assert!(first.contains(" * effects: none\n"));
    assert!(first.contains(" * status-contract: returns spx_status_token;"));
    assert!(first.contains(" * ownership: caller-free / by-value scalars\n"));
    assert!(first.contains(
        "static __attribute__((unused)) spx_status_token spx_decl_70726f62652e646f75626c65(struct spx_context *spx_ctx, int64_t, int64_t *spx_result_out);"
    ));
    assert!(first.ends_with("\n}\n\n#endif\n"));
    cleanup(&path);
}

#[test]
fn envelope_round_trips_through_verify_envelope() {
    let path = write_temp(VALID_SOURCE);
    let envelope = generate(&path, &double_options()).expect("envelope");
    let fragment = verify_envelope(&envelope).expect("verified");
    assert_eq!(fragment, fragment_text(&path, &double_options()).unwrap());
    cleanup(&path);
}

#[test]
fn verify_envelope_detects_tampering() {
    let path = write_temp(VALID_SOURCE);
    let envelope = generate(&path, &double_options()).expect("envelope");
    let payload_tampered = envelope.replace("\"matches_native\":true", "\"matches_native\":false");
    assert!(verify_envelope(&payload_tampered).is_err());
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(verify_envelope(&truncated).is_err());
    assert!(verify_envelope("not json").is_err());
    cleanup(&path);
}

#[test]
fn signature_matches_the_native_projection_line() {
    let path = write_temp(VALID_SOURCE);
    let program = parse(&std::fs::read_to_string(&path).unwrap(), &path).expect("parses");
    let native = codegen::emit_c(&program).expect("native projection");
    let fragment = fragment_text(&path, &double_options()).expect("fragment");
    for line in fragment.lines() {
        if line.starts_with("static __attribute__((unused))") {
            assert!(
                native
                    .lines()
                    .any(|native_line| native_line.trim_end() == line),
                "fragment line must appear verbatim in the native projection"
            );
        }
    }
    cleanup(&path);
}

#[test]
fn selection_errors_fail_closed() {
    let path = write_temp(VALID_SOURCE);
    let unknown = CxxShimOptions::new(vec!["probe.missing".to_owned()], DEFAULT_MAX_BYTES).unwrap();
    assert!(generate(&path, &unknown).is_err());
    let duplicate_target = CxxShimOptions::new(
        vec!["probe.double".to_owned(), "double".to_owned()],
        DEFAULT_MAX_BYTES,
    )
    .unwrap();
    assert!(generate(&path, &duplicate_target).is_err());
    cleanup(&path);
}

#[test]
fn every_exclusion_reason_is_reachable() {
    let source = r#"
module test.probe;
permit { io.release }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("probe.borrowed")
fn borrowed(target: borrow Buffer, amount: i64) -> i64 { amount }

@id("probe.wide")
fn wide(label: [u8; 1]) -> i64 { 0 }

@id("probe.narrow")
fn narrow(ratio: i64) -> string { "x" }

@id("app.main")
fn main() -> i64
ensures result == 7
{
7
}

@id("buffer.type")
resource Buffer {
@id("buffer.type.drop")
drop trivial;
}
"#;
    let path = write_temp(source);
    let options = CxxShimOptions::new(
        vec![
            "probe.generic".to_owned(),
            "probe.effectful".to_owned(),
            "probe.borrowed".to_owned(),
            "probe.wide".to_owned(),
            "probe.narrow".to_owned(),
        ],
        DEFAULT_MAX_BYTES,
    )
    .unwrap();
    let envelope = generate(&path, &options).expect("all-excluded envelope still succeeds");
    assert!(envelope.contains("\"reason\":\"generic_function\""));
    assert!(envelope.contains("\"reason\":\"declared_effects\""));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_mode\""));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_type\""));
    assert!(envelope.contains("\"reason\":\"unsupported_result_type\""));
    assert!(envelope.contains("\"admitted\":0,\"excluded\":5"));
    let fragment = verify_envelope(&envelope).expect("verified");
    assert!(fragment.contains("#include <stdint.h>"));
    assert!(fragment.contains("extern \"C\" {"));
    assert!(!fragment.contains("static __attribute__((unused))"));
    cleanup(&path);
}

#[test]
fn private_functions_are_excluded_by_identity_origin() {
    let source = r#"
module test.probe;

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64
ensures result == 1
{
helper(0)
}
"#;
    let path = write_temp(source);
    let options = CxxShimOptions::new(vec!["helper".to_owned()], DEFAULT_MAX_BYTES).unwrap();
    let envelope = generate(&path, &options).expect("envelope");
    assert!(envelope.contains("\"reason\":\"automatic_identity\""));
    cleanup(&path);
}

#[test]
fn byte_budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(VALID_SOURCE);
    let options = CxxShimOptions::new(
        vec!["probe.double".to_owned()],
        graph::MIN_AGENT_CONTEXT_BYTES,
    )
    .unwrap();
    let outcome = generate(&path, &options);
    let errors = outcome.expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-X103"),
        "expected the byte-budget diagnostic"
    );
    cleanup(&path);
}
