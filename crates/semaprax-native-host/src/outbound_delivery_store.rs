//! Caller-authorized, immutable local storage for typed outbound session checkpoints.
//!
//! The caller supplies an already held directory. This module never resolves a
//! path from configuration and never creates a directory or grants authority.

use std::ffi::OsStr;

use semaprax::outbound_host_adapter::{
    CheckpointCommit, EmailDeliverySessionCheckpoint, EmailDeliverySessionCheckpointStore,
    HttpDeliverySessionCheckpoint, HttpDeliverySessionCheckpointStore,
    WebhookDeliverySessionCheckpoint, WebhookDeliverySessionCheckpointStore,
};
use semaprax_native_rust_interop_platform as platform;
use semaprax_native_rust_interop_platform::HeldDirectory;

/// The first non-test caller for this store: a bounded, host-authorized
/// wiring from a decoded service outbound declaration to a real durable
/// typed delivery session.
pub mod service_invocation;

/// The typed checkpoint family is part of the on-disk namespace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboundCheckpointKind {
    HttpSession,
    WebhookSession,
    EmailSession,
}

impl OutboundCheckpointKind {
    const fn label(self) -> &'static str {
        match self {
            Self::HttpSession => "http",
            Self::WebhookSession => "webhook",
            Self::EmailSession => "email",
        }
    }
}

/// A fail-closed read refusal. The underlying path or OS error is omitted so
/// callers do not accidentally turn storage diagnostics into authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboundCheckpointReadRefusal {
    InvalidDigest,
    StorageUnavailable,
}

/// Selects the local acknowledgment boundary, not filesystem authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutboundCheckpointSyncMode {
    /// Sync checkpoint contents only (the unchanged default).
    FileOnly,
    /// Also sync the caller-held directory before the final named recheck.
    /// Unsupported platforms or sync failures leave the commit uncertain.
    NamespaceSynced,
}

/// A bounded file store rooted at a directory held by its caller.
///
/// A fresh directory capability is required after process restart. The host
/// retains the checkpoint digest alongside its own authenticated reference;
/// the store intentionally does not enumerate or select a "latest" record.
pub struct OutboundDeliveryStore<'directory> {
    directory: &'directory HeldDirectory,
    sync_mode: OutboundCheckpointSyncMode,
}

impl<'directory> OutboundDeliveryStore<'directory> {
    /// Bind the store to a directory authority already acquired by the caller.
    pub fn new(directory: &'directory HeldDirectory) -> Self {
        Self::with_sync_mode(directory, OutboundCheckpointSyncMode::FileOnly)
    }

    /// Select an acknowledgment boundary using independently acquired authority.
    /// This does not provision storage or derive authority from configuration.
    pub fn with_sync_mode(
        directory: &'directory HeldDirectory,
        sync_mode: OutboundCheckpointSyncMode,
    ) -> Self {
        Self {
            directory,
            sync_mode,
        }
    }

    /// Read one exact typed checkpoint by its previously retained digest.
    ///
    /// Returned bytes are data, not authority. The caller must pass them to
    /// the matching SEMAPRAX authenticated restore API with a separately
    /// granted exact-digest/capacity capability.
    pub fn load(
        &self,
        kind: OutboundCheckpointKind,
        digest: &str,
    ) -> Result<Vec<u8>, OutboundCheckpointReadRefusal> {
        let name = checkpoint_filename(kind, digest)
            .ok_or(OutboundCheckpointReadRefusal::InvalidDigest)?;
        platform::recheck_directory(self.directory)
            .map_err(|_| OutboundCheckpointReadRefusal::StorageUnavailable)?;
        let file = platform::hold_regular_file_bounded(
            self.directory,
            OsStr::new(&name),
            MAX_CHECKPOINT_BYTES,
        )
        .map_err(|_| OutboundCheckpointReadRefusal::StorageUnavailable)?;
        let bytes = platform::read_exact(&file, MAX_CHECKPOINT_BYTES)
            .map_err(|_| OutboundCheckpointReadRefusal::StorageUnavailable)?;
        platform::recheck_regular_file(&file)
            .map_err(|_| OutboundCheckpointReadRefusal::StorageUnavailable)?;
        Ok(bytes)
    }

    fn commit_rendered(
        &mut self,
        kind: OutboundCheckpointKind,
        digest: &str,
        rendered: &str,
    ) -> CheckpointCommit {
        self.commit_rendered_with(
            kind,
            digest,
            rendered,
            platform::write_file_new,
            platform::sync_regular_file,
        )
    }

    fn commit_rendered_with<W, S>(
        &mut self,
        kind: OutboundCheckpointKind,
        digest: &str,
        rendered: &str,
        write_new: W,
        sync_file: S,
    ) -> CheckpointCommit
    where
        W: FnOnce(
            &HeldDirectory,
            &OsStr,
            &[u8],
            u32,
        ) -> Result<platform::HeldRegularFile, platform::Error>,
        S: Fn(&platform::HeldRegularFile) -> Result<(), platform::Error>,
    {
        self.commit_rendered_with_directory_sync(
            kind,
            digest,
            rendered,
            write_new,
            sync_file,
            platform::sync_directory,
        )
    }

    fn commit_rendered_with_directory_sync<W, S, D>(
        &mut self,
        kind: OutboundCheckpointKind,
        digest: &str,
        rendered: &str,
        write_new: W,
        sync_file: S,
        sync_directory: D,
    ) -> CheckpointCommit
    where
        W: FnOnce(
            &HeldDirectory,
            &OsStr,
            &[u8],
            u32,
        ) -> Result<platform::HeldRegularFile, platform::Error>,
        S: Fn(&platform::HeldRegularFile) -> Result<(), platform::Error>,
        D: Fn(&HeldDirectory) -> Result<(), platform::Error>,
    {
        let Some(name) = checkpoint_filename(kind, digest) else {
            return CheckpointCommit::NotCommitted;
        };
        let bytes = rendered.as_bytes();
        if bytes.is_empty() || bytes.len() > MAX_CHECKPOINT_BYTES {
            return CheckpointCommit::NotCommitted;
        }
        if platform::recheck_directory(self.directory).is_err() {
            return CheckpointCommit::Uncertain;
        }
        match write_new(self.directory, OsStr::new(&name), bytes, 0o600) {
            // Bind ACK to the current namespace entry rather than trusting the
            // writer's descriptor, which may not be the file now at `name`.
            Ok(_) | Err(platform::Error::Exists) => {
                let existing = platform::hold_regular_file_bounded_for_sync(
                    self.directory,
                    OsStr::new(&name),
                    MAX_CHECKPOINT_BYTES,
                );
                let Ok(existing) = existing else {
                    return CheckpointCommit::Uncertain;
                };
                match platform::read_exact(&existing, MAX_CHECKPOINT_BYTES) {
                    Ok(existing_bytes) if existing_bytes == bytes => {}
                    Ok(_) | Err(_) => return CheckpointCommit::Uncertain,
                }
                if sync_file(&existing).is_err()
                    || (self.sync_mode == OutboundCheckpointSyncMode::NamespaceSynced
                        && sync_directory(self.directory).is_err())
                    || platform::recheck_regular_file_named_bounded(
                        self.directory,
                        OsStr::new(&name),
                        &existing,
                        MAX_CHECKPOINT_BYTES,
                    )
                    .is_err()
                {
                    return CheckpointCommit::Uncertain;
                }
                CheckpointCommit::Committed
            }
            // A failed create may have become visible before the OS reported
            // its failure, so no I/O failure is treated as definite absence.
            Err(_) => CheckpointCommit::Uncertain,
        }
    }
}

const MAX_CHECKPOINT_BYTES: usize = 192 * 1024;

fn checkpoint_filename(kind: OutboundCheckpointKind, digest: &str) -> Option<String> {
    let hex = digest.strip_prefix("sha256:")?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    Some(format!("outbound-{}-{}.checkpoint", kind.label(), hex))
}

impl HttpDeliverySessionCheckpointStore for OutboundDeliveryStore<'_> {
    fn commit(&mut self, checkpoint: &HttpDeliverySessionCheckpoint) -> CheckpointCommit {
        self.commit_rendered(
            OutboundCheckpointKind::HttpSession,
            &checkpoint.digest(),
            &checkpoint.render(),
        )
    }
}

impl WebhookDeliverySessionCheckpointStore for OutboundDeliveryStore<'_> {
    fn commit(&mut self, checkpoint: &WebhookDeliverySessionCheckpoint) -> CheckpointCommit {
        self.commit_rendered(
            OutboundCheckpointKind::WebhookSession,
            &checkpoint.digest(),
            &checkpoint.render(),
        )
    }
}

impl EmailDeliverySessionCheckpointStore for OutboundDeliveryStore<'_> {
    fn commit(&mut self, checkpoint: &EmailDeliverySessionCheckpoint) -> CheckpointCommit {
        self.commit_rendered(
            OutboundCheckpointKind::EmailSession,
            &checkpoint.digest(),
            &checkpoint.render(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    mod namespace_sync;
    use semaprax::outbound_host_adapter::{
        prepare_http_delivery, AdapterObservation, DeliverySessionCheckpointRefusal,
        DurableHttpDeliveryOutcome, HttpDeliverySession, HttpDeliverySessionRestoreCapability,
        HttpDeliverySessionRestoreRefusal, HttpHeader, HttpMethod, HttpRequest, OutboundAdapter,
        OutboundCapability, OutboundPolicy, PreparedRequest,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempDirectory(PathBuf);

    impl TempDirectory {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .canonicalize()
                .expect("canonicalize test temp root");
            for _ in 0..32 {
                let nonce = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
                let path = root.join(format!(
                    "semaprax-outbound-store-{}-{nonce}",
                    std::process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("create test directory: {error}"),
                }
            }
            panic!("could not allocate a unique test directory")
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[derive(Default)]
    struct RecordingAdapter(Vec<PreparedRequest>);

    impl OutboundAdapter for RecordingAdapter {
        fn send(&mut self, request: &PreparedRequest) -> AdapterObservation {
            self.0.push(request.clone());
            AdapterObservation::Response {
                status: 202,
                body: Vec::new(),
            }
        }
    }

    fn capability() -> OutboundCapability {
        OutboundCapability::grant_for_trusted_host(
            "sha256:store-test-deployment",
            "store-test-invocation",
            OutboundPolicy::new(
                "store-test-policy",
                ["https://store.example.test".to_owned()],
                1_024,
                512,
                5_000,
                4,
                3,
            )
            .expect("bounded fixture policy"),
        )
        .expect("trusted fixture authority")
    }

    fn request() -> HttpRequest {
        HttpRequest {
            method: HttpMethod::Post,
            endpoint: "https://store.example.test/events".into(),
            request_id: "store-test-request".into(),
            idempotency_key: "store-test-key".into(),
            content_type: Some("application/json".into()),
            headers: vec![HttpHeader::new("x-store-test", "stable").unwrap()],
            body: br#"{"ready":true}"#.to_vec(),
            deadline_ms: 1_000,
        }
    }

    fn empty_checkpoint() -> HttpDeliverySessionCheckpoint {
        HttpDeliverySession::new(1)
            .expect("bounded session")
            .session_checkpoint()
            .expect("empty session checkpoint")
    }

    #[test]
    fn http_checkpoint_reopens_and_exact_replay_does_not_redispatch() {
        let temp = TempDirectory::new();
        let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
        let mut store = OutboundDeliveryStore::new(&directory);
        let mut session = HttpDeliverySession::new(2).expect("bounded session");
        let mut adapter = RecordingAdapter::default();
        let first = session
            .reconcile_durable(
                prepare_http_delivery(capability(), request()).expect("admitted fixture request"),
                &mut store,
                &mut adapter,
            )
            .expect("durable dispatch");
        assert!(matches!(first, DurableHttpDeliveryOutcome::Dispatched(_)));
        assert_eq!(adapter.0.len(), 1);

        let checkpoint = session.session_checkpoint().expect("terminal checkpoint");
        let digest = checkpoint.digest();
        let capacity = checkpoint.capacity();
        drop(directory);

        // Recovery deliberately reacquires the directory capability rather
        // than persisting a path or selecting authority from config.
        let reopened_directory = platform::hold_directory(temp.path()).expect("reopen directory");
        let reopened = OutboundDeliveryStore::new(&reopened_directory);
        let bytes = reopened
            .load(OutboundCheckpointKind::HttpSession, &digest)
            .expect("load exact durable terminal checkpoint");
        let mut restored = HttpDeliverySession::restore_authenticated(
            &bytes,
            HttpDeliverySessionRestoreCapability::grant_for_trusted_host(&digest, capacity)
                .expect("trusted host binds exact digest and capacity"),
        )
        .expect("authenticated checkpoint restores");

        let mut replay_store = OutboundDeliveryStore::new(&reopened_directory);
        let mut replay_adapter = RecordingAdapter::default();
        let replay = restored
            .reconcile_durable(
                prepare_http_delivery(capability(), request()).expect("same admitted request"),
                &mut replay_store,
                &mut replay_adapter,
            )
            .expect("exact replay is durable and local");
        assert!(matches!(replay, DurableHttpDeliveryOutcome::Replayed(_)));
        assert!(replay_adapter.0.is_empty(), "recovery must not redispatch");
    }

    #[test]
    fn tampered_existing_checkpoint_is_uncertain_and_never_dispatches() {
        let temp = TempDirectory::new();
        let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
        let mut session = HttpDeliverySession::new(1).expect("bounded session");
        let mut store = OutboundDeliveryStore::new(&directory);
        let mut adapter = RecordingAdapter::default();
        let outcome = session
            .reconcile_durable(
                prepare_http_delivery(capability(), request()).expect("prepared request"),
                &mut store,
                &mut adapter,
            )
            .expect("durable dispatch");
        assert!(matches!(outcome, DurableHttpDeliveryOutcome::Dispatched(_)));
        assert_eq!(adapter.0.len(), 1);

        let checkpoint = session.session_checkpoint().expect("terminal checkpoint");
        let digest = checkpoint.digest();
        let capacity = checkpoint.capacity();
        let name = checkpoint_filename(OutboundCheckpointKind::HttpSession, &digest).unwrap();
        fs::write(temp.path().join(name), b"tampered checkpoint")
            .expect("replace file with tampered fixture");
        assert_eq!(
            HttpDeliverySessionCheckpointStore::commit(&mut store, &checkpoint),
            CheckpointCommit::Uncertain,
            "same content-addressed identity with changed bytes is never accepted"
        );
        let bytes = store
            .load(OutboundCheckpointKind::HttpSession, &digest)
            .expect("bounded tampered bytes remain readable as untrusted data");
        assert!(matches!(
            HttpDeliverySession::restore_authenticated(
                &bytes,
                HttpDeliverySessionRestoreCapability::grant_for_trusted_host(&digest, capacity)
                    .expect("trusted host binds exact digest and capacity"),
            ),
            Err(HttpDeliverySessionRestoreRefusal::Checkpoint(
                DeliverySessionCheckpointRefusal::BindingMismatch
            ))
        ));
        assert_eq!(adapter.0.len(), 1, "refused recovery does not redispatch");
    }

    #[test]
    fn sparse_oversized_checkpoint_is_refused_by_load_and_existing_commit() {
        let temp = TempDirectory::new();
        let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
        let mut store = OutboundDeliveryStore::new(&directory);
        let checkpoint = empty_checkpoint();
        let digest = checkpoint.digest();
        let name = checkpoint_filename(OutboundCheckpointKind::HttpSession, &digest).unwrap();
        let file = fs::File::create(temp.path().join(name)).expect("create sparse hostile file");
        file.set_len((MAX_CHECKPOINT_BYTES + 1) as u64)
            .expect("extend sparse hostile file");
        drop(file);

        assert_eq!(
            store.load(OutboundCheckpointKind::HttpSession, &digest),
            Err(OutboundCheckpointReadRefusal::StorageUnavailable)
        );
        assert_eq!(
            HttpDeliverySessionCheckpointStore::commit(&mut store, &checkpoint),
            CheckpointCommit::Uncertain
        );
    }

    #[test]
    fn exact_existing_checkpoint_requires_each_sync_to_succeed() {
        let temp = TempDirectory::new();
        let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
        let mut store = OutboundDeliveryStore::new(&directory);
        let checkpoint = empty_checkpoint();
        let digest = checkpoint.digest();
        let name = checkpoint_filename(OutboundCheckpointKind::HttpSession, &digest).unwrap();
        fs::write(temp.path().join(name), checkpoint.render().as_bytes())
            .expect("create exact preexisting bytes");
        let sync_calls = std::cell::Cell::new(0);
        let commit = store.commit_rendered_with(
            OutboundCheckpointKind::HttpSession,
            &digest,
            &checkpoint.render(),
            |_, _, _, _| Err(platform::Error::Exists),
            |_| {
                sync_calls.set(sync_calls.get() + 1);
                Err(platform::Error::Changed)
            },
        );
        assert_eq!(sync_calls.get(), 1);
        assert_eq!(commit, CheckpointCommit::Uncertain);

        assert_eq!(
            HttpDeliverySessionCheckpointStore::commit(&mut store, &checkpoint),
            CheckpointCommit::Committed,
            "an exact preexisting file is acknowledged only after the real sync succeeds"
        );
    }

    #[cfg(unix)]
    #[test]
    fn replacement_after_new_write_is_not_acknowledged() {
        let temp = TempDirectory::new();
        let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
        let mut store = OutboundDeliveryStore::new(&directory);
        let checkpoint = empty_checkpoint();
        let digest = checkpoint.digest();
        let rendered = checkpoint.render();
        let commit = store.commit_rendered_with(
            OutboundCheckpointKind::HttpSession,
            &digest,
            &rendered,
            |directory, name, bytes, mode| {
                let created = platform::write_file_new(directory, name, bytes, mode)?;
                let replacement = temp.path().join("replacement-checkpoint");
                fs::write(&replacement, b"replacement bytes")
                    .map_err(|_| platform::Error::Changed)?;
                fs::rename(&replacement, temp.path().join(name))
                    .map_err(|_| platform::Error::Changed)?;
                Ok(created)
            },
            platform::sync_regular_file,
        );
        assert_eq!(commit, CheckpointCommit::Uncertain);
    }

    #[cfg(unix)]
    #[test]
    fn replacement_during_sync_is_not_acknowledged() {
        let temp = TempDirectory::new();
        let directory = platform::hold_directory(temp.path()).expect("hold caller directory");
        let mut store = OutboundDeliveryStore::new(&directory);
        let checkpoint = empty_checkpoint();
        let digest = checkpoint.digest();
        let rendered = checkpoint.render();
        let name = checkpoint_filename(OutboundCheckpointKind::HttpSession, &digest).unwrap();
        let commit = store.commit_rendered_with(
            OutboundCheckpointKind::HttpSession,
            &digest,
            &rendered,
            platform::write_file_new,
            |file| {
                platform::sync_regular_file(file)?;
                let replacement = temp.path().join("replacement-checkpoint");
                fs::write(&replacement, b"replacement bytes")
                    .map_err(|_| platform::Error::Changed)?;
                fs::rename(&replacement, temp.path().join(&name))
                    .map_err(|_| platform::Error::Changed)
            },
        );
        assert_eq!(commit, CheckpointCommit::Uncertain);
    }

    #[test]
    fn digest_namespace_and_read_size_are_bounded() {
        assert_eq!(
            checkpoint_filename(
                OutboundCheckpointKind::HttpSession,
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            Some("outbound-http-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa.checkpoint".into())
        );
        assert!(checkpoint_filename(OutboundCheckpointKind::HttpSession, "../escape").is_none());
        assert!(checkpoint_filename(
            OutboundCheckpointKind::HttpSession,
            "sha256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        )
        .is_none());
    }
}
