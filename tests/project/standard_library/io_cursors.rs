//! Focused source and interpreter regressions for the private `std.io` cursor
//! package.  The package is intentionally exercised as ordinary checked source:
//! no host I/O authority is involved in these cursor transitions.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const LIBRARY: &str = include_str!("../../../std/io/src/io.spx");

fn source(main: &str) -> String {
    format!(
        "{}\n{main}\n",
        LIBRARY.replacen("module std.io;", "module app;", 1)
    )
}

fn canonical_checked(main: &str) -> String {
    let program = parse(&source(main), "std-io-cursors.spx").expect("std.io fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "std.io fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "std-io-cursors.spx").expect("canonical std.io reparses");
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::resolve(&reparsed).expect("checked std.io fixture resolves");
    canonical
}

fn source_file(source: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-std-io-cursors-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).expect("writes temporary checked source");
    path
}

fn interpretation(main: &str) -> interpreter::Interpretation {
    let canonical = canonical_checked(main);
    let path = source_file(&canonical);
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("std.io interpreter entry is admitted");
    interpreter::verify_envelope(&result.envelope).expect("interpreter envelope is canonical");
    std::fs::remove_file(path).expect("removes temporary checked source");
    result
}

fn returns(main: &str, expected: &str) {
    let result = interpretation(main);
    assert!(
        result.returned,
        "expected returned {expected}: {:?}",
        result.envelope
    );
    let document: serde_json::Value =
        serde_json::from_str(&result.envelope).expect("envelope JSON");
    assert_eq!(
        document["payload"]["outcome"]["value"].as_str(),
        Some(expected)
    );
}

fn fails(main: &str) {
    let result = interpretation(main);
    assert!(!result.returned, "invalid transition returned");
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    let outcome = &document["payload"]["outcome"];
    assert_eq!(outcome["kind"], "failed", "{document}");
    assert_eq!(outcome["status"]["class"], "contract", "{document}");
}

#[test]
fn io_cursors_round_trip_boundaries_and_zero_length_observer_execute() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let writer0 = writer_from_bytes(bytes_zeroed(2usize));
    let writer1 = writer_write_u8(writer0, 0u8);
    let writer2 = writer_write_u8(writer1, 255u8);
    let reader0 = reader_from_bytes(writer_finish(writer2));
    let first = reader_peek(reader0) == 0u8;
    let reader1 = reader_advance(reader0, 1usize);
    let second = reader_peek(reader1) == 255u8;
    let reader2 = reader_advance(reader1, 8usize);
    let exhausted = reader_remaining(reader2) == 0usize;
    let bytes = reader_finish(reader2);
    let empty = reader_from_bytes(bytes_zeroed(0usize));
    let empty_remaining = reader_remaining(empty) == 0usize;
    if first && second && exhausted && empty_remaining && byte_len(bytes_as_slice(bytes)) == 2usize { 0 } else { 1 }
}
"#,
        "0",
    );
}

#[test]
fn io_cursor_contracts_fail_before_invalid_reader_or_writer_transition() {
    for main in [
        r#"
@id("app.main")
fn main() -> i64
{
    let reader = Reader { data: bytes_zeroed(1usize), position: 2usize };
    if reader_position(reader) == 0usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let writer0 = writer_from_bytes(bytes_zeroed(1usize));
    let writer1 = writer_write_u8(writer0, 0u8);
    let writer2 = writer_write_u8(writer1, 255u8);
    let data = writer_finish(writer2);
    if byte_len(bytes_as_slice(data)) == 1usize { 0 } else { 1 }
}
"#,
        r#"
@id("app.main")
fn main() -> i64
{
    let reader = reader_from_bytes(bytes_zeroed(0usize));
    if reader_peek(reader) == 0u8 { 0 } else { 1 }
}
"#,
    ] {
        fails(main);
    }
}

#[test]
fn borrowed_reader_cannot_be_consumed_to_escape_its_bytes() {
    let source = source(
        r#"
@id("app.leak")
fn leak(reader: borrow Reader) -> Bytes
{
    match own reader { Reader { data, position: _ } => data, }
}

@id("app.main")
fn main() -> i64
{
    let reader = reader_from_bytes(bytes_zeroed(1usize));
    let bytes = leak(reader);
    if byte_len(bytes_as_slice(bytes)) == 1usize { 0 } else { 1 }
}
"#,
    );
    let program = parse(&source, "std-io-borrow-escape.spx").expect("hostile source parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-O117"),
        "borrowed Reader consumption must retain its exact ownership rejection: {diagnostics:?}"
    );
}

#[test]
fn borrowed_match_does_not_admit_unrelated_owner_result() {
    let text = source(
        r#"
@id("app.bad")
fn bad(reader: borrow Reader) -> Bytes {
    let spare = bytes_zeroed(1usize);
    match borrow reader { Reader {data, position} => spare, }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let program = parse(&text, "borrowed-owned-result.spx").unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().any(|d| d.code == "SPX-T216"),
        "{diagnostics:?}"
    );
}

#[test]
fn io_cursors_library_has_no_public_descriptor_or_export_authority() {
    let manifest = super::root().join("std/io/semaprax.toml");
    semaprax::project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        assert!(snapshot.manifest().web_exports().is_empty());
        let diagnostics = snapshot
            .public_api_descriptor()
            .expect_err("library acquired a public descriptor");
        assert!(
            diagnostics.iter().any(|d| d.code == "SPX-J105"),
            "{diagnostics:?}"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn io_cursors_bundled_dependency_executes_without_vendored_library_or_exports() {
    let scratch = super::temporary("io-bundled-dependency");
    std::fs::create_dir(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        r#"schema = "semaprax.manifest.v1"

[package]
name = "cursor-consumer"
version = "0.1.0"
profile = "owned-data-api.v1"

[modules]
entry = "consumer.app"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["consumer.tests"]

[exports]
web = []

[dependencies]
std.io = "^0.1.0"
"#,
    )
    .unwrap();
    for (original, destination, from, to) in [
        ("examples.spx", "app.spx", "std.io.examples", "consumer.app"),
        ("tests.spx", "tests.spx", "std.io.tests", "consumer.tests"),
    ] {
        let text =
            std::fs::read_to_string(super::root().join("std/io/src").join(original)).unwrap();
        std::fs::write(
            scratch.join("src").join(destination),
            text.replace(from, to),
        )
        .unwrap();
    }
    semaprax::project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = semaprax::project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &semaprax::project::ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &semaprax::project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot.public_api_descriptor().is_err());
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(scratch).unwrap();
}
