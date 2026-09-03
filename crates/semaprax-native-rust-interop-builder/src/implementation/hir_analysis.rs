// Bounded post-HIR semantic analysis for the private Native Rust interop lane.
// This module has no filesystem, process, platform, settlement, or publication authority.
//
// The work is divided by what it inspects: `scalars` admits the ABI scalar set,
// `calls` walks resolved expressions and censuses call sites, `closure` selects
// the exported and imported closure, `type_identity` measures and writes
// resolved type identities, and `fingerprint` hashes the resolved program.
use super::*;

mod calls;
mod closure;
mod fingerprint;
mod scalars;
mod type_identity;

pub(super) use calls::*;
pub(super) use closure::*;
pub(super) use fingerprint::*;
pub(super) use scalars::*;
pub(super) use type_identity::*;
