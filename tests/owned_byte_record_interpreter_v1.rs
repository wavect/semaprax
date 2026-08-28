use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.owned_byte_record_interpreter;

@id("owned.packet")
record Packet {
    @id("owned.packet.payload") payload: Bytes,
    @id("owned.packet.marker") marker: i64,
}

@id("owned.consume")
fn consume() -> i64 {
    let source = [1u8, 2u8, 3u8, 4u8];
    let packet = Packet {
        payload: bytes_copy(array_as_slice(source)),
        marker: 38,
    };
    match own packet {
        Packet { payload, marker } => if byte_len(bytes_as_slice(payload)) == 4usize {
            marker + 4
        } else {
            0
        },
    }
}

@id("owned.inspect")
fn inspect(packet: borrow Packet) -> i64 {
    match borrow packet {
        Packet { payload, marker: _ } => if byte_len(bytes_as_slice(payload)) == 3usize {
            3
        } else {
            0
        },
    }
}

@id("owned.take")
fn take(packet: own Packet) -> i64 {
    match own packet {
        Packet { payload, marker } => if byte_len(bytes_as_slice(payload)) == 3usize {
            marker + 3
        } else {
            0
        },
    }
}

@id("owned.borrow-then-consume")
fn borrow_then_consume() -> i64 {
    let source = [7u8, 8u8, 9u8];
    let packet = Packet {
        payload: bytes_copy(array_as_slice(source)),
        marker: 36,
    };
    let borrowed = inspect(packet);
    borrowed + take(packet)
}

@id("app.main")
fn main() -> i64 { consume() }
"#;

fn source_file() -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-owned-record-interpreter-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    path
}

fn returned_value(envelope: &str) -> String {
    let document: serde_json::Value = serde_json::from_str(envelope).unwrap();
    document["payload"]["outcome"]["value"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn interpret(function: &str) -> interpreter::Interpretation {
    let path = source_file();
    let result =
        interpreter::interpret(&path, function, &[], &InterpreterOptions::default()).unwrap();
    std::fs::remove_file(path).unwrap();
    result
}

#[test]
fn owned_record_construction_and_destructuring_execute_exact_field_ids() {
    let parsed = parse(SOURCE, "owned-byte-record-interpreter-v1.spx").unwrap();
    assert!(verify::verify(&parsed).is_empty());
    hir::validate(&hir::resolve(&parsed).unwrap()).unwrap();

    let result = interpret("owned.consume");
    assert!(result.returned);
    assert_eq!(returned_value(&result.envelope), "42");
    interpreter::verify_envelope(&result.envelope).unwrap();
}

#[test]
fn borrowed_alias_settles_before_the_same_owner_reenters_owned_match() {
    let result = interpret("owned.borrow-then-consume");
    assert!(result.returned);
    assert_eq!(returned_value(&result.envelope), "42");
    interpreter::verify_envelope(&result.envelope).unwrap();
}

#[test]
fn owned_record_reentry_without_a_settled_borrow_is_rejected_before_interpretation() {
    let hostile = r#"
module test.owned_record_reentry;
record Packet { payload: Bytes, marker: i64, }
fn invalid() -> i64 {
    let source = [1u8];
    let packet = Packet { payload: bytes_copy(array_as_slice(source)), marker: 0 };
    let first = match own packet { Packet { payload, marker: _ } => 0, };
    match own packet { Packet { payload, marker: _ } => first, }
}
fn main() -> i64 { 0 }
"#;
    let parsed = parse(hostile, "owned-byte-record-reentry.spx").unwrap();
    let diagnostics = verify::verify(&parsed);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "SPX-O101" && diagnostic.message.contains("packet")
    }));
}
