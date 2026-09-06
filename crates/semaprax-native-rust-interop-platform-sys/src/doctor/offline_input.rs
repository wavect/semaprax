//! Immutable input bytes only: no profile, filesystem path or execution authority.
use std::fs::File;

mod create;
pub use create::{create_doctor_offline_executable, create_doctor_offline_input};

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux;

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
use linux::{TestControl, TestFault, TestReadFault, TestStage};

#[cfg(test)]
mod tests;

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[path = "offline_input/bundle_handoff.rs"]
mod bundle_handoff;

#[cfg(all(
    test,
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
#[path = "offline_input/request_handoff.rs"]
mod request_handoff;

/// The immutable carrier ceiling. It is a resource bound on one fallible
/// `try_reserve_exact`, not an authority boundary: seals, digests, the release
/// signature, the ELF contract and the closed inventory establish admission,
/// and none of them depend on size.
///
/// MEASURED, NOT CHOSEN. On a GitHub-hosted `ubuntu-24.04` runner, encoding the
/// full loader closures the pivoted worker root requires
/// (`scripts/doctor-provisioned-linux-bundle.py --closure`) gives:
///
/// | carrier | encoded bytes |
/// | --- | ---: |
/// | node v22.23.2 + rustc 1.88.0, no Clang | 462,424,370 |
/// | + clang 9.0.1, the smallest official LLVM that runs on 24.04 | 568,339,434 |
/// | + clang 17.0.6 | 652,142,493 |
/// | Ubuntu's own clang-18 closure | ~713,000,000 |
///
/// `render_rows` in `src/doctor.rs` admits only Node major 22 or newer and Rust
/// 1.88 or newer, so the two non-Clang roles cannot shrink. Under the previous
/// 536,870,912-byte ceiling the Node and Rust closures alone took 86% of it,
/// leaving 74,446,542 bytes for a whole Clang role that no official LLVM
/// release fits, so the two real-distribution lifecycle fixtures could not be
/// satisfied by any current real distribution set.
///
/// 1,073,741,824 is 1.65x the measured clang-17 three-role carrier and 1.51x
/// Ubuntu's clang-18 closure, so a Clang role may grow by half again before the
/// ceiling binds; it is 6.25% of a hosted runner's 16 GB.
///
/// The bound this ceiling must stay coherent with is the delegated cgroup-v2
/// scope's `memory.max` in `offline_provisioner/cgroup.rs`, which is set to
/// four times this value. A carrier of N bytes costs 2N of unswappable
/// residency inside that scope: the worker's heap snapshot, which
/// `offline_root::Plan` borrows and so cannot drop before the tool children run,
/// plus the page-rounded tmpfs root written out of it. `memory.swap.max` is 0
/// and `memory.oom.group` is 1 there, so an overshoot kills the whole scope
/// instead of refusing cleanly. Raising this constant without raising that one
/// makes the cgroup the real ceiling.
///
/// The signed capsule's `MAX_ARTIFACT_BYTES`, the release directory's
/// `MAX_ARTIFACT_BYTES` and the signed store's `MAX_FILE_BYTES` each bound the
/// same bundle and request bytes on their own path. They are held equal to this
/// value; the smallest of them is always the effective ceiling.
pub const DOCTOR_OFFLINE_INPUT_MAX_BYTES: usize = 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DoctorOfflineInputError {
    Invalid,
    Unsupported,
    Limit,
    Io,
}

/// An owned, read-only snapshot. It contains no OS descriptor or authority to
/// execute, publish, or attest the meaning/provenance of these bytes.
#[derive(Debug)]
pub struct DoctorOfflineInput(Vec<u8>);

impl DoctorOfflineInput {
    /// Borrow an already-open input. This operation never duplicates, closes,
    /// seeks, or changes the caller's descriptor. Opening/provisioning it is
    /// outside this API. Unsafe concurrent descriptor replacement is excluded.
    pub fn acquire(file: &File, max_bytes: usize) -> Result<Self, DoctorOfflineInputError> {
        validate_max(max_bytes)?;
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        {
            linux::snapshot(
                file,
                max_bytes,
                #[cfg(test)]
                None,
            )
            .map(Self)
        }
        #[cfg(not(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        )))]
        {
            let _ = file;
            Err(DoctorOfflineInputError::Unsupported)
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }

    #[cfg(all(
        test,
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    fn acquire_with_test(
        file: &File,
        max_bytes: usize,
        control: &mut TestControl,
    ) -> Result<Self, DoctorOfflineInputError> {
        validate_max(max_bytes)?;
        linux::snapshot(file, max_bytes, Some(control)).map(Self)
    }
}

fn validate_max(max_bytes: usize) -> Result<(), DoctorOfflineInputError> {
    if max_bytes == 0 {
        Err(DoctorOfflineInputError::Invalid)
    } else if max_bytes > DOCTOR_OFFLINE_INPUT_MAX_BYTES {
        Err(DoctorOfflineInputError::Limit)
    } else {
        Ok(())
    }
}

#[cfg(all(
    test,
    not(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))
))]
#[test]
fn unsupported_platform_retains_limit_precedence() {
    let file = File::open(std::env::current_exe().expect("test executable"))
        .expect("open test executable");
    assert_eq!(
        DoctorOfflineInput::acquire(&file, 1).unwrap_err(),
        DoctorOfflineInputError::Unsupported
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&file, 0).unwrap_err(),
        DoctorOfflineInputError::Invalid
    );
    assert_eq!(
        DoctorOfflineInput::acquire(&file, DOCTOR_OFFLINE_INPUT_MAX_BYTES + 1).unwrap_err(),
        DoctorOfflineInputError::Limit
    );
}
