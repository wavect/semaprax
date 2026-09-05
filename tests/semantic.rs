//! Single-file semantic operations and the evidence they emit: impact, review,
//! patch application and its versioned evidence capsules, diagnostic repair, and
//! the durable draft, candidate and image stores.
//!
//! One harness binary for the semantic-operation subject that works on a single
//! source file, as distinct from the managed workspace, which has its own
//! harness in `tests/workspace.rs`. Each module below was its own integration
//! test binary and every one statically linked the whole compiler, so the family
//! cost thirteen executables to express one subject. The modules stay
//! independent: each derives its own temporary fixture root from a distinct
//! literal prefix and asserts only over its own fixtures.
//!
//! Two semantic files are deliberately not modules here:
//!
//!   - `semantic_cache_store_cli_v1` and `semantic_workspace_transaction_v1`
//!     each re-invoke their own test binary. A self-invocation selects a test by
//!     its full path, so merging either would prefix its selector and the helper
//!     would silently stop running.
//!
//! `mod` in a test crate root resolves against `tests/`, so each module names
//! its file explicitly.

#[path = "semantic/automatic_candidate_draft_lifecycle.rs"]
mod automatic_candidate_draft_lifecycle;
#[path = "semantic/candidate_archive_cli.rs"]
mod candidate_archive_cli;
#[path = "semantic/diagnostic_repair.rs"]
mod diagnostic_repair;
#[path = "semantic/draft_archive_cli.rs"]
mod draft_archive_cli;
#[path = "semantic/draft_archive_store.rs"]
mod draft_archive_store;
#[path = "semantic/image_store.rs"]
mod image_store;
#[path = "semantic/impact.rs"]
mod impact;
#[path = "semantic/installed_fix_plan.rs"]
mod installed_fix_plan;
#[path = "semantic/patch.rs"]
mod patch;
#[path = "semantic/patch_evidence_v1.rs"]
mod patch_evidence_v1;
#[path = "semantic/patch_evidence_v2.rs"]
mod patch_evidence_v2;
#[path = "semantic/patch_v2.rs"]
mod patch_v2;
#[path = "semantic/patch_v3.rs"]
mod patch_v3;
#[path = "semantic/retention_store.rs"]
mod retention_store;
#[path = "semantic/review.rs"]
mod review;
#[path = "semantic/target_evidence.rs"]
mod target_evidence;
#[path = "semantic/verify_front.rs"]
mod verify_front;
