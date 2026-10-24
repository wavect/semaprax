//! Complete independent immediate pre-903 census and forecast producer.
//! The unchanged callee sessions/Invoice retain their original types and code.
//! Frozen files differ only by namespace imports and disabled duplicate tests;
//! all original shipping controls remain compiled in their owning modules.
#[allow(unused_imports, reason = "unchanged structural helper bindings")]
use super::super::*;
#[allow(unused_imports, reason = "whole old producer namespace bridge")]
use super::*;

pub(in crate::workspace_graph::expected_projection) struct SyntheticBuilderCosts {
    pub(in crate::workspace_graph::expected_projection) raw_clone_and_hir: usize,
    pub(in crate::workspace_graph::expected_projection) retained_clone_and_hir: usize,
    pub(in crate::workspace_graph::expected_projection) transient_import_clone: usize,
    pub(in crate::workspace_graph::expected_projection) runtime: usize,
    pub(in crate::workspace_graph::expected_projection) retained_hir: usize,
    pub(in crate::workspace_graph::expected_projection) synthetic_ast: usize,
}
#[path = "before903/call_carriers.rs"]
mod call_carriers;
#[path = "before903/modular_pair.rs"]
pub(in crate::workspace_graph::expected_projection) mod modular_pair;
#[path = "before903/paired.rs"]
pub(in crate::workspace_graph::expected_projection) mod paired;
// Preserve the original projection's complete paths without a live producer
// alias. These are namespaces only; no function forwards a paid owner.
mod shared_local_callees {
    pub(super) use super::super::{modular_session, Session};
    pub(super) use super::{modular_pair, paired, Context, Costs, Failure};
}
mod held_projection {
    use super::*;
    #[path = "modular_projection.rs"]
    pub(in crate::workspace_graph::expected_projection) mod modular_projection;
    #[path = "paired_projection.rs"]
    pub(in crate::workspace_graph::expected_projection) mod paired_projection;
}

pub(in crate::workspace_graph::expected_projection) const OLD_PROOF_BYTES: usize =
    std::mem::size_of::<call_carriers::Proof>();
pub(in crate::workspace_graph::expected_projection) const OLD_COST_BYTES: usize =
    std::mem::size_of::<SyntheticBuilderCosts>();
