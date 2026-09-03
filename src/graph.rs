//! Deterministic semantic graph serialization and bounded context queries.
//!
//! Human source supplies the revision. Resolved HIR supplies every semantic
//! identity and fact in graph v10-v14; spans and display names are metadata only.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write;

use sha2::{Digest, Sha256};

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::bounded_output::BudgetedJoin as _;
use crate::call_index::PersistentCallIndex;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::format;
use crate::hir::{
    self, ByteSliceExtent, ByteSliceRootKind, DeclarationId, FunctionExecutionId,
    FunctionInstanceId, IdentityOrigin, OwnershipMode, Place, PlaceProjection, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedImportFailure, ResolvedMatchMode, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedStatement, ResolvedType, ResolvedTypeDeclarationKind,
    TypeFacts, ValueId,
};
use crate::prelude;

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

#[path = "graph/native_import.rs"]
mod native_import;
#[path = "graph/nested_owned.rs"]
mod nested_owned;

use nested_owned::{
    graph_schema_includes_loans, graph_schema_includes_modern_composite_facts,
    graph_schema_includes_projected_provenance, rejected_evidence_schema,
};

pub(crate) use native_import::{reject_native_rust_imports, reject_source_native_rust_imports};
pub(crate) use nested_owned::{graph_schema, graph_schema_from_parts_and_instances};

/// Hash the canonical human-readable source projection and implicit prelude.
///
/// This revision intentionally does not depend on HIR spans, display metadata,
/// or the graph wire format. Semantic transactions therefore remain bound to
/// the exact canonical source meaning that a human can review in Git plus the
/// compiler-owned ordinary prelude that participates in checked meaning.
pub fn revision(program: &Program) -> String {
    let source = format::canonical(program);
    revision_from_canonical_source(&source)
}

pub(crate) fn revision_from_canonical_source(source: &str) -> String {
    let prelude_contract = prelude::contract_bytes_v1();
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.graph-revision.v2\0");
    hasher.update((source.len() as u64).to_le_bytes());
    hasher.update(source.as_bytes());
    hasher.update((prelude::SCHEMA_V1.len() as u64).to_le_bytes());
    hasher.update(prelude::SCHEMA_V1.as_bytes());
    hasher.update((prelude_contract.len() as u64).to_le_bytes());
    hasher.update(&prelude_contract);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Resolve and serialize a parsed program as `semaprax.graph.v10`, as v11 when
/// the validated program contains bounded Option propagation, as v12 when it
/// declares a bounded generic record, or as v13 when it contains an explicit
/// authenticated record pattern, or as v14 when it declares a bounded generic
/// function.
///
/// Resolution is deliberately part of this public boundary. Invalid source
/// cannot be mistaken for a checked semantic graph by library callers.
pub fn to_json(program: &Program) -> Result<String, Vec<Diagnostic>> {
    let revision = revision(program);
    let resolved = hir::resolve(program)?;
    to_hir_json(&resolved, &revision).map_err(|diagnostic| vec![diagnostic])
}

/// Resolve and return a bounded call-dependency slice.
///
/// `symbol` may be either a function's display name or its persistent
/// declaration ID. `Ok(None)` means that no function matched the symbol;
/// resolution or graph validation failures are returned as diagnostics.
pub fn context_json(
    program: &Program,
    symbol: &str,
    depth: usize,
) -> Result<Option<String>, Vec<Diagnostic>> {
    reject_source_native_rust_imports(program)?;
    let revision = revision(program);
    let resolved = hir::resolve(program)?;
    reject_native_rust_imports(&resolved).map_err(|diagnostic| vec![diagnostic])?;
    context_hir_json(&resolved, &revision, symbol, depth).map_err(|diagnostic| vec![diagnostic])
}

/// Maximum byte budget accepted by the deterministic agent-context boundary.
pub const MAX_AGENT_CONTEXT_BYTES: usize = 16 * 1024 * 1024;
/// Smallest accepted requested byte budget.
///
/// A particular query can still fail closed when its canonical envelope and
/// first resumable frontier entry do not fit this budget.
pub const MIN_AGENT_CONTEXT_BYTES: usize = 1024;
/// Maximum function-fact budget accepted by one query.
pub const MAX_AGENT_CONTEXT_NODES: usize = 65_536;
/// Maximum transitive call depth accepted by one context query.
pub const MAX_AGENT_CONTEXT_DEPTH: usize = 1024;

/// One closed semantic facet understood by `semaprax.agent-context.v1`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AgentContextFilter {
    Contracts,
    Ownership,
    Effects,
    Types,
    Targets,
    Diagnostics,
    Tests,
}

impl AgentContextFilter {
    pub const ALL: [Self; 7] = [
        Self::Contracts,
        Self::Ownership,
        Self::Effects,
        Self::Types,
        Self::Targets,
        Self::Diagnostics,
        Self::Tests,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Contracts => "contracts",
            Self::Ownership => "ownership",
            Self::Effects => "effects",
            Self::Types => "types",
            Self::Targets => "targets",
            Self::Diagnostics => "diagnostics",
            Self::Tests => "tests",
        }
    }

    /// Parse one exact filter name. Unknown or case-folded names are rejected.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|filter| filter.name() == name)
    }

    const fn supported_by_graph_v10(self) -> bool {
        matches!(
            self,
            Self::Contracts | Self::Ownership | Self::Effects | Self::Types
        )
    }
}

/// Validated deterministic limits and semantic facets for one agent query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentContextOptions {
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
    filters: BTreeSet<AgentContextFilter>,
}

/// One closed call-graph traversal direction understood by
/// `semaprax.agent-context.v2`.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AgentContextDirection {
    Forward,
    Reverse,
    Both,
}

impl AgentContextDirection {
    pub const ALL: [Self; 3] = [Self::Forward, Self::Reverse, Self::Both];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Reverse => "reverse",
            Self::Both => "both",
        }
    }

    /// Parse one exact direction name. Unknown or case-folded names reject.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|direction| direction.name() == name)
    }

    const fn follows_forward(self) -> bool {
        matches!(self, Self::Forward | Self::Both)
    }

    const fn follows_reverse(self) -> bool {
        matches!(self, Self::Reverse | Self::Both)
    }
}

/// Additive v2 query options. V1 options and output remain a separate exact
/// compatibility surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentContextV2Options {
    base: AgentContextOptions,
    direction: AgentContextDirection,
}

impl AgentContextV2Options {
    pub fn new(
        depth: usize,
        max_bytes: usize,
        max_nodes: usize,
        filters: impl IntoIterator<Item = AgentContextFilter>,
        direction: AgentContextDirection,
    ) -> Result<Self, Diagnostic> {
        Ok(Self {
            base: AgentContextOptions::new(depth, max_bytes, max_nodes, filters)?,
            direction,
        })
    }

    #[must_use]
    pub const fn depth(&self) -> usize {
        self.base.depth
    }

    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.base.max_bytes
    }

    #[must_use]
    pub const fn max_nodes(&self) -> usize {
        self.base.max_nodes
    }

    #[must_use]
    pub const fn direction(&self) -> AgentContextDirection {
        self.direction
    }
}

impl Default for AgentContextOptions {
    fn default() -> Self {
        Self {
            depth: 1,
            max_bytes: 64 * 1024,
            max_nodes: 256,
            filters: BTreeSet::from([
                AgentContextFilter::Contracts,
                AgentContextFilter::Ownership,
                AgentContextFilter::Effects,
                AgentContextFilter::Types,
            ]),
        }
    }
}

impl AgentContextOptions {
    pub fn new(
        depth: usize,
        max_bytes: usize,
        max_nodes: usize,
        filters: impl IntoIterator<Item = AgentContextFilter>,
    ) -> Result<Self, Diagnostic> {
        if depth > MAX_AGENT_CONTEXT_DEPTH {
            return Err(agent_context_option_error(format!(
                "agent context depth {depth} exceeds {MAX_AGENT_CONTEXT_DEPTH}"
            )));
        }
        if !(MIN_AGENT_CONTEXT_BYTES..=MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(agent_context_option_error(format!(
                "agent context max_bytes {max_bytes} is outside {MIN_AGENT_CONTEXT_BYTES}..={MAX_AGENT_CONTEXT_BYTES}"
            )));
        }
        if max_nodes == 0 || max_nodes > MAX_AGENT_CONTEXT_NODES {
            return Err(agent_context_option_error(format!(
                "agent context max_nodes {max_nodes} is outside 1..={MAX_AGENT_CONTEXT_NODES}"
            )));
        }
        let mut normalized_filters = BTreeSet::new();
        for filter in filters {
            if !normalized_filters.insert(filter) {
                return Err(agent_context_option_error(format!(
                    "agent context filter `{}` is duplicated",
                    filter.name()
                )));
            }
        }
        let filters = normalized_filters;
        if filters.is_empty() {
            return Err(agent_context_option_error(
                "agent context requires at least one filter".to_owned(),
            ));
        }
        Ok(Self {
            depth,
            max_bytes,
            max_nodes,
            filters,
        })
    }

    #[must_use]
    pub const fn depth(&self) -> usize {
        self.depth
    }

    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    #[must_use]
    pub const fn max_nodes(&self) -> usize {
        self.max_nodes
    }
}

/// Resolve and serialize one deterministic, byte- and node-bounded agent view.
///
/// `Ok(None)` retains the legacy not-found meaning. `used_bytes` counts the
/// returned UTF-8 JSON bytes and excludes a CLI newline. The frontier is a
/// stable-ID progress cursor; exact omission counts include deferred entries
/// which become addressable after replaying the first budget frontier item.
pub fn agent_context_json(
    program: &Program,
    symbol: &str,
    options: &AgentContextOptions,
) -> Result<Option<String>, Vec<Diagnostic>> {
    reject_source_native_rust_imports(program)?;
    let source_revision = revision(program);
    let resolved = hir::resolve(program)?;
    reject_native_rust_imports(&resolved).map_err(|diagnostic| vec![diagnostic])?;
    agent_context_hir_json(&resolved, &source_revision, symbol, options)
        .map_err(|diagnostic| vec![diagnostic])
}

/// Resolve and serialize one deterministic v2 agent view with explicit
/// forward, reverse, or bidirectional call-graph traversal.
pub fn agent_context_v2_json(
    program: &Program,
    symbol: &str,
    options: &AgentContextV2Options,
) -> Result<Option<String>, Vec<Diagnostic>> {
    reject_source_native_rust_imports(program)?;
    let source_revision = revision(program);
    let resolved = hir::resolve(program)?;
    reject_native_rust_imports(&resolved).map_err(|diagnostic| vec![diagnostic])?;
    agent_context_v2_hir_json(&resolved, &source_revision, symbol, options)
        .map_err(|diagnostic| vec![diagnostic])
}

#[derive(Clone)]
struct AgentFunctionFact {
    id: DeclarationId,
    depth: usize,
    calls: BTreeSet<DeclarationId>,
    json: String,
}

#[derive(Clone)]
struct AgentFunctionFactV2 {
    id: DeclarationId,
    depth: usize,
    reached_by: BTreeSet<AgentContextDirection>,
    calls: BTreeSet<DeclarationId>,
    called_by: BTreeSet<DeclarationId>,
    json: String,
}

struct AgentRenderSelectionV2<'a> {
    selected: usize,
    node_limited: usize,
    required_bytes: &'a BTreeMap<DeclarationId, usize>,
}

#[derive(Default)]
struct AgentTraversalFrontierV2 {
    reasons: BTreeSet<&'static str>,
    directions: BTreeSet<AgentContextDirection>,
}

struct AgentContextV2Index<'a> {
    program: &'a ResolvedProgram,
    calls_by_id: &'a BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    callers_by_id: &'a BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    functions: &'a BTreeMap<DeclarationId, &'a ResolvedFunction>,
    templates: &'a BTreeMap<DeclarationId, &'a crate::hir::ResolvedFunctionTemplate>,
}

struct AgentRenderSelection<'a> {
    selected: usize,
    node_limited: usize,
    required_bytes: &'a BTreeMap<DeclarationId, usize>,
}

#[derive(Clone, Copy)]
struct SourceGraphIdentity<'a> {
    schema: &'a str,
    revision: &'a str,
}

fn agent_context_hir_json(
    program: &ResolvedProgram,
    source_revision: &str,
    symbol: &str,
    options: &AgentContextOptions,
) -> Result<Option<String>, Diagnostic> {
    hir::validate(program)?;
    let source_graph_schema = graph_schema(program)?;
    let source_identity = SourceGraphIdentity {
        schema: source_graph_schema,
        revision: source_revision,
    };
    let Some(root) = find_context_root(program, symbol) else {
        return Ok(None);
    };
    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let templates = program
        .function_templates
        .iter()
        .map(|template| (template.id.clone(), template))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::from([root.clone()]);
    let mut queue = VecDeque::from([(root.clone(), 0_usize)]);
    let mut ordered = Vec::new();
    let mut depth_frontier = BTreeSet::new();
    while let Some((function_id, current_depth)) = queue.pop_front() {
        ordered.push((function_id.clone(), current_depth));
        let calls = if let Some(function) = functions.get(&function_id) {
            function_calls(function)
        } else if let Some(template) = templates.get(&function_id) {
            template_calls(template)
        } else {
            return Err(graph_reference_error("function", &function_id));
        };
        for callee in calls {
            if !functions.contains_key(&callee) && !templates.contains_key(&callee) {
                continue;
            }
            if current_depth >= options.depth {
                if !seen.contains(&callee) {
                    depth_frontier.insert(callee);
                }
            } else if seen.insert(callee.clone()) {
                queue.push_back((callee, current_depth + 1));
            }
        }
    }

    let mut facts = Vec::with_capacity(ordered.len());
    for (id, depth) in ordered {
        let (calls, json) = if let Some(function) = functions.get(&id) {
            (
                agent_function_calls(program, function),
                agent_function_json(program, function, &options.filters)?,
            )
        } else if let Some(template) = templates.get(&id) {
            (
                template_calls(template),
                agent_template_json(program, template, &options.filters)?,
            )
        } else {
            return Err(graph_reference_error("function", &id));
        };
        facts.push(AgentFunctionFact {
            id,
            depth,
            calls,
            json,
        });
    }

    let node_limited = facts.len().min(options.max_nodes);
    let mut selected = node_limited;
    let mut required_bytes = BTreeMap::new();
    loop {
        let output = render_agent_context(
            program,
            source_identity,
            &root,
            options,
            &facts,
            AgentRenderSelection {
                selected,
                node_limited,
                required_bytes: &required_bytes,
            },
            &depth_frontier,
        );
        if output.len() <= options.max_bytes {
            return Ok(Some(output));
        }
        if selected == 0 {
            return Err(agent_context_option_error(format!(
                "agent context max_bytes {} cannot contain the canonical envelope",
                options.max_bytes
            )));
        }
        let omitted = &facts[selected - 1].id;
        let estimated_required = output.len().saturating_add(64);
        let required = if estimated_required <= MAX_AGENT_CONTEXT_BYTES {
            estimated_required
        } else if individual_agent_fact_fits(
            program,
            source_identity,
            options,
            &facts[selected - 1],
        ) {
            MAX_AGENT_CONTEXT_BYTES
        } else {
            return Err(agent_context_option_error(format!(
                "agent context fact `{omitted}` is permanently unavailable within the {MAX_AGENT_CONTEXT_BYTES}-byte contract maximum"
            )));
        };
        required_bytes.insert(omitted.clone(), required);
        selected -= 1;
    }
}

fn agent_context_v2_hir_json(
    program: &ResolvedProgram,
    source_revision: &str,
    symbol: &str,
    options: &AgentContextV2Options,
) -> Result<Option<String>, Diagnostic> {
    hir::validate(program)?;
    let source_graph_schema = graph_schema(program)?;
    let source_identity = SourceGraphIdentity {
        schema: source_graph_schema,
        revision: source_revision,
    };
    let Some(root) = find_context_root(program, symbol) else {
        return Ok(None);
    };
    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let templates = program
        .function_templates
        .iter()
        .map(|template| (template.id.clone(), template))
        .collect::<BTreeMap<_, _>>();
    let call_index = PersistentCallIndex::build(program)?;
    let calls_by_id = call_index.calls_by_owner();
    let callers_by_id = call_index.callers_by_callee();
    let index = AgentContextV2Index {
        program,
        calls_by_id,
        callers_by_id,
        functions: &functions,
        templates: &templates,
    };

    let mut minimum_depth = BTreeMap::from([(root.clone(), 0_usize)]);
    let mut reached_by = BTreeMap::<DeclarationId, BTreeSet<AgentContextDirection>>::new();
    let mut level = BTreeSet::from([root.clone()]);
    let mut current_depth = 0_usize;
    let mut ordered = Vec::new();
    let mut depth_frontier = BTreeMap::<DeclarationId, BTreeSet<AgentContextDirection>>::new();
    while !level.is_empty() {
        let mut candidates = BTreeMap::<DeclarationId, BTreeSet<AgentContextDirection>>::new();
        for function_id in &level {
            ordered.push((function_id.clone(), current_depth));
            let calls = calls_by_id
                .get(function_id)
                .ok_or_else(|| graph_reference_error("function", function_id))?;
            let called_by = callers_by_id
                .get(function_id)
                .ok_or_else(|| graph_reference_error("function", function_id))?;
            if options.direction().follows_forward() {
                for neighbor in calls {
                    candidates
                        .entry(neighbor.clone())
                        .or_default()
                        .insert(AgentContextDirection::Forward);
                }
            }
            if options.direction().follows_reverse() {
                for neighbor in called_by {
                    candidates
                        .entry(neighbor.clone())
                        .or_default()
                        .insert(AgentContextDirection::Reverse);
                }
            }
        }
        let candidate_depth = current_depth + 1;
        let mut next_level = BTreeSet::new();
        for (neighbor, directions) in candidates {
            if current_depth >= options.depth() {
                if !minimum_depth.contains_key(&neighbor) {
                    depth_frontier
                        .entry(neighbor)
                        .or_default()
                        .extend(directions);
                }
                continue;
            }
            match minimum_depth.get(&neighbor).copied() {
                None => {
                    minimum_depth.insert(neighbor.clone(), candidate_depth);
                    reached_by.insert(neighbor.clone(), directions);
                    next_level.insert(neighbor);
                }
                Some(known_depth) if known_depth == candidate_depth => {
                    reached_by.entry(neighbor).or_default().extend(directions);
                }
                Some(known_depth) if candidate_depth < known_depth => {
                    minimum_depth.insert(neighbor.clone(), candidate_depth);
                    reached_by.insert(neighbor.clone(), directions);
                    next_level.insert(neighbor);
                }
                Some(_) => {}
            }
        }
        level = next_level;
        current_depth = candidate_depth;
    }

    let mut facts = Vec::with_capacity(ordered.len());
    for (id, depth) in ordered {
        facts.push(index.build_fact(
            &id,
            depth,
            reached_by.remove(&id).unwrap_or_default(),
            &options.base.filters,
        )?);
    }

    let node_limited = facts.len().min(options.max_nodes());
    let mut selected = node_limited;
    let mut required_bytes = BTreeMap::new();
    loop {
        let output = render_agent_context_v2(
            program,
            source_identity,
            &root,
            options,
            &facts,
            AgentRenderSelectionV2 {
                selected,
                node_limited,
                required_bytes: &required_bytes,
            },
            &depth_frontier,
        );
        if output.len() <= options.max_bytes() {
            let selected_ids = facts[..selected]
                .iter()
                .map(|fact| fact.id.clone())
                .collect::<BTreeSet<_>>();
            let mut resume_targets = BTreeSet::new();
            for fact in &facts[..selected] {
                resume_targets.extend(fact.calls.iter().cloned());
                resume_targets.extend(fact.called_by.iter().cloned());
            }
            resume_targets.extend(depth_frontier.keys().cloned());
            resume_targets.extend(facts[selected..].iter().map(|fact| fact.id.clone()));
            resume_targets.retain(|id| !selected_ids.contains(id));
            for target in resume_targets {
                let target_fact =
                    index.build_fact(&target, 0, BTreeSet::new(), &options.base.filters)?;
                if !individual_agent_v2_fact_fits(program, source_identity, options, &target_fact) {
                    return Err(agent_context_option_error(format!(
                        "agent context reference fact `{target}` is permanently unavailable within the {MAX_AGENT_CONTEXT_BYTES}-byte v2 contract maximum"
                    )));
                }
            }
            return Ok(Some(output));
        }
        if selected == 0 {
            return Err(agent_context_option_error(format!(
                "agent context max_bytes {} cannot contain the canonical v2 envelope",
                options.max_bytes()
            )));
        }
        let omitted = &facts[selected - 1].id;
        let estimated_required = output.len().saturating_add(96);
        let required = if estimated_required <= MAX_AGENT_CONTEXT_BYTES {
            estimated_required
        } else if individual_agent_v2_fact_fits(
            program,
            source_identity,
            options,
            &facts[selected - 1],
        ) {
            MAX_AGENT_CONTEXT_BYTES
        } else {
            return Err(agent_context_option_error(format!(
                "agent context fact `{omitted}` is permanently unavailable within the {MAX_AGENT_CONTEXT_BYTES}-byte v2 contract maximum"
            )));
        };
        required_bytes.insert(omitted.clone(), required);
        selected -= 1;
    }
}

fn agent_v2_fact_json(mut base_json: String, called_by: &BTreeSet<DeclarationId>) -> String {
    assert_eq!(base_json.pop(), Some('}'));
    write!(
        base_json,
        ",\"called_by\":{}}}",
        string_array(
            &called_by
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>()
        )
    )
    .expect("writing to a string cannot fail");
    base_json
}

impl AgentContextV2Index<'_> {
    fn build_fact(
        &self,
        id: &DeclarationId,
        depth: usize,
        reached_by: BTreeSet<AgentContextDirection>,
        filters: &BTreeSet<AgentContextFilter>,
    ) -> Result<AgentFunctionFactV2, Diagnostic> {
        let calls = self
            .calls_by_id
            .get(id)
            .ok_or_else(|| graph_reference_error("function", id))?
            .clone();
        let called_by = self
            .callers_by_id
            .get(id)
            .ok_or_else(|| graph_reference_error("function", id))?
            .clone();
        let base_json = if let Some(function) = self.functions.get(id) {
            agent_function_json(self.program, function, filters)?
        } else if let Some(template) = self.templates.get(id) {
            agent_template_json(self.program, template, filters)?
        } else {
            return Err(graph_reference_error("function", id));
        };
        Ok(AgentFunctionFactV2 {
            id: id.clone(),
            depth,
            reached_by,
            calls,
            called_by: called_by.clone(),
            json: agent_v2_fact_json(base_json, &called_by),
        })
    }
}

fn individual_agent_v2_fact_fits(
    program: &ResolvedProgram,
    source_identity: SourceGraphIdentity<'_>,
    options: &AgentContextV2Options,
    fact: &AgentFunctionFactV2,
) -> bool {
    let mut maximum_options = options.clone();
    maximum_options.base.max_bytes = MAX_AGENT_CONTEXT_BYTES;
    maximum_options.base.max_nodes = 1;
    let mut individual = fact.clone();
    individual.depth = 0;
    individual.reached_by.clear();
    let mut depth_frontier = BTreeMap::<DeclarationId, BTreeSet<AgentContextDirection>>::new();
    for (direction, neighbors) in selected_agent_relations(&individual, options.direction()) {
        for neighbor in neighbors {
            if neighbor != &individual.id {
                depth_frontier
                    .entry(neighbor.clone())
                    .or_default()
                    .insert(direction);
            }
        }
    }
    let required_bytes = BTreeMap::new();
    let root = individual.id.clone();
    render_agent_context_v2(
        program,
        source_identity,
        &root,
        &maximum_options,
        &[individual],
        AgentRenderSelectionV2 {
            selected: 1,
            node_limited: 1,
            required_bytes: &required_bytes,
        },
        &depth_frontier,
    )
    .len()
        <= MAX_AGENT_CONTEXT_BYTES
}

fn individual_agent_fact_fits(
    program: &ResolvedProgram,
    source_identity: SourceGraphIdentity<'_>,
    options: &AgentContextOptions,
    fact: &AgentFunctionFact,
) -> bool {
    let mut maximum_options = options.clone();
    maximum_options.max_bytes = MAX_AGENT_CONTEXT_BYTES;
    let facts = [fact.clone()];
    let direct_frontier = fact
        .calls
        .iter()
        .filter(|callee| *callee != &fact.id)
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_bytes = BTreeMap::new();
    render_agent_context(
        program,
        source_identity,
        &fact.id,
        &maximum_options,
        &facts,
        AgentRenderSelection {
            selected: 1,
            node_limited: 1,
            required_bytes: &required_bytes,
        },
        &direct_frontier,
    )
    .len()
        <= MAX_AGENT_CONTEXT_BYTES
}

fn find_context_root(program: &ResolvedProgram, symbol: &str) -> Option<DeclarationId> {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == symbol)
        .map(|function| function.id.clone())
        .or_else(|| {
            program
                .function_templates
                .iter()
                .find(|template| template.id.as_str() == symbol)
                .map(|template| template.id.clone())
        })
        .or_else(|| program.declarations.function_id(symbol).cloned())
}

fn function_calls(function: &ResolvedFunction) -> BTreeSet<DeclarationId> {
    let mut calls = BTreeSet::new();
    visit_function_calls(function, &mut |callee| {
        calls.insert(callee.clone());
    });
    calls
}

fn template_calls(template: &crate::hir::ResolvedFunctionTemplate) -> BTreeSet<DeclarationId> {
    let mut calls = BTreeSet::new();
    for expression in template
        .requires
        .iter()
        .chain(std::iter::once(&template.body))
        .chain(&template.ensures)
    {
        visit_expr_calls(expression, &mut |callee| {
            calls.insert(callee.clone());
        });
    }
    calls
}

fn agent_function_calls(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> BTreeSet<DeclarationId> {
    let function_ids = program
        .functions
        .iter()
        .map(|candidate| candidate.id.clone())
        .chain(
            program
                .function_templates
                .iter()
                .map(|template| template.id.clone()),
        )
        .collect::<BTreeSet<_>>();
    function_calls(function)
        .into_iter()
        .filter(|callee| function_ids.contains(callee))
        .collect()
}

fn agent_function_json(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    filters: &BTreeSet<AgentContextFilter>,
) -> Result<String, Diagnostic> {
    let calls = agent_function_calls(program, function);
    let mut propagations = Vec::new();
    collect_result_propagations(&function.body, &mut propagations);
    let mut output = format!(
        "{{\"id\":{},\"kind\":\"function\",\"name\":{},\"calls\":{},\"reference_index\":{}",
        quote_json(function.id.as_str()),
        quote_json(&function.name),
        string_array(
            &calls
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>(),
        ),
        agent_reference_index_json(program, function)?
    );
    let schema = graph_schema(program)?;
    if schema == "semaprax.graph.v14" || graph_schema_includes_modern_composite_facts(schema) {
        write!(
            output,
            ",\"call_instances\":[{}],\"body\":{}",
            agent_call_instances_json(function),
            agent_contract_expr_json(&function.body)?,
        )
        .expect("writing to a string cannot fail");
    }
    if !propagations.is_empty() {
        write!(
            output,
            ",\"result_propagations\":[{}]",
            propagations
                .into_iter()
                .map(result_propagation_json)
                .collect::<Vec<_>>()
                .budgeted_join(",")
        )
        .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Contracts) {
        write!(
            output,
            ",\"contracts\":{{\"requires\":[{}],\"ensures\":[{}]}}",
            function
                .requires
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(","),
            function
                .ensures
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",")
        )
        .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Ownership) {
        let result = result_ownership(program, &function.return_type)?;
        write!(
            output,
            ",\"ownership\":{{\"parameters\":[{}],\"result\":{}}}",
            function
                .params
                .iter()
                .map(|parameter| format!(
                    "{{\"id\":{},\"mode\":{}}}",
                    quote_json(parameter.id.as_str()),
                    quote_json(ownership_text(parameter.ownership))
                ))
                .collect::<Vec<_>>()
                .budgeted_join(","),
            quote_json(ownership_text(result))
        )
        .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Effects) {
        write!(output, ",\"effects\":{}", string_array(&function.effects))
            .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Types) {
        let mut selected_types = BTreeSet::new();
        collect_function_type_declarations(function, &mut selected_types);
        close_type_declarations(program, &mut selected_types)?;
        let selected_functions = BTreeSet::from([function.id.clone()]);
        write!(
            output,
            ",\"types\":{{\"parameters\":[{}],\"result\":{},\"facts\":[{}],\"declarations\":[{}]}}",
            function
                .params
                .iter()
                .map(|parameter| format!(
                    "{{\"id\":{},\"type_id\":{}}}",
                    quote_json(parameter.id.as_str()),
                    quote_json(&parameter.ty.identity_key())
                ))
                .collect::<Vec<_>>()
                .budgeted_join(","),
            quote_json(&function.return_type.identity_key()),
            type_facts_array(program, &selected_functions, &selected_types)?,
            agent_type_declarations_json(program, &selected_types)?
        )
        .expect("writing to a string cannot fail");
    }
    if graph_schema_includes_loans(schema) {
        write!(
            output,
            ",\"loans\":{}",
            crate::graph_loan::loan_plan_json(&function.loan_plan)
        )
        .expect("writing to a string cannot fail");
    }
    output.push('}');
    Ok(output)
}

fn agent_call_instances_json(function: &ResolvedFunction) -> String {
    let mut calls = Vec::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        visit_expr_call_instances(
            expression,
            &mut |expression, callee, type_arguments, instance| {
                calls.push(format!(
                    "{{\"expression\":{},\"template\":{},\"instance\":{},\"type_arguments\":[{}]}}",
                    quote_json(expression.id.as_str()),
                    quote_json(callee.as_str()),
                    quote_json(instance.as_str()),
                    type_arguments
                        .iter()
                        .map(type_json)
                        .collect::<Vec<_>>()
                        .budgeted_join(",")
                ));
            },
        );
    }
    calls.budgeted_join(",")
}

fn agent_template_json(
    program: &ResolvedProgram,
    template: &crate::hir::ResolvedFunctionTemplate,
    filters: &BTreeSet<AgentContextFilter>,
) -> Result<String, Diagnostic> {
    let calls = template_calls(template);
    let instances = program
        .function_instances
        .iter()
        .filter(|instance| instance.template == template.id)
        .map(|instance| {
            format!(
                "{{\"id\":{},\"execution_id\":{},\"type_arguments\":[{}]}}",
                quote_json(instance.id.as_str()),
                quote_json(&FunctionExecutionId::Generic(instance.id.clone()).identity_key()),
                instance
                    .type_arguments
                    .iter()
                    .map(type_json)
                    .collect::<Vec<_>>()
                    .budgeted_join(",")
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let mut output = format!(
        "{{\"id\":{},\"kind\":\"function_template\",\"name\":{},\"calls\":{},\"type_parameters\":[{}],\"instances\":[{}],\"body\":{}",
        quote_json(template.id.as_str()),
        quote_json(&template.name),
        string_array(
            &calls
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>()
        ),
        type_parameters_json(&template.id, &template.type_parameters),
        instances,
        agent_contract_expr_json(&template.body)?
    );
    if filters.contains(&AgentContextFilter::Contracts) {
        write!(
            output,
            ",\"contracts\":{{\"requires\":[{}],\"ensures\":[{}]}}",
            template
                .requires
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(","),
            template
                .ensures
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",")
        )
        .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Types) {
        write!(
            output,
            ",\"types\":{{\"parameters\":[{}],\"result\":{}}}",
            template
                .params
                .iter()
                .map(|parameter| format!(
                    "{{\"id\":{},\"type\":{}}}",
                    quote_json(parameter.id.as_str()),
                    type_json(&parameter.ty)
                ))
                .collect::<Vec<_>>()
                .budgeted_join(","),
            type_json(&template.return_type)
        )
        .expect("writing to a string cannot fail");
    }
    output.push('}');
    Ok(output)
}

fn collect_result_propagations<'a>(
    expression: &'a ResolvedExpr,
    propagations: &mut Vec<&'a ResolvedExpr>,
) {
    match &expression.kind {
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            propagations.push(expression);
            collect_result_propagations(operand, propagations);
        }
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_result_propagations(argument, propagations);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_result_propagations(argument, propagations);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                collect_result_propagations(argument, propagations);
            }
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_result_propagations(source, propagations);
            collect_result_propagations(start, propagations);
            collect_result_propagations(end, propagations);
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => {
            collect_result_propagations(value, propagations);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_result_propagations(left, propagations);
            collect_result_propagations(right, propagations);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        collect_result_propagations(child, propagations);
                    }
                }
            }
            collect_result_propagations(tail, propagations);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_result_propagations(condition, propagations);
            collect_result_propagations(then_branch, propagations);
            collect_result_propagations(else_branch, propagations);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for initializer in fields {
                collect_result_propagations(&initializer.value, propagations);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_result_propagations(scrutinee, propagations);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_result_propagations(guard, propagations);
                }
                collect_result_propagations(&arm.value, propagations);
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_result_propagations(base, propagations);
            for initializer in fields {
                collect_result_propagations(&initializer.value, propagations);
            }
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
}

fn result_propagation_json(expression: &ResolvedExpr) -> String {
    match &expression.kind {
        ResolvedExprKind::Try {
            operand,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
        } => format!(
            "{{\"id\":{},\"kind\":\"try_result\",\"evaluation\":\"once\",\"operand\":{},\"source_result_type_id\":{},\"source_result_type\":{},\"result_type_id\":{},\"residual_result_type_id\":{},\"residual_result_type\":{},\"result\":{},\"ok_case\":{},\"ok_field\":{},\"err_case\":{},\"err_field\":{},\"err_exit\":\"normal_result\",\"epilogue\":\"shared_postconditions\"}}",
            quote_json(expression.id.as_str()),
            quote_json(operand.id.as_str()),
            quote_json(&operand.ty.identity_key()),
            type_json(&operand.ty),
            quote_json(&expression.ty.identity_key()),
            quote_json(&residual_type.identity_key()),
            type_json(residual_type),
            quote_json(result.as_str()),
            quote_json(ok_case.as_str()),
            quote_json(ok_field.as_str()),
            quote_json(err_case.as_str()),
            quote_json(err_field.as_str())
        ),
        ResolvedExprKind::TryOption {
            operand,
            option,
            some_case,
            some_field,
            none_case,
            residual_type,
        } => format!(
            "{{\"id\":{},\"kind\":\"try_option\",\"evaluation\":\"once\",\"operand\":{},\"source_option_type_id\":{},\"source_option_type\":{},\"result_type_id\":{},\"residual_option_type_id\":{},\"residual_option_type\":{},\"option\":{},\"some_case\":{},\"some_field\":{},\"none_case\":{},\"none_exit\":\"normal_result\",\"epilogue\":\"shared_postconditions\"}}",
            quote_json(expression.id.as_str()),
            quote_json(operand.id.as_str()),
            quote_json(&operand.ty.identity_key()),
            type_json(&operand.ty),
            quote_json(&expression.ty.identity_key()),
            quote_json(&residual_type.identity_key()),
            type_json(residual_type),
            quote_json(option.as_str()),
            quote_json(some_case.as_str()),
            quote_json(some_field.as_str()),
            quote_json(none_case.as_str())
        ),
        _ => unreachable!("propagation collection returns only Try nodes"),
    }
}

/// Bounded While-Loops v1 nonclaim gate: programs selecting Graph v15 stay
/// outside every evidence/patch flow until that combination is separately
/// evidenced. Refutable Match v1 selects Graph v16 above the same lattice,
/// so the gate rejects both additive schemas; generation fails closed so no
/// capsule can ever carry a schema the independent verifiers reject as
/// unsupported.
/// Public additive view of the evidence-flow schema gate.
pub fn reject_evidence_schema(schema: &str) -> Result<(), Diagnostic> {
    reject_while_loop_evidence_schema(schema)
}

pub(crate) fn reject_while_loop_evidence_schema(schema: &str) -> Result<(), Diagnostic> {
    if let Some(error) = rejected_evidence_schema(schema) {
        Err(error)
    } else if schema == "semaprax.graph.v24" {
        Err(Diagnostic::io(
            "SPX-G410",
            "projected shared-loan programs select `semaprax.graph.v24`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v23" {
        Err(Diagnostic::io(
            "SPX-G410",
            "shared-loan programs select `semaprax.graph.v23`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v22" {
        Err(Diagnostic::io(
            "SPX-G410",
            "owned variant programs select `semaprax.graph.v22`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v21" {
        Err(Diagnostic::io(
            "SPX-G410",
            "ownership-aware match programs select `semaprax.graph.v21`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v20" {
        Err(Diagnostic::io(
            "SPX-G410",
            "dynamic byte-range programs select `semaprax.graph.v20`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v19" {
        Err(Diagnostic::io(
            "SPX-G410",
            "bounded language-command I/O programs select `semaprax.graph.v19`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v18" {
        Err(Diagnostic::io(
            "SPX-G410",
            "bounded-stdout-transcript programs select `semaprax.graph.v18`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v17" {
        Err(Diagnostic::io(
            "SPX-G410",
            "portable-indexed-byte-data programs select `semaprax.graph.v17`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v25" {
        Err(Diagnostic::io(
            "SPX-G410",
            "native Rust import programs select `semaprax.graph.v25`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v15" {
        Err(Diagnostic::io(
            "SPX-G410",
            "while-loop programs select `semaprax.graph.v15`, which is outside this evidence flow's admission",
        ))
    } else if schema == "semaprax.graph.v16" {
        Err(Diagnostic::io(
            "SPX-G410",
            "refutable-match programs select `semaprax.graph.v16`, which is outside this evidence flow's admission",
        ))
    } else {
        Ok(())
    }
}

fn expression_has_byte_range(expression: &ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::ByteRange { .. } => return true,
            ResolvedExprKind::Call { args, .. } => pending.extend(args),
            ResolvedExprKind::NativeRustImportCall(call) => pending.extend(&call.args),
            ResolvedExprKind::HostCommandCall(call) => pending.extend(&call.args),
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(condition);
                pending.push(then_branch);
                pending.push(else_branch);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        pending.push(guard);
                    }
                    pending.push(&arm.value);
                }
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
    }
    false
}

/// True only when source-authenticated HIR carries ownership-changing match
/// meaning. Plain `match` remains the implicit Value mode in every legacy
/// graph schema so its canonical bytes do not change.
fn expression_has_explicit_match_mode(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Match {
            mode,
            scrutinee,
            arms,
        } => {
            *mode != ResolvedMatchMode::Value
                || expression_has_explicit_match_mode(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_deref()
                        .is_some_and(expression_has_explicit_match_mode)
                        || expression_has_explicit_match_mode(&arm.value)
                })
        }
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_explicit_match_mode),
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.args.iter().any(expression_has_explicit_match_mode)
        }
        ResolvedExprKind::HostCommandCall(call) => {
            call.args.iter().any(expression_has_explicit_match_mode)
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            expression_has_explicit_match_mode(source)
                || expression_has_explicit_match_mode(start)
                || expression_has_explicit_match_mode(end)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_explicit_match_mode(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_explicit_match_mode(left) || expression_has_explicit_match_mode(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count()).any(|index| {
                    statement
                        .child(index)
                        .is_some_and(expression_has_explicit_match_mode)
                })
            }) || expression_has_explicit_match_mode(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_explicit_match_mode(condition)
                || expression_has_explicit_match_mode(then_branch)
                || expression_has_explicit_match_mode(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| expression_has_explicit_match_mode(&field.value)),
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_explicit_match_mode(base)
                || fields
                    .iter()
                    .any(|field| expression_has_explicit_match_mode(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => false,
    }
}

pub(crate) fn graph_schema_from_parts_without_loans(
    interfaces: &[hir::ResolvedInterface],
    types: &[hir::ResolvedTypeDeclaration],
    functions: &[ResolvedFunction],
    function_templates: &[hir::ResolvedFunctionTemplate],
) -> Result<&'static str, Diagnostic> {
    if functions.iter().any(|function| {
        matches!(
            function.cleanup_plan.schema,
            crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V7
                | crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V8
        )
    }) && native_import::declares_native_rust_import(interfaces)
    {
        return Err(Diagnostic::io(
            "SPX-G410",
            "native Rust import Graph v25 cannot mask nested owned-record Graph v26-v29 semantics",
        ));
    }
    if native_import::declares_native_rust_import(interfaces) {
        return Ok(native_import::NATIVE_RUST_IMPORT_SCHEMA);
    }
    if functions
        .iter()
        .any(|function| function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V8)
    {
        return Ok("semaprax.graph.v28");
    }
    if functions
        .iter()
        .any(|function| function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V7)
    {
        return Ok("semaprax.graph.v26");
    }
    if functions.iter().any(|function| {
        function.cleanup.schema == crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2
            || function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V6
    }) {
        return Ok("semaprax.graph.v22");
    }
    if functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_explicit_match_mode)
    }) || function_templates.iter().any(|template| {
        template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
            .any(expression_has_explicit_match_mode)
    }) {
        return Ok("semaprax.graph.v21");
    }
    if functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_byte_range)
    }) {
        return Ok("semaprax.graph.v20");
    }
    if functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_command_io)
    }) || function_templates.iter().any(|template| {
        template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
            .any(expression_has_command_io)
    }) {
        return Ok("semaprax.graph.v19");
    }
    if functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_stdout_write)
    }) || function_templates.iter().any(|template| {
        template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
            .any(expression_has_stdout_write)
    }) {
        return Ok("semaprax.graph.v18");
    }
    // Portable Indexed Byte Data v1 selects v17 above the existing v16/v15
    // schemas whenever the authenticated program carries target-independent
    // usize meaning or a borrowed byte view.
    // Programs without the new scalar retain byte-identical v10-v16 output.
    if types.iter().any(type_declaration_has_usize)
        || functions.iter().any(function_has_usize)
        || function_templates.iter().any(|template| {
            template
                .params
                .iter()
                .any(|param| type_has_usize(&param.ty))
                || type_has_usize(&template.return_type)
                || template
                    .requires
                    .iter()
                    .chain(std::iter::once(&template.body))
                    .chain(&template.ensures)
                    .any(expression_has_usize)
        })
    {
        return Ok("semaprax.graph.v17");
    }
    // Refutable Match v1 selects v16 above the whole lower lattice (including
    // the v15 while extension) only when an authenticated refutable node
    // exists; programs without refutable-match syntax keep their exact
    // pre-existing schema and bytes.
    if functions
        .iter()
        .any(|function| expression_has_refutable_match(&function.body))
    {
        return Ok("semaprax.graph.v16");
    }
    // Bounded While-Loops v1 selects v15 above the whole lower lattice only
    // when an authenticated while node exists; programs without while syntax
    // keep their exact pre-existing schema and bytes.
    if functions
        .iter()
        .any(|function| expression_has_while(&function.body))
    {
        return Ok("semaprax.graph.v15");
    }
    if !function_templates.is_empty() {
        return Ok("semaprax.graph.v14");
    }
    if functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_record_pattern)
    }) {
        return Ok("semaprax.graph.v13");
    }
    if types.iter().any(|declaration| {
        matches!(
            declaration.kind,
            ResolvedTypeDeclarationKind::Record { .. } | ResolvedTypeDeclarationKind::Class { .. }
        ) && !declaration.type_parameters.is_empty()
    }) {
        return Ok("semaprax.graph.v12");
    }
    let has_option_try = functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(|expression| {
                let mut propagations = Vec::new();
                collect_result_propagations(expression, &mut propagations);
                propagations
                    .iter()
                    .any(|candidate| matches!(candidate.kind, ResolvedExprKind::TryOption { .. }))
            })
    });
    if has_option_try {
        Ok("semaprax.graph.v11")
    } else {
        Ok("semaprax.graph.v10")
    }
}

fn type_has_usize(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::Bytes
        | ResolvedType::SliceU8 => true,
        ResolvedType::Nominal { arguments, .. } => arguments.iter().any(type_has_usize),
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::String
        | ResolvedType::Str
        | ResolvedType::TypeParameter { .. } => false,
    }
}

fn type_declaration_has_usize(declaration: &hir::ResolvedTypeDeclaration) -> bool {
    match &declaration.kind {
        ResolvedTypeDeclarationKind::Record { fields }
        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
            fields.iter().any(|field| type_has_usize(&field.ty))
        }
        ResolvedTypeDeclarationKind::Variant { cases } => cases
            .iter()
            .flat_map(|case| &case.fields)
            .any(|field| type_has_usize(&field.ty)),
        ResolvedTypeDeclarationKind::Resource { .. } => false,
    }
}

fn function_has_usize(function: &ResolvedFunction) -> bool {
    function
        .params
        .iter()
        .any(|param| type_has_usize(&param.ty))
        || type_has_usize(&function.return_type)
        || function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_usize)
}

fn expression_has_usize(expression: &ResolvedExpr) -> bool {
    if type_has_usize(&expression.ty) {
        return true;
    }
    match &expression.kind {
        ResolvedExprKind::Usize(_) => true,
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count())
                    .any(|index| statement.child(index).is_some_and(expression_has_usize))
            }) || expression_has_usize(tail)
        }
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_usize),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().any(expression_has_usize),
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().any(expression_has_usize),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            expression_has_usize(source) || expression_has_usize(start) || expression_has_usize(end)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_usize(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_usize(left) || expression_has_usize(right)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_usize(condition)
                || expression_has_usize(then_branch)
                || expression_has_usize(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| expression_has_usize(&field.value)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            expression_has_usize(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_deref().is_some_and(expression_has_usize)
                        || expression_has_usize(&arm.value)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_usize(base)
                || fields
                    .iter()
                    .any(|field| expression_has_usize(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => false,
    }
}

/// `true` when the resolved expression tree contains an authenticated while
/// statement anywhere inside its blocks, branches, arms, or nested bodies.
fn expression_has_while(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            expression_has_while(source) || expression_has_while(start) || expression_has_while(end)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| match statement {
                ResolvedStatement::While { .. } => true,
                _ => (0..statement.child_count())
                    .any(|index| statement.child(index).is_some_and(expression_has_while)),
            }) || expression_has_while(tail)
        }
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_while),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().any(expression_has_while),
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().any(expression_has_while),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_while(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_while(left) || expression_has_while(right)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_while(condition)
                || expression_has_while(then_branch)
                || expression_has_while(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| expression_has_while(&field.value)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            expression_has_while(scrutinee)
                || arms.iter().any(|arm| expression_has_while(&arm.value))
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_while(base)
                || fields
                    .iter()
                    .any(|field| expression_has_while(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => false,
    }
}

fn expression_has_stdout_write(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Call { callee, args, .. } => {
            callee.as_str() == crate::host_io_ops::STDOUT_WRITE_ID
                || args.iter().any(expression_has_stdout_write)
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.args.iter().any(expression_has_stdout_write)
        }
        ResolvedExprKind::HostCommandCall(call) => {
            call.args.iter().any(expression_has_stdout_write)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_stdout_write(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_stdout_write(left) || expression_has_stdout_write(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count()).any(|index| {
                    statement
                        .child(index)
                        .is_some_and(expression_has_stdout_write)
                })
            }) || expression_has_stdout_write(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_stdout_write(condition)
                || expression_has_stdout_write(then_branch)
                || expression_has_stdout_write(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| expression_has_stdout_write(&field.value)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            expression_has_stdout_write(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard
                        .as_deref()
                        .is_some_and(expression_has_stdout_write)
                        || expression_has_stdout_write(&arm.value)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_stdout_write(base)
                || fields
                    .iter()
                    .any(|field| expression_has_stdout_write(&field.value))
        }
        _ => false,
    }
}

fn expression_has_command_io(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::HostCommandCall(_) => true,
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            expression_has_command_io(source)
                || expression_has_command_io(start)
                || expression_has_command_io(end)
        }
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_command_io),
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.args.iter().any(expression_has_command_io)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_command_io(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_command_io(left) || expression_has_command_io(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count()).any(|index| {
                    statement
                        .child(index)
                        .is_some_and(expression_has_command_io)
                })
            }) || expression_has_command_io(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_command_io(condition)
                || expression_has_command_io(then_branch)
                || expression_has_command_io(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| expression_has_command_io(&field.value)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            expression_has_command_io(scrutinee)
                || arms.iter().any(|arm| {
                    arm.guard.as_deref().is_some_and(expression_has_command_io)
                        || expression_has_command_io(&arm.value)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_command_io(base)
                || fields
                    .iter()
                    .any(|field| expression_has_command_io(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => false,
    }
}

fn expression_has_command_append(expression: &ResolvedExpr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if matches!(
            &expression.kind,
            ResolvedExprKind::HostCommandCall(call)
                if matches!(
                    call.operation,
                    hir::ResolvedHostCommandOperation::StdoutAppend
                        | hir::ResolvedHostCommandOperation::StderrAppend
                )
        ) {
            return true;
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

fn expression_has_record_pattern(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            expression_has_record_pattern(source)
                || expression_has_record_pattern(start)
                || expression_has_record_pattern(end)
        }
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_record_pattern),
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.args.iter().any(expression_has_record_pattern)
        }
        ResolvedExprKind::HostCommandCall(call) => {
            call.args.iter().any(expression_has_record_pattern)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_record_pattern(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_record_pattern(left) || expression_has_record_pattern(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count()).any(|index| {
                    statement
                        .child(index)
                        .is_some_and(expression_has_record_pattern)
                })
            }) || expression_has_record_pattern(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_record_pattern(condition)
                || expression_has_record_pattern(then_branch)
                || expression_has_record_pattern(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| expression_has_record_pattern(&field.value)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            arms.iter().any(|arm| {
                matches!(
                    &arm.pattern,
                    crate::hir::ResolvedMatchPattern::Record { .. }
                )
            }) || expression_has_record_pattern(scrutinee)
                || arms
                    .iter()
                    .any(|arm| expression_has_record_pattern(&arm.value))
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_record_pattern(base)
                || fields
                    .iter()
                    .any(|field| expression_has_record_pattern(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => false,
    }
}

/// Refutable Match v1: `true` when the resolved expression tree contains a
/// match arm with a guard or a literal/or/binding pattern anywhere inside its
/// blocks, branches, nested matches, or guards.
fn expression_has_refutable_match(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            expression_has_refutable_match(source)
                || expression_has_refutable_match(start)
                || expression_has_refutable_match(end)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count()).any(|index| {
                    statement
                        .child(index)
                        .is_some_and(expression_has_refutable_match)
                })
            }) || expression_has_refutable_match(tail)
        }
        ResolvedExprKind::Call { args, .. } => args.iter().any(expression_has_refutable_match),
        ResolvedExprKind::NativeRustImportCall(call) => {
            call.args.iter().any(expression_has_refutable_match)
        }
        ResolvedExprKind::HostCommandCall(call) => {
            call.args.iter().any(expression_has_refutable_match)
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => expression_has_refutable_match(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            expression_has_refutable_match(left) || expression_has_refutable_match(right)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            expression_has_refutable_match(condition)
                || expression_has_refutable_match(then_branch)
                || expression_has_refutable_match(else_branch)
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .any(|field| expression_has_refutable_match(&field.value)),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            expression_has_refutable_match(scrutinee)
                || arms.iter().any(|arm| {
                    is_refutable_arm(arm)
                        || arm.guard.as_ref().is_some_and(|guard| {
                            expression_has_while(guard) || expression_has_refutable_match(guard)
                        })
                        || expression_has_while(&arm.value)
                        || expression_has_refutable_match(&arm.value)
                })
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            expression_has_refutable_match(base)
                || fields
                    .iter()
                    .any(|field| expression_has_refutable_match(&field.value))
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => false,
    }
}

/// Refutable Match v1: an arm selects Graph v16 exactly when it carries a
/// guard or a literal/or/binding pattern.
fn is_refutable_arm(arm: &crate::hir::ResolvedMatchArm) -> bool {
    arm.guard.is_some()
        || matches!(
            &arm.pattern,
            crate::hir::ResolvedMatchPattern::Literal(_)
                | crate::hir::ResolvedMatchPattern::Or(_)
                | crate::hir::ResolvedMatchPattern::Binding(_)
        )
}

fn collect_record_pattern_values(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
    values: &mut BTreeSet<ValueId>,
) {
    for field in fields {
        match &field.pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                values.insert(binding.id.clone());
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
            crate::hir::ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
                collect_record_pattern_values(fields, values);
            }
        }
    }
}

fn agent_reference_index_json(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<String, Diagnostic> {
    let mut values = function
        .params
        .iter()
        .map(|parameter| parameter.id.clone())
        .collect::<BTreeSet<_>>();
    values.insert(function.result_id.clone());
    for expression in function.requires.iter().chain(&function.ensures) {
        collect_agent_contract_values(expression, &mut values);
    }
    let mut declarations = BTreeSet::new();
    collect_function_type_declarations(function, &mut declarations);
    close_type_declarations(program, &mut declarations)?;
    Ok(format!(
        "{{\"values\":[{}],\"declarations\":[{}]}}",
        values
            .iter()
            .map(|id| quote_json(id.as_str()))
            .collect::<Vec<_>>()
            .budgeted_join(","),
        agent_type_declarations_json(program, &declarations)?
    ))
}

fn collect_agent_contract_values(expression: &ResolvedExpr, values: &mut BTreeSet<ValueId>) {
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_agent_contract_values(source, values);
            collect_agent_contract_values(start, values);
            collect_agent_contract_values(end, values);
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. } => {}
        ResolvedExprKind::Place(place) => {
            values.insert(place.root.clone());
        }
        ResolvedExprKind::BorrowPlace { place, .. } => {
            values.insert(place.root.clone());
        }
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_agent_contract_values(argument, values);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_agent_contract_values(argument, values);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                collect_agent_contract_values(argument, values);
            }
        }
        ResolvedExprKind::Unary { value, .. } => collect_agent_contract_values(value, values),
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_agent_contract_values(left, values);
            collect_agent_contract_values(right, values);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                if let ResolvedStatement::Let { binding, .. } = statement {
                    values.insert(binding.id.clone());
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        collect_agent_contract_values(child, values);
                    }
                }
            }
            collect_agent_contract_values(tail, values);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_agent_contract_values(condition, values);
            collect_agent_contract_values(then_branch, values);
            collect_agent_contract_values(else_branch, values);
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for initializer in fields {
                collect_agent_contract_values(&initializer.value, values);
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for initializer in fields {
                collect_agent_contract_values(&initializer.value, values);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_agent_contract_values(scrutinee, values);
            for arm in arms {
                match &arm.pattern {
                    crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
                        values.extend(fields.iter().map(|field| field.binding.id.clone()));
                    }
                    crate::hir::ResolvedMatchPattern::Record { fields, .. } => {
                        collect_record_pattern_values(fields, values);
                    }
                    crate::hir::ResolvedMatchPattern::Wildcard => {}
                    // Refutable Match v1: a binding arm contributes its own
                    // value; literals and or-patterns contribute nothing.
                    crate::hir::ResolvedMatchPattern::Binding(binding) => {
                        values.insert(binding.id.clone());
                    }
                    crate::hir::ResolvedMatchPattern::Literal(_)
                    | crate::hir::ResolvedMatchPattern::Or(_) => {}
                }
                if let Some(guard) = &arm.guard {
                    collect_agent_contract_values(guard, values);
                }
                collect_agent_contract_values(&arm.value, values);
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            collect_agent_contract_values(operand, values);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_agent_contract_values(base, values);
            for initializer in fields {
                collect_agent_contract_values(&initializer.value, values);
            }
        }
        ResolvedExprKind::Project { base, .. } => collect_agent_contract_values(base, values),
        ResolvedExprKind::Upcast { source } => collect_agent_contract_values(source, values),
    }
}

pub(crate) fn agent_contract_expr_json(expression: &ResolvedExpr) -> Result<String, Diagnostic> {
    Ok(match &expression.kind {
        ResolvedExprKind::Int(value) => format!(
            "{{\"kind\":\"int\",\"value\":{}}}",
            quote_json(&value.to_string())
        ),
        ResolvedExprKind::Int32(value) => {
            format!("{{\"kind\":\"int32\",\"value\":{value}}}")
        }
        ResolvedExprKind::Char(value) => format!(
            "{{\"kind\":\"char\",\"value\":{value},\"display\":{}}}",
            quote_json(&crate::format::canonical_char(*value))
        ),
        ResolvedExprKind::Uint8(value) => {
            format!("{{\"kind\":\"uint8\",\"value\":{value}}}")
        }
        ResolvedExprKind::Usize(value) => format!(
            "{{\"kind\":\"usize\",\"value\":{}}}",
            quote_json(&value.to_string())
        ),
        ResolvedExprKind::ArrayU8(values) => format!(
            "{{\"kind\":\"array_u8\",\"form\":\"explicit\",\"length\":{},\"values\":[{}]}}",
            values.len(),
            values.iter().map(u8::to_string).collect::<Vec<_>>().budgeted_join(",")
        ),
        ResolvedExprKind::RepeatArrayU8 { value, count } => format!(
            "{{\"kind\":\"array_u8\",\"form\":\"repeat\",\"length\":{count},\"value\":{value}}}"
        ),
        ResolvedExprKind::Float32(bits) => format!(
            "{{\"kind\":\"float32\",\"bits\":\"{bits:08x}\",\"value\":{}}}",
            quote_json(&crate::format::canonical_f32_bits(*bits))
        ),
        ResolvedExprKind::Float64(bits) => format!(
            "{{\"kind\":\"float64\",\"bits\":\"{bits:016x}\",\"value\":{}}}",
            quote_json(&crate::format::canonical_f64_bits(*bits))
        ),
        ResolvedExprKind::Bool(value) => format!("{{\"kind\":\"bool\",\"value\":{value}}}"),
        ResolvedExprKind::String(value) => format!(
            "{{\"kind\":\"string\",\"value\":{},\"display\":{}}}",
            quote_json(value),
            quote_json(&crate::format::canonical_string(value))
        ),
        ResolvedExprKind::Place(place) => {
            format!("{{\"kind\":\"place\",\"place\":{}}}", place_json(place))
        }
        ResolvedExprKind::BorrowPlace { operation, place } => format!(
            "{{\"kind\":\"byte_view\",\"operation\":{},\"place\":{}}}",
            quote_json(operation.as_str()),
            place_json(place)
        ),
        ResolvedExprKind::ByteRange { operation, source, start, end } => format!(
            "{{\"kind\":\"byte_range\",\"operation\":{},\"source\":{},\"start\":{},\"end\":{},\"status_domain\":{},\"status_codes\":{{\"start_after_end\":{},\"end_out_of_bounds\":{}}}}}",
            quote_json(operation.as_str()), agent_contract_expr_json(source)?,
            agent_contract_expr_json(start)?, agent_contract_expr_json(end)?,
            quote_json(crate::byte_ops::RANGE_STATUS_DOMAIN),
            crate::byte_ops::RANGE_START_AFTER_END_CODE,
            crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
        ),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            let args = args
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",");
            if let Some(instance) = instance {
                format!(
                    "{{\"kind\":\"call_instance\",\"template\":{},\"instance\":{},\"type_arguments\":[{}],\"args\":[{}]}}",
                    quote_json(callee.as_str()),
                    quote_json(instance.as_str()),
                    type_arguments.iter().map(type_json).collect::<Vec<_>>().budgeted_join(","),
                    args
                )
            } else {
                format!(
                    "{{\"kind\":\"call\",\"callee\":{},\"args\":[{}]}}",
                    quote_json(callee.as_str()),
                    args
                )
            }
        }
        ResolvedExprKind::NativeRustImportCall(_) => {
            return Err(Diagnostic::io(
                "SPX-G218",
                "Native Rust import declarations are outside the current semantic Graph schemas",
            ));
        }
        ResolvedExprKind::HostCommandCall(_) => {
            return Err(Diagnostic::io(
                "SPX-G218",
                "host-command operations are outside semantic Graph contract projections",
            ));
        }
        ResolvedExprKind::Unary { op, value } => format!(
            "{{\"kind\":\"unary\",\"op\":{},\"value\":{}}}",
            quote_json(unary_text(*op)),
            agent_contract_expr_json(value)?
        ),
        ResolvedExprKind::Binary { op, left, right } => format!(
            "{{\"kind\":\"binary\",\"op\":{},\"left\":{},\"right\":{}}}",
            quote_json(binary_text(*op)),
            agent_contract_expr_json(left)?,
            agent_contract_expr_json(right)?
        ),
        ResolvedExprKind::Block { statements, tail } => format!(
            "{{\"kind\":\"block\",\"statements\":[{}],\"tail\":{}}}",
            statements
                .iter()
                .map(|statement| {
                    Ok(match statement {
                        ResolvedStatement::Let { binding, value, .. } => format!(
                            "{{\"kind\":\"let\",\"binding\":{},\"value\":{}}}",
                            quote_json(binding.id.as_str()),
                            agent_contract_expr_json(value)?
                        ),
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value,
                            ..
                        } => format!(
                            "{{\"kind\":\"assign\",\"target\":{},\"value\":{}{}}}",
                            quote_json(binding.id.as_str()),
                            agent_contract_expr_json(value)?,
                            field
                                .as_ref()
                                .map(|field| format!(",\"field\":{}", quote_json(field.as_str())))
                                .unwrap_or_default()
                        ),
                        ResolvedStatement::Unsafe { audit, body, .. } => format!(
                            "{{\"kind\":\"unsafe\",\"audit\":{},\"body\":{}}}",
                            quote_json(audit),
                            agent_contract_expr_json(body)?
                        ),
                        ResolvedStatement::While { .. } => {
                            // Contract expressions reject while statements at
                            // verification time, so this projection can never
                            // observe one.
                            return Err(Diagnostic::io(
                                "SPX-G218",
                                "while statements are outside the current semantic Graph contract projections",
                            ));
                        }
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .budgeted_join(","),
            agent_contract_expr_json(tail)?
        ),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "{{\"kind\":\"if\",\"condition\":{},\"then\":{},\"else\":{}}}",
            agent_contract_expr_json(condition)?,
            agent_contract_expr_json(then_branch)?,
            agent_contract_expr_json(else_branch)?
        ),
        ResolvedExprKind::ConstructRecord { record, fields } => {
            let instance = match &expression.ty {
                ResolvedType::Nominal { arguments, .. } if !arguments.is_empty() => {
                    format!(",\"record_type\":{}", type_json(&expression.ty))
                }
                _ => String::new(),
            };
            format!(
                "{{\"kind\":\"construct_record\",\"record\":{}{instance},\"fields\":[{}]}}",
                quote_json(record.as_str()),
                fields
                    .iter()
                    .map(|initializer| {
                        Ok(format!(
                            "{{\"field\":{},\"value\":{}}}",
                            quote_json(initializer.field.as_str()),
                            agent_contract_expr_json(&initializer.value)?
                        ))
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?
                    .budgeted_join(",")
            )
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => format!(
            "{{\"kind\":\"construct_variant\",\"variant\":{},\"case\":{},\"fields\":[{}]}}",
            quote_json(variant.as_str()),
            quote_json(case.as_str()),
            fields
                .iter()
                .map(|initializer| {
                    Ok(format!(
                        "{{\"field\":{},\"value\":{}}}",
                        quote_json(initializer.field.as_str()),
                        agent_contract_expr_json(&initializer.value)?
                    ))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .budgeted_join(",")
        ),
        ResolvedExprKind::Match {
            mode,
            scrutinee,
            arms,
        } => format!(
            "{{\"kind\":\"match\"{},\"scrutinee\":{},\"arms\":[{}]}}",
            explicit_match_mode_json(*mode),
            agent_contract_expr_json(scrutinee)?,
            arms.iter()
                .map(|arm| Ok(format!(
                    "{{\"pattern\":{},\"value\":{}}}",
                    match_pattern_json(&arm.pattern),
                    agent_contract_expr_json(&arm.value)?
                )))
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .budgeted_join(",")
        ),
        ResolvedExprKind::Try {
            operand,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
        } => format!(
            "{{\"kind\":\"try_result\",\"evaluation\":\"once\",\"operand\":{},\"source_result_type_id\":{},\"source_result_type\":{},\"residual_result_type_id\":{},\"residual_result_type\":{},\"result\":{},\"ok_case\":{},\"ok_field\":{},\"err_case\":{},\"err_field\":{},\"err_exit\":\"normal_result\",\"epilogue\":\"shared_postconditions\"}}",
            agent_contract_expr_json(operand)?,
            quote_json(&operand.ty.identity_key()),
            type_json(&operand.ty),
            quote_json(&residual_type.identity_key()),
            type_json(residual_type),
            quote_json(result.as_str()),
            quote_json(ok_case.as_str()),
            quote_json(ok_field.as_str()),
            quote_json(err_case.as_str()),
            quote_json(err_field.as_str())
        ),
        ResolvedExprKind::TryOption {
            operand,
            option,
            some_case,
            some_field,
            none_case,
            residual_type,
        } => format!(
            "{{\"kind\":\"try_option\",\"evaluation\":\"once\",\"operand\":{},\"source_option_type_id\":{},\"source_option_type\":{},\"residual_option_type_id\":{},\"residual_option_type\":{},\"option\":{},\"some_case\":{},\"some_field\":{},\"none_case\":{},\"none_exit\":\"normal_result\",\"epilogue\":\"shared_postconditions\"}}",
            agent_contract_expr_json(operand)?,
            quote_json(&operand.ty.identity_key()),
            type_json(&operand.ty),
            quote_json(&residual_type.identity_key()),
            type_json(residual_type),
            quote_json(option.as_str()),
            quote_json(some_case.as_str()),
            quote_json(some_field.as_str()),
            quote_json(none_case.as_str())
        ),
        ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } => format!(
            "{{\"kind\":\"update_record\",\"base\":{},\"record\":{},\"fields\":[{}]}}",
            agent_contract_expr_json(base)?,
            quote_json(record.as_str()),
            fields
                .iter()
                .map(|initializer| {
                    Ok(format!(
                        "{{\"field\":{},\"value\":{}}}",
                        quote_json(initializer.field.as_str()),
                        agent_contract_expr_json(&initializer.value)?
                    ))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .budgeted_join(",")
        ),
        ResolvedExprKind::Project { base, field } => format!(
            "{{\"kind\":\"project\",\"base\":{},\"field\":{}}}",
            agent_contract_expr_json(base)?,
            quote_json(field.as_str())
        ),
        ResolvedExprKind::Upcast { source } => format!(
            "{{\"kind\":\"upcast\",\"source\":{}}}",
            agent_contract_expr_json(source)?
        ),
    })
}

fn graph_match_pattern_json(pattern: &crate::hir::ResolvedMatchPattern, id: &str) -> String {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard => format!(
            "{{\"id\":{},\"kind\":\"wildcard_pattern\"}}",
            quote_json(id)
        ),
        // Refutable Match v1: additive literal/or/binding pattern nodes.
        crate::hir::ResolvedMatchPattern::Literal(value) => format!(
            "{{\"id\":{},\"kind\":\"literal_pattern\",\"type\":{},\"value\":{}}}",
            quote_json(id),
            type_json(&value.ty()),
            quote_json(&pattern_value_text(*value))
        ),
        crate::hir::ResolvedMatchPattern::Or(alternatives) => format!(
            "{{\"id\":{},\"kind\":\"or_pattern\",\"alternatives\":[{}]}}",
            quote_json(id),
            alternatives
                .iter()
                .enumerate()
                .map(|(index, alternative)| {
                    let alternative_id = format!("{id}.alternative.{index}");
                    graph_match_pattern_json(alternative, &alternative_id)
                })
                .collect::<Vec<_>>()
                .join(",")
        ),
        crate::hir::ResolvedMatchPattern::Binding(binding) => format!(
            "{{\"id\":{},\"kind\":\"binding_pattern\",\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}}}",
            quote_json(id),
            quote_json(binding.id.as_str()),
            quote_json(&binding.name),
            quote_json(&binding.ty.identity_key()),
            quote_json(ownership_text(binding.ownership))
        ),
        crate::hir::ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => format!(
            "{{\"id\":{},\"kind\":\"variant_pattern\",\"variant\":{},\"case\":{},\"fields\":[{}]}}",
            quote_json(id),
            quote_json(variant.as_str()),
            quote_json(case.as_str()),
            fields
                .iter()
                .map(|field| format!(
                    "{{\"field\":{},\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}}}",
                    quote_json(field.field.as_str()),
                    quote_json(field.binding.id.as_str()),
                    quote_json(&field.binding.name),
                    quote_json(&field.binding.ty.identity_key()),
                    quote_json(ownership_text(field.binding.ownership))
                ))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
        crate::hir::ResolvedMatchPattern::Record {
            record,
            instance,
            fields,
        } => format!(
            "{{\"id\":{},\"kind\":\"record_pattern\",\"record\":{},\"record_type_id\":{},\"record_type\":{},\"fields\":[{}]}}",
            quote_json(id),
            quote_json(record.as_str()),
            quote_json(&instance.identity_key()),
            type_json(instance),
            fields
                .iter()
                .enumerate()
                .map(|(index, field)| graph_record_match_field_json(field, &format!("{id}.field.{index}")))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    }
}

fn graph_record_match_field_json(
    field: &crate::hir::ResolvedRecordMatchPatternField,
    id: &str,
) -> String {
    let pattern = match &field.pattern {
        crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => format!(
            "{{\"id\":{},\"kind\":\"binding_pattern\",\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}}}",
            quote_json(&format!("{id}.pattern")),
            quote_json(binding.id.as_str()),
            quote_json(&binding.name),
            quote_json(&binding.ty.identity_key()),
            quote_json(ownership_text(binding.ownership))
        ),
        crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => format!(
            "{{\"id\":{},\"kind\":\"wildcard_pattern\"}}",
            quote_json(&format!("{id}.pattern"))
        ),
        crate::hir::ResolvedRecordMatchFieldPattern::Record {
            record,
            instance,
            fields,
        } => format!(
            "{{\"id\":{},\"kind\":\"record_pattern\",\"record\":{},\"record_type_id\":{},\"record_type\":{},\"fields\":[{}]}}",
            quote_json(&format!("{id}.pattern")),
            quote_json(record.as_str()),
            quote_json(&instance.identity_key()),
            type_json(instance),
            fields
                .iter()
                .enumerate()
                .map(|(index, field)| graph_record_match_field_json(field, &format!("{id}.pattern.field.{index}")))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    };
    format!(
        "{{\"id\":{},\"field\":{},\"pattern\":{pattern}}}",
        quote_json(id),
        quote_json(field.field.as_str())
    )
}

/// Refutable Match v1: exact canonical text of a literal pattern value,
/// mirroring the canonical formatter so graph consumers read one spelling.
fn pattern_value_text(value: crate::hir::PatternValue) -> String {
    match value {
        crate::hir::PatternValue::Int(value) => value.to_string(),
        crate::hir::PatternValue::Int32(value) => format!("{value}i32"),
        crate::hir::PatternValue::Uint8(value) => format!("{value}u8"),
        crate::hir::PatternValue::Usize(value) => format!("{value}usize"),
        crate::hir::PatternValue::Char(value) => crate::format::canonical_char(value),
        crate::hir::PatternValue::Bool(value) => value.to_string(),
    }
}

fn match_pattern_json(pattern: &crate::hir::ResolvedMatchPattern) -> String {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard => "{\"kind\":\"wildcard\"}".to_owned(),
        // Refutable Match v1: additive literal/or/binding pattern spellings.
        crate::hir::ResolvedMatchPattern::Literal(value) => format!(
            "{{\"kind\":\"literal\",\"type\":{},\"value\":{}}}",
            type_json(&value.ty()),
            quote_json(&pattern_value_text(*value))
        ),
        crate::hir::ResolvedMatchPattern::Or(alternatives) => format!(
            "{{\"kind\":\"or\",\"alternatives\":[{}]}}",
            alternatives
                .iter()
                .map(match_pattern_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        crate::hir::ResolvedMatchPattern::Binding(binding) => format!(
            "{{\"kind\":\"binding\",\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}}}",
            quote_json(binding.id.as_str()),
            quote_json(&binding.name),
            quote_json(&binding.ty.identity_key()),
            quote_json(ownership_text(binding.ownership))
        ),
        crate::hir::ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => format!(
            "{{\"kind\":\"variant\",\"variant\":{},\"case\":{},\"fields\":[{}]}}",
            quote_json(variant.as_str()),
            quote_json(case.as_str()),
            fields
                .iter()
                .map(|field| format!(
                    "{{\"field\":{},\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}}}",
                    quote_json(field.field.as_str()),
                    quote_json(field.binding.id.as_str()),
                    quote_json(&field.binding.name),
                    quote_json(&field.binding.ty.identity_key()),
                    quote_json(ownership_text(field.binding.ownership))
                ))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
        crate::hir::ResolvedMatchPattern::Record {
            record,
            instance,
            fields,
        } => format!(
            "{{\"kind\":\"record\",\"record\":{},\"record_type_id\":{},\"record_type\":{},\"fields\":[{}]}}",
            quote_json(record.as_str()),
            quote_json(&instance.identity_key()),
            type_json(instance),
            fields
                .iter()
                .map(record_match_field_json)
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    }
}

fn record_match_field_json(field: &crate::hir::ResolvedRecordMatchPatternField) -> String {
    let pattern = match &field.pattern {
        crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => format!(
            "{{\"kind\":\"binding\",\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"type\":{},\"ownership_mode\":{}}}}}",
            quote_json(binding.id.as_str()),
            quote_json(&binding.name),
            quote_json(&binding.ty.identity_key()),
            type_json(&binding.ty),
            quote_json(ownership_text(binding.ownership))
        ),
        crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {
            "{\"kind\":\"wildcard\"}".to_owned()
        }
        crate::hir::ResolvedRecordMatchFieldPattern::Record {
            record,
            instance,
            fields,
        } => format!(
            "{{\"kind\":\"record\",\"record\":{},\"record_type_id\":{},\"record_type\":{},\"fields\":[{}]}}",
            quote_json(record.as_str()),
            quote_json(&instance.identity_key()),
            type_json(instance),
            fields
                .iter()
                .map(record_match_field_json)
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    };
    format!(
        "{{\"field\":{},\"pattern\":{pattern}}}",
        quote_json(field.field.as_str())
    )
}

fn agent_type_declarations_json(
    program: &ResolvedProgram,
    selected: &BTreeSet<DeclarationId>,
) -> Result<String, Diagnostic> {
    program
        .types
        .iter()
        .filter(|declaration| selected.contains(&declaration.id))
        .map(|declaration| match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => Ok(format!(
                "{{\"id\":{},\"kind\":\"resource\",\"drop_strategy\":{}}}",
                quote_json(declaration.id.as_str()),
                quote_json(match drop.kind {
                    ResolvedResourceDropKind::Trivial => "trivial",
                    ResolvedResourceDropKind::Imported { .. } => "imported",
                })
            )),
            ResolvedTypeDeclarationKind::Record { fields } => {
                let parameters = if declaration.type_parameters.is_empty() {
                    String::new()
                } else {
                    format!(
                        ",\"type_parameters\":[{}]",
                        type_parameters_json(&declaration.id, &declaration.type_parameters)
                    )
                };
                Ok(format!(
                    "{{\"id\":{},\"kind\":\"record\"{parameters},\"fields\":[{}]}}",
                    quote_json(declaration.id.as_str()),
                    fields
                        .iter()
                        .map(|field| format!(
                            "{{\"id\":{},\"type_id\":{}}}",
                            quote_json(field.id.as_str()),
                            quote_json(&field.ty.identity_key())
                        ))
                        .collect::<Vec<_>>()
                        .budgeted_join(",")
                ))
            }
            ResolvedTypeDeclarationKind::Class { fields, methods } => {
                let parameters = if declaration.type_parameters.is_empty() {
                    String::new()
                } else {
                    format!(
                        ",\"type_parameters\":[{}]",
                        type_parameters_json(&declaration.id, &declaration.type_parameters)
                    )
                };
                Ok(format!(
                    "{{\"id\":{},\"kind\":\"class\"{parameters},\"fields\":[{}],\"methods\":[{}]}}",
                    quote_json(declaration.id.as_str()),
                    fields
                        .iter()
                        .map(|field| format!(
                            "{{\"id\":{},\"type_id\":{}}}",
                            quote_json(field.id.as_str()),
                            quote_json(&field.ty.identity_key())
                        ))
                        .collect::<Vec<_>>()
                        .budgeted_join(","),
                    methods
                        .iter()
                        .map(|method| quote_json(method.as_str()))
                        .collect::<Vec<_>>()
                        .budgeted_join(",")
                ))
            }
            ResolvedTypeDeclarationKind::Variant { cases } => Ok(format!(
                "{{\"id\":{},\"kind\":\"variant\",\"type_parameters\":[{}],\"cases\":[{}]}}",
                quote_json(declaration.id.as_str()),
                type_parameters_json(&declaration.id, &declaration.type_parameters),
                cases
                    .iter()
                    .map(|case| format!(
                        "{{\"id\":{},\"fields\":[{}]}}",
                        quote_json(case.id.as_str()),
                        case.fields
                            .iter()
                            .map(|field| format!(
                                "{{\"id\":{},\"type_id\":{}}}",
                                quote_json(field.id.as_str()),
                                quote_json(&field.ty.identity_key())
                            ))
                            .collect::<Vec<_>>()
                            .budgeted_join(",")
                    ))
                    .collect::<Vec<_>>()
                    .budgeted_join(",")
            )),
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|items| items.budgeted_join(","))
}

fn render_agent_context(
    program: &ResolvedProgram,
    source_identity: SourceGraphIdentity<'_>,
    root: &DeclarationId,
    options: &AgentContextOptions,
    facts: &[AgentFunctionFact],
    selection: AgentRenderSelection<'_>,
    depth_frontier: &BTreeSet<DeclarationId>,
) -> String {
    let AgentRenderSelection {
        selected,
        node_limited,
        required_bytes,
    } = selection;
    let selected_ids = facts[..selected]
        .iter()
        .map(|fact| fact.id.clone())
        .collect::<BTreeSet<_>>();
    let mut omitted_known = depth_frontier.clone();
    omitted_known.extend(facts.iter().skip(selected).map(|fact| fact.id.clone()));
    let mut frontier = BTreeMap::<DeclarationId, BTreeSet<&'static str>>::new();
    for id in depth_frontier {
        if !selected_ids.contains(id) {
            frontier.entry(id.clone()).or_default().insert("depth");
        }
    }
    for fact in facts.iter().skip(node_limited) {
        frontier
            .entry(fact.id.clone())
            .or_default()
            .insert("max_nodes");
    }
    if selected < node_limited {
        let fact = &facts[selected];
        frontier
            .entry(fact.id.clone())
            .or_default()
            .insert("max_bytes");
        let byte_omitted = facts[selected..node_limited]
            .iter()
            .map(|fact| fact.id.clone())
            .collect::<BTreeSet<_>>();
        for callee in facts[..selected].iter().flat_map(|fact| &fact.calls) {
            if byte_omitted.contains(callee) {
                frontier
                    .entry(callee.clone())
                    .or_default()
                    .insert("max_bytes");
            }
        }
    }
    let mut reasons = BTreeSet::new();
    if !depth_frontier.is_empty() {
        reasons.insert("depth");
    }
    if facts.len() > node_limited {
        reasons.insert("max_nodes");
    }
    if selected < node_limited {
        reasons.insert("max_bytes");
    }
    let unavailable_count = options
        .filters
        .iter()
        .filter(|filter| !filter.supported_by_graph_v10())
        .count();
    if unavailable_count != 0 {
        reasons.insert("unavailable_filters");
    }
    let omitted_fact_bytes = facts[selected..]
        .iter()
        .map(|fact| fact.json.len())
        .sum::<usize>();
    let requested = options
        .filters
        .iter()
        .map(|filter| quote_json(filter.name()))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let included = options
        .filters
        .iter()
        .filter(|filter| filter.supported_by_graph_v10())
        .map(|filter| quote_json(filter.name()))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let unavailable = options
        .filters
        .iter()
        .filter(|filter| !filter.supported_by_graph_v10())
        .map(|filter| quote_json(filter.name()))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let frontier_json = frontier
        .iter()
        .map(|(id, why)| {
            let required = required_bytes.get(id).copied();
            let resume_symbol = id;
            let resume_bytes = required.unwrap_or(options.max_bytes);
            format!(
                "{{\"id\":{},\"kind\":\"function\",\"reasons\":[{}],\"resume\":{{\"symbol\":{},\"target\":{},\"min_bytes\":{}}}}}",
                quote_json(id.as_str()),
                why.iter()
                    .map(|reason| quote_json(reason))
                    .collect::<Vec<_>>()
                    .budgeted_join(","),
                quote_json(resume_symbol.as_str()),
                quote_json(id.as_str()),
                resume_bytes
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let reason_json = reasons
        .iter()
        .map(|reason| quote_json(reason))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let facts_json = facts[..selected]
        .iter()
        .map(|fact| fact.json.as_str())
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let max_depth_used = facts[..selected]
        .iter()
        .map(|fact| fact.depth)
        .max()
        .unwrap_or(0);
    let render = |used_bytes: usize| {
        format!(
            "{{\"schema\":\"semaprax.agent-context.v1\",\"source_graph_schema\":{},\"revision\":{},\"prelude\":{{\"schema\":{},\"digest\":{}}},\"module\":{},\"root\":{},\"query\":{{\"depth\":{},\"max_bytes\":{},\"max_nodes\":{},\"filters\":[{}]}},\"filter_support\":{{\"included\":[{}],\"unavailable\":[{}]}},\"budget\":{{\"used_bytes\":{},\"used_nodes\":{},\"max_depth_used\":{}}},\"truncation\":{{\"truncated\":{},\"reasons\":[{}],\"omitted_known_nodes\":{},\"deferred_known_nodes\":{},\"omitted_fact_bytes\":{},\"unavailable_filter_count\":{}}},\"resume_contract\":{{\"depth\":\"query.depth\",\"max_nodes\":\"query.max_nodes\",\"filters\":\"query.filters\",\"max_bytes\":\"frontier.resume.min_bytes\"}},\"frontier\":[{}],\"facts\":[{}]}}",
            quote_json(source_identity.schema),
            quote_json(source_identity.revision),
            quote_json(prelude::SCHEMA_V1),
            quote_json(&prelude::digest_text_v1()),
            quote_json(&program.module),
            quote_json(root.as_str()),
            options.depth,
            options.max_bytes,
            options.max_nodes,
            requested,
            included,
            unavailable,
            used_bytes,
            selected,
            max_depth_used,
            !reasons.is_empty() || unavailable_count != 0,
            reason_json,
            omitted_known.len(),
            omitted_known.len().saturating_sub(frontier.len()),
            omitted_fact_bytes,
            unavailable_count,
            frontier_json,
            facts_json
        )
    };
    let mut used_bytes = 0;
    loop {
        let output = render(used_bytes);
        let actual = output.len();
        if actual == used_bytes {
            return output;
        }
        used_bytes = actual;
    }
}

fn render_agent_context_v2(
    program: &ResolvedProgram,
    source_identity: SourceGraphIdentity<'_>,
    root: &DeclarationId,
    options: &AgentContextV2Options,
    facts: &[AgentFunctionFactV2],
    selection: AgentRenderSelectionV2<'_>,
    depth_frontier: &BTreeMap<DeclarationId, BTreeSet<AgentContextDirection>>,
) -> String {
    let AgentRenderSelectionV2 {
        selected,
        node_limited,
        required_bytes,
    } = selection;
    let selected_ids = facts[..selected]
        .iter()
        .map(|fact| fact.id.clone())
        .collect::<BTreeSet<_>>();
    let mut omitted_traversal = depth_frontier.keys().cloned().collect::<BTreeSet<_>>();
    omitted_traversal.extend(facts.iter().skip(selected).map(|fact| fact.id.clone()));
    let mut frontier = BTreeMap::<DeclarationId, AgentTraversalFrontierV2>::new();
    for (id, directions) in depth_frontier {
        if !selected_ids.contains(id) {
            let entry = frontier.entry(id.clone()).or_default();
            entry.reasons.insert("depth");
            entry.directions.extend(directions);
        }
    }
    for fact in facts.iter().skip(node_limited) {
        let entry = frontier.entry(fact.id.clone()).or_default();
        entry.reasons.insert("max_nodes");
        entry.directions.extend(&fact.reached_by);
    }
    if selected < node_limited {
        let entry = frontier.entry(facts[selected].id.clone()).or_default();
        entry.reasons.insert("max_bytes");
        entry.directions.extend(&facts[selected].reached_by);
        let byte_omitted = facts[selected..node_limited]
            .iter()
            .map(|fact| fact.id.clone())
            .collect::<BTreeSet<_>>();
        for fact in &facts[..selected] {
            for (direction, neighbors) in selected_agent_relations(fact, options.direction()) {
                for neighbor in neighbors {
                    if byte_omitted.contains(neighbor) {
                        let entry = frontier.entry(neighbor.clone()).or_default();
                        entry.reasons.insert("max_bytes");
                        entry.directions.insert(direction);
                    }
                }
            }
        }
    }

    let reached_by = facts
        .iter()
        .map(|fact| (fact.id.clone(), fact.reached_by.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut reference_frontier = BTreeMap::<DeclarationId, BTreeSet<&'static str>>::new();
    for fact in &facts[..selected] {
        let mut add_references = |relation: &'static str, neighbors: &BTreeSet<DeclarationId>| {
            for neighbor in neighbors {
                if selected_ids.contains(neighbor) || frontier.contains_key(neighbor) {
                    continue;
                }
                if omitted_traversal.contains(neighbor) {
                    let entry = frontier.entry(neighbor.clone()).or_default();
                    entry.reasons.insert("max_bytes");
                    if let Some(directions) = reached_by.get(neighbor) {
                        entry.directions.extend(directions);
                    }
                } else {
                    reference_frontier
                        .entry(neighbor.clone())
                        .or_default()
                        .insert(relation);
                }
            }
        };
        if !options.direction().follows_forward() {
            add_references("calls", &fact.calls);
        }
        if !options.direction().follows_reverse() {
            add_references("called_by", &fact.called_by);
        }
    }

    let mut reasons = BTreeSet::new();
    if !depth_frontier.is_empty() {
        reasons.insert("depth");
    }
    if facts.len() > node_limited {
        reasons.insert("max_nodes");
    }
    if selected < node_limited {
        reasons.insert("max_bytes");
    }
    let unavailable_count = options
        .base
        .filters
        .iter()
        .filter(|filter| !filter.supported_by_graph_v10())
        .count();
    if unavailable_count != 0 {
        reasons.insert("unavailable_filters");
    }
    let omitted_fact_bytes = facts[selected..]
        .iter()
        .map(|fact| fact.json.len())
        .sum::<usize>();
    let requested = options
        .base
        .filters
        .iter()
        .map(|filter| quote_json(filter.name()))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let included = options
        .base
        .filters
        .iter()
        .filter(|filter| filter.supported_by_graph_v10())
        .map(|filter| quote_json(filter.name()))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let unavailable = options
        .base
        .filters
        .iter()
        .filter(|filter| !filter.supported_by_graph_v10())
        .map(|filter| quote_json(filter.name()))
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let frontier_json = frontier
        .iter()
        .map(|(id, item)| {
            let resume_bytes = required_bytes
                .get(id)
                .copied()
                .unwrap_or(MAX_AGENT_CONTEXT_BYTES);
            format!(
                "{{\"id\":{},\"kind\":\"function\",\"reasons\":[{}],\"directions\":[{}],\"resume\":{{\"symbol\":{},\"target\":{},\"direction\":{},\"min_bytes\":{}}}}}",
                quote_json(id.as_str()),
                ordered_agent_reasons(&item.reasons),
                agent_directions_json(&item.directions),
                quote_json(id.as_str()),
                quote_json(id.as_str()),
                quote_json(options.direction().name()),
                resume_bytes
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let reference_frontier_json = reference_frontier
        .iter()
        .map(|(id, relations)| {
            format!(
                "{{\"id\":{},\"kind\":\"function\",\"relations\":[{}],\"resume\":{{\"symbol\":{},\"target\":{},\"direction\":{},\"min_bytes\":{}}}}}",
                quote_json(id.as_str()),
                ordered_agent_relations(relations),
                quote_json(id.as_str()),
                quote_json(id.as_str()),
                quote_json(options.direction().name()),
                MAX_AGENT_CONTEXT_BYTES
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let facts_json = facts[..selected]
        .iter()
        .map(|fact| fact.json.as_str())
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let max_depth_used = facts[..selected]
        .iter()
        .map(|fact| fact.depth)
        .max()
        .unwrap_or(0);
    let deferred_traversal = omitted_traversal.len().saturating_sub(frontier.len());
    let render = |used_bytes: usize| {
        format!(
            "{{\"schema\":\"semaprax.agent-context.v2\",\"source_graph_schema\":{},\"revision\":{},\"prelude\":{{\"schema\":{},\"digest\":{}}},\"module\":{},\"root\":{},\"query\":{{\"direction\":{},\"depth\":{},\"max_bytes\":{},\"max_nodes\":{},\"filters\":[{}]}},\"filter_support\":{{\"included\":[{}],\"unavailable\":[{}]}},\"budget\":{{\"used_bytes\":{},\"used_nodes\":{},\"max_depth_used\":{}}},\"truncation\":{{\"truncated\":{},\"reasons\":[{}],\"omitted_known_nodes\":{},\"deferred_known_nodes\":{},\"omitted_fact_bytes\":{},\"unavailable_filter_count\":{}}},\"reference_closure\":{{\"referenced_unselected_nodes\":{}}},\"resume_contract\":{{\"direction\":\"query.direction\",\"depth\":\"query.depth\",\"max_nodes\":\"query.max_nodes\",\"filters\":\"query.filters\",\"max_bytes\":{{\"traversal\":\"frontier.resume.min_bytes\",\"reference\":\"reference_frontier.resume.min_bytes\"}}}},\"frontier\":[{}],\"reference_frontier\":[{}],\"facts\":[{}]}}",
            quote_json(source_identity.schema),
            quote_json(source_identity.revision),
            quote_json(prelude::SCHEMA_V1),
            quote_json(&prelude::digest_text_v1()),
            quote_json(&program.module),
            quote_json(root.as_str()),
            quote_json(options.direction().name()),
            options.depth(),
            options.max_bytes(),
            options.max_nodes(),
            requested,
            included,
            unavailable,
            used_bytes,
            selected,
            max_depth_used,
            !reasons.is_empty() || unavailable_count != 0,
            ordered_agent_reasons(&reasons),
            omitted_traversal.len(),
            deferred_traversal,
            omitted_fact_bytes,
            unavailable_count,
            reference_frontier.len(),
            frontier_json,
            reference_frontier_json,
            facts_json
        )
    };
    let mut used_bytes = 0;
    loop {
        let output = render(used_bytes);
        let actual = output.len();
        if actual == used_bytes {
            return output;
        }
        used_bytes = actual;
    }
}

fn selected_agent_relations(
    fact: &AgentFunctionFactV2,
    direction: AgentContextDirection,
) -> Vec<(AgentContextDirection, &BTreeSet<DeclarationId>)> {
    let mut relations = Vec::new();
    if direction.follows_forward() {
        relations.push((AgentContextDirection::Forward, &fact.calls));
    }
    if direction.follows_reverse() {
        relations.push((AgentContextDirection::Reverse, &fact.called_by));
    }
    relations
}

fn agent_directions_json(directions: &BTreeSet<AgentContextDirection>) -> String {
    directions
        .iter()
        .map(|direction| quote_json(direction.name()))
        .collect::<Vec<_>>()
        .budgeted_join(",")
}

fn ordered_agent_reasons(reasons: &BTreeSet<&'static str>) -> String {
    ["depth", "max_nodes", "max_bytes", "unavailable_filters"]
        .into_iter()
        .filter(|reason| reasons.contains(reason))
        .map(quote_json)
        .collect::<Vec<_>>()
        .budgeted_join(",")
}

fn ordered_agent_relations(relations: &BTreeSet<&'static str>) -> String {
    ["calls", "called_by"]
        .into_iter()
        .filter(|relation| relations.contains(relation))
        .map(quote_json)
        .collect::<Vec<_>>()
        .budgeted_join(",")
}

fn agent_context_option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-G004", message)
}

pub(crate) fn to_hir_json(
    program: &ResolvedProgram,
    source_revision: &str,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    let selected_functions = program
        .functions
        .iter()
        .map(|function| function.id.clone())
        .chain(
            program
                .function_templates
                .iter()
                .map(|template| template.id.clone()),
        )
        .collect();
    let selected_types = program
        .types
        .iter()
        .map(|declaration| declaration.id.clone())
        .collect();
    graph_json(
        program,
        source_revision,
        &selected_functions,
        &selected_types,
        &GraphView::Module,
    )
}

fn context_hir_json(
    program: &ResolvedProgram,
    source_revision: &str,
    symbol: &str,
    depth: usize,
) -> Result<Option<String>, Diagnostic> {
    hir::validate(program)?;

    // Exact declaration identity is authoritative if another function or
    // template's display name happens to contain the same text.
    let root = find_context_root(program, symbol);
    let Some(root) = root else {
        return Ok(None);
    };

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let templates = program
        .function_templates
        .iter()
        .map(|template| (template.id.clone(), template))
        .collect::<BTreeMap<_, _>>();
    let known_functions = functions
        .keys()
        .chain(templates.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::from([root.clone()]);
    let mut queue = VecDeque::from([(root.clone(), 0_usize)]);
    while let Some((function_id, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }
        if let Some(function) = functions.get(&function_id) {
            visit_function_calls(function, &mut |callee| {
                if known_functions.contains(callee) && selected.insert(callee.clone()) {
                    queue.push_back((callee.clone(), current_depth + 1));
                }
            });
        } else if let Some(template) = templates.get(&function_id) {
            for expression in template
                .requires
                .iter()
                .chain(std::iter::once(&template.body))
                .chain(&template.ensures)
            {
                visit_expr_calls(expression, &mut |callee| {
                    if known_functions.contains(callee) && selected.insert(callee.clone()) {
                        queue.push_back((callee.clone(), current_depth + 1));
                    }
                });
            }
        }
    }

    let mut selected_types = BTreeSet::new();
    for function in &program.functions {
        if selected.contains(&function.id) {
            collect_function_type_declarations(function, &mut selected_types);
        }
    }
    close_type_declarations(program, &mut selected_types)?;

    let mut frontier = BTreeSet::new();
    for function in &program.functions {
        if !selected.contains(&function.id) {
            continue;
        }
        visit_function_calls(function, &mut |callee| {
            if known_functions.contains(callee) && !selected.contains(callee) {
                frontier.insert(callee.clone());
            }
        });
    }
    for template in &program.function_templates {
        if !selected.contains(&template.id) {
            continue;
        }
        for callee in template_calls(template) {
            if known_functions.contains(&callee) && !selected.contains(&callee) {
                frontier.insert(callee);
            }
        }
    }

    graph_json(
        program,
        source_revision,
        &selected,
        &selected_types,
        &GraphView::Context {
            root: &root,
            depth,
            frontier: &frontier,
        },
    )
    .map(Some)
}

enum GraphView<'a> {
    Module,
    Context {
        root: &'a DeclarationId,
        depth: usize,
        frontier: &'a BTreeSet<DeclarationId>,
    },
}

fn byte_slice_extent_json(extent: ByteSliceExtent) -> String {
    match extent {
        ByteSliceExtent::Constant(value) => {
            format!("{{\"kind\":\"constant\",\"value\":{value}}}")
        }
        ByteSliceExtent::ParameterLength => "{\"kind\":\"parameter_length\"}".to_owned(),
        ByteSliceExtent::ValueLength => "{\"kind\":\"value_length\"}".to_owned(),
    }
}

fn byte_slice_fact_json(
    schema: &str,
    value: &ValueId,
    provenance: &hir::ByteSliceProvenance,
) -> String {
    let root_kind = match provenance.root_kind {
        ByteSliceRootKind::FunctionParameter => "function_parameter",
        ByteSliceRootKind::OwnedBytes => "owned_bytes",
        ByteSliceRootKind::FixedArray => "fixed_array",
        ByteSliceRootKind::BorrowedStr => "borrowed_str",
        ByteSliceRootKind::CommandArguments => "command_arguments",
    };
    let mut base = format!(
        "{{\"value\":{},\"root\":{},\"root_kind\":{},\"root_length\":{},\"offset\":{},\"length\":{},\"producer\":{}}}",
        quote_json(value.as_str()),
        quote_json(provenance.root.as_str()),
        quote_json(root_kind),
        byte_slice_extent_json(provenance.root_length),
        byte_slice_extent_json(provenance.offset),
        byte_slice_extent_json(provenance.length),
        provenance.producer.as_ref().map_or_else(|| "null".to_owned(), |id| quote_json(id.as_str()))
    );
    if graph_schema_includes_projected_provenance(schema) {
        let projections = provenance
            .projections
            .iter()
            .map(|projection| match projection {
                hir::PlaceProjection::Field(field) => format!(
                    "{{\"kind\":\"field\",\"field\":{}}}",
                    quote_json(field.as_str())
                ),
                hir::PlaceProjection::VariantField { case, field } => format!(
                    "{{\"kind\":\"variant_field\",\"case\":{},\"field\":{}}}",
                    quote_json(case.as_str()),
                    quote_json(field.as_str())
                ),
            })
            .collect::<Vec<_>>()
            .budgeted_join(",");
        base = format!(
            "{},\"projections\":[{}],\"projected_type\":{}}}",
            base.strip_suffix('}').expect("object suffix"),
            projections,
            type_json(&provenance.projected_type)
        );
    }
    if schema != "semaprax.graph.v20"
        && !(graph_schema_includes_modern_composite_facts(schema) && !provenance.ranges.is_empty())
    {
        return base;
    }
    let ranges = provenance
        .ranges
        .iter()
        .map(|range| {
            format!(
                "{{\"source\":{},\"producer\":{},\"start\":{},\"end\":{}}}",
                quote_json(range.source.as_str()),
                quote_json(range.producer.as_str()),
                quote_json(range.start.as_str()),
                quote_json(range.end.as_str())
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    format!(
        "{},\"ranges\":[{}]}}",
        base.strip_suffix('}').expect("object suffix"),
        ranges
    )
}

fn portable_indexed_byte_data_json(
    schema: &str,
    program: &ResolvedProgram,
) -> Result<String, Diagnostic> {
    let v21 = graph_schema_includes_modern_composite_facts(schema);
    let has_portable_indexed_data = program.types.iter().any(type_declaration_has_usize)
        || program.functions.iter().any(function_has_usize)
        || program.function_templates.iter().any(|template| {
            template
                .params
                .iter()
                .any(|param| type_has_usize(&param.ty))
                || type_has_usize(&template.return_type)
                || template
                    .requires
                    .iter()
                    .chain(std::iter::once(&template.body))
                    .chain(&template.ensures)
                    .any(expression_has_usize)
        });
    let has_stdout = program.functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_stdout_write)
    });
    let has_command_io = program.functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_command_io)
    });
    let has_byte_range = program.functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_byte_range)
    });
    let has_line_command = program.functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(expression_has_command_append)
    });
    if matches!(
        schema,
        "semaprax.graph.v17" | "semaprax.graph.v18" | "semaprax.graph.v19" | "semaprax.graph.v20"
    ) || (v21 && has_portable_indexed_data)
    {
        let capacity = hir::analyze_byte_data_capacity(program)?;
        let portable = format!(
            ",\"portable_indexed_byte_data\":{{\"profile\":\"useful-data-v1\",\"semantic_usize_bits\":64,\"max_external_root_bytes\":{},\"max_slice_bytes\":{},\"max_array_bytes\":{},\"max_inline_array_frame_bytes\":{},\"max_active_array_call_path_bytes\":{},\"max_bytes_copy_sites\":{},\"max_owned_byte_payload_bytes\":{},\"wasm_arena_token_min_inclusive\":1,\"wasm_arena_token_max_exclusive\":2147483648,\"wasm_arena_tokens_monotonic\":true,\"wasm_arena_tokens_reused\":false,\"wasm_import_binding\":\"exact-token-length\",\"wasm_carrier\":\"root-word-high32-length-u32-low32\",\"empty_bytes_owns_token\":true,\"zero_array_view\":\"root0-length0\",\"indexed_read\":\"total-option-u8\",\"capacity_summaries\":[{}],\"byte_slice_provenance\":[{}]}}",
            crate::byte_ops::MAX_EXTERNAL_ROOT_BYTES,
            crate::byte_data_capacity::MAX_ARRAY_BYTES,
            crate::byte_data_capacity::MAX_ARRAY_BYTES,
            crate::byte_data_capacity::MAX_INLINE_ARRAY_FRAME_BYTES,
            crate::byte_data_capacity::MAX_ACTIVE_ARRAY_CALL_PATH_BYTES,
            crate::byte_data_capacity::MAX_BYTES_COPY_SITES,
            crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES,
            capacity.functions().map(|(function, _)| {
                let summary = capacity.function(function).expect("enumerated capacity summary remains addressable");
                if matches!(schema, "semaprax.graph.v19" | "semaprax.graph.v20")
                    || (v21 && has_command_io)
                {
                    format!(
                        "{{\"function\":{},\"inline_array_frame_bytes\":{},\"active_array_call_path_bytes\":{},\"bytes_copy_sites\":{},\"stdin_read_sites\":{},\"owned_byte_payload_bytes\":{},\"stdout_write_sites\":{},\"stderr_write_sites\":{},\"combined_transcript_bytes\":{}}}",
                        quote_json(function),
                        summary.inline_array_frame_bytes,
                        summary.active_array_call_path_bytes,
                        summary.bytes_copy_sites,
                        summary.stdin_read_sites,
                        summary.owned_byte_payload_bytes,
                        summary.stdout_write_sites,
                        summary.stderr_write_sites,
                        summary.transcript_bytes,
                    )
                } else if schema == "semaprax.graph.v18" || (v21 && has_stdout) {
                    format!(
                        "{{\"function\":{},\"inline_array_frame_bytes\":{},\"active_array_call_path_bytes\":{},\"bytes_copy_sites\":{},\"owned_byte_payload_bytes\":{},\"stdout_write_sites\":{}}}",
                        quote_json(function),
                        summary.inline_array_frame_bytes,
                        summary.active_array_call_path_bytes,
                        summary.bytes_copy_sites,
                        summary.owned_byte_payload_bytes,
                        summary.stdout_write_sites,
                    )
                } else {
                    format!(
                        "{{\"function\":{},\"inline_array_frame_bytes\":{},\"active_array_call_path_bytes\":{},\"bytes_copy_sites\":{},\"owned_byte_payload_bytes\":{}}}",
                        quote_json(function),
                        summary.inline_array_frame_bytes,
                        summary.active_array_call_path_bytes,
                        summary.bytes_copy_sites,
                        summary.owned_byte_payload_bytes,
                    )
                }
            }).collect::<Vec<_>>().budgeted_join(","),
            program
                .declarations
                .byte_slice_provenances()
                .map(|(value, provenance)| byte_slice_fact_json(schema, value, provenance))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        );
        let transcript = if (matches!(
            schema,
            "semaprax.graph.v18" | "semaprax.graph.v19" | "semaprax.graph.v20"
        ) || v21)
            && has_stdout
        {
            format!(
                ",\"bounded_stdout_transcript\":{{\"profile\":\"bounded-stdout-transcript-v1\",\"operation\":{},\"effect\":{},\"max_transcript_bytes\":{},\"max_writes_per_executable_path\":{},\"publication\":\"terminal-success-only\",\"failure\":\"discard-staged-transcript\"}}",
                quote_json(crate::host_io_ops::STDOUT_WRITE_ID),
                quote_json(crate::host_io_ops::STDOUT_WRITE_EFFECT),
                crate::host_io_ops::MAX_STDOUT_TRANSCRIPT_BYTES,
                crate::host_io_ops::MAX_STDOUT_WRITES_PER_PATH,
            )
        } else {
            String::new()
        };
        let command_io = if matches!(schema, "semaprax.graph.v19" | "semaprax.graph.v20")
            || (v21 && has_command_io)
        {
            format!(
                ",\"bounded_language_command_io\":{{\"profile\":\"language-command-io.v1\",\"operations\":[{{\"name\":{},\"id\":{},\"effect\":{},\"failure\":\"infallible\"}},{{\"name\":{},\"id\":{},\"effect\":{},\"failure\":\"status\"}},{{\"name\":{},\"id\":{},\"effect\":{},\"failure\":\"status\"}},{{\"name\":{},\"id\":{},\"effect\":{},\"failure\":\"infallible\"}}],\"status_domain\":{},\"status_codes\":{{\"arg_index_out_of_bounds\":1,\"arg_invalid_utf8\":2,\"stdin_read_failed\":3,\"input_capacity_exceeded\":4}},\"max_arguments\":{},\"max_input_bytes\":{},\"argument_root\":\"immutable-invocation-owned-arena\",\"max_stdin_reads_per_path\":1,\"max_writes_per_channel_per_path\":1,\"max_combined_output_bytes\":{},\"publication\":\"terminal-success-only\",\"failure\":\"discard-staged-transcripts\"}}",
                quote_json(crate::command_io_ops::ARGS_LEN_NAME), quote_json(crate::command_io_ops::ARGS_LEN_ID), quote_json(crate::command_io_ops::ARGS_READ_EFFECT),
                quote_json(crate::command_io_ops::ARG_UTF8_NAME), quote_json(crate::command_io_ops::ARG_UTF8_ID), quote_json(crate::command_io_ops::ARGS_READ_EFFECT),
                quote_json(crate::command_io_ops::STDIN_READ_NAME), quote_json(crate::command_io_ops::STDIN_READ_ID), quote_json(crate::command_io_ops::STDIN_READ_EFFECT),
                quote_json(crate::command_io_ops::STDERR_WRITE_NAME), quote_json(crate::command_io_ops::STDERR_WRITE_ID), quote_json(crate::command_io_ops::STDERR_WRITE_EFFECT),
                quote_json(crate::command_io_ops::STATUS_DOMAIN),
                crate::command_io_ops::MAX_ARGUMENTS,
                crate::command_io_ops::MAX_INPUT_BYTES,
                crate::byte_data_capacity::MAX_COMBINED_TRANSCRIPT_BYTES,
            )
        } else {
            String::new()
        };
        let byte_range = if schema == "semaprax.graph.v20" || (v21 && has_byte_range) {
            format!(
                ",\"bounded_byte_range\":{{\"profile\":\"byte-range-v1\",\"operation\":{},\"interval\":\"half-open\",\"evaluation_order\":[\"source\",\"start\",\"end\"],\"status_domain\":{},\"status_codes\":{{\"start_after_end\":{},\"end_out_of_bounds\":{}}},\"failure_order\":[\"start_after_end\",\"end_out_of_bounds\"],\"max_derivation_depth\":{},\"provenance\":\"bounded-acyclic-root-preserving-chain\"}}",
                quote_json(crate::byte_ops::RANGE_ID),
                quote_json(crate::byte_ops::RANGE_STATUS_DOMAIN),
                crate::byte_ops::RANGE_START_AFTER_END_CODE,
                crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
                crate::byte_ops::MAX_RANGE_DEPTH,
            )
        } else {
            String::new()
        };
        let line_command_io = if (schema == "semaprax.graph.v20" || v21) && has_line_command {
            format!(
                ",\"bounded_line_command_io\":{{\"profile\":\"line-command-io.v1\",\"operations\":[{{\"name\":{},\"id\":{},\"effect\":{},\"return\":\"usize\",\"failure\":\"status\"}},{{\"name\":{},\"id\":{},\"effect\":{},\"return\":\"usize\",\"failure\":\"status\"}}],\"status_domain\":{},\"status_codes\":{{\"output_capacity_exceeded\":{}}},\"status_marker\":\"__spx_command_output_status_v1\",\"write_mode\":\"cumulative-append.v1\",\"max_combined_output_bytes\":{},\"publication\":\"terminal-success-only\",\"failure\":\"discard-staged-transcripts\"}}",
                quote_json(crate::command_io_ops::STDOUT_APPEND_NAME),
                quote_json(crate::command_io_ops::STDOUT_APPEND_ID),
                quote_json(crate::command_io_ops::STDOUT_WRITE_EFFECT),
                quote_json(crate::command_io_ops::STDERR_APPEND_NAME),
                quote_json(crate::command_io_ops::STDERR_APPEND_ID),
                quote_json(crate::command_io_ops::STDERR_WRITE_EFFECT),
                quote_json(crate::command_io_ops::OUTPUT_STATUS_DOMAIN),
                crate::command_io_ops::OUTPUT_CAPACITY_EXCEEDED,
                crate::command_io_ops::MAX_OUTPUT_BYTES,
            )
        } else {
            String::new()
        };
        Ok(format!(
            "{portable}{transcript}{command_io}{byte_range}{line_command_io}"
        ))
    } else {
        Ok(String::new())
    }
}

fn byte_slice_provenance_json(
    schema: &str,
    program: &ResolvedProgram,
    parameter: &hir::ResolvedParam,
) -> Result<Option<String>, Diagnostic> {
    if parameter.ty != ResolvedType::SliceU8 {
        return Ok(None);
    }
    let provenance = program
        .declarations
        .byte_slice_provenance(&parameter.id)
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-G411",
                format!(
                    "byte-slice parameter `{}` lacks authenticated provenance",
                    parameter.id
                ),
            )
        })?;
    Ok(Some(byte_slice_fact_json(
        schema,
        &parameter.id,
        provenance,
    )))
}

fn graph_json(
    program: &ResolvedProgram,
    source_revision: &str,
    selected_functions: &BTreeSet<DeclarationId>,
    selected_types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut selected_types = selected_types.clone();
    let mut selected_interfaces = match view {
        GraphView::Module => program
            .interfaces
            .iter()
            .map(|interface| interface.id.clone())
            .collect::<BTreeSet<_>>(),
        GraphView::Context { .. } => BTreeSet::new(),
    };

    loop {
        close_type_declarations(program, &mut selected_types)?;
        let referenced_imports = program
            .types
            .iter()
            .filter(|declaration| selected_types.contains(&declaration.id))
            .filter_map(|declaration| match &declaration.kind {
                ResolvedTypeDeclarationKind::Resource { drop } => match &drop.kind {
                    ResolvedResourceDropKind::Imported { import, .. } => Some(import.clone()),
                    ResolvedResourceDropKind::Trivial => None,
                },
                ResolvedTypeDeclarationKind::Record { .. }
                | ResolvedTypeDeclarationKind::Class { .. } => None,
                ResolvedTypeDeclarationKind::Variant { .. } => None,
            })
            .collect::<BTreeSet<_>>();
        let mut changed = false;
        for interface in &program.interfaces {
            if interface
                .imports
                .iter()
                .any(|import| referenced_imports.contains(&import.id))
            {
                changed |= selected_interfaces.insert(interface.id.clone());
            }
            if !selected_interfaces.contains(&interface.id) {
                continue;
            }
            for import in &interface.imports {
                for parameter in &import.parameters {
                    let mut referenced = BTreeSet::new();
                    collect_nominal_declarations(&parameter.ty, &mut referenced);
                    for id in referenced {
                        changed |= selected_types.insert(id);
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    close_type_declarations(program, &mut selected_types)?;
    let schema = graph_schema(program)?;
    let mut output = crate::bounded_output::CappedString::new();
    write!(
        output,
        "{{\"schema\":{},\"revision\":{},\"prelude\":{{\"schema\":{},\"digest\":{}}},\"view\":{},\"identity\":{{\"declarations\":\"explicit-persistent-or-automatic-unstable\",\"values\":\"revision-scoped-structural\",\"expressions\":\"revision-scoped-structural\",\"match_arms\":\"revision-scoped-structural\",\"patterns\":\"revision-scoped-structural\",\"type_parameters\":\"owner-and-index-stable\"}}{},\"module\":{},\"permits\":{},\"entrypoint\":{},\"type_facts\":[{}],\"nodes\":[",
        quote_json(schema),
        quote_json(source_revision),
        quote_json(prelude::SCHEMA_V1),
        quote_json(&prelude::digest_text_v1()),
        view_json(view),
        portable_indexed_byte_data_json(schema, program)?,
        quote_json(&program.module),
        string_array(&program.permits),
        quote_json(program.entrypoint.as_str()),
        type_facts_array(program, selected_functions, &selected_types)?
    )
    .expect("writing to a string cannot fail");

    let mut first = true;
    for declaration in &program.types {
        if !selected_types.contains(&declaration.id) {
            continue;
        }
        if !first {
            output.push(',');
        }
        first = false;
        let ty = ResolvedType::Nominal {
            declaration: declaration.id.clone(),
            arguments: Vec::new(),
        };
        let type_origin = identity_origin(program, &declaration.id)?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                write!(
                    output,
                    "{{\"id\":{},\"kind\":\"resource\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"memory\":\"unique\",\"type_id\":{},\"drop\":{}}}",
                    quote_json(declaration.id.as_str()),
                    quote_json(&declaration.name),
                    quote_json(type_origin.text()),
                    type_origin.is_persistent(),
                    quote_json(&ty.identity_key()),
                    quote_json(drop.id.as_str())
                )
                .expect("writing to a string cannot fail");
                let drop_origin = identity_origin(program, &drop.id)?;
                output.push(',');
                match &drop.kind {
                    ResolvedResourceDropKind::Trivial => write!(
                        output,
                        "{{\"id\":{},\"kind\":\"resource_drop\",\"owner\":{},\"identity_origin\":{},\"persistent\":{},\"strategy\":\"trivial\"}}",
                        quote_json(drop.id.as_str()),
                        quote_json(declaration.id.as_str()),
                        quote_json(drop_origin.text()),
                        drop_origin.is_persistent()
                    ),
                    ResolvedResourceDropKind::Imported { import, import_key } => write!(
                        output,
                        "{{\"id\":{},\"kind\":\"resource_drop\",\"owner\":{},\"identity_origin\":{},\"persistent\":{},\"strategy\":\"imported\",\"import\":{},\"import_key\":{}}}",
                        quote_json(drop.id.as_str()),
                        quote_json(declaration.id.as_str()),
                        quote_json(drop_origin.text()),
                        drop_origin.is_persistent(),
                        quote_json(import.as_str()),
                        quote_json(import_key)
                    ),
                }
                .expect("writing to a string cannot fail");
            }
            ResolvedTypeDeclarationKind::Record { fields } => {
                let (parameters, type_id) = if declaration.type_parameters.is_empty() {
                    (String::new(), quote_json(&ty.identity_key()))
                } else {
                    (
                        format!(
                            ",\"type_parameters\":[{}]",
                            type_parameters_json(&declaration.id, &declaration.type_parameters)
                        ),
                        "null".to_owned(),
                    )
                };
                write!(
                    output,
                    "{{\"id\":{},\"kind\":\"record\",\"name\":{},\"identity_origin\":{},\"persistent\":{}{parameters},\"type_id\":{},\"fields\":[{}]}}",
                    quote_json(declaration.id.as_str()),
                    quote_json(&declaration.name),
                    quote_json(type_origin.text()),
                    type_origin.is_persistent(),
                    type_id,
                    fields
                        .iter()
                        .map(|field| quote_json(field.id.as_str()))
                        .collect::<Vec<_>>()
                        .budgeted_join(",")
                )
                .expect("writing to a string cannot fail");

                for (index, field) in fields.iter().enumerate() {
                    let metadata = program
                        .declarations
                        .declaration(&field.id)
                        .ok_or_else(|| graph_reference_error("field", &field.id))?;
                    if metadata.kind != crate::hir::DeclarationKind::Field
                        || metadata.owner.as_ref() != Some(&declaration.id)
                    {
                        return Err(Diagnostic::io(
                            "SPX-G003",
                            format!(
                                "field `{}` is not indexed under record `{}`",
                                field.id, declaration.id
                            ),
                        ));
                    }
                    output.push(',');
                    write!(
                        output,
                        "{{\"id\":{},\"kind\":\"field\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"owner\":{},\"index\":{index},\"type_id\":{}}}",
                        quote_json(field.id.as_str()),
                        quote_json(&field.name),
                        quote_json(metadata.identity_origin.text()),
                        metadata.identity_origin.is_persistent(),
                        quote_json(declaration.id.as_str()),
                        quote_json(&field.ty.identity_key())
                    )
                    .expect("writing to a string cannot fail");
                }
            }
            ResolvedTypeDeclarationKind::Class { fields, methods } => {
                let (parameters, type_id) = if declaration.type_parameters.is_empty() {
                    (String::new(), quote_json(&ty.identity_key()))
                } else {
                    (
                        format!(
                            ",\"type_parameters\":[{}]",
                            type_parameters_json(&declaration.id, &declaration.type_parameters)
                        ),
                        "null".to_owned(),
                    )
                };
                // Class Inheritance v1: the declared parent is an additive
                // graph fact; parentless classes omit the key entirely so
                // pre-inheritance projections stay byte-identical.
                let extends = program
                    .declarations
                    .class_parent(&declaration.id)
                    .map_or_else(String::new, |parent| {
                        format!(",\"extends\":{}", quote_json(parent.as_str()))
                    });
                write!(
                    output,
                    "{{\"id\":{},\"kind\":\"class\",\"name\":{},\"identity_origin\":{},\"persistent\":{}{parameters},\"type_id\":{}{extends},\"fields\":[{}],\"methods\":[{}]}}",
                    quote_json(declaration.id.as_str()),
                    quote_json(&declaration.name),
                    quote_json(type_origin.text()),
                    type_origin.is_persistent(),
                    type_id,
                    fields
                        .iter()
                        .map(|field| quote_json(field.id.as_str()))
                        .collect::<Vec<_>>()
                        .budgeted_join(","),
                    methods
                        .iter()
                        .map(|method| quote_json(method.as_str()))
                        .collect::<Vec<_>>()
                        .budgeted_join(",")
                )
                .expect("writing to a string cannot fail");

                for (index, field) in fields.iter().enumerate() {
                    let metadata = program
                        .declarations
                        .declaration(&field.id)
                        .ok_or_else(|| graph_reference_error("field", &field.id))?;
                    // Class Inheritance v1: an effective member is declared by
                    // the rendering class or by one of its ancestors; the node
                    // always records the true declaring owner.
                    let declaring_owner = metadata.owner.clone().ok_or_else(|| {
                        Diagnostic::io(
                            "SPX-G003",
                            format!("field `{}` has no owning declaration", field.id),
                        )
                    })?;
                    let owner_is_visible = declaring_owner == declaration.id
                        || program
                            .declarations
                            .class_extends(&declaration.id, &declaring_owner);
                    if metadata.kind != crate::hir::DeclarationKind::Field || !owner_is_visible {
                        return Err(Diagnostic::io(
                            "SPX-G003",
                            format!(
                                "field `{}` is not indexed under class `{}`",
                                field.id, declaration.id
                            ),
                        ));
                    }
                    output.push(',');
                    write!(
                        output,
                        "{{\"id\":{},\"kind\":\"field\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"owner\":{},\"index\":{index},\"type_id\":{}}}",
                        quote_json(field.id.as_str()),
                        quote_json(&field.name),
                        quote_json(metadata.identity_origin.text()),
                        metadata.identity_origin.is_persistent(),
                        quote_json(declaring_owner.as_str()),
                        quote_json(&field.ty.identity_key())
                    )
                    .expect("writing to a string cannot fail");
                }
                for method in methods {
                    let metadata = program
                        .declarations
                        .declaration(method)
                        .ok_or_else(|| graph_reference_error("method", method))?;
                    if metadata.kind != crate::hir::DeclarationKind::Function
                        || metadata.owner.as_ref() != Some(&declaration.id)
                    {
                        return Err(Diagnostic::io(
                            "SPX-G003",
                            format!(
                                "method `{}` is not indexed under class `{}`",
                                method, declaration.id
                            ),
                        ));
                    }
                    output.push(',');
                    let resolved_method =
                        program.resolve_call_target(method, None).ok_or_else(|| {
                            Diagnostic::io(
                                "SPX-G003",
                                format!("method `{method}` has no resolved function body"),
                            )
                        })?;
                    let params_json = resolved_method
                        .params
                        .iter()
                        .map(|param| {
                            format!(
                                "{{\"id\":{},\"name\":{},\"type_id\":{}}}",
                                quote_json(param.id.as_str()),
                                quote_json(&param.name),
                                quote_json(&param.ty.identity_key())
                            )
                        })
                        .collect::<Vec<_>>()
                        .budgeted_join(",");
                    write!(
                        output,
                        "{{\"id\":{},\"kind\":\"function\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"owner\":{},\"params\":[{}],\"return_type_id\":{}}}",
                        quote_json(resolved_method.id.as_str()),
                        quote_json(&resolved_method.name),
                        quote_json(metadata.identity_origin.text()),
                        metadata.identity_origin.is_persistent(),
                        quote_json(declaration.id.as_str()),
                        params_json,
                        quote_json(&resolved_method.return_type.identity_key())
                    )
                    .expect("writing to a string cannot fail");
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                let type_id = if declaration.type_parameters.is_empty() {
                    quote_json(&ty.identity_key())
                } else {
                    "null".to_owned()
                };
                write!(
                    output,
                    "{{\"id\":{},\"kind\":\"variant\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"type_parameters\":[{}],\"type_id\":{},\"cases\":[{}]}}",
                    quote_json(declaration.id.as_str()),
                    quote_json(&declaration.name),
                    quote_json(type_origin.text()),
                    type_origin.is_persistent(),
                    type_parameters_json(&declaration.id, &declaration.type_parameters),
                    type_id,
                    cases
                        .iter()
                        .map(|case| quote_json(case.id.as_str()))
                        .collect::<Vec<_>>()
                        .budgeted_join(",")
                )
                .expect("writing to a string cannot fail");
                for (case_index, case) in cases.iter().enumerate() {
                    let case_metadata = program
                        .declarations
                        .declaration(&case.id)
                        .ok_or_else(|| graph_reference_error("variant case", &case.id))?;
                    if case_metadata.kind != crate::hir::DeclarationKind::VariantCase
                        || case_metadata.owner.as_ref() != Some(&declaration.id)
                    {
                        return Err(Diagnostic::io(
                            "SPX-G003",
                            format!(
                                "case `{}` is not indexed under variant `{}`",
                                case.id, declaration.id
                            ),
                        ));
                    }
                    output.push(',');
                    write!(
                        output,
                        "{{\"id\":{},\"kind\":\"variant_case\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"owner\":{},\"index\":{case_index},\"fields\":[{}]}}",
                        quote_json(case.id.as_str()),
                        quote_json(&case.name),
                        quote_json(case_metadata.identity_origin.text()),
                        case_metadata.identity_origin.is_persistent(),
                        quote_json(declaration.id.as_str()),
                        case.fields
                            .iter()
                            .map(|field| quote_json(field.id.as_str()))
                            .collect::<Vec<_>>()
                            .budgeted_join(",")
                    )
                    .expect("writing to a string cannot fail");
                    for (field_index, field) in case.fields.iter().enumerate() {
                        let field_metadata = program
                            .declarations
                            .declaration(&field.id)
                            .ok_or_else(|| graph_reference_error("case field", &field.id))?;
                        if field_metadata.kind != crate::hir::DeclarationKind::CaseField
                            || field_metadata.owner.as_ref() != Some(&case.id)
                        {
                            return Err(Diagnostic::io(
                                "SPX-G003",
                                format!(
                                    "field `{}` is not indexed under case `{}`",
                                    field.id, case.id
                                ),
                            ));
                        }
                        output.push(',');
                        write!(
                            output,
                            "{{\"id\":{},\"kind\":\"case_field\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"owner\":{},\"index\":{field_index},\"type_id\":{}}}",
                            quote_json(field.id.as_str()),
                            quote_json(&field.name),
                            quote_json(field_metadata.identity_origin.text()),
                            field_metadata.identity_origin.is_persistent(),
                            quote_json(case.id.as_str()),
                            quote_json(&field.ty.identity_key())
                        )
                        .expect("writing to a string cannot fail");
                    }
                }
            }
        }
    }

    for interface in &program.interfaces {
        if !selected_interfaces.contains(&interface.id) {
            continue;
        }
        if !first {
            output.push(',');
        }
        first = false;
        let origin = identity_origin(program, &interface.id)?;
        write!(
            output,
            "{{\"id\":{},\"kind\":\"interface\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"permits\":{},\"imports\":[{}]}}",
            quote_json(interface.id.as_str()),
            quote_json(&interface.name),
            quote_json(origin.text()),
            origin.is_persistent(),
            string_array(&interface.permits),
            interface
                .imports
                .iter()
                .map(|import| quote_json(import.id.as_str()))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        )
        .expect("writing to a string cannot fail");
        for import in &interface.imports {
            let origin = identity_origin(program, &import.id)?;
            let parameters = import
                .parameters
                .iter()
                .map(|parameter| {
                    format!(
                        "{{\"name\":{},\"type_id\":{},\"ownership_mode\":{},\"consumes_on_failure\":{}}}",
                        quote_json(&parameter.name),
                        quote_json(&parameter.ty.identity_key()),
                        quote_json(ownership_text(parameter.ownership)),
                        parameter.consumes_on_failure
                    )
                })
                .collect::<Vec<_>>()
                .budgeted_join(",");
            let failure = match &import.failure {
                ResolvedImportFailure::Infallible => "{\"kind\":\"infallible\"}".to_owned(),
                ResolvedImportFailure::Status {
                    domain_id,
                    normalization,
                } => format!(
                    "{{\"kind\":\"status\",\"domain_id\":{},\"normalization\":{}}}",
                    quote_json(domain_id),
                    quote_json(normalization)
                ),
            };
            output.push(',');
            write!(
                output,
                "{{\"id\":{},\"kind\":\"import\",\"name\":{},\"owner\":{},\"identity_origin\":{},\"persistent\":{},\"import_key\":{},\"parameters\":[{}],\"result\":{{\"type\":{},\"ownership_mode\":\"value\",\"producer\":{},\"out_slot_initialization\":{},\"ownership_transfer\":{}}},\"effects\":{},\"required_authority\":{},\"failure\":{}",
                quote_json(import.id.as_str()),
                quote_json(&import.name),
                quote_json(interface.id.as_str()),
                quote_json(origin.text()),
                origin.is_persistent(),
                quote_json(&import.import_key),
                parameters,
                quote_json(native_import::result_text(&import.result.kind)),
                quote_json(import.result.producer),
                quote_json(import.result.out_slot_initialization),
                quote_json(import.result.ownership_transfer),
                string_array(&import.effects),
                string_array(&import.required_authority),
                failure
            )
            .expect("writing to a string cannot fail");
            native_import::append_import_tail(&mut output, schema, import.native_rust);
        }
    }

    for function in &program.functions {
        if !selected_functions.contains(&function.id) {
            continue;
        }
        if !first {
            output.push(',');
        }
        first = false;

        let mut calls = BTreeSet::new();
        visit_function_calls(function, &mut |callee| {
            calls.insert(callee.as_str().to_owned());
        });
        let params = function
            .params
            .iter()
            .map(|param| {
                let provenance = byte_slice_provenance_json(schema, program, param)?;
                Ok(match provenance {
                    Some(provenance) => format!(
                        "{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{},\"byte_slice_provenance\":{provenance}}}",
                        quote_json(param.id.as_str()),
                        quote_json(&param.name),
                        quote_json(&param.ty.identity_key()),
                        quote_json(ownership_text(param.ownership))
                    ),
                    None => format!(
                        "{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}",
                        quote_json(param.id.as_str()),
                        quote_json(&param.name),
                        quote_json(&param.ty.identity_key()),
                        quote_json(ownership_text(param.ownership))
                    ),
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?
            .budgeted_join(",");
        let requires = function
            .requires
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .budgeted_join(",");
        let ensures = function
            .ensures
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .budgeted_join(",");
        let body = expr_json(program, &function.body)?;
        let cleanup = crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan);
        let identity_origin = identity_origin(program, &function.id)?;
        let result_ownership = result_ownership(program, &function.return_type)?;
        let calls = calls.into_iter().collect::<Vec<_>>();

        write!(
            output,
            "{{\"id\":{},\"kind\":\"function\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"params\":[{}],\"result_id\":{},\"result\":{{\"id\":{},\"type_id\":{},\"ownership_mode\":{}}},\"return_type_id\":{},\"effects\":{},\"requires_graph\":[{}],\"ensures_graph\":[{}],\"calls\":{},\"body\":{},\"cleanup\":{}",
            quote_json(function.id.as_str()),
            quote_json(&function.name),
            quote_json(identity_origin.text()),
            identity_origin.is_persistent(),
            params,
            quote_json(function.result_id.as_str()),
            quote_json(function.result_id.as_str()),
            quote_json(&function.return_type.identity_key()),
            quote_json(ownership_text(result_ownership)),
            quote_json(&function.return_type.identity_key()),
            string_array(&function.effects),
            requires,
            ensures,
            string_array(&calls),
            body,
            cleanup
        )
        .expect("writing to a string cannot fail");
        if graph_schema_includes_loans(schema) {
            write!(
                output,
                ",\"loans\":{}}}",
                crate::graph_loan::loan_plan_json(&function.loan_plan)
            )
            .expect("writing to a string cannot fail");
        } else {
            output.push('}');
        }
    }
    for template in &program.function_templates {
        if !selected_functions.contains(&template.id) {
            continue;
        }
        if !first {
            output.push(',');
        }
        first = false;
        let params = template
            .params
            .iter()
            .map(|param| {
                format!(
                    "{{\"id\":{},\"name\":{},\"type\":{},\"ownership_mode\":\"value\"}}",
                    quote_json(param.id.as_str()),
                    quote_json(&param.name),
                    type_json(&param.ty)
                )
            })
            .collect::<Vec<_>>()
            .budgeted_join(",");
        let requires = template
            .requires
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .budgeted_join(",");
        let ensures = template
            .ensures
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .budgeted_join(",");
        let identity_origin = identity_origin(program, &template.id)?;
        write!(
            output,
            "{{\"id\":{},\"kind\":\"function_template\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"type_parameters\":[{}],\"params\":[{}],\"result_id\":{},\"return_type\":{},\"effects\":{},\"requires_graph\":[{}],\"ensures_graph\":[{}],\"body\":{}}}",
            quote_json(template.id.as_str()),
            quote_json(&template.name),
            quote_json(identity_origin.text()),
            identity_origin.is_persistent(),
            type_parameters_json(&template.id, &template.type_parameters),
            params,
            quote_json(template.result_id.as_str()),
            type_json(&template.return_type),
            string_array(&template.effects),
            requires,
            ensures,
            expr_json(program, &template.body)?
        )
        .expect("writing to a string cannot fail");
    }
    for instance in &program.function_instances {
        if !selected_functions.contains(&instance.template) {
            continue;
        }
        if !first {
            output.push(',');
        }
        first = false;
        let function = &instance.function;
        let params = function
            .params
            .iter()
            .map(|param| {
                format!(
                    "{{\"id\":{},\"name\":{},\"type\":{},\"ownership_mode\":\"value\"}}",
                    quote_json(param.id.as_str()),
                    quote_json(&param.name),
                    type_json(&param.ty)
                )
            })
            .collect::<Vec<_>>()
            .budgeted_join(",");
        let requires = function
            .requires
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .budgeted_join(",");
        let ensures = function
            .ensures
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .budgeted_join(",");
        let execution = FunctionExecutionId::Generic(instance.id.clone()).identity_key();
        write!(
            output,
            "{{\"id\":{},\"kind\":\"function_instance\",\"persistent\":false,\"template\":{},\"instance\":{},\"type_arguments\":[{}],\"params\":[{}],\"result_id\":{},\"return_type\":{},\"requires_graph\":[{}],\"ensures_graph\":[{}],\"body\":{},\"cleanup\":{}",
            quote_json(&execution),
            quote_json(instance.template.as_str()),
            quote_json(instance.id.as_str()),
            instance.type_arguments.iter().map(type_json).collect::<Vec<_>>().budgeted_join(","),
            params,
            quote_json(function.result_id.as_str()),
            type_json(&function.return_type),
            requires,
            ensures,
            expr_json(program, &function.body)?,
            crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan)
        )
        .expect("writing to a string cannot fail");
        if graph_schema_includes_loans(schema) {
            write!(
                output,
                ",\"loans\":{}}}",
                crate::graph_loan::loan_plan_json(&function.loan_plan)
            )
            .expect("writing to a string cannot fail");
        } else {
            output.push('}');
        }
    }
    output.push_str("]}");
    Ok(output.into_string())
}

fn result_ownership(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<OwnershipMode, Diagnostic> {
    program
        .declarations
        .type_facts(ty)
        .map(|facts| {
            if facts.copy {
                OwnershipMode::Value
            } else {
                OwnershipMode::Own
            }
        })
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-G001",
                format!(
                    "semantic graph has no facts for resolved type `{}`",
                    ty.identity_key()
                ),
            )
        })
}

fn view_json(view: &GraphView<'_>) -> String {
    match view {
        GraphView::Module => "{\"kind\":\"module\"}".to_owned(),
        GraphView::Context {
            root,
            depth,
            frontier,
        } => format!(
            "{{\"kind\":\"context\",\"root\":{},\"depth\":{depth},\"truncated\":{},\"frontier\":[{}]}}",
            quote_json(root.as_str()),
            !frontier.is_empty(),
            frontier
                .iter()
                .map(|id| quote_json(id.as_str()))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    }
}

fn identity_origin(
    program: &ResolvedProgram,
    id: &DeclarationId,
) -> Result<IdentityOrigin, Diagnostic> {
    program
        .declarations
        .declaration(id)
        .map(|declaration| declaration.identity_origin)
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-G002",
                format!("semantic graph has no declaration metadata for `{id}`"),
            )
        })
}

fn graph_reference_error(kind: &str, id: &DeclarationId) -> Diagnostic {
    Diagnostic::io(
        "SPX-G003",
        format!("semantic graph contains an unresolved {kind} reference `{id}`"),
    )
}

fn visit_function_calls(function: &ResolvedFunction, visit: &mut impl FnMut(&DeclarationId)) {
    for contract in &function.requires {
        visit_expr_calls(contract, visit);
    }
    visit_expr_calls(&function.body, visit);
    for contract in &function.ensures {
        visit_expr_calls(contract, visit);
    }
}

fn visit_expr_call_instances(
    expression: &ResolvedExpr,
    visit: &mut impl FnMut(&ResolvedExpr, &DeclarationId, &[ResolvedType], &FunctionInstanceId),
) {
    if let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: Some(instance),
        ..
    } = &expression.kind
    {
        visit(expression, callee, type_arguments, instance);
    }
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            visit_expr_call_instances(source, visit);
            visit_expr_call_instances(start, visit);
            visit_expr_call_instances(end, visit);
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                visit_expr_call_instances(argument, visit);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                visit_expr_call_instances(argument, visit);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                visit_expr_call_instances(argument, visit);
            }
        }
        ResolvedExprKind::Unary { value, .. } | ResolvedExprKind::Project { base: value, .. } => {
            visit_expr_call_instances(value, visit);
        }
        ResolvedExprKind::Upcast { source: value } => {
            visit_expr_call_instances(value, visit);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            visit_expr_call_instances(left, visit);
            visit_expr_call_instances(right, visit);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        visit_expr_call_instances(child, visit);
                    }
                }
            }
            visit_expr_call_instances(tail, visit);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_expr_call_instances(condition, visit);
            visit_expr_call_instances(then_branch, visit);
            visit_expr_call_instances(else_branch, visit);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for initializer in fields {
                visit_expr_call_instances(&initializer.value, visit);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            visit_expr_call_instances(scrutinee, visit);
            for arm in arms {
                visit_expr_call_instances(&arm.value, visit);
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            visit_expr_call_instances(operand, visit);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            visit_expr_call_instances(base, visit);
            for initializer in fields {
                visit_expr_call_instances(&initializer.value, visit);
            }
        }
    }
}

fn visit_expr_calls(expression: &ResolvedExpr, visit: &mut impl FnMut(&DeclarationId)) {
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            visit_expr_calls(source, visit);
            visit_expr_calls(start, visit);
            visit_expr_calls(end, visit);
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
        ResolvedExprKind::Call { callee, args, .. } => {
            visit(callee);
            for argument in args {
                visit_expr_calls(argument, visit);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                visit_expr_calls(argument, visit);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                visit_expr_calls(argument, visit);
            }
        }
        ResolvedExprKind::Unary { value, .. } => visit_expr_calls(value, visit),
        ResolvedExprKind::Upcast { source: value } => visit_expr_calls(value, visit),
        ResolvedExprKind::Binary { left, right, .. } => {
            visit_expr_calls(left, visit);
            visit_expr_calls(right, visit);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        visit_expr_calls(child, visit);
                    }
                }
            }
            visit_expr_calls(tail, visit);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_expr_calls(condition, visit);
            visit_expr_calls(then_branch, visit);
            visit_expr_calls(else_branch, visit);
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for initializer in fields {
                visit_expr_calls(&initializer.value, visit);
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for initializer in fields {
                visit_expr_calls(&initializer.value, visit);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            visit_expr_calls(scrutinee, visit);
            for arm in arms {
                visit_expr_calls(&arm.value, visit);
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            visit_expr_calls(operand, visit)
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            visit_expr_calls(base, visit);
            for initializer in fields {
                visit_expr_calls(&initializer.value, visit);
            }
        }
        ResolvedExprKind::Project { base, .. } => visit_expr_calls(base, visit),
    }
}

fn collect_function_type_declarations(
    function: &ResolvedFunction,
    declarations: &mut BTreeSet<DeclarationId>,
) {
    for param in &function.params {
        collect_nominal_declarations(&param.ty, declarations);
    }
    collect_nominal_declarations(&function.return_type, declarations);
    for expression in &function.requires {
        collect_expr_type_declarations(expression, declarations);
    }
    collect_expr_type_declarations(&function.body, declarations);
    for expression in &function.ensures {
        collect_expr_type_declarations(expression, declarations);
    }
}

fn collect_expr_type_declarations(
    expression: &ResolvedExpr,
    declarations: &mut BTreeSet<DeclarationId>,
) {
    collect_nominal_declarations(&expression.ty, declarations);
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_expr_type_declarations(source, declarations);
            collect_expr_type_declarations(start, declarations);
            collect_expr_type_declarations(end, declarations);
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expr_type_declarations(argument, declarations);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_expr_type_declarations(argument, declarations);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                collect_expr_type_declarations(argument, declarations);
            }
        }
        ResolvedExprKind::Unary { value, .. } => {
            collect_expr_type_declarations(value, declarations);
        }
        ResolvedExprKind::Upcast { source: value } => {
            collect_expr_type_declarations(value, declarations);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr_type_declarations(left, declarations);
            collect_expr_type_declarations(right, declarations);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                if let ResolvedStatement::Let { binding, .. } = statement {
                    collect_nominal_declarations(&binding.ty, declarations);
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        collect_expr_type_declarations(child, declarations);
                    }
                }
            }
            collect_expr_type_declarations(tail, declarations);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_type_declarations(condition, declarations);
            collect_expr_type_declarations(then_branch, declarations);
            collect_expr_type_declarations(else_branch, declarations);
        }
        ResolvedExprKind::ConstructRecord { record, fields } => {
            declarations.insert(record.clone());
            for initializer in fields {
                collect_expr_type_declarations(&initializer.value, declarations);
            }
        }
        ResolvedExprKind::ConstructVariant {
            variant, fields, ..
        } => {
            declarations.insert(variant.clone());
            for initializer in fields {
                collect_expr_type_declarations(&initializer.value, declarations);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_expr_type_declarations(scrutinee, declarations);
            for arm in arms {
                match &arm.pattern {
                    crate::hir::ResolvedMatchPattern::Variant {
                        variant, fields, ..
                    } => {
                        declarations.insert(variant.clone());
                        for field in fields {
                            collect_nominal_declarations(&field.binding.ty, declarations);
                        }
                    }
                    crate::hir::ResolvedMatchPattern::Record {
                        record,
                        instance,
                        fields,
                    } => collect_record_pattern_type_declarations(
                        record,
                        instance,
                        fields,
                        declarations,
                    ),
                    crate::hir::ResolvedMatchPattern::Wildcard => {}
                    // Refutable Match v1: scalar binding types join the
                    // closure; literals and or-patterns carry no declarations.
                    crate::hir::ResolvedMatchPattern::Binding(binding) => {
                        collect_nominal_declarations(&binding.ty, declarations);
                    }
                    crate::hir::ResolvedMatchPattern::Literal(_)
                    | crate::hir::ResolvedMatchPattern::Or(_) => {}
                }
                if let Some(guard) = &arm.guard {
                    collect_expr_type_declarations(guard, declarations);
                }
                collect_expr_type_declarations(&arm.value, declarations);
            }
        }
        ResolvedExprKind::Try {
            operand,
            result,
            residual_type,
            ..
        } => {
            declarations.insert(result.clone());
            collect_expr_type_declarations(operand, declarations);
            collect_nominal_declarations(residual_type, declarations);
        }
        ResolvedExprKind::TryOption {
            operand,
            option,
            residual_type,
            ..
        } => {
            declarations.insert(option.clone());
            collect_expr_type_declarations(operand, declarations);
            collect_nominal_declarations(residual_type, declarations);
        }
        ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } => {
            declarations.insert(record.clone());
            collect_expr_type_declarations(base, declarations);
            for initializer in fields {
                collect_expr_type_declarations(&initializer.value, declarations);
            }
        }
        ResolvedExprKind::Project { base, .. } => {
            collect_expr_type_declarations(base, declarations);
        }
    }
}

fn collect_record_pattern_type_declarations(
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
    declarations: &mut BTreeSet<DeclarationId>,
) {
    declarations.insert(record.clone());
    collect_nominal_declarations(instance, declarations);
    for field in fields {
        match &field.pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                collect_nominal_declarations(&binding.ty, declarations);
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
            crate::hir::ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => collect_record_pattern_type_declarations(record, instance, fields, declarations),
        }
    }
}

fn collect_nominal_declarations(ty: &ResolvedType, declarations: &mut BTreeSet<DeclarationId>) {
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        declarations.insert(declaration.clone());
        for argument in arguments {
            collect_nominal_declarations(argument, declarations);
        }
    }
}

fn close_type_declarations(
    program: &ResolvedProgram,
    declarations: &mut BTreeSet<DeclarationId>,
) -> Result<(), Diagnostic> {
    let types = program
        .types
        .iter()
        .map(|declaration| (declaration.id.clone(), declaration))
        .collect::<BTreeMap<_, _>>();
    let mut queue = declarations.iter().cloned().collect::<VecDeque<_>>();
    while let Some(id) = queue.pop_front() {
        let declaration = types
            .get(&id)
            .copied()
            .ok_or_else(|| graph_reference_error("type declaration", &id))?;
        let fields = match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                fields.iter().collect::<Vec<_>>()
            }
            ResolvedTypeDeclarationKind::Variant { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .collect::<Vec<_>>(),
            ResolvedTypeDeclarationKind::Resource { .. } => Vec::new(),
        };
        for field in fields {
            let mut referenced = BTreeSet::new();
            collect_nominal_declarations(&field.ty, &mut referenced);
            for referenced_id in referenced {
                if declarations.insert(referenced_id.clone()) {
                    queue.push_back(referenced_id);
                }
            }
        }
    }
    Ok(())
}

fn expr_json(program: &ResolvedProgram, expression: &ResolvedExpr) -> Result<String, Diagnostic> {
    let header = format!(
        "\"id\":{},\"type_id\":{},\"ownership_mode\":{}",
        quote_json(expression.id.as_str()),
        quote_json(&expression.ty.identity_key()),
        quote_json(ownership_text(expression.ownership))
    );
    let output = match &expression.kind {
        ResolvedExprKind::Int(value) => {
            format!(
                "{{{header},\"kind\":\"int\",\"value\":{}}}",
                quote_json(&value.to_string())
            )
        }
        ResolvedExprKind::Int32(value) => {
            format!("{{{header},\"kind\":\"int32\",\"value\":{value}}}")
        }
        ResolvedExprKind::Char(value) => format!(
            "{{{header},\"kind\":\"char\",\"value\":{value},\"display\":{}}}",
            quote_json(&crate::format::canonical_char(*value))
        ),
        ResolvedExprKind::Uint8(value) => {
            format!("{{{header},\"kind\":\"uint8\",\"value\":{value}}}")
        }
        ResolvedExprKind::Usize(value) => format!(
            "{{{header},\"kind\":\"usize\",\"value\":{}}}",
            quote_json(&value.to_string())
        ),
        ResolvedExprKind::ArrayU8(values) => format!(
            "{{{header},\"kind\":\"array_u8\",\"form\":\"explicit\",\"length\":{},\"values\":[{}]}}",
            values.len(),
            values.iter().map(u8::to_string).collect::<Vec<_>>().budgeted_join(",")
        ),
        ResolvedExprKind::RepeatArrayU8 { value, count } => format!(
            "{{{header},\"kind\":\"array_u8\",\"form\":\"repeat\",\"length\":{count},\"value\":{value}}}"
        ),
        ResolvedExprKind::Float32(bits) => format!(
            "{{{header},\"kind\":\"float32\",\"bits\":\"{bits:08x}\",\"value\":{}}}",
            quote_json(&crate::format::canonical_f32_bits(*bits))
        ),
        ResolvedExprKind::Float64(bits) => format!(
            "{{{header},\"kind\":\"float64\",\"bits\":\"{bits:016x}\",\"value\":{}}}",
            quote_json(&crate::format::canonical_f64_bits(*bits))
        ),
        ResolvedExprKind::Bool(value) => {
            format!("{{{header},\"kind\":\"bool\",\"value\":{value}}}")
        }
        ResolvedExprKind::String(value) => format!(
            "{{{header},\"kind\":\"string\",\"value\":{},\"display\":{}}}",
            quote_json(value),
            quote_json(&crate::format::canonical_string(value))
        ),
        ResolvedExprKind::Place(place) => format!(
            "{{{header},\"kind\":\"place\",\"place\":{}}}",
            place_json(place)
        ),
        ResolvedExprKind::BorrowPlace { operation, place } => format!(
            "{{{header},\"kind\":\"byte_view\",\"operation\":{},\"place\":{}}}",
            quote_json(operation.as_str()),
            place_json(place)
        ),
        ResolvedExprKind::ByteRange { operation, source, start, end } => format!(
            "{{{header},\"kind\":\"byte_range\",\"operation\":{},\"source\":{},\"start\":{},\"end\":{},\"status_domain\":{},\"status_codes\":{{\"start_after_end\":{},\"end_out_of_bounds\":{}}}}}",
            quote_json(operation.as_str()), expr_json(program, source)?,
            expr_json(program, start)?, expr_json(program, end)?,
            quote_json(crate::byte_ops::RANGE_STATUS_DOMAIN),
            crate::byte_ops::RANGE_START_AFTER_END_CODE,
            crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
        ),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            let args = args
                .iter()
                .map(|argument| expr_json(program, argument))
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",");
            if let Some(instance) = instance {
                format!(
                    "{{{header},\"kind\":\"call_instance\",\"template\":{},\"instance\":{},\"type_arguments\":[{}],\"args\":[{}]}}",
                    quote_json(callee.as_str()),
                    quote_json(instance.as_str()),
                    type_arguments.iter().map(type_json).collect::<Vec<_>>().budgeted_join(","),
                    args
                )
            } else {
                format!(
                    "{{{header},\"kind\":\"call\",\"callee\":{},\"args\":[{}]}}",
                    quote_json(callee.as_str()),
                    args
                )
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let args = call
                .args
                .iter()
                .map(|argument| expr_json(program, argument))
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",");
            format!(
                "{{{header},\"kind\":\"native_rust_import_call\",\"import\":{},\"result\":{},\"args\":[{}]}}",
                quote_json(call.import.as_str()),
                quote_json(native_import::result_text(&call.result)),
                args
            )
        }
        ResolvedExprKind::HostCommandCall(call) => {
            let args = call
                .args
                .iter()
                .map(|argument| expr_json(program, argument))
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",");
            format!(
                "{{{header},\"kind\":\"host_command_call\",\"operation\":{},\"args\":[{}]}}",
                quote_json(crate::command_io_ops::id(call.operation)),
                args
            )
        }
        ResolvedExprKind::Unary { op, value } => format!(
            "{{{header},\"kind\":\"unary\",\"op\":{},\"value\":{}}}",
            quote_json(unary_text(*op)),
            expr_json(program, value)?
        ),
        ResolvedExprKind::Binary { op, left, right } => format!(
            "{{{header},\"kind\":\"binary\",\"op\":{},\"left\":{},\"right\":{}}}",
            quote_json(binary_text(*op)),
            expr_json(program, left)?,
            expr_json(program, right)?
        ),
        ResolvedExprKind::Block { statements, tail } => format!(
            "{{{header},\"kind\":\"block\",\"statements\":[{}],\"tail\":{}}}",
            statements
                .iter()
                .map(|statement| statement_json(program, statement))
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(","),
            expr_json(program, tail)?
        ),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "{{{header},\"kind\":\"if\",\"condition\":{},\"then\":{},\"else\":{}}}",
            expr_json(program, condition)?,
            expr_json(program, then_branch)?,
            expr_json(program, else_branch)?
        ),
        ResolvedExprKind::ConstructRecord { record, fields } => {
            let instance = match &expression.ty {
                ResolvedType::Nominal { arguments, .. } if !arguments.is_empty() => {
                    format!(",\"record_type\":{}", type_json(&expression.ty))
                }
                _ => String::new(),
            };
            format!(
                "{{{header},\"kind\":\"construct_record\",\"record\":{}{instance},\"fields\":[{}]}}",
                quote_json(record.as_str()),
                fields
                    .iter()
                    .map(|initializer| {
                        Ok(format!(
                            "{{\"field\":{},\"value\":{}}}",
                            quote_json(initializer.field.as_str()),
                            expr_json(program, &initializer.value)?
                        ))
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?
                    .budgeted_join(",")
            )
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => format!(
            "{{{header},\"kind\":\"construct_variant\",\"variant\":{},\"case\":{},\"fields\":[{}]}}",
            quote_json(variant.as_str()),
            quote_json(case.as_str()),
            fields
                .iter()
                .map(|initializer| {
                    Ok(format!(
                        "{{\"field\":{},\"value\":{}}}",
                        quote_json(initializer.field.as_str()),
                        expr_json(program, &initializer.value)?
                    ))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .budgeted_join(",")
        ),
        ResolvedExprKind::Match {
            mode,
            scrutinee,
            arms,
        } => {
            // Refutable Match v1: matches carrying guards or literal/or
            // patterns project `"exhaustive":false` plus additive per-arm
            // guard nodes; every pre-feature match keeps the exact
            // `"exhaustive":true` bytes.
            let exhaustive = !arms.iter().any(|arm| {
                arm.guard.is_some()
                    || matches!(
                        &arm.pattern,
                        crate::hir::ResolvedMatchPattern::Literal(_)
                            | crate::hir::ResolvedMatchPattern::Or(_)
                            | crate::hir::ResolvedMatchPattern::Binding(_)
                    )
            });
            format!(
                "{{{header},\"kind\":\"match\"{},\"exhaustive\":{exhaustive},\"scrutinee\":{},\"arms\":[{}]}}",
                explicit_match_mode_json(*mode),
                expr_json(program, scrutinee)?,
                arms.iter()
                    .enumerate()
                    .map(|(index, arm)| {
                        let arm_id = format!("{}:match-arm:{index}", expression.id.as_str());
                        let pattern_id = format!("{arm_id}:pattern");
                        let guard = match &arm.guard {
                            Some(guard) => {
                                let guard_id = format!("{arm_id}:guard");
                                format!(
                                    ",\"guard\":{{\"id\":{},\"kind\":\"guard\",\"condition\":{}}}",
                                    quote_json(&guard_id),
                                    expr_json(program, guard)?
                                )
                            }
                            None => String::new(),
                        };
                        Ok(format!(
                            "{{\"id\":{},\"kind\":\"match_arm\",\"pattern\":{},\"value\":{}{guard}}}",
                            quote_json(&arm_id),
                            graph_match_pattern_json(&arm.pattern, &pattern_id),
                            expr_json(program, &arm.value)?
                        ))
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?
                    .budgeted_join(",")
            )
        }
        ResolvedExprKind::Try {
            operand,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
        } => format!(
            "{{{header},\"kind\":\"try_result\",\"evaluation\":\"once\",\"operand\":{},\"source_result_type_id\":{},\"source_result_type\":{},\"residual_result_type_id\":{},\"residual_result_type\":{},\"result\":{},\"ok_case\":{},\"ok_field\":{},\"err_case\":{},\"err_field\":{},\"err_exit\":\"normal_result\",\"epilogue\":\"shared_postconditions\"}}",
            expr_json(program, operand)?,
            quote_json(&operand.ty.identity_key()),
            type_json(&operand.ty),
            quote_json(&residual_type.identity_key()),
            type_json(residual_type),
            quote_json(result.as_str()),
            quote_json(ok_case.as_str()),
            quote_json(ok_field.as_str()),
            quote_json(err_case.as_str()),
            quote_json(err_field.as_str())
        ),
        ResolvedExprKind::TryOption {
            operand,
            option,
            some_case,
            some_field,
            none_case,
            residual_type,
        } => format!(
            "{{{header},\"kind\":\"try_option\",\"evaluation\":\"once\",\"operand\":{},\"source_option_type_id\":{},\"source_option_type\":{},\"residual_option_type_id\":{},\"residual_option_type\":{},\"option\":{},\"some_case\":{},\"some_field\":{},\"none_case\":{},\"none_exit\":\"normal_result\",\"epilogue\":\"shared_postconditions\"}}",
            expr_json(program, operand)?,
            quote_json(&operand.ty.identity_key()),
            type_json(&operand.ty),
            quote_json(&residual_type.identity_key()),
            type_json(residual_type),
            quote_json(option.as_str()),
            quote_json(some_case.as_str()),
            quote_json(some_field.as_str()),
            quote_json(none_case.as_str())
        ),
        ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } => format!(
            "{{{header},\"kind\":\"update_record\",\"base\":{},\"record\":{},\"fields\":[{}]}}",
            expr_json(program, base)?,
            quote_json(record.as_str()),
            fields
                .iter()
                .map(|initializer| {
                    Ok(format!(
                        "{{\"field\":{},\"value\":{}}}",
                        quote_json(initializer.field.as_str()),
                        expr_json(program, &initializer.value)?
                    ))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .budgeted_join(",")
        ),
        ResolvedExprKind::Project { base, field } => format!(
            "{{{header},\"kind\":\"project\",\"base\":{},\"field\":{}}}",
            expr_json(program, base)?,
            quote_json(field.as_str())
        ),
        ResolvedExprKind::Upcast { source } => format!(
            "{{{header},\"kind\":\"upcast\",\"source\":{}}}",
            expr_json(program, source)?
        ),
    };
    Ok(output)
}

fn statement_json(
    program: &ResolvedProgram,
    statement: &ResolvedStatement,
) -> Result<String, Diagnostic> {
    match statement {
        ResolvedStatement::Let {
            binding,
            mutable,
            value,
            ..
        } => {
            // The mutable flag is additive and emitted only for `let mut`
            // bindings so pre-mutation graphs stay byte-identical.
            let mutable_field = if *mutable { ",\"mutable\":true" } else { "" };
            Ok(format!(
                "{{\"kind\":\"let\",\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}{},\"value\":{}}}",
                quote_json(binding.id.as_str()),
                quote_json(&binding.name),
                quote_json(&binding.ty.identity_key()),
                quote_json(ownership_text(binding.ownership)),
                mutable_field,
                expr_json(program, value)?
            ))
        }
        ResolvedStatement::Assign {
            binding,
            field,
            value,
            ..
        } => {
            // The field attribute is additive and emitted only on
            // `<binding>.<field>` targets so pre-field-mutation graphs stay
            // byte-identical.
            let field_attribute = match field {
                Some(field) => format!(",\"field\":{}", quote_json(field.as_str())),
                None => String::new(),
            };
            Ok(format!(
                "{{\"kind\":\"assign\",\"target\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}},\"value\":{}{field_attribute}}}",
                quote_json(binding.id.as_str()),
                quote_json(&binding.name),
                quote_json(&binding.ty.identity_key()),
                quote_json(ownership_text(binding.ownership)),
                expr_json(program, value)?
            ))
        }
        ResolvedStatement::Unsafe { audit, body, .. } => Ok(format!(
            "{{\"kind\":\"unsafe\",\"audit\":{},\"body\":{}}}",
            quote_json(audit),
            expr_json(program, body)?
        )),
        ResolvedStatement::While {
            condition, body, ..
        } => Ok(format!(
            "{{\"kind\":\"while\",\"condition\":{},\"body\":{}}}",
            expr_json(program, condition)?,
            expr_json(program, body)?
        )),
    }
}

fn place_json(place: &Place) -> String {
    format!(
        "{{\"root\":{},\"projections\":[{}]}}",
        quote_json(place.root.as_str()),
        place
            .projections
            .iter()
            .map(projection_json)
            .collect::<Vec<_>>()
            .budgeted_join(",")
    )
}

fn projection_json(projection: &PlaceProjection) -> String {
    match projection {
        PlaceProjection::Field(field) => format!(
            "{{\"kind\":\"field\",\"field\":{}}}",
            quote_json(field.as_str())
        ),
        PlaceProjection::VariantField { case, field } => format!(
            "{{\"kind\":\"variant_field\",\"case\":{},\"field\":{}}}",
            quote_json(case.as_str()),
            quote_json(field.as_str())
        ),
    }
}

fn type_facts_array(
    program: &ResolvedProgram,
    selected_functions: &BTreeSet<DeclarationId>,
    selected_types: &BTreeSet<DeclarationId>,
) -> Result<String, Diagnostic> {
    let mut types = BTreeMap::new();
    for declaration in &program.types {
        if !selected_types.contains(&declaration.id) {
            continue;
        }
        if declaration.type_parameters.is_empty() {
            collect_type(
                &ResolvedType::Nominal {
                    declaration: declaration.id.clone(),
                    arguments: Vec::new(),
                },
                &mut types,
            );
        }
        if declaration.type_parameters.is_empty() {
            if let ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } = &declaration.kind
            {
                for field in fields {
                    collect_type(&field.ty, &mut types);
                }
            }
        }
        if declaration.type_parameters.is_empty() {
            if let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind {
                for case in cases {
                    for field in &case.fields {
                        collect_type(&field.ty, &mut types);
                    }
                }
            }
        }
    }
    for function in &program.functions {
        if !selected_functions.contains(&function.id) {
            continue;
        }
        for param in &function.params {
            collect_type(&param.ty, &mut types);
        }
        collect_type(&function.return_type, &mut types);
        for expression in &function.requires {
            collect_expr_types(expression, &mut types);
        }
        collect_expr_types(&function.body, &mut types);
        for expression in &function.ensures {
            collect_expr_types(expression, &mut types);
        }
    }
    types
        .values()
        .map(|ty| {
            Ok(format!(
                "{{\"id\":{},\"type\":{},\"facts\":{}}}",
                quote_json(&ty.identity_key()),
                type_json(ty),
                facts_json(program, ty)?
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|items| items.budgeted_join(","))
}

fn collect_expr_types(expression: &ResolvedExpr, types: &mut BTreeMap<String, ResolvedType>) {
    collect_type(&expression.ty, types);
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_expr_types(source, types);
            collect_expr_types(start, types);
            collect_expr_types(end, types);
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expr_types(argument, types);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_expr_types(argument, types);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                collect_expr_types(argument, types);
            }
        }
        ResolvedExprKind::Unary { value, .. } => collect_expr_types(value, types),
        ResolvedExprKind::Upcast { source: value } => collect_expr_types(value, types),
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr_types(left, types);
            collect_expr_types(right, types);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                if let ResolvedStatement::Let { binding, .. } = statement {
                    collect_type(&binding.ty, types);
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        collect_expr_types(child, types);
                    }
                }
            }
            collect_expr_types(tail, types);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr_types(condition, types);
            collect_expr_types(then_branch, types);
            collect_expr_types(else_branch, types);
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for initializer in fields {
                collect_expr_types(&initializer.value, types);
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for initializer in fields {
                collect_expr_types(&initializer.value, types);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_expr_types(scrutinee, types);
            for arm in arms {
                match &arm.pattern {
                    crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
                        for field in fields {
                            collect_type(&field.binding.ty, types);
                        }
                    }
                    crate::hir::ResolvedMatchPattern::Record {
                        instance, fields, ..
                    } => collect_record_pattern_types(instance, fields, types),
                    crate::hir::ResolvedMatchPattern::Wildcard => {}
                    // Refutable Match v1: scalar binding types join the
                    // table; literals and or-patterns contribute nothing.
                    crate::hir::ResolvedMatchPattern::Binding(binding) => {
                        collect_type(&binding.ty, types);
                    }
                    crate::hir::ResolvedMatchPattern::Literal(_)
                    | crate::hir::ResolvedMatchPattern::Or(_) => {}
                }
                if let Some(guard) = &arm.guard {
                    collect_expr_types(guard, types);
                }
                collect_expr_types(&arm.value, types);
            }
        }
        ResolvedExprKind::Try {
            operand,
            residual_type,
            ..
        }
        | ResolvedExprKind::TryOption {
            operand,
            residual_type,
            ..
        } => {
            collect_expr_types(operand, types);
            collect_type(residual_type, types);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_expr_types(base, types);
            for initializer in fields {
                collect_expr_types(&initializer.value, types);
            }
        }
        ResolvedExprKind::Project { base, .. } => collect_expr_types(base, types),
    }
}

fn collect_record_pattern_types(
    instance: &ResolvedType,
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
    types: &mut BTreeMap<String, ResolvedType>,
) {
    collect_type(instance, types);
    for field in fields {
        match &field.pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                collect_type(&binding.ty, types);
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
            crate::hir::ResolvedRecordMatchFieldPattern::Record {
                instance, fields, ..
            } => collect_record_pattern_types(instance, fields, types),
        }
    }
}

fn collect_type(ty: &ResolvedType, types: &mut BTreeMap<String, ResolvedType>) {
    types.entry(ty.identity_key()).or_insert_with(|| ty.clone());
    if let ResolvedType::Nominal { arguments, .. } = ty {
        for argument in arguments {
            collect_type(argument, types);
        }
    }
}

fn type_json(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::Unit => "{\"kind\":\"primitive\",\"name\":\"unit\"}".to_owned(),
        ResolvedType::I64 => "{\"kind\":\"primitive\",\"name\":\"i64\"}".to_owned(),
        ResolvedType::I32 => "{\"kind\":\"primitive\",\"name\":\"i32\"}".to_owned(),
        ResolvedType::Char => "{\"kind\":\"primitive\",\"name\":\"char\"}".to_owned(),
        ResolvedType::U8 => "{\"kind\":\"primitive\",\"name\":\"u8\"}".to_owned(),
        ResolvedType::Usize => "{\"kind\":\"primitive\",\"name\":\"usize\"}".to_owned(),
        ResolvedType::ArrayU8(length) => format!(
            "{{\"element\":{{\"kind\":\"primitive\",\"name\":\"u8\"}},\"kind\":\"fixed_array\",\"length\":{length}}}"
        ),
        ResolvedType::F32 => "{\"kind\":\"primitive\",\"name\":\"f32\"}".to_owned(),
        ResolvedType::F64 => "{\"kind\":\"primitive\",\"name\":\"f64\"}".to_owned(),
        ResolvedType::Bool => "{\"kind\":\"primitive\",\"name\":\"bool\"}".to_owned(),
        ResolvedType::String => "{\"kind\":\"primitive\",\"name\":\"string\"}".to_owned(),
        ResolvedType::Bytes => "{\"kind\":\"owned_bytes\"}".to_owned(),
        ResolvedType::Str => "{\"kind\":\"primitive\",\"name\":\"str\"}".to_owned(),
        ResolvedType::SliceU8 => {
            "{\"element\":{\"kind\":\"primitive\",\"name\":\"u8\"},\"kind\":\"borrowed_slice\"}"
                .to_owned()
        }
        ResolvedType::TypeParameter { owner, index } => format!(
            "{{\"kind\":\"type_parameter\",\"owner\":{},\"index\":{index}}}",
            quote_json(owner.as_str())
        ),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => format!(
            "{{\"kind\":\"nominal\",\"declaration\":{},\"arguments\":[{}]}}",
            quote_json(declaration.as_str()),
            arguments
                .iter()
                .map(type_json)
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    }
}

fn type_parameters_json(
    owner: &DeclarationId,
    parameters: &[crate::hir::ResolvedTypeParameterDeclaration],
) -> String {
    parameters
        .iter()
        .map(|parameter| {
            let ty = ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: parameter.index,
            };
            format!(
                "{{\"id\":{},\"owner\":{},\"index\":{},\"name\":{}}}",
                quote_json(&ty.identity_key()),
                quote_json(owner.as_str()),
                parameter.index,
                quote_json(&parameter.name)
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",")
}

fn facts_json(program: &ResolvedProgram, ty: &ResolvedType) -> Result<String, Diagnostic> {
    program
        .declarations
        .type_facts(ty)
        .map(|facts| facts_object(&facts))
        .ok_or_else(|| {
            Diagnostic::io(
                "SPX-G001",
                format!(
                    "semantic graph has no facts for resolved type `{}`",
                    ty.identity_key()
                ),
            )
        })
}

fn facts_object(facts: &TypeFacts) -> String {
    format!(
        "{{\"copy\":{},\"contains_resource\":{},\"sized\":{},\"needs_drop\":{},\"layout_key\":{}}}",
        facts.copy,
        facts.contains_resource,
        facts.sized,
        facts.needs_drop,
        quote_json(&facts.layout_key)
    )
}

fn ownership_text(ownership: OwnershipMode) -> &'static str {
    match ownership {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}

/// Additive match-mode projection. Value is intentionally absent because it
/// was implicit in every graph through v20; emitting it would change legacy
/// module, context, and agent-context bytes.
fn explicit_match_mode_json(mode: ResolvedMatchMode) -> &'static str {
    match mode {
        ResolvedMatchMode::Value => "",
        ResolvedMatchMode::Own => ",\"ownership_mode\":\"own\"",
        ResolvedMatchMode::Borrow => ",\"ownership_mode\":\"borrow\"",
    }
}

fn unary_text(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "-",
        UnaryOp::Not => "!",
    }
}

fn binary_text(op: BinaryOp) -> &'static str {
    op.text()
}

fn string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .budgeted_join(",")
    )
}

#[cfg(test)]
#[path = "graph/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "graph/nested_owned_records_tests.rs"]
mod nested_owned_records_tests;
