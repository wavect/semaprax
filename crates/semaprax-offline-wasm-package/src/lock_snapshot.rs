use std::path::{Path, PathBuf};

use semaprax::package_resolution_snapshot::{self, ResolutionSnapshot};
use semaprax::package_resolver::VerifiedResolution;

use crate::{validate_output_path, CompilerReplayFailure, PublicationError};

pub const INPUT_FILE: &str = "semaprax.package-resolution-input.json";
pub const RESOLUTION_FILE: &str = "semaprax.package-resolution.evidence.json";
pub const LOCK_FILE: &str = "semaprax.lock.json";

#[derive(Debug)]
pub struct PublishedOfflinePackageLockSnapshot {
    pub output: PathBuf,
    pub verified: VerifiedResolution,
}

/// Publishes one completely replayed semantic lock snapshot into one fresh,
/// fixed-inventory directory. Snapshot evidence never carries authority.
///
/// The host must exclude all uncooperative namespace/content mutation of the
/// destination, parent, complete ancestor chain, and stage for the invocation.
/// On Unix/macOS the parent must additionally be current-euid-owned with exact
/// mode 0700; Darwin ACL authority remains a host precondition. These are the
/// existing publisher's coordination requirements, not an advisory-lock,
/// hermetic-sandbox, mutable-lockfile, or cache guarantee.
pub fn publish_lock_snapshot(
    output: &Path,
    snapshot: ResolutionSnapshot,
) -> Result<PublishedOfflinePackageLockSnapshot, PublicationError> {
    validate_output_path(output)?;
    let output = output.to_path_buf();
    let mut verifier =
        || package_resolution_snapshot::verify(&snapshot).map_err(CompilerReplayFailure::from);
    let verified = verifier().map_err(PublicationError::replay)?;
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    {
        crate::authority::publish_lock_snapshot_verified(&output, &snapshot, &mut verifier)?;
        Ok(PublishedOfflinePackageLockSnapshot { output, verified })
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = (output, verified);
        Err(PublicationError::plain(
            crate::PP_INVALID,
            "offline lock snapshot publication is unsupported on this platform",
        ))
    }
}
