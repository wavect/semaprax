//! Deterministic, read-only Semantic Impact v1 previews.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::call_index::{PersistentCallIndex, PersistentCallableKind};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::graph;
use crate::hir::{self, DeclarationId, IdentityOrigin, ResolvedProgram, ResolvedType};
use crate::patch::{
    self, PatchPreflight, PreflightChange, PreflightOperation, SourceConsumerKey,
    SourceConsumerRole,
};

const DEFAULT_DEPTH: usize = 1;
const DEFAULT_MAX_BYTES: usize = 64 * 1024;
const DEFAULT_MAX_NODES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticImpactOptions {
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
}

impl SemanticImpactOptions {
    pub fn new(depth: usize, max_bytes: usize, max_nodes: usize) -> Result<Self, Diagnostic> {
        if depth > graph::MAX_AGENT_CONTEXT_DEPTH {
            return Err(impact_option_error(format!(
                "semantic impact depth {depth} exceeds maximum {}",
                graph::MAX_AGENT_CONTEXT_DEPTH
            )));
        }
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(impact_option_error(format!(
                "semantic impact max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        if !(1..=graph::MAX_AGENT_CONTEXT_NODES).contains(&max_nodes) {
            return Err(impact_option_error(format!(
                "semantic impact max_nodes must be between 1 and {}",
                graph::MAX_AGENT_CONTEXT_NODES
            )));
        }
        Ok(Self {
            depth,
            max_bytes,
            max_nodes,
        })
    }
}

impl Default for SemanticImpactOptions {
    fn default() -> Self {
        Self {
            depth: DEFAULT_DEPTH,
            max_bytes: DEFAULT_MAX_BYTES,
            max_nodes: DEFAULT_MAX_NODES,
        }
    }
}

pub fn preview(
    source_path: &Path,
    patch_path: &Path,
    options: &SemanticImpactOptions,
) -> Result<String, Vec<Diagnostic>> {
    preview_with_hook(source_path, patch_path, options, |_, _, _| Ok(()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreviewPhase {
    AfterPatchRead,
    BeforeFinalCheck,
}

fn preview_with_hook(
    source_path: &Path,
    patch_path: &Path,
    options: &SemanticImpactOptions,
    mut hook: impl FnMut(PreviewPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let patch_source = std::fs::read_to_string(patch_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("cannot read {}: {error}", patch_path.display()),
        )]
    })?;
    hook(
        PreviewPhase::AfterPatchRead,
        &canonical_source_path,
        patch_path,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("semantic impact snapshot hook failed: {error}"),
        )]
    })?;
    let preflight = patch::preflight_impact_owned(
        snapshot.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
    )?;
    if preflight.source() != snapshot.source() {
        return Err(vec![impact_invariant_error(
            "semantic impact preflight source differs from its authenticated snapshot",
        )]);
    }
    let report = build_report(&preflight, options)?.json;
    hook(
        PreviewPhase::BeforeFinalCheck,
        &canonical_source_path,
        patch_path,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("semantic impact final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged(
        &canonical_source_path,
        source_path,
        &snapshot,
        preflight.base_revision(),
    )?;
    Ok(report)
}

#[derive(Clone)]
struct AffectedFunction {
    id: DeclarationId,
    kind: PersistentCallableKind,
    depth: usize,
    operation_indices: BTreeSet<usize>,
}

struct ConsumerFact {
    key: SourceConsumerKey,
    identity_origin: &'static str,
    roles: BTreeSet<SourceConsumerRole>,
    site_count: usize,
}

struct BuiltChanges {
    json: String,
    seeds: BTreeMap<DeclarationId, BTreeSet<usize>>,
}

struct BuiltImpactReport {
    json: String,
    truncated: bool,
    omitted: usize,
    deferred: usize,
    frontier_empty: bool,
    used_depth: usize,
    used_nodes: usize,
}

fn build_report(
    preflight: &PatchPreflight,
    options: &SemanticImpactOptions,
) -> Result<BuiltImpactReport, Vec<Diagnostic>> {
    let before = hir::resolve(preflight.before())?;
    let candidate = hir::resolve(preflight.candidate())?;
    hir::validate(&before).map_err(|error| vec![error])?;
    hir::validate(&candidate).map_err(|error| vec![error])?;
    let base_schema = graph::graph_schema(&before);
    let candidate_schema = graph::graph_schema(&candidate);
    if base_schema != candidate_schema {
        return Err(vec![impact_invariant_error(format!(
            "semantic impact base graph schema `{base_schema}` differs from candidate `{candidate_schema}`"
        ))]);
    }
    let call_index = PersistentCallIndex::build(&before)
        .map_err(|error| vec![impact_invariant_error(error.message)])?;
    let built_changes = changes_json(preflight, &before, &call_index)?;
    let all_affected = reverse_closure(&built_changes.seeds, &call_index)?;
    let operations = operations_json(preflight.operations());
    let patch_digest = patch_digest(preflight.patch_source());

    let within_depth = all_affected
        .iter()
        .take_while(|fact| fact.depth <= options.depth)
        .cloned()
        .collect::<Vec<_>>();
    let node_selected = within_depth.len().min(options.max_nodes);
    render_with_budget(RenderInputs {
        preflight,
        options,
        source_graph_schema: base_schema,
        patch_digest: &patch_digest,
        operations: &operations,
        changes: &built_changes.json,
        all_affected: &all_affected,
        within_depth: &within_depth,
        node_selected,
    })
}

pub(crate) struct CompleteImpactEvidence {
    report: String,
    used_depth: usize,
    used_nodes: usize,
}

impl CompleteImpactEvidence {
    pub(crate) fn report(&self) -> &str {
        &self.report
    }

    pub(crate) fn used_depth(&self) -> usize {
        self.used_depth
    }

    pub(crate) fn used_nodes(&self) -> usize {
        self.used_nodes
    }
}

pub(crate) fn complete_review_evidence(
    preflight: &PatchPreflight,
) -> Result<CompleteImpactEvidence, Vec<Diagnostic>> {
    let options =
        SemanticImpactOptions::new(1024, 16 * 1024 * 1024, 1024).map_err(|error| vec![error])?;
    let report = build_report(preflight, &options)?;
    if report.truncated || report.omitted != 0 || report.deferred != 0 || !report.frontier_empty {
        return Err(vec![Diagnostic::io(
            "SPX-G120",
            "semantic review requires complete, nontruncated Semantic Impact v1 evidence",
        )]);
    }
    Ok(CompleteImpactEvidence {
        report: report.json,
        used_depth: report.used_depth,
        used_nodes: report.used_nodes,
    })
}

fn operations_json(operations: &[PreflightOperation]) -> String {
    operations
        .iter()
        .map(|operation| match operation {
            PreflightOperation::AssignFunctionId {
                index,
                repair_id,
                target,
                name,
                to,
            } => {
                let _ = (index, repair_id, target, name, to);
                unreachable!("Impact v1 rejects Patch v3 before report construction")
            }
            PreflightOperation::Rename { index, target, to } => format!(
                "{{\"index\":{index},\"kind\":\"rename\",\"target\":{},\"to\":{}}}",
                quote_json(target),
                quote_json(to)
            ),
            PreflightOperation::RenameMember {
                index,
                owner,
                member,
                to,
            } => format!(
                "{{\"index\":{index},\"kind\":\"rename_member\",\"owner\":{},\"member\":{},\"to\":{}}}",
                quote_json(owner),
                quote_json(member),
                quote_json(to)
            ),
            PreflightOperation::RenameCase {
                index,
                owner,
                case,
                to,
            } => format!(
                "{{\"index\":{index},\"kind\":\"rename_case\",\"owner\":{},\"case\":{},\"to\":{}}}",
                quote_json(owner),
                quote_json(case),
                quote_json(to)
            ),
            PreflightOperation::ReplaceCallTypeArgument {
                index,
                expression,
                template,
                old_instance,
                argument_index,
                from,
                to,
            } => format!(
                "{{\"index\":{index},\"kind\":\"replace_call_type_argument\",\"expression\":{},\"template\":{},\"old_instance\":{},\"argument_index\":{argument_index},\"from\":{},\"to\":{}}}",
                quote_json(expression),
                quote_json(template),
                quote_json(old_instance),
                quote_json(from.text()),
                quote_json(to.text())
            ),
            PreflightOperation::RequireNoNewEffects { index } => {
                format!("{{\"index\":{index},\"kind\":\"require_no_new_effects\"}}")
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn changes_json(
    preflight: &PatchPreflight,
    before: &ResolvedProgram,
    call_index: &PersistentCallIndex,
) -> Result<BuiltChanges, Vec<Diagnostic>> {
    let mut output = Vec::new();
    let mut seeds = BTreeMap::<DeclarationId, BTreeSet<usize>>::new();
    let consumers_by_change = consumers_by_change(preflight, before)?;
    for (change_index, change) in preflight.changes().iter().enumerate() {
        let consumers_json = consumers_by_change[change_index]
            .iter()
            .map(consumer_json)
            .collect::<Vec<_>>()
            .join(",");
        match change {
            PreflightChange::Rename {
                target,
                target_kind,
                before,
                after,
                operation_indices,
            } => output.push(format!(
                "{{\"kind\":\"rename\",\"target\":{},\"target_kind\":{},\"before\":{},\"after\":{},\"classification\":\"source_projection\",\"operation_indices\":{},\"source_consumers\":[{}]}}",
                quote_json(target),
                quote_json(target_kind.text()),
                quote_json(before),
                quote_json(after),
                usize_array(operation_indices),
                consumers_json
            )),
            PreflightChange::CallInstance {
                expression,
                template,
                before_arguments,
                after_arguments,
                before_instance,
                after_instance,
                operation_indices,
            } => {
                let site = call_index.site(expression).ok_or_else(|| {
                    vec![impact_invariant_error(format!(
                        "semantic impact has no exact HIR owner for call expression `{expression}`"
                    ))]
                })?;
                if site.owner_origin != IdentityOrigin::Explicit
                    || site.callee.as_str() != template
                    || site.type_arguments != *before_arguments
                    || site.instance.as_ref().map(|value| value.as_str())
                        != Some(before_instance.as_str())
                {
                    return Err(vec![impact_invariant_error(format!(
                        "semantic impact call expression `{expression}` owner or selector is not exact persistent HIR"
                    ))]);
                }
                seeds
                    .entry(site.owner.clone())
                    .or_default()
                    .extend(operation_indices.iter().copied());
                output.push(format!(
                    "{{\"kind\":\"call_instance\",\"expression\":{},\"containing_function\":{},\"containing_kind\":{},\"template\":{},\"before_type_arguments\":{},\"after_type_arguments\":{},\"before_instance\":{},\"after_instance\":{},\"classification\":\"behavioral_call_instance\",\"operation_indices\":{},\"source_consumers\":[{}]}}",
                    quote_json(expression),
                    quote_json(site.owner.as_str()),
                    quote_json(site.owner_kind.text()),
                    quote_json(template),
                    type_array(before_arguments),
                    type_array(after_arguments),
                    quote_json(before_instance),
                    quote_json(after_instance),
                    usize_array(operation_indices),
                    consumers_json
                ));
            }
        }
    }
    Ok(BuiltChanges {
        json: output.join(","),
        seeds,
    })
}

fn consumers_by_change(
    preflight: &PatchPreflight,
    before: &ResolvedProgram,
) -> Result<Vec<Vec<ConsumerFact>>, Vec<Diagnostic>> {
    let mut facts = (0..preflight.changes().len())
        .map(|_| BTreeMap::<SourceConsumerKey, ConsumerFact>::new())
        .collect::<Vec<_>>();
    let mut edit_counts = vec![0usize; preflight.changes().len()];
    for edit in preflight.planned_edits() {
        let Some(change_facts) = facts.get_mut(edit.change) else {
            return Err(vec![impact_invariant_error(format!(
                "planned edit names missing change {}",
                edit.change
            ))]);
        };
        edit_counts[edit.change] += 1;
        let consumer = edit.consumer.clone().ok_or_else(|| {
            vec![impact_invariant_error(format!(
                "planned edit {}..{} has no semantic source consumer",
                edit.start, edit.end
            ))]
        })?;
        let role = edit.role.ok_or_else(|| {
            vec![impact_invariant_error(
                "planned edit has no semantic source-consumer role",
            )]
        })?;
        let declaration = before
            .declarations
            .declaration(&DeclarationId::new(consumer.id.clone()))
            .ok_or_else(|| {
                vec![impact_invariant_error(format!(
                    "source consumer `{}` is absent from HIR declaration metadata",
                    consumer.id
                ))]
            })?;
        let identity_origin = match declaration.identity_origin {
            IdentityOrigin::Explicit => "explicit",
            IdentityOrigin::Automatic => "automatic",
            IdentityOrigin::CompilerOwned => {
                return Err(vec![impact_invariant_error(format!(
                    "source consumer `{}` is compiler-owned",
                    consumer.id
                ))]);
            }
        };
        let fact = change_facts
            .entry(consumer.clone())
            .or_insert_with(|| ConsumerFact {
                key: consumer,
                identity_origin,
                roles: BTreeSet::new(),
                site_count: 0,
            });
        fact.roles.insert(role);
        fact.site_count += 1;
    }
    let mut output = Vec::with_capacity(facts.len());
    for (change, change_facts) in facts.into_iter().enumerate() {
        let change_facts = change_facts.into_values().collect::<Vec<_>>();
        if change_facts
            .iter()
            .map(|fact| fact.site_count)
            .sum::<usize>()
            != edit_counts[change]
            || edit_counts[change] == 0
        {
            return Err(vec![impact_invariant_error(
                "semantic impact source-consumer coverage does not match planned edits",
            )]);
        }
        output.push(change_facts);
    }
    Ok(output)
}

fn consumer_json(fact: &ConsumerFact) -> String {
    let roles = fact
        .roles
        .iter()
        .map(|role| quote_json(role.text()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":{},\"kind\":{},\"identity_origin\":{},\"roles\":[{}],\"site_count\":{}}}",
        quote_json(&fact.key.id),
        quote_json(fact.key.kind.text()),
        quote_json(fact.identity_origin),
        roles,
        fact.site_count
    )
}

fn reverse_closure(
    seeds: &BTreeMap<DeclarationId, BTreeSet<usize>>,
    index: &PersistentCallIndex,
) -> Result<Vec<AffectedFunction>, Vec<Diagnostic>> {
    let mut minimum_depth = BTreeMap::<DeclarationId, usize>::new();
    let mut provenance = BTreeMap::<DeclarationId, BTreeSet<usize>>::new();
    let mut level = BTreeSet::new();
    for (seed, operations) in seeds {
        minimum_depth.insert(seed.clone(), 0);
        provenance
            .entry(seed.clone())
            .or_default()
            .extend(operations.iter().copied());
        level.insert(seed.clone());
    }
    let mut ordered = Vec::new();
    let mut depth = 0usize;
    while !level.is_empty() {
        for id in &level {
            let kind = index.kind(id).ok_or_else(|| {
                vec![impact_invariant_error(format!(
                    "reverse-call node `{id}` has no callable kind"
                ))]
            })?;
            if index.origin(id) != Some(IdentityOrigin::Explicit) {
                return Err(vec![impact_invariant_error(format!(
                    "reverse-call node `{id}` is not a persistent authored callable"
                ))]);
            }
            ordered.push(AffectedFunction {
                id: id.clone(),
                kind,
                depth,
                operation_indices: provenance.get(id).cloned().unwrap_or_default(),
            });
        }
        let mut candidates = BTreeMap::<DeclarationId, BTreeSet<usize>>::new();
        for id in &level {
            let callers = index.callers_by_callee().get(id).ok_or_else(|| {
                vec![impact_invariant_error(format!(
                    "reverse-call node `{id}` has no caller-index entry"
                ))]
            })?;
            for caller in callers {
                candidates
                    .entry(caller.clone())
                    .or_default()
                    .extend(provenance.get(id).into_iter().flatten().copied());
            }
        }
        let candidate_depth = depth + 1;
        let mut next = BTreeSet::new();
        for (caller, operations) in candidates {
            match minimum_depth.get(&caller).copied() {
                None => {
                    minimum_depth.insert(caller.clone(), candidate_depth);
                    provenance.insert(caller.clone(), operations);
                    next.insert(caller);
                }
                Some(known) if known == candidate_depth => {
                    provenance
                        .entry(caller.clone())
                        .or_default()
                        .extend(operations);
                    next.insert(caller);
                }
                Some(_) => {}
            }
        }
        level = next;
        depth = candidate_depth;
    }
    Ok(ordered)
}

struct RenderInputs<'a> {
    preflight: &'a PatchPreflight,
    options: &'a SemanticImpactOptions,
    source_graph_schema: &'a str,
    patch_digest: &'a str,
    operations: &'a str,
    changes: &'a str,
    all_affected: &'a [AffectedFunction],
    within_depth: &'a [AffectedFunction],
    node_selected: usize,
}

struct RenderState {
    selected: usize,
    max_depth_used: usize,
    truncated: bool,
    reasons: String,
    omitted: usize,
    deferred: usize,
    frontier_json: String,
    affected_json: String,
}

fn affected_json(fact: &AffectedFunction) -> String {
    format!(
        "{{\"id\":{},\"kind\":{},\"depth\":{},\"operation_indices\":{}}}",
        quote_json(fact.id.as_str()),
        quote_json(fact.kind.text()),
        fact.depth,
        usize_array(&fact.operation_indices)
    )
}

fn frontier_json(fact: &AffectedFunction, reason: &'static str) -> String {
    format!(
        "{{\"id\":{},\"kind\":{},\"depth\":{},\"reasons\":[{}],\"operation_indices\":{}}}",
        quote_json(fact.id.as_str()),
        quote_json(fact.kind.text()),
        fact.depth,
        quote_json(reason),
        usize_array(&fact.operation_indices)
    )
}

fn decimal_len(value: usize) -> usize {
    value.to_string().len()
}

fn joined_range_len(prefix: &[usize], start: usize, end: usize) -> usize {
    if start == end {
        0
    } else {
        prefix[end] - prefix[start] + end - start - 1
    }
}

fn truncation_reasons(inputs: &RenderInputs<'_>, selected: usize) -> String {
    let mut reasons = Vec::with_capacity(3);
    if inputs.within_depth.len() < inputs.all_affected.len() {
        reasons.push(quote_json("depth"));
    }
    if inputs.node_selected < inputs.within_depth.len() {
        reasons.push(quote_json("max_nodes"));
    }
    if selected < inputs.node_selected {
        reasons.push(quote_json("max_bytes"));
    }
    reasons.join(",")
}

fn render_with_budget(inputs: RenderInputs<'_>) -> Result<BuiltImpactReport, Vec<Diagnostic>> {
    let affected = inputs
        .within_depth
        .iter()
        .map(affected_json)
        .collect::<Vec<_>>();
    let mut affected_prefix = vec![0usize; affected.len() + 1];
    for (index, json) in affected.iter().enumerate() {
        affected_prefix[index + 1] = affected_prefix[index] + json.len();
    }
    let frontier = inputs
        .all_affected
        .iter()
        .enumerate()
        .map(|(index, fact)| {
            frontier_json(
                fact,
                if index < inputs.node_selected {
                    "max_bytes"
                } else if index < inputs.within_depth.len() {
                    "max_nodes"
                } else {
                    "depth"
                },
            )
        })
        .collect::<Vec<_>>();
    let mut frontier_prefix = vec![0usize; frontier.len() + 1];
    let mut depth_end = vec![0usize; frontier.len()];
    let mut level_start = 0usize;
    for (index, json) in frontier.iter().enumerate() {
        frontier_prefix[index + 1] = frontier_prefix[index] + json.len();
        if index + 1 == frontier.len()
            || inputs.all_affected[index + 1].depth != inputs.all_affected[index].depth
        {
            for end in &mut depth_end[level_start..=index] {
                *end = index + 1;
            }
            level_start = index + 1;
        }
    }
    let empty = RenderState {
        selected: 0,
        max_depth_used: 0,
        truncated: false,
        reasons: String::new(),
        omitted: 0,
        deferred: 0,
        frontier_json: String::new(),
        affected_json: String::new(),
    };
    let baseline = render_report(&inputs, &empty, 0).len();
    let mut chosen = None;
    for selected in (0..=inputs.node_selected).rev() {
        let omitted = inputs.all_affected.len() - selected;
        let frontier_end = if omitted == 0 {
            selected
        } else {
            depth_end[selected]
        };
        let frontier_count = frontier_end - selected;
        let deferred = omitted - frontier_count;
        let reasons = truncation_reasons(&inputs, selected);
        let max_depth_used = inputs
            .within_depth
            .get(selected.saturating_sub(1))
            .map_or(0, |fact| fact.depth);
        let frontier_len = joined_range_len(&frontier_prefix, selected, frontier_end);
        let affected_len = joined_range_len(&affected_prefix, 0, selected);
        let mut without_used = baseline
            + decimal_len(selected)
            + decimal_len(max_depth_used)
            + decimal_len(omitted)
            + decimal_len(deferred)
            + reasons.len()
            + frontier_len
            + affected_len
            - 4;
        if omitted != 0 {
            without_used -= 1;
        }
        let mut used_digits = 1usize;
        let used_bytes = loop {
            let value = without_used + used_digits - 1;
            let next = decimal_len(value);
            if next == used_digits {
                break value;
            }
            used_digits = next;
        };
        if used_bytes <= inputs.options.max_bytes {
            chosen = Some((selected, used_bytes, frontier_end, reasons, max_depth_used));
            break;
        }
    }
    let Some((selected, used_bytes, frontier_end, reasons, max_depth_used)) = chosen else {
        return Err(vec![impact_option_error(format!(
            "semantic impact max_bytes {} cannot contain the mandatory canonical envelope",
            inputs.options.max_bytes
        ))]);
    };
    let state = RenderState {
        selected,
        max_depth_used,
        truncated: selected < inputs.all_affected.len(),
        reasons,
        omitted: inputs.all_affected.len() - selected,
        deferred: inputs.all_affected.len() - frontier_end,
        frontier_json: frontier[selected..frontier_end].join(","),
        affected_json: affected[..selected].join(","),
    };
    let output = render_report(&inputs, &state, used_bytes);
    if output.len() != used_bytes {
        return Err(vec![impact_invariant_error(
            "semantic impact incremental byte accounting disagrees with final rendering",
        )]);
    }
    Ok(BuiltImpactReport {
        json: output,
        truncated: state.truncated,
        omitted: state.omitted,
        deferred: state.deferred,
        frontier_empty: state.frontier_json.is_empty(),
        used_depth: state.max_depth_used,
        used_nodes: state.selected,
    })
}

fn render_report(inputs: &RenderInputs<'_>, state: &RenderState, used_bytes: usize) -> String {
    format!(
        "{{\"schema\":\"semaprax.semantic-impact.v1\",\"source_graph_schema\":{},\"base_revision\":{},\"candidate_revision\":{},\"patch\":{{\"schema\":{},\"digest\":{}}},\"operations\":[{}],\"changes\":[{}],\"query\":{{\"direction\":\"reverse\",\"depth\":{},\"max_bytes\":{},\"max_nodes\":{}}},\"budget\":{{\"used_bytes\":{},\"used_nodes\":{},\"max_depth_used\":{}}},\"truncation\":{{\"truncated\":{},\"reasons\":[{}],\"omitted_known_nodes\":{},\"deferred_known_nodes\":{}}},\"frontier\":[{}],\"affected_functions\":[{}]}}",
        quote_json(inputs.source_graph_schema),
        quote_json(inputs.preflight.base_revision()),
        quote_json(inputs.preflight.candidate_revision()),
        quote_json(inputs.preflight.schema_label()),
        quote_json(inputs.patch_digest),
        inputs.operations,
        inputs.changes,
        inputs.options.depth,
        inputs.options.max_bytes,
        inputs.options.max_nodes,
        used_bytes,
        state.selected,
        state.max_depth_used,
        state.truncated,
        state.reasons,
        state.omitted,
        state.deferred,
        state.frontier_json,
        state.affected_json
    )
}

fn patch_digest(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.semantic-impact.patch-digest.v1\0");
    hasher.update((source.len() as u64).to_le_bytes());
    hasher.update(source.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

fn type_array(types: &[ResolvedType]) -> String {
    format!(
        "[{}]",
        types
            .iter()
            .map(|ty| quote_json(&ty.identity_key()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn usize_array(values: &BTreeSet<usize>) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn impact_option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-G109", message)
}

fn impact_invariant_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G110", message)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::{graph, parse};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    const SOURCE: &str = r#"module impact.final_check;
@id("helper.answer") fn answer()->i64{42}
@id("app.main") fn main()->i64{answer()}
"#;

    fn fixture(label: &str) -> (std::path::PathBuf, std::path::PathBuf, String) {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-impact-unit-{}-{label}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source = directory.join("module.spx");
        let patch = directory.join("change.spatch");
        std::fs::write(&source, SOURCE).unwrap();
        let revision = graph::revision(&parse(SOURCE, &source).unwrap());
        std::fs::write(
            &patch,
            format!("base {revision}\nrename helper.answer to computed\n"),
        )
        .unwrap();
        (source, patch, revision)
    }

    #[test]
    fn canonical_equivalent_source_byte_drift_is_rejected_at_final_check() {
        let (source, patch, _) = fixture("format-drift");
        let error = preview_with_hook(
            &source,
            &patch,
            &SemanticImpactOptions::default(),
            |phase, source, _| {
                if phase == PreviewPhase::BeforeFinalCheck {
                    std::fs::write(source, SOURCE.replace("fn answer()", "fn  answer()"))?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
    }

    #[test]
    fn same_bytes_with_replaced_identity_are_rejected_at_final_check() {
        let (source, patch, _) = fixture("identity-drift");
        let displaced = source.with_extension("original.spx");
        let error = preview_with_hook(
            &source,
            &patch,
            &SemanticImpactOptions::default(),
            |phase, source, _| {
                if phase == PreviewPhase::BeforeFinalCheck {
                    std::fs::rename(source, &displaced)?;
                    std::fs::write(source, SOURCE)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        assert_eq!(std::fs::read_to_string(source).unwrap(), SOURCE);
    }

    #[test]
    fn patch_path_mutation_after_one_read_does_not_change_processed_digest() {
        let (source, patch, revision) = fixture("patch-drift");
        let original = std::fs::read_to_string(&patch).unwrap();
        let report = preview_with_hook(
            &source,
            &patch,
            &SemanticImpactOptions::default(),
            |phase, _, patch| {
                if phase == PreviewPhase::AfterPatchRead {
                    std::fs::write(
                        patch,
                        format!("base {revision}\nrename helper.answer to changed_again\n"),
                    )?;
                }
                Ok(())
            },
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&report).unwrap();
        assert_eq!(parsed["patch"]["digest"], patch_digest(&original));
        assert_eq!(parsed["operations"][0]["to"], "computed");
    }
}
