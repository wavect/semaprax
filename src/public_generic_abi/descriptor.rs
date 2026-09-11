//! Reference codec for [Public Generic Descriptor
//! v1](../../docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md): encode, decode, and
//! independent byte-exact replay of one descriptor value.
//!
//! `DescriptorV1` values here are hand-constructed in tests, never derived
//! from real checked HIR — that derivation is the next tranche's classifier
//! (issue #150's implementation half), out of scope this round. What this
//! module proves is that the wire format is deterministic, that a display
//! rename cannot move identity, and that malformed or cross-paired bytes fail
//! closed rather than being repaired or partially accepted.

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::boundary_profile::{
    BOUNDARY_PROFILE_SCHEMA, MAX_DESCRIPTOR_WIRE_BYTES, MAX_INSTANCE_TERM_BYTES,
};
use crate::public_generic_abi::{digest, frame, read_frame};
use crate::public_generic_type::{InstanceFacts, PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA};

/// The versioned descriptor schema.
pub const DESCRIPTOR_SCHEMA: &str = "semaprax.public-generic-descriptor.v1";

const IDENTITY_DOMAIN: &[u8] = b"semaprax.public-generic-descriptor.v1.identity\0";

/// Malformed wire bytes: bad framing, an unknown schema literal, trailing
/// bytes, or a truncated field.
pub const MALFORMED_DESCRIPTOR: &str = "SPX-PG701";
/// A descriptor bound was reached (total bytes or a framed field's length).
pub const DESCRIPTOR_CAPACITY: &str = "SPX-PG702";
/// Independent replay found the recomputed identity preimage does not equal
/// the submitted one.
pub const DESCRIPTOR_REPLAY_MISMATCH: &str = "SPX-PG703";
/// The descriptor's `boundary_profile` or `type_grammar_schema` does not
/// match the trusted value's, even though the bytes otherwise decode.
pub const DESCRIPTOR_VERSION_MISMATCH: &str = "SPX-PG704";

/// The identity-bearing subset of one [`InstanceFacts`] value carried on the
/// wire: the canonical grammar term and the grammar's own instance digest.
/// The term already recursively encodes every ordered argument, and the
/// instance digest already commits to the substituted field tree and the
/// owned-leaf paths, so this pair is the whole identity — not a partial
/// projection of it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceBinding {
    pub term: String,
    pub instance_digest: String,
}

impl InstanceBinding {
    /// Bind to the identity-bearing subset of an existing, already-derived
    /// [`InstanceFacts`] value, reusing the grammar's own facts rather than
    /// re-deriving them.
    pub fn from_facts(facts: &InstanceFacts) -> Self {
        Self {
            term: facts.term.clone(),
            instance_digest: facts.instance_digest.clone(),
        }
    }
}

/// One Public Generic Descriptor v1 value: everything a foreign consumer
/// needs to know a selected export's identity, its input/result generic
/// instances, and the exact checked-program context it is bound to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DescriptorV1 {
    schema: String,
    boundary_profile: String,
    type_grammar_schema: String,
    export_id: String,
    export_name: String,
    program_root_digest: String,
    source_projection_digest: String,
    public_surface_digest: String,
    input: InstanceBinding,
    result: InstanceBinding,
}

impl DescriptorV1 {
    /// Construct a descriptor bound to the current schema, boundary profile,
    /// and type-grammar versions.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        export_id: impl Into<String>,
        export_name: impl Into<String>,
        program_root_digest: impl Into<String>,
        source_projection_digest: impl Into<String>,
        public_surface_digest: impl Into<String>,
        input: InstanceBinding,
        result: InstanceBinding,
    ) -> Self {
        Self {
            schema: DESCRIPTOR_SCHEMA.to_owned(),
            boundary_profile: BOUNDARY_PROFILE_SCHEMA.to_owned(),
            type_grammar_schema: PUBLIC_GENERIC_TYPE_GRAMMAR_SCHEMA.to_owned(),
            export_id: export_id.into(),
            export_name: export_name.into(),
            program_root_digest: program_root_digest.into(),
            source_projection_digest: source_projection_digest.into(),
            public_surface_digest: public_surface_digest.into(),
            input,
            result,
        }
    }

    pub fn export_id(&self) -> &str {
        &self.export_id
    }

    pub fn export_name(&self) -> &str {
        &self.export_name
    }

    pub fn boundary_profile(&self) -> &str {
        &self.boundary_profile
    }

    pub fn type_grammar_schema(&self) -> &str {
        &self.type_grammar_schema
    }

    pub fn input(&self) -> &InstanceBinding {
        &self.input
    }

    pub fn result(&self) -> &InstanceBinding {
        &self.result
    }

    /// Presentation-only rename. Never changes the identity preimage or
    /// digest, only the trailing wire field.
    pub fn with_export_name(mut self, export_name: impl Into<String>) -> Self {
        self.export_name = export_name.into();
        self
    }

    fn identity_preimage(&self) -> Vec<u8> {
        let mut preimage = Vec::new();
        frame(&mut preimage, self.schema.as_bytes());
        frame(&mut preimage, self.boundary_profile.as_bytes());
        frame(&mut preimage, self.type_grammar_schema.as_bytes());
        frame(&mut preimage, self.export_id.as_bytes());
        frame(&mut preimage, self.program_root_digest.as_bytes());
        frame(&mut preimage, self.source_projection_digest.as_bytes());
        frame(&mut preimage, self.public_surface_digest.as_bytes());
        frame(&mut preimage, self.input.term.as_bytes());
        frame(&mut preimage, self.input.instance_digest.as_bytes());
        frame(&mut preimage, self.result.term.as_bytes());
        frame(&mut preimage, self.result.instance_digest.as_bytes());
        preimage
    }

    /// The domain-separated digest of the identity preimage. Never
    /// transmitted; always recomputed from the fields it covers, so it can
    /// never be forged independently of them.
    pub fn identity_digest(&self) -> String {
        digest(IDENTITY_DOMAIN, &self.identity_preimage())
    }

    /// Canonical wire bytes: the identity preimage, then one trailing framed
    /// presentation field (`export_name`) that a rename changes without
    /// moving the identity digest.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = self.identity_preimage();
        frame(&mut bytes, self.export_name.as_bytes());
        bytes
    }
}

fn malformed(subject: &str) -> Diagnostic {
    Diagnostic::io(
        MALFORMED_DESCRIPTOR,
        format!("not a canonical {DESCRIPTOR_SCHEMA} descriptor: {subject}"),
    )
}

fn capacity(subject: &str) -> Diagnostic {
    Diagnostic::io(
        DESCRIPTOR_CAPACITY,
        format!("{DESCRIPTOR_SCHEMA} exceeded its {subject}"),
    )
}

/// Parse well-formed wire bytes into a [`DescriptorV1`]. This validates
/// framing, the schema literal, and bounds only; it does **not** validate
/// that the descriptor matches any particular trusted program — that is
/// [`replay`]'s job.
pub fn decode(bytes: &[u8]) -> Result<DescriptorV1, Diagnostic> {
    if bytes.len() > MAX_DESCRIPTOR_WIRE_BYTES {
        return Err(capacity("total wire bytes"));
    }
    let mut offset = 0usize;
    let next = |name: &'static str, offset: &mut usize| -> Result<String, Diagnostic> {
        let (field, next_offset) = read_frame(bytes, *offset, MAX_INSTANCE_TERM_BYTES)
            .ok_or_else(|| malformed(&format!("truncated or oversized {name} field")))?;
        *offset = next_offset;
        String::from_utf8(field.to_vec()).map_err(|_| malformed(&format!("{name} is not UTF-8")))
    };

    let schema = next("schema", &mut offset)?;
    if schema != DESCRIPTOR_SCHEMA {
        return Err(malformed("unknown descriptor schema"));
    }
    let boundary_profile = next("boundary_profile", &mut offset)?;
    let type_grammar_schema = next("type_grammar_schema", &mut offset)?;
    let export_id = next("export_id", &mut offset)?;
    let program_root_digest = next("program_root_digest", &mut offset)?;
    let source_projection_digest = next("source_projection_digest", &mut offset)?;
    let public_surface_digest = next("public_surface_digest", &mut offset)?;
    let input_term = next("input.term", &mut offset)?;
    let input_instance_digest = next("input.instance_digest", &mut offset)?;
    let result_term = next("result.term", &mut offset)?;
    let result_instance_digest = next("result.instance_digest", &mut offset)?;
    let export_name = next("export_name", &mut offset)?;

    if offset != bytes.len() {
        return Err(malformed("trailing bytes after the descriptor"));
    }

    Ok(DescriptorV1 {
        schema,
        boundary_profile,
        type_grammar_schema,
        export_id,
        export_name,
        program_root_digest,
        source_projection_digest,
        public_surface_digest,
        input: InstanceBinding {
            term: input_term,
            instance_digest: input_instance_digest,
        },
        result: InstanceBinding {
            term: result_term,
            instance_digest: result_instance_digest,
        },
    })
}

/// Decode `candidate` and require its identity preimage to equal `trusted`'s,
/// byte-for-byte — never merely digest-equal, and never by trusting a
/// transmitted digest, since the wire format carries none. This is the
/// descriptor's independent-replay contract: a verifier that already
/// possesses (or, in a future round, independently rederives from checked
/// HIR) a trusted descriptor uses this to accept or reject a submitted one.
pub fn replay(candidate: &[u8], trusted: &DescriptorV1) -> Result<DescriptorV1, Diagnostic> {
    let decoded = decode(candidate)?;
    if decoded.boundary_profile != trusted.boundary_profile
        || decoded.type_grammar_schema != trusted.type_grammar_schema
    {
        return Err(Diagnostic::io(
            DESCRIPTOR_VERSION_MISMATCH,
            format!("{DESCRIPTOR_SCHEMA} boundary profile or type grammar version does not match the trusted value"),
        ));
    }
    if decoded.identity_preimage() != trusted.identity_preimage() {
        return Err(Diagnostic::io(
            DESCRIPTOR_REPLAY_MISMATCH,
            format!("{DESCRIPTOR_SCHEMA} independent replay does not match the trusted value"),
        ));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests;
