//! Private native-desktop application runner for callable-v3 evidence.
//!
//! This module is feature-gated, unpublished, and intentionally exposes no
//! general admission or ownership API. Packaging supplies one exact generated
//! provider beside the executable (Windows) or in `Contents/Resources`
//! (macOS); the runner exercises that image through the existing loader and
//! authenticated settlement host.

#![forbid(unsafe_op_in_unsafe_fn)]

use std::error::Error;
use std::fs;
use std::io;
use std::path::PathBuf;

use semaprax_native_loader::open_admitted_settlement_exact;

use crate::callable_wire_v3::{ExecuteOutcome, Publication};
use crate::settlement_host_v3::PrivateSettlementHostV3;

const PROVIDER_BASENAME: &str = if cfg!(target_os = "windows") {
    "SemapraxPrivateProvider.dll"
} else if cfg!(target_os = "macos") {
    "SemapraxPrivateProvider.dylib"
} else {
    "libSemapraxPrivateProvider.so"
};
const DESCRIPTOR_BASENAME: &str = "SemapraxPrivateProvider.spxnabi3";

/// Run the private packaged desktop fixture.
///
/// This is exported only so the feature-gated binary target can remain a tiny
/// launcher. It is not a supported host API and does not relax `SPX-B104`.
///
/// # Safety
///
/// The executable directory and packaged provider/descriptor must be trusted
/// as one exact build output. Loading a substituted native image can execute
/// arbitrary initializer, provider, finalizer, and terminator code.
pub unsafe fn private_desktop_v3_app_main() -> Result<(), Box<dyn Error>> {
    let executable = fs::canonicalize(std::env::current_exe()?)?;
    let executable_directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("desktop executable has no parent"))?;
    let asset_directory: PathBuf = if cfg!(target_os = "macos") {
        executable_directory.join("../Resources")
    } else {
        executable_directory.to_owned()
    };
    let provider = fs::canonicalize(asset_directory.join(PROVIDER_BASENAME))?;
    let descriptor = fs::read(asset_directory.join(DESCRIPTOR_BASENAME))?;

    // SAFETY: The packaging gate compiles this exact compiler-generated,
    // synchronous provider, places its immutable descriptor beside it, and
    // admits the canonical root image without an ambient dependency namespace.
    let lease = unsafe { open_admitted_settlement_exact(&provider, &descriptor) }
        .map_err(|error| io::Error::other(format!("admit provider: {error:?}")))?;
    let host = PrivateSettlementHostV3::from_admitted(lease, &descriptor)
        .map_err(|error| io::Error::other(format!("construct host: {error:?}")))?;
    let original = host
        .register_owner(0x4453_4b54, 7)
        .map_err(|error| io::Error::other(format!("register owner: {error:?}")))?;
    let first = host
        .execute_owned_success(&[original], &[41])
        .map_err(|error| io::Error::other(format!("first execution: {error:?}")))?;
    require_owned(&first.outcome, 41)?;
    if first.committed.publication != Publication::Owned(0) {
        return Err(io::Error::other("first publication was not owner zero").into());
    }
    let replay = host
        .replay_committed(first.identity, &first.candidate_bytes)
        .map_err(|error| io::Error::other(format!("replay: {error:?}")))?;
    if replay != first.committed {
        return Err(io::Error::other("receipt replay changed committed state").into());
    }
    let refreshed = first
        .committed
        .published_owner
        .ok_or_else(|| io::Error::other("owned publication omitted refreshed authority"))?;
    let second = host
        .execute_owned_success(&[refreshed], &[43])
        .map_err(|error| io::Error::other(format!("second execution: {error:?}")))?;
    require_owned(&second.outcome, 43)?;
    if second.committed.publication != Publication::Owned(0)
        || second.committed.published_owner.is_none()
        || host.is_poisoned()
        || host.is_draining()
    {
        return Err(io::Error::other("second publication did not remain live and exact").into());
    }

    println!(
        "SEMAPRAX_DESKTOP_V3_OK platform={} calls=2 owner=0 payloads=41,43 replay=exact",
        std::env::consts::OS
    );
    Ok(())
}

fn require_owned(outcome: &ExecuteOutcome, payload: u64) -> io::Result<()> {
    if *outcome
        != (ExecuteOutcome::Owned {
            owner_ordinal: 0,
            payload,
        })
    {
        return Err(io::Error::other(
            "desktop provider returned the wrong outcome",
        ));
    }
    Ok(())
}
