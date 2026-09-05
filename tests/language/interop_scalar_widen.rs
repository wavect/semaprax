//! Executable evidence for the interop scalar widening across
//! `semaprax.abi-report.v1`, `semaprax.c-header.v1`, and
//! `semaprax.cxx-shim.v1`.
//!
//! All three read-only interop projections admit the full Copy-scalar surface
//! (`i64`, `i32`, `u8`, `bool`, `f32`, `f64`, `char`) with mixed signatures,
//! mirroring exactly what the production native C11 projection and the
//! Core-Wasm value-type lowering emit for those scalars. This file pins golden
//! KATs over real repository examples and a deterministic mixed-signature
//! fixture, proves verbatim-native-line agreement and byte-level Wasm
//! cross-consistency, exercises the closed exclusion vocabulary alongside
//! widened admissions, and verifies determinism, budget fail-closure, tamper
//! rejection including forged-but-re-signed envelopes, and CLI exit codes.
//! No C/C++ compiler, Node runtime, or target execution is involved.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

use semaprax::abi_report::{self, AbiReportOptions};
use semaprax::c_header::{self, CHeaderOptions};
use semaprax::cxx_shim::{self, CxxShimOptions};
use semaprax::{codegen, graph, hir, parse, verify, wasm};
use sha2::{Digest as _, Sha256};
use wasmparser::{Parser, Payload};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-interop-widen-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

const MIXED_SOURCE: &str = r#"
module test.widen;

@id("widen.bool")
fn pick_bool(value: bool) -> bool { value }

@id("widen.char")
fn pick_char(value: char) -> char { value }

@id("widen.f32")
fn pick_f32(value: f32) -> f32 { value }

@id("widen.f64")
fn pick_f64(value: f64) -> f64 { value }

@id("widen.i32")
fn pick_i32(value: i32) -> i32 { value }

@id("widen.i64")
fn pick_i64(value: i64) -> i64 { value }

@id("widen.mixed")
fn mixed(flag: bool, count: i64, small: u8, medium: i32, code: char, ratio: f32) -> f64 { 2.5 }

@id("widen.u8")
fn pick_u8(value: u8) -> u8 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Deterministic mixed-signature fixture written once to a fixed relative
/// path so envelope KATs stay machine-independent: the payload binds the
/// displayed source path exactly as given. The file lives under the
/// gitignored build directory and is intentionally never deleted while tests
/// run, so parallel readers stay safe.
const MIXED_FIXTURE_PATH: &str = "target/semaprax-interop-widen-kat/mixed.spx";

fn mixed_fixture() -> PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE
        .get_or_init(|| {
            let directory = Path::new(MIXED_FIXTURE_PATH).parent().expect("parent");
            std::fs::create_dir_all(directory).unwrap();
            let path = PathBuf::from(MIXED_FIXTURE_PATH);
            std::fs::write(&path, MIXED_SOURCE).unwrap();
            path
        })
        .clone()
}

const ALL_WIDENED_TOKENS: &[&str] = &[
    "widen.bool",
    "widen.char",
    "widen.f32",
    "widen.f64",
    "widen.i32",
    "widen.i64",
    "widen.mixed",
    "widen.u8",
];

fn all_widened_options() -> AbiReportOptions {
    AbiReportOptions::new(
        ALL_WIDENED_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect(),
        64 * 1024,
    )
    .expect("valid options")
}

fn cli(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(args)
        .output()
        .expect("semaprax binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

// ---------------------------------------------------------------------------
// Canonical ABI Report v1 over the widened surface.
// ---------------------------------------------------------------------------

/// Pinned golden envelope over the i32 example: mixed widened parameters and
/// a widened result reported through the relative repository path.
#[test]
fn abi_golden_envelope_over_the_i32_example_is_pinned() {
    let options = AbiReportOptions::new(
        vec![
            "sum.pair".to_owned(),
            "compare.pair".to_owned(),
            "sum.checked".to_owned(),
        ],
        64 * 1024,
    )
    .expect("valid options");
    let envelope =
        abi_report::generate(Path::new("examples/integers_i32.spx"), &options).expect("envelope");
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:bb0d2f145c6cb1ab8221bcbaea6794392396c4e1e0b064b72326018ac8efa8f5"
    );
}

/// Pinned golden envelope over the u8 example, including a zero-parameter
/// widened scalar admission.
#[test]
fn abi_golden_envelope_over_the_u8_example_is_pinned() {
    let options = AbiReportOptions::new(
        vec!["byte.limit".to_owned(), "byte.saturating_add".to_owned()],
        64 * 1024,
    )
    .expect("valid options");
    let envelope =
        abi_report::generate(Path::new("examples/bytes_u8.spx"), &options).expect("envelope");
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:6343ee056926e7e5cfae696b76c127771e20bea223067e43a103523adf51efae"
    );
}

/// Pinned golden envelope over the f32 example: one widened admission plus
/// record-parameter exclusions from the same program.
#[test]
fn abi_golden_envelope_over_the_f32_example_is_pinned() {
    let options =
        AbiReportOptions::new(vec!["geometry.half".to_owned()], 64 * 1024).expect("valid options");
    let envelope =
        abi_report::generate(Path::new("examples/floats.spx"), &options).expect("envelope");
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:6ed7ca355b4b645d2ad5a7ec5eb507a988dc5c3be5bebbf32dab5ee7ce90e99b"
    );
}

/// Pinned golden envelope over the fixed-path mixed fixture: every widened
/// type appears in one authenticated envelope.
#[test]
fn abi_golden_envelope_over_the_mixed_fixture_is_pinned() {
    let path = mixed_fixture();
    let options = all_widened_options();
    let envelope = abi_report::generate(&path, &options).expect("envelope");
    assert_eq!(
        sha256_hex(envelope.as_bytes()),
        "sha256:62c60919292c92f2d9ac6b39bf8d79eab0fb8e044e4018b191da0c3a0b586b9e"
    );
    assert!(envelope.contains("\"admitted\":8,\"excluded\":0"));
}

/// The mixed-signature fixture admits every widened scalar at once; native
/// facts mirror the checked layouts and the production C spellings while the
/// canonical section mirrors the exact Core-Wasm value-type lowering.
#[test]
fn abi_widened_mixed_signature_reports_exact_native_and_canonical_facts() {
    let path = mixed_fixture();
    let options = all_widened_options();

    let first = abi_report::generate(&path, &options).expect("envelope");
    let second = abi_report::generate(&path, &options).expect("envelope");
    assert_eq!(first, second, "generation must be deterministic");

    // Checked Native64 layout facts for every widened scalar.
    assert!(first.contains(
        "\"type\":\"bool\",\"c_type\":\"bool\",\"size_bytes\":1,\"align_bytes\":1,\"mode\":\"value\""
    ));
    assert!(first.contains(
        "\"type\":\"u8\",\"c_type\":\"uint8_t\",\"size_bytes\":1,\"align_bytes\":1,\"mode\":\"value\""
    ));
    assert!(first.contains(
        "\"type\":\"i32\",\"c_type\":\"int32_t\",\"size_bytes\":4,\"align_bytes\":4,\"mode\":\"value\""
    ));
    assert!(first.contains(
        "\"type\":\"char\",\"c_type\":\"uint32_t\",\"size_bytes\":4,\"align_bytes\":4,\"mode\":\"value\""
    ));
    assert!(first.contains(
        "\"type\":\"f32\",\"c_type\":\"float\",\"size_bytes\":4,\"align_bytes\":4,\"mode\":\"value\""
    ));
    assert!(first.contains(
        "\"type\":\"f64\",\"c_type\":\"double\",\"size_bytes\":8,\"align_bytes\":8,\"mode\":\"value\""
    ));
    assert!(first.contains(
        "\"type\":\"i64\",\"c_type\":\"int64_t\",\"size_bytes\":8,\"align_bytes\":8,\"mode\":\"value\""
    ));

    let report = abi_report::verify_envelope(&first).expect("verified");
    assert_eq!(report.functions.len(), 8);
    let mixed = report
        .functions
        .iter()
        .find(|function| function.stable_id == "widen.mixed")
        .expect("mixed row");
    assert_eq!(
        mixed.native_signature,
        "static __attribute__((unused)) spx_status_token \
spx_decl_776964656e2e6d69786564(struct spx_context *spx_ctx, bool, int64_t, uint8_t, int32_t, \
uint32_t, float, double *spx_result_out);"
    );
    assert_eq!(
        mixed.wasm_parameters,
        vec!["i32", "i64", "i32", "i32", "i32", "f32"]
    );
    assert_eq!(mixed.wasm_result, "f64");

    // Every reported prototype must be the exact production native line.
    let source = std::fs::read_to_string(&path).unwrap();
    let program = parse(&source, &path).unwrap();
    assert!(verify::verify(&program).is_empty());
    let native = codegen::emit_c(&program).expect("native projection");
    for function in &report.functions {
        assert!(
            native
                .lines()
                .any(|line| line.trim_end() == function.native_signature),
            "widened prototype must appear verbatim in the native projection"
        );
    }
}

/// Byte-level cross-consistency against the real Core-Wasm backend for
/// widened scalars: every reported canonical parameter and result equals the
/// value types the emitted module actually assigns to that function.
#[test]
fn abi_widened_canonical_mapping_matches_the_emitted_core_wasm_module() {
    let path = mixed_fixture();
    let tokens: Vec<String> = [
        "widen.char",
        "widen.f32",
        "widen.f64",
        "widen.i32",
        "widen.mixed",
    ]
    .iter()
    .map(|token| (*token).to_owned())
    .collect();
    let options = AbiReportOptions::new(tokens, 64 * 1024).expect("valid options");
    let envelope = abi_report::generate(&path, &options).expect("envelope");
    let report = abi_report::verify_envelope(&envelope).expect("verified");

    let source = std::fs::read_to_string(&path).unwrap();
    let program = parse(&source, &path).unwrap();
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).expect("resolved");
    let module = wasm::emit_resolved_module(&resolved).expect("core wasm module");

    let mut types: Vec<(Vec<wasmparser::ValType>, Vec<wasmparser::ValType>)> = Vec::new();
    let mut function_types: Vec<u32> = Vec::new();
    for payload in Parser::new(0).parse_all(&module) {
        match payload.unwrap() {
            Payload::TypeSection(section) => {
                types.extend(section.into_iter_err_on_gc_types().map(|ty| {
                    let ty = ty.unwrap();
                    (ty.params().to_vec(), ty.results().to_vec())
                }))
            }
            Payload::FunctionSection(section) => {
                function_types.extend(section.into_iter().map(Result::unwrap));
            }
            _ => {}
        }
    }

    fn text(value: wasmparser::ValType) -> &'static str {
        match value {
            wasmparser::ValType::I64 => "i64",
            wasmparser::ValType::I32 => "i32",
            wasmparser::ValType::F32 => "f32",
            wasmparser::ValType::F64 => "f64",
            other => panic!("unexpected core value type: {other:?}"),
        }
    }

    for function in &report.functions {
        // The FunctionSection lists defined functions in executable order,
        // which equals resolved-HIR source order for monomorphic programs;
        // its entries are type-section indices for those defined functions.
        let position = resolved
            .functions
            .iter()
            .position(|candidate| candidate.id.as_str() == function.stable_id)
            .expect("admitted function must exist in resolved HIR");
        let type_index = function_types[position];
        let (params, results) = types[type_index as usize].clone();
        let observed_params: Vec<&'static str> = params.iter().map(|value| text(*value)).collect();
        let observed_results: Vec<String> = results
            .iter()
            .map(|value| text(*value).to_owned())
            .collect();
        assert_eq!(
            observed_params, function.wasm_parameters,
            "canonical parameters must equal the emitted module for {}",
            function.stable_id
        );
        assert_eq!(
            observed_results,
            vec![function.wasm_result.clone()],
            "canonical result must equal the emitted module for {}",
            function.stable_id
        );
    }
}

/// The closed exclusion vocabulary still holds beside widened admissions.
/// Part one keeps one envelope carrying widened admissions and exclusions
/// whose shapes still lower natively (no resources, effects, or imports).
/// Part two re-proves the whole six-reason vocabulary on an all-excluded
/// program, which never enters backend emission.
#[test]
fn abi_exclusion_reasons_coexist_with_widened_admissions() {
    let coexisting = r#"
module test.probe;

@id("probe.point")
record Point {
    @id("probe.point.x")
    x: i64,
    @id("probe.point.y")
    y: i64,
}

fn hidden(value: i64) -> i64 { value }

@id("probe.stringly")
fn stringly(label: string) -> string { label }

@id("probe.recordly")
fn recordly(point: Point) -> Point { point }

@id("probe.boxed")
fn boxed() -> Point
{
    Point { x: 1, y: 2 }
}

@id("widen.mixed")
fn mixed(flag: bool, count: i64, small: u8, medium: i32, code: char, ratio: f32) -> f64 { 2.5 }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let path = write_temp(coexisting);
    let options = AbiReportOptions::new(
        vec![
            "hidden".to_owned(),
            "probe.stringly".to_owned(),
            "probe.recordly".to_owned(),
            "probe.boxed".to_owned(),
            "widen.mixed".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    let envelope = abi_report::generate(&path, &options).expect("coexistence envelope");
    assert!(envelope.contains("\"reason\":\"automatic_identity\""));
    assert!(envelope.contains("\"reason\":\"unsupported_parameter_type\""));
    assert!(envelope.contains("\"reason\":\"unsupported_result_type\""));
    assert!(envelope.contains("\"admitted\":1,\"excluded\":4"));
    let report = abi_report::verify_envelope(&envelope).expect("verified");
    assert_eq!(
        report.functions[0].wasm_parameters,
        vec!["i32", "i64", "i32", "i32", "i32", "f32"],
        "u8 narrows to i32 while f64 stays exact"
    );
    assert_eq!(report.functions[0].wasm_result, "f64");
    cleanup(&path);

    let all_excluded = r#"
module test.probe;
permit { io.release }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }

@id("probe.effectful")
fn effectful(value: i64) -> i64 uses { io.release } { value }

@id("probe.borrowed")
fn borrowed(target: borrow Buffer, amount: i64) -> i64 { amount }

@id("probe.string")
fn stringly(label: string) -> i64 { 0 }

@id("probe.narrow")
fn narrow(value: i64) -> string { "x" }

@id("app.main")
fn main() -> i64 { 7 }

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}
"#;
    let path = write_temp(all_excluded);
    let excluded_only = AbiReportOptions::new(
        vec![
            "probe.generic".to_owned(),
            "probe.effectful".to_owned(),
            "probe.borrowed".to_owned(),
            "probe.string".to_owned(),
            "probe.narrow".to_owned(),
        ],
        64 * 1024,
    )
    .unwrap();
    let excluded_envelope =
        abi_report::generate(&path, &excluded_only).expect("all-excluded envelope");
    assert!(excluded_envelope.contains("\"reason\":\"generic_function\""));
    assert!(excluded_envelope.contains("\"reason\":\"declared_effects\""));
    assert!(excluded_envelope.contains("\"reason\":\"unsupported_parameter_mode\""));
    assert!(excluded_envelope.contains("\"reason\":\"unsupported_parameter_type\""));
    assert!(excluded_envelope.contains("\"reason\":\"unsupported_result_type\""));
    assert!(excluded_envelope.contains("\"admitted\":0,\"excluded\":5"));
    cleanup(&path);
}

#[test]
fn abi_budget_and_cli_contracts_hold_for_widened_selections() {
    let path = mixed_fixture();
    // Budget exhaustion fails closed without truncation.
    let tiny = AbiReportOptions::new(
        vec!["widen.mixed".to_owned()],
        graph::MIN_AGENT_CONTEXT_BYTES,
    )
    .unwrap();
    let errors = abi_report::generate(&path, &tiny).expect_err("tiny budgets must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-A203"));

    // CLI success over a widened mixed signature.
    let args = [
        "abi-report",
        MIXED_FIXTURE_PATH,
        "--function",
        "widen.mixed",
    ];
    let (code, out, _) = cli(&args);
    assert_eq!(code, 0);
    assert!(out.contains("\"schema\":\"semaprax.abi-report.v1\""));
    assert!(out.contains("\"c_type\":\"uint32_t\""));

    // CLI budget exhaustion exits 1 with the closed diagnostic code.
    let (code, _, err) = cli(&[
        "abi-report",
        MIXED_FIXTURE_PATH,
        "--function",
        "widen.mixed",
        "--max-bytes",
        "2048",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-A203"));
}

/// Tampering with any widened digest cell fails verification, including a
/// forged-but-re-signed envelope that only the independent inner replay of
/// the native signature digest catches.
#[test]
fn abi_tampered_widened_envelopes_are_rejected_even_when_re_signed() {
    let path = mixed_fixture();
    let options =
        AbiReportOptions::new(vec!["widen.mixed".to_owned()], 64 * 1024).expect("options");
    let envelope = abi_report::generate(&path, &options).expect("envelope");

    // Native signature text mutation invalidates its embedded digest.
    let native_tampered = envelope.replace(", uint8_t,", ", uint8_t ,");
    assert_ne!(native_tampered, envelope);
    assert!(abi_report::verify_envelope(&native_tampered).is_err());

    // Canonical mapping mutation invalidates the rebuilt canonical digest.
    let canonical_tampered = envelope.replace("\"results\":[\"f64\"]", "\"results\":[\"f65\"]");
    assert_ne!(canonical_tampered, envelope);
    assert!(abi_report::verify_envelope(&canonical_tampered).is_err());

    // Forged-but-re-signed: honestly recompute the outer digest and byte
    // count over the mutated payload; the inner replay must still fail.
    let forged = resign_abi(envelope.replace(", uint8_t,", ", uint16_t,"));
    assert_ne!(forged, envelope);
    assert!(abi_report::verify_envelope(&forged).is_err());
}

/// Recompute the outer wrapper fields over the current payload bytes using
/// the ABI-report payload domain, so verification can only fail through the
/// independently replayed inner digests.
fn resign_abi(mut envelope: String) -> String {
    resign_with_domain(&mut envelope, b"semaprax.abi-report.payload.v1\0");
    envelope
}

/// Re-sign a mutated shim envelope with the honest outer fields so only the
/// inner artifact-digest replay can reject it.
fn resign_cxx(envelope: String) -> String {
    let mut envelope = envelope;
    resign_with_domain(&mut envelope, b"semaprax.cxx-shim.payload.v1\0");
    envelope
}

fn resign_with_domain(envelope: &mut String, domain: &[u8]) {
    let marker = "\"payload\":";
    let offset = envelope.find(marker).expect("payload member");
    let payload_len = envelope.len() - 1 - (offset + marker.len());
    let payload = &envelope[offset + marker.len()..envelope.len() - 1];
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    let digest = format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    );
    let start = envelope.find("\"digest\":\"").expect("digest member");
    let rest = envelope[start + "\"digest\":\"".len()..].to_owned();
    let end = rest.find('"').expect("digest value end") + start + "\"digest\":\"".len();
    envelope.replace_range(start..end, &format!("\"digest\":\"{digest}\""));
    let bytes_marker = "\"bytes\":";
    let bytes_start = envelope.find(bytes_marker).expect("bytes member");
    let bytes_rest = &envelope[bytes_start + bytes_marker.len()..];
    let bytes_end = bytes_rest.find(',').expect("bytes comma") + bytes_start + bytes_marker.len();
    envelope.replace_range(bytes_start..bytes_end, &format!("\"bytes\":{payload_len}"));
}

fn splice_first(envelope: &str, needle: &str, replacement: &str) -> String {
    let offset = envelope
        .find(needle)
        .unwrap_or_else(|| panic!("anchor `{needle}` must be present"));
    let mut tampered = String::with_capacity(envelope.len() + replacement.len());
    tampered.push_str(&envelope[..offset]);
    tampered.push_str(replacement);
    tampered.push_str(&envelope[offset + needle.len()..]);
    tampered
}

// ---------------------------------------------------------------------------
// C Header Emission v1 over the widened surface.
// ---------------------------------------------------------------------------

/// Header declaration lines for widened scalars are extracted verbatim from
/// the production native projection, and the bare header bytes are pinned,
/// deterministic, and path-independent.
#[test]
fn c_header_widened_lines_are_verbatim_native_declarations_with_pinned_digest() {
    let path = mixed_fixture();
    let options = CHeaderOptions::new(
        ALL_WIDENED_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect(),
        64 * 1024,
    )
    .expect("valid options");

    let header = c_header::header_text(&path, &options).expect("header");
    assert_eq!(
        header,
        c_header::header_text(&path, &options).expect("header"),
        "header emission must be deterministic"
    );

    let source = std::fs::read_to_string(&path).unwrap();
    let program = parse(&source, &path).unwrap();
    let native = codegen::emit_c(&program).expect("native projection");
    let mut prototype_lines = 0usize;
    for line in header.lines() {
        if line.starts_with("static __attribute__((unused))") {
            prototype_lines += 1;
            assert!(
                native
                    .lines()
                    .any(|native_line| native_line.trim_end() == line),
                "widened header line must appear verbatim in the native projection: {line}"
            );
        }
    }
    assert_eq!(prototype_lines, 8);
    assert!(header.contains(
        "(struct spx_context *spx_ctx, bool, int64_t, uint8_t, int32_t, \
uint32_t, float, double *spx_result_out);"
    ));
    assert_eq!(
        sha256_hex(header.as_bytes()),
        "sha256:1686cdb158fe53e12c8646abb7484f26bab66a514a333741d2ee874db5be87ee"
    );

    let envelope = c_header::generate(&path, &options).expect("envelope");
    let verified = c_header::verify_envelope(&envelope).expect("verified");
    assert_eq!(verified, header);

    // Any payload splice invalidates the authenticated envelope.
    let spliced = format!(
        "{}{}",
        &envelope[..envelope.len() - 1],
        ",\"injected\":true}"
    );
    assert!(c_header::verify_envelope(&spliced).is_err());
}

/// A forged-but-re-signed header envelope fails through the independently
/// replayed embedded-header digest, not merely the outer wrapper.
#[test]
fn c_header_forged_but_re_signed_widened_envelopes_are_rejected() {
    let path = mixed_fixture();
    let options =
        CHeaderOptions::new(vec!["widen.mixed".to_owned()], 64 * 1024).expect("valid options");
    let envelope = c_header::generate(&path, &options).expect("envelope");

    let plain_splice = splice_first(&envelope, "double *spx_result_out", "double *spx_result_x");
    assert_ne!(plain_splice, envelope);
    assert!(c_header::verify_envelope(&plain_splice).is_err());

    let forged = {
        let mut envelope = splice_first(&envelope, "SPX_HEADER_", "SPX_HEADER_X");
        resign_with_domain(&mut envelope, b"semaprax.c-header.payload.v1\0");
        envelope
    };
    assert!(c_header::verify_envelope(&forged).is_err());
}

#[test]
fn c_header_budget_and_cli_contracts_hold_for_widened_selections() {
    let path = mixed_fixture();
    let tiny = CHeaderOptions::new(
        vec!["widen.mixed".to_owned()],
        graph::MIN_AGENT_CONTEXT_BYTES,
    )
    .unwrap();
    let errors = c_header::generate(&path, &tiny).expect_err("tiny budgets must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-D103"));

    let args = [
        "c-header",
        MIXED_FIXTURE_PATH,
        "--function",
        "widen.mixed",
        "--emit-header",
    ];
    let (code, out, _) = cli(&args);
    assert_eq!(code, 0);
    assert!(out.starts_with("/*\n"));
    assert!(out.ends_with("#endif\n"));
    assert!(out.contains("double *spx_result_out);"));

    let (code, _, err) = cli(&[
        "c-header",
        MIXED_FIXTURE_PATH,
        "--function",
        "widen.mixed",
        "--max-bytes",
        "2048",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-D103"));
}

// ---------------------------------------------------------------------------
// C++ Shim Projection v1 over the widened surface.
// ---------------------------------------------------------------------------

/// `extern "C"` fragments for widened scalars carry verbatim native lines and
/// pin to stable, path-independent fragment bytes.
#[test]
fn cxx_shim_widened_fragments_are_verbatim_native_declarations_with_pinned_digest() {
    let path = mixed_fixture();
    let options = CxxShimOptions::new(
        ALL_WIDENED_TOKENS
            .iter()
            .map(|token| (*token).to_owned())
            .collect(),
        64 * 1024,
    )
    .expect("valid options");

    let fragment = cxx_shim::fragment_text(&path, &options).expect("fragment");
    assert_eq!(
        fragment,
        cxx_shim::fragment_text(&path, &options).expect("fragment"),
        "fragment emission must be deterministic"
    );
    assert!(fragment.contains("extern \"C\" {"));
    assert!(fragment.ends_with("\n}\n\n#endif\n"));
    assert!(fragment.contains(
        "(struct spx_context *spx_ctx, bool, int64_t, uint8_t, int32_t, \
uint32_t, float, double *spx_result_out);"
    ));

    let source = std::fs::read_to_string(&path).unwrap();
    let program = parse(&source, &path).unwrap();
    let native = codegen::emit_c(&program).expect("native projection");
    for line in fragment.lines() {
        if line.starts_with("static __attribute__((unused))") {
            assert!(
                native
                    .lines()
                    .any(|native_line| native_line.trim_end() == line),
                "widened fragment line must appear verbatim in the native projection: {line}"
            );
        }
    }
    assert_eq!(
        sha256_hex(fragment.as_bytes()),
        "sha256:cde573faf7f9854a9eb31854636b2222b477a85704982a2cc9b055dd5f58f8a6"
    );

    let envelope = cxx_shim::generate(&path, &options).expect("envelope");
    let verified = cxx_shim::verify_envelope(&envelope).expect("verified");
    assert_eq!(verified, fragment);
}

/// A forged-but-re-signed shim envelope fails through the independently
/// replayed embedded-fragment digest, not merely the outer wrapper.
#[test]
fn cxx_shim_forged_but_re_signed_widened_envelopes_are_rejected() {
    let path = mixed_fixture();
    let options = CxxShimOptions::new(vec!["widen.mixed".to_owned()], 64 * 1024).expect("options");
    let envelope = cxx_shim::generate(&path, &options).expect("envelope");

    let plain_splice = splice_first(&envelope, "double *spx_result_out", "double *spx_result_x");
    assert_ne!(plain_splice, envelope);
    assert!(cxx_shim::verify_envelope(&plain_splice).is_err());

    let forged = resign_cxx(splice_first(&envelope, "SPX_CXX_SHIM_", "SPX_CXX_SHIX_"));
    assert_ne!(forged, envelope);
    assert!(cxx_shim::verify_envelope(&forged).is_err());
}

#[test]
fn cxx_shim_budget_and_cli_contracts_hold_for_widened_selections() {
    let path = mixed_fixture();
    let tiny = CxxShimOptions::new(
        vec!["widen.mixed".to_owned()],
        graph::MIN_AGENT_CONTEXT_BYTES,
    )
    .unwrap();
    let errors = cxx_shim::generate(&path, &tiny).expect_err("tiny budgets must fail closed");
    assert!(errors.iter().any(|item| item.code == "SPX-X103"));

    let args = [
        "cxx-shim",
        MIXED_FIXTURE_PATH,
        "--function",
        "widen.mixed",
        "--emit-fragment",
    ];
    let (code, out, _) = cli(&args);
    assert_eq!(code, 0);
    assert!(out.contains("extern \"C\" {"));
    assert!(out.contains("float, double *spx_result_out);"));

    let (code, _, err) = cli(&[
        "cxx-shim",
        MIXED_FIXTURE_PATH,
        "--function",
        "widen.mixed",
        "--max-bytes",
        "2048",
    ]);
    assert_eq!(code, 1);
    assert!(err.contains("SPX-X103"));
}
