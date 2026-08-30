//! Authority-free Offline Published Semantic Lock Snapshot v1.

use crate::diagnostic::Diagnostic;
use crate::package_resolver::{self, ResolutionInput, ResolutionOptions, VerifiedResolution};

mod model;
mod wire;

pub use model::ResolutionSnapshot;

pub const INPUT_SCHEMA: &str = "semaprax.offline-package-resolution-input.v1";
// Exact upper-bound derivation for bytes outside the raw Subject-v2 bodies:
// 1,114 fixed bytes (closed wrapper/payload/limits/nonclaims with the longest
// target and maximum-width canonical integers) + four maximum requirement rows
// + 256 maximum quoted capabilities + 63 subject delimiters. All admitted
// grammar bytes are ASCII and need no extra escaping.
pub const MAX_FIXED_INPUT_BYTES: usize = 1_114;
pub const MAX_REQUIREMENT_FRAMING_BYTES: usize = package_resolver::MAX_REQUIREMENTS
    * (11 + 2 + 255 + 9 + 2 + 33 + 1)
    + (package_resolver::MAX_REQUIREMENTS - 1);
pub const MAX_CAPABILITY_FRAMING_BYTES: usize = package_resolver::MAX_ALLOWED_CAPABILITIES
    * (2 + 255)
    + (package_resolver::MAX_ALLOWED_CAPABILITIES - 1);
pub const MAX_SUBJECT_DELIMITER_BYTES: usize = package_resolver::MAX_SUBJECTS - 1;
pub const MAX_INPUT_FRAMING_BYTES: usize = MAX_FIXED_INPUT_BYTES
    + MAX_REQUIREMENT_FRAMING_BYTES
    + MAX_CAPABILITY_FRAMING_BYTES
    + MAX_SUBJECT_DELIMITER_BYTES;
pub const MAX_INPUT_BYTES: usize =
    package_resolver::MAX_TOTAL_SUBJECT_BYTES + MAX_INPUT_FRAMING_BYTES;
pub const MAX_INPUT_RENDER_BYTES: usize = MAX_INPUT_BYTES * 3 + MAX_INPUT_FRAMING_BYTES * 2;
pub const MAX_SNAPSHOT_BYTES: usize =
    MAX_INPUT_BYTES + package_resolver::MAX_OUTPUT_BYTES + crate::package_lock_v2::MAX_OUTPUT_BYTES;

const INPUT_DOMAIN: &[u8] = b"semaprax.offline-package-resolution-input.v1\0";

/// Generates an exact, authority-free three-part snapshot from caller-owned
/// Resolver-v1 inputs and evidence.
pub fn generate(
    input: &ResolutionInput,
    options: &ResolutionOptions,
    resolution_evidence: &str,
) -> Result<ResolutionSnapshot, Diagnostic> {
    let resolution = package_resolver::verify(resolution_evidence, input, options)
        .map_err(map_resolver_error)?;
    let input_json = wire::render_input(input, options)?;
    let snapshot = ResolutionSnapshot {
        input_json,
        resolution_evidence_json: resolution_evidence.to_owned(),
        lock_json: resolution.lock,
    };
    model::validate_cumulative(&snapshot)?;
    Ok(snapshot)
}

/// Independently reconstructs and exactly replays every submitted snapshot
/// byte. The returned receipt is the unchanged Resolver-v1 receipt.
pub fn verify(snapshot: &ResolutionSnapshot) -> Result<VerifiedResolution, Diagnostic> {
    model::validate_cumulative(snapshot)?;
    let parsed = wire::parse_input(&snapshot.input_json)?;
    let resolution = package_resolver::verify(
        &snapshot.resolution_evidence_json,
        &parsed.input,
        &parsed.options,
    )
    .map_err(map_resolver_error)?;
    if resolution.lock != snapshot.lock_json {
        return Err(replay_error(
            "snapshot Lock-v2 differs from exact Resolver-v1 result",
        ));
    }
    let rebuilt = generate(
        &parsed.input,
        &parsed.options,
        &snapshot.resolution_evidence_json,
    )?;
    if &rebuilt != snapshot {
        return Err(replay_error("snapshot bytes do not exactly replay"));
    }
    Ok(resolution)
}

fn input_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PK501", message)
}

fn authentication_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PK502", message)
}

fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PK503", message)
}

fn wire_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PK504", message)
}

fn replay_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PK505", message)
}

fn map_resolver_error(error: Diagnostic) -> Diagnostic {
    match error.code {
        "SPX-PR501" => input_error("Resolver-v1 rejected snapshot input/options"),
        "SPX-PR505" => limit_error("Resolver-v1 rejected snapshot bounds"),
        "SPX-PR506" => wire_error("Resolver-v1 rejected submitted evidence wire"),
        "SPX-PR507" => replay_error("Resolver-v1 exact replay rejected snapshot evidence"),
        _ => authentication_error("Resolver-v1 rejected snapshot association or policy"),
    }
}

#[cfg(test)]
mod tests;
