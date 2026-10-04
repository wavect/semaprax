//! Validated atomic updates of curated skills and adapter packages (HN-05).
//!
//! One generic resolver (`resolve`) turns a channel into an immutable commit;
//! a pluggable [`fetch::Fetcher`] supplies releases, trees and blobs; `stage`
//! downloads, verifies and validates into the content-addressed store; `ops`
//! applies the policy and records the result in `state`. Nothing here runs
//! upstream code or hooks, and nothing here is reachable from compile/check.
//! See `docs/HARNESS-UPDATES-V1.md`.

pub mod cli;
pub mod fetch;
pub mod fixture;
pub mod gh;
pub mod ops;
pub mod propose;
pub mod resolve;
pub mod sha1;
pub mod stage;
pub mod state;

pub use cli::cli_updates;
pub use fetch::Fetcher;
pub use fixture::{DirectoryFetcher, MemoryFetcher};
pub use gh::GitHubCliFetcher;
pub use ops::{effective_set, effective_set_from, session_pin, Ctx, Report, SessionPin};

pub(crate) fn d(code: &'static str, msg: impl Into<String>) -> crate::diag::HarnessDiagnostic {
    crate::diag::HarnessDiagnostic::new(code, msg)
}

/// Longest status notice or diagnostic message kept in state.
pub const NOTICE_MAX_CHARS: usize = 200;

pub(crate) fn bounded(s: &str) -> String {
    let one_line: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    one_line.chars().take(NOTICE_MAX_CHARS).collect()
}
