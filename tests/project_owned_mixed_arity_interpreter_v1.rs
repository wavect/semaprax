//! Retained-Project reference evaluation for the complete Project-v8 API shape.

use semaprax::interpreter::{OwnedDataCleanupEvent, DEFAULT_MAX_STEPS, MAX_STEPS_LIMIT};
use semaprax::project::{
    with_authenticated_project, ProjectRevision, PublicApiArgument, PublicApiEvaluation,
    PublicApiEvaluationOutcome, PublicApiValue,
};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[path = "support/owned_mixed_arity_product.rs"]
#[allow(dead_code)]
mod mixed;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const FIRST_SLICE: &[u8] = &[0, 255, 128];
const SECOND_SLICE: &[u8] = &[65, 0, 255, 127, 128, 42];

const EXTRA_SOURCE: &str = r#"
@id("reference.bool")
fn bool_value(value: bool) -> bool { value }

@id("reference.bytes")
fn bytes_value(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }

@id("reference.capacity")
fn capacity(text: borrow str, input: borrow Slice<u8>) -> usize {
    byte_len(str_as_bytes(text)) + byte_len(input)
}

@id("reference.failure")
fn failure(divisor: i64) -> Bytes {
    if (1 / divisor) == 1 {
        let output = [111u8, 107u8];
        bytes_copy(array_as_slice(output))
    } else {
        let output = [98u8, 97u8, 100u8];
        bytes_copy(array_as_slice(output))
    }
}

@id("reference.i64")
fn i64_value(value: i64) -> i64 { unselected(value) }

@id("reference.maybe")
fn maybe(input: borrow Slice<u8>, active: bool) -> Option<Bytes> {
    if active {
        Option<Bytes>::Some { value: bytes_copy(input) }
    } else {
        Option<Bytes>::None {}
    }
}

@id("reference.result")
fn result_value(input: borrow Slice<u8>, active: bool, error: i64) -> Result<Bytes, i64> {
    if active {
        Result<Bytes, i64>::Ok { value: bytes_copy(input) }
    } else {
        Result<Bytes, i64>::Err { error: error }
    }
}

@id("reference.usize")
fn usize_value(input: borrow Slice<u8>) -> usize { byte_len(input) }

@id("reference.unselected")
fn unselected(value: i64) -> i64 { value }
"#;

const SELECTED_EXTRA: [&str; 8] = [
    "reference.bool",
    "reference.bytes",
    "reference.capacity",
    "reference.failure",
    "reference.i64",
    "reference.maybe",
    "reference.result",
    "reference.usize",
];

fn canonical(source: &str) -> String {
    let checked = semaprax::check(source, "mixed-reference.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(&canonical, "mixed-reference.spx").unwrap()),
        canonical
    );
    canonical
}

fn fixture() -> (Arc<ProjectRevision>, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-mixed-interpreter-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!("retained mixed interpreter fixture: {}", root.display());
    fs::create_dir(root.join("src")).unwrap();

    let mut source = mixed::source(8);
    source.push_str(EXTRA_SOURCE);
    let source = canonical(&source);
    let tests = canonical("module mixed.tests; @id(\"mixed.tests.main\") fn main() -> i64 { 0 }");
    let mut selected = mixed::selected(8);
    selected.extend(SELECTED_EXTRA.iter().map(|value| (*value).to_owned()));
    selected.sort();
    let exports = selected
        .iter()
        .map(|value| format!("\"{value}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let manifest_text = format!(
        "schema = \"semaprax.project.v8\"\nname = \"mixed-interpreter\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"mixed.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [{exports}]\ntests = [\"mixed.tests\"]\n"
    );
    for (path, bytes) in [
        (root.join("semaprax.toml"), manifest_text.as_bytes()),
        (root.join("src/app.spx"), source.as_bytes()),
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
    assert_eq!(
        revision.public_api_descriptor().unwrap().exports().len(),
        17
    );
    (revision, root)
}

fn clean(root: &Path) {
    let expected = ["semaprax.toml", "src/app.spx", "src/tests.spx"].map(str::to_owned);
    let mut actual = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().into_string().unwrap();
        if name == "src" {
            for child in fs::read_dir(entry.path()).unwrap() {
                let child = child.unwrap();
                actual.push(format!("src/{}", child.file_name().into_string().unwrap()));
            }
        } else {
            actual.push(name);
        }
    }
    actual.sort();
    assert_eq!(actual, expected);
    for relative in ["semaprax.toml", "src/app.spx", "src/tests.spx"] {
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        fs::remove_file(path).unwrap();
    }
    fs::remove_dir(root.join("src")).unwrap();
    fs::remove_dir(root).unwrap();
}

fn evaluate(
    revision: &ProjectRevision,
    id: &str,
    arguments: &[PublicApiArgument<'_>],
) -> PublicApiEvaluation {
    revision
        .evaluate_public_api_v1(id, arguments, DEFAULT_MAX_STEPS)
        .unwrap()
}

fn returned(evaluation: &PublicApiEvaluation, value: PublicApiValue) {
    assert_eq!(
        evaluation.outcome,
        PublicApiEvaluationOutcome::Returned(value)
    );
}

fn mixed_arguments<'a>(first: &'a str, second: &'a str) -> [PublicApiArgument<'a>; 8] {
    [
        PublicApiArgument::I64(-13),
        PublicApiArgument::Bool(true),
        PublicApiArgument::BorrowStr(first),
        PublicApiArgument::BorrowSliceU8(FIRST_SLICE),
        PublicApiArgument::I64(29),
        PublicApiArgument::Bool(false),
        PublicApiArgument::BorrowStr(second),
        PublicApiArgument::BorrowSliceU8(SECOND_SLICE),
    ]
}

#[test]
fn retained_descriptor_drives_zero_through_eight_mixed_argument_positions() {
    let (revision, root) = fixture();
    clean(&root);
    let arguments = mixed_arguments("é\0A", "Z\0λ!");
    for arity in 0..=8 {
        let evaluation = evaluate(
            &revision,
            &format!("mixed.arity{arity}"),
            &arguments[..arity],
        );
        assert_eq!(
            evaluation.function_id.as_str(),
            format!("mixed.arity{arity}")
        );
        returned(&evaluation, PublicApiValue::Bytes(b"ok".to_vec()));
        assert_eq!(
            evaluation.cleanup_events,
            [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
        );

        if arity != 0 {
            let mut wrong = mixed_arguments("é\0A", "Z\0λ!");
            wrong[arity - 1] = match arity - 1 {
                0 => PublicApiArgument::I64(29),
                1 => PublicApiArgument::Bool(false),
                2 => PublicApiArgument::BorrowStr("abcde"),
                3 => PublicApiArgument::BorrowSliceU8(b"four"),
                4 => PublicApiArgument::I64(-13),
                5 => PublicApiArgument::Bool(true),
                6 => PublicApiArgument::BorrowStr("abc"),
                7 => PublicApiArgument::BorrowSliceU8(b"x"),
                _ => unreachable!(),
            };
            let evaluation = evaluate(&revision, &format!("mixed.arity{arity}"), &wrong[..arity]);
            returned(&evaluation, PublicApiValue::Bytes(b"bad".to_vec()));
            assert_eq!(
                evaluation.cleanup_events,
                [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
            );
        }
    }

    let mut swapped = mixed_arguments("é\0A", "Z\0λ!");
    swapped.swap(0, 4);
    swapped.swap(1, 5);
    swapped.swap(2, 6);
    swapped.swap(3, 7);
    returned(
        &evaluate(&revision, "mixed.arity8", &swapped),
        PublicApiValue::Bytes(b"bad".to_vec()),
    );
}

#[test]
fn all_six_result_shapes_extrema_failures_and_cleanup_are_normalized() {
    let (revision, root) = fixture();
    clean(&root);

    for value in [i64::MIN, 0, i64::MAX] {
        let arguments = [PublicApiArgument::I64(value)];
        let evaluation = evaluate(&revision, "reference.i64", &arguments);
        returned(&evaluation, PublicApiValue::I64(value));
        assert!(evaluation.cleanup_events.is_empty());
    }
    for value in [false, true] {
        let arguments = [PublicApiArgument::Bool(value)];
        let evaluation = evaluate(&revision, "reference.bool", &arguments);
        returned(&evaluation, PublicApiValue::Bool(value));
        assert!(evaluation.cleanup_events.is_empty());
    }
    for input in [&[][..], &[0, 255, 128][..]] {
        let arguments = [PublicApiArgument::BorrowSliceU8(input)];
        let evaluation = evaluate(&revision, "reference.usize", &arguments);
        returned(&evaluation, PublicApiValue::Usize(input.len() as u64));
        assert!(evaluation.cleanup_events.is_empty());

        let evaluation = evaluate(&revision, "reference.bytes", &arguments);
        returned(&evaluation, PublicApiValue::Bytes(input.to_vec()));
        assert_eq!(
            evaluation.cleanup_events,
            [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
        );
    }

    let payload = [0, 255, 128];
    for active in [false, true] {
        let arguments = [
            PublicApiArgument::BorrowSliceU8(&payload),
            PublicApiArgument::Bool(active),
        ];
        let evaluation = evaluate(&revision, "reference.maybe", &arguments);
        returned(
            &evaluation,
            PublicApiValue::OptionBytes(active.then(|| payload.to_vec())),
        );
        assert_eq!(
            evaluation.cleanup_events,
            if active {
                vec![OwnedDataCleanupEvent::CopyOutAndSettleBytes]
            } else {
                Vec::new()
            }
        );
    }
    for error in [i64::MIN, 0, i64::MAX] {
        for active in [false, true] {
            let arguments = [
                PublicApiArgument::BorrowSliceU8(&payload),
                PublicApiArgument::Bool(active),
                PublicApiArgument::I64(error),
            ];
            let evaluation = evaluate(&revision, "reference.result", &arguments);
            returned(
                &evaluation,
                PublicApiValue::ResultBytesI64(if active {
                    Ok(payload.to_vec())
                } else {
                    Err(error)
                }),
            );
            assert_eq!(
                evaluation.cleanup_events,
                if active {
                    vec![OwnedDataCleanupEvent::CopyOutAndSettleBytes]
                } else {
                    Vec::new()
                }
            );
        }
    }

    let arguments = [PublicApiArgument::I64(0)];
    let evaluation = evaluate(&revision, "reference.failure", &arguments);
    let PublicApiEvaluationOutcome::LanguageFailure(status) = evaluation.outcome else {
        panic!("expected a normalized language failure")
    };
    assert_eq!(status.domain_id(), "semaprax.arithmetic.v1");
    assert_eq!(status.code(), 4);
    assert!(evaluation.cleanup_events.is_empty());
}

#[test]
fn arguments_are_bounded_and_authenticated_before_evaluation() {
    let (revision, root) = fixture();
    clean(&root);

    let text = "é".repeat(32_767);
    assert_eq!(text.chars().count(), 32_767);
    assert_eq!(text.len(), 65_534);
    for (tail, expected) in [(&[7][..], 65_535_u64), (&[7, 8][..], 65_536)] {
        let arguments = [
            PublicApiArgument::BorrowStr(&text),
            PublicApiArgument::BorrowSliceU8(tail),
        ];
        let evaluation = evaluate(&revision, "reference.capacity", &arguments);
        returned(&evaluation, PublicApiValue::Usize(expected));
        assert!(evaluation.cleanup_events.is_empty());
    }
    let oversized = [7, 8, 9];
    let errors = revision
        .evaluate_public_api_v1(
            "reference.capacity",
            &[
                PublicApiArgument::BorrowStr(&text),
                PublicApiArgument::BorrowSliceU8(&oversized),
            ],
            DEFAULT_MAX_STEPS,
        )
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-F103");
    assert_eq!(
        errors[0].message,
        "public API cumulative borrowed input exceeds 65536 bytes"
    );

    for (arguments, message) in [
        (
            Vec::new(),
            "public API export `reference.i64` takes 1 argument(s), 0 were provided",
        ),
        (
            vec![PublicApiArgument::I64(0), PublicApiArgument::I64(1)],
            "public API export `reference.i64` takes 1 argument(s), 2 were provided",
        ),
        (
            vec![PublicApiArgument::Bool(false)],
            "parameter `value` at ordinal 0 of public API export `reference.i64` expects i64, but the argument is bool",
        ),
    ] {
        let errors = revision
            .evaluate_public_api_v1("reference.i64", &arguments, DEFAULT_MAX_STEPS)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-F103");
        assert_eq!(errors[0].message, message);
    }

    let wrong_order = [
        PublicApiArgument::BorrowSliceU8(b"x"),
        PublicApiArgument::BorrowStr("a"),
    ];
    let errors = revision
        .evaluate_public_api_v1("reference.capacity", &wrong_order, DEFAULT_MAX_STEPS)
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-F103");
    assert_eq!(
        errors[0].message,
        "parameter `text` at ordinal 0 of public API export `reference.capacity` expects borrow-str, but the argument is borrow-slice-u8"
    );

    // This explicitly identified function is retained in a selected closure
    // and has an otherwise admitted signature, but is not itself selected.
    // An admission diagnostic proves descriptor membership wins before entry.
    assert!(revision
        .entry_program()
        .functions
        .iter()
        .any(|function| function.id.as_str() == "reference.unselected"));
    assert!(!revision
        .public_api_descriptor()
        .unwrap()
        .exports()
        .iter()
        .any(|export| export.stable_id().as_str() == "reference.unselected"));
    let errors = revision
        .evaluate_public_api_v1(
            "reference.unselected",
            &[PublicApiArgument::I64(0)],
            DEFAULT_MAX_STEPS,
        )
        .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "SPX-F102");
    assert_eq!(
        errors[0].message,
        "interpreter admission failed (unsupported_callee): retained Project v8 descriptor does not select export `reference.unselected`"
    );

    let invalid_selector_message =
        "interpreter admission failed (unsupported_callee): public API selector is invalid";
    let oversized_selector = "x".repeat(129);
    for entry_id in ["\0", "reference.\ncontrol", oversized_selector.as_str()] {
        let errors = revision
            .evaluate_public_api_v1(entry_id, &[], DEFAULT_MAX_STEPS)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-F102");
        assert_eq!(errors[0].message, invalid_selector_message);
        assert!(!errors[0].message.contains(entry_id));
    }
}

#[test]
fn fuel_is_bounded_before_work_and_exhaustion_publishes_no_owner_cleanup() {
    let (revision, root) = fixture();
    clean(&root);

    let evaluation = revision
        .evaluate_public_api_v1(
            "reference.bytes",
            &[PublicApiArgument::BorrowSliceU8(b"owned")],
            1,
        )
        .unwrap();
    assert_eq!(
        evaluation.outcome,
        PublicApiEvaluationOutcome::FuelExhausted
    );
    assert_eq!(evaluation.steps_used, 1);
    assert_eq!(evaluation.max_steps, 1);
    assert!(evaluation.cleanup_events.is_empty());

    for max_steps in [0, MAX_STEPS_LIMIT + 1] {
        let errors = revision
            .evaluate_public_api_v1("reference.i64", &[PublicApiArgument::I64(1)], max_steps)
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-F101");
        assert_eq!(
            errors[0].message,
            format!("public API evaluation max_steps must be between 1 and {MAX_STEPS_LIMIT}")
        );
    }
}
