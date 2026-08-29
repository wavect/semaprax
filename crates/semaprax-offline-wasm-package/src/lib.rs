//! Safe create-new publication for effect-free Core-Wasm package builds.
//!
//! The compiler remains authority-free. This lower crate owns the only held
//! filesystem authority, independently replays the caller-owned build before
//! acquiring it, and never treats evidence or a verification receipt as a
//! publication token.

#![forbid(unsafe_code)]

use std::fmt;
use std::path::{Component, Path, PathBuf};

use semaprax::package_build::{
    self, OfflinePackageBuild, OfflinePackageBuildOptions, VerifiedOfflinePackageBuild,
};
use semaprax::package_resolver::{ResolutionInput, ResolutionOptions};

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod authority;

pub const MODULE_FILE: &str = "module.wasm";
pub const EVIDENCE_FILE: &str = "semaprax.package-build.evidence.json";
pub const MANIFEST_FILE: &str = "semaprax.package-build.json";

pub const PP_INVALID: &str = "SPX-PP501";
pub const PP_REPLAY: &str = "SPX-PP502";
pub const PP_EXISTS: &str = "SPX-PP503";
pub const PP_STAGE_EXHAUSTED: &str = "SPX-PP504";
pub const PP_CHANGED: &str = "SPX-PP505";
pub const PP_CLEANUP: &str = "SPX-PP506";
pub const PP_PUBLISHED_CHANGED: &str = "SPX-PP507";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationVisibility {
    NotPublished,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CleanupStatus {
    NotNeeded,
    Settled,
    Incomplete,
    SuppressedAfterPublicationAttempt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationError {
    pub code: &'static str,
    pub message: String,
    pub compiler_code: Option<&'static str>,
    pub primary_code: Option<&'static str>,
    pub visibility: PublicationVisibility,
    pub cleanup: CleanupStatus,
}

impl fmt::Display for PublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for PublicationError {}

#[derive(Debug)]
pub struct PublishedOfflinePackageBuild {
    pub output: PathBuf,
    pub verified: VerifiedOfflinePackageBuild,
}

/// Independently replays `build` before acquiring filesystem authority and a
/// second time immediately before the no-replace publication attempt.
///
/// On every platform the host must exclude all uncooperative namespace or
/// content mutation of the destination path, its parent, and its ancestor chain
/// for the complete invocation; held parent-relative checks cannot prove that
/// an absolute ancestor path was not concurrently rebound. On Unix/macOS the
/// destination parent must additionally be current-euid-owned with exact mode
/// 0700. POSIX directory creation cannot atomically return the created directory
/// handle, and Darwin ACL authority is outside the mechanical mode-bit check.
/// These coordination requirements are part of the v1 authority contract, not
/// an advisory lock guarantee.
pub fn publish(
    output: &Path,
    build: OfflinePackageBuild,
    resolution_evidence: String,
    resolution_input: ResolutionInput,
    resolution_options: ResolutionOptions,
    build_options: OfflinePackageBuildOptions,
) -> Result<PublishedOfflinePackageBuild, PublicationError> {
    validate_output_path(output)?;
    // Own every success-result allocation before replay can acquire and publish
    // through filesystem authority. The post-rename success path only moves
    // already-owned state.
    let output = output.to_path_buf();
    let mut verifier = |candidate: &OfflinePackageBuild| {
        package_build::verify(
            candidate,
            &resolution_evidence,
            &resolution_input,
            &resolution_options,
            &build_options,
        )
        .map_err(CompilerReplayFailure::from)
    };
    let verified = verifier(&build).map_err(PublicationError::replay)?;

    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    {
        authority::publish_verified(&output, &build, &mut verifier)?;
        Ok(PublishedOfflinePackageBuild { output, verified })
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (output, build, verifier, verified);
        Err(PublicationError::plain(
            PP_INVALID,
            "offline package publication is unsupported on this platform",
        ))
    }
}

fn validate_output_path(output: &Path) -> Result<(), PublicationError> {
    if !output.is_absolute()
        || output.file_name().is_none()
        || output.parent().is_none()
        || output
            .components()
            .any(|part| matches!(part, Component::CurDir | Component::ParentDir))
    {
        return Err(PublicationError::plain(
            PP_INVALID,
            "publication output must be an absolute normalized child path",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use semaprax::package_resolver::Requirement;

    #[test]
    fn destination_must_be_absolute_normalized_and_have_a_leaf() {
        assert_eq!(
            validate_output_path(Path::new("relative/package"))
                .unwrap_err()
                .code,
            PP_INVALID
        );
        assert_eq!(
            validate_output_path(Path::new("/tmp/../package"))
                .unwrap_err()
                .code,
            PP_INVALID
        );
        assert_eq!(
            validate_output_path(Path::new("/")).unwrap_err().code,
            PP_INVALID
        );
    }

    #[test]
    fn compiler_replay_rejection_precedes_destination_parent_authority() {
        let parent = std::env::temp_dir().join(format!(
            "semaprax-publisher-ordering-missing-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let error = publish(
            &parent.join("package"),
            OfflinePackageBuild {
                module_wasm: Vec::new(),
                manifest_json: String::new(),
                evidence_json: String::new(),
            },
            String::new(),
            ResolutionInput {
                requirements: vec![Requirement {
                    package: "missing".to_owned(),
                    range: "=1.0.0".to_owned(),
                }],
                subjects: Vec::new(),
                target: "wasm32".to_owned(),
                allowed_capabilities: Vec::new(),
            },
            ResolutionOptions::default(),
            OfflinePackageBuildOptions {
                root_package: "missing".to_owned(),
                exports: vec!["fn:missing".to_owned()],
                max_artifact_bytes: 4 * 1024,
                max_evidence_bytes: 4 * 1024,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, PP_REPLAY);
        assert_eq!(error.cleanup, CleanupStatus::NotNeeded);
        assert!(!parent.exists());
    }

    #[test]
    fn unheld_created_namespace_is_explicitly_cleanup_incomplete() {
        let error = PublicationError::unheld_namespace(PublicationError::plain(
            PP_CHANGED,
            "create held staging directory",
        ));
        assert_eq!(error.code, PP_CLEANUP);
        assert_eq!(error.primary_code, Some(PP_CHANGED));
        assert_eq!(error.visibility, PublicationVisibility::NotPublished);
        assert_eq!(error.cleanup, CleanupStatus::Incomplete);
    }
}

struct CompilerReplayFailure {
    code: &'static str,
    message: String,
}

impl From<semaprax::diagnostic::Diagnostic> for CompilerReplayFailure {
    fn from(value: semaprax::diagnostic::Diagnostic) -> Self {
        Self {
            code: value.code,
            message: value.to_string(),
        }
    }
}

impl PublicationError {
    fn plain(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            compiler_code: None,
            primary_code: None,
            visibility: PublicationVisibility::NotPublished,
            cleanup: CleanupStatus::NotNeeded,
        }
    }

    fn replay(failure: CompilerReplayFailure) -> Self {
        Self {
            code: PP_REPLAY,
            message: format!(
                "compiler replay rejected package build: {}",
                failure.message
            ),
            compiler_code: Some(failure.code),
            primary_code: None,
            visibility: PublicationVisibility::NotPublished,
            cleanup: CleanupStatus::NotNeeded,
        }
    }

    fn cleanup_incomplete(mut primary: Self) -> Self {
        let primary_code = primary.code;
        primary.code = PP_CLEANUP;
        primary.message = format!(
            "{}; exact authenticated stage cleanup did not settle",
            primary.message
        );
        primary.primary_code = Some(primary_code);
        primary.cleanup = CleanupStatus::Incomplete;
        primary
    }

    fn unheld_namespace(primary: Self) -> Self {
        let primary_code = primary.code;
        Self {
            code: PP_CLEANUP,
            message: format!(
                "{}; create-new staging may have produced a namespace entry without returning authenticated cleanup authority",
                primary.message
            ),
            compiler_code: primary.compiler_code,
            primary_code: Some(primary_code),
            visibility: PublicationVisibility::NotPublished,
            cleanup: CleanupStatus::Incomplete,
        }
    }

    fn suppressed_after_attempt(mut primary: Self) -> Self {
        primary.cleanup = CleanupStatus::SuppressedAfterPublicationAttempt;
        primary
    }
}
