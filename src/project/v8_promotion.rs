//! Authority-free Project v8 promotion observations and exact replay.
//!
//! A receipt records caller-owned evidence facts. It does not run a gate,
//! authenticate a host, approve support, or grant publication authority.

use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};

pub const PROJECT_V8_PROMOTION_RECEIPT_SCHEMA: &str = "semaprax.project-v8-promotion-receipt.v1";
pub const MAX_PROJECT_V8_PROMOTION_RECEIPT_BYTES: usize = 1024 * 1024;

const PROJECT_SCHEMA: &str = "semaprax.project.v8";
const PROFILE: &str = "owned-data-api.v1";
const DIGEST_DOMAIN: &[u8] = b"semaprax.project-v8-promotion-receipt.v1\0";
const EXPORTS: [&str; 3] = [
    "frame.payload",
    "frame.payload-maybe",
    "frame.payload-result",
];
const ARTIFACT_IDS: [&str; 8] = [
    "baseline.descriptor",
    "baseline.npm-carrier",
    "baseline.rust-package",
    "display-rename.descriptor",
    "display-rename.npm-carrier",
    "display-rename.rust-package",
    "browser-toolchain-lock",
    "compatibility-kat-inventory",
];

#[derive(Clone, Copy)]
struct RequiredGate {
    gate_id: &'static str,
    platform_id: &'static str,
    tool_profile_id: &'static str,
}

const REQUIRED_GATES: [RequiredGate; 15] = [
    RequiredGate {
        gate_id: "manifest-descriptor-compatibility",
        platform_id: "linux-x86_64",
        tool_profile_id: "rust-1.88",
    },
    RequiredGate {
        gate_id: "manifest-descriptor-compatibility",
        platform_id: "macos-aarch64",
        tool_profile_id: "rust-1.88",
    },
    RequiredGate {
        gate_id: "manifest-descriptor-compatibility",
        platform_id: "windows-x86_64",
        tool_profile_id: "rust-1.88",
    },
    RequiredGate {
        gate_id: "npm-installed-consumer",
        platform_id: "linux-x86_64",
        tool_profile_id: "node-22-typescript-5.8.3",
    },
    RequiredGate {
        gate_id: "npm-installed-consumer",
        platform_id: "macos-aarch64",
        tool_profile_id: "node-22-typescript-5.8.3",
    },
    RequiredGate {
        gate_id: "npm-installed-consumer",
        platform_id: "windows-x86_64",
        tool_profile_id: "node-22-typescript-5.8.3",
    },
    RequiredGate {
        gate_id: "rust-sdk-consumer",
        platform_id: "linux-x86_64",
        tool_profile_id: "rust-1.88",
    },
    RequiredGate {
        gate_id: "rust-sdk-consumer",
        platform_id: "macos-aarch64",
        tool_profile_id: "rust-1.88",
    },
    RequiredGate {
        gate_id: "rust-sdk-consumer",
        platform_id: "windows-x86_64",
        tool_profile_id: "rust-1.88",
    },
    RequiredGate {
        gate_id: "backend-equivalence-o0-o2",
        platform_id: "linux-x86_64",
        tool_profile_id: "clang-native-core-wasm",
    },
    RequiredGate {
        gate_id: "browser-consumer",
        platform_id: "linux-x86_64",
        tool_profile_id: "playwright-1.55-chromium",
    },
    RequiredGate {
        gate_id: "browser-consumer",
        platform_id: "linux-x86_64",
        tool_profile_id: "playwright-1.55-firefox",
    },
    RequiredGate {
        gate_id: "browser-consumer",
        platform_id: "linux-x86_64",
        tool_profile_id: "playwright-1.55-webkit",
    },
    RequiredGate {
        gate_id: "asan-ubsan",
        platform_id: "linux-x86_64",
        tool_profile_id: "clang-address-undefined",
    },
    RequiredGate {
        gate_id: "hostile-carrier-settlement",
        platform_id: "linux-x86_64",
        tool_profile_id: "rust-1.88-clang",
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectV8PromotionSubject {
    pub label: String,
    pub project_revision: String,
    pub workspace_revision: String,
    pub project_graph_digest: String,
    pub descriptor_digest: String,
    pub npm_carrier_digest: String,
    pub rust_package_digest: String,
    pub export_stable_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectV8PromotionArtifact {
    pub artifact_id: String,
    pub digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectV8PromotionGateOutcome {
    Passed,
    Failed,
    Skipped,
    Masked,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectV8PromotionGateObservation {
    pub gate_id: String,
    pub platform_id: String,
    pub tool_profile_id: String,
    pub project_schema: String,
    pub profile: String,
    pub commit: String,
    pub outcome: ProjectV8PromotionGateOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Subjects {
    baseline: ProjectV8PromotionSubject,
    display_rename: ProjectV8PromotionSubject,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GateRow {
    gate_id: String,
    platform_id: String,
    tool_profile_id: String,
    project_schema: String,
    profile: String,
    commit: String,
    outcome: ProjectV8PromotionGateOutcome,
    artifact_inventory_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WireReceipt {
    schema: String,
    commit: String,
    project_schema: String,
    profile: String,
    subjects: Subjects,
    artifacts: Vec<ProjectV8PromotionArtifact>,
    gates: Vec<GateRow>,
    nonclaims: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectV8PromotionReceipt {
    wire: WireReceipt,
    canonical: String,
    digest: String,
}

impl ProjectV8PromotionReceipt {
    pub fn canonical_json(&self) -> &str {
        &self.canonical
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn commit(&self) -> &str {
        &self.wire.commit
    }

    pub fn gates(&self) -> usize {
        self.wire.gates.len()
    }
}

pub fn derive_project_v8_promotion_receipt(
    commit: &str,
    baseline: ProjectV8PromotionSubject,
    display_rename: ProjectV8PromotionSubject,
    artifacts: Vec<ProjectV8PromotionArtifact>,
    observations: Vec<ProjectV8PromotionGateObservation>,
) -> Result<ProjectV8PromotionReceipt, Vec<Diagnostic>> {
    validate_commit(commit)?;
    validate_subjects(&baseline, &display_rename)?;
    validate_artifacts(&artifacts, &baseline, &display_rename)?;
    validate_observations(commit, &observations)?;
    let artifact_inventory_digest = artifact_inventory_digest(&artifacts)?;
    let gates = observations
        .into_iter()
        .map(|row| GateRow {
            gate_id: row.gate_id,
            platform_id: row.platform_id,
            tool_profile_id: row.tool_profile_id,
            project_schema: row.project_schema,
            profile: row.profile,
            commit: row.commit,
            outcome: row.outcome,
            artifact_inventory_digest: artifact_inventory_digest.clone(),
        })
        .collect();
    finish(WireReceipt {
        schema: PROJECT_V8_PROMOTION_RECEIPT_SCHEMA.to_owned(),
        commit: commit.to_owned(),
        project_schema: PROJECT_SCHEMA.to_owned(),
        profile: PROFILE.to_owned(),
        subjects: Subjects {
            baseline,
            display_rename,
        },
        artifacts,
        gates,
        nonclaims: nonclaims(),
    })
}

pub fn parse_project_v8_promotion_receipt(
    bytes: &str,
) -> Result<ProjectV8PromotionReceipt, Vec<Diagnostic>> {
    if bytes.is_empty()
        || bytes.len() > MAX_PROJECT_V8_PROMOTION_RECEIPT_BYTES
        || !bytes.ends_with('\n')
        || bytes[..bytes.len() - 1].contains(['\n', '\r', '\0'])
    {
        return Err(invalid("Project v8 promotion receipt framing is invalid"));
    }
    let value: serde_json::Value = serde_json::from_str(bytes)
        .map_err(|_| invalid("Project v8 promotion receipt JSON is invalid"))?;
    let wire = parse_wire(&value)?;
    validate_wire(&wire)?;
    let receipt = finish(wire)?;
    if receipt.canonical != bytes {
        return Err(invalid("Project v8 promotion receipt is not canonical"));
    }
    Ok(receipt)
}

pub fn replay_project_v8_promotion_receipt(
    bytes: &str,
    commit: &str,
    baseline: ProjectV8PromotionSubject,
    display_rename: ProjectV8PromotionSubject,
    artifacts: Vec<ProjectV8PromotionArtifact>,
    observations: Vec<ProjectV8PromotionGateObservation>,
) -> Result<ProjectV8PromotionReceipt, Vec<Diagnostic>> {
    let parsed = parse_project_v8_promotion_receipt(bytes)?;
    let derived = derive_project_v8_promotion_receipt(
        commit,
        baseline,
        display_rename,
        artifacts,
        observations,
    )?;
    if parsed != derived {
        return Err(invalid(
            "Project v8 promotion receipt disagrees with explicit gate observations",
        ));
    }
    Ok(parsed)
}

fn validate_wire(wire: &WireReceipt) -> Result<(), Vec<Diagnostic>> {
    if wire.schema != PROJECT_V8_PROMOTION_RECEIPT_SCHEMA
        || wire.project_schema != PROJECT_SCHEMA
        || wire.profile != PROFILE
        || wire.nonclaims != nonclaims()
    {
        return Err(invalid("Project v8 promotion receipt contract is invalid"));
    }
    validate_commit(&wire.commit)?;
    validate_subjects(&wire.subjects.baseline, &wire.subjects.display_rename)?;
    validate_artifacts(
        &wire.artifacts,
        &wire.subjects.baseline,
        &wire.subjects.display_rename,
    )?;
    let observations = wire
        .gates
        .iter()
        .map(|row| ProjectV8PromotionGateObservation {
            gate_id: row.gate_id.clone(),
            platform_id: row.platform_id.clone(),
            tool_profile_id: row.tool_profile_id.clone(),
            project_schema: row.project_schema.clone(),
            profile: row.profile.clone(),
            commit: row.commit.clone(),
            outcome: row.outcome,
        })
        .collect::<Vec<_>>();
    validate_observations(&wire.commit, &observations)?;
    let digest = artifact_inventory_digest(&wire.artifacts)?;
    if wire
        .gates
        .iter()
        .any(|row| row.artifact_inventory_digest != digest)
    {
        return Err(invalid(
            "Project v8 promotion gate has a foreign artifact inventory",
        ));
    }
    Ok(())
}

fn validate_commit(commit: &str) -> Result<(), Vec<Diagnostic>> {
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "Project v8 promotion commit must be 40 lowercase hexadecimal bytes",
        ));
    }
    Ok(())
}

fn validate_subjects(
    baseline: &ProjectV8PromotionSubject,
    renamed: &ProjectV8PromotionSubject,
) -> Result<(), Vec<Diagnostic>> {
    for (subject, label) in [(baseline, "baseline"), (renamed, "display-rename")] {
        if subject.label != label
            || subject.export_stable_ids.len() != EXPORTS.len()
            || subject
                .export_stable_ids
                .iter()
                .zip(EXPORTS)
                .any(|(actual, expected)| actual != expected)
            || !subject_digests(subject).all(valid_digest)
        {
            return Err(invalid("Project v8 promotion subject is invalid"));
        }
    }
    if baseline.export_stable_ids != renamed.export_stable_ids
        || baseline.project_revision == renamed.project_revision
        || baseline.workspace_revision == renamed.workspace_revision
        || baseline.project_graph_digest == renamed.project_graph_digest
    {
        return Err(invalid(
            "Project v8 display rename subjects are not independently bound",
        ));
    }
    Ok(())
}

fn subject_digests(subject: &ProjectV8PromotionSubject) -> impl Iterator<Item = &str> {
    [
        subject.project_revision.as_str(),
        subject.workspace_revision.as_str(),
        subject.project_graph_digest.as_str(),
        subject.descriptor_digest.as_str(),
        subject.npm_carrier_digest.as_str(),
        subject.rust_package_digest.as_str(),
    ]
    .into_iter()
}

fn validate_artifacts(
    artifacts: &[ProjectV8PromotionArtifact],
    baseline: &ProjectV8PromotionSubject,
    renamed: &ProjectV8PromotionSubject,
) -> Result<(), Vec<Diagnostic>> {
    if artifacts.len() != ARTIFACT_IDS.len() {
        return Err(invalid(
            "Project v8 promotion artifact inventory is incomplete",
        ));
    }
    for (artifact, expected) in artifacts.iter().zip(ARTIFACT_IDS) {
        if artifact.artifact_id != expected || !valid_digest(&artifact.digest) {
            return Err(invalid(
                "Project v8 promotion artifact inventory is invalid",
            ));
        }
    }
    let expected = [
        baseline.descriptor_digest.as_str(),
        baseline.npm_carrier_digest.as_str(),
        baseline.rust_package_digest.as_str(),
        renamed.descriptor_digest.as_str(),
        renamed.npm_carrier_digest.as_str(),
        renamed.rust_package_digest.as_str(),
    ];
    if artifacts[..6]
        .iter()
        .zip(expected)
        .any(|(artifact, digest)| artifact.digest != digest)
    {
        return Err(invalid(
            "Project v8 promotion artifacts disagree with retained subjects",
        ));
    }
    Ok(())
}

fn validate_observations(
    commit: &str,
    observations: &[ProjectV8PromotionGateObservation],
) -> Result<(), Vec<Diagnostic>> {
    if observations.len() != REQUIRED_GATES.len() {
        return Err(invalid("Project v8 promotion gate inventory is incomplete"));
    }
    for (row, required) in observations.iter().zip(REQUIRED_GATES) {
        if row.gate_id != required.gate_id
            || row.platform_id != required.platform_id
            || row.tool_profile_id != required.tool_profile_id
            || row.project_schema != PROJECT_SCHEMA
            || row.profile != PROFILE
            || row.commit != commit
            || row.outcome != ProjectV8PromotionGateOutcome::Passed
        {
            return Err(invalid("Project v8 promotion gate observation is invalid"));
        }
    }
    Ok(())
}

fn artifact_inventory_digest(
    artifacts: &[ProjectV8PromotionArtifact],
) -> Result<String, Vec<Diagnostic>> {
    let bytes = render_artifacts(artifacts);
    Ok(domain_digest(
        b"semaprax.project-v8-promotion-artifacts.v1\0",
        bytes.as_bytes(),
    ))
}

fn parse_wire(value: &serde_json::Value) -> Result<WireReceipt, Vec<Diagnostic>> {
    let object = exact_object(
        value,
        &[
            "schema",
            "commit",
            "project_schema",
            "profile",
            "subjects",
            "artifacts",
            "gates",
            "nonclaims",
        ],
    )?;
    let subjects = exact_object(field(object, "subjects")?, &["baseline", "display_rename"])?;
    Ok(WireReceipt {
        schema: string(field(object, "schema")?)?,
        commit: string(field(object, "commit")?)?,
        project_schema: string(field(object, "project_schema")?)?,
        profile: string(field(object, "profile")?)?,
        subjects: Subjects {
            baseline: parse_subject(field(subjects, "baseline")?)?,
            display_rename: parse_subject(field(subjects, "display_rename")?)?,
        },
        artifacts: exact_array(field(object, "artifacts")?, ARTIFACT_IDS.len())?
            .iter()
            .map(parse_artifact)
            .collect::<Result<_, _>>()?,
        gates: exact_array(field(object, "gates")?, REQUIRED_GATES.len())?
            .iter()
            .map(parse_gate)
            .collect::<Result<_, _>>()?,
        nonclaims: exact_array(field(object, "nonclaims")?, 5)?
            .iter()
            .map(string)
            .collect::<Result<_, _>>()?,
    })
}

fn parse_subject(value: &serde_json::Value) -> Result<ProjectV8PromotionSubject, Vec<Diagnostic>> {
    let object = exact_object(
        value,
        &[
            "label",
            "project_revision",
            "workspace_revision",
            "project_graph_digest",
            "descriptor_digest",
            "npm_carrier_digest",
            "rust_package_digest",
            "export_stable_ids",
        ],
    )?;
    Ok(ProjectV8PromotionSubject {
        label: string(field(object, "label")?)?,
        project_revision: string(field(object, "project_revision")?)?,
        workspace_revision: string(field(object, "workspace_revision")?)?,
        project_graph_digest: string(field(object, "project_graph_digest")?)?,
        descriptor_digest: string(field(object, "descriptor_digest")?)?,
        npm_carrier_digest: string(field(object, "npm_carrier_digest")?)?,
        rust_package_digest: string(field(object, "rust_package_digest")?)?,
        export_stable_ids: exact_array(field(object, "export_stable_ids")?, EXPORTS.len())?
            .iter()
            .map(string)
            .collect::<Result<_, _>>()?,
    })
}

fn parse_artifact(
    value: &serde_json::Value,
) -> Result<ProjectV8PromotionArtifact, Vec<Diagnostic>> {
    let object = exact_object(value, &["artifact_id", "digest"])?;
    Ok(ProjectV8PromotionArtifact {
        artifact_id: string(field(object, "artifact_id")?)?,
        digest: string(field(object, "digest")?)?,
    })
}

fn parse_gate(value: &serde_json::Value) -> Result<GateRow, Vec<Diagnostic>> {
    let object = exact_object(
        value,
        &[
            "gate_id",
            "platform_id",
            "tool_profile_id",
            "project_schema",
            "profile",
            "commit",
            "outcome",
            "artifact_inventory_digest",
        ],
    )?;
    let outcome = match field(object, "outcome")?.as_str() {
        Some("passed") => ProjectV8PromotionGateOutcome::Passed,
        Some("failed") => ProjectV8PromotionGateOutcome::Failed,
        Some("skipped") => ProjectV8PromotionGateOutcome::Skipped,
        Some("masked") => ProjectV8PromotionGateOutcome::Masked,
        Some("cancelled") => ProjectV8PromotionGateOutcome::Cancelled,
        _ => return Err(invalid("Project v8 promotion gate outcome is invalid")),
    };
    Ok(GateRow {
        gate_id: string(field(object, "gate_id")?)?,
        platform_id: string(field(object, "platform_id")?)?,
        tool_profile_id: string(field(object, "tool_profile_id")?)?,
        project_schema: string(field(object, "project_schema")?)?,
        profile: string(field(object, "profile")?)?,
        commit: string(field(object, "commit")?)?,
        outcome,
        artifact_inventory_digest: string(field(object, "artifact_inventory_digest")?)?,
    })
}

fn exact_object<'a>(
    value: &'a serde_json::Value,
    keys: &[&str],
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("Project v8 promotion receipt value is not an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("Project v8 promotion receipt object is not closed"));
    }
    Ok(object)
}

fn field<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<&'a serde_json::Value, Vec<Diagnostic>> {
    object
        .get(name)
        .ok_or_else(|| invalid("Project v8 promotion receipt field is absent"))
}

fn string(value: &serde_json::Value) -> Result<String, Vec<Diagnostic>> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| invalid("Project v8 promotion receipt field is not a string"))
}

fn array(value: &serde_json::Value) -> Result<&[serde_json::Value], Vec<Diagnostic>> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| invalid("Project v8 promotion receipt field is not an array"))
}

fn exact_array(
    value: &serde_json::Value,
    length: usize,
) -> Result<&[serde_json::Value], Vec<Diagnostic>> {
    let values = array(value)?;
    if values.len() != length {
        return Err(invalid(
            "Project v8 promotion receipt array inventory is invalid",
        ));
    }
    Ok(values)
}

fn render_wire(wire: &WireReceipt) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"commit\":{},\"project_schema\":{},\"profile\":{},\"subjects\":{{\"baseline\":{},\"display_rename\":{}}},\"artifacts\":{},\"gates\":[",
        quote_json(&wire.schema),
        quote_json(&wire.commit),
        quote_json(&wire.project_schema),
        quote_json(&wire.profile),
        render_subject(&wire.subjects.baseline),
        render_subject(&wire.subjects.display_rename),
        render_artifacts(&wire.artifacts),
    );
    for (index, gate) in wire.gates.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"gate_id\":{},\"platform_id\":{},\"tool_profile_id\":{},\"project_schema\":{},\"profile\":{},\"commit\":{},\"outcome\":{},\"artifact_inventory_digest\":{}}}",
            quote_json(&gate.gate_id),
            quote_json(&gate.platform_id),
            quote_json(&gate.tool_profile_id),
            quote_json(&gate.project_schema),
            quote_json(&gate.profile),
            quote_json(&gate.commit),
            quote_json(outcome_name(gate.outcome)),
            quote_json(&gate.artifact_inventory_digest),
        ));
    }
    output.push_str("],\"nonclaims\":[");
    for (index, claim) in wire.nonclaims.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&quote_json(claim));
    }
    output.push_str("]}\n");
    output
}

fn render_subject(subject: &ProjectV8PromotionSubject) -> String {
    format!(
        "{{\"label\":{},\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"descriptor_digest\":{},\"npm_carrier_digest\":{},\"rust_package_digest\":{},\"export_stable_ids\":[{}]}}",
        quote_json(&subject.label),
        quote_json(&subject.project_revision),
        quote_json(&subject.workspace_revision),
        quote_json(&subject.project_graph_digest),
        quote_json(&subject.descriptor_digest),
        quote_json(&subject.npm_carrier_digest),
        quote_json(&subject.rust_package_digest),
        subject
            .export_stable_ids
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn render_artifacts(artifacts: &[ProjectV8PromotionArtifact]) -> String {
    let mut output = String::from("[");
    for (index, artifact) in artifacts.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"artifact_id\":{},\"digest\":{}}}",
            quote_json(&artifact.artifact_id),
            quote_json(&artifact.digest)
        ));
    }
    output.push(']');
    output
}

fn outcome_name(outcome: ProjectV8PromotionGateOutcome) -> &'static str {
    match outcome {
        ProjectV8PromotionGateOutcome::Passed => "passed",
        ProjectV8PromotionGateOutcome::Failed => "failed",
        ProjectV8PromotionGateOutcome::Skipped => "skipped",
        ProjectV8PromotionGateOutcome::Masked => "masked",
        ProjectV8PromotionGateOutcome::Cancelled => "cancelled",
    }
}

fn finish(wire: WireReceipt) -> Result<ProjectV8PromotionReceipt, Vec<Diagnostic>> {
    let canonical = render_wire(&wire);
    if canonical.len() > MAX_PROJECT_V8_PROMOTION_RECEIPT_BYTES {
        return Err(invalid(
            "Project v8 promotion receipt exceeds its byte bound",
        ));
    }
    let digest = domain_digest(DIGEST_DOMAIN, canonical.as_bytes());
    Ok(ProjectV8PromotionReceipt {
        wire,
        canonical,
        digest,
    })
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn nonclaims() -> Vec<String> {
    [
        "caller_owned_observations_not_host_attestation",
        "no_gate_execution_or_green_status_inference",
        "no_support_registry_release_or_publication_authority",
        "digest_integrity_not_signature_or_provenance",
        "receipt_replay_not_project_v8_promotion",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J119", message)]
}
