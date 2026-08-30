//! Explicit startup host policy for the unified semantic workspace protocol.
use semaprax::diagnostic::Diagnostic;
use semaprax::image_transport::{
    serve_vnext, GitCommitHost, VNextPolicy, VNextSession, VNextSessionFailure,
};
use semaprax::project::{
    CandidateGitCommitMetadata, CandidateGitProcessAuthority, CandidateGitTarget,
    CandidateTestPolicy,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) fn run(manifest: &Path, policy_path: &Path) -> Result<(), Vec<Diagnostic>> {
    let bytes =
        super::project_image::read_bounded(policy_path, 65536).map_err(|error| vec![error])?;
    let policy: Value = serde_json::from_slice(&bytes)
        .map_err(|_| invalid("workspace host policy must be bounded JSON"))?;
    exact(
        &policy,
        &[
            "schema",
            "candidate_prepare",
            "diagnostics",
            "build_enabled",
            "test_policy",
            "git_commit",
        ],
    )?;
    if policy["schema"] != "semaprax.workspace-host-policy.v1" {
        return Err(invalid("unknown workspace host policy schema"));
    }
    let test_policy = if policy["test_policy"].is_null() {
        None
    } else {
        let value = &policy["test_policy"];
        exact(
            value,
            &["max_steps", "max_execution_bytes", "max_report_bytes"],
        )?;
        Some(CandidateTestPolicy::new(
            size(value, "max_steps")?,
            size(value, "max_execution_bytes")?,
            size(value, "max_report_bytes")?,
        )?)
    };
    let capability = VNextPolicy {
        candidate_prepare: boolean(&policy, "candidate_prepare")?,
        diagnostics: boolean(&policy, "diagnostics")?,
        build_enabled: boolean(&policy, "build_enabled")?,
        test_policy,
    };
    let manifest: PathBuf = if manifest.is_absolute() {
        manifest.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|_| invalid("cannot resolve host manifest working directory"))?
            .join(manifest)
    };
    let mut session = VNextSession::open(&manifest, capability)?;
    if !policy["git_commit"].is_null() {
        if !capability.candidate_prepare {
            return Err(invalid(
                "source commit requires host-selected candidate preparation",
            ));
        }
        let git = &policy["git_commit"];
        exact(
            git,
            &[
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
                "approved_candidate_digest",
            ],
        )?;
        let metadata = CandidateGitCommitMetadata::new(
            string(git, "author_name")?,
            string(git, "author_email")?,
            integer(git, "unix_seconds")?,
            string(git, "message")?,
        )?;
        let authority = CandidateGitProcessAuthority::open(
            Path::new(string(git, "git_executable")?),
            Path::new(string(git, "repository")?),
            size(git, "max_commands")?,
            integer(git, "timeout_ms")?,
        )?;
        let target = CandidateGitTarget::new(
            authority.repository_identity(),
            string(git, "reference")?,
            string(git, "base_commit")?,
            string(git, "project_prefix")?,
        )?;
        let host = GitCommitHost::new(&manifest, target, metadata, Box::new(authority))?;
        session = session.with_git_commit_host(host)?;
        // This value comes from the trusted host startup file, never an RPC
        // argument or a candidate capsule. Its public correlation handle is
        // discoverable through source-commit/status; no secret token is implied.
        session.approve_git_commit(string(git, "approved_candidate_digest")?)?;
    }
    serve_vnext(std::io::stdin().lock(), std::io::stdout().lock(), session).map_err(|error| {
        if let Some(failure) = error
            .get_ref()
            .and_then(|source| source.downcast_ref::<VNextSessionFailure>())
        {
            failure.diagnostics().to_vec()
        } else {
            vec![Diagnostic::io(
                "SPX-G280",
                format!("workspace session ended: {error}"),
            )]
        }
    })
}
fn exact(value: &Value, keys: &[&str]) -> Result<(), Vec<Diagnostic>> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("host policy requires an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("host policy has missing or unknown fields"));
    }
    Ok(())
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, Vec<Diagnostic>> {
    value[key]
        .as_str()
        .ok_or_else(|| invalid("host policy text field is invalid"))
}
fn integer(value: &Value, key: &str) -> Result<u64, Vec<Diagnostic>> {
    value[key]
        .as_u64()
        .ok_or_else(|| invalid("host policy integer field is invalid"))
}
fn size(value: &Value, key: &str) -> Result<usize, Vec<Diagnostic>> {
    usize::try_from(integer(value, key)?).map_err(|_| invalid("host policy size exceeds this host"))
}
fn boolean(value: &Value, key: &str) -> Result<bool, Vec<Diagnostic>> {
    value[key]
        .as_bool()
        .ok_or_else(|| invalid("host policy capability must be boolean"))
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G280", message)]
}
