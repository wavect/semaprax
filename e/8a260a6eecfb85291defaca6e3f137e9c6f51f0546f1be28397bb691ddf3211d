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
#[path = "before903/held_projection"]
mod held_projection {
    use super::*;
    pub(super) use super::before_local_projection::synthetic_builder_bytes_core;
    #[path = "modular_projection.rs"]
    pub(in crate::workspace_graph::expected_projection) mod modular_projection;
    #[path = "paired_projection.rs"]
    pub(in crate::workspace_graph::expected_projection) mod paired_projection;
}

pub(in crate::workspace_graph::expected_projection) const OLD_PROOF_BYTES: usize =
    std::mem::size_of::<call_carriers::Proof>();
pub(in crate::workspace_graph::expected_projection) const OLD_COST_BYTES: usize =
    std::mem::size_of::<SyntheticBuilderCosts>();

// Complete pre-local-callee formula with the already frozen two-word census
// and six-word return type; no live widened 903 ABI enters this oracle.
#[path = "before903/before_local_projection.rs"]
#[allow(
    dead_code,
    reason = "complete pre-local-callee forecast and constructor oracle"
)]
pub(in crate::workspace_graph::expected_projection) mod before_local_projection;

// Test observation only: the old projection itself calls count directly.
pub(in crate::workspace_graph::expected_projection) fn before_local_carrier_costs(
    program: &crate::ast::Program,
    authored: &std::collections::BTreeMap<&str, crate::workspace_graph::AuthoredDeclaration<'_>>,
    imports: bool,
) -> (usize, usize) {
    let proof = call_carriers::count(program, authored, imports);
    (proof.roots, proof.bytes)
}

#[path = "before903/before_local_uncached.rs"]
#[allow(dead_code, reason = "whole pre-certificate sequential caller oracle")]
pub(in crate::workspace_graph::expected_projection) mod before_local_uncached;

// The original seven-argument caller frame and six-word result. The frozen
// sequential caller replaces its original same-signature callee exactly once.
fn synthetic_builder_bytes_scoped_with_registry(
    program: &crate::ast::Program,
    authored: &std::collections::BTreeMap<&str, crate::workspace_graph::AuthoredDeclaration<'_>>,
    programs: &[crate::ast::Program],
    maximum_identity: Option<usize>,
    layout_mode: u8,
    include_imports: bool,
    registry: Option<&private_layout::registry::LayoutRegistry<'_>>,
) -> Result<SyntheticBuilderCosts, Vec<crate::diagnostic::Diagnostic>> {
    before_local_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
        program,
        authored,
        programs,
        maximum_identity,
        include_imports,
        (layout_mode, cached_exact::Retention::Parsed),
        registry,
    )
}
