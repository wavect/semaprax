//! Exact Project Lock association for one already-derived ProgramRoot.
//!
//! The association delegates lock admission to Project Lock v1 and carries no
//! path, network, write, resolution, or publication authority.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{ProgramRoot, ProgramRootSegment, MAX_PROGRAM_ROOT_BYTES};
use crate::project::{
    verify_project_lock, ProjectSnapshot, MAX_PROJECT_LOCK_BYTES, PROJECT_LOCK_SCHEMA,
};

pub const PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA: &str =
    "semaprax.program-root.dependency-lock-association.v1";
pub const MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES: usize = 64 * 1024;

const ASSOCIATION_DOMAIN: &[u8] = b"semaprax.program-root.dependency-lock-association.digest.v1\0";
const LOCK_BYTES_DOMAIN: &[u8] =
    b"semaprax.program-root.dependency-lock-association.lock-bytes.digest.v1\0";
const NONCLAIMS: [&str; 4] = [
    "exact_project_lock_association_not_dependency_resolution",
    "no_registry_acquisition_cache_signature_or_provenance_claim",
    "no_filesystem_network_process_commit_or_publication_authority",
    "canonical_workspace_dependency_closure_digest_remains_a_distinct_component",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Immutable association between one ProgramRoot and exact admitted Project Lock bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRootDependencyLockAssociation {
    association_digest: String,
    program_root_digest: String,
    project_lock_digest: String,
    project_lock_bytes_digest: String,
    project_lock_bytes: String,
    json: String,
}

impl ProgramRootDependencyLockAssociation {
    pub fn derive(
        snapshot: &ProjectSnapshot,
        program_root: &ProgramRoot,
        expected_program_root_digest: &str,
        project_lock_bytes: &str,
    ) -> Result<Self> {
        validate_digest(expected_program_root_digest)?;
        if project_lock_bytes.len() > MAX_PROJECT_LOCK_BYTES {
            return Err(invalid(
                "ProgramRoot dependency lock exceeds the Project Lock byte limit",
            ));
        }
        let canonical = snapshot.canonical_workspace_revision()?;
        let expected_root = canonical.program_root()?;
        if expected_program_root_digest != program_root.program_root_digest()
            || program_root != &expected_root
        {
            return Err(stale(
                "ProgramRoot dependency lock association selected a stale ProgramRoot",
            ));
        }
        let verified = verify_project_lock(snapshot, project_lock_bytes)?;
        if verified.program_root() != snapshot.project_revision() {
            return Err(stale(
                "ProgramRoot dependency lock Project revision association is stale",
            ));
        }
        let project_lock_bytes_digest =
            framed_digest(LOCK_BYTES_DOMAIN, project_lock_bytes.as_bytes());
        let payload = json!({
            "canonical_workspace_dependency_lock_digest": canonical.dependency_lock_digest(),
            "canonical_workspace_revision": canonical.workspace_revision(),
            "limits": {
                "max_association_bytes": MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES,
                "max_program_root_bytes": MAX_PROGRAM_ROOT_BYTES,
                "max_project_lock_bytes": MAX_PROJECT_LOCK_BYTES,
            },
            "nonclaims": NONCLAIMS,
            "program_root_digest": program_root.program_root_digest(),
            "project_lock": {
                "bytes": project_lock_bytes.len(),
                "bytes_digest": project_lock_bytes_digest,
                "digest": verified.digest(),
                "program_root": verified.program_root(),
                "schema": PROJECT_LOCK_SCHEMA,
            },
            "project_revision": snapshot.project_revision(),
            "schema": PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA,
        });
        let identity_bytes = canonical_json(
            payload.clone(),
            MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES,
        )?;
        let association_digest = framed_digest(ASSOCIATION_DOMAIN, identity_bytes.as_bytes());
        let json = canonical_json(
            with_field(payload, "association_digest", json!(association_digest)),
            MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES,
        )?;
        Ok(Self {
            association_digest,
            program_root_digest: program_root.program_root_digest().to_owned(),
            project_lock_digest: verified.digest().to_owned(),
            project_lock_bytes_digest,
            project_lock_bytes: project_lock_bytes.to_owned(),
            json,
        })
    }

    pub fn replay(
        snapshot: &ProjectSnapshot,
        program_root: &ProgramRoot,
        expected_association_digest: &str,
        project_lock_bytes: &str,
        association_bytes: &[u8],
    ) -> Result<Self> {
        validate_digest(expected_association_digest)?;
        if association_bytes.len() > MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES {
            return Err(invalid(
                "ProgramRoot dependency lock association exceeds its byte limit",
            ));
        }
        let source = std::str::from_utf8(association_bytes)
            .map_err(|_| invalid("ProgramRoot dependency lock association is not UTF-8"))?;
        let value: Value = serde_json::from_str(source)
            .map_err(|_| invalid("ProgramRoot dependency lock association is not JSON"))?;
        if canonical_json(
            value.clone(),
            MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES,
        )?
        .as_bytes()
            != association_bytes
        {
            return Err(invalid(
                "ProgramRoot dependency lock association is not exact canonical JSON",
            ));
        }
        validate_wire(&value, project_lock_bytes)?;
        let derived = Self::derive(
            snapshot,
            program_root,
            program_root.program_root_digest(),
            project_lock_bytes,
        )?;
        if expected_association_digest != derived.association_digest()
            || association_bytes != derived.to_json().as_bytes()
        {
            return Err(stale(
                "ProgramRoot dependency lock association failed exact replay",
            ));
        }
        Ok(derived)
    }

    pub fn association_digest(&self) -> &str {
        &self.association_digest
    }
    pub fn program_root_digest(&self) -> &str {
        &self.program_root_digest
    }
    pub fn project_lock_digest(&self) -> &str {
        &self.project_lock_digest
    }
    pub fn project_lock_bytes_digest(&self) -> &str {
        &self.project_lock_bytes_digest
    }
    /// The exact already-admitted bytes retained for later context replay.
    pub fn project_lock_bytes(&self) -> &str {
        &self.project_lock_bytes
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }

    /// Project this exact association as an optional content-addressed
    /// ProgramRoot extension segment. It is never inserted into the default
    /// nine-segment ProgramRoot manifest.
    pub fn program_root_segment(&self) -> Result<ProgramRootSegment> {
        ProgramRootSegment::derive(
            "project_lock_association",
            PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA,
            self.association_digest(),
            self.to_json(),
        )
    }
}

impl ProgramRoot {
    /// Bind exact caller-supplied Project Lock bytes after ordinary lock verification.
    pub fn associate_dependency_lock(
        &self,
        snapshot: &ProjectSnapshot,
        expected_program_root_digest: &str,
        project_lock_bytes: &str,
    ) -> Result<ProgramRootDependencyLockAssociation> {
        ProgramRootDependencyLockAssociation::derive(
            snapshot,
            self,
            expected_program_root_digest,
            project_lock_bytes,
        )
    }
}

fn validate_wire(value: &Value, project_lock_bytes: &str) -> Result<()> {
    let object = exact_object(value, "ProgramRoot dependency lock association")?;
    exact_fields(
        object,
        &[
            "association_digest",
            "canonical_workspace_dependency_lock_digest",
            "canonical_workspace_revision",
            "limits",
            "nonclaims",
            "program_root_digest",
            "project_lock",
            "project_revision",
            "schema",
        ],
        "ProgramRoot dependency lock association",
    )?;
    if value["schema"] != PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_SCHEMA {
        return Err(invalid(
            "ProgramRoot dependency lock association schema is unsupported",
        ));
    }
    if value["limits"]
        != json!({
            "max_association_bytes": MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES,
            "max_program_root_bytes": MAX_PROGRAM_ROOT_BYTES,
            "max_project_lock_bytes": MAX_PROJECT_LOCK_BYTES,
        })
        || value["nonclaims"] != json!(NONCLAIMS)
    {
        return Err(invalid(
            "ProgramRoot dependency lock association fixed limits or nonclaims are invalid",
        ));
    }
    for key in [
        "association_digest",
        "canonical_workspace_dependency_lock_digest",
        "canonical_workspace_revision",
        "program_root_digest",
        "project_revision",
    ] {
        validate_digest(
            object[key]
                .as_str()
                .ok_or_else(|| invalid("ProgramRoot dependency lock digest is invalid"))?,
        )?;
    }
    let lock = exact_object(
        &value["project_lock"],
        "ProgramRoot dependency lock descriptor",
    )?;
    exact_fields(
        lock,
        &["bytes", "bytes_digest", "digest", "program_root", "schema"],
        "ProgramRoot dependency lock descriptor",
    )?;
    if lock["schema"] != PROJECT_LOCK_SCHEMA
        || lock["bytes"].as_u64() != Some(project_lock_bytes.len() as u64)
        || project_lock_bytes.len() > MAX_PROJECT_LOCK_BYTES
        || lock["bytes_digest"] != framed_digest(LOCK_BYTES_DOMAIN, project_lock_bytes.as_bytes())
        || lock["program_root"] != value["project_revision"]
    {
        return Err(invalid(
            "ProgramRoot dependency lock descriptor fixed fields are invalid",
        ));
    }
    for key in ["bytes_digest", "digest", "program_root"] {
        validate_digest(
            lock[key]
                .as_str()
                .ok_or_else(|| invalid("ProgramRoot dependency lock descriptor is invalid"))?,
        )?;
    }
    let submitted_lock: Value = serde_json::from_str(project_lock_bytes)
        .map_err(|_| invalid("ProgramRoot dependency lock retained bytes are not JSON"))?;
    if submitted_lock.get("schema") != Some(&json!(PROJECT_LOCK_SCHEMA))
        || submitted_lock.get("digest") != Some(&lock["digest"])
        || submitted_lock
            .get("payload")
            .and_then(|payload| payload.get("program_root"))
            != Some(&lock["program_root"])
    {
        return Err(invalid(
            "ProgramRoot dependency lock descriptor does not match the retained lock bytes",
        ));
    }
    let identity = framed_digest(
        ASSOCIATION_DOMAIN,
        canonical_json(
            without_field(value, "association_digest")?,
            MAX_PROGRAM_ROOT_DEPENDENCY_LOCK_ASSOCIATION_BYTES,
        )?
        .as_bytes(),
    );
    if value["association_digest"] != identity {
        return Err(invalid(
            "ProgramRoot dependency lock association digest is invalid",
        ));
    }
    Ok(())
}

fn canonical_json(mut value: Value, maximum: usize) -> Result<String> {
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("ProgramRoot dependency lock association cannot be rendered"))?;
    output.push('\n');
    if output.len() > maximum {
        return Err(invalid(
            "ProgramRoot dependency lock association exceeds its byte limit",
        ));
    }
    Ok(output)
}

fn with_field(value: Value, key: &str, field: Value) -> Value {
    let mut object = value
        .as_object()
        .expect("dependency lock association construction uses an object")
        .clone();
    object.insert(key.to_owned(), field);
    Value::Object(object)
}

fn without_field(value: &Value, key: &str) -> Result<Value> {
    let mut object = exact_object(value, "ProgramRoot dependency lock digest subject")?.clone();
    if object.remove(key).is_none() {
        return Err(invalid(
            "ProgramRoot dependency lock digest subject lacks its identity",
        ));
    }
    Ok(Value::Object(object))
}

fn exact_object<'a>(value: &'a Value, subject: &'static str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| invalid(subject))
}

fn exact_fields(object: &Map<String, Value>, fields: &[&str], subject: &'static str) -> Result<()> {
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid(subject));
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid("ProgramRoot dependency lock digest is invalid"));
    }
    Ok(())
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

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G550", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G551", message)]
}
