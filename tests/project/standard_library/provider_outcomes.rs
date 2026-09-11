//! GitHub issue #123 (SPX-AI-024): compose the existing bundled `std.io`
//! Reader/Writer cursors with the existing bundled `std.data.json.dec` and
//! `std.num.overflow` packages into the two small typed adapters the frozen
//! catalog-normalizer acceptance application
//! (`docs/CATALOG-NORMALIZER-ORACLE-V1.md`, issue #117/#124) needs and that
//! no existing package already exposes as a value:
//!
//! - a bounded-read outcome that turns "the provider handed back more bytes
//!   than the declared limit" into a deterministic value distinct from a
//!   legitimate end-of-stream read (CNORM-002/003/004/011's "limit error, not
//!   EOF or truncated success", and issue #123's second acceptance
//!   criterion). The technique is exactly the one `std.fs.read`/`net_recv`
//!   already support with no interpreter or backend change: request one byte
//!   more than the application's real limit, then compare the returned
//!   length against that limit.
//! - a provider-reply outcome that classifies a decoded JSON string token
//!   (the fixture table's `"kind"` value, CNORM-061) against the five frozen
//!   words with `std.data.json.dec.decoded_eq` — the same decoded-value
//!   comparison primitive `std.data.json.dec` already documents and tests for
//!   arbitrary content, applied here to a closed keyword set. An
//!   unrecognized reply is *not* silently treated as `missing`: it defaults
//!   to `malformed`, so a provider reply outside the closed contract is
//!   always typed and never mistaken for a benign lookup miss.
//!
//! Neither adapter adds a compiler primitive, a host operation, or a new
//! bundled package: both are ordinary checked SEMAPRAX composed entirely from
//! functions the standard library already ships and already gates
//! independently (`std.io`'s cursors, `std.data.json.dec`'s decoded-value
//! comparison, `std.num.overflow`'s checked addition). This file proves the
//! composition executes, not merely that each package passes alone.
//!
//! What this file intentionally does not attempt: the full catalog-normalizer
//! JSON object grammar (`std.data.json.doc`'s member walk already owns that,
//! independently gated) and the fixture-provider host boundary itself
//! (`std.fs`'s existing `FileProvider` already serves that role for a
//! read-only lookup table with no new host operation, per
//! `docs/FILESYSTEM-IO-V1.md`). Both remain issue #124's integration, built
//! from these primitives plus the two adapters here.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{format, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const CURSORS: &str = include_str!("../../../std/io/src/io.spx");
const DEC: &str = include_str!("../../../std/data-json-dec/src/dec.spx");
const OVERFLOW: &str = include_str!("../../../std/num-overflow/src/overflow.spx");

/// The two adapters under test, composed from the three libraries above.
/// `bounded_oversized` takes the Reader `std.fs.read`/`net_recv` already
/// hand back (see `docs/FILESYSTEM-IO-V1.md` and
/// `docs/BOUNDED-LANGUAGE-NETWORK-IO-V1.md`); `provider_outcome` takes a
/// decoded-string-token position the way `std.data.json.doc`'s member walk
/// already locates one (see `docs/BOUNDED-JSON-SCANNER-V1.md`).
const ADAPTERS: &str = r#"
@id("app.bounded.oversized")
fn bounded_oversized(probe: borrow Reader, limit: usize) -> bool
    requires match borrow probe { Reader { data, position } => position <= byte_len(bytes_as_slice(data)), }
{
    reader_remaining(probe) > limit
}

@id("app.provider.outcome")
fn provider_outcome(input: borrow Slice<u8>, start: usize) -> usize
{
    let found_word = [102u8, 111u8, 117u8, 110u8, 100u8];
    let missing_word = [109u8, 105u8, 115u8, 115u8, 105u8, 110u8, 103u8];
    let denied_word = [100u8, 101u8, 110u8, 105u8, 101u8, 100u8];
    let timeout_word = [116u8, 105u8, 109u8, 101u8, 111u8, 117u8, 116u8];
    let found = decoded_eq(input, start, array_as_slice(found_word));
    let missing = decoded_eq(input, start, array_as_slice(missing_word));
    let denied = decoded_eq(input, start, array_as_slice(denied_word));
    let timeout = decoded_eq(input, start, array_as_slice(timeout_word));
    if found { 1usize } else { if missing { 0usize } else { if denied { 2usize } else { if timeout { 3usize } else { 4usize } } } }
}
"#;

/// One checked module holds all three libraries plus the adapters: the
/// cursor shapes the bounded-read adapter composes, the decoder shapes the
/// provider-outcome adapter composes, and the overflow package proves the
/// same module can carry the checked-total half of the frozen application
/// (CNORM-015) alongside both I/O adapters with no name collision. The
/// imports each package's own source resolves through its `[dependencies]`
/// route become local declarations with the same names here, so the fixture
/// exercises the identical bodies the bundled packages ship.
fn source(main: &str) -> String {
    let cursors = CURSORS.replacen("module std.io;", "module app;", 1);
    let strip_header = |text: &str| -> String {
        text.lines()
            .filter(|line| !line.starts_with("module ") && !line.starts_with("use "))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let dec = strip_header(DEC);
    let overflow = strip_header(OVERFLOW);
    format!("{cursors}\n{dec}\n{overflow}\n{ADAPTERS}\n{main}\n")
}

fn canonical_checked(main: &str) -> String {
    let program =
        parse(&source(main), "provider-outcomes.spx").expect("provider-outcomes fixture parses");
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "provider-outcomes fixture source diagnostics: {diagnostics:?}"
    );
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "provider-outcomes.spx").expect("canonical fixture reparses");
    assert_eq!(format::canonical(&reparsed), canonical);
    hir::resolve(&reparsed).expect("checked provider-outcomes fixture resolves");
    canonical
}

fn source_file(source: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-provider-outcomes-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).expect("writes temporary checked source");
    path
}

fn interpretation(main: &str) -> interpreter::Interpretation {
    let canonical = canonical_checked(main);
    let path = source_file(&canonical);
    let result = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
        .expect("provider-outcomes interpreter entry is admitted");
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

/// CNORM-002/003/004/011's exact requirement: a body over the declared limit
/// is a limit error, never mistaken for a clean end-of-stream read or a
/// silently truncated success. `bounded_oversized` reads with `limit + 1`
/// requested and compares the returned length against `limit`, which is
/// exactly the technique `std.fs.read(path, length, limit + 1usize)` and
/// `net_recv(handle, limit + 1usize)` already support today with no new host
/// operation: a provider that has at most `limit` bytes can never fill the
/// extra byte, so the comparison is unambiguous in both directions.
#[test]
fn bounded_oversized_distinguishes_limit_from_clean_eof() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let short_bytes = [1u8, 2u8, 3u8];
    let short = reader_from_bytes(bytes_copy(array_as_slice(short_bytes)));
    let at_limit_is_clean = !bounded_oversized(short, 3usize);
    let over_bytes = [1u8, 2u8, 3u8, 4u8];
    let over = reader_from_bytes(bytes_copy(array_as_slice(over_bytes)));
    let over_limit_is_oversized = bounded_oversized(over, 3usize);
    let empty = reader_from_bytes(bytes_zeroed(0usize));
    let empty_is_clean = !bounded_oversized(empty, 0usize);
    if at_limit_is_clean && over_limit_is_oversized && empty_is_clean { 0 } else { 1 }
}
"#,
        "0",
    );
}

/// CNORM-060..062: every enrichment outcome the frozen fixture table can
/// name (`found`, `missing`, `denied`) decodes to its own typed value via the
/// same `decoded_eq` the JSON cursor package already ships and gates
/// independently. `provider_outcome`'s own worst-case path calls
/// `decoded_eq` up to four times (one per candidate word before the
/// `malformed` default), so each call from `main` below is itself already
/// four allocation sites against `std/byte_data_capacity`'s existing
/// module-wide `bytes_copy`-path budget (`SPX-T267`); this test and
/// `provider_outcome_defaults_unrecognized_and_timeout_replies_to_typed_categories`
/// below split the five frozen words across two mains so neither exceeds it,
/// exactly the same budget every other bundled JSON-cursor consumer already
/// composes under.
#[test]
fn provider_outcome_types_found_missing_and_denied() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let found_bytes = [34u8, 102u8, 111u8, 117u8, 110u8, 100u8, 34u8];
    let found_code = provider_outcome(array_as_slice(found_bytes), 0usize) == 1usize;
    let missing_bytes = [34u8, 109u8, 105u8, 115u8, 115u8, 105u8, 110u8, 103u8, 34u8];
    let missing_code = provider_outcome(array_as_slice(missing_bytes), 0usize) == 0usize;
    let denied_bytes = [34u8, 100u8, 101u8, 110u8, 105u8, 101u8, 100u8, 34u8];
    let denied_code = provider_outcome(array_as_slice(denied_bytes), 0usize) == 2usize;
    if found_code && missing_code && denied_code { 0 } else { 1 }
}
"#,
        "0",
    );
}

/// The remaining two frozen provider replies: `timeout` decodes to its own
/// value, and any reply outside the closed five-word set (a provider bug, a
/// corrupted fixture entry, or a future unadvertised reply) defaults to
/// `malformed` rather than being silently accepted as a benign `missing`
/// lookup.
#[test]
fn provider_outcome_defaults_unrecognized_and_timeout_replies_to_typed_categories() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let timeout_bytes = [34u8, 116u8, 105u8, 109u8, 101u8, 111u8, 117u8, 116u8, 34u8];
    let timeout_code = provider_outcome(array_as_slice(timeout_bytes), 0usize) == 3usize;
    let bogus_bytes = [34u8, 98u8, 111u8, 103u8, 117u8, 115u8, 34u8];
    let bogus_defaults_malformed = provider_outcome(array_as_slice(bogus_bytes), 0usize) == 4usize;
    if timeout_code && bogus_defaults_malformed { 0 } else { 1 }
}
"#,
        "0",
    );
}

/// CNORM-015's checked running total lives one function call away from both
/// adapters above with no name collision, so the same module can carry the
/// application's parse-time, provider-reply, and capacity-typed failure
/// categories together, as issue #123's outcome asks for: "the application
/// can compose existing read/parse/provider/write operations with typed
/// errors."
#[test]
fn checked_total_composes_alongside_both_adapters_with_no_collision() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let ordinary_sum_is_safe = !add_overflows(9223372036854775806, 1);
    let boundary_sum_overflows = add_overflows(9223372036854775807, 1);
    let probe_bytes = [1u8, 2u8];
    let probe = reader_from_bytes(bytes_copy(array_as_slice(probe_bytes)));
    let bounds_still_composes = !bounded_oversized(probe, 2usize);
    let found_bytes = [34u8, 102u8, 111u8, 117u8, 110u8, 100u8, 34u8];
    let provider_still_composes = provider_outcome(array_as_slice(found_bytes), 0usize) == 1usize;
    if ordinary_sum_is_safe && boundary_sum_overflows && bounds_still_composes && provider_still_composes { 0 } else { 1 }
}
"#,
        "0",
    );
}

/// Issue #123's required-tests list, first row: "Chunking does not change
/// successful values or zero-based error positions." Neither `std.io`'s
/// `Writer` nor `std.data.json.dec`'s scanner has a streaming mode; a
/// provider that hands back a bounded read in more than one call is
/// composed by writing each arrival into the same pre-sized `Writer` at its
/// current position, exactly as `reader_line_into`/`decode_into` already
/// require their output buffer to be sized (`docs/BOUNDED-JSON-SCANNER-V1.md`).
/// This proves two things about that composition with no new adapter: a
/// buffer read only up to the first chunk (the rest still zero-filled
/// placeholder capacity) decodes as a deterministic failure rather than a
/// corrupted or silently short "success" — the placeholder bytes are outside
/// the JSON string-body range `[32, 255]` — and once every chunk has
/// arrived, the decoded value is byte-identical to writing the same content
/// in one pass, regardless of where the chunk boundary fell.
#[test]
fn chunking_does_not_change_the_decoded_value_once_every_chunk_has_arrived() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let token_bytes = [34u8, 97u8, 98u8, 34u8];
    let one_shot = writer_write_u8(writer_write_u8(writer_write_u8(writer_write_u8(writer_from_bytes(bytes_zeroed(4usize)), 34u8), 97u8), 98u8), 34u8);
    let one_shot_bytes = writer_finish(one_shot);
    let one_shot_value = decoded_len(bytes_as_slice(one_shot_bytes), 0usize);
    let first_chunk = writer_write_u8(writer_write_u8(writer_from_bytes(bytes_zeroed(4usize)), 34u8), 97u8);
    let still_incomplete = match borrow first_chunk { Writer { data, position: _ } => is_failure(bytes_as_slice(data), decoded_len(bytes_as_slice(data), 0usize)), };
    let assembled = writer_write_u8(writer_write_u8(first_chunk, 98u8), 34u8);
    let assembled_bytes = writer_finish(assembled);
    let two_chunk_value = decoded_len(bytes_as_slice(assembled_bytes), 0usize);
    let split_elsewhere_first = writer_write_u8(writer_write_u8(writer_write_u8(writer_from_bytes(bytes_zeroed(4usize)), 34u8), 97u8), 98u8);
    let split_elsewhere = writer_write_u8(split_elsewhere_first, 34u8);
    let split_elsewhere_bytes = writer_finish(split_elsewhere);
    let split_elsewhere_value = decoded_len(bytes_as_slice(split_elsewhere_bytes), 0usize);
    let known_content = byte_len(array_as_slice(token_bytes)) == 4usize;
    if known_content && still_incomplete && two_chunk_value == one_shot_value && split_elsewhere_value == one_shot_value { 0 } else { 1 }
}
"#,
        "0",
    );
}

/// Issue #123's required-tests list, first row, error-position half: a
/// malformed token's reported position (`std.data.json.dec.failure`'s
/// `byte_len(input) + 1 + offset`, which is absolute and zero-based within
/// the assembled buffer, not relative to any one provider call) does not
/// move depending on how many separate writes assembled the buffer before
/// the fault, only on where the fault byte actually sits.
#[test]
fn chunking_does_not_change_the_zero_based_error_position() {
    returns(
        r#"
@id("app.main")
fn main() -> i64
{
    let one_shot = writer_write_u8(writer_write_u8(writer_write_u8(writer_write_u8(writer_write_u8(writer_write_u8(writer_from_bytes(bytes_zeroed(6usize)), 34u8), 97u8), 92u8), 122u8), 98u8), 34u8);
    let one_shot_bytes = writer_finish(one_shot);
    let one_shot_fault = decoded_len(bytes_as_slice(one_shot_bytes), 0usize);
    let one_shot_is_failure = is_failure(bytes_as_slice(one_shot_bytes), one_shot_fault);
    let split_before_fault = writer_write_u8(writer_write_u8(writer_from_bytes(bytes_zeroed(6usize)), 34u8), 97u8);
    let split_before_rest = writer_write_u8(writer_write_u8(writer_write_u8(writer_write_u8(split_before_fault, 92u8), 122u8), 98u8), 34u8);
    let split_before_bytes = writer_finish(split_before_rest);
    let split_before_fault_value = decoded_len(bytes_as_slice(split_before_bytes), 0usize);
    let split_after_fault = writer_write_u8(writer_write_u8(writer_write_u8(writer_write_u8(writer_from_bytes(bytes_zeroed(6usize)), 34u8), 97u8), 92u8), 122u8);
    let split_after_rest = writer_write_u8(writer_write_u8(split_after_fault, 98u8), 34u8);
    let split_after_bytes = writer_finish(split_after_rest);
    let split_after_fault_value = decoded_len(bytes_as_slice(split_after_bytes), 0usize);
    if one_shot_is_failure && split_before_fault_value == one_shot_fault && split_after_fault_value == one_shot_fault { 0 } else { 1 }
}
"#,
        "0",
    );
}
