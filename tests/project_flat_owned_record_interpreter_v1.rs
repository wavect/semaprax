//! Retained-Project reference evaluation for Project-v9 flat owned records.

use semaprax::interpreter::{OwnedDataCleanupEvent, DEFAULT_MAX_STEPS, MAX_STEPS_LIMIT};
use semaprax::project::{
    with_authenticated_project, FlatOwnedRecordEvaluation, FlatOwnedRecordEvaluationOutcome,
    FlatOwnedRecordMemberValue, ProjectRevision, PublicApiArgument,
};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

#[path = "support/flat_record_product.rs"]
mod subject;

static SERIAL: AtomicU64 = AtomicU64::new(0);

const EXTREMA_SOURCE: &str = r#"module extrema.app;
@id("extrema.Record")
record Record {
    @id("extrema.low") low: i64,
    @id("extrema.flag") flag: bool,
    @id("extrema.maximum") maximum: usize,
    @id("extrema.bytes") bytes: Bytes,
}
@id("extrema.value")
fn value(input: borrow Slice<u8>, low: i64, flag: bool) -> Record {
    Record {
        low: low,
        flag: flag,
        maximum: 18446744073709551615usize,
        bytes: bytes_copy(input),
    }
}
@id("extrema.cumulative")
fn cumulative(text: borrow str, input: borrow Slice<u8>) -> Record {
    Record {
        low: 0,
        flag: true,
        maximum: byte_len(str_as_bytes(text)) + byte_len(input),
        bytes: bytes_copy(input),
    }
}
@id("extrema.eight")
fn eight(
    first: i64,
    first_flag: bool,
    first_text: borrow str,
    first_bytes: borrow Slice<u8>,
    second: i64,
    second_flag: bool,
    second_text: borrow str,
    second_bytes: borrow Slice<u8>
) -> Record {
    Record {
        low: first + second,
        flag: first_flag && !second_flag,
        maximum: byte_len(str_as_bytes(first_text)) + byte_len(first_bytes)
            + byte_len(str_as_bytes(second_text)) + byte_len(second_bytes),
        bytes: bytes_copy(second_bytes),
    }
}
@id("extrema.zero")
fn zero() -> Record {
    let empty = [0u8; 0];
    Record { low: 0, flag: false, maximum: 0usize, bytes: bytes_copy(array_as_slice(empty)) }
}
@id("extrema.unselected")
fn unselected(input: borrow Slice<u8>) -> Record {
    Record { low: 0, flag: false, maximum: 0usize, bytes: bytes_copy(input) }
}
@id("extrema.main") fn main() -> i64 { 0 }
"#;

fn fixture() -> (Arc<ProjectRevision>, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-flat-record-interpreter-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let manifest = subject::write_project(&root, false);
    let revision = with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    assert_eq!(
        revision
            .flat_owned_record_api_descriptor()
            .unwrap()
            .exports()
            .len(),
        2
    );
    (revision, root)
}

fn extrema_fixture() -> (Arc<ProjectRevision>, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-flat-record-extrema-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    fs::create_dir(root.join("src")).unwrap();
    let source =
        semaprax::format::canonical(&semaprax::check(EXTREMA_SOURCE, "extrema.spx").unwrap());
    let tests = semaprax::format::canonical(
        &semaprax::check(
            "module extrema.tests; @id(\"extrema.tests.main\") fn main() -> i64 { 0 }",
            "tests.spx",
        )
        .unwrap(),
    );
    let manifest = b"schema = \"semaprax.project.v9\"\nname = \"flat-extrema\"\nversion = \"0.1.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"extrema.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"extrema.cumulative\", \"extrema.eight\", \"extrema.value\", \"extrema.zero\"]\ntests = [\"extrema.tests\"]\n";
    for (path, bytes) in [
        (root.join("semaprax.toml"), manifest.as_slice()),
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
    (revision, root)
}

fn clean(root: &Path) {
    let expected = [
        "semaprax.toml",
        "src/app.spx",
        "src/left.spx",
        "src/right.spx",
        "src/tests.spx",
    ];
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
    for relative in expected.into_iter().rev() {
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
        fs::remove_file(path).unwrap();
    }
    fs::remove_dir(root.join("src")).unwrap();
    fs::remove_dir(root).unwrap();
}

fn clean_extrema(root: &Path) {
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
    input: &[u8],
    divisor: i64,
    valid: bool,
) -> FlatOwnedRecordEvaluation {
    revision
        .evaluate_flat_owned_record_api_v1(
            id,
            &[
                PublicApiArgument::BorrowSliceU8(input),
                PublicApiArgument::I64(divisor),
                PublicApiArgument::Bool(valid),
            ],
            DEFAULT_MAX_STEPS,
        )
        .unwrap()
}

fn returned(
    evaluation: FlatOwnedRecordEvaluation,
    function_id: &str,
    record_id: &str,
) -> Vec<(String, FlatOwnedRecordMemberValue)> {
    assert_eq!(evaluation.function_id.as_str(), function_id);
    assert_eq!(evaluation.max_steps, DEFAULT_MAX_STEPS);
    assert!((1..=DEFAULT_MAX_STEPS).contains(&evaluation.steps_used));
    assert_eq!(
        evaluation.cleanup_events,
        [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
    );
    let FlatOwnedRecordEvaluationOutcome::Returned(value) = evaluation.outcome else {
        panic!("expected a returned flat owned-record value")
    };
    assert_eq!(value.record_id.as_str(), record_id);
    value
        .fields
        .into_iter()
        .map(|field| (field.field_id.as_str().to_owned(), field.value))
        .collect()
}

#[test]
fn descriptor_order_controls_byte_first_and_byte_last_publication_at_boundaries() {
    let (revision, root) = fixture();
    clean(&root);
    for input in [&[][..], &vec![0xa5; 65_536][..]] {
        assert_eq!(
            returned(
                evaluate(&revision, "left.payload", input, -1, false),
                "left.payload",
                "left.Payload\u{8}\u{c}\u{7f}\u{85}",
            ),
            [
                (
                    "".to_owned(),
                    FlatOwnedRecordMemberValue::Bytes(input.to_vec())
                ),
                (
                    "left.count".to_owned(),
                    FlatOwnedRecordMemberValue::I64(-84)
                ),
                (
                    "left.valid".to_owned(),
                    FlatOwnedRecordMemberValue::Bool(false)
                ),
                (
                    "left.size".to_owned(),
                    FlatOwnedRecordMemberValue::Usize(input.len() as u64)
                ),
            ]
        );
        assert_eq!(
            returned(
                evaluate(&revision, "right.payload", input, 1, true),
                "right.payload",
                "right.Payload",
            ),
            [
                (
                    "right.size".to_owned(),
                    FlatOwnedRecordMemberValue::Usize(input.len() as u64)
                ),
                (
                    "right.valid".to_owned(),
                    FlatOwnedRecordMemberValue::Bool(true)
                ),
                (
                    "right.count".to_owned(),
                    FlatOwnedRecordMemberValue::I64(84)
                ),
                (
                    "right.bytes".to_owned(),
                    FlatOwnedRecordMemberValue::Bytes(input.to_vec())
                ),
            ]
        );
    }
}

#[test]
fn failure_before_or_after_byte_creation_never_publishes_partial_fields() {
    let (revision, root) = fixture();
    clean(&root);
    for id in ["left.payload", "right.payload"] {
        let evaluation = evaluate(&revision, id, b"private", 0, true);
        assert_eq!(evaluation.function_id.as_str(), id);
        assert_eq!(evaluation.max_steps, DEFAULT_MAX_STEPS);
        assert!((1..=DEFAULT_MAX_STEPS).contains(&evaluation.steps_used));
        let FlatOwnedRecordEvaluationOutcome::LanguageFailure(status) = evaluation.outcome else {
            panic!("expected normalized division failure for {id}")
        };
        assert_eq!(status.domain_id(), "semaprax.arithmetic.v1");
        assert_eq!(status.code(), 4);
        assert!(evaluation.cleanup_events.is_empty());
    }
}

#[test]
fn scalar_extrema_and_explicit_unselected_export_remain_descriptor_bound() {
    let (revision, root) = extrema_fixture();
    clean_extrema(&root);
    for (low, flag) in [(i64::MIN, false), (i64::MAX, true)] {
        let evaluation = revision
            .evaluate_flat_owned_record_api_v1(
                "extrema.value",
                &[
                    PublicApiArgument::BorrowSliceU8(b"edge"),
                    PublicApiArgument::I64(low),
                    PublicApiArgument::Bool(flag),
                ],
                DEFAULT_MAX_STEPS,
            )
            .unwrap();
        assert_eq!(
            returned(evaluation, "extrema.value", "extrema.Record"),
            [
                (
                    "extrema.low".to_owned(),
                    FlatOwnedRecordMemberValue::I64(low)
                ),
                (
                    "extrema.flag".to_owned(),
                    FlatOwnedRecordMemberValue::Bool(flag)
                ),
                (
                    "extrema.maximum".to_owned(),
                    FlatOwnedRecordMemberValue::Usize(u64::MAX)
                ),
                (
                    "extrema.bytes".to_owned(),
                    FlatOwnedRecordMemberValue::Bytes(b"edge".to_vec())
                ),
            ]
        );
    }
    let zero = revision
        .evaluate_flat_owned_record_api_v1("extrema.zero", &[], DEFAULT_MAX_STEPS)
        .unwrap();
    assert_eq!(
        returned(zero, "extrema.zero", "extrema.Record"),
        [
            ("extrema.low".to_owned(), FlatOwnedRecordMemberValue::I64(0)),
            (
                "extrema.flag".to_owned(),
                FlatOwnedRecordMemberValue::Bool(false)
            ),
            (
                "extrema.maximum".to_owned(),
                FlatOwnedRecordMemberValue::Usize(0)
            ),
            (
                "extrema.bytes".to_owned(),
                FlatOwnedRecordMemberValue::Bytes(Vec::new())
            ),
        ]
    );
    let eight = revision
        .evaluate_flat_owned_record_api_v1(
            "extrema.eight",
            &[
                PublicApiArgument::I64(-5),
                PublicApiArgument::Bool(true),
                PublicApiArgument::BorrowStr("é"),
                PublicApiArgument::BorrowSliceU8(b"abc"),
                PublicApiArgument::I64(17),
                PublicApiArgument::Bool(false),
                PublicApiArgument::BorrowStr("λ"),
                PublicApiArgument::BorrowSliceU8(b"last"),
            ],
            DEFAULT_MAX_STEPS,
        )
        .unwrap();
    assert_eq!(
        returned(eight, "extrema.eight", "extrema.Record"),
        [
            (
                "extrema.low".to_owned(),
                FlatOwnedRecordMemberValue::I64(12)
            ),
            (
                "extrema.flag".to_owned(),
                FlatOwnedRecordMemberValue::Bool(true)
            ),
            (
                "extrema.maximum".to_owned(),
                FlatOwnedRecordMemberValue::Usize(11)
            ),
            (
                "extrema.bytes".to_owned(),
                FlatOwnedRecordMemberValue::Bytes(b"last".to_vec())
            ),
        ]
    );

    let text = "é".repeat(32_767);
    assert_eq!(text.len(), 65_534);
    for (tail, total) in [(&[7][..], 65_535_u64), (&[7, 8][..], 65_536)] {
        let evaluation = revision
            .evaluate_flat_owned_record_api_v1(
                "extrema.cumulative",
                &[
                    PublicApiArgument::BorrowStr(&text),
                    PublicApiArgument::BorrowSliceU8(tail),
                ],
                DEFAULT_MAX_STEPS,
            )
            .unwrap();
        assert_eq!(
            returned(evaluation, "extrema.cumulative", "extrema.Record"),
            [
                ("extrema.low".to_owned(), FlatOwnedRecordMemberValue::I64(0)),
                (
                    "extrema.flag".to_owned(),
                    FlatOwnedRecordMemberValue::Bool(true)
                ),
                (
                    "extrema.maximum".to_owned(),
                    FlatOwnedRecordMemberValue::Usize(total)
                ),
                (
                    "extrema.bytes".to_owned(),
                    FlatOwnedRecordMemberValue::Bytes(tail.to_vec())
                ),
            ]
        );
    }
    let errors = revision
        .evaluate_flat_owned_record_api_v1(
            "extrema.cumulative",
            &[
                PublicApiArgument::BorrowStr(&text),
                PublicApiArgument::BorrowSliceU8(&[7, 8, 9]),
            ],
            DEFAULT_MAX_STEPS,
        )
        .unwrap_err();
    assert_eq!(errors[0].code, "SPX-F103");
    assert!(errors[0].message.contains("exceeds 65536 bytes"));

    let errors = revision
        .evaluate_flat_owned_record_api_v1(
            "extrema.unselected",
            &[PublicApiArgument::BorrowSliceU8(b"edge")],
            DEFAULT_MAX_STEPS,
        )
        .unwrap_err();
    assert_eq!(errors[0].code, "SPX-F102");
    assert!(errors[0]
        .message
        .contains("does not select export `extrema.unselected`"));
}

#[test]
fn selector_argument_borrowed_capacity_and_fuel_guards_precede_publication() {
    let (revision, root) = fixture();
    clean(&root);
    let valid = [
        PublicApiArgument::BorrowSliceU8(b"x"),
        PublicApiArgument::I64(i64::MAX),
        PublicApiArgument::Bool(true),
    ];
    for selector in ["bad\nselector", "bad\nselector-secret"] {
        let errors = revision
            .evaluate_flat_owned_record_api_v1(selector, &valid, DEFAULT_MAX_STEPS)
            .unwrap_err();
        assert_eq!(errors[0].code, "SPX-F102");
        assert_eq!(
            errors[0].message,
            "interpreter admission failed (unsupported_callee): flat owned-record selector is invalid"
        );
        assert!(!errors[0].message.contains("secret"));
    }
    let errors = revision
        .evaluate_flat_owned_record_api_v1("product.main", &[], DEFAULT_MAX_STEPS)
        .unwrap_err();
    assert_eq!(errors[0].code, "SPX-F102");
    assert!(errors[0]
        .message
        .contains("does not select export `product.main`"));
    for arguments in [
        &valid[..2],
        &[
            PublicApiArgument::I64(i64::MIN),
            PublicApiArgument::BorrowSliceU8(b"x"),
            PublicApiArgument::Bool(false),
        ][..],
        &[
            PublicApiArgument::BorrowSliceU8(b"x"),
            PublicApiArgument::Bool(false),
            PublicApiArgument::I64(i64::MIN),
        ][..],
    ] {
        let errors = revision
            .evaluate_flat_owned_record_api_v1("left.payload", arguments, DEFAULT_MAX_STEPS)
            .unwrap_err();
        assert_eq!(errors[0].code, "SPX-F103");
    }
    let descriptor = revision.flat_owned_record_api_descriptor().unwrap();
    let left = descriptor
        .exports()
        .iter()
        .find(|export| export.stable_id().as_str() == "left.payload")
        .unwrap();
    let persistent_parameter_id = &left.parameters()[0].0;
    let wrong_type = [
        PublicApiArgument::I64(i64::MIN),
        PublicApiArgument::I64(1),
        PublicApiArgument::Bool(false),
    ];
    let errors = revision
        .evaluate_flat_owned_record_api_v1("left.payload", &wrong_type, DEFAULT_MAX_STEPS)
        .unwrap_err();
    assert_eq!(errors[0].code, "SPX-F103");
    assert_eq!(
        errors[0].message,
        "parameter `input` at ordinal 0 of flat owned-record export `left.payload` expects borrow-slice-u8, but the argument is i64"
    );
    assert!(!errors[0]
        .message
        .contains(&format!("parameter `{persistent_parameter_id}`")));
    let oversized = vec![0_u8; 65_537];
    let errors = revision
        .evaluate_flat_owned_record_api_v1(
            "left.payload",
            &[
                PublicApiArgument::BorrowSliceU8(&oversized),
                PublicApiArgument::I64(1),
                PublicApiArgument::Bool(true),
            ],
            DEFAULT_MAX_STEPS,
        )
        .unwrap_err();
    assert_eq!(errors[0].code, "SPX-F103");
    assert!(errors[0].message.contains("exceeds 65536 bytes"));
    for max_steps in [0, MAX_STEPS_LIMIT + 1] {
        assert!(revision
            .evaluate_flat_owned_record_api_v1("left.payload", &valid, max_steps)
            .is_err());
    }
    let evaluation = revision
        .evaluate_flat_owned_record_api_v1("left.payload", &valid, 1)
        .unwrap();
    assert_eq!(evaluation.function_id.as_str(), "left.payload");
    assert_eq!(evaluation.max_steps, 1);
    assert_eq!(evaluation.steps_used, 1);
    assert_eq!(
        evaluation.outcome,
        FlatOwnedRecordEvaluationOutcome::FuelExhausted
    );
    assert!(evaluation.cleanup_events.is_empty());
}
