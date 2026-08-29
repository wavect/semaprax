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

pub struct PublishedOfflinePackageBuild {
    pub output: PathBuf,
    pub verified: VerifiedOfflinePackageBuild,
}

/// Independently replays `build` before acquiring filesystem authority and a
/// second time immediately before the no-replace publication attempt.
pub fn publish(
    output: &Path,
    build: OfflinePackageBuild,
    resolution_evidence: String,
    resolution_input: ResolutionInput,
    resolution_options: ResolutionOptions,
    build_options: OfflinePackageBuildOptions,
) -> Result<PublishedOfflinePackageBuild, PublicationError> {
    validate_output_path(output)?;
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
        authority::publish_verified(output, &build, &mut verifier)?;
        Ok(PublishedOfflinePackageBuild {
            output: output.to_path_buf(),
            verified,
        })
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (build, verifier, verified);
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
            validate_output_path(Path::new("/"))
                .unwrap_err()
                .code,
            PP_INVALID
        );
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
            message: format!("compiler replay rejected package build: {}", failure.message),
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
}
