//! The frozen bounds of [RFC 0005: Compute Kernel
//! Profile](../../docs/RFC-0005-COMPUTE-KERNEL-PROFILE.md), as one
//! importable set of constants so [`super::classifier`] and any later
//! descriptor, dispatch-report, or backend-adapter code never restate a
//! number independently.
//!
//! None of these bounds are reused from an existing hosted-green
//! specification: no prior profile in this repository governs a device
//! buffer, a workgroup, or a dispatch grid, so every value here is a new,
//! first-profile choice, deliberately conservative, and explicitly not a
//! claim about any real device's actual limit. The owning document explains
//! each choice; see its "Bounds" section.

/// The frozen admission-profile schema identifier.
pub const COMPUTE_KERNEL_PROFILE_SCHEMA: &str = "semaprax.compute-kernel-profile.v1";

/// Max kernel-signature parameters admitted. New: chosen small for a first
/// profile; a kernel with a wide parameter list can always be reshaped into
/// one owned/borrowed aggregate the way the public generic boundary already
/// requires for its own owned aggregate boundary.
pub const MAX_KERNEL_PARAMS: usize = 8;

/// Max admitted size of one workgroup dimension. New: chosen well under the
/// smallest per-dimension limit this RFC's backend evaluation found quoted
/// for any evaluated target, so a kernel admitted under this bound is never
/// refused for exceeding a *target's* limit; it is not itself a claim about
/// any specific target's real ceiling.
pub const MAX_WORKGROUP_DIM: u32 = 1024;

/// Max admitted product of all three workgroup dimensions (total
/// invocations per workgroup). New, chosen equal to
/// [`MAX_WORKGROUP_DIM`] so a single-dimension workgroup can reach the full
/// per-dimension bound while a three-dimensional one is still bounded
/// overall.
pub const MAX_WORKGROUP_INVOCATIONS: u32 = 1024;

/// Max admitted size of one dispatch-grid dimension (in workgroups). New,
/// chosen well under the smallest per-dimension grid limit this RFC's
/// backend evaluation found quoted for any evaluated target.
pub const MAX_GRID_DIM: u32 = 65_535;

/// Max admitted elements in one device buffer. New: a first, conservative
/// bound; unrelated to any existing owned-payload bound in this repository,
/// since no earlier profile has ever admitted a device-resident buffer.
pub const MAX_BUFFER_ELEMENTS: usize = 16 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workgroup_invocation_bound_is_not_looser_than_the_per_dimension_bound() {
        assert!(MAX_WORKGROUP_INVOCATIONS <= MAX_WORKGROUP_DIM * MAX_WORKGROUP_DIM);
    }

    #[test]
    fn schema_identifier_is_the_frozen_v1_string() {
        assert_eq!(COMPUTE_KERNEL_PROFILE_SCHEMA, "semaprax.compute-kernel-profile.v1");
    }
}
