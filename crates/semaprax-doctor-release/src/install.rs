//! Unix-only, handle-relative installation of authenticated doctor releases.
//!
//! This module does not execute a release or activate the ordinary doctor CLI.
//! The caller supplies an independently trusted release identity and a private,
//! quiescent local-filesystem store root.

mod model;
mod unix;

pub use model::{GenerationId, InstallReceipt, RecoveryReceipt, StoreExpectation};
pub use unix::DoctorStore;

use std::path::Path;

use crate::ReleaseExpectation;

pub fn open_store(root: &Path, expectation: StoreExpectation) -> Result<DoctorStore, String> {
    unix::open_store(root, expectation)
}

pub fn install_from_verified_directory(
    store: &DoctorStore,
    source: &Path,
    expected: &ReleaseExpectation,
) -> Result<InstallReceipt, String> {
    unix::install(store, source, expected)
}

pub fn inspect_active(store: &DoctorStore) -> Result<Option<GenerationId>, String> {
    unix::inspect_active(store)
}

pub fn activate(
    store: &DoctorStore,
    generation: &GenerationId,
    expected_active: Option<&GenerationId>,
    expected_release: &ReleaseExpectation,
) -> Result<(), String> {
    unix::activate(store, generation, expected_active, expected_release)
}

pub fn rollback(
    store: &DoctorStore,
    generation: &GenerationId,
    expected_active: &GenerationId,
    expected_release: &ReleaseExpectation,
) -> Result<(), String> {
    unix::activate(store, generation, Some(expected_active), expected_release)
}

pub fn recover(
    store: &DoctorStore,
    expected_release: &ReleaseExpectation,
) -> Result<RecoveryReceipt, String> {
    unix::recover(store, expected_release)
}
