//! [Public Generic Carrier v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#canonical-carrier-bytes)'s
//! canonical carrier bytes: the bounded, self-digested, leaf-payload-bearing
//! wire frame a real value crosses the boundary as, and the plan
//! ([`CarrierFrameBinding`]) that validates one parsed frame against a
//! trusted binding derived from a real
//! [`VerifiedPublicGenericDescriptor`](crate::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor) —
//! never from raw or merely parsed descriptor data, matching this issue's
//! "Required APIs" requirement that the carrier's descriptor binding "must
//! require `VerifiedPublicGenericDescriptor`... raw or merely parsed
//! descriptor data is insufficient."
//!
//! [`LogicalCarrierFrame`] is target-neutral: it names no native pointer,
//! Wasm address, allocator, or memory layout, and its bytes are the same on
//! every target that can produce them. Like [`super::CarrierBindingV1`], it
//! is a pure codec — encoding, bounded parsing, and a self-consistency
//! digest check — never a physical allocation or transfer. Consistent with
//! this issue's own flat-owned-`Bytes`-only scope (nested records and Copy
//! scalars remain blocked on #119, exactly as the rest of this milestone's
//! generated consumers and physical adapters already document), every leaf
//! this frame carries is a direct owned `Bytes` leaf; [`LeafKind`] is
//! deliberately closed to that one variant this round.

use std::collections::HashSet;

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::boundary_profile::{
    MAX_BYTES_PER_LEAF, MAX_OWNED_LEAVES_PER_INSTANCE, MAX_TOTAL_PAYLOAD_BYTES,
};
use crate::public_generic_abi::carrier::trace::Direction;
use crate::public_generic_abi::carrier::{
    CARRIER_CAPACITY, CARRIER_REPLAY_MISMATCH, CARRIER_SCHEMA, MALFORMED_CARRIER,
};
use crate::public_generic_abi::descriptor::verify::VerifiedPublicGenericDescriptor;
use crate::public_generic_abi::{digest, frame, read_frame};

/// Domain separation for [`LogicalCarrierFrame::carrier_facts_digest`].
/// Independent of [`super::BINDING_DOMAIN`] (private to `carrier.rs`) and
/// every other domain this module allocates: a frame's digest and a
/// binding's digest must never collide even over identical bytes.
const FRAME_DOMAIN: &[u8] = b"semaprax.public-generic-carrier.v1.frame\0";
/// Domain separation for the derived `endpoint_identity_digest` a
/// [`CarrierFrameBinding`] computes over a verified descriptor's `export_id`.
const ENDPOINT_IDENTITY_DOMAIN: &[u8] = b"semaprax.public-generic-carrier.v1.endpoint-identity\0";
/// Domain separation for the derived `leaf_inventory_digest` a
/// [`CarrierFrameBinding`] computes over a verified descriptor's canonical
/// owned-leaf path list for one direction.
const LEAF_INVENTORY_DOMAIN: &[u8] = b"semaprax.public-generic-carrier.v1.leaf-inventory\0";

/// Bound on one identity/path field's byte length while parsing — schema,
/// digests, and leaf paths alike. Mirrors `carrier.rs`'s own
/// `MAX_BINDING_FIELD_BYTES` bound for the same reason: an attacker-supplied
/// length claim never justifies an unbounded allocation.
const MAX_FRAME_FIELD_BYTES: usize = 64 * 1024;
/// Bound on the whole encoded frame's wire length, checked before any field
/// is parsed. Sized to the payload bound plus generous slack for identity
/// fields and per-leaf framing overhead — never a way to admit more than
/// [`MAX_TOTAL_PAYLOAD_BYTES`] of real payload.
const MAX_FRAME_WIRE_BYTES: usize = MAX_TOTAL_PAYLOAD_BYTES + 4 * 1024 * 1024;

fn malformed(subject: &str) -> Diagnostic {
    Diagnostic::io(
        MALFORMED_CARRIER,
        format!("not a canonical {CARRIER_SCHEMA} frame: {subject}"),
    )
}

fn capacity(subject: &str) -> Diagnostic {
    Diagnostic::io(
        CARRIER_CAPACITY,
        format!("{CARRIER_SCHEMA} frame exceeded its {subject} bound"),
    )
}

fn replay_mismatch(subject: &str) -> Diagnostic {
    Diagnostic::io(
        CARRIER_REPLAY_MISMATCH,
        format!("{CARRIER_SCHEMA} frame {subject}"),
    )
}

fn direction_text(direction: Direction) -> &'static str {
    match direction {
        Direction::Input => "input",
        Direction::Result => "result",
    }
}

fn direction_from_text(text: &str) -> Option<Direction> {
    Some(match text {
        "input" => Direction::Input,
        "result" => Direction::Result,
        _ => return None,
    })
}

/// The closed, canonical leaf type/kind vocabulary. Deliberately one variant
/// this round: every leaf a frame carries is a direct owned `Bytes` leaf,
/// matching the flat-owned-`Bytes`-only scope the rest of this milestone's
/// generated consumers and physical adapters already document (nested
/// records and Copy scalars stay blocked on #119).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeafKind {
    Bytes,
}

impl LeafKind {
    fn tag(self) -> u8 {
        match self {
            Self::Bytes => 0,
        }
    }

    fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            0 => Some(Self::Bytes),
            _ => None,
        }
    }
}

/// One owned leaf in canonical order: its structural path identity, its
/// canonical kind, and its exact payload bytes (zero length and embedded
/// zero bytes both preserved exactly).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarrierLeaf {
    path: String,
    kind: LeafKind,
    payload: Vec<u8>,
}

impl CarrierLeaf {
    pub fn new(path: impl Into<String>, kind: LeafKind, payload: Vec<u8>) -> Self {
        Self {
            path: path.into(),
            kind,
            payload,
        }
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn kind(&self) -> LeafKind {
        self.kind
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// One [Public Generic Carrier v1 canonical carrier
/// bytes](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#canonical-carrier-bytes)
/// value: the exact semantic binding facts plus every owned leaf in
/// canonical order, self-digested so a bit flip anywhere in the frame is
/// caught before [`CarrierFrameBinding::validate_frame`] ever runs. Binding
/// this frame to a trusted descriptor/endpoint/instance/direction is
/// [`CarrierFrameBinding`]'s job, kept deliberately separate — matching
/// [`super::CarrierBindingV1`]'s own decode/replay split — so a frame that
/// merely decodes is never confused with one that is semantically bound to
/// the right value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalCarrierFrame {
    schema: String,
    direction: Direction,
    descriptor_digest: String,
    endpoint_identity_digest: String,
    instance_identity_digest: String,
    leaf_inventory_digest: String,
    leaves: Vec<CarrierLeaf>,
}

impl LogicalCarrierFrame {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        direction: Direction,
        descriptor_digest: impl Into<String>,
        endpoint_identity_digest: impl Into<String>,
        instance_identity_digest: impl Into<String>,
        leaf_inventory_digest: impl Into<String>,
        leaves: Vec<CarrierLeaf>,
    ) -> Self {
        Self {
            schema: CARRIER_SCHEMA.to_owned(),
            direction,
            descriptor_digest: descriptor_digest.into(),
            endpoint_identity_digest: endpoint_identity_digest.into(),
            instance_identity_digest: instance_identity_digest.into(),
            leaf_inventory_digest: leaf_inventory_digest.into(),
            leaves,
        }
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn leaves(&self) -> &[CarrierLeaf] {
        &self.leaves
    }

    pub fn total_payload_length(&self) -> u64 {
        self.leaves
            .iter()
            .map(|leaf| leaf.payload.len() as u64)
            .sum()
    }

    /// Every field in canonical byte order except the trailing
    /// `carrier_facts_digest` — the exact preimage that digest is computed
    /// over, so parsing and construction always agree on what is signed.
    fn body_preimage(&self) -> Vec<u8> {
        let mut preimage = Vec::new();
        frame(&mut preimage, self.schema.as_bytes());
        frame(&mut preimage, direction_text(self.direction).as_bytes());
        frame(&mut preimage, self.descriptor_digest.as_bytes());
        frame(&mut preimage, self.endpoint_identity_digest.as_bytes());
        frame(&mut preimage, self.instance_identity_digest.as_bytes());
        frame(&mut preimage, self.leaf_inventory_digest.as_bytes());
        preimage.extend_from_slice(&(self.leaves.len() as u64).to_le_bytes());
        preimage.extend_from_slice(&self.total_payload_length().to_le_bytes());
        for leaf in &self.leaves {
            frame(&mut preimage, leaf.path.as_bytes());
            preimage.push(leaf.kind.tag());
            frame(&mut preimage, &leaf.payload);
        }
        preimage
    }

    /// The domain-separated digest over every semantic and payload byte.
    /// Never transmitted as a separate trust input; [`Self::encode`] appends
    /// it and [`parse_bounded`] always independently recomputes and compares
    /// it before returning a value — the frame's own bytes are always
    /// self-checking, the same way [`super::CarrierBindingV1::binding_digest`]
    /// is always recomputed rather than trusted.
    pub fn carrier_facts_digest(&self) -> String {
        digest(FRAME_DOMAIN, &self.body_preimage())
    }

    /// Canonical wire bytes: every field above, then the self-digest.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = self.body_preimage();
        frame(&mut bytes, self.carrier_facts_digest().as_bytes());
        bytes
    }
}

fn read_string_field<'a>(
    bytes: &'a [u8],
    offset: &mut usize,
    name: &'static str,
) -> Result<&'a str, Diagnostic> {
    let (field, next_offset) = read_frame(bytes, *offset, MAX_FRAME_FIELD_BYTES)
        .ok_or_else(|| malformed(&format!("truncated or oversized {name} field")))?;
    *offset = next_offset;
    std::str::from_utf8(field).map_err(|_| malformed(&format!("{name} is not UTF-8")))
}

fn read_u64_field(bytes: &[u8], offset: &mut usize, name: &'static str) -> Result<u64, Diagnostic> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| malformed(&format!("truncated {name}")))?;
    let field = bytes
        .get(*offset..end)
        .ok_or_else(|| malformed(&format!("truncated {name}")))?;
    let value = u64::from_le_bytes(field.try_into().expect("checked 8-byte slice"));
    *offset = end;
    Ok(value)
}

/// Parse and bound-check untrusted `bytes` into a self-consistent
/// [`LogicalCarrierFrame`]. Bounded and allocation-conscious: the total wire
/// length, the declared leaf count, the declared total payload length, and
/// each leaf's own declared payload length are all checked against their
/// frozen [Boundary Profile v1](../../../docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md#bounds)
/// bound *before* the corresponding allocation is made. Independently
/// recomputes and compares the trailing `carrier_facts_digest`: a
/// bit-flipped or hand-tampered frame that otherwise decodes is still
/// rejected here, before [`CarrierFrameBinding::validate_frame`] ever sees
/// it. Semantic binding (does this frame belong to the expected
/// descriptor/endpoint/instance/direction, do its leaves match the expected
/// canonical inventory) is deliberately not this function's job — that is
/// [`CarrierFrameBinding::validate_frame`], run separately, matching
/// [`super::CarrierBindingV1::decode_binding`] versus
/// [`super::CarrierBindingV1`]'s own `replay_binding` split.
pub fn parse_bounded(bytes: &[u8]) -> Result<LogicalCarrierFrame, Diagnostic> {
    if bytes.len() > MAX_FRAME_WIRE_BYTES {
        return Err(capacity("total wire-byte"));
    }

    let mut offset = 0usize;
    let schema = read_string_field(bytes, &mut offset, "schema")?.to_owned();
    if schema != CARRIER_SCHEMA {
        return Err(malformed("unknown carrier schema"));
    }
    let direction_text_value = read_string_field(bytes, &mut offset, "direction")?.to_owned();
    let direction =
        direction_from_text(&direction_text_value).ok_or_else(|| malformed("unknown direction"))?;
    let descriptor_digest = read_string_field(bytes, &mut offset, "descriptor_digest")?.to_owned();
    let endpoint_identity_digest =
        read_string_field(bytes, &mut offset, "endpoint_identity_digest")?.to_owned();
    let instance_identity_digest =
        read_string_field(bytes, &mut offset, "instance_identity_digest")?.to_owned();
    let leaf_inventory_digest =
        read_string_field(bytes, &mut offset, "leaf_inventory_digest")?.to_owned();

    let leaf_count = read_u64_field(bytes, &mut offset, "leaf_count")?;
    if leaf_count > MAX_OWNED_LEAVES_PER_INSTANCE as u64 {
        return Err(capacity("leaf-count"));
    }
    let declared_total_payload_length = read_u64_field(bytes, &mut offset, "total_payload_length")?;
    if declared_total_payload_length > MAX_TOTAL_PAYLOAD_BYTES as u64 {
        return Err(capacity("total-payload-byte"));
    }

    let mut leaves = Vec::with_capacity(leaf_count as usize);
    let mut seen_paths: HashSet<String> = HashSet::with_capacity(leaf_count as usize);
    let mut actual_total_payload: u64 = 0;
    for _ in 0..leaf_count {
        let path = read_string_field(bytes, &mut offset, "leaf_path")?.to_owned();
        if !seen_paths.insert(path.clone()) {
            return Err(malformed("duplicate leaf path"));
        }
        let tag = *bytes
            .get(offset)
            .ok_or_else(|| malformed("truncated leaf kind"))?;
        offset += 1;
        let kind = LeafKind::from_tag(tag).ok_or_else(|| malformed("unknown leaf kind"))?;

        let declared_leaf_len = read_u64_field(bytes, &mut offset, "leaf payload length")?;
        // Roll the read back: `read_frame` re-reads the same 8-byte length
        // header itself. Checking the declared length against the bound
        // *before* calling it means an over-bound leaf never drives a large
        // allocation attempt, matching "malformed or noncanonical bytes fail
        // before allocation where possible".
        offset -= 8;
        if declared_leaf_len > MAX_BYTES_PER_LEAF as u64 {
            return Err(capacity("single-leaf-byte"));
        }
        let (payload, next_offset) = read_frame(bytes, offset, MAX_BYTES_PER_LEAF)
            .ok_or_else(|| malformed("truncated leaf payload"))?;
        offset = next_offset;
        actual_total_payload = actual_total_payload
            .checked_add(payload.len() as u64)
            .ok_or_else(|| capacity("total-payload-byte"))?;
        // Defense in depth, not an independently reachable failure mode:
        // `MAX_TOTAL_PAYLOAD_BYTES` equals exactly `leaf_count`'s own bound
        // times `MAX_BYTES_PER_LEAF`'s own bound, so the two checks above
        // already make this comparison true by construction on every path
        // that reaches here. Kept so a future change to either bound that
        // breaks that exact relationship fails closed immediately rather
        // than silently admitting a larger total.
        if actual_total_payload > MAX_TOTAL_PAYLOAD_BYTES as u64 {
            return Err(capacity("total-payload-byte"));
        }
        leaves.push(CarrierLeaf {
            path,
            kind,
            payload: payload.to_vec(),
        });
    }

    if actual_total_payload != declared_total_payload_length {
        return Err(malformed(
            "declared total payload length does not match the sum of leaf payload lengths",
        ));
    }

    let carrier_facts_digest =
        read_string_field(bytes, &mut offset, "carrier_facts_digest")?.to_owned();
    if offset != bytes.len() {
        return Err(malformed("trailing bytes after the carrier frame"));
    }

    let candidate = LogicalCarrierFrame {
        schema,
        direction,
        descriptor_digest,
        endpoint_identity_digest,
        instance_identity_digest,
        leaf_inventory_digest,
        leaves,
    };
    if candidate.carrier_facts_digest() != carrier_facts_digest {
        return Err(replay_mismatch(
            "independent replay found the recomputed carrier-facts digest does not equal the \
             submitted one",
        ));
    }
    Ok(candidate)
}

/// The length-framed, count-prefixed preimage of one canonical leaf-path
/// list, in structural order. Shared by every [`CarrierFrameBinding`]
/// constructor so two bindings built from the same ordered path list always
/// derive byte-identical `leaf_inventory_digest`s.
fn framed_leaf_paths(paths: &[String]) -> Vec<u8> {
    let mut preimage = Vec::new();
    preimage.extend_from_slice(&(paths.len() as u64).to_le_bytes());
    for path in paths {
        frame(&mut preimage, path.as_bytes());
    }
    preimage
}

/// The trusted plan a parsed [`LogicalCarrierFrame`] is validated against:
/// the exact descriptor, endpoint, instance, direction, and canonical
/// leaf-path inventory one carrier instance is bound to. Equivalent in
/// responsibility to this issue's `LogicalCarrierPlan`.
///
/// [`Self::from_verified_descriptor`] requires a real
/// [`VerifiedPublicGenericDescriptor`] — never raw or merely parsed
/// descriptor bytes — reusing its already-independently-verified
/// `descriptor_digest`, `export_id`, and per-direction [`InstanceFacts`]
/// (`instance_digest` and the canonical `owned_leaves` path list) rather
/// than re-deriving or trusting any of those facts a second time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarrierFrameBinding {
    direction: Direction,
    descriptor_digest: String,
    endpoint_identity_digest: String,
    instance_identity_digest: String,
    leaf_inventory_digest: String,
    leaf_paths: Vec<String>,
}

impl CarrierFrameBinding {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        direction: Direction,
        descriptor_digest: impl Into<String>,
        endpoint_identity_digest: impl Into<String>,
        instance_identity_digest: impl Into<String>,
        leaf_paths: Vec<String>,
    ) -> Self {
        let leaf_inventory_digest = digest(LEAF_INVENTORY_DOMAIN, &framed_leaf_paths(&leaf_paths));
        Self {
            direction,
            descriptor_digest: descriptor_digest.into(),
            endpoint_identity_digest: endpoint_identity_digest.into(),
            instance_identity_digest: instance_identity_digest.into(),
            leaf_inventory_digest,
            leaf_paths,
        }
    }

    /// Derive the trusted binding for `direction` directly from a real,
    /// already-verified descriptor. Raw or merely parsed descriptor data has
    /// no `descriptor_digest()`/`input_facts()`/`result_facts()` to call —
    /// this constructor exists only for
    /// [`VerifiedPublicGenericDescriptor`], matching this issue's own
    /// requirement.
    pub fn from_verified_descriptor(
        descriptor: &VerifiedPublicGenericDescriptor,
        direction: Direction,
    ) -> Self {
        let facts = match direction {
            Direction::Input => descriptor.input_facts(),
            Direction::Result => descriptor.result_facts(),
        };
        let endpoint_identity_digest =
            digest(ENDPOINT_IDENTITY_DOMAIN, descriptor.export_id().as_bytes());
        Self::new(
            direction,
            descriptor.descriptor_digest(),
            endpoint_identity_digest,
            facts.instance_digest.clone(),
            facts.owned_leaves.clone(),
        )
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    pub fn leaf_paths(&self) -> &[String] {
        &self.leaf_paths
    }

    /// Require `frame` to be bound to exactly this plan: same direction,
    /// same descriptor/endpoint/instance/leaf-inventory digests, and the
    /// exact same leaf-path sequence, in canonical order — one `Vec`
    /// equality check that simultaneously rejects a missing leaf, an extra
    /// leaf, and a reordered leaf, since each of those changes the sequence
    /// relative to the canonical inventory. A frame that decodes cleanly
    /// (see [`parse_bounded`]) but is bound to a different descriptor,
    /// endpoint, instance, or direction — a "reminted carrier digest with
    /// the wrong semantic binding" — is rejected here, never at parse time,
    /// matching [`super::CarrierBindingV1::replay_binding`]'s own decode/bind
    /// split.
    pub fn validate_frame(&self, frame: &LogicalCarrierFrame) -> Result<(), Diagnostic> {
        if frame.direction != self.direction {
            return Err(replay_mismatch("direction does not match the trusted plan"));
        }
        if frame.descriptor_digest != self.descriptor_digest {
            return Err(replay_mismatch(
                "descriptor digest does not match the trusted plan",
            ));
        }
        if frame.endpoint_identity_digest != self.endpoint_identity_digest {
            return Err(replay_mismatch(
                "endpoint identity digest does not match the trusted plan",
            ));
        }
        if frame.instance_identity_digest != self.instance_identity_digest {
            return Err(replay_mismatch(
                "instance identity digest does not match the trusted plan",
            ));
        }
        if frame.leaf_inventory_digest != self.leaf_inventory_digest {
            return Err(replay_mismatch(
                "leaf inventory digest does not match the trusted plan",
            ));
        }
        let actual_paths: Vec<&str> = frame.leaves.iter().map(|leaf| leaf.path.as_str()).collect();
        let expected_paths: Vec<&str> = self.leaf_paths.iter().map(String::as_str).collect();
        if actual_paths != expected_paths {
            return Err(replay_mismatch(
                "leaf sequence does not match the trusted canonical inventory (missing, extra, \
                 or reordered leaf)",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
