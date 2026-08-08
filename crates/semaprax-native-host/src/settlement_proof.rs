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
mod tests {
    use std::collections::BTreeSet;
    use std::path::Path;

    use semaprax::codegen::emit_native_callable_settlement_proof;
    use semaprax::hir::{self, DeclarationId};
    use semaprax::owned_resource_corpus::build_owned_resource_corpus_v1;

    use super::*;

    const SOURCE: &str = r#"module test.callable_proof_host;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.consume")
fn consume(value: own Token) -> i64 {
    7
}

@id("token.keep")
fn keep(value: own Token) -> Token {
    value
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn compiler_proof(source: &str, function: &str) -> Vec<u8> {
        let parsed = semaprax::parse(source, Path::new("callable-proof-host.spx")).unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        emit_native_callable_settlement_proof(&resolved, &DeclarationId::new(function))
            .unwrap()
            .bytes()
            .to_vec()
    }

    fn canonical() -> Vec<u8> {
        compiler_proof(SOURCE, "token.consume")
    }

    fn components(proof: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let v2_len = u32::from_le_bytes(proof[148..152].try_into().unwrap()) as usize;
        let v2_start = 152;
        let v2_end = v2_start + v2_len;
        let graph_len = u32::from_le_bytes(proof[v2_end..v2_end + 4].try_into().unwrap()) as usize;
        let graph_start = v2_end + 4;
        (
            proof[v2_start..v2_end].to_vec(),
            proof[graph_start..graph_start + graph_len].to_vec(),
        )
    }

    fn envelope(v2: &[u8], graph: &[u8]) -> Vec<u8> {
        let schema = schema_fingerprint();
        let v2_fingerprint = payload_fingerprint(V2_BYTES_DOMAIN, v2);
        let graph_fingerprint = payload_fingerprint(GRAPH_DOMAIN, graph);
        let envelope_fingerprint = envelope_fingerprint_for(
            &schema,
            &v2_fingerprint,
            &graph_fingerprint,
            v2.len(),
            graph.len(),
        )
        .unwrap();
        let total = 20 + 4 * 32 + 4 + v2.len() + 4 + graph.len();
        let mut bytes = Vec::with_capacity(total);
        bytes.extend_from_slice(MAGIC);
        bytes.extend_from_slice(&VERSION.to_le_bytes());
        bytes.extend_from_slice(&HEADER_SIZE.to_le_bytes());
        bytes.extend_from_slice(&(total as u32).to_le_bytes());
        bytes.extend_from_slice(&schema);
        bytes.extend_from_slice(&v2_fingerprint);
        bytes.extend_from_slice(&graph_fingerprint);
        bytes.extend_from_slice(&envelope_fingerprint);
        bytes.extend_from_slice(&(v2.len() as u32).to_le_bytes());
        bytes.extend_from_slice(v2);
        bytes.extend_from_slice(&(graph.len() as u32).to_le_bytes());
        bytes.extend_from_slice(graph);
        bytes
    }

    fn decoded_components() -> (Vec<u8>, SettlementGraph) {
        let proof = canonical();
        let (v2, graph) = components(&proof);
        (v2, SettlementGraph::parse(&graph).unwrap())
    }

    #[test]
    fn independently_accepts_exact_compiler_proof() {
        let first = canonical();
        let second = canonical();
        assert_eq!(first, second);
        assert_eq!(&first[..8], b"SPXNPRF1");
        let bound = BoundSettlementProof::parse(&first).unwrap();
        assert_eq!(bound.callable_v2.function, "token.consume");
        assert_eq!(bound.graph.function, "token.consume");
        assert_eq!(
            bound.graph.source_v2_call_contract,
            bound.callable_v2.fingerprints.call_contract
        );
        assert_eq!(
            bound.graph.trace_path_certificate_fingerprint,
            bound.callable_v2.fingerprints.trace_path_certificate
        );
        assert_eq!(bound.graph.starts, [1]);
        assert!(bound
            .graph
            .edges
            .iter()
            .any(|edge| matches!(edge.action, Action::Finalize(0))));
        assert!(bound
            .graph
            .edges
            .iter()
            .any(|edge| matches!(edge.action, Action::CertifyOutcome(_))));
        assert_eq!(bound.proof_bytes, first);
    }

    #[test]
    fn independently_accepts_owned_result_staging() {
        let owned = compiler_proof(SOURCE, "token.keep");
        let bound = BoundSettlementProof::parse(&owned).unwrap();
        assert!(matches!(
            bound.callable_v2.result,
            ResultShape::OwnedInput {
                owner_ordinal: 0,
                ..
            }
        ));
        assert!(bound
            .graph
            .edges
            .iter()
            .any(|edge| matches!(edge.action, Action::StageOwnedResult(0))));
        assert!(bound
            .graph
            .checkpoints
            .iter()
            .any(|checkpoint| checkpoint.outcome == Some(Outcome::OwnedSuccess(0))));
    }

    #[test]
    fn independently_accepts_all_fourteen_authoritative_corpus_cases() {
        let corpus = build_owned_resource_corpus_v1().unwrap();
        assert_eq!(corpus.cases.len(), 14);
        let mut functions = BTreeSet::new();
        let mut outcomes = BTreeSet::new();
        for case in &corpus.cases {
            let proof = emit_native_callable_settlement_proof(
                &corpus.program,
                &DeclarationId::new(case.function_id),
            )
            .unwrap();
            let bound = BoundSettlementProof::parse(proof.bytes()).unwrap();
            assert_eq!(bound.callable_v2.function, case.function_id);
            functions.insert(case.function_id);
            for checkpoint in &bound.graph.checkpoints {
                if let Some(outcome) = checkpoint.outcome {
                    outcomes.insert(match outcome {
                        Outcome::ScalarSuccess => 1,
                        Outcome::SemanticFailure => 2,
                        Outcome::OwnedSuccess(_) => 3,
                    });
                }
            }
        }
        assert_eq!(functions.len(), 7);
        assert_eq!(outcomes, BTreeSet::from([1, 2, 3]));
    }

    #[test]
    fn rejects_every_prefix_truncation_trailing_byte_and_single_byte_mutation() {
        let bytes = canonical();
        for length in 0..bytes.len() {
            assert!(
                BoundSettlementProof::parse(&bytes[..length]).is_err(),
                "accepted prefix length {length}"
            );
        }
        for trailing in [0_u8, 1, 0x7f, 0xff] {
            let mut hostile = bytes.clone();
            hostile.push(trailing);
            assert!(BoundSettlementProof::parse(&hostile).is_err());
        }
        for offset in 0..bytes.len() {
            let mut hostile = bytes.clone();
            hostile[offset] ^= 1;
            assert!(
                BoundSettlementProof::parse(&hostile).is_err(),
                "accepted mutation at {offset}"
            );
        }
    }

    #[test]
    fn rejects_rehashed_hostile_graph_semantics_and_binding_zeros() {
        let (v2, canonical_graph) = decoded_components();
        let mut cases = Vec::new();
        let mut function = canonical_graph.clone();
        function.function = "token.other".to_owned();
        cases.push(function);
        let mut recovery = canonical_graph.clone();
        recovery.recovery_contract = [0; 32];
        cases.push(recovery);
        let mut call_contract = canonical_graph.clone();
        call_contract.source_v2_call_contract = [0; 32];
        cases.push(call_contract);
        let mut trace = canonical_graph.clone();
        trace.trace_path_certificate_fingerprint = [0; 32];
        cases.push(trace);
        let mut resources = canonical_graph.clone();
        resources.resource_count += 1;
        cases.push(resources);
        let mut checkpoint = canonical_graph.clone();
        checkpoint.checkpoints[0].id = 2;
        cases.push(checkpoint);
        let mut state = canonical_graph.clone();
        state.checkpoints[0].resources[0] = ResourceState::Finalizing;
        cases.push(state);
        let mut abort_order = canonical_graph.clone();
        abort_order.checkpoints[0].abort_order.clear();
        cases.push(abort_order);
        let mut start = canonical_graph.clone();
        start.starts = vec![2];
        cases.push(start);
        let mut edge = canonical_graph.clone();
        edge.edges[0].to = edge.edges[0].from;
        cases.push(edge);
        let mut trace_evidence = canonical_graph;
        let certify = trace_evidence
            .edges
            .iter_mut()
            .find(|edge| matches!(edge.action, Action::CertifyOutcome(_)))
            .unwrap();
        certify.action = Action::CertifyOutcome([0; 32]);
        cases.push(trace_evidence);

        for hostile_graph in cases {
            let hostile = envelope(&v2, &encode_graph(&hostile_graph).unwrap());
            assert!(BoundSettlementProof::parse(&hostile).is_err());
        }
    }

    #[test]
    fn rejects_rehashed_unknown_tags_invalid_text_and_hostile_counts() {
        let proof = canonical();
        let (v2, canonical_graph) = components(&proof);
        let function_start = 8;
        let function_len = "token.consume".len();
        let recovery_start = function_start + function_len;
        let resource_count = recovery_start + 32 + 32 + 32;
        let checkpoint_count = resource_count + 4;
        let first_state_tag = checkpoint_count + 4 + 4 + 4;

        let mut cases = Vec::new();
        let mut bad_utf8 = canonical_graph.clone();
        bad_utf8[function_start] = 0xff;
        cases.push(bad_utf8);
        let mut nul = canonical_graph.clone();
        nul[function_start] = 0;
        cases.push(nul);
        let mut resource_overflow = canonical_graph.clone();
        resource_overflow[resource_count..resource_count + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        cases.push(resource_overflow);
        let mut checkpoint_overflow = canonical_graph.clone();
        checkpoint_overflow[checkpoint_count..checkpoint_count + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        cases.push(checkpoint_overflow);
        let mut unknown_state = canonical_graph;
        unknown_state[first_state_tag..first_state_tag + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        cases.push(unknown_state);

        for graph in cases {
            assert!(BoundSettlementProof::parse(&envelope(&v2, &graph)).is_err());
        }
    }

    #[test]
    fn rejects_rehashed_same_shape_cross_module_graph_swap() {
        let other_module = SOURCE.replace(
            "module test.callable_proof_host;",
            "module test.callable_proof_other;",
        );
        let left = canonical();
        let right = compiler_proof(&other_module, "token.consume");
        let (left_v2, _) = components(&left);
        let (_, right_graph) = components(&right);
        assert_eq!(
            BoundSettlementProof::parse(&envelope(&left_v2, &right_graph)),
            Err(SettlementProofError::ArtifactMismatch)
        );
    }

    #[test]
    fn rejects_rehashed_same_module_function_changed_trace_graph_swap() {
        let changed_trace = SOURCE.replace(
            "fn consume(value: own Token) -> i64 {\n    7\n}",
            "fn consume(value: own Token) -> i64\nrequires true\n{\n    7\n}",
        );
        let left = canonical();
        let right = compiler_proof(&changed_trace, "token.consume");
        let (left_v2, _) = components(&left);
        let (_, right_graph) = components(&right);
        let left_descriptor = DescriptorV2::parse(&left_v2).unwrap();
        let right_v2 = components(&right).0;
        let right_descriptor = DescriptorV2::parse(&right_v2).unwrap();
        assert_ne!(
            left_descriptor.fingerprints.trace_path_certificate,
            right_descriptor.fingerprints.trace_path_certificate
        );
        assert_eq!(
            BoundSettlementProof::parse(&envelope(&left_v2, &right_graph)),
            Err(SettlementProofError::ArtifactMismatch)
        );
    }

    #[test]
    fn rejects_proof_over_exact_global_cap() {
        let hostile = vec![0_u8; MAX_PROOF_BYTES + 1];
        assert_eq!(
            BoundSettlementProof::parse(&hostile),
            Err(SettlementProofError::Malformed)
        );
    }
}
