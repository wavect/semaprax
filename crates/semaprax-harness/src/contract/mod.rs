//! Typed harness-provider contracts (HP-01): descriptors, capability kinds and
//! payload validators, request/result envelopes, deterministic negotiation.
//!
//! Diagnostics for this module are `SPX-HPA001..`; see
//! `docs/HARNESS-PROVIDER-V1.md`. Nothing here creates a permission grant.

pub mod descriptor;
pub mod envelope;
pub mod kind;
pub mod mock_peer;
pub mod negotiate;
pub mod payload;

pub use descriptor::{
    CancellationMode, ConfigField, DeclaredCapability, Descriptor, ExtensionCapability,
    PermissionRequest, ResourceBounds, Runtime, SupportRecord, UpstreamIdentity, DESCRIPTOR_SCHEMA,
};
pub use envelope::{
    ProjectBinding, Provenance, RequestEnvelope, ResultEnvelope, ResultStatus, REQUEST_SCHEMA,
    RESULT_SCHEMA,
};
pub use kind::{CapabilityKind, CapabilityRef};
pub use negotiate::{
    check_duplicate_identities, negotiate, refuse_downgrade, ActiveCapability, HostSupport,
    InactiveCapability, LockedIdentity, Negotiation,
};
pub use payload::{validate_payload, Direction};
