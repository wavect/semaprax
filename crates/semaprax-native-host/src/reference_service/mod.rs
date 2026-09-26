//! The runnable reference-service host for the existing service scaffold.
//!
//! This module wires the scaffold's checked service decisions (the
//! `service` template / `examples/task-service-project`) to physical
//! adapters the host operator explicitly grants. It reuses the
//! repository's existing machinery and adds no new authority:
//!
//! - configuration intent is decoded by the existing closed service-config
//!   and adapter-request decoders
//!   ([`semaprax::project::derive_service_host_adapter_request_v1`]); decoded
//!   intent never mints authority, and fixture-mode intent is refused here
//!   (fixture mode stays on the existing `semaprax run` / `semaprax test`
//!   path, which this host never invokes);
//! - per-request decisions are evaluated from the operator-loaded,
//!   authenticated project revision through
//!   [`semaprax::project::ProjectRevision::evaluate_service_decision_v1`];
//! - persistence commits canonical snapshots through the existing durable
//!   checkpoint store under a host-held directory;
//! - outbound delivery runs through R17's
//!   [`deliver_http_durable`](crate::outbound_delivery_store::service_invocation::deliver_http_durable)
//!   with the existing TLS client path;
//! - HTTP serving is loopback-only over the existing
//!   [`TcpNetworkProvider`](semaprax::network_provider::TcpNetworkProvider).
//!
//! Non-claims: no SQLite/PostgreSQL wire protocol is implemented (state is
//! canonical snapshots in the durable store), no TLS server provisioning
//! exists (loopback plaintext only; the provider's `accept_tls` path needs
//! operator-supplied certificate material this host does not mint), no OTLP
//! protobuf is emitted (telemetry is an HTTPS POST to the granted origin's
//! fixed `/v1/events` route), and no hosted, public, or production support
//! is claimed. Only the decisions whose signatures the frozen
//! public-invocation vocabulary admits are invoked; the remaining scaffold
//! decisions keep their existing fixture-mode coverage.

pub mod bundle;
pub mod decisions;
pub mod delivery;
pub mod json;
pub mod mapping;
pub mod secrets;
pub mod serve;
pub mod state;

/// The content digest addressing canonical bytes: `sha256:` plus lowercase
/// hex. Shared by state snapshots, delivery evidence, and run bundles so
/// all three address content identically.
pub(crate) fn content_digest(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    let mut digest = String::from("sha256:");
    for byte in hasher.finalize() {
        digest.push_str(&format!("{byte:02x}"));
    }
    digest
}

/// A minimal temporary held-directory guard for module tests. The durable
/// store's own fixture is private to its module, so these tests own one.
#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use semaprax_native_rust_interop_platform::HeldDirectory;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    pub(crate) struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        pub(crate) fn hold(label: &str) -> (Self, HeldDirectory) {
            let root = std::env::temp_dir()
                .canonicalize()
                .expect("canonicalize test temp root");
            for _ in 0..32 {
                let nonce = NEXT.fetch_add(1, Ordering::Relaxed);
                let path = root.join(format!("semaprax-{label}-{}-{nonce}", std::process::id()));
                if std::fs::create_dir(&path).is_ok() {
                    let directory = semaprax_native_rust_interop_platform::hold_directory(&path)
                        .expect("hold test directory");
                    return (Self { path }, directory);
                }
            }
            panic!("could not allocate a unique test directory");
        }

        pub(crate) fn join(&self, name: &str) -> PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
