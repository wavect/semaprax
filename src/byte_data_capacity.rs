//! Target-neutral capacity authority for Portable Indexed Byte Data v1.
//!
//! This module deliberately consumes a small, identity-bearing projection
//! instead of backend storage. Source verification and hostile-HIR validation
//! can build the projection independently and must obtain the same summaries.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub(crate) const MAX_ARRAY_BYTES: u64 = 65_536;
pub(crate) const MAX_INLINE_ARRAY_FRAME_BYTES: u64 = 65_536;
pub(crate) const MAX_ACTIVE_ARRAY_CALL_PATH_BYTES: u64 = 65_536;
pub(crate) const MAX_BYTES_COPY_SITES: u32 = 16;
pub(crate) const MAX_OWNED_BYTE_PAYLOAD_BYTES: u64 = 1_048_576;

const MAX_FUNCTIONS: usize = 4_096;
const MAX_FLOW_NODES: usize = 65_536;
const MAX_ARRAY_SLOTS_PER_FUNCTION: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ArrayStorageKind {
    Parameter,
    Binding,
    Temporary,
    CallStaging,
    ProvisionalResult,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ArrayStorageSlot {
    pub identity: String,
    pub kind: ArrayStorageKind,
    pub length: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CapacityFlow {
    Empty,
    Sequence(Vec<CapacityFlow>),
    Alternative(Vec<CapacityFlow>),
    Call {
        site: String,
        callee: String,
    },
    BytesCopy {
        site: String,
        conservative_payload_bytes: u64,
    },
    Loop {
        condition: Box<CapacityFlow>,
        body: Box<CapacityFlow>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionCapacityInput {
    pub function: String,
    pub array_slots: Vec<ArrayStorageSlot>,
    pub execution: CapacityFlow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct FunctionCapacitySummary {
    pub inline_array_frame_bytes: u64,
    pub active_array_call_path_bytes: u64,
    pub bytes_copy_sites: u32,
    pub owned_byte_payload_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProgramCapacitySummary {
    functions: BTreeMap<String, FunctionCapacitySummary>,
}

impl ProgramCapacitySummary {
    pub(crate) fn function(&self, identity: &str) -> Option<&FunctionCapacitySummary> {
        self.functions.get(identity)
    }

    pub(crate) fn functions(
        &self,
    ) -> impl ExactSizeIterator<Item = (&str, &FunctionCapacitySummary)> {
        self.functions
            .iter()
            .map(|(identity, summary)| (identity.as_str(), summary))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapacityDiagnostic {
    Array,
    Allocation,
    Invariant,
}

impl CapacityDiagnostic {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::Array => "SPX-T261",
            Self::Allocation => "SPX-T267",
            Self::Invariant => "SPX-H006",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CapacityError {
    pub diagnostic: CapacityDiagnostic,
    pub function: Option<String>,
    pub detail: String,
}

impl fmt::Display for CapacityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(function) = &self.function {
            write!(
                formatter,
                "{} in `{function}`: {}",
                self.diagnostic.code(),
                self.detail
            )
        } else {
            write!(formatter, "{}: {}", self.diagnostic.code(), self.detail)
        }
    }
}

impl std::error::Error for CapacityError {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AllocationSummary {
    sites: u32,
    payload_bytes: u64,
}

#[derive(Clone, Copy)]
enum FoldMode {
    Array,
    Allocation,
}

struct ValidatedFunction<'a> {
    input: &'a FunctionCapacityInput,
    frame_bytes: u64,
    callees: BTreeSet<&'a str>,
    has_local_copy: bool,
}

pub(crate) fn analyze(
    functions: &[FunctionCapacityInput],
) -> Result<ProgramCapacitySummary, CapacityError> {
    if functions.len() > MAX_FUNCTIONS {
        return Err(invariant(
            None,
            "byte-data capacity function count exceeds the compiler bound",
        ));
    }
    let mut by_id = BTreeMap::<&str, &FunctionCapacityInput>::new();
    for function in functions {
        require_identity(&function.function, None, "function")?;
        if by_id.insert(&function.function, function).is_some() {
            return Err(invariant(
                Some(&function.function),
                "duplicate function identity in byte-data capacity input",
            ));
        }
    }

    let mut validated = BTreeMap::<&str, ValidatedFunction<'_>>::new();
    for (identity, function) in &by_id {
        let frame_bytes = validate_slots(function)?;
        let (callees, has_local_copy) = validate_flow(function, &by_id)?;
        validated.insert(
            identity,
            ValidatedFunction {
                input: function,
                frame_bytes,
                callees,
                has_local_copy,
            },
        );
    }

    let array_relevant = reverse_reachable(&validated, |item| item.frame_bytes != 0);
    let allocation_relevant = reverse_reachable(&validated, |item| item.has_local_copy);
    let array_order =
        relevant_topological_order(&validated, &array_relevant, CapacityDiagnostic::Array)?;
    let allocation_order = relevant_topological_order(
        &validated,
        &allocation_relevant,
        CapacityDiagnostic::Allocation,
    )?;

    let mut summaries = validated
        .iter()
        .map(|(identity, item)| {
            (
                (*identity).to_owned(),
                FunctionCapacitySummary {
                    inline_array_frame_bytes: item.frame_bytes,
                    ..FunctionCapacitySummary::default()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for identity in array_order.into_iter().rev() {
        let item = &validated[identity];
        let child_bytes =
            fold_flow(&item.input.execution, FoldMode::Array, identity, &summaries)?.0;
        let active = item.frame_bytes.checked_add(child_bytes).ok_or_else(|| {
            array_error(
                identity,
                "active array call-path byte calculation overflowed",
            )
        })?;
        if active > MAX_ACTIVE_ARRAY_CALL_PATH_BYTES {
            return Err(array_error(
                identity,
                format!(
                    "active array call-path uses {active} bytes; limit is {MAX_ACTIVE_ARRAY_CALL_PATH_BYTES}"
                ),
            ));
        }
        summaries
            .get_mut(identity)
            .expect("validated function has a summary")
            .active_array_call_path_bytes = active;
    }

    for identity in allocation_order.into_iter().rev() {
        let item = &validated[identity];
        let (_, allocation) = fold_flow(
            &item.input.execution,
            FoldMode::Allocation,
            identity,
            &summaries,
        )?;
        if allocation.sites > MAX_BYTES_COPY_SITES {
            return Err(allocation_error(
                identity,
                format!(
                    "bytes_copy path reaches {} sites; limit is {MAX_BYTES_COPY_SITES}",
                    allocation.sites
                ),
            ));
        }
        if allocation.payload_bytes > MAX_OWNED_BYTE_PAYLOAD_BYTES {
            return Err(allocation_error(
                identity,
                format!(
                    "bytes_copy path admits {} payload bytes; limit is {MAX_OWNED_BYTE_PAYLOAD_BYTES}",
                    allocation.payload_bytes
                ),
            ));
        }
        let summary = summaries
            .get_mut(identity)
            .expect("validated function has a summary");
        summary.bytes_copy_sites = allocation.sites;
        summary.owned_byte_payload_bytes = allocation.payload_bytes;
    }

    Ok(ProgramCapacitySummary {
        functions: summaries,
    })
}

fn validate_slots(function: &FunctionCapacityInput) -> Result<u64, CapacityError> {
    if function.array_slots.len() > MAX_ARRAY_SLOTS_PER_FUNCTION {
        return Err(array_error(
            &function.function,
            "inline array slot count exceeds the compiler bound",
        ));
    }
    let mut identities = BTreeSet::new();
    let mut total = 0u64;
    for slot in &function.array_slots {
        require_identity(
            &slot.identity,
            Some(&function.function),
            "array storage slot",
        )?;
        if !identities.insert(slot.identity.as_str()) {
            return Err(invariant(
                Some(&function.function),
                format!("duplicate array storage slot `{}`", slot.identity),
            ));
        }
        let length = u64::from(slot.length);
        if length > MAX_ARRAY_BYTES {
            return Err(array_error(
                &function.function,
                format!(
                    "array storage slot `{}` has length {length}; limit is {MAX_ARRAY_BYTES}",
                    slot.identity
                ),
            ));
        }
        total = total.checked_add(length).ok_or_else(|| {
            array_error(
                &function.function,
                "inline array frame byte calculation overflowed",
            )
        })?;
    }
    if total > MAX_INLINE_ARRAY_FRAME_BYTES {
        return Err(array_error(
            &function.function,
            format!(
                "inline array frame uses {total} bytes; limit is {MAX_INLINE_ARRAY_FRAME_BYTES}"
            ),
        ));
    }
    Ok(total)
}

fn validate_flow<'a>(
    function: &'a FunctionCapacityInput,
    functions: &BTreeMap<&str, &FunctionCapacityInput>,
) -> Result<(BTreeSet<&'a str>, bool), CapacityError> {
    let mut pending = vec![&function.execution];
    let mut nodes = 0usize;
    let mut callees = BTreeSet::new();
    let mut sites = BTreeSet::new();
    let mut has_local_copy = false;
    while let Some(flow) = pending.pop() {
        nodes = nodes.checked_add(1).ok_or_else(|| {
            invariant(
                Some(&function.function),
                "capacity flow node count overflowed",
            )
        })?;
        if nodes > MAX_FLOW_NODES {
            return Err(invariant(
                Some(&function.function),
                "capacity flow exceeds the compiler node bound",
            ));
        }
        match flow {
            CapacityFlow::Empty => {}
            CapacityFlow::Sequence(children) | CapacityFlow::Alternative(children) => {
                pending.extend(children.iter().rev());
            }
            CapacityFlow::Call { site, callee } => {
                require_identity(site, Some(&function.function), "call site")?;
                require_identity(callee, Some(&function.function), "callee")?;
                if !sites.insert(site.as_str()) {
                    return Err(invariant(
                        Some(&function.function),
                        format!("duplicate capacity site `{site}`"),
                    ));
                }
                if !functions.contains_key(callee.as_str()) {
                    return Err(invariant(
                        Some(&function.function),
                        format!("unknown capacity callee `{callee}`"),
                    ));
                }
                callees.insert(callee.as_str());
            }
            CapacityFlow::BytesCopy {
                site,
                conservative_payload_bytes,
            } => {
                require_identity(site, Some(&function.function), "bytes_copy site")?;
                if !sites.insert(site.as_str()) {
                    return Err(invariant(
                        Some(&function.function),
                        format!("duplicate capacity site `{site}`"),
                    ));
                }
                if *conservative_payload_bytes > MAX_ARRAY_BYTES {
                    return Err(allocation_error(
                        &function.function,
                        format!(
                            "bytes_copy site `{site}` admits {conservative_payload_bytes} bytes; per-value limit is {MAX_ARRAY_BYTES}"
                        ),
                    ));
                }
                has_local_copy = true;
            }
            CapacityFlow::Loop { condition, body } => {
                pending.push(body);
                pending.push(condition);
            }
        }
    }
    Ok((callees, has_local_copy))
}

fn reverse_reachable<'a>(
    functions: &BTreeMap<&'a str, ValidatedFunction<'a>>,
    seed: impl Fn(&ValidatedFunction<'a>) -> bool,
) -> BTreeSet<&'a str> {
    let mut relevant = functions
        .iter()
        .filter_map(|(identity, item)| seed(item).then_some(*identity))
        .collect::<BTreeSet<_>>();
    loop {
        let before = relevant.len();
        for (identity, item) in functions {
            if item.callees.iter().any(|callee| relevant.contains(callee)) {
                relevant.insert(*identity);
            }
        }
        if relevant.len() == before {
            return relevant;
        }
    }
}

fn relevant_topological_order<'a>(
    functions: &BTreeMap<&'a str, ValidatedFunction<'a>>,
    relevant: &BTreeSet<&'a str>,
    diagnostic: CapacityDiagnostic,
) -> Result<Vec<&'a str>, CapacityError> {
    let mut indegree = relevant
        .iter()
        .map(|identity| (*identity, 0usize))
        .collect::<BTreeMap<_, _>>();
    for identity in relevant {
        for callee in &functions[identity].callees {
            if relevant.contains(callee) {
                let degree = indegree
                    .get_mut(callee)
                    .expect("relevant callee has indegree");
                *degree = degree.checked_add(1).ok_or_else(|| {
                    invariant(Some(identity), "capacity graph indegree overflowed")
                })?;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(identity, degree)| (*degree == 0).then_some(*identity))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(relevant.len());
    while let Some(identity) = ready.pop_first() {
        order.push(identity);
        for callee in &functions[identity].callees {
            if !relevant.contains(callee) {
                continue;
            }
            let degree = indegree
                .get_mut(callee)
                .expect("relevant callee has indegree");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(callee);
            }
        }
    }
    if order.len() != relevant.len() {
        let identity = indegree
            .iter()
            .find_map(|(identity, degree)| (*degree != 0).then_some(*identity))
            .expect("incomplete topological order has a cycle member");
        let detail = match diagnostic {
            CapacityDiagnostic::Array => "call-graph cycle can reach nonzero inline array storage",
            CapacityDiagnostic::Allocation => "bytes_copy executable closure is cyclic",
            CapacityDiagnostic::Invariant => unreachable!(),
        };
        return Err(CapacityError {
            diagnostic,
            function: Some(identity.to_owned()),
            detail: detail.to_owned(),
        });
    }
    Ok(order)
}

fn fold_flow(
    flow: &CapacityFlow,
    mode: FoldMode,
    function: &str,
    summaries: &BTreeMap<String, FunctionCapacitySummary>,
) -> Result<(u64, AllocationSummary), CapacityError> {
    enum Frame<'a> {
        Enter(&'a CapacityFlow),
        Finish(&'a CapacityFlow, usize),
    }
    let mut frames = vec![Frame::Enter(flow)];
    let mut values = Vec::<(u64, AllocationSummary)>::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(node) => match node {
                CapacityFlow::Empty => values.push((0, AllocationSummary::default())),
                CapacityFlow::Call { callee, .. } => {
                    let summary = &summaries[callee];
                    values.push((
                        summary.active_array_call_path_bytes,
                        AllocationSummary {
                            sites: summary.bytes_copy_sites,
                            payload_bytes: summary.owned_byte_payload_bytes,
                        },
                    ));
                }
                CapacityFlow::BytesCopy {
                    conservative_payload_bytes,
                    ..
                } => values.push((
                    0,
                    AllocationSummary {
                        sites: 1,
                        payload_bytes: *conservative_payload_bytes,
                    },
                )),
                CapacityFlow::Sequence(children) | CapacityFlow::Alternative(children) => {
                    frames.push(Frame::Finish(node, children.len()));
                    frames.extend(children.iter().rev().map(Frame::Enter));
                }
                CapacityFlow::Loop { condition, body } => {
                    frames.push(Frame::Finish(node, 2));
                    frames.push(Frame::Enter(body));
                    frames.push(Frame::Enter(condition));
                }
            },
            Frame::Finish(node, child_count) => {
                let split = values.len().checked_sub(child_count).ok_or_else(|| {
                    invariant(Some(function), "capacity fold result stack underflowed")
                })?;
                let children = values.drain(split..).collect::<Vec<_>>();
                let alternative = matches!(node, CapacityFlow::Alternative(_));
                let mut array_bytes = 0u64;
                let mut allocation = AllocationSummary::default();
                for (child_array, child_allocation) in children {
                    if alternative {
                        array_bytes = array_bytes.max(child_array);
                        allocation.sites = allocation.sites.max(child_allocation.sites);
                        allocation.payload_bytes =
                            allocation.payload_bytes.max(child_allocation.payload_bytes);
                    } else {
                        array_bytes = array_bytes.checked_add(child_array).ok_or_else(|| {
                            array_error(function, "active array call-path calculation overflowed")
                        })?;
                        allocation.sites = allocation
                            .sites
                            .checked_add(child_allocation.sites)
                            .ok_or_else(|| {
                                allocation_error(function, "bytes_copy site count overflowed")
                            })?;
                        allocation.payload_bytes = allocation
                            .payload_bytes
                            .checked_add(child_allocation.payload_bytes)
                            .ok_or_else(|| {
                                allocation_error(function, "owned byte payload sum overflowed")
                            })?;
                    }
                }
                if matches!(mode, FoldMode::Allocation)
                    && matches!(node, CapacityFlow::Loop { .. })
                    && allocation.sites != 0
                {
                    return Err(allocation_error(
                        function,
                        "bytes_copy is reachable from a while condition or body",
                    ));
                }
                values.push((array_bytes, allocation));
            }
        }
    }
    if values.len() != 1 {
        return Err(invariant(
            Some(function),
            "capacity fold did not produce exactly one result",
        ));
    }
    Ok(values.pop().expect("capacity fold result count checked"))
}

fn require_identity(
    identity: &str,
    function: Option<&str>,
    kind: &str,
) -> Result<(), CapacityError> {
    if identity.is_empty() || identity.contains('\0') {
        return Err(invariant(
            function,
            format!("{kind} identity must be nonempty and NUL-free"),
        ));
    }
    Ok(())
}

fn array_error(function: &str, detail: impl Into<String>) -> CapacityError {
    CapacityError {
        diagnostic: CapacityDiagnostic::Array,
        function: Some(function.to_owned()),
        detail: detail.into(),
    }
}

fn allocation_error(function: &str, detail: impl Into<String>) -> CapacityError {
    CapacityError {
        diagnostic: CapacityDiagnostic::Allocation,
        function: Some(function.to_owned()),
        detail: detail.into(),
    }
}

fn invariant(function: Option<&str>, detail: impl Into<String>) -> CapacityError {
    CapacityError {
        diagnostic: CapacityDiagnostic::Invariant,
        function: function.map(str::to_owned),
        detail: detail.into(),
    }
}
