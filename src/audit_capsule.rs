//! `semaprax.audit-capsule.v1`: a manifest plus a content-addressed object
//! set that references existing SEMAPRAX evidence documents by digest and
//! binds them to one subject -- a semantic change, an Agent run, or a
//! release (issue #209).
//!
//! ## What a capsule is
//!
//! A capsule is exactly two things: a **manifest** (the bytes [`parse_capsule`]
//! reads) naming a `profile`, a `subject` (the change/run/release this
//! capsule is about), an ordered list of [`ObjectRef`] envelopes, an
//! [`AssociationEdge`] graph between them, a set of role-tagged
//! [`SignatureEntry`] records, and an optional [`TransparencyEntry`]; and a
//! **content-addressed object set** -- the raw bytes each non-redacted
//! [`ObjectRef`] names, supplied separately as a `BTreeMap<String, Vec<u8>>`
//! keyed by object id (`object_bytes` throughout this module). Nothing here
//! reparses or reinterprets those bytes as anything other than an opaque
//! blob with a schema label: **existing object semantics remain owned by
//! their original schemas** (`src/release_provenance.rs` #168,
//! `src/model_call_receipt/` #180, `src/job_evidence.rs` #192,
//! `src/live_invocation/` #108/#177, `src/assurance_manifest.rs`, and every
//! other `semaprax.*.v*` schema this crate already defines), all of which
//! are read-only inputs from this module's perspective. A referenced
//! object's `digest` is the *plain* SHA-256 of its exact bytes -- the same
//! digest anyone would get from `sha256sum`, with no capsule-specific domain
//! tag -- so a capsule never becomes a second, incompatible way to hash an
//! object its owning schema already hashes one way.
//!
//! ## What this module deliberately does not do
//!
//! **Signing is unimplemented and `HUMAN_BLOCKED`.** [`SignatureEntry`]
//! carries `algorithm`/`identity`/`signature` exactly as opaque,
//! structurally-checked strings the way
//! [`crate::release_provenance::ParsedSignatureClaim`] does, for the same
//! reason: no signing key, keyless-signing (Sigstore) identity, or signature
//! -verification dependency exists in this repository, and generated code
//! and compiler tooling gain no ambient signing authority (`AGENTS.md`).
//! [`check_signature_policy`] validates role, expiry, and revocation --
//! plaintext policy facts that need no cryptography -- and never decodes or
//! verifies `signature` bytes. A forged signature naming an approved
//! identity and an unexpired timestamp is **not** rejected by this module;
//! only pairing this policy check with a real external verifier (the same
//! `cosign verify-blob`-shaped gap #168 documents) closes it.
//!
//! **Transparency-log submission and verification are unimplemented and
//! `HUMAN_BLOCKED`.** [`check_transparency`] only checks that a
//! caller-supplied [`TransparencyEntry`] is internally consistent (its
//! `leaf_digest` matches [`transparency_leaf_digest`] recomputed from the
//! manifest under test) and not stale
//! relative to a caller-supplied trusted checkpoint size -- it never
//! contacts a real log (e.g. Sigstore's Rekor) over the network, because
//! this module must verify with no network access at all. Nothing here
//! proves a log entry was actually accepted by a real, independently
//! operated log.
//!
//! ## Portability
//!
//! Every function in this module takes only in-memory byte slices, string
//! maps, and plain closed-vocabulary values -- never a [`std::path::Path`],
//! a socket, or a subprocess handle. A capsule built on one machine verifies
//! identically on any other: `cargo run --locked -p semaprax` is not even
//! required, since [`parse_capsule`] and every `check_*`/[`verify_capsule`]
//! function only need `serde_json` and `sha2`, both ordinary library
//! dependencies with no ambient authority. See `tests` for the fixtures this
//! module verifies purely in memory, with no filesystem or network access at
//! any point in the call graph.
//!
//! ## Evidence, not authority
//!
//! A successfully verified capsule proves exactly what its retained objects'
//! own schemas already prove, bound together and unmodified since assembly.
//! It never authorizes a release, widens a budget, replays an effect, or
//! approves itself: nothing in this module executes, publishes, or spawns
//! anything, and possessing (or even fully verifying) a capsule is not the
//! same fact as a human or policy having granted permission for whatever the
//! capsule describes.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::diagnostic::Diagnostic;

pub const CAPSULE_SCHEMA: &str = "semaprax.audit-capsule.v1";

/// A capsule's manifest bytes may not exceed this many bytes. Guards against
/// "a capsule can become enormous" (issue #209's failure list) at the
/// cheapest possible check, before any JSON parsing is attempted.
pub const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;

/// At most this many object envelopes per capsule.
pub const MAX_OBJECTS: usize = 512;

/// At most this many association edges per capsule.
pub const MAX_ASSOCIATIONS: usize = 2048;

/// At most this many bytes for any single referenced object's retained
/// payload, checked against the caller-supplied `object_bytes` map.
pub const MAX_OBJECT_BYTES: usize = 64 * 1024 * 1024;

/// The closed object-type vocabulary a capsule may reference. Deliberately
/// excludes `"audit-capsule"` itself: a capsule can never embed another
/// capsule as one of its own objects, which is what keeps "a capsule
/// recursively references itself" (issue #209's failure list) structurally
/// unrepresentable rather than merely discouraged.
pub const KNOWN_OBJECT_TYPES: &[&str] = &[
    "source-projection",
    "program-root",
    "semantic-transaction",
    "requirement-evidence",
    "architecture-claim",
    "assurance-manifest",
    "test-evidence",
    "build-evidence",
    "agent-definition",
    "agent-deployment",
    "agent-invocation",
    "agent-checkpoint",
    "agent-migration",
    "model-call-receipt",
    "tool-receipt",
    "artifact",
    "package-manifest",
    "sbom",
    "release-provenance",
    "release-signature-claim",
    "job-evidence-log",
    "decision-record",
];

/// The closed relation vocabulary an [`AssociationEdge`] may declare.
pub const KNOWN_RELATIONS: &[&str] = &["derived_from", "supersedes", "redacts", "attests"];

/// The closed role vocabulary a [`SignatureEntry`] may declare, matching
/// issue #209's own "Proposer/reviewer/validator/approver/publisher
/// decisions" list exactly. At most one signature per role is admitted (see
/// [`parse_capsule`]), which is what keeps "signatures from different roles
/// can be confused" (issue #209's failure list) from being possible: a
/// caller can look a role up and get exactly the one entry that claimed it,
/// never a different role's entry silently substituted.
pub const KNOWN_SIGNATURE_ROLES: &[&str] =
    &["proposer", "reviewer", "validator", "approver", "publisher"];

/// Recognizing an algorithm identifier here is a structural admission only,
/// never a cryptographic endorsement -- see the module doc.
pub const KNOWN_SIGNATURE_ALGORITHMS: &[&str] = &["sigstore-cosign-bundle-v0.3", "ed25519-raw-v1"];

pub const REQUIRED_OBJECT_TYPES_CHANGE: &[&str] = &[
    "source-projection",
    "program-root",
    "semantic-transaction",
    "assurance-manifest",
];
pub const REQUIRED_OBJECT_TYPES_AGENT_RUN: &[&str] = &[
    "agent-definition",
    "agent-deployment",
    "agent-invocation",
    "model-call-receipt",
];
pub const REQUIRED_OBJECT_TYPES_RELEASE: &[&str] = &[
    "release-provenance",
    "release-signature-claim",
    "artifact",
    "package-manifest",
];

pub const SUBJECT_KEYS_CHANGE: &[&str] = &["source_digest", "root_digest", "revision"];
pub const SUBJECT_KEYS_AGENT_RUN: &[&str] = &["session_id", "deployment_digest", "target_digest"];
pub const SUBJECT_KEYS_RELEASE: &[&str] = &["release_tag", "commit", "artifact_digest"];

/// One of the three capsule profiles issue #209 names. Composing several
/// profiles into one capsule (e.g. a release capsule embedding a prior
/// change capsule's objects) is intentionally **not** implemented here: it
/// is future scope, not a blocked dependency, and is called out as such in
/// `docs/AUDIT-CAPSULE-V1.md` rather than silently claimed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Change,
    AgentRun,
    Release,
}

impl Profile {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Profile::Change => "change",
            Profile::AgentRun => "agent-run",
            Profile::Release => "release",
        }
    }

    fn from_wire(value: &str) -> Option<Self> {
        match value {
            "change" => Some(Profile::Change),
            "agent-run" => Some(Profile::AgentRun),
            "release" => Some(Profile::Release),
            _ => None,
        }
    }

    #[must_use]
    pub fn required_object_types(self) -> &'static [&'static str] {
        match self {
            Profile::Change => REQUIRED_OBJECT_TYPES_CHANGE,
            Profile::AgentRun => REQUIRED_OBJECT_TYPES_AGENT_RUN,
            Profile::Release => REQUIRED_OBJECT_TYPES_RELEASE,
        }
    }

    #[must_use]
    pub fn subject_keys(self) -> &'static [&'static str] {
        match self {
            Profile::Change => SUBJECT_KEYS_CHANGE,
            Profile::AgentRun => SUBJECT_KEYS_AGENT_RUN,
            Profile::Release => SUBJECT_KEYS_RELEASE,
        }
    }
}

fn shape_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z901", message)
}

fn vocabulary_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z902", message)
}

fn association_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z903", message)
}

fn binding_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z904", message)
}

fn signature_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z905", message)
}

fn transparency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z906", message)
}

fn capacity_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-Z907", message)
}

/// The plain SHA-256 of `bytes`, with no capsule-specific domain
/// separation -- see the module doc for why this must be the same digest an
/// independent `sha256sum` invocation would produce.
#[must_use]
pub fn sha256_digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Recursively sorts every JSON object's keys, leaving array order and all
/// scalar values untouched. Used only to compute
/// [`transparency_leaf_digest`]'s canonical bytes -- never to reinterpret or
/// re-derive the meaning of a referenced object, which stays owned by its
/// own schema.
fn sorted_json(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(sorted_json).collect()),
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut result = serde_json::Map::new();
            for key in keys {
                result.insert(key.clone(), sorted_json(&map[key]));
            }
            Value::Object(result)
        }
        other => other.clone(),
    }
}

/// The canonical bytes a [`TransparencyEntry::leaf_digest`] commits to: the
/// capsule manifest with its `transparency` field replaced by `null` and its
/// object keys sorted, terminated by one LF.
///
/// A leaf digest cannot honestly commit to "the exact manifest bytes"
/// *including* the transparency field that carries the leaf digest itself --
/// that is a fixed-point requirement no hash function lets you satisfy by
/// construction, not a real integrity property. Real transparency logs sign
/// over the payload as it was **submitted for logging**, before the
/// resulting inclusion proof is appended back onto it; nulling the
/// `transparency` field before hashing reproduces exactly that ordering
/// without needing to track a separate pre-submission copy of the manifest.
/// Every other field (`profile`, `subject`, `objects`, `associations`,
/// `signatures`) is covered, so tampering any of them still changes this
/// digest.
fn transparency_leaf_digest_bytes(manifest_bytes: &[u8]) -> Result<Vec<u8>, Diagnostic> {
    let mut value = parse_json(manifest_bytes, "audit capsule")?;
    let Some(map) = value.as_object_mut() else {
        return Err(shape_error("audit capsule must be a JSON object".to_owned()));
    };
    map.insert("transparency".to_owned(), Value::Null);
    let mut text = serde_json::to_string(&sorted_json(&value)).map_err(|_| {
        shape_error("audit capsule cannot be canonically re-serialized".to_owned())
    })?;
    text.push('\n');
    Ok(text.into_bytes())
}

/// Computes the digest a [`TransparencyEntry::leaf_digest`] must equal for
/// `manifest_bytes` -- see [`transparency_leaf_digest_bytes`] for why this is
/// not simply `sha256_digest(manifest_bytes)`. A real capsule producer calls
/// this **before** it has a transparency entry to attach, submits the
/// resulting digest to a log, and only then fills in the `transparency`
/// field with the log's response.
pub fn transparency_leaf_digest(manifest_bytes: &[u8]) -> Result<String, Diagnostic> {
    Ok(sha256_digest(&transparency_leaf_digest_bytes(
        manifest_bytes,
    )?))
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

fn object<'a>(
    value: &'a Value,
    context: &str,
) -> Result<&'a serde_json::Map<String, Value>, Diagnostic> {
    value
        .as_object()
        .ok_or_else(|| shape_error(format!("{context} must be a JSON object")))
}

fn require_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, Diagnostic> {
    value
        .as_str()
        .ok_or_else(|| shape_error(format!("`{field}` must be a string")))
}

fn require_nonempty_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, Diagnostic> {
    let text = require_string(value, field)?;
    if text.is_empty() {
        return Err(shape_error(format!("`{field}` must not be empty")));
    }
    Ok(text)
}

fn require_bool(value: &Value, field: &str) -> Result<bool, Diagnostic> {
    value
        .as_bool()
        .ok_or_else(|| shape_error(format!("`{field}` must be a boolean")))
}

fn require_u64(value: &Value, field: &str) -> Result<u64, Diagnostic> {
    value
        .as_u64()
        .ok_or_else(|| shape_error(format!("`{field}` must be an unsigned integer")))
}

fn require_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, Diagnostic> {
    value
        .as_array()
        .ok_or_else(|| shape_error(format!("`{field}` must be an array")))
}

fn check_exact_keys(
    map: &serde_json::Map<String, Value>,
    expected: &[&str],
    context: &str,
) -> Result<(), Diagnostic> {
    let mut found: Vec<&str> = map.keys().map(String::as_str).collect();
    found.sort_unstable();
    let mut expected_sorted: Vec<&str> = expected.to_vec();
    expected_sorted.sort_unstable();
    if found != expected_sorted {
        return Err(shape_error(format!(
            "{context} keys must be exactly {expected_sorted:?}, found {found:?}"
        )));
    }
    Ok(())
}

fn parse_json(bytes: &[u8], context: &str) -> Result<Value, Diagnostic> {
    serde_json::from_slice(bytes)
        .map_err(|error| shape_error(format!("{context} is not valid JSON: {error}")))
}

/// One content-addressed object envelope: metadata about a referenced
/// evidence document, never the document's own interpreted meaning. Exactly
/// one of "retained" (`redacted == false`, bytes supplied separately in
/// `object_bytes`, keyed by `id`) or "redacted" (`redacted == true`, no
/// bytes supplied, `redaction_reason` explains why) applies to any object;
/// [`parse_capsule`] rejects a manifest that claims otherwise.
#[derive(Debug, Clone)]
pub struct ObjectRef {
    pub id: String,
    pub object_type: String,
    /// The exact schema id the referenced object's own owning module
    /// defines (e.g. `"semaprax.model-call-receipt.v1"`). Opaque and
    /// unvalidated here -- see the module doc's "existing object semantics
    /// remain owned by their original schemas."
    pub schema: String,
    pub digest: String,
    pub redacted: bool,
    pub redaction_reason: Option<String>,
    /// A subset of the capsule's `subject` keys this object claims to be
    /// bound to, with the value it claims for each. [`check_subject_bindings`]
    /// rejects a claimed value that disagrees with the capsule's own subject.
    pub binds: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AssociationEdge {
    pub from_id: String,
    pub relation: String,
    pub to_id: String,
}

/// A role-tagged signature claim over the capsule manifest. See the module
/// doc: `signature` is opaque and never cryptographically verified here.
#[derive(Debug, Clone)]
pub struct SignatureEntry {
    pub role: String,
    pub identity: String,
    pub algorithm: String,
    pub signature: String,
    pub not_valid_after_unix_seconds: u64,
}

/// A caller-supplied claim that this capsule's exact manifest bytes were
/// included in some transparency log at some checkpoint. See the module doc
/// -- this is never checked against a real, network-reachable log.
#[derive(Debug, Clone)]
pub struct TransparencyEntry {
    pub log_id: String,
    pub leaf_digest: String,
    pub inclusion_proof: Vec<String>,
    pub observed_checkpoint_size: u64,
}

/// A structurally validated `semaprax.audit-capsule.v1` manifest,
/// independently re-derived from raw bytes rather than trusted from a
/// caller-supplied struct -- the same discipline
/// `crate::release_provenance::parse_provenance` uses.
#[derive(Debug, Clone)]
pub struct ParsedCapsule {
    pub profile: Profile,
    pub subject: BTreeMap<String, String>,
    pub objects: Vec<ObjectRef>,
    pub associations: Vec<AssociationEdge>,
    pub signatures: Vec<SignatureEntry>,
    pub transparency: Option<TransparencyEntry>,
}

fn parse_object(value: &Value, profile: Profile) -> Result<ObjectRef, Diagnostic> {
    let map = object(value, "object")?;
    check_exact_keys(
        map,
        &[
            "id",
            "object_type",
            "schema",
            "digest",
            "redacted",
            "redaction_reason",
            "binds",
        ],
        "object",
    )?;
    let id = require_nonempty_string(&value["id"], "object.id")?.to_owned();
    let object_type = require_nonempty_string(&value["object_type"], "object.object_type")?;
    if !KNOWN_OBJECT_TYPES.contains(&object_type) {
        return Err(vocabulary_error(format!(
            "object `{id}` declares object_type `{object_type}`, which is not in the closed \
             object-type vocabulary {KNOWN_OBJECT_TYPES:?}"
        )));
    }
    let object_type = object_type.to_owned();
    let schema = require_nonempty_string(&value["schema"], "object.schema")?.to_owned();
    let digest = require_string(&value["digest"], "object.digest")?.to_owned();
    if !is_sha256_wire_form(&digest) {
        return Err(shape_error(format!(
            "object `{id}` digest must be `sha256:<64 lowercase hex>`"
        )));
    }
    let redacted = require_bool(&value["redacted"], "object.redacted")?;
    let redaction_reason = match &value["redaction_reason"] {
        Value::Null => None,
        other => Some(require_nonempty_string(other, "object.redaction_reason")?.to_owned()),
    };
    match (redacted, &redaction_reason) {
        (true, None) => {
            return Err(shape_error(format!(
                "object `{id}` is redacted but carries no redaction_reason"
            )));
        }
        (false, Some(_)) => {
            return Err(shape_error(format!(
                "object `{id}` carries a redaction_reason but redacted is false"
            )));
        }
        _ => {}
    }
    let binds_map = object(&value["binds"], "object.binds")?;
    let mut binds = BTreeMap::new();
    for (key, bind_value) in binds_map {
        if !profile.subject_keys().contains(&key.as_str()) {
            return Err(shape_error(format!(
                "object `{id}` binds unknown subject key `{key}`; profile `{}` only recognizes \
                 {:?}",
                profile.as_str(),
                profile.subject_keys()
            )));
        }
        let value = require_nonempty_string(bind_value, &format!("object.binds.{key}"))?;
        binds.insert(key.clone(), value.to_owned());
    }
    Ok(ObjectRef {
        id,
        object_type,
        schema,
        digest,
        redacted,
        redaction_reason,
        binds,
    })
}

fn parse_association(value: &Value) -> Result<AssociationEdge, Diagnostic> {
    let map = object(value, "association")?;
    check_exact_keys(map, &["from_id", "relation", "to_id"], "association")?;
    let from_id = require_nonempty_string(&value["from_id"], "association.from_id")?.to_owned();
    let to_id = require_nonempty_string(&value["to_id"], "association.to_id")?.to_owned();
    let relation = require_nonempty_string(&value["relation"], "association.relation")?;
    if !KNOWN_RELATIONS.contains(&relation) {
        return Err(vocabulary_error(format!(
            "association from `{from_id}` to `{to_id}` declares relation `{relation}`, which is \
             not in the closed relation vocabulary {KNOWN_RELATIONS:?}"
        )));
    }
    Ok(AssociationEdge {
        from_id,
        relation: relation.to_owned(),
        to_id,
    })
}

fn parse_signature(value: &Value) -> Result<SignatureEntry, Diagnostic> {
    let map = object(value, "signature")?;
    check_exact_keys(
        map,
        &[
            "role",
            "identity",
            "algorithm",
            "signature",
            "not_valid_after_unix_seconds",
        ],
        "signature",
    )?;
    let role = require_nonempty_string(&value["role"], "signature.role")?;
    if !KNOWN_SIGNATURE_ROLES.contains(&role) {
        return Err(vocabulary_error(format!(
            "signature declares role `{role}`, which is not in the closed role vocabulary \
             {KNOWN_SIGNATURE_ROLES:?}"
        )));
    }
    let role = role.to_owned();
    let identity = require_nonempty_string(&value["identity"], "signature.identity")?.to_owned();
    let algorithm = require_nonempty_string(&value["algorithm"], "signature.algorithm")?;
    if !KNOWN_SIGNATURE_ALGORITHMS.contains(&algorithm) {
        return Err(vocabulary_error(format!(
            "signature declares algorithm `{algorithm}`, which is not in the closed algorithm \
             vocabulary {KNOWN_SIGNATURE_ALGORITHMS:?}"
        )));
    }
    let algorithm = algorithm.to_owned();
    let signature =
        require_nonempty_string(&value["signature"], "signature.signature")?.to_owned();
    let not_valid_after_unix_seconds = require_u64(
        &value["not_valid_after_unix_seconds"],
        "signature.not_valid_after_unix_seconds",
    )?;
    Ok(SignatureEntry {
        role,
        identity,
        algorithm,
        signature,
        not_valid_after_unix_seconds,
    })
}

fn parse_transparency(value: &Value) -> Result<TransparencyEntry, Diagnostic> {
    let map = object(value, "transparency")?;
    check_exact_keys(
        map,
        &[
            "log_id",
            "leaf_digest",
            "inclusion_proof",
            "observed_checkpoint_size",
        ],
        "transparency",
    )?;
    let log_id = require_nonempty_string(&value["log_id"], "transparency.log_id")?.to_owned();
    let leaf_digest = require_string(&value["leaf_digest"], "transparency.leaf_digest")?;
    if !is_sha256_wire_form(leaf_digest) {
        return Err(shape_error(
            "transparency.leaf_digest must be `sha256:<64 lowercase hex>`".to_owned(),
        ));
    }
    let leaf_digest = leaf_digest.to_owned();
    let proof_array = require_array(&value["inclusion_proof"], "transparency.inclusion_proof")?;
    if proof_array.is_empty() {
        return Err(shape_error(
            "transparency.inclusion_proof must not be empty".to_owned(),
        ));
    }
    let mut inclusion_proof = Vec::with_capacity(proof_array.len());
    for entry in proof_array {
        inclusion_proof
            .push(require_nonempty_string(entry, "transparency.inclusion_proof[]")?.to_owned());
    }
    let observed_checkpoint_size = require_u64(
        &value["observed_checkpoint_size"],
        "transparency.observed_checkpoint_size",
    )?;
    Ok(TransparencyEntry {
        log_id,
        leaf_digest,
        inclusion_proof,
        observed_checkpoint_size,
    })
}

/// Independently parse and structurally validate a
/// `semaprax.audit-capsule.v1` manifest from its exact bytes.
///
/// Rejects (fails closed): malformed JSON, a wrong or missing schema/profile,
/// a subject whose keys disagree with the profile's closed set, an object
/// whose `object_type` is outside [`KNOWN_OBJECT_TYPES`], an object that is
/// simultaneously (or neither) redacted and reasoned, an object bound to an
/// unrecognized subject key, a duplicate object id, an unsorted object list
/// (canonical ordering is part of this schema, not a rendering nicety), an
/// association naming an unrecognized relation, a signature naming an
/// unrecognized role or algorithm, a duplicate signature role, and a
/// malformed transparency entry. Also enforces [`MAX_MANIFEST_BYTES`],
/// [`MAX_OBJECTS`], and [`MAX_ASSOCIATIONS`] before doing any heavier work.
pub fn parse_capsule(bytes: &[u8]) -> Result<ParsedCapsule, Diagnostic> {
    if bytes.len() > MAX_MANIFEST_BYTES {
        return Err(capacity_error(format!(
            "capsule manifest is {} bytes, exceeding the {MAX_MANIFEST_BYTES}-byte limit",
            bytes.len()
        )));
    }
    let value = parse_json(bytes, "audit capsule")?;
    let map = object(&value, "audit capsule")?;
    check_exact_keys(
        map,
        &[
            "schema",
            "profile",
            "subject",
            "objects",
            "associations",
            "signatures",
            "transparency",
        ],
        "audit capsule",
    )?;
    if require_string(&value["schema"], "schema")? != CAPSULE_SCHEMA {
        return Err(shape_error(format!("capsule schema must be {CAPSULE_SCHEMA}")));
    }
    let profile_wire = require_string(&value["profile"], "profile")?;
    let profile = Profile::from_wire(profile_wire).ok_or_else(|| {
        vocabulary_error(format!(
            "profile `{profile_wire}` is not one of the admitted profiles \
             (\"change\", \"agent-run\", \"release\")"
        ))
    })?;

    let subject_map = object(&value["subject"], "subject")?;
    check_exact_keys(subject_map, profile.subject_keys(), "subject")?;
    let mut subject = BTreeMap::new();
    for key in profile.subject_keys() {
        let text = require_nonempty_string(&value["subject"][*key], &format!("subject.{key}"))?;
        subject.insert((*key).to_owned(), text.to_owned());
    }

    let objects_array = require_array(&value["objects"], "objects")?;
    if objects_array.len() > MAX_OBJECTS {
        return Err(capacity_error(format!(
            "capsule declares {} objects, exceeding the {MAX_OBJECTS}-object limit",
            objects_array.len()
        )));
    }
    let mut objects = Vec::with_capacity(objects_array.len());
    for entry in objects_array {
        objects.push(parse_object(entry, profile)?);
    }
    for window in objects.windows(2) {
        let [previous, current] = window else {
            unreachable!("windows(2) always yields exactly two elements")
        };
        if current.id == previous.id {
            return Err(shape_error(format!(
                "duplicate object id `{}` in the capsule's object list",
                current.id
            )));
        }
        if current.id < previous.id {
            return Err(shape_error(format!(
                "objects are not in ascending canonical order by id: `{}` follows `{}`",
                current.id, previous.id
            )));
        }
    }

    let associations_array = require_array(&value["associations"], "associations")?;
    if associations_array.len() > MAX_ASSOCIATIONS {
        return Err(capacity_error(format!(
            "capsule declares {} associations, exceeding the {MAX_ASSOCIATIONS}-association limit",
            associations_array.len()
        )));
    }
    let mut associations = Vec::with_capacity(associations_array.len());
    for entry in associations_array {
        associations.push(parse_association(entry)?);
    }

    let signatures_array = require_array(&value["signatures"], "signatures")?;
    let mut signatures = Vec::with_capacity(signatures_array.len());
    let mut seen_roles = BTreeSet::new();
    for entry in signatures_array {
        let signature = parse_signature(entry)?;
        if !seen_roles.insert(signature.role.clone()) {
            return Err(shape_error(format!(
                "role `{}` carries more than one signature; each role may sign at most once",
                signature.role
            )));
        }
        signatures.push(signature);
    }

    let transparency = match &value["transparency"] {
        Value::Null => None,
        other => Some(parse_transparency(other)?),
    };

    Ok(ParsedCapsule {
        profile,
        subject,
        objects,
        associations,
        signatures,
        transparency,
    })
}

/// Checks that every object type [`Profile::required_object_types`] names
/// appears **exactly once** among `capsule.objects`. Zero occurrences is a
/// missing required object; more than one is rejected as an ambiguous extra
/// (issue #209's "missing" and "extra" object-reference failure cases).
/// Object types outside the required set may still appear (already
/// constrained to [`KNOWN_OBJECT_TYPES`] by [`parse_capsule`]).
pub fn check_required_object_types(capsule: &ParsedCapsule) -> Result<(), Diagnostic> {
    for required_type in capsule.profile.required_object_types() {
        let count = capsule
            .objects
            .iter()
            .filter(|candidate| candidate.object_type == *required_type)
            .count();
        if count == 0 {
            return Err(vocabulary_error(format!(
                "capsule profile `{}` requires an object of type `{required_type}`, but none is \
                 present",
                capsule.profile.as_str()
            )));
        }
        if count > 1 {
            return Err(vocabulary_error(format!(
                "capsule profile `{}` expects exactly one object of type `{required_type}`, but \
                 {count} are present",
                capsule.profile.as_str()
            )));
        }
    }
    Ok(())
}

/// Checks the association graph: every edge's `from_id`/`to_id` must name an
/// object actually present in `capsule.objects` (a dangling reference is
/// rejected), and the graph itself must be acyclic, including a one-node
/// self-loop -- issue #209's "a capsule can ... recursively reference
/// itself" failure case, applied to the association graph rather than only
/// to object-type nesting.
pub fn check_associations(capsule: &ParsedCapsule) -> Result<(), Diagnostic> {
    let ids: BTreeSet<&str> = capsule.objects.iter().map(|o| o.id.as_str()).collect();
    let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for edge in &capsule.associations {
        if !ids.contains(edge.from_id.as_str()) {
            return Err(association_error(format!(
                "association names unknown object id `{}` as from_id",
                edge.from_id
            )));
        }
        if !ids.contains(edge.to_id.as_str()) {
            return Err(association_error(format!(
                "association names unknown object id `{}` as to_id",
                edge.to_id
            )));
        }
        adjacency
            .entry(edge.from_id.as_str())
            .or_default()
            .push(edge.to_id.as_str());
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Done,
    }

    fn visit<'a>(
        node: &'a str,
        adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
        marks: &mut BTreeMap<&'a str, Mark>,
    ) -> Result<(), &'a str> {
        match marks.get(node) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Visiting) => return Err(node),
            None => {}
        }
        marks.insert(node, Mark::Visiting);
        if let Some(children) = adjacency.get(node) {
            for &child in children {
                visit(child, adjacency, marks)?;
            }
        }
        marks.insert(node, Mark::Done);
        Ok(())
    }

    let mut marks: BTreeMap<&str, Mark> = BTreeMap::new();
    for &id in &ids {
        if let Err(cycle_node) = visit(id, &adjacency, &mut marks) {
            return Err(association_error(format!(
                "association graph is cyclic: object id `{cycle_node}` reaches itself"
            )));
        }
    }
    Ok(())
}

/// Checks that every object's declared [`ObjectRef::binds`] agrees, key for
/// key, with the capsule's own `subject`. A mismatch is exactly issue
/// #209's "stale" object-reference failure case: an object honestly minted
/// for a different revision/session/target that was left in a manifest
/// without updating its bound claim.
pub fn check_subject_bindings(capsule: &ParsedCapsule) -> Result<(), Diagnostic> {
    for candidate in &capsule.objects {
        for (key, claimed_value) in &candidate.binds {
            let Some(subject_value) = capsule.subject.get(key) else {
                return Err(binding_error(format!(
                    "object `{}` binds subject key `{key}`, which the capsule subject does not \
                     declare",
                    candidate.id
                )));
            };
            if claimed_value != subject_value {
                return Err(binding_error(format!(
                    "object `{}` is stale: it binds `{key}` = `{claimed_value}`, but the capsule \
                     subject declares `{key}` = `{subject_value}`",
                    candidate.id
                )));
            }
        }
    }
    Ok(())
}

/// Checks every object's byte-level integrity against `object_bytes`, a
/// caller-supplied content-addressed store keyed by object id.
///
/// A retained (`redacted == false`) object must have an entry in
/// `object_bytes`; that entry's plain SHA-256 digest must equal
/// [`ObjectRef::digest`] exactly, or the object is rejected as **substituted**
/// -- issue #209's failure case for an object whose declared digest was not
/// recomputed from the bytes actually being verified. A redacted object must
/// have **no** entry in `object_bytes` (a caller that supplies bytes for a
/// nominally redacted object leaks exactly what redaction is meant to
/// withhold, so this fails closed rather than silently ignoring the leak).
///
/// Returns the ids of every object whose integrity was independently
/// verified (the retained ones).
pub fn check_object_bytes(
    capsule: &ParsedCapsule,
    object_bytes: &BTreeMap<String, Vec<u8>>,
) -> Result<Vec<String>, Diagnostic> {
    let mut verified = Vec::new();
    for candidate in &capsule.objects {
        if candidate.redacted {
            if object_bytes.contains_key(&candidate.id) {
                return Err(binding_error(format!(
                    "object `{}` is marked redacted but retained bytes were supplied for it",
                    candidate.id
                )));
            }
            continue;
        }
        let bytes = object_bytes.get(&candidate.id).ok_or_else(|| {
            binding_error(format!(
                "object `{}` is not redacted, but no retained bytes were supplied for it",
                candidate.id
            ))
        })?;
        if bytes.len() > MAX_OBJECT_BYTES {
            return Err(capacity_error(format!(
                "object `{}` is {} bytes, exceeding the {MAX_OBJECT_BYTES}-byte limit",
                candidate.id,
                bytes.len()
            )));
        }
        let recomputed = sha256_digest(bytes);
        if recomputed != candidate.digest {
            return Err(binding_error(format!(
                "object `{}` is substituted: its recorded digest `{}` does not match the digest \
                 `{recomputed}` recomputed from the exact bytes under test",
                candidate.id, candidate.digest
            )));
        }
        verified.push(candidate.id.clone());
    }
    Ok(verified)
}

/// Caller-supplied trust context for [`check_signature_policy`]. Everything
/// here is a plaintext policy fact the caller must already trust from some
/// other source -- this module pins no roster of its own, unlike
/// `crate::release_provenance`'s single pinned CI identity, because an
/// audit capsule's proposer/reviewer/validator/approver/publisher roster is
/// per-deployment, not a single repository-wide constant.
#[derive(Debug, Clone, Default)]
pub struct SignaturePolicyContext {
    pub verification_time_unix_seconds: u64,
    pub revoked_identities: BTreeSet<String>,
    pub required_roles: Vec<String>,
}

/// Checks role presence, expiry, and revocation for a capsule's signatures.
/// Never decodes or cryptographically verifies a [`SignatureEntry::signature`]
/// -- see the module doc.
pub fn check_signature_policy(
    capsule: &ParsedCapsule,
    ctx: &SignaturePolicyContext,
) -> Result<(), Diagnostic> {
    for required_role in &ctx.required_roles {
        if !capsule
            .signatures
            .iter()
            .any(|signature| &signature.role == required_role)
        {
            return Err(signature_error(format!(
                "no signature carries the required role `{required_role}`"
            )));
        }
    }
    for signature in &capsule.signatures {
        if ctx.revoked_identities.contains(&signature.identity) {
            return Err(signature_error(format!(
                "signature role `{}` identity `{}` is revoked",
                signature.role, signature.identity
            )));
        }
        if signature.not_valid_after_unix_seconds < ctx.verification_time_unix_seconds {
            return Err(signature_error(format!(
                "signature role `{}` identity `{}` expired at unix time {}, before the \
                 verification time {}",
                signature.role,
                signature.identity,
                signature.not_valid_after_unix_seconds,
                ctx.verification_time_unix_seconds
            )));
        }
    }
    Ok(())
}

/// Caller-supplied trust context for [`check_transparency`].
#[derive(Debug, Clone, Default)]
pub struct TransparencyContext {
    pub known_logs: BTreeSet<String>,
    pub minimum_accepted_checkpoint_size: u64,
}

/// Checks a capsule's optional [`TransparencyEntry`] against `manifest_bytes`
/// and `ctx`. A capsule with no transparency entry always passes here --
/// transparency-log submission is optional (issue #209: "Optionally submit
/// capsule digest to a transparency log"). See the module doc: this never
/// contacts a real log.
pub fn check_transparency(
    capsule: &ParsedCapsule,
    manifest_bytes: &[u8],
    ctx: &TransparencyContext,
) -> Result<(), Diagnostic> {
    let Some(entry) = &capsule.transparency else {
        return Ok(());
    };
    let recomputed = transparency_leaf_digest(manifest_bytes)?;
    if entry.leaf_digest != recomputed {
        return Err(transparency_error(format!(
            "transparency entry is invalid: leaf_digest `{}` does not match the digest `{recomputed}` \
             recomputed from the exact manifest bytes under test",
            entry.leaf_digest
        )));
    }
    if !ctx.known_logs.contains(&entry.log_id) {
        return Err(transparency_error(format!(
            "transparency entry is invalid: log id `{}` is not one of the trusted logs {:?}",
            entry.log_id, ctx.known_logs
        )));
    }
    if entry.observed_checkpoint_size < ctx.minimum_accepted_checkpoint_size {
        return Err(transparency_error(format!(
            "transparency entry is stale: observed checkpoint size {} is older than the trusted \
             minimum {}",
            entry.observed_checkpoint_size, ctx.minimum_accepted_checkpoint_size
        )));
    }
    Ok(())
}

/// The result of a fully successful [`verify_capsule`] call.
#[derive(Debug, Clone)]
pub struct CapsuleVerificationReport {
    pub profile: Profile,
    /// Ids of every retained object whose bytes were independently
    /// recomputed and matched.
    pub verified_object_ids: Vec<String>,
    /// `(object_id, object_type, redaction_reason)` for every redacted
    /// object -- exactly which required facts are unavailable, per issue
    /// #209's redaction requirement, rather than a green summary that
    /// silently omits them.
    pub unavailable_claims: Vec<(String, String, String)>,
}

/// Verifies one capsule end to end: structural/vocabulary validity, the
/// profile's required object-type set, the association graph, subject
/// bindings, per-object byte integrity, signature policy, and transparency
/// inclusion, in that order (fail-closed at the first failure).
///
/// Every parameter is an in-memory value; this function opens no file, no
/// socket, and never executes, decodes as executable, or publishes any
/// object it is handed.
pub fn verify_capsule(
    manifest_bytes: &[u8],
    object_bytes: &BTreeMap<String, Vec<u8>>,
    signature_ctx: &SignaturePolicyContext,
    transparency_ctx: &TransparencyContext,
) -> Result<CapsuleVerificationReport, Diagnostic> {
    let capsule = parse_capsule(manifest_bytes)?;
    check_required_object_types(&capsule)?;
    check_associations(&capsule)?;
    check_subject_bindings(&capsule)?;
    let verified_object_ids = check_object_bytes(&capsule, object_bytes)?;
    check_signature_policy(&capsule, signature_ctx)?;
    check_transparency(&capsule, manifest_bytes, transparency_ctx)?;
    let unavailable_claims = capsule
        .objects
        .iter()
        .filter(|candidate| candidate.redacted)
        .map(|candidate| {
            (
                candidate.id.clone(),
                candidate.object_type.clone(),
                candidate
                    .redaction_reason
                    .clone()
                    .unwrap_or_else(|| "no reason recorded".to_owned()),
            )
        })
        .collect();
    Ok(CapsuleVerificationReport {
        profile: capsule.profile,
        verified_object_ids,
        unavailable_claims,
    })
}

/// The capsule digest: the plain SHA-256 of the exact manifest bytes as
/// handed to this function -- an identity for one exact byte-for-byte
/// version of a capsule, distinct from [`transparency_leaf_digest`] (which
/// a [`TransparencyEntry::leaf_digest`] must equal instead; the two differ
/// because the transparency field itself cannot be part of what it commits
/// to -- see [`transparency_leaf_digest_bytes`]).
#[must_use]
pub fn capsule_digest(manifest_bytes: &[u8]) -> String {
    sha256_digest(manifest_bytes)
}

#[cfg(test)]
mod tests;
