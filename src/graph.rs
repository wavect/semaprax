//! Deterministic semantic graph serialization and bounded context queries.
//!
//! Human source supplies the revision. Resolved HIR supplies every semantic
//! identity and fact in graph v6; spans and display names are metadata only.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write;

use sha2::{Digest, Sha256};

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::format;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, OwnershipMode, Place, PlaceProjection, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedImportFailure, ResolvedProgram,
    ResolvedResourceDropKind, ResolvedStatement, ResolvedType, ResolvedTypeDeclarationKind,
    TypeFacts, ValueId,
};

/// Hash the canonical human-readable source projection.
///
/// This revision intentionally does not depend on HIR spans, display metadata,
/// or the graph wire format. Semantic transactions therefore remain bound to
/// the exact canonical source meaning that a human can review in Git.
pub fn revision(program: &Program) -> String {
    let source = format::canonical(program);
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.graph-revision.v1\0");
    hasher.update(source.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

/// Resolve and serialize a parsed program as `semaprax.graph.v6`.
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
    let revision = revision(program);
    let resolved = hir::resolve(program)?;
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

    const fn supported_by_graph_v6(self) -> bool {
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
    let source_revision = revision(program);
    let resolved = hir::resolve(program)?;
    agent_context_hir_json(&resolved, &source_revision, symbol, options)
        .map_err(|diagnostic| vec![diagnostic])
}

#[derive(Clone)]
struct AgentFunctionFact {
    id: DeclarationId,
    depth: usize,
    calls: BTreeSet<DeclarationId>,
    json: String,
}

struct AgentRenderSelection<'a> {
    selected: usize,
    node_limited: usize,
    required_bytes: &'a BTreeMap<DeclarationId, usize>,
}

fn agent_context_hir_json(
    program: &ResolvedProgram,
    source_revision: &str,
    symbol: &str,
    options: &AgentContextOptions,
) -> Result<Option<String>, Diagnostic> {
    hir::validate(program)?;
    let Some(root) = find_context_root(program, symbol) else {
        return Ok(None);
    };
    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::from([root.clone()]);
    let mut queue = VecDeque::from([(root.clone(), 0_usize)]);
    let mut ordered = Vec::new();
    let mut depth_frontier = BTreeSet::new();
    while let Some((function_id, current_depth)) = queue.pop_front() {
        let function = functions
            .get(&function_id)
            .copied()
            .ok_or_else(|| graph_reference_error("function", &function_id))?;
        ordered.push((function_id.clone(), current_depth));
        let calls = function_calls(function);
        for callee in calls {
            if !functions.contains_key(&callee) {
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
        let function = functions
            .get(&id)
            .copied()
            .ok_or_else(|| graph_reference_error("function", &id))?;
        facts.push(AgentFunctionFact {
            id,
            depth,
            calls: agent_function_calls(program, function),
            json: agent_function_json(program, function, &options.filters)?,
        });
    }

    let node_limited = facts.len().min(options.max_nodes);
    let mut selected = node_limited;
    let mut required_bytes = BTreeMap::new();
    loop {
        let output = render_agent_context(
            program,
            source_revision,
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
            source_revision,
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

fn individual_agent_fact_fits(
    program: &ResolvedProgram,
    source_revision: &str,
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
        source_revision,
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
        .or_else(|| program.declarations.function_id(symbol).cloned())
}

fn function_calls(function: &ResolvedFunction) -> BTreeSet<DeclarationId> {
    let mut calls = BTreeSet::new();
    visit_function_calls(function, &mut |callee| {
        calls.insert(callee.clone());
    });
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
    if filters.contains(&AgentContextFilter::Contracts) {
        write!(
            output,
            ",\"contracts\":{{\"requires\":[{}],\"ensures\":[{}]}}",
            function
                .requires
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .join(","),
            function
                .ensures
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
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
                .join(","),
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
        close_record_type_declarations(program, &mut selected_types)?;
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
                .join(","),
            quote_json(&function.return_type.identity_key()),
            type_facts_array(program, &selected_functions, &selected_types)?,
            agent_type_declarations_json(program, &selected_types)?
        )
        .expect("writing to a string cannot fail");
    }
    output.push('}');
    Ok(output)
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
    close_record_type_declarations(program, &mut declarations)?;
    Ok(format!(
        "{{\"values\":[{}],\"declarations\":[{}]}}",
        values
            .iter()
            .map(|id| quote_json(id.as_str()))
            .collect::<Vec<_>>()
            .join(","),
        agent_type_declarations_json(program, &declarations)?
    ))
}

fn collect_agent_contract_values(expression: &ResolvedExpr, values: &mut BTreeSet<ValueId>) {
    match &expression.kind {
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) => {}
        ResolvedExprKind::Place(place) => {
            values.insert(place.root.clone());
        }
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
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
                let ResolvedStatement::Let { binding, value, .. } = statement;
                values.insert(binding.id.clone());
                collect_agent_contract_values(value, values);
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
        ResolvedExprKind::Project { base, .. } => collect_agent_contract_values(base, values),
    }
}

fn agent_contract_expr_json(expression: &ResolvedExpr) -> Result<String, Diagnostic> {
    Ok(match &expression.kind {
        ResolvedExprKind::Int(value) => format!(
            "{{\"kind\":\"int\",\"value\":{}}}",
            quote_json(&value.to_string())
        ),
        ResolvedExprKind::Bool(value) => format!("{{\"kind\":\"bool\",\"value\":{value}}}"),
        ResolvedExprKind::Place(place) => {
            format!("{{\"kind\":\"place\",\"place\":{}}}", place_json(place))
        }
        ResolvedExprKind::Call { callee, args } => format!(
            "{{\"kind\":\"call\",\"callee\":{},\"args\":[{}]}}",
            quote_json(callee.as_str()),
            args.iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
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
                    let ResolvedStatement::Let { binding, value, .. } = statement;
                    Ok(format!(
                        "{{\"kind\":\"let\",\"binding\":{},\"value\":{}}}",
                        quote_json(binding.id.as_str()),
                        agent_contract_expr_json(value)?
                    ))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .join(","),
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
        ResolvedExprKind::ConstructRecord { record, fields } => format!(
            "{{\"kind\":\"construct_record\",\"record\":{},\"fields\":[{}]}}",
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
                .join(",")
        ),
        ResolvedExprKind::Project { base, field } => format!(
            "{{\"kind\":\"project\",\"base\":{},\"field\":{}}}",
            agent_contract_expr_json(base)?,
            quote_json(field.as_str())
        ),
    })
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
            ResolvedTypeDeclarationKind::Record { fields } => Ok(format!(
                "{{\"id\":{},\"kind\":\"record\",\"fields\":[{}]}}",
                quote_json(declaration.id.as_str()),
                fields
                    .iter()
                    .map(|field| format!(
                        "{{\"id\":{},\"type_id\":{}}}",
                        quote_json(field.id.as_str()),
                        quote_json(&field.ty.identity_key())
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )),
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|items| items.join(","))
}

fn render_agent_context(
    program: &ResolvedProgram,
    source_revision: &str,
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
        .filter(|filter| !filter.supported_by_graph_v6())
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
        .join(",");
    let included = options
        .filters
        .iter()
        .filter(|filter| filter.supported_by_graph_v6())
        .map(|filter| quote_json(filter.name()))
        .collect::<Vec<_>>()
        .join(",");
    let unavailable = options
        .filters
        .iter()
        .filter(|filter| !filter.supported_by_graph_v6())
        .map(|filter| quote_json(filter.name()))
        .collect::<Vec<_>>()
        .join(",");
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
                    .join(","),
                quote_json(resume_symbol.as_str()),
                quote_json(id.as_str()),
                resume_bytes
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let reason_json = reasons
        .iter()
        .map(|reason| quote_json(reason))
        .collect::<Vec<_>>()
        .join(",");
    let facts_json = facts[..selected]
        .iter()
        .map(|fact| fact.json.as_str())
        .collect::<Vec<_>>()
        .join(",");
    let max_depth_used = facts[..selected]
        .iter()
        .map(|fact| fact.depth)
        .max()
        .unwrap_or(0);
    let render = |used_bytes: usize| {
        format!(
            "{{\"schema\":\"semaprax.agent-context.v1\",\"source_graph_schema\":\"semaprax.graph.v6\",\"revision\":{},\"module\":{},\"root\":{},\"query\":{{\"depth\":{},\"max_bytes\":{},\"max_nodes\":{},\"filters\":[{}]}},\"filter_support\":{{\"included\":[{}],\"unavailable\":[{}]}},\"budget\":{{\"used_bytes\":{},\"used_nodes\":{},\"max_depth_used\":{}}},\"truncation\":{{\"truncated\":{},\"reasons\":[{}],\"omitted_known_nodes\":{},\"deferred_known_nodes\":{},\"omitted_fact_bytes\":{},\"unavailable_filter_count\":{}}},\"resume_contract\":{{\"depth\":\"query.depth\",\"max_nodes\":\"query.max_nodes\",\"filters\":\"query.filters\",\"max_bytes\":\"frontier.resume.min_bytes\"}},\"frontier\":[{}],\"facts\":[{}]}}",
            quote_json(source_revision),
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

fn agent_context_option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-G004", message)
}

fn to_hir_json(program: &ResolvedProgram, source_revision: &str) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    let selected_functions = program
        .functions
        .iter()
        .map(|function| function.id.clone())
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

    // Exact declaration identity is authoritative if another function's
    // display name happens to contain the same text.
    let root = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == symbol)
        .map(|function| function.id.clone())
        .or_else(|| program.declarations.function_id(symbol).cloned());
    let Some(root) = root else {
        return Ok(None);
    };

    let functions = program
        .functions
        .iter()
        .map(|function| (function.id.clone(), function))
        .collect::<BTreeMap<_, _>>();
    let mut selected = BTreeSet::from([root.clone()]);
    let mut queue = VecDeque::from([(root.clone(), 0_usize)]);
    while let Some((function_id, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }
        if let Some(function) = functions.get(&function_id) {
            visit_function_calls(function, &mut |callee| {
                if functions.contains_key(callee) && selected.insert(callee.clone()) {
                    queue.push_back((callee.clone(), current_depth + 1));
                }
            });
        }
    }

    let mut selected_types = BTreeSet::new();
    for function in &program.functions {
        if selected.contains(&function.id) {
            collect_function_type_declarations(function, &mut selected_types);
        }
    }
    close_record_type_declarations(program, &mut selected_types)?;

    let mut frontier = BTreeSet::new();
    for function in &program.functions {
        if !selected.contains(&function.id) {
            continue;
        }
        visit_function_calls(function, &mut |callee| {
            if functions.contains_key(callee) && !selected.contains(callee) {
                frontier.insert(callee.clone());
            }
        });
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
        close_record_type_declarations(program, &mut selected_types)?;
        let referenced_imports = program
            .types
            .iter()
            .filter(|declaration| selected_types.contains(&declaration.id))
            .filter_map(|declaration| match &declaration.kind {
                ResolvedTypeDeclarationKind::Resource { drop } => match &drop.kind {
                    ResolvedResourceDropKind::Imported { import, .. } => Some(import.clone()),
                    ResolvedResourceDropKind::Trivial => None,
                },
                ResolvedTypeDeclarationKind::Record { .. } => None,
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
    close_record_type_declarations(program, &mut selected_types)?;
    let mut output = String::new();
    write!(
        output,
        "{{\"schema\":\"semaprax.graph.v6\",\"revision\":{},\"view\":{},\"identity\":{{\"declarations\":\"explicit-persistent-or-automatic-unstable\",\"values\":\"revision-scoped-structural\",\"expressions\":\"revision-scoped-structural\"}},\"module\":{},\"permits\":{},\"entrypoint\":{},\"type_facts\":[{}],\"nodes\":[",
        quote_json(source_revision),
        view_json(view),
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
                write!(
                    output,
                    "{{\"id\":{},\"kind\":\"record\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"type_id\":{},\"fields\":[{}]}}",
                    quote_json(declaration.id.as_str()),
                    quote_json(&declaration.name),
                    quote_json(type_origin.text()),
                    type_origin.is_persistent(),
                    quote_json(&ty.identity_key()),
                    fields
                        .iter()
                        .map(|field| quote_json(field.id.as_str()))
                        .collect::<Vec<_>>()
                        .join(",")
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
                .join(",")
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
                .join(",");
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
                "{{\"id\":{},\"kind\":\"import\",\"name\":{},\"owner\":{},\"identity_origin\":{},\"persistent\":{},\"import_key\":{},\"parameters\":[{}],\"result\":{{\"type\":\"unit\",\"ownership_mode\":\"value\",\"producer\":{},\"out_slot_initialization\":{},\"ownership_transfer\":{}}},\"effects\":{},\"required_authority\":{},\"failure\":{}}}",
                quote_json(import.id.as_str()),
                quote_json(&import.name),
                quote_json(interface.id.as_str()),
                quote_json(origin.text()),
                origin.is_persistent(),
                quote_json(&import.import_key),
                parameters,
                quote_json(import.result.producer),
                quote_json(import.result.out_slot_initialization),
                quote_json(import.result.ownership_transfer),
                string_array(&import.effects),
                string_array(&import.required_authority),
                failure
            )
            .expect("writing to a string cannot fail");
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
                Ok(format!(
                    "{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}",
                    quote_json(param.id.as_str()),
                    quote_json(&param.name),
                    quote_json(&param.ty.identity_key()),
                    quote_json(ownership_text(param.ownership))
                ))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?
            .join(",");
        let requires = function
            .requires
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let ensures = function
            .ensures
            .iter()
            .map(|expression| expr_json(program, expression))
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        let body = expr_json(program, &function.body)?;
        let cleanup = crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan);
        let identity_origin = identity_origin(program, &function.id)?;
        let result_ownership = result_ownership(program, &function.return_type)?;
        let calls = calls.into_iter().collect::<Vec<_>>();

        write!(
            output,
            "{{\"id\":{},\"kind\":\"function\",\"name\":{},\"identity_origin\":{},\"persistent\":{},\"params\":[{}],\"result_id\":{},\"result\":{{\"id\":{},\"type_id\":{},\"ownership_mode\":{}}},\"return_type_id\":{},\"effects\":{},\"requires_graph\":[{}],\"ensures_graph\":[{}],\"calls\":{},\"body\":{},\"cleanup\":{}}}",
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
    }
    output.push_str("]}");
    Ok(output)
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
                .join(",")
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

fn visit_expr_calls(expression: &ResolvedExpr, visit: &mut impl FnMut(&DeclarationId)) {
    match &expression.kind {
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        ResolvedExprKind::Call { callee, args } => {
            visit(callee);
            for argument in args {
                visit_expr_calls(argument, visit);
            }
        }
        ResolvedExprKind::Unary { value, .. } => visit_expr_calls(value, visit),
        ResolvedExprKind::Binary { left, right, .. } => {
            visit_expr_calls(left, visit);
            visit_expr_calls(right, visit);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { value, .. } = statement;
                visit_expr_calls(value, visit);
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
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expr_type_declarations(argument, declarations);
            }
        }
        ResolvedExprKind::Unary { value, .. } => {
            collect_expr_type_declarations(value, declarations);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr_type_declarations(left, declarations);
            collect_expr_type_declarations(right, declarations);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                collect_nominal_declarations(&binding.ty, declarations);
                collect_expr_type_declarations(value, declarations);
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
        ResolvedExprKind::Project { base, .. } => {
            collect_expr_type_declarations(base, declarations);
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

fn close_record_type_declarations(
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
        if let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind {
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
        ResolvedExprKind::Bool(value) => {
            format!("{{{header},\"kind\":\"bool\",\"value\":{value}}}")
        }
        ResolvedExprKind::Place(place) => format!(
            "{{{header},\"kind\":\"place\",\"place\":{}}}",
            place_json(place)
        ),
        ResolvedExprKind::Call { callee, args } => format!(
            "{{{header},\"kind\":\"call\",\"callee\":{},\"args\":[{}]}}",
            quote_json(callee.as_str()),
            args.iter()
                .map(|argument| expr_json(program, argument))
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
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
                .join(","),
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
        ResolvedExprKind::ConstructRecord { record, fields } => format!(
            "{{{header},\"kind\":\"construct_record\",\"record\":{},\"fields\":[{}]}}",
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
                .join(",")
        ),
        ResolvedExprKind::Project { base, field } => format!(
            "{{{header},\"kind\":\"project\",\"base\":{},\"field\":{}}}",
            expr_json(program, base)?,
            quote_json(field.as_str())
        ),
    };
    Ok(output)
}

fn statement_json(
    program: &ResolvedProgram,
    statement: &ResolvedStatement,
) -> Result<String, Diagnostic> {
    match statement {
        ResolvedStatement::Let { binding, value, .. } => Ok(format!(
            "{{\"kind\":\"let\",\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}},\"value\":{}}}",
            quote_json(binding.id.as_str()),
            quote_json(&binding.name),
            quote_json(&binding.ty.identity_key()),
            quote_json(ownership_text(binding.ownership)),
            expr_json(program, value)?
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
            .join(",")
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
        collect_type(
            &ResolvedType::Nominal {
                declaration: declaration.id.clone(),
                arguments: Vec::new(),
            },
            &mut types,
        );
        if let ResolvedTypeDeclarationKind::Record { fields } = &declaration.kind {
            for field in fields {
                collect_type(&field.ty, &mut types);
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
        .map(|items| items.join(","))
}

fn collect_expr_types(expression: &ResolvedExpr, types: &mut BTreeMap<String, ResolvedType>) {
    collect_type(&expression.ty, types);
    match &expression.kind {
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
        ResolvedExprKind::Call { args, .. } => {
            for argument in args {
                collect_expr_types(argument, types);
            }
        }
        ResolvedExprKind::Unary { value, .. } => collect_expr_types(value, types),
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr_types(left, types);
            collect_expr_types(right, types);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                collect_type(&binding.ty, types);
                collect_expr_types(value, types);
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
        ResolvedExprKind::Project { base, .. } => collect_expr_types(base, types),
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
        ResolvedType::I64 => "{\"kind\":\"primitive\",\"name\":\"i64\"}".to_owned(),
        ResolvedType::Bool => "{\"kind\":\"primitive\",\"name\":\"bool\"}".to_owned(),
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
                .join(",")
        ),
    }
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
            .join(",")
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{to_hir_json, DeclarationId, ResolvedExprKind, ResolvedProgram};
    use crate::{hir, parse};

    fn resolved_program() -> ResolvedProgram {
        let source = r#"
module test.graph_hir;
@id("app.main")
fn main() -> i64 { 42 }
"#;
        hir::resolve(&parse(source, Path::new("graph-hir.spx")).unwrap()).unwrap()
    }

    fn resolved_record_program() -> ResolvedProgram {
        let source = r#"
module test.graph_record_hir;
@id("geometry.point")
record Point { @id("geometry.point.x") x: i64, }
@id("app.main")
fn main() -> i64 { Point { x: 42 }.x }
"#;
        hir::resolve(&parse(source, Path::new("graph-record-hir.spx")).unwrap()).unwrap()
    }

    fn resolved_resource_program() -> ResolvedProgram {
        let source = r#"
module test.graph_resource_hir;
@id("token.type")
resource Token { @id("token.drop") drop trivial; }
@id("token.discard")
fn discard(token: own Token) -> i64 { 0 }
@id("app.main")
fn main() -> i64 { 0 }
"#;
        hir::resolve(&parse(source, Path::new("graph-resource-hir.spx")).unwrap()).unwrap()
    }

    #[test]
    fn internal_hir_renderer_revalidates_before_serializing() {
        let mut program = resolved_program();
        program.entrypoint = hir::DeclarationId::new("missing.entrypoint");
        assert_eq!(
            to_hir_json(&program, "trusted-source-revision")
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }

    #[test]
    fn internal_hir_renderer_rejects_nul_identity_before_serializing() {
        let mut program = resolved_program();
        program.functions[0].body.ty = hir::ResolvedType::Nominal {
            declaration: hir::DeclarationId::new("type\0forged"),
            arguments: Vec::new(),
        };
        let diagnostic = to_hir_json(&program, "trusted-source-revision").unwrap_err();
        assert_eq!(diagnostic.code, "SPX-H006");
        assert!(diagnostic.message.contains("contains NUL"));
    }

    #[test]
    fn internal_hir_renderer_rejects_nul_cleanup_reference_before_serializing() {
        let mut program = resolved_resource_program();
        let discard = program
            .functions
            .iter_mut()
            .find(|function| function.name == "discard")
            .unwrap();
        let finalizer = discard
            .cleanup_plan
            .exits
            .iter_mut()
            .find_map(|exit| exit.finalize_in_order.first_mut())
            .expect("discard must finalize its parameter");
        finalizer.lifecycle_id = hir::DeclarationId::new("token.drop\0forged");

        let diagnostic = to_hir_json(&program, "trusted-source-revision").unwrap_err();
        assert_eq!(diagnostic.code, "SPX-H006");
        assert!(diagnostic.message.contains("contains NUL"));
    }

    #[test]
    fn internal_hir_renderer_preserves_its_trusted_source_revision() {
        let graph = to_hir_json(&resolved_program(), "trusted-source-revision").unwrap();
        assert!(graph.contains("\"revision\":\"trusted-source-revision\""));
    }

    #[test]
    fn internal_hir_renderer_rejects_a_foreign_record_field_reference() {
        let mut program = resolved_record_program();
        let ResolvedExprKind::Block { tail, .. } = &mut program.functions[0].body.kind else {
            panic!("function body should be a block");
        };
        let ResolvedExprKind::Project { base, .. } = &mut tail.kind else {
            panic!("function tail should be a temporary projection");
        };
        let ResolvedExprKind::ConstructRecord { fields, .. } = &mut base.kind else {
            panic!("projection base should be a record constructor");
        };
        fields[0].field = DeclarationId::new("foreign.field");

        assert_eq!(
            to_hir_json(&program, "trusted-source-revision")
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
}
