//! Explicit host-selected canonical Git publication. Never selected by a capsule.
use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    self, apply_candidate_git_publication, CandidateGitCommitMetadata,
    CandidateGitProcessAuthority, CandidateGitTarget, ProjectCandidate,
};
use serde_json::Value;

const MAX_POLICY_BYTES: usize = 65_536;

pub(crate) fn publish(
    manifest: &Path,
    capsule: &Path,
    approved_candidate: &str,
    policy_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    let bytes = super::project_image::read_bounded(policy_path, MAX_POLICY_BYTES)
        .map_err(|error| vec![error])?;
    let policy: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("Git host policy must be bounded JSON"))?;
    let object = policy
        .as_object()
        .ok_or_else(|| invalid("Git host policy must be an object"))?;
    const KEYS: &[&str] = &[
        "schema",
        "git_executable",
        "repository",
        "reference",
        "base_commit",
        "project_prefix",
        "author_name",
        "author_email",
        "unix_seconds",
        "message",
        "max_commands",
        "timeout_ms",
    ];
    if object.len() != KEYS.len()
        || KEYS.iter().any(|key| !object.contains_key(*key))
        || policy["schema"] != "semaprax.candidate-git-host-policy.v1"
    {
        return Err(invalid("Git host policy schema or exact fields differ"));
    }
    let text = |key: &str| {
        policy[key]
            .as_str()
            .ok_or_else(|| invalid("Git host policy text field is invalid"))
    };
    let number = |key: &str| {
        policy[key]
            .as_u64()
            .ok_or_else(|| invalid("Git host policy numeric field is invalid"))
    };
    let metadata = CandidateGitCommitMetadata::new(
        text("author_name")?,
        text("author_email")?,
        number("unix_seconds")?,
        text("message")?,
    )?;
    let max_commands = usize::try_from(number("max_commands")?)
        .map_err(|_| invalid("Git command bound is invalid"))?;
    let capsule = super::project_candidate::read_capsule(capsule).map_err(|error| vec![error])?;
    // End this read-only authority before entering publication. An outer final
    // recheck must not turn a published result into an ordinary read failure.
    let candidate = project::with_authenticated_project(manifest, |snapshot| {
        ProjectCandidate::restore(
            snapshot.retain_revision(),
            snapshot.project_revision(),
            &capsule,
        )
    })?;
    if approved_candidate != candidate.candidate_digest() {
        return Err(invalid(
            "Git publication requires the exact separately supplied candidate approval",
        ));
    }
    let mut authority = CandidateGitProcessAuthority::open(
        Path::new(text("git_executable")?),
        Path::new(text("repository")?),
        max_commands,
        number("timeout_ms")?,
    )?;
    let target = CandidateGitTarget::new(
        authority.repository_identity(),
        text("reference")?,
        text("base_commit")?,
        text("project_prefix")?,
    )?;
    apply_candidate_git_publication(
        &candidate,
        approved_candidate,
        manifest,
        &target,
        &metadata,
        &mut authority,
    )
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G263", message)]
}
