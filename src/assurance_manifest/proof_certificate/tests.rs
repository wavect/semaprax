//! Tests for the standalone SMT proof certificate: rendering determinism,
//! structural (filesystem-free) replay, source-binding drift, the
//! mutation-ladder ("source, proof, compiler ... changes stale the
//! certificate"), and — only under `--ignored`, with `SEMAPRAX_SMT_Z3_PATH`
//! provisioned — real end-to-end coverage against Z3, following the same
//! opt-in pattern `smt_discharge::tests` uses for its own solver-backed
//! cases.
//!
//! The single most important test here is
//! `verify_certificate_against_source_rejects_a_script_tampered_with_the_known_vacuous_result_axiom_bug_class`:
//! it reproduces, at the certificate layer, the exact shape of #184's worst
//! bug (giving `result` the same unconditional range axiom a genuine
//! parameter gets) and confirms independent replay refuses to accept a
//! certificate carrying it, because the embedded script no longer matches
//! what the real translator deterministically derives from the unchanged
//! source.

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::render::{render, CertificateBody, RenderInput};
use super::{
    export_postcondition_certificate, verify_certificate, verify_certificate_against_artifact,
    verify_certificate_against_source, verify_certificate_with_solver,
};

use crate::assurance_manifest::smt_discharge::{
    self, postcondition_obligation_id, provision_from_env, render_postcondition_script, run,
    translate_function, Model, ModelValue, Provisioning, ReplayOutcome, RunLimits, Verdict,
    ENV_Z3_PATH,
};
use crate::diagnostic::quote_json;

fn short_limits() -> RunLimits {
    RunLimits {
        timeout: Duration::from_secs(5),
        max_output_bytes: 65_536,
    }
}

/// Append the executable entry point every fixture below needs: `hir::resolve`
/// (and so [`crate::wasm::emit_resolved_module`], the artifact-binding
/// target) rejects a module with no `fn main() -> i64` as a library module
/// (`SPX-T105`), independently of whatever this module's own bounded
/// SMT-discharge subset otherwise accepts.
fn with_executable_main(source: &str) -> String {
    format!("{source}\n@id(\"app.t.proof_certificate_test_main\")\nfn main() -> i64 {{ 0 }}\n")
}

fn write_temp(source: &str, label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-proof-certificate-{label}-{}-{}.spx",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, with_executable_main(source)).unwrap();
    path
}

fn true_postcondition_source() -> String {
    "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a >= 0\n    ensures result >= 0\n{ a }\n"
        .to_owned()
}

fn false_postcondition_source() -> String {
    "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    ensures result > a\n{ a }\n"
        .to_owned()
}

fn extremum_overflow_source() -> String {
    format!(
        "module app.t;\n@id(\"app.t.f\")\nfn f(a: i64) -> i64\n    requires a == {}\n    ensures result > a\n{{ a + 1 }}\n",
        i64::MAX
    )
}

/// Genuinely compile `program` to its Wasm core module bytes the same way
/// [`super::export_postcondition_certificate`] does, and return `(bytes_len,
/// sha256)` for embedding in a test certificate. Used both by
/// [`genuine_certificate`] and by hand-crafted certificates whose script
/// portion is genuine (so execution reaches the artifact-recomputation
/// check in [`verify_certificate_against_source`] before whatever later
/// check the test targets).
fn compute_wasm_artifact(program: &crate::ast::Program) -> (usize, String) {
    let resolved = crate::hir::resolve(program).expect("resolves");
    let bytes = crate::wasm::emit_resolved_module(&resolved).expect("compiles to Wasm");
    (bytes.len(), super::render::artifact_digest(&bytes))
}

/// A structurally well-formed but otherwise meaningless artifact triple, for
/// hand-crafted certificates whose test never reaches the point where
/// [`verify_certificate_against_source`] would recompute and compare the
/// real artifact (either because the test only calls the filesystem-free
/// [`verify_certificate`], or because an earlier check is expected to reject
/// the certificate first).
fn placeholder_artifact_sha256() -> String {
    format!("sha256:{}", "a".repeat(64))
}

fn unsupported_subset_source() -> String {
    "module app.t;\n@id(\"app.t.helper\")\nfn helper(a: i64) -> i64 { a }\n\
     @id(\"app.t.f\")\nfn f(a: i64) -> i64\n    ensures result >= 0\n{ helper(a) }\n"
        .to_owned()
}

/// Build a `CertificateBody::Proved`/`Refuted` certificate whose `script` is
/// the real deterministic translation of `declaration_id` inside
/// `source_text`, exactly as [`super::export_postcondition_certificate`]
/// would render it (offline: no solver is spawned to build these — the
/// `Refuted` case is validated genuinely via
/// [`smt_discharge::replay_function`], the same checked-arithmetic
/// evaluator a real discharge attempt uses; the `Proved` case's `unsat`
/// claim is asserted by the test, not confirmed by a solver, and is only
/// used to test rendering/structural-replay plumbing — real soundness
/// coverage is the `provisioned_z3_*` tests below).
fn genuine_certificate(
    source_path: &Path,
    source_text: &str,
    declaration_id: &str,
    ensures_index: usize,
    body: CertificateBody,
) -> String {
    let program = crate::parse(source_text, source_path).expect("parse");
    let revision = crate::graph::revision(&program);
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == declaration_id)
        .expect("declaration present")
        .clone();
    let encoding = translate_function(&function).expect("supported");
    let timeout_ms = 5_000u64;
    let script = render_postcondition_script(&encoding, ensures_index, timeout_ms);
    let obligation_id = postcondition_obligation_id(declaration_id, ensures_index);
    let source_sha256 = super::render::source_digest(source_text);
    let (artifact_bytes, artifact_sha256) = compute_wasm_artifact(&program);
    let input = RenderInput {
        source_path_text: &source_path.display().to_string(),
        revision: &revision,
        source_sha256: &source_sha256,
        declaration_id,
        obligation_id: &obligation_id,
        ensures_index,
        compiler_version: env!("CARGO_PKG_VERSION"),
        timeout_ms,
        max_output_bytes: 65_536,
        solver_identity: "z3",
        solver_version: "test-fixture",
        artifact_sha256: &artifact_sha256,
        artifact_bytes,
        script: &script,
        body: &body,
    };
    render(&input)
}

fn proved_certificate(source_path: &Path, source_text: &str, declaration_id: &str) -> String {
    genuine_certificate(
        source_path,
        source_text,
        declaration_id,
        0,
        CertificateBody::Proved,
    )
}

/// Build a genuinely validated `Refuted` certificate: `model` is replayed
/// against the real declaration via the same checked-arithmetic evaluator a
/// live discharge attempt uses, and the test asserts the outcome really is
/// a validated counterexample before certifying it — no solver involved,
/// but no fabrication either.
fn refuted_certificate(
    source_path: &Path,
    source_text: &str,
    declaration_id: &str,
    model: Model,
) -> String {
    let program = crate::parse(source_text, source_path).expect("parse");
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == declaration_id)
        .expect("declaration present")
        .clone();
    let outcome = smt_discharge::replay_function(&function, &model).expect("replay evaluates");
    assert!(
        matches!(
            outcome,
            ReplayOutcome::Trapped { .. } | ReplayOutcome::EnsuresViolated { .. }
        ),
        "test fixture model must be a genuine, validated counterexample, got {outcome:?}"
    );
    genuine_certificate(
        source_path,
        source_text,
        declaration_id,
        0,
        CertificateBody::Refuted { model, outcome },
    )
}

/// Hand-craft a certificate directly from field values, bypassing
/// [`render::render`] entirely. Used to construct certificates an honest
/// exporter would never produce (a false `verdict`/`counterexample`
/// pairing, a forged `obligation_id`, a claimed declaration absent from the
/// bound source, a tampered script) so the independent verifier's own
/// checks — not this module's own renderer — are what is under test.
#[allow(clippy::too_many_arguments)]
fn hand_crafted_certificate(
    source_path_text: &str,
    revision: &str,
    source_sha256: &str,
    declaration_id: &str,
    obligation_id: &str,
    ensures_index: usize,
    compiler_version: &str,
    script: &str,
    script_sha256: &str,
    verdict: &str,
    counterexample_json: &str,
    artifact_bytes: usize,
    artifact_sha256: &str,
) -> String {
    let payload = format!(
        "{{\"schema\":{schema},\"source\":{{\"path\":{path},\"revision\":{revision},\"sha256\":{sha}}},\
\"declaration_id\":{declaration_id},\"obligation_id\":{obligation_id},\"ensures_index\":{ensures_index},\
\"compiler_version\":{compiler_version},\"bounds\":{bounds},\
\"artifact\":{{\"target\":{artifact_target},\"bytes\":{artifact_bytes},\"sha256\":{artifact_sha256}}},\
\"solver\":{{\"identity\":{identity},\"version\":{version}}},\
\"limits\":{{\"timeout_ms\":5000,\"max_output_bytes\":65536}},\"script\":{script_json},\
\"script_sha256\":{script_sha_json},\"verdict\":{verdict_json},\"counterexample\":{counterexample_json},\
\"nonclaims\":[]}}",
        schema = quote_json(super::render::SCHEMA),
        path = quote_json(source_path_text),
        revision = quote_json(revision),
        sha = quote_json(source_sha256),
        declaration_id = quote_json(declaration_id),
        obligation_id = quote_json(obligation_id),
        ensures_index = ensures_index,
        compiler_version = quote_json(compiler_version),
        bounds = quote_json(smt_discharge::BOUNDS_V1),
        artifact_target = quote_json(super::render::ARTIFACT_TARGET_WASM_CORE_MODULE_V1),
        artifact_bytes = artifact_bytes,
        artifact_sha256 = quote_json(artifact_sha256),
        identity = quote_json("z3"),
        version = quote_json("test"),
        script_json = quote_json(script),
        script_sha_json = quote_json(script_sha256),
        verdict_json = quote_json(verdict),
    );
    format!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(super::render::SCHEMA),
        quote_json(&super::render::payload_digest(payload.as_bytes())),
        payload.len(),
        payload,
    )
}

fn extract_between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let s = text
        .find(start)
        .unwrap_or_else(|| panic!("{start} not found"))
        + start.len();
    let e = s + text[s..]
        .find(end)
        .unwrap_or_else(|| panic!("{end} not found after {start}"));
    &text[s..e]
}

// ---------------------------------------------------------------------
// Rendering determinism and structural (filesystem-free) replay
// ---------------------------------------------------------------------

#[test]
fn rendering_is_deterministic_for_a_proved_certificate() {
    let source = with_executable_main(&true_postcondition_source());
    let path = Path::new("in-memory-fixture.spx");
    let first = proved_certificate(path, &source, "app.t.f");
    let second = proved_certificate(path, &source, "app.t.f");
    assert_eq!(first, second);
}

#[test]
fn a_proved_certificate_round_trips_through_verify_certificate() {
    let source = with_executable_main(&true_postcondition_source());
    let path = Path::new("in-memory-fixture.spx");
    let certificate = proved_certificate(path, &source, "app.t.f");
    verify_certificate(&certificate).expect("a genuine proved certificate must verify");
}

#[test]
fn a_refuted_certificate_round_trips_through_verify_certificate() {
    let source = with_executable_main(&extremum_overflow_source());
    let path = Path::new("in-memory-fixture.spx");
    let mut model = Model::new();
    model.insert("a".to_owned(), ModelValue::Int(i64::MAX as i128));
    let certificate = refuted_certificate(path, &source, "app.t.f", model);
    verify_certificate(&certificate).expect("a genuine refuted certificate must verify");
}

#[test]
fn verify_certificate_rejects_malformed_json() {
    let error = verify_certificate("not json").expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
}

#[test]
fn verify_certificate_rejects_a_tampered_envelope_digest() {
    let source = with_executable_main(&true_postcondition_source());
    let path = Path::new("in-memory-fixture.spx");
    let certificate = proved_certificate(path, &source, "app.t.f");
    let real_digest = extract_between(&certificate, "\"digest\":\"", "\"").to_owned();
    let fake_digest = format!("sha256:{}", "0".repeat(64));
    assert_ne!(real_digest, fake_digest);
    let tampered = certificate.replacen(&real_digest, &fake_digest, 1);
    let error = verify_certificate(&tampered).expect_err("tampered digest must be rejected");
    assert_eq!(error.code, "SPX-Z106");
}

#[test]
fn verify_certificate_rejects_a_mismatched_script_sha256() {
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let certificate = hand_crafted_certificate(
        "t.spx",
        "rev",
        &format!("sha256:{}", "0".repeat(64)),
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &format!("sha256:{}", "1".repeat(64)),
        "proved",
        "null",
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate(&certificate).expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
}

#[test]
fn verify_certificate_rejects_an_obligation_id_that_does_not_match_declaration_and_index() {
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let certificate = hand_crafted_certificate(
        "t.spx",
        "rev",
        &format!("sha256:{}", "0".repeat(64)),
        "app.t.f",
        "semaprax.obligation.v1:not-a-real-id",
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &super::render::script_digest(script),
        "proved",
        "null",
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate(&certificate).expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
}

#[test]
fn verify_certificate_rejects_proved_with_a_nonnull_counterexample() {
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let certificate = hand_crafted_certificate(
        "t.spx",
        "rev",
        &format!("sha256:{}", "0".repeat(64)),
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &super::render::script_digest(script),
        "proved",
        "{\"kind\":\"trapped\",\"detail\":\"x\",\"ensures_index\":null,\"model\":[]}",
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate(&certificate).expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
}

#[test]
fn verify_certificate_rejects_refuted_with_a_null_counterexample() {
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let certificate = hand_crafted_certificate(
        "t.spx",
        "rev",
        &format!("sha256:{}", "0".repeat(64)),
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &super::render::script_digest(script),
        "refuted",
        "null",
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate(&certificate).expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
}

#[test]
fn verify_certificate_rejects_an_unrecognized_verdict_token() {
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let certificate = hand_crafted_certificate(
        "t.spx",
        "rev",
        &format!("sha256:{}", "0".repeat(64)),
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &super::render::script_digest(script),
        "maybe",
        "null",
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate(&certificate).expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
}

#[test]
fn verify_certificate_rejects_model_entries_out_of_ascending_order() {
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let counterexample = "{\"kind\":\"trapped\",\"detail\":\"x\",\"ensures_index\":null,\
\"model\":[{\"name\":\"b\",\"sort\":\"int\",\"value\":\"1\"},{\"name\":\"a\",\"sort\":\"int\",\"value\":\"0\"}]}";
    let certificate = hand_crafted_certificate(
        "t.spx",
        "rev",
        &format!("sha256:{}", "0".repeat(64)),
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &super::render::script_digest(script),
        "refuted",
        counterexample,
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate(&certificate).expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
}

#[test]
fn verify_certificate_rejects_a_non_integer_value_for_an_int_sort_model_entry() {
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let counterexample = "{\"kind\":\"trapped\",\"detail\":\"x\",\"ensures_index\":null,\
\"model\":[{\"name\":\"a\",\"sort\":\"int\",\"value\":\"not-a-number\"}]}";
    let certificate = hand_crafted_certificate(
        "t.spx",
        "rev",
        &format!("sha256:{}", "0".repeat(64)),
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &super::render::script_digest(script),
        "refuted",
        counterexample,
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate(&certificate).expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
}

// ---------------------------------------------------------------------
// Source binding, drift, and the mutation ladder
// (`verify_certificate_against_source`)
// ---------------------------------------------------------------------

#[test]
fn verify_certificate_against_source_accepts_a_genuine_certificate() {
    let path = write_temp(&true_postcondition_source(), "accept");
    let source_text = std::fs::read_to_string(&path).unwrap();
    let certificate = proved_certificate(&path, &source_text, "app.t.f");
    let result = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    result.expect("a genuine certificate bound to its exact source must verify");
}

#[test]
fn verify_certificate_against_source_rejects_after_source_drift() {
    let path = write_temp(&true_postcondition_source(), "drift");
    let source_text = std::fs::read_to_string(&path).unwrap();
    let certificate = proved_certificate(&path, &source_text, "app.t.f");

    // Mutate the file on disk after the certificate was produced.
    std::fs::write(
        &path,
        source_text.replace("requires a >= 0", "requires a >= 1"),
    )
    .unwrap();

    let error = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    let error = error.expect_err("drifted source must be rejected");
    assert_eq!(error.code, "SPX-Z107");
}

#[test]
fn verify_certificate_against_source_rejects_a_declaration_absent_from_the_bound_source() {
    // The certificate's `source_sha256` correctly matches this exact file's
    // bytes; the file simply never contained the declaration the
    // certificate claims.
    let source = "module app.t;\n@id(\"app.t.other\")\nfn other(a: i64) -> i64 { a }\n\
                  @id(\"app.t.proof_certificate_test_main\")\nfn main() -> i64 { 0 }\n";
    let path = write_temp("", "missing-declaration");
    std::fs::write(&path, source).unwrap();
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let source_sha256 = super::render::source_digest(source);
    let revision = crate::graph::revision(&crate::parse(source, &path).unwrap());
    let certificate = hand_crafted_certificate(
        &path.display().to_string(),
        &revision,
        &source_sha256,
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &super::render::script_digest(script),
        "proved",
        "null",
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    let error =
        error.expect_err("a claimed declaration absent from the bound source must be rejected");
    assert_eq!(error.code, "SPX-Z107");
}

#[test]
fn verify_certificate_against_source_rejects_a_compiler_version_mismatch() {
    let path = write_temp(&true_postcondition_source(), "compiler-drift");
    let source_text = std::fs::read_to_string(&path).unwrap();
    let program = crate::parse(&source_text, &path).unwrap();
    let revision = crate::graph::revision(&program);
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == "app.t.f")
        .unwrap()
        .clone();
    let encoding = translate_function(&function).unwrap();
    let script = render_postcondition_script(&encoding, 0, 5000);
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let source_sha256 = super::render::source_digest(&source_text);
    let certificate = hand_crafted_certificate(
        &path.display().to_string(),
        &revision,
        &source_sha256,
        "app.t.f",
        &obligation_id,
        0,
        "0.0.0-not-the-real-compiler",
        &script,
        &super::render::script_digest(&script),
        "proved",
        "null",
        0,
        &placeholder_artifact_sha256(),
    );
    let error = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    let error = error.expect_err("a compiler-version mismatch must be rejected as stale");
    assert_eq!(error.code, "SPX-Z107");
}

/// The core regression this module exists to make possible: reproduce, at
/// the certificate layer, the exact shape of #184's worst bug — giving
/// `result` the same unconditional range axiom a genuine parameter gets,
/// which made a real `i64::MAX + 1` overflow vacuously "unsat" — and
/// confirm independent replay refuses to accept a certificate carrying it,
/// because the tampered script no longer matches what the real translator
/// deterministically re-derives from the unchanged source. `verify_
/// certificate` alone (no source access) cannot detect this: the tampered
/// certificate is perfectly self-consistent by its own internal digests.
#[test]
fn verify_certificate_against_source_rejects_a_script_tampered_with_the_known_vacuous_result_axiom_bug_class(
) {
    let source = extremum_overflow_source();
    let path = write_temp(&source, "vacuous-result-axiom");
    let source_text = std::fs::read_to_string(&path).unwrap();
    let program = crate::parse(&source_text, &path).unwrap();
    let revision = crate::graph::revision(&program);
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == "app.t.f")
        .unwrap()
        .clone();
    let encoding = translate_function(&function).expect("supported");
    let honest_script = render_postcondition_script(&encoding, 0, 5000);
    assert!(!honest_script.contains("(and (>= result"));

    let tampered_script = honest_script.replacen(
        "(declare-const result Int)\n",
        "(declare-const result Int)\n(assert (and (>= result -9223372036854775808) \
         (<= result 9223372036854775807)))\n",
        1,
    );
    assert_ne!(tampered_script, honest_script);
    assert!(tampered_script.contains("(and (>= result"));

    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let source_sha256 = super::render::source_digest(&source_text);
    let certificate = hand_crafted_certificate(
        &path.display().to_string(),
        &revision,
        &source_sha256,
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        &tampered_script,
        &super::render::script_digest(&tampered_script),
        "proved",
        "null",
        0,
        &placeholder_artifact_sha256(),
    );

    // Internally self-consistent: nothing about the tampering is visible
    // without recomputing the script from source.
    verify_certificate(&certificate).expect("internally consistent certificate");

    let result = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    let error = result.expect_err(
        "a script that is not the deterministic translation of the exact source must be rejected",
    );
    assert_eq!(error.code, "SPX-Z106");
    assert!(error.message.contains("vacuously"));
}

#[test]
fn verify_certificate_against_source_reproduces_a_refuted_extremum_counterexample() {
    let source = extremum_overflow_source();
    let path = write_temp(&source, "extremum-refuted");
    let source_text = std::fs::read_to_string(&path).unwrap();
    let mut model = Model::new();
    model.insert("a".to_owned(), ModelValue::Int(i64::MAX as i128));
    let certificate = refuted_certificate(&path, &source_text, "app.t.f", model);
    let result = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    result.expect("a genuine, validated counterexample must independently replay");
}

#[test]
fn verify_certificate_against_source_rejects_a_counterexample_that_does_not_replay() {
    let source = true_postcondition_source();
    let path = write_temp(&source, "false-counterexample");
    let source_text = std::fs::read_to_string(&path).unwrap();
    // `a = 5` never traps and never violates `result >= 0` for `f`, so this
    // "counterexample" is fabricated, not a genuine replay result.
    let mut model = Model::new();
    model.insert("a".to_owned(), ModelValue::Int(5));
    let program = crate::parse(&source_text, &path).unwrap();
    let revision = crate::graph::revision(&program);
    let encoding = translate_function(
        program
            .functions
            .iter()
            .find(|candidate| candidate.stable_id == "app.t.f")
            .unwrap(),
    )
    .unwrap();
    let script = render_postcondition_script(&encoding, 0, 5000);
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let source_sha256 = super::render::source_digest(&source_text);
    let counterexample = "{\"kind\":\"trapped\",\"detail\":\"fabricated\",\"ensures_index\":null,\
\"model\":[{\"name\":\"a\",\"sort\":\"int\",\"value\":\"5\"}]}";
    // The script is the honest translation of the real source, so execution
    // must reach the artifact-recomputation check before the replay check
    // this test targets — the bound artifact must therefore be genuine too,
    // or this test would spuriously fail with an artifact-drift error
    // instead of the fabricated-counterexample rejection it means to cover.
    let (artifact_bytes, artifact_sha256) = compute_wasm_artifact(&program);
    let certificate = hand_crafted_certificate(
        &path.display().to_string(),
        &revision,
        &source_sha256,
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        &script,
        &super::render::script_digest(&script),
        "refuted",
        counterexample,
        artifact_bytes,
        &artifact_sha256,
    );
    let result = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    let error = result.expect_err("a fabricated counterexample must not independently replay");
    assert_eq!(error.code, "SPX-Z106");
}

// ---------------------------------------------------------------------
// Artifact binding (issue #186 "Artifact binding for one target"): the
// certificate's `payload.artifact` field and the two independent ways to
// check it — recompiling from bound source, or hashing a caller-supplied
// artifact directly.
// ---------------------------------------------------------------------

/// The artifact-binding analogue of
/// `verify_certificate_against_source_rejects_a_script_tampered_with_the_known_vacuous_result_axiom_bug_class`:
/// a certificate whose `script` is the genuine, honest translation of the
/// exact bound source (so it passes every earlier check, including the
/// script-recomputation check) but whose `artifact.sha256` does not match
/// what that exact source actually compiles to. `verify_certificate` alone
/// cannot detect this — the certificate is perfectly self-consistent by its
/// own internal digests — only recompiling the real artifact from source
/// catches it. Without this test, a tampered or stale artifact digest would
/// pass every check this module runs by default.
#[test]
fn verify_certificate_against_source_rejects_a_tampered_artifact_digest_even_with_a_genuine_script()
{
    let source = true_postcondition_source();
    let path = write_temp(&source, "tampered-artifact");
    let source_text = std::fs::read_to_string(&path).unwrap();
    let program = crate::parse(&source_text, &path).unwrap();
    let revision = crate::graph::revision(&program);
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == "app.t.f")
        .unwrap();
    let encoding = translate_function(function).unwrap();
    let script = render_postcondition_script(&encoding, 0, 5000);
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let source_sha256 = super::render::source_digest(&source_text);

    // A well-formed digest that is not the real compiled artifact's.
    let (real_artifact_bytes, real_artifact_sha256) = compute_wasm_artifact(&program);
    let fake_artifact_sha256 = format!("sha256:{}", "0".repeat(64));
    assert_ne!(fake_artifact_sha256, real_artifact_sha256);

    let certificate = hand_crafted_certificate(
        &path.display().to_string(),
        &revision,
        &source_sha256,
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        &script,
        &super::render::script_digest(&script),
        "proved",
        "null",
        real_artifact_bytes,
        &fake_artifact_sha256,
    );

    // Internally self-consistent: nothing about the tampering is visible
    // without recompiling the artifact from source.
    verify_certificate(&certificate).expect("internally consistent certificate");

    let result = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    let error = result.expect_err(
        "an artifact digest that is not the exact current source's compiled Wasm core module \
         must be rejected",
    );
    assert_eq!(error.code, "SPX-Z107");
    assert!(error.message.contains("artifact"));
}

/// `hand_crafted_certificate` always writes the one currently supported
/// target (`wasm-core-module-v1`); splice in an unrecognized one instead and
/// re-derive the envelope digest over the tampered payload (otherwise the
/// envelope-digest check — not the target-vocabulary check this test
/// targets — would be the first thing to reject it).
#[test]
fn verify_certificate_rejects_an_artifact_target_outside_the_closed_vocabulary() {
    let script = "(set-option :timeout 5000)\n(set-logic QF_LIA)\n(check-sat)\n";
    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let certificate = hand_crafted_certificate(
        "t.spx",
        "rev",
        &format!("sha256:{}", "0".repeat(64)),
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        script,
        &super::render::script_digest(script),
        "proved",
        "null",
        0,
        &placeholder_artifact_sha256(),
    );
    let tampered_target = certificate.replacen(
        &format!(
            "\"target\":{}",
            quote_json(super::render::ARTIFACT_TARGET_WASM_CORE_MODULE_V1)
        ),
        "\"target\":\"native-c11-source-v1\"",
        1,
    );
    // Splicing in a longer target string shifts the payload's byte length,
    // so both the envelope digest and the envelope's declared `bytes` count
    // must be re-derived from the tampered payload — otherwise the earlier,
    // unrelated "declares N payload bytes but M are present" check would
    // reject this certificate first, defeating the point of this test.
    let payload_start = tampered_target.find("\"payload\":").unwrap() + "\"payload\":".len();
    let payload = &tampered_target[payload_start..tampered_target.len() - 1];
    let fixed_digest = super::render::payload_digest(payload.as_bytes());
    let fixed_bytes = payload.len();

    let real_digest = extract_between(&certificate, "\"digest\":\"", "\"").to_owned();
    let real_bytes = extract_between(&certificate, "\"bytes\":", ",\"payload\":").to_owned();
    let refixed = tampered_target
        .replacen(&real_digest, &fixed_digest, 1)
        .replacen(
            &format!("\"bytes\":{real_bytes}"),
            &format!("\"bytes\":{fixed_bytes}"),
            1,
        );

    let error = verify_certificate(&refixed).expect_err("must reject");
    assert_eq!(error.code, "SPX-Z106");
    assert!(error.message.contains("closed vocabulary"));
}

#[test]
fn verify_certificate_against_artifact_accepts_the_genuine_compiled_wasm_bytes() {
    let source = with_executable_main(&true_postcondition_source());
    let path = Path::new("in-memory-fixture.spx");
    let certificate = proved_certificate(path, &source, "app.t.f");
    let program = crate::parse(&source, path).unwrap();
    let resolved = crate::hir::resolve(&program).unwrap();
    let artifact_bytes = crate::wasm::emit_resolved_module(&resolved).unwrap();
    verify_certificate_against_artifact(&certificate, &artifact_bytes)
        .expect("the exact genuine compiled artifact must bind");
}

#[test]
fn verify_certificate_against_artifact_rejects_different_bytes() {
    let source = with_executable_main(&true_postcondition_source());
    let path = Path::new("in-memory-fixture.spx");
    let certificate = proved_certificate(path, &source, "app.t.f");
    let program = crate::parse(&source, path).unwrap();
    let resolved = crate::hir::resolve(&program).unwrap();
    let mut artifact_bytes = crate::wasm::emit_resolved_module(&resolved).unwrap();
    // Flip one byte: still plausible Wasm-shaped bytes, but not the exact
    // artifact this certificate is bound to.
    let last = artifact_bytes.len() - 1;
    artifact_bytes[last] ^= 0xFF;
    let error = verify_certificate_against_artifact(&certificate, &artifact_bytes)
        .expect_err("tampered artifact bytes must not bind");
    assert_eq!(error.code, "SPX-Z106");
}

/// The cross-artifact case `..._rejects_different_bytes` above only proves
/// with a single flipped byte, not with a genuinely distinct program's
/// independently compiled artifact. Build a certificate bound to program
/// A's Wasm core module, then present program B's — a different function
/// body and `requires` literal, compiled through the exact same pipeline —
/// and confirm the certificate for A is refused against B's bytes, not just
/// against a corrupted copy of A's own. This is the direct evidence for
/// "bind certificates to artifact bytes": a certificate that proves
/// something about artifact A must not be replayable against artifact B.
#[test]
fn verify_certificate_against_artifact_rejects_a_different_programs_artifact() {
    let source_a = with_executable_main(&true_postcondition_source());
    let path_a = Path::new("in-memory-fixture-a.spx");
    let certificate = proved_certificate(path_a, &source_a, "app.t.f");
    let program_a = crate::parse(&source_a, path_a).unwrap();
    let resolved_a = crate::hir::resolve(&program_a).unwrap();
    let artifact_bytes_a = crate::wasm::emit_resolved_module(&resolved_a).unwrap();

    let source_b = with_executable_main(&extremum_overflow_source());
    let path_b = Path::new("in-memory-fixture-b.spx");
    let program_b = crate::parse(&source_b, path_b).unwrap();
    let resolved_b = crate::hir::resolve(&program_b).unwrap();
    let artifact_bytes_b = crate::wasm::emit_resolved_module(&resolved_b).unwrap();

    // Confirm these really are two different artifacts, or the rejection
    // below would be vacuous.
    assert_ne!(artifact_bytes_a, artifact_bytes_b);

    // Sanity: the certificate does bind to its own program's artifact.
    verify_certificate_against_artifact(&certificate, &artifact_bytes_a)
        .expect("a certificate must still bind to its own program's genuine artifact");

    let error = verify_certificate_against_artifact(&certificate, &artifact_bytes_b)
        .expect_err("a certificate bound to program A's artifact must refuse program B's");
    assert_eq!(error.code, "SPX-Z106");
}

// ---------------------------------------------------------------------
// `export_postcondition_certificate` without a real solver
// ---------------------------------------------------------------------

#[test]
fn export_without_a_solver_provisioned_is_refused() {
    let path = write_temp(&true_postcondition_source(), "no-solver");
    let result = export_postcondition_certificate(&path, "app.t.f", 0, None, &short_limits());
    std::fs::remove_file(&path).ok();
    let diagnostics = result.expect_err("no certificate without a real solver run");
    assert_eq!(diagnostics[0].code, "SPX-Z105");
    assert!(diagnostics[0].message.contains(ENV_Z3_PATH));
}

#[test]
fn export_rejects_a_declaration_outside_the_bounded_subset_before_requiring_a_solver() {
    let path = write_temp(&unsupported_subset_source(), "unsupported");
    let result = export_postcondition_certificate(&path, "app.t.f", 0, None, &short_limits());
    std::fs::remove_file(&path).ok();
    let diagnostics = result.expect_err("a call is outside the bounded subset");
    assert_eq!(diagnostics[0].code, "SPX-Z105");
    assert!(diagnostics[0]
        .message
        .contains("outside the bounded SMT-discharge subset"));
}

#[test]
fn export_rejects_an_out_of_range_ensures_index() {
    let path = write_temp(&true_postcondition_source(), "oob-index");
    let result = export_postcondition_certificate(&path, "app.t.f", 5, None, &short_limits());
    std::fs::remove_file(&path).ok();
    let diagnostics = result.expect_err("ensures_index 5 is out of range");
    assert_eq!(diagnostics[0].code, "SPX-Z105");
}

#[test]
fn export_rejects_an_unknown_declaration_id() {
    let path = write_temp(&true_postcondition_source(), "unknown-declaration");
    let result =
        export_postcondition_certificate(&path, "app.t.does_not_exist", 0, None, &short_limits());
    std::fs::remove_file(&path).ok();
    let diagnostics = result.expect_err("declaration does not exist");
    assert_eq!(diagnostics[0].code, "SPX-Z105");
}

// ---------------------------------------------------------------------
// Real end-to-end coverage against a provisioned Z3. Skipped by a bare
// `cargo test`; run explicitly with:
//   SEMAPRAX_SMT_Z3_PATH=/opt/homebrew/bin/z3 \
//     cargo test --locked -p semaprax --lib \
//     assurance_manifest::proof_certificate -- --ignored
// ---------------------------------------------------------------------

fn provisioned() -> Provisioning {
    provision_from_env().expect(
        "this test is #[ignore]d and must only be run with SEMAPRAX_SMT_Z3_PATH set to an \
         absolute path to a provisioned z3 binary",
    )
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_exports_a_proved_certificate_that_independently_verifies() {
    let provisioning = provisioned();
    let path = write_temp(&true_postcondition_source(), "z3-proved");
    let certificate =
        export_postcondition_certificate(&path, "app.t.f", 0, Some(&provisioning), &short_limits())
            .expect("a true postcondition must produce a certificate");

    verify_certificate(&certificate).expect("structural replay");
    verify_certificate_against_source(&certificate, &path).expect("source-bound replay");
    let with_solver_result =
        verify_certificate_with_solver(&certificate, &path, &provisioning, &short_limits());
    std::fs::remove_file(&path).ok();
    with_solver_result.expect("re-running the exact embedded script must also say unsat");
    assert!(certificate.contains("\"verdict\":\"proved\""));
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_exports_a_refuted_certificate_with_a_validated_counterexample() {
    let provisioning = provisioned();
    let path = write_temp(&false_postcondition_source(), "z3-refuted");
    let certificate =
        export_postcondition_certificate(&path, "app.t.f", 0, Some(&provisioning), &short_limits())
            .expect("a false postcondition must produce a refuted certificate");

    assert!(certificate.contains("\"verdict\":\"refuted\""));
    verify_certificate(&certificate).expect("structural replay");
    let against_source_result = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    against_source_result.expect("the recorded counterexample must independently replay");
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_exports_the_exact_i64_extremum_overflow_as_a_refuted_certificate() {
    let provisioning = provisioned();
    let path = write_temp(&extremum_overflow_source(), "z3-extremum");
    let certificate =
        export_postcondition_certificate(&path, "app.t.f", 0, Some(&provisioning), &short_limits())
            .expect("the exact i64::MAX extremum must produce a refuted certificate");

    assert!(certificate.contains("\"kind\":\"trapped\""));
    let result = verify_certificate_against_source(&certificate, &path);
    std::fs::remove_file(&path).ok();
    result.expect("the validated trap must independently replay");
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_certificate_export_is_byte_identical_across_repeated_runs() {
    let provisioning = provisioned();
    let path = write_temp(&true_postcondition_source(), "z3-determinism");
    let first =
        export_postcondition_certificate(&path, "app.t.f", 0, Some(&provisioning), &short_limits())
            .expect("first export");
    let second =
        export_postcondition_certificate(&path, "app.t.f", 0, Some(&provisioning), &short_limits())
            .expect("second export");
    std::fs::remove_file(&path).ok();
    assert_eq!(first, second, "repeated export must be byte-identical");
}

#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_SMT_Z3_PATH (z3)"]
fn provisioned_z3_verify_certificate_with_solver_rejects_a_falsely_claimed_proved_certificate() {
    // Demonstrates the property that matters most: a third party does not
    // need to trust this exporter. Here the certificate's own `verdict` is
    // a lie — the embedded script is the honest deterministic translation
    // of the exact bound source, but this refutable postcondition is not
    // actually unsat — and only a genuinely independent step (re-running
    // the exact embedded script through a real solver) can catch it.
    let provisioning = provisioned();
    let source = false_postcondition_source();
    let path = write_temp(&source, "z3-false-claim");
    let source_text = std::fs::read_to_string(&path).unwrap();
    let program = crate::parse(&source_text, &path).unwrap();
    let revision = crate::graph::revision(&program);
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.stable_id == "app.t.f")
        .unwrap()
        .clone();
    let encoding = translate_function(&function).unwrap();
    let script = render_postcondition_script(&encoding, 0, 5000);

    match run(&provisioning, &script, &short_limits()) {
        Verdict::Sat(_) => {}
        other => {
            std::fs::remove_file(&path).ok();
            panic!("expected this false postcondition to be sat, got {other:?}");
        }
    }

    let obligation_id = postcondition_obligation_id("app.t.f", 0);
    let source_sha256 = super::render::source_digest(&source_text);
    // The script is the honest translation of the real source, so
    // `verify_certificate_against_source` below reaches the artifact
    // recomputation check and expects it to succeed: the bound artifact
    // must be genuine, not a placeholder.
    let (artifact_bytes, artifact_sha256) = compute_wasm_artifact(&program);
    let certificate = hand_crafted_certificate(
        &path.display().to_string(),
        &revision,
        &source_sha256,
        "app.t.f",
        &obligation_id,
        0,
        env!("CARGO_PKG_VERSION"),
        &script,
        &super::render::script_digest(&script),
        "proved",
        "null",
        artifact_bytes,
        &artifact_sha256,
    );

    verify_certificate(&certificate).expect("internally consistent");
    verify_certificate_against_source(&certificate, &path)
        .expect("script is the honest translation of the unchanged source");

    let result =
        verify_certificate_with_solver(&certificate, &path, &provisioning, &short_limits());
    std::fs::remove_file(&path).ok();
    let error = result.expect_err(
        "a falsely claimed proved certificate must be rejected once a solver actually runs its \
         exact embedded script",
    );
    assert_eq!(error.code, "SPX-Z106");
}
