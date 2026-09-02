//! Retained-Project interpreter evidence for synchronous borrowed calls over
//! owned `Bytes` places and one direct owned-record field.

use semaprax::interpreter::{OwnedDataCleanupEvent, DEFAULT_MAX_STEPS};
use semaprax::project::{
    with_authenticated_project, ProjectRevision, PublicApiArgument, PublicApiEvaluationOutcome,
    PublicApiValue,
};
use std::fs::{self, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = include_str!("../project_borrowed_bytes_call_interpreter_v1/source.spx");
const PAYLOAD: &[u8] = &[0, 255, 128, 65, 0, 42];

fn canonical(source: &str, name: &str) -> String {
    let checked = semaprax::check(source, name).unwrap();
    let canonical = semaprax::format::canonical(&checked);
    assert_eq!(
        semaprax::format::canonical(&semaprax::check(&canonical, name).unwrap()),
        canonical
    );
    canonical
}

fn fixture() -> (Arc<ProjectRevision>, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "semaprax-borrowed-bytes-call-interpreter-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    fs::create_dir(root.join("src")).unwrap();
    let source = canonical(SOURCE, "borrowed-call.spx");
    let tests = canonical(
        "module borrowed.call.tests; @id(\"borrowed.tests.main\") fn main() -> i64 { 0 }",
        "tests.spx",
    );
    let manifest = r#"schema = "semaprax.project.v8"
name = "borrowed-call-interpreter"
version = "0.1.0"
profile = "owned-data-api.v1"
entry = "borrowed.call.interpreter"
sources = ["src/app.spx", "src/tests.spx"]
web_exports = ["borrowed.failure", "borrowed.field", "borrowed.root"]
tests = ["borrowed.call.tests"]
"#;
    for (path, bytes) in [
        (root.join("semaprax.toml"), manifest.as_bytes()),
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
    for relative in ["semaprax.toml", "src/app.spx", "src/tests.spx"] {
        fs::remove_file(root.join(relative)).unwrap();
    }
    fs::remove_dir(root.join("src")).unwrap();
    fs::remove_dir(root).unwrap();
}

fn assert_bytes(revision: &ProjectRevision, id: &str, arguments: &[PublicApiArgument<'_>]) {
    let evaluation = revision
        .evaluate_public_api_v1(id, arguments, DEFAULT_MAX_STEPS)
        .unwrap();
    assert_eq!(
        evaluation.outcome,
        PublicApiEvaluationOutcome::Returned(PublicApiValue::Bytes(PAYLOAD.to_vec()))
    );
    assert_eq!(
        evaluation.cleanup_events,
        [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
    );
}

#[test]
fn exact_places_alias_one_allocation_and_remain_owned_after_success_or_failure() {
    let (revision, root) = fixture();
    clean(&root);

    let borrowed = [PublicApiArgument::BorrowSliceU8(PAYLOAD)];
    assert_bytes(&revision, "borrowed.root", &borrowed);
    assert_bytes(&revision, "borrowed.field", &borrowed);

    let failed = revision
        .evaluate_public_api_v1(
            "borrowed.failure",
            &[
                PublicApiArgument::BorrowSliceU8(PAYLOAD),
                PublicApiArgument::I64(0),
            ],
            DEFAULT_MAX_STEPS,
        )
        .unwrap();
    let PublicApiEvaluationOutcome::LanguageFailure(status) = failed.outcome else {
        panic!("borrowed callee failure was not normalized")
    };
    assert_eq!(status.domain_id(), "semaprax.arithmetic.v1");
    assert_eq!(status.code(), 4);
    assert!(failed.cleanup_events.is_empty());

    // The normalized failure publishes no owned-data settlement, and the
    // retained revision remains independently evaluable afterward.
    assert_bytes(
        &revision,
        "borrowed.failure",
        &[
            PublicApiArgument::BorrowSliceU8(PAYLOAD),
            PublicApiArgument::I64(1),
        ],
    );
}
