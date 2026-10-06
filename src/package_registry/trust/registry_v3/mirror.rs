//! Exact acquired-metadata composition. Downloaded bytes remain untrusted
//! until the ordinary Registry-v3 verifier authenticates them against the
//! independently installed root and durable checkpoint.

use super::*;
use crate::package_registry::mirror_transport::{MirrorBytes, MirrorObjectKind};
use std::collections::BTreeSet;

/// One exact publisher role and its caller-selected immutable metadata path.
pub struct MirrorPublisherPath<'a> {
    pub role: &'a str,
    pub path: &'a str,
}

/// Names the complete metadata set expected from one already-authorized mirror
/// acquisition. The sealed registry is supplied independently; remote bytes
/// cannot create or decode their way into this type.
pub struct MirrorMetadataPaths<'registry, 'path> {
    pub timestamp_path: &'path str,
    pub snapshot_path: &'path str,
    pub publishers: &'path [MirrorPublisherPath<'path>],
    pub registry: &'registry registry::RegistrySnapshotV3,
}

/// Bridge-local checkpoint state. It carries the ordinary Registry-v3
/// checkpoint plus the observation tied to the last *new* signed timestamp.
/// Callers must retain this whole value between mirror verifications; a raw
/// Registry-v3 checkpoint alone deliberately cannot reset mirror freshness.
#[derive(Clone)]
pub struct MirrorCheckpoint {
    checkpoint: RegistryCheckpoint,
    last_new_timestamp: Option<TimestampObservation>,
}

#[derive(Clone)]
struct TimestampObservation {
    version: u64,
    digest: String,
    observed_time: u64,
}

impl MirrorCheckpoint {
    /// Only for an independently authorized first install.
    pub fn initial(root: &InstalledRoot) -> Self {
        Self {
            checkpoint: RegistryCheckpoint::initial(root),
            last_new_timestamp: None,
        }
    }

    /// Bootstrap bridge state for a held store installed at this explicit
    /// trusted time. The host flow exact-compares its complete checkpoint wire
    /// with the live held receipt before allowing any network operation.
    pub fn initial_at(root: &InstalledRoot, bootstrap_trusted_time: u64) -> Self {
        let mut checkpoint = RegistryCheckpoint::initial(root);
        checkpoint.previous.observed_time = bootstrap_trusted_time;
        Self {
            checkpoint,
            last_new_timestamp: None,
        }
    }

    /// The ordinary checkpoint remains available for its existing durable
    /// trust boundary, but it is insufficient to resume mirror verification.
    pub fn registry_checkpoint(&self) -> &RegistryCheckpoint {
        &self.checkpoint
    }

    /// True only before any signed timestamp has been accepted through this
    /// bridge. This does not expose a way to reconstruct bridge state.
    pub fn is_initial(&self) -> bool {
        self.last_new_timestamp.is_none()
    }

    /// Reconstructs bridge state only from an authenticated held generation.
    /// This is crate-private so a caller cannot mint an old observation or
    /// detach it from the generation checkpoint that authenticated it.
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    pub(crate) fn from_held(
        checkpoint: RegistryCheckpoint,
        anchor: Option<(u64, String, u64)>,
    ) -> Self {
        Self {
            checkpoint,
            last_new_timestamp: anchor.map(|(version, digest, observed_time)| {
                TimestampObservation {
                    version,
                    digest,
                    observed_time,
                }
            }),
        }
    }

    /// The anchor is opaque outside the trust/host boundary. Its values are
    /// only serialized after the held generation independently binds them to
    /// the candidate's authenticated timestamp role. Like `from_held`, its
    /// only reader is the held store, which exists only on these hosts.
    #[cfg(any(target_os = "linux", target_os = "android", target_vendor = "apple"))]
    pub(crate) fn anchor(&self) -> Option<(u64, &str, u64)> {
        self.last_new_timestamp
            .as_ref()
            .map(|anchor| (anchor.version, anchor.digest.as_str(), anchor.observed_time))
    }
}

/// A non-authoritative mirror verification result. Its bridge checkpoint must
/// be retained with the ordinary candidate's proof before another mirror
/// verification; it grants no durable commit or fetch authority.
pub struct MirrorUpdateCandidate<'a> {
    candidate: RegistryUpdateCandidate<'a>,
    checkpoint: MirrorCheckpoint,
}

impl MirrorUpdateCandidate<'_> {
    pub fn checkpoint(&self) -> &MirrorCheckpoint {
        &self.checkpoint
    }
    pub fn prior_checkpoint_digest(&self) -> &str {
        self.candidate.prior_checkpoint_digest()
    }
    pub fn registry_snapshot_digest(&self) -> &str {
        self.candidate.registry_snapshot_digest()
    }
    pub fn check_lock(&self, lock: &str, subjects: &[String]) -> Result<()> {
        self.candidate.check_lock(lock, subjects)
    }
    pub fn check_artifact(
        &self,
        package: &str,
        version: &str,
        path: &str,
        bytes: &[u8],
    ) -> Result<()> {
        self.candidate.check_artifact(package, version, path, bytes)
    }
}

/// Replays acquired mirror metadata through the ordinary signature, snapshot,
/// publisher-manifest, revocation and checkpoint verifier. It never fetches,
/// writes, installs roots, or returns a durable/fetch capability.
pub fn verify_mirror_update<'registry>(
    root: &InstalledRoot,
    stored: &MirrorCheckpoint,
    now: u64,
    paths: &MirrorMetadataPaths<'registry, '_>,
    downloaded: &[MirrorBytes],
) -> Result<MirrorUpdateCandidate<'registry>> {
    // A first independently authorized installation has no timestamp stamp.
    // Afterwards, reject a mirror that has been offline beyond the explicit
    // Wavect freshness interval even if its old signed expiry was longer.
    if stored.last_new_timestamp.as_ref().is_some_and(|timestamp| {
        now.checked_sub(timestamp.observed_time)
            .filter(|elapsed| *elapsed <= MAX_MIRROR_OFFLINE_SECONDS)
            .is_none()
    }) {
        return Err(stale());
    }

    let mut expected_paths = BTreeSet::new();
    for path in std::iter::once(paths.timestamp_path)
        .chain(std::iter::once(paths.snapshot_path))
        .chain(paths.publishers.iter().map(|publisher| publisher.path))
    {
        if !expected_paths.insert(path) {
            return Err(binding());
        }
    }
    let mut roles = BTreeSet::new();
    if paths
        .publishers
        .iter()
        .any(|publisher| !roles.insert(publisher.role))
        || downloaded.len() != expected_paths.len()
    {
        return Err(binding());
    }
    for item in downloaded {
        if item.kind != MirrorObjectKind::Metadata || !expected_paths.contains(item.path.as_str()) {
            return Err(binding());
        }
    }
    let metadata = |path| {
        let matches = downloaded
            .iter()
            .filter(|item| item.path == path)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(binding());
        }
        std::str::from_utf8(matches[0].bytes()).map_err(|_| shape())
    };
    let timestamp = metadata(paths.timestamp_path)?;
    let snapshot = metadata(paths.snapshot_path)?;
    let publishers = paths
        .publishers
        .iter()
        .map(|publisher| Ok((publisher.role, metadata(publisher.path)?)))
        .collect::<Result<Vec<_>>>()?;
    let candidate = verify_update(
        root,
        &stored.checkpoint,
        now,
        &UpdateInputs {
            timestamp,
            snapshot,
            publishers: &publishers,
            registry: paths.registry,
        },
    )?;
    // Keep Trust-v2's invocation-time high-water intact. Separately preserve
    // the observation tied to this timestamp, so identical replay cannot move
    // the mirror-local freshness anchor forward.
    let timestamp = candidate
        .checkpoint()
        .previous
        .roles
        .get("timestamp")
        .ok_or_else(stale)?;
    let last_new_timestamp = match stored.last_new_timestamp.as_ref() {
        Some(previous)
            if previous.version == timestamp.version && previous.digest == timestamp.digest =>
        {
            previous.clone()
        }
        _ => TimestampObservation {
            version: timestamp.version,
            digest: timestamp.digest.clone(),
            observed_time: now,
        },
    };
    let checkpoint = MirrorCheckpoint {
        checkpoint: candidate.checkpoint().clone(),
        last_new_timestamp: Some(last_new_timestamp),
    };
    Ok(MirrorUpdateCandidate {
        candidate,
        checkpoint,
    })
}
