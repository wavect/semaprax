//! Retained-Project reference evaluation for the closed Project-v10 API.

use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{DEFAULT_MAX_STEPS, MAX_STEPS_LIMIT};
use semaprax::project::{
    with_authenticated_project, OwnedUtf8ApiEvaluation, OwnedUtf8ApiEvaluationOutcome,
    OwnedUtf8ApiValue, OwnedUtf8SettlementEvent, ProjectRevision, PublicApiArgument,
    MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS, MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn canonical(source: &str, path: &str) -> String {
    let parsed = semaprax::parse(source, Path::new(path)).unwrap();
    let output = semaprax::format::canonical(&parsed);
    semaprax::check(&output, path).unwrap();
    output
}

fn fixture() -> (Fixture, std::sync::Arc<ProjectRevision>) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-utf8-interpreter-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    fs::create_dir(root.join("src")).unwrap();
    let mut exports = (0..=8)
        .map(|arity| format!("mixed.arity{arity}"))
        .collect::<Vec<_>>();
    exports.extend(
        [
            "v10.automatic",
            "v10.bool",
            "v10.bytes",
            "v10.empty",
            "v10.failure",
            "v10.i64",
            "v10.input-total",
            "v10.maybe",
            "v10.result",
            "v10.usize",
            "v10.utf8",
        ]
        .map(str::to_owned),
    );
    let selected = exports
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest = format!(
        "schema = \"semaprax.project.v10\"\nname = \"owned-utf8-interpreter\"\nversion = \"0.1.0\"\nprofile = \"owned-utf8-api.v1\"\nentry = \"v10.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [{selected}]\ntests = [\"v10.tests\"]\n"
    );
    let mut app_source = String::from(
        r#"module v10.app;

fn automatic_text() -> string { "automatic" }

@id("v10.automatic")
fn automatic_value() -> string { automatic_text() }

@id("v10.bool")
fn bool_value(value: bool) -> bool { value }

@id("v10.bytes")
fn bytes_value(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }

@id("v10.empty")
fn empty_value() -> string { "" }

@id("v10.failure")
fn failure(divisor: i64) -> Bytes {
    if (1 / divisor) == 1 {
        let output = [111u8, 107u8];
        bytes_copy(array_as_slice(output))
    } else {
        let output = [98u8, 97u8, 100u8];
        bytes_copy(array_as_slice(output))
    }
}

@id("v10.i64")
fn i64_value(value: i64) -> i64 { value }

@id("v10.input-total")
fn input_total(text: borrow str, input: borrow Slice<u8>) -> usize {
    byte_len(str_as_bytes(text)) + byte_len(input)
}

@id("v10.maybe")
fn maybe(input: borrow Slice<u8>, active: bool) -> Option<Bytes> {
    if active {
        Option<Bytes>::Some { value: bytes_copy(input) }
    } else {
        Option<Bytes>::None {}
    }
}

@id("v10.result")
fn result_value(input: borrow Slice<u8>, active: bool, error: i64) -> Result<Bytes, i64> {
    if active {
        Result<Bytes, i64>::Ok { value: bytes_copy(input) }
    } else {
        Result<Bytes, i64>::Err { error: error }
    }
}

@id("v10.usize")
fn usize_value(input: borrow Slice<u8>) -> usize { byte_len(input) }

@id("v10.utf8")
fn utf8_value() -> string { "\u{feff}\u{0}\u{e9}\u{1f980}" }

@id("v10.hidden")
fn hidden() -> i64 { 9 }
"#,
    );
    let parameters = [
        "p0: i64",
        "p1: bool",
        "p2: borrow str",
        "p3: borrow Slice<u8>",
        "p4: i64",
        "p5: bool",
        "p6: borrow str",
        "p7: borrow Slice<u8>",
    ];
    let predicates = [
        "p0 == (0 - 13)",
        "p1 == true",
        "byte_len(str_as_bytes(p2)) == 4usize",
        "byte_len(p3) == 3usize",
        "p4 == 29",
        "p5 == false",
        "byte_len(str_as_bytes(p6)) == 5usize",
        "byte_len(p7) == 6usize",
    ];
    for arity in 0..=8 {
        let signature = parameters[..arity].join(", ");
        let condition = if arity == 0 {
            "true".to_owned()
        } else {
            predicates[..arity].join(" && ")
        };
        app_source.push_str(&format!(
            "@id(\"mixed.arity{arity}\") fn arity{arity}({signature}) -> Bytes {{\n\
             if {condition} {{ let output = [111u8, 107u8]; bytes_copy(array_as_slice(output)) }} \
             else {{ let output = [98u8, 97u8, 100u8]; bytes_copy(array_as_slice(output)) }}\n}}\n"
        ));
    }
    app_source.push_str("@id(\"v10.app.main\") fn main() -> i64 { 0 }\n");
    let app = canonical(&app_source, "src/app.spx");
    let tests = canonical(
        "module v10.tests; @id(\"v10.tests.main\") fn main() -> i64 { 0 }",
        "src/tests.spx",
    );
    for (path, bytes) in [
        (root.join("semaprax.toml"), manifest.as_bytes()),
        (root.join("src/app.spx"), app.as_bytes()),
        (root.join("src/tests.spx"), tests.as_bytes()),
    ] {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
    let revision = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    (Fixture(root), revision)
}

fn evaluate(
    revision: &ProjectRevision,
    id: &str,
    arguments: &[PublicApiArgument<'_>],
) -> OwnedUtf8ApiEvaluation {
    revision
        .evaluate_owned_utf8_api_v1(id, arguments, DEFAULT_MAX_STEPS)
        .unwrap()
}

fn returned(evaluation: &OwnedUtf8ApiEvaluation, expected: OwnedUtf8ApiValue) {
    assert_eq!(
        evaluation.outcome,
        OwnedUtf8ApiEvaluationOutcome::Returned(expected)
    );
    assert_eq!(
        evaluation.utf8_materializations_max,
        MAX_OWNED_UTF8_LOGICAL_ALLOCATIONS
    );
    assert_eq!(
        evaluation.utf8_bytes_max,
        MAX_OWNED_UTF8_LOGICAL_ALLOCATION_BYTES
    );
    assert!(evaluation.utf8_materializations_used <= evaluation.utf8_materializations_max);
    assert!(evaluation.utf8_bytes_used <= evaluation.utf8_bytes_max);
}

#[test]
fn retained_v10_evaluator_normalizes_all_seven_results_and_settlement() {
    let (_fixture, revision) = fixture();
    let bytes = [0, 255, 128];

    let scalar_cases = [
        (
            "v10.i64",
            vec![PublicApiArgument::I64(i64::MIN)],
            OwnedUtf8ApiValue::I64(i64::MIN),
        ),
        (
            "v10.bool",
            vec![PublicApiArgument::Bool(true)],
            OwnedUtf8ApiValue::Bool(true),
        ),
        (
            "v10.usize",
            vec![PublicApiArgument::BorrowSliceU8(&bytes)],
            OwnedUtf8ApiValue::Usize(3),
        ),
    ];
    for (id, arguments, expected) in scalar_cases {
        let evaluation = evaluate(&revision, id, &arguments);
        returned(&evaluation, expected);
        assert!(evaluation.settlement_events.is_empty());
    }

    let evaluation = evaluate(
        &revision,
        "v10.bytes",
        &[PublicApiArgument::BorrowSliceU8(&bytes)],
    );
    returned(&evaluation, OwnedUtf8ApiValue::Bytes(bytes.to_vec()));
    assert_eq!(
        evaluation.settlement_events,
        [OwnedUtf8SettlementEvent::CopyOutAndSettleBytes]
    );

    for active in [false, true] {
        let evaluation = evaluate(
            &revision,
            "v10.maybe",
            &[
                PublicApiArgument::BorrowSliceU8(&bytes),
                PublicApiArgument::Bool(active),
            ],
        );
        returned(
            &evaluation,
            OwnedUtf8ApiValue::OptionBytes(active.then(|| bytes.to_vec())),
        );
        assert_eq!(
            evaluation.settlement_events,
            if active {
                vec![OwnedUtf8SettlementEvent::CopyOutAndSettleBytes]
            } else {
                Vec::new()
            }
        );
    }

    for active in [false, true] {
        let evaluation = evaluate(
            &revision,
            "v10.result",
            &[
                PublicApiArgument::BorrowSliceU8(&bytes),
                PublicApiArgument::Bool(active),
                PublicApiArgument::I64(-7),
            ],
        );
        returned(
            &evaluation,
            OwnedUtf8ApiValue::ResultBytesI64(if active { Ok(bytes.to_vec()) } else { Err(-7) }),
        );
        assert_eq!(
            evaluation.settlement_events,
            if active {
                vec![OwnedUtf8SettlementEvent::CopyOutAndSettleBytes]
            } else {
                Vec::new()
            }
        );
    }

    let evaluation = evaluate(&revision, "v10.utf8", &[]);
    returned(
        &evaluation,
        OwnedUtf8ApiValue::Utf8("\u{feff}\0\u{e9}\u{1f980}".to_owned()),
    );
    assert_eq!(
        evaluation.settlement_events,
        [OwnedUtf8SettlementEvent::CopyOutAndSettleUtf8]
    );

    let evaluation = evaluate(&revision, "v10.empty", &[]);
    returned(&evaluation, OwnedUtf8ApiValue::Utf8(String::new()));
    assert_eq!(
        evaluation.settlement_events,
        [OwnedUtf8SettlementEvent::CopyOutAndSettleUtf8]
    );
}

#[test]
fn retained_v10_descriptor_and_invocation_shape_win_before_evaluation() {
    let (_fixture, revision) = fixture();
    let invalid_message =
        "interpreter admission failed (unsupported_callee): owned UTF-8 API selector is invalid";
    let oversized = "x".repeat(129);
    for selector in ["\0", "v10.\ncontrol", oversized.as_str()] {
        let error = revision
            .evaluate_owned_utf8_api_v1(selector, &[], DEFAULT_MAX_STEPS)
            .unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-F102");
        assert_eq!(error[0].message, invalid_message);
        assert!(!error[0].message.contains(selector));
    }

    let error = revision
        .evaluate_owned_utf8_api_v1("v10.hidden", &[], DEFAULT_MAX_STEPS)
        .unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-F102");

    let error = revision
        .evaluate_owned_utf8_api_v1("v10.bool", &[PublicApiArgument::I64(1)], DEFAULT_MAX_STEPS)
        .unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-F103");
    assert_eq!(
        error[0].message,
        "parameter `value` at ordinal 0 of owned UTF-8 API export `v10.bool` expects bool, but the argument is i64"
    );

    let error = revision
        .evaluate_owned_utf8_api_v1("v10.bool", &[], DEFAULT_MAX_STEPS)
        .unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-F103");
    assert_eq!(
        error[0].message,
        "owned UTF-8 API export `v10.bool` takes 1 argument(s), 0 were provided"
    );

    let wrong_order = [
        PublicApiArgument::BorrowSliceU8(b"x"),
        PublicApiArgument::BorrowStr("x"),
    ];
    let error = revision
        .evaluate_owned_utf8_api_v1("v10.input-total", &wrong_order, DEFAULT_MAX_STEPS)
        .unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-F103");
    assert_eq!(
        error[0].message,
        "parameter `text` at ordinal 0 of owned UTF-8 API export `v10.input-total` expects borrow-str, but the argument is borrow-slice-u8"
    );

    for max_steps in [0, MAX_STEPS_LIMIT + 1] {
        let error = revision
            .evaluate_owned_utf8_api_v1("v10.bool", &[PublicApiArgument::Bool(true)], max_steps)
            .unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-F101");
        assert_eq!(
            error[0].message,
            format!("owned UTF-8 API evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}")
        );
    }
}

#[test]
fn wrong_project_profile_rejects_before_selector_or_evaluation() {
    let manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/frame-payload-project/semaprax.toml");
    with_authenticated_project(&manifest, |snapshot| {
        let revision = snapshot.retain_revision();
        let error = revision
            .evaluate_owned_utf8_api_v1("\0", &[], 0)
            .unwrap_err();
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-J105");
        assert_eq!(
            error[0].message,
            "public owned UTF-8 API description requires Project v10 owned-utf8-api.v1"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn retained_v10_mixed_arity_and_borrowed_input_preflight_are_exact() {
    let (_fixture, revision) = fixture();
    let first = [0, 255, 128];
    let second = [65, 0, 255, 127, 128, 42];
    let arguments = [
        PublicApiArgument::I64(-13),
        PublicApiArgument::Bool(true),
        PublicApiArgument::BorrowStr("é\0A"),
        PublicApiArgument::BorrowSliceU8(&first),
        PublicApiArgument::I64(29),
        PublicApiArgument::Bool(false),
        PublicApiArgument::BorrowStr("Z\0λ!"),
        PublicApiArgument::BorrowSliceU8(&second),
    ];
    for arity in 0..=8 {
        let evaluation = evaluate(
            &revision,
            &format!("mixed.arity{arity}"),
            &arguments[..arity],
        );
        returned(&evaluation, OwnedUtf8ApiValue::Bytes(b"ok".to_vec()));
        assert_eq!(
            evaluation.settlement_events,
            [OwnedUtf8SettlementEvent::CopyOutAndSettleBytes]
        );
    }

    let text = "é".repeat(32_767);
    assert_eq!(text.len(), 65_534);
    let evaluation = evaluate(
        &revision,
        "v10.input-total",
        &[
            PublicApiArgument::BorrowStr(&text),
            PublicApiArgument::BorrowSliceU8(&[7, 8]),
        ],
    );
    returned(&evaluation, OwnedUtf8ApiValue::Usize(65_536));
    assert_eq!(evaluation.utf8_materializations_used, 0);
    assert_eq!(evaluation.utf8_bytes_used, 0);

    let error = revision
        .evaluate_owned_utf8_api_v1(
            "v10.input-total",
            &[
                PublicApiArgument::BorrowStr(&text),
                PublicApiArgument::BorrowSliceU8(&[7, 8, 9]),
            ],
            DEFAULT_MAX_STEPS,
        )
        .unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-F103");
    assert_eq!(
        error[0].message,
        "owned UTF-8 API cumulative borrowed input exceeds 65536 bytes"
    );
}

#[test]
fn automatic_helpers_failures_and_repeated_invocations_preserve_boundaries() {
    let (_fixture, revision) = fixture();

    let automatic = evaluate(&revision, "v10.automatic", &[]);
    returned(&automatic, OwnedUtf8ApiValue::Utf8("automatic".to_owned()));
    assert_eq!(
        automatic.settlement_events,
        [OwnedUtf8SettlementEvent::CopyOutAndSettleUtf8]
    );

    let language_failure = evaluate(&revision, "v10.failure", &[PublicApiArgument::I64(0)]);
    assert!(matches!(
        language_failure.outcome,
        OwnedUtf8ApiEvaluationOutcome::LanguageFailure(_)
    ));
    assert!(language_failure.settlement_events.is_empty());

    let fuel = revision
        .evaluate_owned_utf8_api_v1(
            "v10.bytes",
            &[PublicApiArgument::BorrowSliceU8(b"owned")],
            1,
        )
        .unwrap();
    assert_eq!(fuel.outcome, OwnedUtf8ApiEvaluationOutcome::FuelExhausted);
    assert!(fuel.settlement_events.is_empty());

    let first = evaluate(&revision, "v10.utf8", &[]);
    let second = evaluate(&revision, "v10.utf8", &[]);
    assert_eq!(
        first.utf8_materializations_used,
        second.utf8_materializations_used
    );
    assert_eq!(first.utf8_bytes_used, second.utf8_bytes_used);
    assert_ne!(first.utf8_materializations_used, 0);
    assert_ne!(first.utf8_bytes_used, 0);
}
