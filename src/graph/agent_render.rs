//! Exact byte-budget fitting for agent-context responses (REF-10).
//!
//! A response is evaluated as constant envelope fragments around its
//! self-describing `used_bytes` value plus the already rendered, immutable
//! fact payloads. The fitter sizes candidate selections from those fragments
//! and cached fact lengths, and materializes only the response it returns.
//! The fragments are produced by the same canonical `format!` calls that
//! emit the response, so sizing and emission share every field and escaping
//! rule; `AgentResponse::materialize` re-checks the emitted length.

use super::work_counter::{record, Work};
use super::*;

/// A fact whose canonical JSON is rendered once and only borrowed afterwards.
pub(super) trait AgentFactJson {
    fn fact_json(&self) -> &str;
}

impl AgentFactJson for AgentFunctionFact {
    fn fact_json(&self) -> &str {
        &self.json
    }
}

impl AgentFactJson for AgentFunctionFactV2 {
    fn fact_json(&self) -> &str {
        &self.json
    }
}

/// Cumulative UTF-8 byte lengths of rendered facts: `prefix[k]` is the
/// payload length of the first `k` facts. Computed once per query.
pub(super) fn fact_byte_prefix(facts: &[impl AgentFactJson]) -> Vec<usize> {
    let mut prefix = Vec::with_capacity(facts.len() + 1);
    let mut total = 0_usize;
    prefix.push(total);
    for fact in facts {
        total = total.saturating_add(fact.fact_json().len());
        prefix.push(total);
    }
    prefix
}

/// One exact candidate response: `head`, the decimal `used_bytes`, `tail`,
/// the comma-joined selected facts, and the closing `]}`.
pub(super) struct AgentResponse<'f, F> {
    head: String,
    tail: String,
    facts: &'f [F],
    used_bytes: usize,
}

const RESPONSE_CLOSE: &str = "]}";

fn decimal_width(mut value: usize) -> usize {
    let mut width = 1;
    while value >= 10 {
        value /= 10;
        width += 1;
    }
    width
}

impl<'f, F: AgentFactJson> AgentResponse<'f, F> {
    /// Fix `used_bytes` exactly as the former render loop did: start at zero
    /// and re-evaluate with the previous length until it is self-consistent.
    /// Only the decimal width of `used_bytes` varies between evaluations.
    fn new(head: String, tail: String, facts: &'f [F], fact_bytes: &[usize]) -> Self {
        record(Work::ResponseSizeEvaluation, 1);
        record(Work::EnvelopeFragmentBytes, head.len() + tail.len());
        let joined_facts = fact_bytes[facts.len()].saturating_add(facts.len().saturating_sub(1));
        let fixed = head
            .len()
            .saturating_add(tail.len())
            .saturating_add(joined_facts)
            .saturating_add(RESPONSE_CLOSE.len());
        let mut used_bytes = 0;
        loop {
            let actual = fixed.saturating_add(decimal_width(used_bytes));
            if actual == used_bytes {
                break;
            }
            used_bytes = actual;
        }
        Self {
            head,
            tail,
            facts,
            used_bytes,
        }
    }

    /// The exact UTF-8 byte length `materialize` emits.
    pub(super) fn len(&self) -> usize {
        self.used_bytes
    }

    /// Emit the canonical response bytes. The allocation is charged to an
    /// active `bounded_output` budget like the `format!` it replaces; a
    /// refused reservation yields the same empty string that `format!` did.
    pub(super) fn materialize(self) -> Result<String, Diagnostic> {
        if !crate::bounded_output::reserve_active(self.used_bytes) {
            return Ok(String::new());
        }
        let mut output = String::with_capacity(self.used_bytes);
        output.push_str(&self.head);
        write!(output, "{}", self.used_bytes).expect("writing to a string cannot fail");
        output.push_str(&self.tail);
        for (index, fact) in self.facts.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push_str(fact.fact_json());
        }
        output.push_str(RESPONSE_CLOSE);
        record(Work::FullResponseMaterialization, 1);
        record(Work::MaterializedResponseBytes, output.len());
        if output.len() != self.used_bytes {
            return Err(Diagnostic::io(
                "SPX-G004",
                format!(
                    "agent context response emitted {} bytes but was sized as {}",
                    output.len(),
                    self.used_bytes
                ),
            ));
        }
        Ok(output)
    }
}

pub(super) fn individual_agent_v2_fact_fits(
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
    let fact_bytes = [0, individual.json.len()];
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
            fact_bytes: &fact_bytes,
        },
        &depth_frontier,
    )
    .len()
        <= MAX_AGENT_CONTEXT_BYTES
}

pub(super) fn individual_agent_fact_fits(
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
    let fact_bytes = [0, fact.json.len()];
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
            fact_bytes: &fact_bytes,
        },
        &direct_frontier,
    )
    .len()
        <= MAX_AGENT_CONTEXT_BYTES
}

pub(super) fn render_agent_context<'f>(
    program: &ResolvedProgram,
    source_identity: SourceGraphIdentity<'_>,
    root: &DeclarationId,
    options: &AgentContextOptions,
    facts: &'f [AgentFunctionFact],
    selection: AgentRenderSelection<'_>,
    depth_frontier: &BTreeSet<DeclarationId>,
) -> AgentResponse<'f, AgentFunctionFact> {
    let AgentRenderSelection {
        selected,
        node_limited,
        required_bytes,
        fact_bytes,
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
    let omitted_fact_bytes = fact_bytes[facts.len()] - fact_bytes[selected];
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
    let max_depth_used = facts[..selected]
        .iter()
        .map(|fact| fact.depth)
        .max()
        .unwrap_or(0);
    // Issue #206: envelope-level, declaration-independent reference data --
    // see `session_protocol_facet`'s module doc for exactly what this is and
    // is not a fact about. Present only when selected (like every other
    // filter-gated field in this envelope), so an unrelated query's bytes
    // are unaffected -- following the same optional-leading-comma-fragment
    // idiom `portable_indexed_byte_data_json` already uses in `graph`'s own
    // header.
    let session_protocol_kernel = if options
        .filters
        .contains(&AgentContextFilter::SessionProtocol)
    {
        format!(
            ",\"session_protocol_kernel\":{}",
            session_protocol_facet::summary_catalog_json(
                &options.declared_session_protocols,
                &options.follows_bindings,
            )
        )
    } else {
        String::new()
    };
    let head = format!(
        "{{\"schema\":\"semaprax.agent-context.v1\",\"source_graph_schema\":{},\"revision\":{},\"prelude\":{{\"schema\":{},\"digest\":{}}},\"module\":{},\"root\":{},\"query\":{{\"depth\":{},\"max_bytes\":{},\"max_nodes\":{},\"filters\":[{}]}},\"filter_support\":{{\"included\":[{}],\"unavailable\":[{}]}},\"budget\":{{\"used_bytes\":",
        quote_json(source_identity.schema),
        quote_json(source_identity.revision),
        quote_json(prelude_binding::schema(program)),
        quote_json(&prelude_binding::digest(program)),
        quote_json(&program.module),
        quote_json(root.as_str()),
        options.depth,
        options.max_bytes,
        options.max_nodes,
        requested,
        included,
        unavailable,
    );
    let tail = format!(
        ",\"used_nodes\":{},\"max_depth_used\":{}}},\"truncation\":{{\"truncated\":{},\"reasons\":[{}],\"omitted_known_nodes\":{},\"deferred_known_nodes\":{},\"omitted_fact_bytes\":{},\"unavailable_filter_count\":{}}},\"resume_contract\":{{\"depth\":\"query.depth\",\"max_nodes\":\"query.max_nodes\",\"filters\":\"query.filters\",\"max_bytes\":\"frontier.resume.min_bytes\"}}{},\"frontier\":[{}],\"facts\":[",
        selected,
        max_depth_used,
        !reasons.is_empty() || unavailable_count != 0,
        reason_json,
        omitted_known.len(),
        omitted_known.len().saturating_sub(frontier.len()),
        omitted_fact_bytes,
        unavailable_count,
        session_protocol_kernel,
        frontier_json,
    );
    AgentResponse::new(head, tail, &facts[..selected], fact_bytes)
}

pub(super) fn render_agent_context_v2<'f>(
    program: &ResolvedProgram,
    source_identity: SourceGraphIdentity<'_>,
    root: &DeclarationId,
    options: &AgentContextV2Options,
    facts: &'f [AgentFunctionFactV2],
    selection: AgentRenderSelectionV2<'_>,
    depth_frontier: &BTreeMap<DeclarationId, BTreeSet<AgentContextDirection>>,
) -> AgentResponse<'f, AgentFunctionFactV2> {
    let AgentRenderSelectionV2 {
        selected,
        node_limited,
        required_bytes,
        fact_bytes,
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
    let omitted_fact_bytes = fact_bytes[facts.len()] - fact_bytes[selected];
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
    let max_depth_used = facts[..selected]
        .iter()
        .map(|fact| fact.depth)
        .max()
        .unwrap_or(0);
    let deferred_traversal = omitted_traversal.len().saturating_sub(frontier.len());
    // Issue #206: envelope-level, declaration-independent reference data --
    // see `session_protocol_facet`'s module doc for exactly what this is and
    // is not a fact about. Present only when selected -- see the matching
    // v1 comment above for why.
    let session_protocol_kernel = if options
        .base
        .filters
        .contains(&AgentContextFilter::SessionProtocol)
    {
        format!(
            ",\"session_protocol_kernel\":{}",
            session_protocol_facet::summary_catalog_json(
                &options.base.declared_session_protocols,
                &options.base.follows_bindings,
            )
        )
    } else {
        String::new()
    };
    let head = format!(
        "{{\"schema\":\"semaprax.agent-context.v2\",\"source_graph_schema\":{},\"revision\":{},\"prelude\":{{\"schema\":{},\"digest\":{}}},\"module\":{},\"root\":{},\"query\":{{\"direction\":{},\"depth\":{},\"max_bytes\":{},\"max_nodes\":{},\"filters\":[{}]}},\"filter_support\":{{\"included\":[{}],\"unavailable\":[{}]}},\"budget\":{{\"used_bytes\":",
        quote_json(source_identity.schema),
        quote_json(source_identity.revision),
        quote_json(prelude_binding::schema(program)),
        quote_json(&prelude_binding::digest(program)),
        quote_json(&program.module),
        quote_json(root.as_str()),
        quote_json(options.direction().name()),
        options.depth(),
        options.max_bytes(),
        options.max_nodes(),
        requested,
        included,
        unavailable,
    );
    let tail = format!(
        ",\"used_nodes\":{},\"max_depth_used\":{}}},\"truncation\":{{\"truncated\":{},\"reasons\":[{}],\"omitted_known_nodes\":{},\"deferred_known_nodes\":{},\"omitted_fact_bytes\":{},\"unavailable_filter_count\":{}}},\"reference_closure\":{{\"referenced_unselected_nodes\":{}}},\"resume_contract\":{{\"direction\":\"query.direction\",\"depth\":\"query.depth\",\"max_nodes\":\"query.max_nodes\",\"filters\":\"query.filters\",\"max_bytes\":{{\"traversal\":\"frontier.resume.min_bytes\",\"reference\":\"reference_frontier.resume.min_bytes\"}}}}{},\"frontier\":[{}],\"reference_frontier\":[{}],\"facts\":[",
        selected,
        max_depth_used,
        !reasons.is_empty() || unavailable_count != 0,
        ordered_agent_reasons(&reasons),
        omitted_traversal.len(),
        deferred_traversal,
        omitted_fact_bytes,
        unavailable_count,
        reference_frontier.len(),
        session_protocol_kernel,
        frontier_json,
        reference_frontier_json,
    );
    AgentResponse::new(head, tail, &facts[..selected], fact_bytes)
}
