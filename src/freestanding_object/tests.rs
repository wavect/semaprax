use super::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-freestanding-{}-{}.spx",
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

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    if double(21) == 42 { 42 } else { 0 }
}
"#;

#[test]
fn options_reject_out_of_bounds_values() {
    assert!(FreestandingObjectOptions::new(512).is_err());
    assert!(FreestandingObjectOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
    assert!(FreestandingObjectOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
}

#[test]
fn symbols_match_the_native_hex_encoding() {
    assert_eq!(c_function_symbol("app.main"), "spx_decl_6170702e6d61696e");
}

#[test]
fn assertion_checks_catch_planted_violations() {
    assert!(run_profile_assertions("int main(void) {}").is_err());
    assert!(run_profile_assertions("void *p = malloc(3);").is_err());
    assert!(run_profile_assertions("sleep(1);").is_err());
    assert!(run_profile_assertions("#include <stdio.h>").is_err());
    assert!(run_profile_assertions("").is_err());
    assert!(run_profile_assertions(INVARIANT_FAILURE_FAILSTOP).is_ok());
}

#[test]
fn golden_unit_has_documented_shape_and_is_deterministic() {
    let path = write_temp(VALID_SOURCE);
    let first = unit_text(&path, &FreestandingObjectOptions::default()).expect("unit");
    let second = unit_text(&path, &FreestandingObjectOptions::default()).expect("unit");
    assert_eq!(first, second);
    assert!(!first.contains(ENTRY_WRAPPER_START));
    assert!(!first.contains("<stdio.h>"));
    assert!(!first.contains("<stdlib.h>"));
    assert!(first.contains(INVARIANT_FAILURE_FAILSTOP));
    assert!(first.contains(
            "spx_status_token spx_decl_70726f62652e646f75626c65(struct spx_context *spx_ctx, int64_t, int64_t *spx_result_out);"
        ));
    cleanup(&path);
}

#[test]
fn envelope_round_trips_through_verify_envelope() {
    let path = write_temp(VALID_SOURCE);
    let envelope = generate(&path, &FreestandingObjectOptions::default()).expect("envelope");
    let verified = verify_envelope(&envelope).expect("verified");
    assert_eq!(
        verified.translation_unit,
        unit_text(&path, &FreestandingObjectOptions::default()).unwrap()
    );
    cleanup(&path);
}

#[test]
fn verify_envelope_detects_tampering() {
    let path = write_temp(VALID_SOURCE);
    let envelope = generate(&path, &FreestandingObjectOptions::default()).expect("envelope");
    let payload_tampered = envelope.replace("\"no_blocking\":true", "\"no_blocking\":false");
    assert!(verify_envelope(&payload_tampered).is_err());
    let truncated = envelope[..envelope.len() - 4].to_owned();
    assert!(verify_envelope(&truncated).is_err());
    assert!(verify_envelope("not json").is_err());
    cleanup(&path);
}

#[test]
fn module_outside_the_scalar_profile_fails_closed() {
    let cases = [
        (
            r#"
module test.bad;
permit { io.release }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "permits",
        ),
        (
            r#"
module test.bad;

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "generic_function",
        ),
        (
            r#"
module test.bad;

@id("probe.wide")
fn wide(ratio: f64) -> f64 { ratio }

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "unsupported_parameter_type",
        ),
        (
            r#"
module test.bad;

@id("probe.ratio")
fn ratio() -> f64 { 1.0 }

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "unsupported_result_type",
        ),
        (
            r#"
module test.bad;

resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
            "type declarations",
        ),
    ];
    for (source, needle) in cases {
        let path = write_temp(source);
        let errors =
            generate(&path, &FreestandingObjectOptions::default()).expect_err("must fail closed");
        assert!(
            errors
                .iter()
                .any(|item| item.code == "SPX-A102" && item.message.contains(needle)),
            "expected SPX-A102 mentioning {needle}: {errors:?}"
        );
        cleanup(&path);
    }
}

#[test]
fn private_functions_are_excluded_by_identity_origin() {
    let source = r#"
module test.probe;

fn helper(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { helper(0) }
"#;
    let path = write_temp(source);
    let errors = generate(&path, &FreestandingObjectOptions::default()).unwrap_err();
    assert!(errors[0].message.contains(REASON_AUTOMATIC_IDENTITY));
    cleanup(&path);
}

#[test]
fn byte_budget_exhaustion_fails_closed_without_truncation() {
    let path = write_temp(VALID_SOURCE);
    let options = FreestandingObjectOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).unwrap();
    let errors = generate(&path, &options).expect_err("tiny budgets must fail closed");
    assert!(
        errors.iter().any(|item| item.code == "SPX-A103"),
        "expected the byte-budget diagnostic"
    );
    cleanup(&path);
}
