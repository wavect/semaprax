//! Issue #103: a bounded feature-combination and differential backend
//! conformance corpus for VIEW COMPOSITION, distinct from the scalar-status
//! subject the parent harness and `differential` (this file's sibling
//! module) already cover.
//!
//! ## What is new here, and why it is not redundant with the parent harness
//!
//! `scalar_status_backend_equivalence.rs` differentially checks arithmetic
//! overflow/contract status codes; `differential/grammar.rs` differentially
//! fuzzes control flow (`if`, bounded `while`, `requires`/`ensures`) over
//! plain scalars, but its own header doc says it "never emits ... a
//! record, a string" -- no owned `Bytes`, no borrowed `Slice<u8>` views.
//! `tests/public_generic_native_adapter_v1/settlement_corpus.rs`
//! differentially checks ownership-transfer SETTLEMENT (trace order, sticky
//! status, live-resource counts) for the Public Generic Carrier ABI
//! specifically. None of the three composes VIEW ORIGIN (a whole owned
//! buffer vs. a `byte_range` sub-view of it) with CONTROL-FLOW CONTEXT
//! (`if`, `while`, `match`) and compares the resulting VALUE -- not just
//! "did it run" -- across interpreter, native C11 (`-O0`/`-O2`), and Core
//! Wasm. That composition is exactly issue #100's own bug shape (a view
//! operation combined with a loop context disagreed between backends),
//! issue #103 names it explicitly as a required dimension set ("view
//! origin, local/call/return use, conditional/loop/match context, ownership
//! transfer, ... backend"), and `run_examples_and_conformance`
//! (`tests/project/standard_library.rs`, issue #102's own deliverable)
//! only asserts each backend independently "returned 0" -- never that the
//! three backends computed the SAME value, which is the exact defect class
//! ("three engines each individually succeed" proves nothing about
//! equivalence) this issue was opened to close.
//!
//! ## The combination bound, and why it holds
//!
//! **Main corpus** (`view_ownership_composition_agrees_across_interpreter_native_and_core_wasm`):
//! view origin (2: whole buffer / `byte_range` sub-view) x control-flow
//! context (3: `if`, `while`, direct `match`) = 6 cells, enumerated in
//! FULL -- not pairwise-reduced, because with only 2 and 3 values a full
//! enumeration is already smaller than a pairwise covering array would be
//! for a third dimension, and every cell is independently meaningful (a
//! reader can point at any one of the 6 and know exactly which origin and
//! context it proves equivalent). A 7th case
//! (`voc.case_growing_range_in_while`) is not part of the 2x3 matrix; it is
//! issue #100's own high-risk shape, generalized and re-run because of what
//! reading the current checker turned up (see "A finding" below). All 7
//! cases execute on interpreter, native `-O0`, native `-O2`, and Core-Wasm.
//!
//! **Why Core-Wasm dispatches by one aggregate `app.main`, not by stable id
//! per case.** `wasm::build_web_with_scalar_exports`'s whole-program Public
//! Scalar Export Profile v1 (`src/wasm/scalar_exports.rs`) turned out to
//! reject `Match`, `ByteRange`, and any non-`Value`-ownership local binding
//! ANYWHERE in the program -- confirmed empirically the same way as the
//! ownership finding below, not assumed: an earlier draft hit
//! `SPX-W115 ... binds a non-value scalar` on the very first case's own
//! `let view = bytes_as_slice(data)` local, which is a `Borrow`-ownership
//! binding. That profile is a deliberately narrow arithmetic-only ABI
//! (matching this harness's own scalar-status subject), not a general Wasm
//! byte-view surface, and per-id multi-function dispatch on Core-Wasm does
//! not exist for byte/view/`Option`-composed code anywhere in this
//! repository -- `run_examples_and_conformance`
//! (`tests/project/standard_library.rs`) only ever builds ONE fixed
//! `semaprax_main` entry per Wasm module for exactly this class of code.
//! So this corpus uses the same plain, unrestricted builder
//! (`wasm::build_web`, full admitted language, one `semaprax_main` entry)
//! and has `app.main` combine all 7 cases into one aggregate
//! (`case_i() * 100^i`, decoded back into 7 values in Rust afterward,
//! since every case's value is under 100 so no positional digit can carry
//! into its neighbor). This still yields a genuine per-case differential
//! comparison, not a blurred sum: [`decode_case_values`] recovers each
//! case's own digit group, and a wrong value in any one case changes only
//! that position, so a discrepancy is still reported against the specific
//! case id and expected value that produced it (see `assert_case_agrees`'s
//! use in the main test).
//!
//! **Ownership corpus** (`ownership_route_agrees_across_interpreter_and_native`):
//! borrow-only vs. a real `own Bytes` transfer to a separate consuming
//! function, compared on interpreter and native `-O0`/`-O2` only -- NOT
//! Core-Wasm. This is a verified bound, not an oversight: `WASM_SOURCE`'s
//! `wasm::build_web_with_scalar_exports` calls
//! `scalar_exports::validate_program_profile`
//! (`src/wasm/scalar_exports.rs`), which rejects EVERY function in the
//! whole program -- not only the selected exports -- that has a
//! non-`Value`-ownership or non-scalar parameter. A function taking
//! `own Bytes` (the only way to demonstrate a real cross-function ownership
//! transfer, as opposed to a compiler-owned chain like `bytes_set`) fails
//! that check unconditionally, confirmed empirically: an earlier draft of
//! this corpus that included such a function in the same source Core-Wasm
//! also built from failed with
//! `SPX-W115 ... has a non-scalar result`/`... non-scalar parameter` on the
//! HELPER function, not on anything selected for export. So "ownership
//! transfer composed with Core-Wasm" is bounded OUT of this corpus for a
//! real, load-bearing reason: the Public Scalar Export Profile v1 backend
//! path categorically cannot express it, not because this corpus declined
//! to try.
//!
//! Both corpora together stop at these dimensions rather than adding
//! "imported/standalone route" or "profile" as a 3rd/4th: import
//! resolution across `useful-text-consumer.v1`/`useful-data.v1` (issue
//! #101's own shape) is a project/package-manifest concern with its own
//! established harness (`tests/project/standard_library.rs`,
//! `execution_matrix.rs`); composing it here would require standing up a
//! temporary package per cell, which is exactly the unbounded explosion
//! issue #103 warns against rather than a deliberate, justified bound.
//!
//! ## A finding: issue #100's documented restriction is stale for at least
//! one shape
//!
//! `std/data-toml/src/toml.spx`'s own comment on `bare_key_end` states that
//! `byte_range` "is not one of the two shapes a while loop may call ...
//! so it is rejected with SPX-H006" and restores the issue-#218 growing-
//! subslice idiom OUTSIDE the loop instead, citing
//! `hir::validation::owned_buffer::require_admitted_while_operation`.
//! Reading that validator plus its caller
//! (`src/hir/validation/iterator_loops.rs`) shows `byte_range` calls do NOT
//! reach `require_admitted_while_operation` at all: `byte_range` resolves
//! to its own dedicated `ResolvedExprKind::ByteRange` node, not a generic
//! `Call`, and `validate_iterator_body`'s `ByteRange` arm (line ~115)
//! ADMITS it inside a `while` body whenever its `source` is a `Place` with
//! authenticated byte-slice provenance (`byte_slice_aliases` or
//! `Declarations::byte_slice_provenance`) -- confirmed empirically: an
//! earlier draft of this file wrote issue #100's reproduction shape as a
//! rejection test expecting `hir::resolve` to fail, and instead
//! `hir::resolve` SUCCEEDED. Either the validator was widened after that
//! comment was written (most likely, given the provenance-tracking
//! machinery it clearly did not have when the comment cites only
//! `byte_len`/`byte_get`/`bytes_set`), or the comment describes a narrower
//! shape than this corpus's. Either way, this corpus does NOT silently
//! accept the discrepancy: `voc.case_growing_range_in_while` runs the
//! ADMITTED shape (a `byte_range` sub-view recomputed fresh every `while`
//! iteration from a locally bound view, issue #100's own reproduction
//! generalized) through all four engines and compares the VALUE exactly,
//! which is the one thing that actually matters for the non-negotiable
//! invariant this issue exists to enforce. This is reported as a
//! documentation-staleness finding for whoever owns `toml.spx`'s comment
//! and `docs/PUBLIC-GENERIC-CARRIER-V1.md`-adjacent specs to reconcile, not
//! silently corrected here -- correcting stale prose in a file this
//! worker's lease does not grant is out of scope.
//!
//! ## Negative control (issue #103's own required test)
//!
//! [`assert_case_agrees`] is the one function every comparison funnels
//! through; `assert_case_agrees_rejects_a_perturbed_engine_value` feeds it a
//! deliberately wrong observed value (not a compiler-source mutation) and
//! proves the comparison is real and failable, following the same pattern
//! `tests/public_generic_native_adapter_v1/settlement_corpus.rs`'s own
//! negative controls already established.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, parse, verify, wasm};

/// The 2x3 view-origin x context matrix, plus the additional issue-#100
/// finding case, all self-contained (no function anywhere in this source
/// crosses a function boundary with a `Bytes`/`Slice<u8>` parameter or
/// result -- `code_for` is `u8 -> i64`, purely scalar) so the SAME source
/// builds through `wasm::build_web_with_scalar_exports`'s whole-program
/// Public Scalar Export Profile v1 check. Each case rebuilds its own
/// 4-byte buffer `[10, 20, 30, 40]` (`code_for` maps those to
/// `[1, 2, 3, 4]`) rather than sharing a helper, precisely because a
/// shared `() -> Bytes` helper is itself a non-scalar-result function that
/// would fail that same whole-program check (see this module's header doc
/// for how that was confirmed, not assumed).
const SOURCE: &str = r#"
module test.view_ownership_composition;

@id("voc.code_for")
fn code_for(value: u8) -> i64
{
    if value == 10u8 { 1 } else { if value == 20u8 { 2 } else { if value == 30u8 { 3 } else { if value == 40u8 { 4 } else { 0 } } } }
}

@id("voc.case_whole_if")
fn case_whole_if() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    let view = bytes_as_slice(data);
    let a = match byte_get(view, 0usize) { Option::Some { value } => code_for(value), Option::None {} => 0, };
    let b = match byte_get(view, 2usize) { Option::Some { value } => code_for(value), Option::None {} => 0, };
    if a > 0 && b > 0 { a + b } else { -1 }
}

@id("voc.case_whole_loop")
fn case_whole_loop() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    let view = bytes_as_slice(data);
    let length = byte_len(view);
    let mut index = 0usize;
    let mut total = 0;
    while index < length {
        total = total + match byte_get(view, index) { Option::Some { value } => code_for(value), Option::None {} => 0, };
        index = index + 1usize;
        0
    }
    total
}

@id("voc.case_whole_match")
fn case_whole_match() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    let view = bytes_as_slice(data);
    match byte_get(view, 3usize) { Option::Some { value } => code_for(value), Option::None {} => -1, }
}

@id("voc.case_sub_if")
fn case_sub_if() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    let view = bytes_as_slice(data);
    let sub = byte_range(view, 1usize, 3usize);
    let a = match byte_get(sub, 0usize) { Option::Some { value } => code_for(value), Option::None {} => 0, };
    let b = match byte_get(sub, 1usize) { Option::Some { value } => code_for(value), Option::None {} => 0, };
    if a > 0 && b > 0 { a + b } else { -1 }
}

@id("voc.case_sub_loop")
fn case_sub_loop() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    let view = bytes_as_slice(data);
    let sub = byte_range(view, 1usize, 3usize);
    let length = byte_len(sub);
    let mut index = 0usize;
    let mut total = 0;
    while index < length {
        total = total + match byte_get(sub, index) { Option::Some { value } => code_for(value), Option::None {} => 0, };
        index = index + 1usize;
        0
    }
    total
}

@id("voc.case_sub_match")
fn case_sub_match() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    let view = bytes_as_slice(data);
    let sub = byte_range(view, 1usize, 3usize);
    match byte_get(sub, 1usize) { Option::Some { value } => code_for(value), Option::None {} => -1, }
}

@id("voc.case_growing_range_in_while")
fn case_growing_range_in_while() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    let view = bytes_as_slice(data);
    let length = byte_len(view);
    let mut index = 0usize;
    let mut total = 0;
    while index < length {
        let sub = byte_range(view, 0usize, index + 1usize);
        total = total + match byte_get(sub, index) { Option::Some { value } => code_for(value), Option::None {} => 0, };
        index = index + 1usize;
        0
    }
    total
}

@id("app.main")
fn main() -> i64
{
    case_whole_if() + case_whole_loop() * 100 + case_whole_match() * 10000 + case_sub_if() * 1000000 + case_sub_loop() * 100000000 + case_sub_match() * 10000000000 + case_growing_range_in_while() * 1000000000000
}
"#;

/// One (case, independently pinned expected value) row per cell of the
/// 2x3 matrix, plus `voc.case_growing_range_in_while`, in the exact order
/// this module's header doc describes. Buffer bytes are `[10, 20, 30,
/// 40]`; `code_for` maps them to `[1, 2, 3, 4]`. Whole-view cells read
/// indices `0`/`2`/`3` (codes `1`/`3`/`4`) or sum all four
/// (`1+2+3+4=10`); sub-view cells operate on `byte_range(view, 1, 3)` =
/// `[20, 30]` (codes `2`/`3`); the growing-range case reads the growing
/// sub-view's own last index each iteration, which is the same as reading
/// the whole buffer in order (`1+2+3+4=10`).
const CASES: [(&str, i64); 7] = [
    ("voc.case_whole_if", 4),
    ("voc.case_whole_loop", 10),
    ("voc.case_whole_match", 4),
    ("voc.case_sub_if", 5),
    ("voc.case_sub_loop", 5),
    ("voc.case_sub_match", 3),
    ("voc.case_growing_range_in_while", 10),
];

/// The ownership-route pair: `voc.case_borrow_only` never lets the owned
/// buffer leave the function that allocated it; `voc.case_transfer`
/// allocates it and immediately moves it into `consume_transfer(data: own
/// Bytes)` -- a real cross-function ownership transfer, at its declared
/// commit boundary, per this repository's own invariant ("An owned call
/// stages arguments left to right and transfers them together at its
/// declared commit boundary"). Both must compute the identical value:
/// which function currently owns the buffer must never be observable.
const OWNERSHIP_SOURCE: &str = r#"
module test.view_ownership_composition_transfer;

@id("voc.code_for_transfer")
fn code_for_transfer(value: u8) -> i64
{
    if value == 10u8 { 1 } else { if value == 20u8 { 2 } else { if value == 30u8 { 3 } else { if value == 40u8 { 4 } else { 0 } } } }
}

@id("voc.consume_transfer")
fn consume_transfer(data: own Bytes) -> i64
{
    let view = bytes_as_slice(data);
    let length = byte_len(view);
    let mut index = 0usize;
    let mut total = 0;
    while index < length {
        total = total + match byte_get(view, index) { Option::Some { value } => code_for_transfer(value), Option::None {} => 0, };
        index = index + 1usize;
        0
    }
    total
}

@id("voc.case_transfer")
fn case_transfer() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    consume_transfer(data)
}

@id("voc.case_borrow_only")
fn case_borrow_only() -> i64
{
    let data = bytes_set(bytes_set(bytes_set(bytes_set(bytes_zeroed(4usize), 0usize, 10u8), 1usize, 20u8), 2usize, 30u8), 3usize, 40u8);
    let view = bytes_as_slice(data);
    let length = byte_len(view);
    let mut index = 0usize;
    let mut total = 0;
    while index < length {
        total = total + match byte_get(view, index) { Option::Some { value } => code_for_transfer(value), Option::None {} => 0, };
        index = index + 1usize;
        0
    }
    total
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

const OWNERSHIP_CASES: [(&str, i64); 2] =
    [("voc.case_borrow_only", 10), ("voc.case_transfer", 10)];

const REQUIRE_ENV: &str = "SEMAPRAX_REQUIRE_VIEW_OWNERSHIP_COMPOSITION";

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

fn tool_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn required() -> bool {
    std::env::var_os(REQUIRE_ENV).is_some()
}

fn require_clang_or_skip() -> bool {
    if tool_available("clang") {
        return true;
    }
    assert!(!required(), "{REQUIRE_ENV} requires clang; it is missing");
    false
}

fn require_tools_or_skip() -> bool {
    let missing = ["clang", "node"]
        .into_iter()
        .filter(|tool| !tool_available(tool))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return true;
    }
    assert!(
        !required(),
        "{REQUIRE_ENV} requires clang and Node; missing {}",
        missing.join(", ")
    );
    false
}

fn temporary_root(label: &str) -> PathBuf {
    let ordinal = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-view-ownership-composition-{label}-{}-{ordinal}",
        std::process::id()
    ))
}

/// The Legacy native profile's per-declaration C symbol: `spx_decl_` plus
/// the hex of every stable-id byte. Identical scheme to the sibling
/// `scalar_status_backend_equivalence.rs`'s own `c_symbol`, duplicated
/// rather than shared across modules, matching this harness's existing
/// convention (`differential.rs` defines its own `temporary_root` rather
/// than reaching into the parent's).
fn c_symbol(declaration_id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in declaration_id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

fn normalized_stdout(output: Output, label: &str) -> String {
    assert!(
        output.status.success(),
        "{label} failed with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn native_probe(cases: &[(&str, i64)]) -> String {
    let mut declarations = String::new();
    let mut calls = String::new();
    for (id, _) in cases {
        declarations.push_str(&format!(
            "static spx_status_token {}(struct spx_context *, int64_t *);\n",
            c_symbol(id)
        ));
        calls.push_str(&format!(
            "    result = spx_emit_i64({:?}, {});\n    if (result != 0) return result;\n",
            id,
            c_symbol(id)
        ));
    }
    format!(
        r#"{declarations}typedef spx_status_token (*spx_i64_case)(struct spx_context *, int64_t *);

static int spx_emit_i64(const char *id, spx_i64_case test_case) {{
    struct spx_status_entry records[UINT32_C(2)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(601), records, UINT32_C(2), NULL, NULL, NULL)) return 10;
    int64_t value = -INT64_C(1);
    spx_status_token token = test_case(&context, &value);
    if (token != SPX_STATUS_SUCCESS || context.status_arena.length != UINT32_C(0)) return 11;
    printf("{{\"id\":\"%s\",\"value\":%lld}}\n", id, (long long)value);
    return 0;
}}

int main(void) {{
    int result = 0;
{calls}    return 0;
}}
"#
    )
}

fn run_native(
    generated: &str,
    root: &Path,
    optimization: &str,
    cases: &[(&str, i64)],
) -> Vec<(String, i64)> {
    let source = root.join(format!("native-{optimization}.c"));
    let executable = root.join(format!(
        "native-{optimization}{}",
        std::env::consts::EXE_SUFFIX
    ));
    fs::write(&source, format!("{generated}\n{}", native_probe(cases))).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "native {optimization} compilation failed: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let stdout = normalized_stdout(
        Command::new(&executable).output().unwrap(),
        &format!("native {optimization}"),
    );
    parse_json_lines(&stdout)
}

fn parse_json_lines(stdout: &str) -> Vec<(String, i64)> {
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let document: serde_json::Value = serde_json::from_str(line)
                .unwrap_or_else(|error| panic!("malformed JSON line {line:?}: {error}"));
            let id = document["id"].as_str().unwrap().to_owned();
            let value = document["value"].as_i64().unwrap();
            (id, value)
        })
        .collect()
}

/// `100^index`: the positional weight `app.main` (in [`SOURCE`]) multiplies
/// each case's own result by. Every case's value is under 100, so no
/// position can carry into its neighbor and the encoding is exactly
/// invertible by [`decode_case_values`].
fn case_weight(index: usize) -> i64 {
    100i64.pow(index as u32)
}

fn expected_aggregate(cases: &[(&str, i64)]) -> i64 {
    cases
        .iter()
        .enumerate()
        .map(|(index, (_, value))| value * case_weight(index))
        .sum()
}

fn decode_case_values(aggregate: i64, cases: &[(&str, i64)]) -> Vec<i64> {
    (0..cases.len())
        .map(|index| (aggregate / case_weight(index)) % 100)
        .collect()
}

/// Builds the SAME parsed `program` through the plain, unrestricted
/// `wasm::build_web` (full admitted language: `Bytes`, `Slice<u8>`,
/// `Option`, `match`, `while` -- see this module's header doc for why NOT
/// `build_web_with_scalar_exports`), runs its one `semaprax_main` entry
/// (`app.main`) once via a hand-instantiated Node script, and returns the
/// raw aggregate result. The host-import shim below is the owned-`Bytes`
/// decode/read/allocate protocol `tests/project/standard_library.rs`'s own
/// `wasm_conformance_js` already established for exactly this class of
/// module (range-descriptor carriers included), trimmed of the `Box`
/// operations this corpus never uses.
fn run_core_wasm_aggregate(program: &semaprax::ast::Program, root: &Path) -> i64 {
    let package = root.join("web");
    wasm::build_web(program, &package).unwrap();
    let script = root.join("observe-core-wasm.mjs");
    fs::write(
        &script,
        r#"import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const packageDirectory = resolve(process.argv[2]);
const bytes = await readFile(resolve(packageDirectory, "app.wasm"));
const entries = new Map();
let next = 1;
let linked;
const decode = (carrier) => {
  const word = BigInt.asUintN(64, carrier);
  const length = Number(word & 0xffffffffn);
  const root = Number((word >> 32n) & 0xffffffffn);
  return { word, length, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff };
};
const read = (decoded) => {
  if ((decoded.root & 0xc0000000) === 0x40000000) {
    const pointer = (decoded.root & 0xffff) * 8;
    const key = (decoded.root >>> 16) & 0x1fff;
    const view = new DataView((linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer);
    if (
      pointer + 32 > view.byteLength ||
      view.getUint32(pointer, true) !== key ||
      view.getUint32(pointer + 4, true) !== pointer ||
      Number(view.getBigUint64(pointer + 24, true)) !== decoded.length
    ) {
      throw new Error("corrupt range descriptor");
    }
    const rootCarrier = view.getBigInt64(pointer + 8, true);
    const offset = Number(view.getBigUint64(pointer + 16, true));
    const all = read(decode(rootCarrier));
    if (offset > all.length || decoded.length > all.length - offset) {
      throw new Error("byte range");
    }
    return all.slice(offset, offset + decoded.length);
  }
  if (decoded.tagged) {
    const value = entries.get(decoded.token);
    if (!(value instanceof Uint8Array) || value.length !== decoded.length) {
      throw new Error("stale byte token");
    }
    return value;
  }
  const memory = new Uint8Array((linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer);
  if (decoded.root > memory.length - decoded.length) {
    throw new Error("byte range");
  }
  return memory.slice(decoded.root, decoded.root + decoded.length);
};
const allocate = (raw) => {
  if (entries.size >= 64) {
    throw new Error("owned Bytes live entry limit exceeded");
  }
  const token = next++;
  const owned = new Uint8Array(raw);
  entries.set(token, owned);
  return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(owned.length));
};
const checked = (operation) => (a, b) => {
  const value = operation(a, b);
  if (value < -(1n << 63n) || value > (1n << 63n) - 1n) {
    throw new RangeError();
  }
  return value;
};
const imports = {
  env: {
    spx_add: checked((a, b) => a + b),
    spx_sub: checked((a, b) => a - b),
    spx_mul: checked((a, b) => a * b),
    spx_div: (a, b) => a / b,
    spx_rem: (a, b) => a % b,
    spx_neg: (a) => -a,
    spx_contract_fail: () => {
      throw new Error("contract failure");
    },
    spx_bytes_copy: (c) => allocate(read(decode(c))),
    spx_bytes_get: (c, i) => {
      const b = read(decode(c));
      const u = BigInt.asUintN(64, i);
      return u >= BigInt(b.length) ? -1 : b[Number(u)];
    },
    spx_bytes_drop: (c) => {
      const d = decode(c);
      read(d);
      entries.delete(d.token);
    },
    spx_bytes_as_slice: (c) => {
      const d = decode(c);
      read(d);
      return BigInt.asIntN(64, d.word);
    },
    spx_bytes_zeroed: (count) => {
      if (typeof count !== "bigint" || count < 0n || count > 65536n) {
        throw new Error("owned byte buffer capacity invariant");
      }
      return allocate(new Uint8Array(Number(count)));
    },
    spx_bytes_set: (c, i, v) => {
      const d = decode(c);
      const b = read(d);
      if (typeof i !== "bigint" || i < 0n || i >= BigInt(b.length) || !Number.isInteger(v) || v < 0 || v > 255) {
        throw new Error("owned byte buffer element invariant");
      }
      b[Number(i)] = v;
      return BigInt.asIntN(64, d.word);
    },
  },
};
linked = await WebAssembly.instantiate(bytes, imports);
const result = linked.instance.exports.semaprax_main();
process.stdout.write(`${result.toString()}\n`);
"#,
    )
    .unwrap();
    let stdout = normalized_stdout(
        Command::new("node")
            .arg(&script)
            .arg(&package)
            .output()
            .unwrap(),
        "Core-Wasm Node observer",
    );
    stdout
        .trim()
        .parse::<i64>()
        .unwrap_or_else(|error| panic!("malformed Core-Wasm aggregate {stdout:?}: {error}"))
}

fn run_interpreter(path: &Path, cases: &[(&str, i64)]) -> Vec<(String, i64)> {
    cases
        .iter()
        .map(|(id, _)| {
            let result = interpreter::interpret(path, id, &[], &InterpreterOptions::default())
                .unwrap_or_else(|diagnostics| {
                    panic!("interpreter rejected admitted case {id:?}: {diagnostics:?}")
                });
            interpreter::verify_envelope(&result.envelope)
                .expect("interpreter envelope is canonical");
            assert!(
                result.returned,
                "case {id:?} did not return on the interpreter: {}",
                result.envelope
            );
            let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            let value = document["payload"]["outcome"]["value"]
                .as_str()
                .unwrap_or_else(|| panic!("case {id:?}: no returned value in {document}"))
                .parse::<i64>()
                .unwrap_or_else(|error| panic!("case {id:?}: non-integer returned value: {error}"));
            ((*id).to_owned(), value)
        })
        .collect()
}

/// The one function every case's four observations funnel through. Panics
/// naming the case and the disagreeing side on any mismatch.
fn assert_case_agrees(
    case_id: &str,
    expected: i64,
    interpreter_value: i64,
    native_o0: i64,
    native_o2: i64,
    core_wasm: i64,
) {
    assert_eq!(
        interpreter_value, expected,
        "case {case_id:?}: interpreter disagrees with the independently pinned expectation"
    );
    assert_eq!(
        native_o0, expected,
        "case {case_id:?}: native-O0 disagrees with the independently pinned expectation"
    );
    assert_eq!(
        native_o2, expected,
        "case {case_id:?}: native-O2 disagrees with the independently pinned expectation"
    );
    assert_eq!(
        core_wasm, expected,
        "case {case_id:?}: core-wasm disagrees with the independently pinned expectation"
    );
    assert_eq!(
        native_o0, native_o2,
        "case {case_id:?}: native optimization level changed the result"
    );
    assert_eq!(
        interpreter_value, core_wasm,
        "case {case_id:?}: interpreter and core-wasm disagree with each other"
    );
}

#[test]
fn view_ownership_composition_agrees_across_interpreter_native_and_core_wasm() {
    if !require_tools_or_skip() {
        return;
    }

    let program = parse(SOURCE, Path::new("view-ownership-composition.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().all(|d| !d.severity.is_error()),
        "fixture verification failed: {diagnostics:?}"
    );
    let generated = codegen::emit_c(&program).unwrap();
    let root = temporary_root("run");
    fs::create_dir(&root).unwrap();

    let native_o0 = run_native(&generated, &root, "-O0", &CASES);
    let native_o2 = run_native(&generated, &root, "-O2", &CASES);
    let core_wasm_aggregate = run_core_wasm_aggregate(&program, &root);
    let core_wasm = decode_case_values(core_wasm_aggregate, &CASES);

    let interpreter_path = root.join("interpret.spx");
    fs::write(&interpreter_path, SOURCE).unwrap();
    let interpreter_values = run_interpreter(&interpreter_path, &CASES);

    assert_eq!(native_o0.len(), CASES.len());
    assert_eq!(native_o2.len(), CASES.len());
    assert_eq!(core_wasm.len(), CASES.len());
    assert_eq!(interpreter_values.len(), CASES.len());
    assert_eq!(
        core_wasm_aggregate,
        expected_aggregate(&CASES),
        "Core-Wasm's own app.main aggregate does not match the independently computed \
         expectation before it is even decoded per case"
    );

    for (index, (case_id, expected)) in CASES.iter().enumerate() {
        assert_eq!(native_o0[index].0, *case_id);
        assert_eq!(native_o2[index].0, *case_id);
        assert_eq!(interpreter_values[index].0, *case_id);
        assert_case_agrees(
            case_id,
            *expected,
            interpreter_values[index].1,
            native_o0[index].1,
            native_o2[index].1,
            core_wasm[index],
        );
    }

    let _ = fs::remove_dir_all(root);
}

/// The ownership-route pair, on interpreter and native only -- see this
/// module's header doc for exactly why Core-Wasm is bounded out.
#[test]
fn ownership_route_agrees_across_interpreter_and_native() {
    if !require_clang_or_skip() {
        return;
    }

    let program = parse(
        OWNERSHIP_SOURCE,
        Path::new("view-ownership-composition-transfer.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().all(|d| !d.severity.is_error()),
        "ownership fixture verification failed: {diagnostics:?}"
    );
    let generated = codegen::emit_c(&program).unwrap();
    let root = temporary_root("ownership");
    fs::create_dir(&root).unwrap();

    let native_o0 = run_native(&generated, &root, "-O0", &OWNERSHIP_CASES);
    let native_o2 = run_native(&generated, &root, "-O2", &OWNERSHIP_CASES);

    let interpreter_path = root.join("interpret.spx");
    fs::write(&interpreter_path, OWNERSHIP_SOURCE).unwrap();
    let interpreter_values = run_interpreter(&interpreter_path, &OWNERSHIP_CASES);

    assert_eq!(native_o0.len(), OWNERSHIP_CASES.len());
    assert_eq!(native_o2.len(), OWNERSHIP_CASES.len());
    assert_eq!(interpreter_values.len(), OWNERSHIP_CASES.len());

    for (index, (case_id, expected)) in OWNERSHIP_CASES.iter().enumerate() {
        assert_eq!(native_o0[index].0, *case_id);
        assert_eq!(native_o2[index].0, *case_id);
        assert_eq!(interpreter_values[index].0, *case_id);
        assert_eq!(
            interpreter_values[index].1, *expected,
            "case {case_id:?}: interpreter disagrees with the independently pinned expectation"
        );
        assert_eq!(
            native_o0[index].1, *expected,
            "case {case_id:?}: native-O0 disagrees with the independently pinned expectation"
        );
        assert_eq!(
            native_o2[index].1, *expected,
            "case {case_id:?}: native-O2 disagrees with the independently pinned expectation"
        );
    }
    assert_eq!(
        native_o0[0].1, native_o0[1].1,
        "borrow-only and transfer routes disagree on native-O0: ownership route must not be \
         observable"
    );
    assert_eq!(
        interpreter_values[0].1, interpreter_values[1].1,
        "borrow-only and transfer routes disagree on the interpreter: ownership route must not \
         be observable"
    );

    let _ = fs::remove_dir_all(root);
}

/// Negative control (issue #103's own required test): proves
/// `assert_case_agrees` is a real, failable comparison by feeding it a
/// deliberately wrong observed value -- not a compiler-source mutation, per
/// this repository's own guidance to prefer tampered DATA over tampered
/// CODE for a negative control.
#[test]
#[should_panic(expected = "core-wasm disagrees with the independently pinned expectation")]
fn assert_case_agrees_rejects_a_perturbed_engine_value() {
    assert_case_agrees("voc.case_whole_if", 4, 4, 4, 4, 999);
}
