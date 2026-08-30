//! Startup-selected Git authority and a separate, host-only approval slot.
//! Requests choose retained semantic candidates, never publication policy.
use super::super::{Method, Operation, Parameter, ParameterKind, REVISION};
use super::Action;
use crate::diagnostic::Diagnostic;
use crate::project::{
    apply_candidate_git_publication, CandidateGitAuthority, CandidateGitCommitMetadata,
    CandidateGitObject, CandidateGitObjectKind, CandidateGitRefUpdate, CandidateGitRepository,
    CandidateGitTarget, ProjectCandidate,
};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
const MAX_RECEIPT_BYTES: usize = 1024 * 1024;
const MAX_CHUNK_BYTES: usize = 32_768;
pub const SOURCE_COMMIT_HANDLE_SCHEMA: &str = "semaprax.image-source-commit-handle.v1";
const APPROVAL_DOMAIN: &[u8] = b"semaprax.image-source-commit.host-approval.v1\0";
const RECEIPT_DOMAIN: &[u8] = b"semaprax.image-source-commit.receipt.v1\0";

const METHODS: &[Method] = &[
    Method {
        name: "candidate/commit",
        operation: Operation::VNext(Action::Commit),
        parameters: &[
            REVISION,
            Parameter {
                name: "candidate_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "approval_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
        ],
        query: false,
        payload_schema: SOURCE_COMMIT_HANDLE_SCHEMA,
    },
    Method {
        name: "source-commit/status",
        operation: Operation::VNext(Action::Commit),
        parameters: &[REVISION],
        query: true,
        payload_schema: "semaprax.image-source-commit-status.v1",
    },
    Method {
        name: "candidate/commit-report",
        operation: Operation::VNext(Action::Commit),
        parameters: &[
            REVISION,
            Parameter {
                name: "report_revision",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "offset",
                kind: ParameterKind::Integer(0, MAX_RECEIPT_BYTES),
                required: false,
            },
            Parameter {
                name: "chunk_bytes",
                kind: ParameterKind::Integer(1, MAX_CHUNK_BYTES),
                required: false,
            },
        ],
        query: true,
        payload_schema: "semaprax.image-source-commit-report-chunk.v1",
    },
];
pub(super) fn methods() -> &'static [Method] {
    METHODS
}

struct OwnedAuthority(Box<dyn CandidateGitAuthority>);
impl CandidateGitAuthority for OwnedAuthority {
    fn repository(&self) -> io::Result<CandidateGitRepository> {
        self.0.repository()
    }
    fn read_ref(&mut self, reference: &str) -> io::Result<Option<String>> {
        self.0.read_ref(reference)
    }
    fn read_object(&mut self, oid: &str, max_bytes: usize) -> io::Result<CandidateGitObject> {
        self.0.read_object(oid, max_bytes)
    }
    fn write_object(
        &mut self,
        kind: CandidateGitObjectKind,
        bytes: &[u8],
        expected_oid: &str,
    ) -> io::Result<()> {
        self.0.write_object(kind, bytes, expected_oid)
    }
    fn compare_and_swap_ref(
        &mut self,
        reference: &str,
        expected_old: &str,
        new_commit: &str,
    ) -> io::Result<CandidateGitRefUpdate> {
        self.0
            .compare_and_swap_ref(reference, expected_old, new_commit)
    }
}
struct Approval {
    candidate: String,
    revision: String,
}
#[derive(Clone, Copy)]
enum State {
    Available,
    Published,
    Uncertain,
}
impl State {
    fn name(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Published => "published",
            Self::Uncertain => "publication_uncertain",
        }
    }
}

/// An independently granted startup capability. Neither its fixed policy nor
/// its approval slot is constructible or writable through the request protocol.
/// The boxed provider is a trusted host authority, not an agent-supplied object.
pub struct GitCommitHost {
    manifest: PathBuf,
    repository_identity: String,
    target: CandidateGitTarget,
    metadata: CandidateGitCommitMetadata,
    authority: OwnedAuthority,
    approval: Option<Approval>,
    sequence: u64,
    state: State,
    receipt: Option<(String, String)>,
    last_error_codes: Vec<String>,
}
impl GitCommitHost {
    pub fn new(
        manifest: &Path,
        target: CandidateGitTarget,
        metadata: CandidateGitCommitMetadata,
        authority: Box<dyn CandidateGitAuthority>,
    ) -> Result<Self> {
        let manifest_text = manifest
            .to_str()
            .ok_or_else(|| failure("SPX-G284", "Git commit manifest must be UTF8"))?;
        if !manifest.is_absolute()
            || manifest.file_name().and_then(|name| name.to_str()) != Some("semaprax.toml")
            || manifest_text.len() > 4096
            || manifest
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(failure(
                "SPX-G284",
                "Git commit host requires a bounded absolute Project manifest",
            ));
        }
        let repository = authority
            .repository()
            .map_err(|_| failure("SPX-G284", "cannot inspect startup Git authority"))?;
        if !repository.bare || repository.identity.is_empty() || repository.identity.len() > 4096 {
            return Err(failure(
                "SPX-G284",
                "Git commit host requires a bounded bare repository identity",
            ));
        }
        Ok(Self {
            manifest: manifest.to_path_buf(),
            repository_identity: repository.identity,
            target,
            metadata,
            authority: OwnedAuthority(authority),
            approval: None,
            sequence: 0,
            state: State::Available,
            receipt: None,
            last_error_codes: Vec::new(),
        })
    }
    /// Trusted host call only. A digest in a request is never sufficient to
    /// create this slot. The returned binding is public correlation, not a secret.
    pub fn approve(&mut self, candidate_digest: &str) -> Result<String> {
        digest(candidate_digest)?;
        if self.is_terminal() {
            return Err(failure("SPX-G287","Git commit session is terminal; inspect its publication before creating another host"));
        }
        if self.approval.is_some() {
            return Err(failure("SPX-G284", "a host approval is already pending"));
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| failure("SPX-G285", "Git approval sequence exhausted"))?;
        let sequence = self.sequence.to_be_bytes();
        let revision = hash(
            APPROVAL_DOMAIN,
            &[
                self.manifest
                    .to_str()
                    .expect("validated UTF8 manifest")
                    .as_bytes(),
                self.repository_identity.as_bytes(),
                candidate_digest.as_bytes(),
                &sequence,
            ],
        );
        self.approval = Some(Approval {
            candidate: candidate_digest.to_owned(),
            revision: revision.clone(),
        });
        Ok(revision)
    }
    pub fn is_terminal(&self) -> bool {
        !matches!(self.state, State::Available)
    }
    pub fn manifest(&self) -> &Path {
        &self.manifest
    }
    /// Host-state observation only, not current source or repository admission.
    pub fn status(&self) -> Value {
        json!({"schema":"semaprax.image-source-commit-status.v1","capability":"source_commit","authority":"startup_fixed_host_git_policy","state":self.state.name(),"pending_approval":self.approval.as_ref().map(|approval|json!({"candidate_revision":approval.candidate,"approval_revision":approval.revision})),"report_revision":self.receipt.as_ref().map(|(revision,_)|revision),"last_error_codes":self.last_error_codes,"approval_via_request":false,"raw_working_tree_write":false,"host_state_only":true})
    }
    /// Framework must first authenticate the held request and select a retained
    /// candidate whose original base equals its held Project revision. It must
    /// not wrap this call in a generic final source check after the Git pivot.
    pub(super) fn execute(
        &mut self,
        candidate: &Arc<ProjectCandidate>,
        manifest: &Path,
        params: &Map<String, Value>,
    ) -> Result<Value> {
        if self.is_terminal() {
            return Err(failure(
                "SPX-G287",
                "Git publication is terminal; inspect host status instead of retrying",
            ));
        }
        if manifest != self.manifest {
            return Err(failure(
                "SPX-G284",
                "Git host belongs to a different fixed Project manifest",
            ));
        }
        let requested = params
            .get("candidate_revision")
            .and_then(Value::as_str)
            .ok_or_else(|| failure("SPX-G284", "candidate revision is required"))?;
        let approval_revision = params
            .get("approval_revision")
            .and_then(Value::as_str)
            .ok_or_else(|| failure("SPX-G284", "separate host approval binding is required"))?;
        if requested != candidate.candidate_digest()
            || !self.approval.as_ref().is_some_and(|approval| {
                approval.candidate == requested && approval.revision == approval_revision
            })
        {
            return Err(failure(
                "SPX-G286",
                "candidate has no matching independently granted host approval",
            ));
        }
        // Exactly one application attempt consumes approval, including failures
        // before the pivot. Retrying needs another explicit host approval.
        let approval = self.approval.take().expect("matched pending host approval");
        self.last_error_codes.clear();
        let result = apply_candidate_git_publication(
            candidate,
            &approval.candidate,
            &self.manifest,
            &self.target,
            &self.metadata,
            &mut self.authority,
        );
        match result {
            Ok(receipt) => {
                self.state = State::Published;
                if receipt.len() > MAX_RECEIPT_BYTES {
                    self.state = State::Uncertain;
                    return Err(failure("SPX-G267","Git publication returned but its bounded receipt contract disagreed; inspect the fixed ref and do not retry"));
                }
                let report_revision = hash(RECEIPT_DOMAIN, &[receipt.as_bytes()]);
                let bytes = receipt.len();
                self.receipt = Some((report_revision.clone(), receipt));
                // Fixed-size compact handle: full receipts are never embedded
                // in a response whose escaping could overflow after publication.
                Ok(
                    json!({"schema":SOURCE_COMMIT_HANDLE_SCHEMA,"state":"published","candidate_revision":approval.candidate,"approval_revision":approval.revision,"report_revision":report_revision,"report_bytes":bytes,"receipt_method":"candidate/commit-report","raw_working_tree_write":false,"source_commit_authority":"startup_fixed_host_git_policy"}),
                )
            }
            Err(errors) => {
                self.last_error_codes = errors
                    .iter()
                    .take(8)
                    .map(|error| error.code.chars().take(32).collect())
                    .collect();
                if errors.iter().any(|error| error.code == "SPX-G267") {
                    self.state = State::Uncertain;
                }
                Err(errors)
            }
        }
    }
    pub(super) fn report(&self, params: &Map<String, Value>) -> Result<Value> {
        let (revision, report) = self.receipt.as_ref().ok_or_else(|| {
            failure(
                "SPX-G286",
                "no successful Git publication receipt is retained",
            )
        })?;
        if params.get("report_revision").and_then(Value::as_str) != Some(revision.as_str()) {
            return Err(failure(
                "SPX-G286",
                "Git receipt revision is stale or unknown",
            ));
        }
        let offset = number(params, "offset", 0)?;
        let chunk = number(params, "chunk_bytes", 16_384)?;
        if offset > report.len()
            || !report.is_char_boundary(offset)
            || !(1..=MAX_CHUNK_BYTES).contains(&chunk)
        {
            return Err(failure(
                "SPX-G285",
                "Git receipt chunk selection exceeds its bounds",
            ));
        }
        let mut end = offset.saturating_add(chunk).min(report.len());
        while !report.is_char_boundary(end) {
            end -= 1;
        }
        if end == offset && offset < report.len() {
            return Err(failure(
                "SPX-G285",
                "Git receipt chunk cannot hold the next UTF8 character",
            ));
        }
        Ok(
            json!({"schema":"semaprax.image-source-commit-report-chunk.v1","report_revision":revision,"report_schema":crate::project::PROJECT_CANDIDATE_GIT_PUBLICATION_SCHEMA,"offset":offset,"total_bytes":report.len(),"chunk":&report[offset..end],"next_offset":(end<report.len()).then_some(end),"historical_publication_receipt":true,"current_source_admission":false}),
        )
    }
}
fn digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        Err(failure(
            "SPX-G284",
            "host approval requires an exact canonical candidate digest",
        ))
    } else {
        Ok(())
    }
}
fn hash(domain: &[u8], parts: &[&[u8]]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    for part in parts {
        hash.update((part.len() as u64).to_be_bytes());
        hash.update(part);
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
fn number(params: &Map<String, Value>, key: &str, default: usize) -> Result<usize> {
    match params.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| failure("SPX-G285", "Git receipt offset or chunk bound is invalid")),
    }
}
fn failure(code: &'static str, message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(code, message)]
}

#[cfg(test)]
mod tests;
