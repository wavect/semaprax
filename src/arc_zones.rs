//! Target-neutral deterministic model of shared-immutable ARC inside explicit
//! opt-in managed zones.
//!
//! This module deliberately contains no allocator, no reference-counted smart
//! pointer, no runtime integration, no language syntax, and no compiler/backend
//! wiring. It fixes the bounded proof data that a future managed-zone
//! implementation must preserve: a bounded object graph per zone, a
//! retain/release state machine with exact deterministic finalization order
//! (reverse construction, cycle-participation deferral), a closed cycle policy
//! that rejects cycles at zone exit with canonical diagnostics instead of
//! leaking silently, escape-demotion rewrite facts for proven zone-local shared
//! handles, and closed concurrency annotations under which zones are
//! single-threaded by declaration and cross-thread sharing requires an
//! explicit `Shareable` mark.
//!
//! Like the callable-v3 settlement model, everything here is evidence, not
//! authority: an `ArcZonesRun` records what a conforming implementation MUST
//! do; it allocates nothing, finalizes nothing, and grants no capability. It
//! performs no aliasing or liveness analysis of real programs.

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use sha2::{Digest, Sha256};

use crate::diagnostic::quote_json;
use crate::hir::DeclarationId;

pub const ARC_ZONES_MODEL_V1: &str = "semaprax.arc-zones-model.v1";
pub const ARC_ZONES_TRACE_V1: &str = "semaprax.arc-zones-trace.v1";

pub const MAX_ZONES: usize = 4_096;
pub const MAX_OBJECTS: usize = 4_096;
pub const MAX_SCRIPT_OPS: usize = 65_536;
const MODEL_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.arc-zones-model-fingerprint.v1\0";
const TRACE_FINGERPRINT_DOMAIN: &[u8] = b"semaprax.arc-zones-trace-fingerprint.v1\0";

/// Closed concurrency annotation on one object. Zones are single-threaded by
/// declaration; this mark is the only admitted way to share an object across
/// zones whose declared threads differ. The model performs no thread analysis.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ShareableMark {
    Shareable,
    NotShareable,
}

/// One managed zone in the containment tree. The root zone has no parent; every
/// other zone names exactly one existing parent. Each zone declares its single
/// executing thread; there is no inside-zone parallelism by declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ZoneSpec {
    id: DeclarationId,
    parent: Option<DeclarationId>,
    thread: DeclarationId,
}

impl ZoneSpec {
    #[must_use]
    pub fn root(id: impl Into<String>, thread: impl Into<String>) -> Self {
        Self {
            id: DeclarationId::new(id),
            parent: None,
            thread: DeclarationId::new(thread),
        }
    }

    #[must_use]
    pub fn child(
        id: impl Into<String>,
        parent: impl Into<String>,
        thread: impl Into<String>,
    ) -> Self {
        Self {
            id: DeclarationId::new(id),
            parent: Some(DeclarationId::new(parent)),
            thread: DeclarationId::new(thread),
        }
    }

    #[must_use]
    pub fn id(&self) -> &DeclarationId {
        &self.id
    }

    #[must_use]
    pub fn parent(&self) -> Option<&DeclarationId> {
        self.parent.as_ref()
    }

    #[must_use]
    pub fn thread(&self) -> &DeclarationId {
        &self.thread
    }
}

/// One modeled shared-immutable object: identity, home zone, and the closed
/// concurrency annotation. Objects are constructed by the script, never by the
/// model; no allocation is performed or implied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObjectSpec {
    id: DeclarationId,
    zone: DeclarationId,
    shareable: ShareableMark,
}

impl ObjectSpec {
    #[must_use]
    pub fn new(id: impl Into<String>, zone: impl Into<String>, shareable: ShareableMark) -> Self {
        Self {
            id: DeclarationId::new(id),
            zone: DeclarationId::new(zone),
            shareable,
        }
    }

    #[must_use]
    pub fn id(&self) -> &DeclarationId {
        &self.id
    }

    #[must_use]
    pub fn zone(&self) -> &DeclarationId {
        &self.zone
    }

    #[must_use]
    pub const fn shareable(&self) -> ShareableMark {
        self.shareable
    }
}

/// Closed scripted operation. Operations execute strictly in script order; the
/// script is ordered semantics, while the declarative zone/object inventories
/// are canonically reordered so input permutation cannot change projections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Op {
    Construct {
        object: DeclarationId,
    },
    Retain {
        object: DeclarationId,
    },
    Release {
        object: DeclarationId,
    },
    Link {
        from: DeclarationId,
        to: DeclarationId,
    },
    Unlink {
        from: DeclarationId,
        to: DeclarationId,
    },
    Demote {
        object: DeclarationId,
    },
    EnterZone {
        zone: DeclarationId,
    },
    ExitZone {
        zone: DeclarationId,
    },
}

impl Op {
    #[must_use]
    pub fn construct(object: impl Into<String>) -> Self {
        Self::Construct {
            object: DeclarationId::new(object),
        }
    }

    #[must_use]
    pub fn retain(object: impl Into<String>) -> Self {
        Self::Retain {
            object: DeclarationId::new(object),
        }
    }

    #[must_use]
    pub fn release(object: impl Into<String>) -> Self {
        Self::Release {
            object: DeclarationId::new(object),
        }
    }

    #[must_use]
    pub fn link(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::Link {
            from: DeclarationId::new(from),
            to: DeclarationId::new(to),
        }
    }

    #[must_use]
    pub fn unlink(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self::Unlink {
            from: DeclarationId::new(from),
            to: DeclarationId::new(to),
        }
    }

    #[must_use]
    pub fn demote(object: impl Into<String>) -> Self {
        Self::Demote {
            object: DeclarationId::new(object),
        }
    }

    #[must_use]
    pub fn enter_zone(zone: impl Into<String>) -> Self {
        Self::EnterZone {
            zone: DeclarationId::new(zone),
        }
    }

    #[must_use]
    pub fn exit_zone(zone: impl Into<String>) -> Self {
        Self::ExitZone {
            zone: DeclarationId::new(zone),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ZoneEntry {
    parent: Option<DeclarationId>,
    depth: u32,
    thread: DeclarationId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObjectEntry {
    zone: DeclarationId,
    shareable: ShareableMark,
}

/// Immutable certified structure: the bounded zone tree and object inventory.
/// Construction rejects every structural ambiguity and computes a
/// domain-separated fingerprint. Inventories are canonically ordered, so input
/// order cannot change any projection. Declared links are not part of the
/// static structure: they are created and destroyed by script operations, so
/// cycles are observed where they arise, at zone exit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcZonesModel {
    schema: &'static str,
    root: DeclarationId,
    zones: BTreeMap<DeclarationId, ZoneEntry>,
    objects: BTreeMap<DeclarationId, ObjectEntry>,
    fingerprint: [u8; 32],
}

impl ArcZonesModel {
    /// Build and fully validate one bounded model.
    pub fn try_new(zones: Vec<ZoneSpec>, objects: Vec<ObjectSpec>) -> Result<Self, ArcZonesError> {
        if zones.is_empty() {
            return Err(ArcZonesError::MissingRoot);
        }
        if zones.len() > MAX_ZONES {
            return Err(ArcZonesError::ZoneBoundExceeded);
        }
        if objects.len() > MAX_OBJECTS {
            return Err(ArcZonesError::ObjectBoundExceeded);
        }
        let work = (zones.len() as u64)
            .checked_mul(objects.len() as u64 + 1)
            .ok_or(ArcZonesError::WorkBudgetExceeded)?
            .checked_add(objects.len() as u64)
            .ok_or(ArcZonesError::WorkBudgetExceeded)?;
        if work > 1_000_000 {
            return Err(ArcZonesError::WorkBudgetExceeded);
        }

        let mut parents = BTreeMap::<DeclarationId, Option<DeclarationId>>::new();
        let mut threads = BTreeMap::<DeclarationId, DeclarationId>::new();
        for zone in &zones {
            validate_identity(zone.id.as_str())?;
            validate_identity(zone.thread.as_str())?;
            if parents
                .insert(zone.id.clone(), zone.parent.clone())
                .is_some()
            {
                return Err(ArcZonesError::DuplicateZone);
            }
            threads.insert(zone.id.clone(), zone.thread.clone());
        }
        match zones.iter().filter(|zone| zone.parent.is_none()).count() {
            0 => return Err(ArcZonesError::MissingRoot),
            1 => {}
            _ => return Err(ArcZonesError::MultipleRoots),
        }
        for zone in &zones {
            if let Some(parent) = &zone.parent {
                if zone.id == *parent {
                    return Err(ArcZonesError::ZoneCycle);
                }
                if !parents.contains_key(parent) {
                    return Err(ArcZonesError::UnknownZone);
                }
            }
        }
        let depths = canonical_depths(&parents)?;
        let mut zone_entries = BTreeMap::new();
        for (id, parent) in &parents {
            zone_entries.insert(
                id.clone(),
                ZoneEntry {
                    parent: parent.clone(),
                    depth: depths[id],
                    thread: threads[id].clone(),
                },
            );
        }
        let root = zones
            .iter()
            .find(|zone| zone.parent.is_none())
            .map(|zone| zone.id.clone())
            .expect("exactly one root was validated");

        let mut object_entries = BTreeMap::<DeclarationId, ObjectEntry>::new();
        for object in &objects {
            validate_identity(object.id.as_str())?;
            if !zone_entries.contains_key(&object.zone) {
                return Err(ArcZonesError::UnknownZone);
            }
            if object_entries.contains_key(&object.id) {
                return Err(ArcZonesError::DuplicateObject);
            }
            object_entries.insert(
                object.id.clone(),
                ObjectEntry {
                    zone: object.zone.clone(),
                    shareable: object.shareable,
                },
            );
        }

        let mut model = Self {
            schema: ARC_ZONES_MODEL_V1,
            root,
            zones: zone_entries,
            objects: object_entries,
            fingerprint: [0; 32],
        };
        model.fingerprint =
            fingerprint(MODEL_FINGERPRINT_DOMAIN, model.canonical_json().as_bytes());
        Ok(model)
    }

    #[must_use]
    pub const fn schema(&self) -> &'static str {
        self.schema
    }

    #[must_use]
    pub fn root(&self) -> &DeclarationId {
        &self.root
    }

    pub fn zones(&self) -> impl Iterator<Item = (&DeclarationId, Option<&DeclarationId>)> {
        self.zones
            .iter()
            .map(|(id, entry)| (id, entry.parent.as_ref()))
    }

    pub fn objects(&self) -> impl Iterator<Item = (&DeclarationId, &DeclarationId, ShareableMark)> {
        self.objects
            .iter()
            .map(|(id, entry)| (id, &entry.zone, entry.shareable))
    }

    #[must_use]
    pub const fn fingerprint(&self) -> [u8; 32] {
        self.fingerprint
    }

    #[must_use]
    pub fn canonical_json(&self) -> String {
        let zones = self
            .zones
            .iter()
            .map(|(id, entry)| {
                let parent = entry
                    .parent
                    .as_ref()
                    .map_or_else(|| "null".to_owned(), |parent| quote_json(parent.as_str()));
                format!(
                    "{{\"id\":{},\"parent\":{},\"thread\":{}}}",
                    quote_json(id.as_str()),
                    parent,
                    quote_json(entry.thread.as_str())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let objects = self
            .objects
            .iter()
            .map(|(id, entry)| {
                format!(
                    "{{\"id\":{},\"zone\":{},\"shareable\":{}}}",
                    quote_json(id.as_str()),
                    quote_json(entry.zone.as_str()),
                    quote_json(shareable_name(entry.shareable)),
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":{},\"root\":{},\"zones\":[{}],\"objects\":[{}]}}",
            quote_json(self.schema),
            quote_json(self.root.as_str()),
            zones,
            objects,
        )
    }

    /// Prepare the linear deterministic run bound to this exact model and one
    /// ordered script. The script length is bounded.
    #[must_use]
    pub fn prepare_run(&self, script: &[Op]) -> ArcZonesRun<'_> {
        ArcZonesRun {
            model_fingerprint: self.fingerprint,
            model: self,
            script: script.to_vec(),
            cursor: 0,
            phases: BTreeMap::new(),
            construction_seq: BTreeMap::new(),
            held_handles: BTreeMap::new(),
            base_released: BTreeSet::new(),
            unique: BTreeSet::new(),
            live_links: BTreeMap::new(),
            in_degree: BTreeMap::new(),
            next_seq: 0,
            open_zones: Vec::new(),
            events: Vec::new(),
            status: RunStatus::Running,
            rejected_witness: None,
            totals: ArcZoneTotals::default(),
        }
    }
}

fn validate_identity(name: &str) -> Result<(), ArcZonesError> {
    if name.is_empty() || name.as_bytes().contains(&0) {
        return Err(ArcZonesError::InvalidIdentity);
    }
    Ok(())
}

fn canonical_depths(
    parents: &BTreeMap<DeclarationId, Option<DeclarationId>>,
) -> Result<BTreeMap<DeclarationId, u32>, ArcZonesError> {
    let mut depths = BTreeMap::<DeclarationId, u32>::new();
    for id in parents.keys() {
        let mut stack = Vec::new();
        let mut cursor = id.clone();
        let base = loop {
            if let Some(known) = depths.get(&cursor) {
                break *known;
            }
            match parents[&cursor].clone() {
                None => break 0,
                Some(parent) => {
                    stack.push(cursor);
                    if stack.len() > parents.len() {
                        return Err(ArcZonesError::ZoneCycle);
                    }
                    cursor = parent;
                }
            }
        };
        let mut running = base;
        for name in stack.iter().rev() {
            running = running.checked_add(1).ok_or(ArcZonesError::ZoneCycle)?;
            depths.insert(name.clone(), running);
        }
        depths.entry(cursor).or_insert(base);
    }
    Ok(depths)
}

/// Closed observable lifecycle event. Events are evidence of what a conforming
/// implementation must do; they perform no work themselves.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ArcZoneEvent {
    Constructed {
        object: DeclarationId,
    },
    Retained {
        object: DeclarationId,
    },
    Released {
        object: DeclarationId,
    },
    Linked {
        from: DeclarationId,
        to: DeclarationId,
    },
    Unlinked {
        from: DeclarationId,
        to: DeclarationId,
    },
    EscapedToUnique {
        object: DeclarationId,
    },
    Finalized {
        object: DeclarationId,
        cause: FinalizeCause,
    },
    ZoneEntered {
        zone: DeclarationId,
    },
    ZoneExited {
        zone: DeclarationId,
    },
    ZoneRejectedCycle {
        zone: DeclarationId,
        witness: DeclarationId,
    },
}

impl ArcZoneEvent {
    #[must_use]
    pub fn canonical_json(&self) -> String {
        match self {
            Self::Constructed { object } => format!(
                "{{\"kind\":\"constructed\",\"object\":{}}}",
                quote_json(object.as_str())
            ),
            Self::Retained { object } => format!(
                "{{\"kind\":\"retained\",\"object\":{}}}",
                quote_json(object.as_str())
            ),
            Self::Released { object } => format!(
                "{{\"kind\":\"released\",\"object\":{}}}",
                quote_json(object.as_str())
            ),
            Self::Linked { from, to } => format!(
                "{{\"kind\":\"linked\",\"from\":{},\"to\":{}}}",
                quote_json(from.as_str()),
                quote_json(to.as_str())
            ),
            Self::Unlinked { from, to } => format!(
                "{{\"kind\":\"unlinked\",\"from\":{},\"to\":{}}}",
                quote_json(from.as_str()),
                quote_json(to.as_str())
            ),
            Self::EscapedToUnique { object } => format!(
                "{{\"kind\":\"escaped_to_unique\",\"object\":{}}}",
                quote_json(object.as_str())
            ),
            Self::Finalized { object, cause } => format!(
                "{{\"kind\":\"finalized\",\"object\":{},\"cause\":{}}}",
                quote_json(object.as_str()),
                quote_json(cause_name(*cause))
            ),
            Self::ZoneEntered { zone } => format!(
                "{{\"kind\":\"zone_entered\",\"zone\":{}}}",
                quote_json(zone.as_str())
            ),
            Self::ZoneExited { zone } => format!(
                "{{\"kind\":\"zone_exited\",\"zone\":{}}}",
                quote_json(zone.as_str())
            ),
            Self::ZoneRejectedCycle { zone, witness } => format!(
                "{{\"kind\":\"zone_rejected_cycle\",\"zone\":{},\"witness\":{}}}",
                quote_json(zone.as_str()),
                quote_json(witness.as_str())
            ),
        }
    }
}

/// Closed reason why an object reached zero strong references.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum FinalizeCause {
    Release,
    Cascade,
    ZoneExit,
}

const fn cause_name(cause: FinalizeCause) -> &'static str {
    match cause {
        FinalizeCause::Release => "release",
        FinalizeCause::Cascade => "cascade",
        FinalizeCause::ZoneExit => "zone_exit",
    }
}

/// Terminal status of one finished run. Rejection is sticky evidence: a zone
/// whose live objects participate in a reference cycle is rejected at zone exit
/// with a canonical smallest-member witness; it is never silently leaked and
/// never auto-collected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArcZoneStatus {
    Complete,
    Rejected,
}

/// Immutable terminal evidence of one complete run. Cloning or dropping it has
/// no effect on any modeled object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArcZoneSummary {
    status: ArcZoneStatus,
    rejected_witness: Option<DeclarationId>,
    totals: ArcZoneTotals,
}

impl ArcZoneSummary {
    #[must_use]
    pub const fn status(&self) -> &ArcZoneStatus {
        &self.status
    }

    #[must_use]
    pub const fn rejected_witness(&self) -> Option<&DeclarationId> {
        self.rejected_witness.as_ref()
    }

    #[must_use]
    pub const fn totals(&self) -> ArcZoneTotals {
        self.totals
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ArcZoneTotals {
    pub constructed: u64,
    pub retained: u64,
    pub released: u64,
    pub finalized: u64,
    pub zones_entered: u64,
    pub zones_exited: u64,
    pub zones_rejected: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    NotConstructed,
    Live,
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RunStatus {
    Running,
    Complete,
    Rejected,
}

/// Linear deterministic execution of one prepared script. The run is not
/// cloneable: it models exactly one sequential execution in one thread per
/// zone. Every operation is validated against the live state and fails closed.
#[derive(Eq, PartialEq)]
pub struct ArcZonesRun<'a> {
    model_fingerprint: [u8; 32],
    model: &'a ArcZonesModel,
    script: Vec<Op>,
    cursor: usize,
    phases: BTreeMap<DeclarationId, Phase>,
    construction_seq: BTreeMap<DeclarationId, u64>,
    held_handles: BTreeMap<DeclarationId, u32>,
    base_released: BTreeSet<DeclarationId>,
    unique: BTreeSet<DeclarationId>,
    live_links: BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    in_degree: BTreeMap<DeclarationId, u32>,
    next_seq: u64,
    open_zones: Vec<DeclarationId>,
    events: Vec<ArcZoneEvent>,
    status: RunStatus,
    rejected_witness: Option<DeclarationId>,
    totals: ArcZoneTotals,
}

impl<'a> ArcZonesRun<'a> {
    #[must_use]
    pub const fn model_fingerprint(&self) -> [u8; 32] {
        self.model_fingerprint
    }

    #[must_use]
    pub fn events(&self) -> &[ArcZoneEvent] {
        &self.events
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self.status, RunStatus::Complete)
    }

    #[must_use]
    pub const fn is_rejected(&self) -> bool {
        matches!(self.status, RunStatus::Rejected)
    }

    #[must_use]
    pub fn rejected_witness(&self) -> Option<&DeclarationId> {
        self.rejected_witness.as_ref()
    }

    #[must_use]
    pub fn open_zones(&self) -> &[DeclarationId] {
        &self.open_zones
    }

    /// Apply the next script operation. Returns the batch of events it
    /// produced, in emission order. Once the root zone exited or the run was
    /// rejected every call stays quiescent and returns an empty batch.
    pub fn step(&mut self) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        self.authenticate()?;
        if self.script.len() > MAX_SCRIPT_OPS {
            return Err(ArcZonesError::ScriptBoundExceeded);
        }
        if matches!(self.status, RunStatus::Complete | RunStatus::Rejected)
            || self.cursor >= self.script.len()
        {
            return Ok(Vec::new());
        }
        let op = self.script[self.cursor].clone();
        self.cursor += 1;
        self.apply_op(&op)
    }

    /// Drive the script to quiescence and consume the terminal summary.
    pub fn finish(&mut self) -> Result<ArcZoneSummary, ArcZonesError> {
        self.authenticate()?;
        loop {
            if matches!(self.status, RunStatus::Complete | RunStatus::Rejected) {
                break;
            }
            let produced = self.step()?;
            if produced.is_empty() {
                return Err(ArcZonesError::RunNotFinished);
            }
        }
        Ok(ArcZoneSummary {
            status: match self.status {
                RunStatus::Rejected => ArcZoneStatus::Rejected,
                _ => ArcZoneStatus::Complete,
            },
            rejected_witness: self.rejected_witness.clone(),
            totals: self.totals,
        })
    }

    /// Strong-reference total: the unreleased base reference plus one per live
    /// incoming payload link plus one per outstanding explicit handle.
    #[must_use]
    pub fn strong_count(&self, object: &str) -> Option<u32> {
        let id = DeclarationId::new(object);
        if self.phases.get(&id).copied()? != Phase::Live {
            return None;
        }
        Some(self.live_total(&id))
    }

    fn authenticate(&self) -> Result<(), ArcZonesError> {
        if self.model_fingerprint != self.model.fingerprint {
            return Err(ArcZonesError::FrameBindingMismatch);
        }
        Ok(())
    }

    fn live_total(&self, object: &DeclarationId) -> u32 {
        let base = u32::from(!self.base_released.contains(object));
        let incoming = self.in_degree.get(object).copied().unwrap_or_default();
        let held = self.held_handles.get(object).copied().unwrap_or_default();
        base + incoming + held
    }

    fn record(&mut self, mut batch: Vec<ArcZoneEvent>) -> Vec<ArcZoneEvent> {
        let start = self.events.len();
        self.events.append(&mut batch);
        self.events[start..].to_vec()
    }

    fn current_zone(&self) -> Result<&DeclarationId, ArcZonesError> {
        self.open_zones.last().ok_or(ArcZonesError::NoOpenZone)
    }

    fn live_object_in_current_zone(&self, object: &DeclarationId) -> Result<(), ArcZonesError> {
        let entry = self
            .model
            .objects
            .get(object)
            .ok_or(ArcZonesError::UnknownObject)?;
        if self
            .phases
            .get(object)
            .copied()
            .unwrap_or(Phase::NotConstructed)
            != Phase::Live
        {
            return Err(ArcZonesError::DeadOrUnconstructedObject);
        }
        if entry.zone != *self.current_zone()? {
            return Err(ArcZonesError::ForeignZoneObject);
        }
        Ok(())
    }

    fn apply_op(&mut self, op: &Op) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        match op {
            Op::Construct { object } => self.construct(object),
            Op::Retain { object } => self.retain(object),
            Op::Release { object } => self.release(object),
            Op::Link { from, to } => self.link(from, to),
            Op::Unlink { from, to } => self.unlink(from, to),
            Op::Demote { object } => self.demote(object),
            Op::EnterZone { zone } => self.enter_zone(zone),
            Op::ExitZone { zone } => self.exit_zone(zone),
        }
    }

    fn construct(&mut self, object: &DeclarationId) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        self.live_object_in_current_zone_for_construction(object)?;
        self.phases.insert(object.clone(), Phase::Live);
        self.construction_seq.insert(object.clone(), self.next_seq);
        self.next_seq += 1;
        self.held_handles.insert(object.clone(), 0);
        self.totals.constructed += 1;
        Ok(self.record(vec![ArcZoneEvent::Constructed {
            object: object.clone(),
        }]))
    }

    fn live_object_in_current_zone_for_construction(
        &self,
        object: &DeclarationId,
    ) -> Result<(), ArcZonesError> {
        let entry = self
            .model
            .objects
            .get(object)
            .ok_or(ArcZonesError::UnknownObject)?;
        if self
            .phases
            .get(object)
            .copied()
            .unwrap_or(Phase::NotConstructed)
            != Phase::NotConstructed
        {
            return Err(ArcZonesError::AlreadyConstructed);
        }
        if entry.zone != *self.current_zone()? {
            return Err(ArcZonesError::ForeignZoneObject);
        }
        Ok(())
    }

    fn retain(&mut self, object: &DeclarationId) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        self.live_object_in_current_zone(object)?;
        if self.unique.contains(object) {
            return Err(ArcZonesError::SharedUseOfUnique);
        }
        *self
            .held_handles
            .get_mut(object)
            .expect("constructed objects carry a handle slot") += 1;
        self.totals.retained += 1;
        Ok(self.record(vec![ArcZoneEvent::Retained {
            object: object.clone(),
        }]))
    }

    fn release(&mut self, object: &DeclarationId) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        self.live_object_in_current_zone(object)?;
        let held = self
            .held_handles
            .get(object)
            .copied()
            .expect("constructed objects carry a handle slot");
        let base_gone = self.base_released.contains(object);
        if held == 0 && base_gone {
            return Err(ArcZonesError::DoubleRelease);
        }
        if held > 0 {
            *self
                .held_handles
                .get_mut(object)
                .expect("constructed objects carry a handle slot") -= 1;
        } else {
            self.base_released.insert(object.clone());
        }
        self.totals.released += 1;
        let mut batch = vec![ArcZoneEvent::Released {
            object: object.clone(),
        }];
        if self.live_total(object) == 0 {
            self.finalize(object, FinalizeCause::Release, &mut batch);
        }
        Ok(self.record(batch))
    }

    fn link(
        &mut self,
        from: &DeclarationId,
        to: &DeclarationId,
    ) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        self.live_object_in_current_zone(from)?;
        let to_entry = self
            .model
            .objects
            .get(to)
            .ok_or(ArcZonesError::UnknownObject)?;
        if self
            .phases
            .get(to)
            .copied()
            .unwrap_or(Phase::NotConstructed)
            != Phase::Live
        {
            return Err(ArcZonesError::DeadOrUnconstructedObject);
        }
        let current = self.current_zone()?.clone();
        if self.unique.contains(from) || self.unique.contains(to) {
            return Err(ArcZonesError::SharedUseOfUnique);
        }
        let cross_zone = to_entry.zone != current;
        let from_thread = self.model.zones[&current].thread.clone();
        let to_thread = self.model.zones[&to_entry.zone].thread.clone();
        let cross_thread = from_thread != to_thread;
        if (cross_zone || cross_thread) && to_entry.shareable != ShareableMark::Shareable {
            return Err(ArcZonesError::SharingWithoutShareable);
        }
        if !self
            .live_links
            .entry(from.clone())
            .or_default()
            .insert(to.clone())
        {
            return Err(ArcZonesError::DuplicateLiveLink);
        }
        *self.in_degree.entry(to.clone()).or_default() += 1;
        Ok(self.record(vec![ArcZoneEvent::Linked {
            from: from.clone(),
            to: to.clone(),
        }]))
    }

    fn unlink(
        &mut self,
        from: &DeclarationId,
        to: &DeclarationId,
    ) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        self.live_object_in_current_zone(from)?;
        if self
            .phases
            .get(to)
            .copied()
            .unwrap_or(Phase::NotConstructed)
            != Phase::Live
        {
            return Err(ArcZonesError::DeadOrUnconstructedObject);
        }
        let removed = self
            .live_links
            .get_mut(from)
            .map(|targets| targets.remove(to))
            .unwrap_or(false);
        if !removed {
            return Err(ArcZonesError::UnknownLiveLink);
        }
        if self.live_links.get(from).is_some_and(BTreeSet::is_empty) {
            self.live_links.remove(from);
        }
        let degree = self.in_degree.get_mut(to).expect("edge was tracked");
        *degree -= 1;
        if *degree == 0 {
            self.in_degree.remove(to);
        }
        let mut batch = vec![ArcZoneEvent::Unlinked {
            from: from.clone(),
            to: to.clone(),
        }];
        if self.live_total(to) == 0 {
            self.finalize(to, FinalizeCause::Release, &mut batch);
        }
        Ok(self.record(batch))
    }

    /// Escape-demotion rewrite rule: a shared handle proven zone-local —
    /// exactly the unreleased base reference, no incoming payload links, no
    /// prior demotion — is rewritten to unique ownership. Any later shared use
    /// of a demoted object fails closed.
    fn demote(&mut self, object: &DeclarationId) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        self.live_object_in_current_zone(object)?;
        if self.unique.contains(object) || self.live_total(object) != 1 {
            return Err(ArcZonesError::DemotionNotApplicable);
        }
        self.unique.insert(object.clone());
        Ok(self.record(vec![ArcZoneEvent::EscapedToUnique {
            object: object.clone(),
        }]))
    }

    fn enter_zone(&mut self, zone: &DeclarationId) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        let entry = self
            .model
            .zones
            .get(zone)
            .ok_or(ArcZonesError::UnknownZone)?;
        if self.open_zones.iter().any(|open| open == zone) {
            return Err(ArcZonesError::ZoneAlreadyOpen);
        }
        if let Some(parent) = &entry.parent {
            if self.open_zones.last() != Some(parent) {
                return Err(ArcZonesError::ParentNotOpen);
            }
        } else if !self.open_zones.is_empty() {
            return Err(ArcZonesError::ParentNotOpen);
        }
        self.open_zones.push(zone.clone());
        self.totals.zones_entered += 1;
        Ok(self.record(vec![ArcZoneEvent::ZoneEntered { zone: zone.clone() }]))
    }

    fn exit_zone(&mut self, zone: &DeclarationId) -> Result<Vec<ArcZoneEvent>, ArcZonesError> {
        if !self.model.zones.contains_key(zone) {
            return Err(ArcZonesError::UnknownZone);
        }
        let innermost = self.open_zones.last() == Some(zone);
        if !innermost {
            if self.open_zones.contains(zone) {
                return Err(ArcZonesError::UnbalancedZoneExit);
            }
            return Err(ArcZonesError::ZoneNotOpen);
        }

        let live_here: Vec<DeclarationId> = self
            .phases
            .iter()
            .filter(|(id, phase)| {
                **phase == Phase::Live
                    && self.model.objects.get(*id).map(|entry| &entry.zone) == Some(zone)
            })
            .map(|(id, _)| id.clone())
            .collect();
        let participants = cycle_participants(&live_here, &self.live_links, &self.phases);
        let witnesses: Vec<&DeclarationId> = live_here
            .iter()
            .filter(|id| participants.contains(*id))
            .collect();
        if let Some(witness) = witnesses.iter().min() {
            let witness = (*witness).clone();
            self.status = RunStatus::Rejected;
            self.rejected_witness = Some(witness.clone());
            self.totals.zones_rejected += 1;
            let event = ArcZoneEvent::ZoneRejectedCycle {
                zone: zone.clone(),
                witness,
            };
            return Ok(self.record(vec![event]));
        }

        let mut batch = Vec::new();
        let mut draining: Vec<(u64, DeclarationId)> = live_here
            .iter()
            .map(|id| {
                (
                    self.construction_seq
                        .get(id)
                        .copied()
                        .expect("live objects were constructed"),
                    id.clone(),
                )
            })
            .collect();
        draining.sort();
        draining.reverse();
        for (_, id) in draining {
            if self.phases.get(&id).copied() == Some(Phase::Live) {
                self.finalize(&id, FinalizeCause::ZoneExit, &mut batch);
            }
        }
        self.open_zones.pop();
        self.totals.zones_exited += 1;
        batch.push(ArcZoneEvent::ZoneExited { zone: zone.clone() });
        if self.open_zones.is_empty() {
            self.status = RunStatus::Complete;
        }
        Ok(self.record(batch))
    }

    /// Finalize one live object and cascade through its outgoing live links in
    /// canonical target order, depth-first. This fixes the exact deterministic
    /// finalization order; it performs no physical destruction.
    fn finalize(
        &mut self,
        object: &DeclarationId,
        cause: FinalizeCause,
        batch: &mut Vec<ArcZoneEvent>,
    ) {
        debug_assert_eq!(self.phases.get(object).copied(), Some(Phase::Live));
        self.phases.insert(object.clone(), Phase::Finalized);
        let targets: Vec<DeclarationId> = self
            .live_links
            .get(object)
            .map(|targets| targets.iter().cloned().collect())
            .unwrap_or_default();
        for target in &targets {
            let degree = self.in_degree.get_mut(target).expect("edge was tracked");
            *degree -= 1;
            if *degree == 0 {
                self.in_degree.remove(target);
            }
        }
        self.live_links.remove(object);
        batch.push(ArcZoneEvent::Finalized {
            object: object.clone(),
            cause,
        });
        self.totals.finalized += 1;
        for target in targets {
            if self.phases.get(&target).copied() == Some(Phase::Live)
                && self.live_total(&target) == 0
            {
                self.finalize(&target, FinalizeCause::Cascade, batch);
            }
        }
    }

    /// Canonical JSON projection of the trace so far, bound to the model
    /// fingerprint, the run status, and the rejection witness once present.
    #[must_use]
    pub fn trace_canonical_json(&self) -> String {
        let events = self
            .events
            .iter()
            .map(ArcZoneEvent::canonical_json)
            .collect::<Vec<_>>()
            .join(",");
        let status = match self.status {
            RunStatus::Running => "running",
            RunStatus::Complete => "complete",
            RunStatus::Rejected => "rejected",
        };
        let witness = self
            .rejected_witness
            .as_ref()
            .map_or_else(|| "null".to_owned(), |witness| quote_json(witness.as_str()));
        format!(
            "{{\"schema\":{},\"model_fingerprint\":\"{}\",\"status\":{},\"rejected_witness\":{},\"events\":[{}]}}",
            quote_json(ARC_ZONES_TRACE_V1),
            hex(&self.model_fingerprint),
            quote_json(status),
            witness,
            events,
        )
    }

    /// Domain-separated SHA-256 digest over the canonical trace projection.
    #[must_use]
    pub fn trace_digest(&self) -> [u8; 32] {
        fingerprint(
            TRACE_FINGERPRINT_DOMAIN,
            self.trace_canonical_json().as_bytes(),
        )
    }
}

/// Nodes lying on a directed cycle of the live subgraph: members of a strongly
/// connected component of size greater than one plus self-loops. Iterative
/// Tarjan over canonically ordered adjacency; bounded by the object cap.
fn cycle_participants(
    live: &[DeclarationId],
    live_links: &BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    phases: &BTreeMap<DeclarationId, Phase>,
) -> BTreeSet<DeclarationId> {
    let live_set: BTreeSet<&DeclarationId> = live.iter().collect();
    let mut adjacency: BTreeMap<&DeclarationId, Vec<&DeclarationId>> = BTreeMap::new();
    for id in &live_set {
        let successors = live_links
            .get(*id)
            .map(|targets| {
                targets
                    .iter()
                    .filter(|target| live_set.contains(target))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        adjacency.insert(id, successors);
    }
    let mut index_of: BTreeMap<&DeclarationId, usize> = BTreeMap::new();
    let mut lowlink: BTreeMap<&DeclarationId, usize> = BTreeMap::new();
    let mut on_stack: BTreeSet<&DeclarationId> = BTreeSet::new();
    let mut component: Vec<&DeclarationId> = Vec::new();
    let mut participants: BTreeSet<DeclarationId> = BTreeSet::new();
    let mut next_index = 0_usize;

    for start in &live_set {
        if index_of.contains_key(*start) {
            continue;
        }
        let mut work: Vec<(&DeclarationId, usize)> = vec![(start, 0)];
        index_of.insert(start, next_index);
        lowlink.insert(start, next_index);
        next_index += 1;
        component.push(start);
        on_stack.insert(start);
        while let Some((node, child_slot)) = work.pop() {
            let successors = &adjacency[node];
            if child_slot < successors.len() {
                work.push((node, child_slot + 1));
                let successor = successors[child_slot];
                if !index_of.contains_key(successor) {
                    index_of.insert(successor, next_index);
                    lowlink.insert(successor, next_index);
                    next_index += 1;
                    component.push(successor);
                    on_stack.insert(successor);
                    work.push((successor, 0));
                } else if on_stack.contains(successor) {
                    let successor_index = index_of[successor];
                    let node_low = lowlink.get_mut(node).expect("visited node");
                    *node_low = (*node_low).min(successor_index);
                }
            } else {
                let node_low = *lowlink.get(node).expect("visited node");
                if let Some((parent, _)) = work.last().copied() {
                    let parent_low = lowlink.get_mut(parent).expect("visited parent");
                    *parent_low = (*parent_low).min(node_low);
                }
                if node_low == index_of[node] {
                    let mut members: Vec<&DeclarationId> = Vec::new();
                    while let Some(member) = component.pop() {
                        on_stack.remove(member);
                        members.push(member);
                        if member == node {
                            break;
                        }
                    }
                    if members.len() > 1 {
                        for member in members {
                            participants.insert(member.clone());
                        }
                    }
                }
            }
        }
    }

    for id in &live_set {
        if phases.get(*id).copied() == Some(Phase::Live)
            && live_links
                .get(*id)
                .is_some_and(|targets| targets.contains(*id))
        {
            participants.insert((*id).clone());
        }
    }
    participants
}

impl fmt::Debug for ArcZonesRun<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ArcZonesRun")
            .field("model_fingerprint", &hex(&self.model_fingerprint))
            .field("events", &self.events.len())
            .field("cursor", &self.cursor)
            .field("script", &self.script.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArcZonesError {
    MissingRoot,
    MultipleRoots,
    InvalidIdentity,
    DuplicateZone,
    DuplicateObject,
    UnknownZone,
    UnknownObject,
    ZoneCycle,
    ZoneBoundExceeded,
    ObjectBoundExceeded,
    ScriptBoundExceeded,
    WorkBudgetExceeded,
    NoOpenZone,
    ZoneAlreadyOpen,
    ParentNotOpen,
    ZoneNotOpen,
    UnbalancedZoneExit,
    AlreadyConstructed,
    DeadOrUnconstructedObject,
    ForeignZoneObject,
    DoubleRelease,
    SharingWithoutShareable,
    SharedUseOfUnique,
    UnknownLiveLink,
    DuplicateLiveLink,
    DemotionNotApplicable,
    RunNotFinished,
    FrameBindingMismatch,
}

impl fmt::Display for ArcZonesError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingRoot => "arc-zone model has no root zone",
            Self::MultipleRoots => "arc-zone model has more than one root zone",
            Self::InvalidIdentity => "arc-zone identity is empty or contains NUL",
            Self::DuplicateZone => "arc-zone model declares a duplicate zone identity",
            Self::DuplicateObject => "arc-zone model declares a duplicate object identity",
            Self::UnknownZone => "arc-zone model references an unknown zone",
            Self::UnknownObject => "arc-zone script references an unknown object",
            Self::ZoneCycle => "arc-zone zone tree contains a cycle",
            Self::ZoneBoundExceeded => "arc-zone zone count is outside bounds",
            Self::ObjectBoundExceeded => "arc-zone object count is outside bounds",
            Self::ScriptBoundExceeded => "arc-zone script length is outside bounds",
            Self::WorkBudgetExceeded => "arc-zone model work budget is exceeded",
            Self::NoOpenZone => "arc-zone operation requires an open zone",
            Self::ZoneAlreadyOpen => "arc-zone is already open on the zone stack",
            Self::ParentNotOpen => "arc-zone parent zone is not the innermost open zone",
            Self::ZoneNotOpen => "arc-zone exit names a zone that is not open",
            Self::UnbalancedZoneExit => "arc-zone exit skips an innermost open child zone",
            Self::AlreadyConstructed => "arc-zone object is constructed twice",
            Self::DeadOrUnconstructedObject => "arc-zone object is unconstructed or finalized",
            Self::ForeignZoneObject => "arc-zone operation targets another zone's object",
            Self::DoubleRelease => "arc-zone release exceeds outstanding references",
            Self::SharingWithoutShareable => {
                "cross-zone or cross-thread sharing requires an explicit Shareable annotation"
            }
            Self::SharedUseOfUnique => "demoted unique object cannot regain shared aliases",
            Self::UnknownLiveLink => "arc-zone unlink names no live payload link",
            Self::DuplicateLiveLink => "arc-zone declares a duplicate live payload link",
            Self::DemotionNotApplicable => {
                "escape demotion requires a sole-held zone-local shared object"
            }
            Self::RunNotFinished => "arc-zone script ended before the root zone exited",
            Self::FrameBindingMismatch => "arc-zone run binding does not match its model",
        })
    }
}

impl Error for ArcZonesError {}

fn fingerprint(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use fmt::Write as _;
        write!(output, "{byte:02x}").expect("writing to String cannot fail");
    }
    output
}

const fn shareable_name(mark: ShareableMark) -> &'static str {
    match mark {
        ShareableMark::Shareable => "shareable",
        ShareableMark::NotShareable => "not_shareable",
    }
}

#[cfg(test)]
#[path = "arc_zones/tests.rs"]
mod tests;
