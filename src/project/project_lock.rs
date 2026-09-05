//! Project Lock v1: the deterministic `semaprax.lock` of one authenticated
//! project.
//!
//! The lock binds the facts a consumer needs to recognize the same package: the
//! canonical manifest and the frozen contract it lowers to, the project
//! revision (the program root), every source file's revision and digest, the
//! retained interface descriptor digest where the profile has one, the declared
//! target matrix, the required capabilities, the compiler, and the resolution
//! policy. Rendering is a pure function of the authenticated snapshot, and
//! verification re-renders and compares bytes, so any source, manifest, or
//! compiler drift fails closed. The lock carries no authority; like every other
//! package operation in this repository it is produced only by an explicit
//! command, never as an implicit effect of `check`.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    ProjectProfile, ProjectSnapshot, PACKAGE_MANIFEST_SCHEMA, PACKAGE_TARGET_NATIVE64,
    PACKAGE_TARGET_WASM32,
};
use crate::diagnostic::Diagnostic;

/// The lock envelope schema.
pub const PROJECT_LOCK_SCHEMA: &str = "semaprax.project-lock.v1";
/// The file name beside `semaprax.toml`.
pub const PROJECT_LOCK_FILE: &str = "semaprax.lock";
/// Upper bound on the bytes a lock file may hold before it is rejected unread.
pub const MAX_PROJECT_LOCK_BYTES: usize = 1024 * 1024;
const DIGEST_DOMAIN: &[u8] = b"semaprax.project-lock.v1\0";
const MANIFEST_DIGEST_DOMAIN: &[u8] = b"semaprax.project-lock.manifest.v1\0";
const CODE_STALE: &str = "SPX-J123";
const CODE_FOREIGN: &str = "SPX-J124";
const NONCLAIMS: [&str; 4] = [
    "no_dependency_resolution_acquisition_registry_or_cache",
    "no_effect_license_sbom_provenance_or_signature_facts",
    "no_target_execution_or_availability_proof",
    "no_filesystem_process_or_network_authority",
];

/// The digest and program root of a lock that verified against a snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedProjectLock {
    digest: String,
    program_root: String,
}

impl VerifiedProjectLock {
    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn program_root(&self) -> &str {
        &self.program_root
    }
}

/// Render the canonical lock for one checked snapshot. Two renders of the same
/// snapshot are byte-identical; every object's keys are in byte order.
pub fn render_project_lock(snapshot: &ProjectSnapshot) -> Result<String, Vec<Diagnostic>> {
    let manifest = snapshot.manifest();
    let (interface_kind, interface_digest) = match manifest.project_profile() {
        ProjectProfile::ScalarV1 => (
            "scalar-wit.v1",
            Some(snapshot.scalar_wit_interface_v1()?.wit_digest().to_owned()),
        ),
        ProjectProfile::OwnedDataApiV1 => (
            "public-owned-data-api.v1",
            Some(snapshot.public_api_descriptor()?.digest()),
        ),
        ProjectProfile::FlatOwnedRecordApiV1 => (
            "flat-owned-record-api.v1",
            Some(snapshot.flat_owned_record_api_descriptor()?.digest()),
        ),
        ProjectProfile::OwnedUtf8ApiV1 => (
            "owned-utf8-api.v1",
            Some(snapshot.owned_utf8_api_descriptor()?.digest()),
        ),
        ProjectProfile::NestedOwnedRecordApiV1 => (
            "nested-owned-record-api.v1",
            Some(snapshot.nested_owned_record_api_descriptor()?.digest()),
        ),
        ProjectProfile::UsefulTextConsumerV1
        | ProjectProfile::UsefulDataV1
        | ProjectProfile::UsefulDataCommandV1
        | ProjectProfile::UsefulDataCommandV2
        | ProjectProfile::LanguageCommandIoV1
        | ProjectProfile::LineCommandIoV1
        | ProjectProfile::NetworkCommandIoV1 => ("unproven", None),
    };
    let default_targets = [
        PACKAGE_TARGET_NATIVE64.to_owned(),
        PACKAGE_TARGET_WASM32.to_owned(),
    ];
    let (target_state, targets) = match manifest.target_matrix() {
        Some(matrix) => ("declared", matrix),
        None => ("default", &default_targets[..]),
    };
    let canonical_manifest = manifest.to_canonical_toml();
    let payload = json!({
        "schema": PROJECT_LOCK_SCHEMA,
        "package": {
            "name": manifest.name(),
            "version": manifest.package_version(),
            "manifest_schema": manifest.manifest_schema(),
            "contract": manifest.schema(),
            "profile": manifest.profile().unwrap_or("scalar"),
            "manifest_digest": framed_digest(MANIFEST_DIGEST_DOMAIN, canonical_manifest.as_bytes()),
        },
        "program_root": snapshot.project_revision(),
        "source": {
            "workspace_revision": snapshot.workspace_revision(),
            "files": snapshot.sources().iter().map(|source| json!({
                "path": source.path(),
                "source_revision": source.source_revision(),
                "source_digest": source.source_digest(),
            })).collect::<Vec<_>>(),
        },
        "interface": {
            "kind": interface_kind,
            "digest": interface_digest,
            "exports": manifest.web_exports(),
        },
        "dependencies": manifest.dependencies().iter().map(|dependency| json!({
            "name": dependency.name(),
            "range": dependency.range(),
        })).collect::<Vec<_>>(),
        "targets": targets.iter().map(|target| json!({
            "target": target,
            "state": target_state,
        })).collect::<Vec<_>>(),
        "capabilities": manifest.capabilities(),
        "compiler": {
            "package": env!("CARGO_PKG_NAME"),
            "version": env!("CARGO_PKG_VERSION"),
            "lock_compatibility": PROJECT_LOCK_SCHEMA,
            "manifest_layouts": [PACKAGE_MANIFEST_SCHEMA, "semaprax.project.v1-v12"],
        },
        "resolution_policy": {
            "dependencies": "none",
            "range_grammar": "exact-tilde-caret.v1",
            "registry": "none",
            "cache": "none",
        },
        "nonclaims": NONCLAIMS,
    });
    let payload = serde_json::to_string(&payload).map_err(|error| {
        vec![Diagnostic::io(
            CODE_FOREIGN,
            format!("Project Lock v1 payload could not be rendered: {error}"),
        )]
    })?;
    let digest = framed_digest(DIGEST_DOMAIN, payload.as_bytes());
    let lock = format!(
        "{{\"bytes\":{},\"digest\":{},\"payload\":{payload},\"schema\":{}}}\n",
        payload.len(),
        serde_json::to_string(&digest).expect("digest text is JSON"),
        serde_json::to_string(PROJECT_LOCK_SCHEMA).expect("schema text is JSON"),
    );
    if lock.len() > MAX_PROJECT_LOCK_BYTES {
        return Err(vec![Diagnostic::io(
            CODE_FOREIGN,
            format!("Project Lock v1 exceeds {MAX_PROJECT_LOCK_BYTES} bytes"),
        )]);
    }
    Ok(lock)
}

/// Verify caller-supplied lock bytes against one checked snapshot. A lock that
/// is not a readable Project Lock v1 rejects with `SPX-J124`; a readable lock
/// whose bytes differ from the fresh rendering rejects with `SPX-J123` and
/// names the payload fields that drifted.
pub fn verify_project_lock(
    snapshot: &ProjectSnapshot,
    lock: &str,
) -> Result<VerifiedProjectLock, Vec<Diagnostic>> {
    if lock.len() > MAX_PROJECT_LOCK_BYTES {
        return Err(foreign(format!(
            "{PROJECT_LOCK_FILE} exceeds {MAX_PROJECT_LOCK_BYTES} bytes"
        )));
    }
    let submitted: Value = serde_json::from_str(lock)
        .map_err(|_| foreign(format!("{PROJECT_LOCK_FILE} is not a JSON object")))?;
    if submitted.get("schema").and_then(Value::as_str) != Some(PROJECT_LOCK_SCHEMA) {
        return Err(foreign(format!(
            "{PROJECT_LOCK_FILE} does not carry schema {PROJECT_LOCK_SCHEMA}"
        )));
    }
    let expected = render_project_lock(snapshot)?;
    let expected_value: Value = serde_json::from_str(&expected).expect("rendered lock is JSON");
    if expected == lock {
        return Ok(VerifiedProjectLock {
            digest: expected_value["digest"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            program_root: expected_value["payload"]["program_root"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        });
    }
    let mut drifted = Vec::new();
    if let (Some(expected), Some(submitted)) = (
        expected_value["payload"].as_object(),
        submitted["payload"].as_object(),
    ) {
        for (key, value) in expected {
            if submitted.get(key) != Some(value) {
                drifted.push(key.as_str());
            }
        }
        for key in submitted.keys() {
            if !expected.contains_key(key) {
                drifted.push(key.as_str());
            }
        }
    } else {
        drifted.push("payload");
    }
    if drifted.is_empty() {
        drifted.push("envelope");
    }
    Err(vec![Diagnostic::io(
        CODE_STALE,
        format!(
            "{PROJECT_LOCK_FILE} is stale: {} {} from the checked project",
            drifted.join(", "),
            if drifted.len() == 1 { "differs" } else { "differ" }
        ),
    )
    .with_help(
        "run `semaprax lock semaprax.toml --write` to re-lock the current sources, or restore the sources the lock describes",
    )])
}

/// The classification of a candidate project's interface against a baseline
/// lock. `breaking` is true when any change strengthens a consumer's
/// obligations or removes something it may depend on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockCompatibility {
    breaking: bool,
    report: String,
}

impl LockCompatibility {
    pub fn breaking(&self) -> bool {
        self.breaking
    }

    pub fn report(&self) -> &str {
        &self.report
    }
}

/// Classify the change from a `baseline` Project Lock v1 to a `candidate` one.
///
/// This is a coarse, digest-level compatibility verdict over the facts the lock
/// records: the package identity and contract, the exported stable-id set and
/// the retained interface descriptor digest, the required capabilities, and the
/// target set. A changed interface digest with an unchanged export set means an
/// existing export's types, ownership, or contracts changed, which is breaking;
/// a pure display rename does not change the digest, so it is not breaking. The
/// fine-grained per-export classification is the separate offline
/// [Compatibility Evidence v1] over Report-v2 subjects; this verdict does not
/// replace it.
pub fn classify_lock_change(
    baseline: &str,
    candidate: &str,
) -> Result<LockCompatibility, Vec<Diagnostic>> {
    let base = parse_lock_payload(baseline, "baseline")?;
    let head = parse_lock_payload(candidate, "candidate")?;
    let mut changes: Vec<(&str, &str, String)> = Vec::new();
    let mut breaking = false;
    let mut note = |kind: &'static str, classification: &'static str, detail: String| {
        if classification == "breaking" {
            breaking = true;
        }
        changes.push((kind, classification, detail));
    };

    let base_name = string_at(&base, "package", "name");
    let head_name = string_at(&head, "package", "name");
    if base_name != head_name {
        note(
            "package-name",
            "breaking",
            format!("{base_name} became {head_name}"),
        );
    }
    let base_contract = string_at(&base, "package", "contract");
    let head_contract = string_at(&head, "package", "contract");
    if base_contract != head_contract {
        note(
            "contract",
            "breaking",
            format!("{base_contract} became {head_contract}"),
        );
    }

    let base_exports = string_set(&base["interface"]["exports"]);
    let head_exports = string_set(&head["interface"]["exports"]);
    let removed: Vec<&String> = base_exports
        .iter()
        .filter(|id| !head_exports.contains(*id))
        .collect();
    let added: Vec<&String> = head_exports
        .iter()
        .filter(|id| !base_exports.contains(*id))
        .collect();
    if !removed.is_empty() {
        note("exports-removed", "breaking", join(&removed));
    }
    if !added.is_empty() {
        note("exports-added", "nonbreaking", join(&added));
    }
    let base_interface = base["interface"]["digest"].as_str().unwrap_or("");
    let head_interface = head["interface"]["digest"].as_str().unwrap_or("");
    if base_interface != head_interface && removed.is_empty() && added.is_empty() {
        note(
            "interface-digest",
            "breaking",
            "a retained export changed its types, ownership, or contracts".to_owned(),
        );
    }

    let base_caps = string_set(&base["capabilities"]);
    let head_caps = string_set(&head["capabilities"]);
    let widened: Vec<&String> = head_caps
        .iter()
        .filter(|id| !base_caps.contains(*id))
        .collect();
    let dropped: Vec<&String> = base_caps
        .iter()
        .filter(|id| !head_caps.contains(*id))
        .collect();
    if !widened.is_empty() {
        note("capabilities-widened", "breaking", join(&widened));
    }
    if !dropped.is_empty() {
        note("capabilities-narrowed", "nonbreaking", join(&dropped));
    }

    let base_targets = target_set(&base);
    let head_targets = target_set(&head);
    let targets_removed: Vec<&String> = base_targets
        .iter()
        .filter(|t| !head_targets.contains(*t))
        .collect();
    let targets_added: Vec<&String> = head_targets
        .iter()
        .filter(|t| !base_targets.contains(*t))
        .collect();
    if !targets_removed.is_empty() {
        note("targets-removed", "breaking", join(&targets_removed));
    }
    if !targets_added.is_empty() {
        note("targets-added", "nonbreaking", join(&targets_added));
    }

    let base_version = base["package"]["version"].as_str().unwrap_or("none");
    let head_version = head["package"]["version"].as_str().unwrap_or("none");
    if base_version != head_version {
        note(
            "version",
            "informational",
            format!("{base_version} became {head_version}"),
        );
    }

    let verdict = if breaking { "breaking" } else { "compatible" };
    let change_rows = changes
        .iter()
        .map(|(kind, classification, detail)| {
            format!(
                "{{\"classification\":{},\"detail\":{},\"kind\":{}}}",
                json_string(classification),
                json_string(detail),
                json_string(kind),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let report = format!(
        "{{\"baseline\":{},\"candidate\":{},\"changes\":[{change_rows}],\"schema\":{},\"verdict\":{}}}\n",
        json_string(base_interface),
        json_string(head_interface),
        json_string("semaprax.project-lock-compatibility.v1"),
        json_string(verdict),
    );
    Ok(LockCompatibility { breaking, report })
}

fn parse_lock_payload(lock: &str, role: &str) -> Result<Value, Vec<Diagnostic>> {
    if lock.len() > MAX_PROJECT_LOCK_BYTES {
        return Err(foreign(format!(
            "{role} lock exceeds {MAX_PROJECT_LOCK_BYTES} bytes"
        )));
    }
    let value: Value = serde_json::from_str(lock)
        .map_err(|_| foreign(format!("{role} lock is not a JSON object")))?;
    if value.get("schema").and_then(Value::as_str) != Some(PROJECT_LOCK_SCHEMA) {
        return Err(foreign(format!(
            "{role} lock does not carry schema {PROJECT_LOCK_SCHEMA}"
        )));
    }
    let payload = value.get("payload").cloned().unwrap_or(Value::Null);
    if !payload.is_object() {
        return Err(foreign(format!("{role} lock has no payload object")));
    }
    Ok(payload)
}

fn string_at(payload: &Value, table: &str, key: &str) -> String {
    payload[table][key].as_str().unwrap_or("").to_owned()
}

fn string_set(value: &Value) -> std::collections::BTreeSet<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.as_str().map(str::to_owned))
        .collect()
}

fn target_set(payload: &Value) -> std::collections::BTreeSet<String> {
    payload["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row["target"].as_str().map(str::to_owned))
        .collect()
}

fn join(values: &[&String]) -> String {
    values
        .iter()
        .map(|value| value.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).expect("string is JSON")
}

fn foreign(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::io(CODE_FOREIGN, message)]
}

fn framed_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}
