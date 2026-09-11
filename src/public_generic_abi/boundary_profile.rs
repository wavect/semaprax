//! The frozen bounds of [Public Generic Boundary Profile
//! v1](../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md), as one importable
//! set of constants so the descriptor and carrier codecs — and the future
//! admission classifier — never restate a number independently.
//!
//! This module defines **no classifier**. Whether a given checked export is
//! admitted under the boundary profile is not decided here, or anywhere in
//! this repository yet: that is issue #150's implementation half, explicitly
//! out of scope for this round. The constants below are the bounds table the
//! frozen specification document reuses from existing hosted-green
//! specifications, plus the profile's own new v1 invariants; every value here
//! traces to a citation in the specification document.

use crate::public_generic_type::{
    MAX_OWNED_LEAVES, MAX_RECORD_DEPTH, MAX_TEMPLATE_ARITY, MAX_TERM_BYTES, MAX_VISITED_NODES,
};

/// The frozen admission-profile schema identifier.
pub const BOUNDARY_PROFILE_SCHEMA: &str = "semaprax.public-generic-boundary-profile.v1";

/// Max canonical term bytes per instance. Reused from [Public Generic Type
/// Grammar v1](../../docs/PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md)'s own bound.
pub const MAX_INSTANCE_TERM_BYTES: usize = MAX_TERM_BYTES;

/// Max record nesting depth. Reused from the type grammar, which already
/// matches the internal nested owned-byte-record bound.
pub const MAX_NESTING_DEPTH: usize = MAX_RECORD_DEPTH;

/// Max transitive owned (`Bytes`) leaves per instance. Reused from the type
/// grammar.
pub const MAX_OWNED_LEAVES_PER_INSTANCE: usize = MAX_OWNED_LEAVES;

/// Max visited type nodes per instance closure. Reused from the type
/// grammar; also stands in for "max record declarations in the substituted
/// closure" and "max total fields," since every declaration and field
/// visited while deriving a term is counted here.
pub const MAX_VISITED_NODES_PER_INSTANCE: usize = MAX_VISITED_NODES;

/// Max declared template arity. Reused from the type grammar.
pub const MAX_TEMPLATE_ARITY_BOUND: usize = MAX_TEMPLATE_ARITY;

/// Exactly one owned aggregate input parameter is admitted in v1. Not a
/// ceiling to raise; a fixed profile invariant.
pub const OWNED_INPUT_PARAMETER_COUNT: usize = 1;

/// Exactly one owned aggregate result is admitted in v1.
pub const OWNED_RESULT_COUNT: usize = 1;

/// Max fields admitted on one record declaration reachable from an instance.
/// New: chosen for symmetry with the 256-count bounds already used
/// elsewhere in this corpus (the owned-leaf bound; the existing 256-function
/// public-export bound in Public Owned Data API v1).
pub const MAX_FIELDS_PER_RECORD: usize = 256;

/// Max bytes admitted in one owned `Bytes` leaf. Reused from Public Flat
/// Owned Record API v1's existing per-value bound.
pub const MAX_BYTES_PER_LEAF: usize = 64 * 1024;

/// Max total owned payload bytes per carrier (one input or one result).
/// Reused from Public Owned Data API v1's module-input bound. Also exactly
/// [`MAX_OWNED_LEAVES_PER_INSTANCE`] times [`MAX_BYTES_PER_LEAF`], so the
/// three bounds are mutually consistent rather than independently chosen.
pub const MAX_TOTAL_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

/// Max live handles per carrier instance: the owned-leaf bound plus one root
/// aggregate handle. See [Public Generic Carrier
/// v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md).
pub const MAX_LIVE_HANDLES: usize = MAX_OWNED_LEAVES_PER_INSTANCE + 1;

/// Max total wire bytes for one descriptor: two instance terms at
/// [`MAX_INSTANCE_TERM_BYTES`] each, rounded up generously for the fixed
/// identity fields around them. See [Public Generic Descriptor
/// v1](../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md#bounds).
pub const MAX_DESCRIPTOR_WIRE_BYTES: usize = 2 * MAX_INSTANCE_TERM_BYTES;

#[cfg(test)]
mod tests {
    use super::*;

    /// The specification document states these three bounds are mutually
    /// consistent, not independently chosen. Pin the arithmetic so a future
    /// edit to any one of them cannot silently break that claim.
    #[test]
    fn leaf_and_payload_bounds_are_mutually_consistent() {
        assert_eq!(
            MAX_OWNED_LEAVES_PER_INSTANCE * MAX_BYTES_PER_LEAF,
            MAX_TOTAL_PAYLOAD_BYTES
        );
    }

    #[test]
    fn handle_bound_is_leaf_bound_plus_the_root_handle() {
        assert_eq!(MAX_LIVE_HANDLES, MAX_OWNED_LEAVES_PER_INSTANCE + 1);
    }

    #[test]
    fn exactly_one_input_and_one_result_is_the_v1_invariant() {
        assert_eq!(OWNED_INPUT_PARAMETER_COUNT, 1);
        assert_eq!(OWNED_RESULT_COUNT, 1);
    }
}
