//! Independent proof-only decoder for callable settlement-proof envelope v1.
//!
//! The proof envelope binds an unchanged callable-v2 descriptor to a
//! canonical, pointer-free settlement graph. Decoding this data grants no authority,
//! loads no symbol, and admits no physical finalizer.

#![forbid(unsafe_code)]
#![allow(
    dead_code,
    reason = "callable settlement-proof decoding is not an execution surface"
)]

use std::collections::HashSet;

use sha2::{Digest, Sha256};

use crate::descriptor_v2::{
    Descriptor as DescriptorV2, DescriptorError as DescriptorV2Error, Parameter, ResultShape,
};

const MAGIC: &[u8; 8] = b"SPXNPRF1";
const VERSION: u32 = 1;
const HEADER_SIZE: u32 = 20;
const FINGERPRINT_BYTES: usize = 32;
const MAX_PROOF_BYTES: usize = 64 * 1024;
const MAX_TEXT_BYTES: usize = 64 * 1024;
const MAX_RESOURCES: usize = 4_096;
const MAX_CHECKPOINTS: usize = 65_536;
const MAX_WORK_UNITS: usize = 1_000_000;
const GRAPH_VERSION: u32 = 1;
const FIXED_PREFIX_BYTES: usize = 20 + 4 * FINGERPRINT_BYTES + 4;

const SCHEMA_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-proof-schema.v1\0";
const V2_BYTES_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-proof-v2-bytes.v1\0";
const GRAPH_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-proof-graph.v1\0";
const ENVELOPE_DOMAIN: &[u8] = b"semaprax.native-callable-settlement-proof-envelope.v1\0";
const SCHEMA_STATEMENT: &[u8] = b"SPXNPRF1;u32le;header=20;body=schema32,v2_hash32,graph_hash32,envelope_hash32,v2_len32,v2_bytes,graph_len32,graph_bytes";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SettlementProofError {
    Malformed,
    UnsupportedSchema,
    WrongTarget,
    NonCanonical,
    ArtifactMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResourceState {
    Live,
    ProvisionalResult,
    Finalizing,
    Dead,
    Published,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Outcome {
    ScalarSuccess,
    SemanticFailure,
    OwnedSuccess(u32),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Action {
    Finalize(u32),
    StageOwnedResult(u32),
    CertifyOutcome([u8; 32]),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Checkpoint {
    id: u32,
    resources: Vec<ResourceState>,
    outcome: Option<Outcome>,
    abort_order: Vec<u32>,
    accept_order: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Edge {
    from: u32,
    to: u32,
    action: Action,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SettlementGraph {
    function: String,
    recovery_contract: [u8; 32],
    source_v2_call_contract: [u8; 32],
    trace_path_certificate_fingerprint: [u8; 32],
    resource_count: usize,
    checkpoints: Vec<Checkpoint>,
    starts: Vec<u32>,
    edges: Vec<Edge>,
}

/// Fully checked proof bytes. This type intentionally has no execution API.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BoundSettlementProof {
    pub(crate) callable_v2: DescriptorV2,
    graph: SettlementGraph,
    proof_bytes: Vec<u8>,
}

impl BoundSettlementProof {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, SettlementProofError> {
        if bytes.len() > MAX_PROOF_BYTES || bytes.len() < FIXED_PREFIX_BYTES + 4 {
            return Err(SettlementProofError::Malformed);
        }
        let mut reader = Reader::new(bytes);
        if reader.take(8)? != MAGIC || reader.u32()? != VERSION || reader.u32()? != HEADER_SIZE {
            return Err(SettlementProofError::UnsupportedSchema);
        }
        let declared = reader.usize()?;
        if declared != bytes.len() {
            return Err(SettlementProofError::Malformed);
        }
        let schema = reader.fingerprint()?;
        let v2_fingerprint = reader.fingerprint()?;
        let graph_fingerprint = reader.fingerprint()?;
        let envelope_fingerprint = reader.fingerprint()?;
        if schema != schema_fingerprint() {
            return Err(SettlementProofError::UnsupportedSchema);
        }
        if [
            schema,
            v2_fingerprint,
            graph_fingerprint,
            envelope_fingerprint,
        ]
        .contains(&[0; 32])
        {
            return Err(SettlementProofError::NonCanonical);
        }

        let v2_len = reader.usize()?;
        if v2_len == 0 || v2_len > MAX_PROOF_BYTES {
            return Err(SettlementProofError::NonCanonical);
        }
        let v2_bytes = reader.take(v2_len)?;
        let graph_len = reader.usize()?;
        if graph_len == 0 || graph_len > MAX_PROOF_BYTES {
            return Err(SettlementProofError::NonCanonical);
        }
        let graph_bytes = reader.take(graph_len)?;
        if !reader.is_finished() {
            return Err(SettlementProofError::Malformed);
        }

        let expected_v2 = payload_fingerprint(V2_BYTES_DOMAIN, v2_bytes);
        let expected_graph = payload_fingerprint(GRAPH_DOMAIN, graph_bytes);
        if v2_fingerprint != expected_v2 || graph_fingerprint != expected_graph {
            return Err(SettlementProofError::ArtifactMismatch);
        }
        let expected_envelope = envelope_fingerprint_for(
            &schema,
            &v2_fingerprint,
            &graph_fingerprint,
            v2_len,
            graph_len,
        )?;
        if envelope_fingerprint != expected_envelope {
            return Err(SettlementProofError::ArtifactMismatch);
        }

        let callable_v2 = DescriptorV2::parse(v2_bytes).map_err(map_v2_error)?;
        let graph = SettlementGraph::parse(graph_bytes)?;
        if encode_graph(&graph)? != graph_bytes {
            return Err(SettlementProofError::NonCanonical);
        }
        validate_cross_artifact(&callable_v2, &graph)?;
        Ok(Self {
            callable_v2,
            graph,
            proof_bytes: bytes.to_vec(),
        })
    }
}

impl SettlementGraph {
    fn parse(bytes: &[u8]) -> Result<Self, SettlementProofError> {
        let mut reader = Reader::new(bytes);
        if reader.u32()? != GRAPH_VERSION {
            return Err(SettlementProofError::UnsupportedSchema);
        }
        let function = reader.text(MAX_TEXT_BYTES)?;
        let recovery_contract = reader.fingerprint()?;
        let source_v2_call_contract = reader.fingerprint()?;
        let trace_path_certificate_fingerprint = reader.fingerprint()?;
        if [
            recovery_contract,
            source_v2_call_contract,
            trace_path_certificate_fingerprint,
        ]
        .contains(&[0; 32])
        {
            return Err(SettlementProofError::NonCanonical);
        }
        let resource_count = reader.usize()?;
        if resource_count == 0 || resource_count > MAX_RESOURCES {
            return Err(SettlementProofError::NonCanonical);
        }
        let checkpoint_count = reader.usize()?;
        if checkpoint_count == 0 || checkpoint_count > MAX_CHECKPOINTS {
            return Err(SettlementProofError::NonCanonical);
        }
        let base_work = resource_count
            .checked_mul(checkpoint_count)
            .ok_or(SettlementProofError::Malformed)?;
        if base_work > MAX_WORK_UNITS {
            return Err(SettlementProofError::NonCanonical);
        }
        // A checkpoint needs at least id/state-count/states/outcome/two counts.
        let min_checkpoint = 20_usize
            .checked_add(
                resource_count
                    .checked_mul(4)
                    .ok_or(SettlementProofError::Malformed)?,
            )
            .ok_or(SettlementProofError::Malformed)?;
        if checkpoint_count
            > reader
                .remaining()
                .checked_div(min_checkpoint)
                .ok_or(SettlementProofError::Malformed)?
        {
            return Err(SettlementProofError::Malformed);
        }
        let mut checkpoints = Vec::with_capacity(checkpoint_count);
        for index in 0..checkpoint_count {
            let id = reader.u32()?;
            let expected = u32::try_from(index + 1).map_err(|_| SettlementProofError::Malformed)?;
            if id != expected {
                return Err(SettlementProofError::NonCanonical);
            }
            let state_count = reader.usize()?;
            if state_count != resource_count {
                return Err(SettlementProofError::NonCanonical);
            }
            let mut resources = Vec::with_capacity(resource_count);
            for _ in 0..resource_count {
                resources.push(match reader.u32()? {
                    1 => ResourceState::Live,
                    2 => ResourceState::ProvisionalResult,
                    3 => ResourceState::Finalizing,
                    4 => ResourceState::Dead,
                    5 => ResourceState::Published,
                    _ => return Err(SettlementProofError::NonCanonical),
                });
            }
            let outcome = match reader.u32()? {
                0 => None,
                1 => Some(Outcome::ScalarSuccess),
                2 => Some(Outcome::SemanticFailure),
                3 => Some(Outcome::OwnedSuccess(reader.u32()?)),
                _ => return Err(SettlementProofError::NonCanonical),
            };
            let abort_order = reader.ordinals(resource_count)?;
            let accept_order = reader.ordinals(resource_count)?;
            let checkpoint = Checkpoint {
                id,
                resources,
                outcome,
                abort_order,
                accept_order,
            };
            validate_checkpoint(&checkpoint, resource_count)?;
            checkpoints.push(checkpoint);
        }

        let start_count = reader.usize()?;
        if start_count > checkpoint_count {
            return Err(SettlementProofError::NonCanonical);
        }
        let mut starts = Vec::with_capacity(start_count);
        for _ in 0..start_count {
            starts.push(reader.u32()?);
        }
        let edge_count = reader.usize()?;
        let progress_work = base_work
            .checked_add(start_count)
            .and_then(|work| work.checked_add(edge_count))
            .ok_or(SettlementProofError::Malformed)?;
        if progress_work > MAX_WORK_UNITS || edge_count > reader.remaining() / 12 {
            return Err(SettlementProofError::NonCanonical);
        }
        let mut edges = Vec::with_capacity(edge_count);
        for _ in 0..edge_count {
            let from = reader.u32()?;
            let to = reader.u32()?;
            let action = match reader.u32()? {
                1 => Action::Finalize(reader.u32()?),
                2 => Action::StageOwnedResult(reader.u32()?),
                3 => Action::CertifyOutcome(reader.fingerprint()?),
                _ => return Err(SettlementProofError::NonCanonical),
            };
            edges.push(Edge { from, to, action });
        }
        if !reader.is_finished() {
            return Err(SettlementProofError::Malformed);
        }
        validate_progress(&checkpoints, &starts, &edges)?;
        Ok(Self {
            function,
            recovery_contract,
            source_v2_call_contract,
            trace_path_certificate_fingerprint,
            resource_count,
            checkpoints,
            starts,
            edges,
        })
    }
}

fn validate_checkpoint(
    checkpoint: &Checkpoint,
    resource_count: usize,
) -> Result<(), SettlementProofError> {
    if checkpoint.resources.len() != resource_count
        || checkpoint
            .resources
            .iter()
            .any(|state| matches!(state, ResourceState::Finalizing | ResourceState::Published))
    {
        return Err(SettlementProofError::NonCanonical);
    }
    let provisional = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter_map(|(ordinal, state)| {
            (*state == ResourceState::ProvisionalResult).then_some(ordinal as u32)
        })
        .collect::<Vec<_>>();
    if provisional.len() > 1 {
        return Err(SettlementProofError::NonCanonical);
    }
    let abort_required = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter_map(|(ordinal, state)| (*state != ResourceState::Dead).then_some(ordinal as u32))
        .collect::<HashSet<_>>();
    validate_exact_order(&checkpoint.abort_order, &abort_required)?;
    let accept_required = checkpoint
        .resources
        .iter()
        .enumerate()
        .filter_map(|(ordinal, state)| (*state == ResourceState::Live).then_some(ordinal as u32))
        .collect::<HashSet<_>>();
    match checkpoint.outcome {
        None if checkpoint.accept_order.is_empty() => {}
        None => return Err(SettlementProofError::NonCanonical),
        Some(Outcome::ScalarSuccess | Outcome::SemanticFailure) if provisional.is_empty() => {
            validate_exact_order(&checkpoint.accept_order, &accept_required)?;
        }
        Some(Outcome::OwnedSuccess(owner)) if provisional.as_slice() == [owner] => {
            validate_exact_order(&checkpoint.accept_order, &accept_required)?;
        }
        Some(_) => return Err(SettlementProofError::NonCanonical),
    }
    Ok(())
}

fn validate_exact_order(
    order: &[u32],
    required: &HashSet<u32>,
) -> Result<(), SettlementProofError> {
    let actual = order.iter().copied().collect::<HashSet<_>>();
    if actual.len() != order.len() || actual != *required {
        return Err(SettlementProofError::NonCanonical);
    }
    Ok(())
}

fn validate_progress(
    checkpoints: &[Checkpoint],
    starts: &[u32],
    edges: &[Edge],
) -> Result<(), SettlementProofError> {
    if starts != [1]
        || checkpoints[0].outcome.is_some()
        || checkpoints[0]
            .resources
            .iter()
            .any(|state| *state != ResourceState::Live)
    {
        return Err(SettlementProofError::NonCanonical);
    }
    let mut seen_edges = HashSet::new();
    let mut seen_actions = HashSet::new();
    let mut reachable = HashSet::from([1_u32]);
    let mut outgoing = HashSet::new();
    for edge in edges {
        if edge.from == 0
            || edge.to == 0
            || edge.from >= edge.to
            || edge.to as usize > checkpoints.len()
            || !seen_edges.insert(*edge)
            || !seen_actions.insert((edge.from, edge.action))
            || !reachable.contains(&edge.from)
        {
            return Err(SettlementProofError::NonCanonical);
        }
        let from = &checkpoints[(edge.from - 1) as usize];
        let to = &checkpoints[(edge.to - 1) as usize];
        if from.outcome.is_some() || !valid_transition(from, to, edge.action) {
            return Err(SettlementProofError::NonCanonical);
        }
        outgoing.insert(edge.from);
        reachable.insert(edge.to);
    }
    if reachable.len() != checkpoints.len()
        || checkpoints
            .iter()
            .any(|checkpoint| checkpoint.outcome.is_none() != outgoing.contains(&checkpoint.id))
    {
        return Err(SettlementProofError::NonCanonical);
    }
    Ok(())
}

fn valid_transition(from: &Checkpoint, to: &Checkpoint, action: Action) -> bool {
    match action {
        Action::Finalize(owner) => {
            let Some(position) = from
                .abort_order
                .iter()
                .position(|candidate| *candidate == owner)
            else {
                return false;
            };
            let prefix_only_provisional = from.abort_order[..position].iter().all(|ordinal| {
                from.resources[*ordinal as usize] == ResourceState::ProvisionalResult
            });
            let mut expected_abort = from.abort_order.clone();
            expected_abort.remove(position);
            let state_transition =
                changed_state(from, to, owner, ResourceState::Live, ResourceState::Dead)
                    || changed_state(
                        from,
                        to,
                        owner,
                        ResourceState::ProvisionalResult,
                        ResourceState::Dead,
                    );
            to.outcome.is_none()
                && state_transition
                && from.accept_order.is_empty()
                && to.accept_order.is_empty()
                && prefix_only_provisional
                && to.abort_order == expected_abort
        }
        Action::StageOwnedResult(owner) => {
            to.outcome.is_none()
                && changed_state(
                    from,
                    to,
                    owner,
                    ResourceState::Live,
                    ResourceState::ProvisionalResult,
                )
                && from.accept_order.is_empty()
                && to.accept_order.is_empty()
                && from.abort_order == to.abort_order
        }
        Action::CertifyOutcome(trace) => {
            let expected_accept = to
                .abort_order
                .iter()
                .copied()
                .filter(|ordinal| to.resources[*ordinal as usize] == ResourceState::Live)
                .collect::<Vec<_>>();
            from.outcome.is_none()
                && to.outcome.is_some()
                && from.resources == to.resources
                && from.accept_order.is_empty()
                && from.abort_order == to.abort_order
                && to.accept_order == expected_accept
                && trace != [0; 32]
        }
    }
}

fn changed_state(
    from: &Checkpoint,
    to: &Checkpoint,
    owner: u32,
    expected_from: ResourceState,
    expected_to: ResourceState,
) -> bool {
    let Ok(owner) = usize::try_from(owner) else {
        return false;
    };
    owner < from.resources.len()
        && from.resources.len() == to.resources.len()
        && from
            .resources
            .iter()
            .zip(&to.resources)
            .enumerate()
            .all(|(index, (left, right))| {
                if index == owner {
                    *left == expected_from && *right == expected_to
                } else {
                    left == right
                }
            })
}

fn validate_cross_artifact(
    descriptor: &DescriptorV2,
    graph: &SettlementGraph,
) -> Result<(), SettlementProofError> {
    if descriptor.function != graph.function {
        return Err(SettlementProofError::ArtifactMismatch);
    }
    if descriptor.fingerprints.call_contract != graph.source_v2_call_contract
        || descriptor.fingerprints.trace_path_certificate
            != graph.trace_path_certificate_fingerprint
    {
        return Err(SettlementProofError::ArtifactMismatch);
    }
    let owned_count = descriptor
        .parameters
        .iter()
        .filter(|parameter| matches!(parameter, Parameter::Owned { .. }))
        .count();
    if owned_count != graph.resource_count {
        return Err(SettlementProofError::ArtifactMismatch);
    }
    for checkpoint in &graph.checkpoints {
        match (descriptor.result, checkpoint.outcome) {
            (_, None | Some(Outcome::SemanticFailure)) => {}
            (ResultShape::ScalarI64, Some(Outcome::ScalarSuccess)) => {}
            (
                ResultShape::OwnedInput { owner_ordinal, .. },
                Some(Outcome::OwnedSuccess(graph_owner)),
            ) if owner_ordinal == graph_owner as usize => {}
            _ => return Err(SettlementProofError::ArtifactMismatch),
        }
    }
    Ok(())
}

fn encode_graph(graph: &SettlementGraph) -> Result<Vec<u8>, SettlementProofError> {
    let mut writer = Writer::new();
    writer.u32(GRAPH_VERSION);
    writer.text(&graph.function)?;
    writer.bytes(&graph.recovery_contract);
    writer.bytes(&graph.source_v2_call_contract);
    writer.bytes(&graph.trace_path_certificate_fingerprint);
    writer.usize(graph.resource_count)?;
    writer.usize(graph.checkpoints.len())?;
    for checkpoint in &graph.checkpoints {
        writer.u32(checkpoint.id);
        writer.usize(checkpoint.resources.len())?;
        for state in &checkpoint.resources {
            writer.u32(match state {
                ResourceState::Live => 1,
                ResourceState::ProvisionalResult => 2,
                ResourceState::Finalizing => 3,
                ResourceState::Dead => 4,
                ResourceState::Published => 5,
            });
        }
        match checkpoint.outcome {
            None => writer.u32(0),
            Some(Outcome::ScalarSuccess) => writer.u32(1),
            Some(Outcome::SemanticFailure) => writer.u32(2),
            Some(Outcome::OwnedSuccess(owner)) => {
                writer.u32(3);
                writer.u32(owner);
            }
        }
        writer.ordinals(&checkpoint.abort_order)?;
        writer.ordinals(&checkpoint.accept_order)?;
    }
    writer.ordinals(&graph.starts)?;
    writer.usize(graph.edges.len())?;
    for edge in &graph.edges {
        writer.u32(edge.from);
        writer.u32(edge.to);
        match edge.action {
            Action::Finalize(owner) => {
                writer.u32(1);
                writer.u32(owner);
            }
            Action::StageOwnedResult(owner) => {
                writer.u32(2);
                writer.u32(owner);
            }
            Action::CertifyOutcome(trace) => {
                writer.u32(3);
                writer.bytes(&trace);
            }
        }
    }
    Ok(writer.finish())
}

fn schema_fingerprint() -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SCHEMA_DOMAIN);
    hash_field(&mut hasher, SCHEMA_STATEMENT);
    hasher.finalize().into()
}

fn payload_fingerprint(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn envelope_fingerprint_for(
    schema: &[u8; 32],
    v2: &[u8; 32],
    graph: &[u8; 32],
    v2_len: usize,
    graph_len: usize,
) -> Result<[u8; 32], SettlementProofError> {
    let v2_len = u32::try_from(v2_len).map_err(|_| SettlementProofError::Malformed)?;
    let graph_len = u32::try_from(graph_len).map_err(|_| SettlementProofError::Malformed)?;
    let mut hasher = Sha256::new();
    hasher.update(ENVELOPE_DOMAIN);
    hash_field(&mut hasher, schema);
    hash_field(&mut hasher, v2);
    hash_field(&mut hasher, graph);
    hash_field(&mut hasher, &v2_len.to_le_bytes());
    hash_field(&mut hasher, &graph_len.to_le_bytes());
    Ok(hasher.finalize().into())
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn map_v2_error(error: DescriptorV2Error) -> SettlementProofError {
    match error {
        DescriptorV2Error::Malformed => SettlementProofError::Malformed,
        DescriptorV2Error::UnsupportedSchema => SettlementProofError::UnsupportedSchema,
        DescriptorV2Error::WrongTarget => SettlementProofError::WrongTarget,
        DescriptorV2Error::NonCanonical => SettlementProofError::NonCanonical,
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], SettlementProofError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(SettlementProofError::Malformed)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(SettlementProofError::Malformed)?;
        self.offset = end;
        Ok(bytes)
    }

    fn u32(&mut self) -> Result<u32, SettlementProofError> {
        let bytes: [u8; 4] = self
            .take(4)?
            .try_into()
            .map_err(|_| SettlementProofError::Malformed)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn usize(&mut self) -> Result<usize, SettlementProofError> {
        usize::try_from(self.u32()?).map_err(|_| SettlementProofError::Malformed)
    }

    fn fingerprint(&mut self) -> Result<[u8; 32], SettlementProofError> {
        self.take(32)?
            .try_into()
            .map_err(|_| SettlementProofError::Malformed)
    }

    fn text(&mut self, max: usize) -> Result<String, SettlementProofError> {
        let len = self.usize()?;
        if len == 0 || len > max {
            return Err(SettlementProofError::NonCanonical);
        }
        let bytes = self.take(len)?;
        let text = std::str::from_utf8(bytes).map_err(|_| SettlementProofError::Malformed)?;
        if text.contains('\0') {
            return Err(SettlementProofError::NonCanonical);
        }
        Ok(text.to_owned())
    }

    fn ordinals(&mut self, max: usize) -> Result<Vec<u32>, SettlementProofError> {
        let count = self.usize()?;
        if count > max || count > self.remaining() / 4 {
            return Err(SettlementProofError::NonCanonical);
        }
        let mut ordinals = Vec::with_capacity(count);
        for _ in 0..count {
            let ordinal = self.u32()?;
            if ordinal as usize >= max {
                return Err(SettlementProofError::NonCanonical);
            }
            ordinals.push(ordinal);
        }
        Ok(ordinals)
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) -> Result<(), SettlementProofError> {
        self.u32(u32::try_from(value).map_err(|_| SettlementProofError::Malformed)?);
        Ok(())
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn text(&mut self, value: &str) -> Result<(), SettlementProofError> {
        self.usize(value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }

    fn ordinals(&mut self, values: &[u32]) -> Result<(), SettlementProofError> {
        self.usize(values.len())?;
        for value in values {
            self.u32(*value);
        }
        Ok(())
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
#[path = "settlement_proof/tests.rs"]
mod tests;
