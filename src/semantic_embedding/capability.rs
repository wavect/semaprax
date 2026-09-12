//! Explicit, non-ambient authority to reach a semantic embedding provider.
//!
//! Mirrors `crate::live_invocation::model_invoke::ModelInvokeCapability`'s
//! shape deliberately: a capability is a plain value a caller constructs
//! and holds, never something the compiler, a checked program, or a
//! provider's own response can synthesize. See
//! `docs/SEMANTIC-EMBEDDING-V1.md`.

/// Explicit, non-ambient authority to reach a semantic embedding provider.
///
/// There is no [`Default`] implementation and no constructor that does not
/// name why the grant exists. A caller wires this in explicitly, typically
/// once per session or call site it chooses to trust; nothing in compiled
/// program text, a request, or a provider's own response can construct
/// one. [`super::kernel::embed`] takes this by reference as a required
/// parameter, so the type system — not a runtime check — is what makes the
/// capability mandatory.
#[derive(Clone, Debug)]
pub struct EmbeddingCapability {
    reason: String,
}

impl EmbeddingCapability {
    /// Grants the capability. `reason` is caller-facing diagnostic text
    /// (e.g. `"editor semantic search index"`, `"fixture test"`); it
    /// carries no authority of its own and is never parsed or matched
    /// against anything.
    #[must_use]
    pub fn grant(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }
}
