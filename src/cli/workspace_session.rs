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
    let (frontend_cache, semantic_cache) = cache_policy(&policy)?;
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
    let archives = policy.get("candidate_archives").and_then(Value::as_array);
    if archives.is_some_and(|archives| !archives.is_empty()) && !capability.candidate_prepare {
        return Err(invalid(
            "startup candidate recovery requires candidate preparation",
        ));
    }
    let manifest: PathBuf = if manifest.is_absolute() {
        manifest.to_owned()
    } else {
        std::env::current_dir()
            .map_err(|_| invalid("cannot resolve host manifest working directory"))?
            .join(manifest)
    };
    let restored = policy
        .get("semantic_cache_entry")
        .filter(|entry| !entry.is_null());
    let mut session = if let Some(entry) = restored {
        let cache = semaprax::semantic_cache_store::load(
            Path::new(string(entry, "root")?),
            string(entry, "entry_digest")?,
        )?;
        VNextSession::open_with_retained_semantic_cache(&manifest, capability, cache)?
    } else if semantic_cache {
        VNextSession::open_with_semantic_cache(&manifest, capability)?
    } else if frontend_cache {
        VNextSession::open_with_frontend_cache(&manifest, capability)?
    } else {
        VNextSession::open(&manifest, capability)?
    };
    // Archives contain historical source and intentions, never host policy or
    // approval. Load completely before opening any deadline-bound Git provider.
    if let Some(archives) = archives {
        for archive in archives {
            let candidate = semaprax::candidate_archive_store::load(
                Path::new(string(archive, "root")?),
                string(archive, "archive_digest")?,
                string(archive, "candidate_digest")?,
            )?;
            session.retain_archived_candidate(candidate, string(archive, "candidate_digest")?)?;
        }
    }
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
// Preserve the closed v1 startup contract. Cache selection belongs to a new
// host policy, and never to a frame or a silently accepted extension field.
fn cache_policy(value: &Value) -> Result<(bool, bool), Vec<Diagnostic>> {
    const COMMON: &[&str] = &[
        "schema",
        "candidate_prepare",
        "diagnostics",
        "build_enabled",
        "test_policy",
        "git_commit",
    ];
    match value["schema"].as_str() {
        Some("semaprax.workspace-host-policy.v1") => {
            exact(value, COMMON)?;
            Ok((false, false))
        }
        Some("semaprax.workspace-host-policy.v2") => {
            let mut keys = COMMON.to_vec();
            keys.push("frontend_cache");
            exact(value, &keys)?;
            Ok((boolean(value, "frontend_cache")?, false))
        }
        Some(
            "semaprax.workspace-host-policy.v3"
            | "semaprax.workspace-host-policy.v4"
            | "semaprax.workspace-host-policy.v5",
        ) => {
            let persistent_policy = value["schema"] == "semaprax.workspace-host-policy.v5";
            let semantic_policy =
                persistent_policy || value["schema"] == "semaprax.workspace-host-policy.v4";
            let mut keys = COMMON.to_vec();
            keys.extend(["frontend_cache", "candidate_archives"]);
            if semantic_policy {
                keys.push("semantic_cache");
            }
            if persistent_policy {
                keys.push("semantic_cache_entry");
            }
            exact(value, &keys)?;
            let archives = value["candidate_archives"]
                .as_array()
                .ok_or_else(|| invalid("startup candidate archives must be an array"))?;
            if archives.len() > 16 {
                return Err(invalid(
                    "startup candidate archive inventory exceeds its bound",
                ));
            }
            let mut selected = std::collections::BTreeSet::new();
            for archive in archives {
                exact(archive, &["root", "archive_digest", "candidate_digest"])?;
                let root = string(archive, "root")?;
                let digest = string(archive, "archive_digest")?;
                let candidate = string(archive, "candidate_digest")?;
                if !Path::new(root).is_absolute() || !selected.insert(candidate) {
                    return Err(invalid(
                        "startup archive roots must be absolute and candidates unique",
                    ));
                }
                for digest in [digest, candidate] {
                    if digest.len() != 71
                        || !digest.starts_with("sha256:")
                        || !digest.as_bytes()[7..]
                            .iter()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                    {
                        return Err(invalid(
                            "startup archive selectors require canonical SHA256 digests",
                        ));
                    }
                }
            }
            let frontend = boolean(value, "frontend_cache")?;
            let semantic = if semantic_policy {
                boolean(value, "semantic_cache")?
            } else {
                false
            };
            if semantic && !frontend {
                return Err(invalid(
                    "semantic cache requires the explicit frontend cache selection",
                ));
            }
            if persistent_policy && !value["semantic_cache_entry"].is_null() {
                let entry = &value["semantic_cache_entry"];
                exact(entry, &["root", "entry_digest"])?;
                let digest = string(entry, "entry_digest")?;
                if !semantic
                    || !Path::new(string(entry, "root")?).is_absolute()
                    || digest.len() != 71
                    || !digest.starts_with("sha256:")
                    || !digest.as_bytes()[7..]
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
                {
                    return Err(invalid("persistent semantic cache requires semantic mode, an absolute root and canonical SHA256 selector"));
                }
            }
            Ok((frontend, semantic))
        }
        _ => Err(invalid("unknown workspace host policy schema")),
    }
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
