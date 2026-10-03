//! Independent replay of a `semaprax.lean-proof-certificate.v1`.
//!
//! Three layers, each usable on its own and each fail-closed:
//!
//! 1. [`verify_certificate`] — filesystem-free structural replay. Recomputes
//!    the envelope digest over the exact payload bytes, requires closed key
//!    sets and a closed verdict vocabulary, recomputes the embedded Lean
//!    document's digest, and requires every recorded axiom to be one of
//!    Lean's three standard axioms. A certificate recording `sorryAx` is
//!    rejected here, with no toolchain and no source in sight.
//! 2. [`verify_certificate_against_source`] — additionally re-parses and
//!    re-verifies the current source at a caller-supplied path, re-derives
//!    the semantic revision, **re-renders the Lean document from scratch**
//!    and requires byte equality, and recompiles the bound Wasm core module
//!    and requires digest equality. Any drift in source, revision, profile
//!    output, compiler version, or artifact fails closed.
//! 3. [`verify_certificate_with_capability`] — bindings first, then an
//!    explicitly supplied external kernel. This deliberately reuses
//!    [`crate::assurance_manifest::proof_certificate::ExternalKernelCapability`],
//!    the seam that module introduced *for this issue* rather than
//!    introducing a second one.
//! 4. [`super::program_root::verify_certificate_against_program_root`] —
//!    additionally replays a separate exact-certificate-to-retained-Project
//!    ProgramRoot association without reopening a raw source path. Its kernel
//!    variant preserves the same binding-first ordering.
//!
//! Re-rendering rather than trusting the certificate's own embedded bytes is
//! the whole point: a hand-edited Lean document that weakens a theorem
//! statement, or drops an obligation, no longer equals what the translator
//! deterministically produces from the bound source, so it is refused even
//! though its recorded verdict still says `kernel_checked`.

use std::path::Path;

use serde_json::Value;

use crate::assurance_manifest::proof_certificate::ExternalKernelCapability;
use crate::diagnostic::Diagnostic;
use crate::graph;

use super::certificate::{
    artifact_digest, lean_digest, payload_digest, source_digest, ARTIFACT_TARGET,
    CERTIFICATE_SCHEMA,
};
use super::kernel_report::{
    parse as parse_kernel_report, KernelVerdict, KERNEL_IDENTITY, PINNED_TOOLCHAIN, STANDARD_AXIOMS,
};
use super::lean::{export_module, EXPORT_SCHEMA, NAMESPACE};
use super::profile::PROFILE_V1;

/// Structural inconsistency inside the certificate itself.
fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z111", message)
}

/// The certificate is internally consistent but no longer matches the world
/// it binds to.
fn drift_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z112", message)
}

const PAYLOAD_KEYS: [&str; 19] = [
    "artifact",
    "assumptions",
    "compiler_version",
    "declaration_id",
    "ensures_index",
    "export_schema",
    "kernel",
    "lean_source",
    "lean_source_sha256",
    "module",
    "nonclaims",
    "obligation_id",
    "obligations",
    "profile",
    "schema",
    "source",
    "theorem_name",
    "unsupported",
    "verdict",
];

/// What a structurally valid certificate asserts, extracted once so the
/// source-binding and artifact-binding layers never re-parse.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedCertificate {
    pub declaration_id: String,
    pub obligation_id: String,
    pub theorem_name: String,
    pub ensures_index: usize,
    pub compiler_version: String,
    pub revision: String,
    pub source_sha256: String,
    pub lean_source: String,
    pub artifact_sha256: String,
    pub artifact_bytes: u64,
}

fn is_sha256_wire_form(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn object_keys(value: &Value, context: &str) -> Result<Vec<String>, Diagnostic> {
    value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .ok_or_else(|| consistency_error(format!("{context} must be a JSON object")))
}

fn require_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, Diagnostic> {
    value
        .as_str()
        .ok_or_else(|| consistency_error(format!("`{field}` must be a string")))
}

fn require_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, Diagnostic> {
    value
        .as_array()
        .ok_or_else(|| consistency_error(format!("`{field}` must be an array")))
}

fn check_exact_keys(
    mut found: Vec<String>,
    expected: &[&str],
    context: &str,
) -> Result<(), Diagnostic> {
    found.sort();
    let mut expected_sorted: Vec<&str> = expected.to_vec();
    expected_sorted.sort_unstable();
    if found
        .iter()
        .map(String::as_str)
        .ne(expected_sorted.iter().copied())
    {
        return Err(consistency_error(format!(
            "{context} keys must be exactly {expected_sorted:?}, found {found:?}"
        )));
    }
    Ok(())
}

fn check_obligations(
    obligations: &[Value],
    declaration_id: &str,
    theorem_name: &str,
    obligation_id: &str,
) -> Result<(), Diagnostic> {
    let mut headline_seen = false;
    for entry in obligations {
        check_exact_keys(
            object_keys(entry, "payload.obligations[]")?,
            &[
                "axioms",
                "declaration_id",
                "ensures_index",
                "kind",
                "obligation_id",
                "origin",
                "theorem_name",
            ],
            "payload.obligations[]",
        )?;
        let entry_declaration =
            require_string(&entry["declaration_id"], "obligation.declaration_id")?;
        let entry_theorem = require_string(&entry["theorem_name"], "obligation.theorem_name")?;
        if !entry_theorem.starts_with(&format!("{NAMESPACE}.")) {
            return Err(consistency_error(format!(
                "obligation theorem `{entry_theorem}` is not in the `{NAMESPACE}` namespace"
            )));
        }
        let kind = require_string(&entry["kind"], "obligation.kind")?;
        if !matches!(kind, "checked_arithmetic_range" | "postcondition") {
            return Err(consistency_error(format!(
                "obligation kind `{kind}` is outside the closed vocabulary"
            )));
        }
        // Every obligation belonging to the certified declaration must have
        // been kernel-confirmed: a postcondition whose enclosing function
        // has an unproved overflow obligation is not proved. Obligations of
        // *other* declarations in the same file may legitimately carry a
        // null axiom set when this certificate did not need them.
        if entry_declaration == declaration_id {
            let axioms = match &entry["axioms"] {
                Value::Null => {
                    return Err(consistency_error(format!(
                        "obligation `{entry_theorem}` belongs to the certified declaration but \
                         records no kernel axiom set"
                    )))
                }
                other => require_array(other, "obligation.axioms")?,
            };
            for axiom in axioms {
                let name = require_string(axiom, "obligation.axioms[]")?;
                if !STANDARD_AXIOMS.contains(&name) {
                    return Err(consistency_error(format!(
                        "obligation `{entry_theorem}` records non-standard axiom `{name}`"
                    )));
                }
            }
        }
        if entry_theorem == theorem_name {
            if require_string(&entry["obligation_id"], "obligation.obligation_id")? != obligation_id
            {
                return Err(consistency_error(
                    "the headline theorem's obligation id disagrees with the certificate's"
                        .to_owned(),
                ));
            }
            if entry_declaration != declaration_id {
                return Err(consistency_error(
                    "the headline theorem is recorded under a different declaration".to_owned(),
                ));
            }
            if kind != "postcondition" {
                return Err(consistency_error(
                    "a certificate's headline obligation must be a postcondition".to_owned(),
                ));
            }
            headline_seen = true;
        }
    }
    if !headline_seen {
        return Err(consistency_error(format!(
            "the certificate's `theorem_name` `{theorem_name}` is not among its obligations"
        )));
    }
    Ok(())
}

/// Filesystem-free structural replay. See the module doc.
pub fn verify_certificate(certificate: &str) -> Result<CheckedCertificate, Diagnostic> {
    let value: Value = serde_json::from_str(certificate)
        .map_err(|error| consistency_error(format!("certificate is not valid JSON: {error}")))?;
    check_exact_keys(
        object_keys(&value, "certificate")?,
        &["bytes", "digest", "payload", "schema"],
        "certificate",
    )?;
    if value["schema"].as_str() != Some(CERTIFICATE_SCHEMA) {
        return Err(consistency_error(format!(
            "certificate schema must be {CERTIFICATE_SCHEMA}"
        )));
    }
    let envelope_digest = require_string(&value["digest"], "digest")?;
    if !is_sha256_wire_form(envelope_digest) {
        return Err(consistency_error(
            "certificate digest must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }
    let declared_bytes = value["bytes"].as_u64().ok_or_else(|| {
        consistency_error("certificate `bytes` must be an unsigned integer".to_owned())
    })?;

    const PAYLOAD_KEY: &str = "\"payload\":";
    let offset = certificate
        .find(PAYLOAD_KEY)
        .ok_or_else(|| consistency_error("certificate is missing its payload member".to_owned()))?;
    if !certificate.ends_with('}') {
        return Err(consistency_error(
            "certificate must end with `}`".to_owned(),
        ));
    }
    let payload = &certificate[offset + PAYLOAD_KEY.len()..certificate.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(consistency_error(
            "certificate payload must be a JSON object".to_owned(),
        ));
    }
    if declared_bytes != payload.len() as u64 {
        return Err(consistency_error(format!(
            "certificate declares {declared_bytes} payload bytes but {} are present",
            payload.len()
        )));
    }
    if envelope_digest != payload_digest(payload.as_bytes()) {
        return Err(consistency_error(
            "certificate digest does not match the exact payload bytes".to_owned(),
        ));
    }

    let payload_value: Value = serde_json::from_str(payload)
        .map_err(|error| consistency_error(format!("payload is not valid JSON: {error}")))?;
    check_exact_keys(
        object_keys(&payload_value, "payload")?,
        &PAYLOAD_KEYS,
        "payload",
    )?;
    if payload_value["schema"].as_str() != Some(CERTIFICATE_SCHEMA) {
        return Err(consistency_error(format!(
            "payload schema must be {CERTIFICATE_SCHEMA}"
        )));
    }
    if payload_value["export_schema"].as_str() != Some(EXPORT_SCHEMA) {
        return Err(consistency_error(format!(
            "payload export_schema must be {EXPORT_SCHEMA}"
        )));
    }
    if payload_value["profile"].as_str() != Some(PROFILE_V1) {
        return Err(consistency_error(
            "payload profile does not match this compiler's Lean export profile".to_owned(),
        ));
    }
    if payload_value["verdict"].as_str() != Some("kernel_checked") {
        return Err(consistency_error(
            "the only verdict this schema admits is `kernel_checked`".to_owned(),
        ));
    }

    check_exact_keys(
        object_keys(&payload_value["source"], "payload.source")?,
        &["path", "revision", "sha256"],
        "payload.source",
    )?;
    require_string(&payload_value["source"]["path"], "source.path")?;
    let revision =
        require_string(&payload_value["source"]["revision"], "source.revision")?.to_owned();
    let source_sha256 =
        require_string(&payload_value["source"]["sha256"], "source.sha256")?.to_owned();
    if !is_sha256_wire_form(&source_sha256) {
        return Err(consistency_error(
            "source.sha256 must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }

    check_exact_keys(
        object_keys(&payload_value["kernel"], "payload.kernel")?,
        &["identity", "standard_axioms", "toolchain"],
        "payload.kernel",
    )?;
    if payload_value["kernel"]["identity"].as_str() != Some(KERNEL_IDENTITY) {
        return Err(consistency_error(format!(
            "kernel.identity must be `{KERNEL_IDENTITY}`"
        )));
    }
    if payload_value["kernel"]["toolchain"].as_str() != Some(PINNED_TOOLCHAIN) {
        return Err(drift_error(format!(
            "kernel.toolchain is not the pinned `{PINNED_TOOLCHAIN}`"
        )));
    }

    check_exact_keys(
        object_keys(&payload_value["artifact"], "payload.artifact")?,
        &["bytes", "sha256", "target"],
        "payload.artifact",
    )?;
    if payload_value["artifact"]["target"].as_str() != Some(ARTIFACT_TARGET) {
        return Err(consistency_error(format!(
            "artifact.target must be `{ARTIFACT_TARGET}`"
        )));
    }
    let artifact_sha256 =
        require_string(&payload_value["artifact"]["sha256"], "artifact.sha256")?.to_owned();
    if !is_sha256_wire_form(&artifact_sha256) {
        return Err(consistency_error(
            "artifact.sha256 must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }
    let artifact_bytes = payload_value["artifact"]["bytes"].as_u64().ok_or_else(|| {
        consistency_error("artifact.bytes must be an unsigned integer".to_owned())
    })?;

    let lean_source = require_string(&payload_value["lean_source"], "lean_source")?.to_owned();
    let recorded_lean_digest =
        require_string(&payload_value["lean_source_sha256"], "lean_source_sha256")?;
    if recorded_lean_digest != lean_digest(&lean_source) {
        return Err(consistency_error(
            "lean_source_sha256 does not match the embedded Lean document".to_owned(),
        ));
    }

    let declaration_id =
        require_string(&payload_value["declaration_id"], "declaration_id")?.to_owned();
    let obligation_id =
        require_string(&payload_value["obligation_id"], "obligation_id")?.to_owned();
    let theorem_name = require_string(&payload_value["theorem_name"], "theorem_name")?.to_owned();
    let ensures_index = payload_value["ensures_index"]
        .as_u64()
        .ok_or_else(|| consistency_error("ensures_index must be an unsigned integer".to_owned()))?
        as usize;
    let compiler_version =
        require_string(&payload_value["compiler_version"], "compiler_version")?.to_owned();

    require_array(&payload_value["assumptions"], "assumptions")?;
    require_array(&payload_value["unsupported"], "unsupported")?;
    require_array(&payload_value["nonclaims"], "nonclaims")?;
    let obligations = require_array(&payload_value["obligations"], "obligations")?;
    check_obligations(obligations, &declaration_id, &theorem_name, &obligation_id)?;

    Ok(CheckedCertificate {
        declaration_id,
        obligation_id,
        theorem_name,
        ensures_index,
        compiler_version,
        revision,
        source_sha256,
        lean_source,
        artifact_sha256,
        artifact_bytes,
    })
}

/// Extract the payload facts which remain private implementation detail of a
/// live kernel recheck. [`verify_certificate`] runs first, so this helper
/// never turns permissive JSON parsing into an admission path.
fn recheck_payload_facts(
    certificate: &str,
    declaration_id: &str,
) -> Result<(String, Vec<(String, Vec<String>)>), Diagnostic> {
    let value: Value = serde_json::from_str(certificate)
        .map_err(|error| consistency_error(format!("certificate is not valid JSON: {error}")))?;
    let payload = &value["payload"];
    let module = require_string(&payload["module"], "payload.module")?.to_owned();
    let obligations = require_array(&payload["obligations"], "payload.obligations")?;
    let mut axioms = Vec::new();
    for obligation in obligations {
        if require_string(&obligation["declaration_id"], "obligation.declaration_id")?
            != declaration_id
        {
            continue;
        }
        let name = require_string(&obligation["theorem_name"], "obligation.theorem_name")?;
        let recorded = require_array(&obligation["axioms"], "obligation.axioms")?
            .iter()
            .map(|axiom| require_string(axiom, "obligation.axioms[]").map(str::to_owned))
            .collect::<Result<Vec<_>, _>>()?;
        axioms.push((name.to_owned(), recorded));
    }
    Ok((module, axioms))
}

/// Rebind a certificate against already-held exact source bytes. Kept
/// crate-visible so a Project/ProgramRoot association can replay the same
/// source, compiler and artifact chain without re-opening a raw path.
pub(crate) fn rebind_certificate_against_source_text(
    certificate: &str,
    source: &str,
    source_path: &Path,
) -> Result<
    (
        CheckedCertificate,
        super::ModuleExport,
        Vec<(String, Vec<String>)>,
    ),
    Diagnostic,
> {
    let checked = verify_certificate(certificate)?;
    let (recorded_module, recorded_axioms) =
        recheck_payload_facts(certificate, &checked.declaration_id)?;
    if checked.compiler_version != env!("CARGO_PKG_VERSION") {
        return Err(drift_error(format!(
            "certificate was produced by compiler {} but this is {}",
            checked.compiler_version,
            env!("CARGO_PKG_VERSION")
        )));
    }
    if source_digest(source) != checked.source_sha256 {
        return Err(drift_error(
            "current source bytes do not match the certificate's `source.sha256`".to_owned(),
        ));
    }
    let program = crate::parse(source, source_path).map_err(|error| {
        drift_error(format!(
            "the bound source no longer parses: {}",
            error.message
        ))
    })?;
    let diagnostics = crate::verify::verify(&program);
    if let Some(first) = diagnostics.iter().find(|item| item.severity.is_error()) {
        return Err(drift_error(format!(
            "the bound source no longer verifies: {}",
            first.message
        )));
    }
    let revision = graph::revision(&program);
    if revision != checked.revision {
        return Err(drift_error(
            "the bound source's semantic revision no longer matches the certificate".to_owned(),
        ));
    }
    let export = export_module(&program, &revision);
    if recorded_module != export.module {
        return Err(drift_error(
            "the bound source's module name does not match the certificate".to_owned(),
        ));
    }
    let function = export
        .exported
        .iter()
        .find(|function| function.declaration_id == checked.declaration_id)
        .ok_or_else(|| {
            drift_error(
                "the certificate's declaration_id is not an exported declaration of the bound source"
                    .to_owned(),
            )
        })?;
    let source_function = program
        .functions
        .iter()
        .find(|item| item.stable_id == checked.declaration_id)
        .expect("exported declaration is present in parsed source");
    if crate::assurance_manifest::smt_discharge::bounded_domain_witness(source_function, 256)
        .is_none()
    {
        return Err(drift_error(
            "the bound source has no bounded checked precondition witness; domain status is unknown"
                .to_owned(),
        ));
    }
    let obligation = function
        .obligations
        .iter()
        .find(|obligation| obligation.ensures_index == Some(checked.ensures_index))
        .ok_or_else(|| {
            drift_error(
                "the certificate's ensures_index is not a postcondition of its bound declaration"
                    .to_owned(),
            )
        })?;
    if checked.obligation_id != obligation.obligation_id
        || checked.theorem_name != format!("{NAMESPACE}.{}", obligation.theorem_name)
    {
        return Err(drift_error(
            "the certificate's selected obligation identity does not match the bound source"
                .to_owned(),
        ));
    }
    let expected_axiom_names = function
        .obligations
        .iter()
        .map(|obligation| format!("{NAMESPACE}.{}", obligation.theorem_name))
        .collect::<Vec<_>>();
    if recorded_axioms
        .iter()
        .map(|(name, _)| name)
        .ne(expected_axiom_names.iter())
    {
        return Err(drift_error(
            "the certificate does not record the complete canonical obligation inventory for \
             its bound declaration"
                .to_owned(),
        ));
    }
    if export.lean_source != checked.lean_source {
        return Err(drift_error(
            "re-rendering the Lean document from the bound source does not reproduce the \
             certificate's embedded bytes"
                .to_owned(),
        ));
    }
    let resolved = crate::hir::resolve(&program).map_err(|diagnostics| {
        drift_error(format!(
            "the bound source no longer resolves: {}",
            diagnostics
                .first()
                .map_or_else(|| "unknown".to_owned(), |item| item.message.clone())
        ))
    })?;
    let artifact = super::compile_wasm_core_module(&resolved).map_err(drift_error)?;
    if artifact_digest(&artifact) != checked.artifact_sha256
        || artifact.len() as u64 != checked.artifact_bytes
    {
        return Err(drift_error(
            "recompiling the bound source does not reproduce the certificate's bound artifact"
                .to_owned(),
        ));
    }
    Ok((checked, export, recorded_axioms))
}

fn rebind_certificate_against_source(
    certificate: &str,
    source_path: &Path,
) -> Result<
    (
        CheckedCertificate,
        super::ModuleExport,
        Vec<(String, Vec<String>)>,
    ),
    Diagnostic,
> {
    let source = std::fs::read_to_string(source_path).map_err(|error| {
        drift_error(format!(
            "cannot read `{}` to rebind this certificate: {error}",
            source_path.display()
        ))
    })?;
    rebind_certificate_against_source_text(certificate, &source, source_path)
}

/// Everything [`verify_certificate`] checks, plus rebinding to the current
/// bytes at `source_path`. Fails closed on any drift.
pub fn verify_certificate_against_source(
    certificate: &str,
    source_path: &Path,
) -> Result<CheckedCertificate, Diagnostic> {
    rebind_certificate_against_source(certificate, source_path).map(|(checked, _, _)| checked)
}

/// Confirm that artifact bytes a caller already holds are the ones this
/// certificate binds. Compiler-free and filesystem-free: it says nothing
/// about where those bytes came from, only that they are the bound ones.
pub fn verify_certificate_against_artifact(
    certificate: &str,
    artifact_bytes: &[u8],
) -> Result<CheckedCertificate, Diagnostic> {
    let checked = verify_certificate(certificate)?;
    if artifact_digest(artifact_bytes) != checked.artifact_sha256
        || artifact_bytes.len() as u64 != checked.artifact_bytes
    {
        return Err(drift_error(
            "the supplied artifact bytes do not match this certificate's `artifact` binding"
                .to_owned(),
        ));
    }
    Ok(checked)
}

/// Bindings first, then an explicitly supplied external kernel — the exact
/// ordering guarantee
/// [`crate::assurance_manifest::proof_certificate::verify_certificate_with_capability`]
/// established, reused here rather than restated. A capability that always
/// confirms can never widen what the binding layer already refused, because
/// it is consulted only after every binding check has passed.
pub fn verify_certificate_with_capability(
    certificate: &str,
    source_path: &Path,
    capability: &dyn ExternalKernelCapability,
) -> Result<CheckedCertificate, Diagnostic> {
    let checked = verify_certificate_against_source(certificate, source_path)?;
    capability.confirm(&checked.lean_source)?;
    Ok(checked)
}

/// Rebind a certificate first, then have an explicitly supplied Lean kernel
/// check the exact re-rendered document and reproduce its recorded axiom
/// results. Unlike the generic capability seam, this adapter knows Lean's
/// closed report grammar and therefore refuses a substituted clean result.
///
/// The caller supplies all external authority through [`super::LeanKernel`].
/// This function performs no process, network, or tool discovery; source and
/// artifact reads are the same ones required by source-bound replay.
pub fn verify_certificate_with_kernel(
    certificate: &str,
    source_path: &Path,
    kernel: &dyn super::LeanKernel,
) -> Result<CheckedCertificate, Diagnostic> {
    let (checked, export, recorded_axioms) =
        rebind_certificate_against_source(certificate, source_path)?;
    verify_kernel_replay(checked, export, recorded_axioms, kernel)
}

/// Check an already rebound export with a supplied kernel. All byte, source,
/// semantic-revision and artifact binding must have completed before this
/// helper is reachable.
pub(crate) fn verify_kernel_replay(
    checked: CheckedCertificate,
    export: super::ModuleExport,
    recorded_axioms: Vec<(String, Vec<String>)>,
    kernel: &dyn super::LeanKernel,
) -> Result<CheckedCertificate, Diagnostic> {
    let run = kernel.check(&export.lean_source)?;
    let axioms = match parse_kernel_report(&export.theorem_names(), &run.toolchain, &run.output) {
        KernelVerdict::Checked { axioms } => axioms,
        KernelVerdict::Rejected(rejection) => {
            return Err(Diagnostic::io(
                "SPX-Z110",
                format!(
                    "the pinned Lean kernel did not accept this recheck ({}): {}",
                    rejection.code(),
                    rejection.detail()
                ),
            ))
        }
    };
    let observed: Vec<(String, Vec<String>)> = axioms
        .into_iter()
        .filter(|(name, _)| recorded_axioms.iter().any(|(recorded, _)| recorded == name))
        .collect();
    if observed != recorded_axioms {
        return Err(drift_error(
            "the external Lean kernel's axiom results do not reproduce the certificate's exact \
             recorded obligations"
                .to_owned(),
        ));
    }
    Ok(checked)
}
