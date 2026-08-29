//! Deterministic, read-only Region Structure Report v1.
//!
//! [`generate`] projects one verified single-file SEMAPRAX module into one
//! canonical compact JSON envelope (`semaprax.region-report.v1`): a
//! lifetime-structure report per admitted explicit-ID monomorphic effect-free
//! scalar function, derived entirely from facts the existing borrow/move
//! checking already proves. For every admitted function the report carries:
//!
//! - every value binding (parameters, `let`/`let mut` locals, and match
//!   pattern bindings) with its real resolved-HIR [`hir::ValueId`], ownership
//!   mode, canonical type key, definition byte offset (the binding name
//!   token), effective live-range end byte offset (the end of the innermost
//!   statement or block tail containing its last read, assignment, contract
//!   clause, or own-consumption; equal to the definition offset when the
//!   binding is never used), and use count;
//! - the canonical region-cluster partition of those bindings under the rule
//!   that bindings whose live ranges `[def_offset, last_use_offset]` overlap
//!   can never share one region, greedily clustered in the canonical
//!   stable-id-then-binding-id order so the partition itself is deterministic;
//! - explicit per-function escape facts: today every borrow is provably
//!   non-escaping because return-position escape is rejected by the ownership
//!   checker diagnostic `SPX-O104` ("cannot return a borrowed or shared
//!   resource as owned"), and the closed admission profile excludes
//!   borrowed/shared parameter modes outright;
//! - move facts recomputed from the resolved call graph: own-consumption
//!   sites (a place passed to an `own` callee parameter - exactly the
//!   consumption the move-after-use diagnostics police), ordered by binding
//!   id then offset;
//! - bulk-release grouping candidates: maximal sets of at least two bindings
//!   whose effective live-range ends coincide, so one release point could
//!   cover them.
//!
//! Every non-admitted function is recorded as an exclusion with one closed
//! reason mirroring the shared scalar projection profile exactly.
//!
//! [`verify_envelope`] independently replays one envelope: exact envelope
//! shape, declared byte count, domain-separated payload digest, module
//! counts, closed exclusion vocabulary, strict stable-id/binding-id ordering,
//! the full greedy region-clustering re-derivation from the reported live
//! ranges, escape-fact re-derivation, consumption-site consistency, and the
//! exact bulk-release grouping re-derivation.
//! [`verify_envelope_against_source`] additionally binds the current source
//! bytes to the embedded source digest.
//!
//! Diagnostics use the previously unused `SPX-L1xx` family:
//! - `SPX-L101`: invalid options (bounds, malformed values).
//! - `SPX-L102`: output byte-budget exhaustion (fail-closed, no truncation).
//! - `SPX-L103`: envelope consistency, replay failure, or resolved-HIR
//!   inconsistency at generation time.
//!
//! This tranche implements no region inference, adds no region annotation
//! syntax, introduces no arena type, performs no bulk release, changes no
//! destructor behavior, executes nothing, and changes no source.

use std::path::Path;

use sha2::{Digest as _, Sha256};

use crate::ast::{Function, ParamMode, Type};
use crate::bounded_output::{with_limit, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedMatchPattern, ResolvedProgram, ResolvedRecordMatchFieldPattern, ResolvedStatement,
};
use crate::{graph, hir, parse, patch, verify};

mod replay;

macro_rules! bformat {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

pub const SCHEMA: &str = "semaprax.region-report.v1";

const DEFAULT_MAX_BYTES: usize = 64 * 1024;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.region-report.source.v1\0";
const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.region-report.payload.v1\0";

const REASON_AUTOMATIC_IDENTITY: &str = "automatic_identity";
const REASON_GENERIC_FUNCTION: &str = "generic_function";
const REASON_DECLARED_EFFECTS: &str = "declared_effects";
const REASON_UNSUPPORTED_PARAMETER_MODE: &str = "unsupported_parameter_mode";
const REASON_UNSUPPORTED_PARAMETER_TYPE: &str = "unsupported_parameter_type";
const REASON_UNSUPPORTED_RESULT_TYPE: &str = "unsupported_result_type";
const EXCLUSION_REASONS: [&str; 6] = [
    REASON_AUTOMATIC_IDENTITY,
    REASON_GENERIC_FUNCTION,
    REASON_DECLARED_EFFECTS,
    REASON_UNSUPPORTED_PARAMETER_MODE,
    REASON_UNSUPPORTED_PARAMETER_TYPE,
    REASON_UNSUPPORTED_RESULT_TYPE,
];

/// The ownership-checker diagnostic that rejects the one escape route a
/// borrowed view has today: being returned. Recorded verbatim in every
/// per-function escape section and re-checked by replay.
pub const ESCAPE_ENFORCING_CHECK: &str = "SPX-O104";
pub const ESCAPE_ENFORCING_CHECK_SUMMARY: &str =
    "return-position borrow escape is rejected: a function cannot return a borrowed or shared resource as owned";

const KIND_PARAM: &str = "param";
const KIND_LOCAL: &str = "local";
const KIND_PATTERN: &str = "match_pattern";

const NONCLAIMS_JSON: &str = "\"no_region_inference_implementation\",\
\"no_region_annotation_syntax\",\
\"no_arena_type\",\
\"no_bulk_release_runtime_behavior\",\
\"no_destructor_changes\",\
\"read_only_no_source_changes\",\
\"no_target_execution\"";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegionReportOptions {
    pub max_bytes: usize,
}

impl RegionReportOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=graph::MAX_AGENT_CONTEXT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "region-report max_bytes must be between {} and {}",
                graph::MIN_AGENT_CONTEXT_BYTES,
                graph::MAX_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for RegionReportOptions {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

fn option_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L101", message)
}

fn consistency_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-L103", message)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BindingFact {
    id: String,
    name: String,
    kind: &'static str,
    mutable: bool,
    ownership: String,
    type_key: String,
    def_offset: usize,
    /// Effective live-range end: the innermost statement/tail end covering
    /// the last use event, or the definition offset when never used.
    last_use_offset: usize,
    use_count: usize,
}

impl BindingFact {
    fn range_end(&self) -> usize {
        self.last_use_offset.max(self.def_offset)
    }

    fn overlaps(&self, other: &Self) -> bool {
        self.def_offset <= other.range_end() && other.def_offset <= self.range_end()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConsumptionSite {
    binding: String,
    offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FunctionReport {
    stable_id: String,
    name: String,
    bindings: Vec<BindingFact>,
    regions: Vec<Vec<String>>,
    consumption_sites: Vec<ConsumptionSite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExcludedFunction {
    stable_id: String,
    name: String,
    reason: &'static str,
}

/// One independently replayed function summary returned by
/// [`verify_envelope`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFunctionReport {
    pub stable_id: String,
    pub name: String,
    pub bindings_total: usize,
    pub regions_total: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VerifiedRegionReport {
    pub functions: Vec<VerifiedFunctionReport>,
}

/// Generate the canonical `semaprax.region-report.v1` envelope JSON for one
/// verified source file.
///
/// Read-only: source bytes must remain unchanged between the snapshot and the
/// final check or generation fails closed.
pub fn generate(
    source_path: &Path,
    options: &RegionReportOptions,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot(&canonical_source_path)?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&program);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    let revision = graph::revision(&program);
    let resolved = hir::resolve(&program)?;

    let mut sorted = program.functions.iter().collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    let functions_total = sorted.len();

    let mut excluded: Vec<ExcludedFunction> = Vec::new();
    let mut reports: Vec<FunctionReport> = Vec::new();
    for function in sorted {
        match admission(function) {
            Some(reason) => excluded.push(ExcludedFunction {
                stable_id: function.stable_id.clone(),
                name: function.name.clone(),
                reason,
            }),
            None => {
                let resolved_function = resolved
                    .functions
                    .iter()
                    .find(|candidate| candidate.id.as_str() == function.stable_id)
                    .ok_or_else(|| {
                        vec![consistency_error(format!(
                            "resolved HIR has no monomorphic function for `{}`",
                            function.stable_id
                        ))]
                    })?;
                reports.push(function_report(&resolved, resolved_function)?);
            }
        }
    }

    let digest = source_digest(snapshot.source());
    let path_text = source_path.display().to_string();
    let (envelope, overflowed) = with_limit(options.max_bytes, || {
        render(
            &path_text,
            &revision,
            &digest,
            &resolved.module,
            options.max_bytes,
            functions_total,
            &reports,
            &excluded,
        )
    });
    if overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-L102",
            "region-report output exceeds the max-bytes budget; refusing to truncate".to_owned(),
        )]);
    }
    patch::validate_source_unchanged(&canonical_source_path, source_path, &snapshot, &revision)?;
    Ok(envelope)
}

/// Closed AST-level admission gate mirroring the shared scalar profile.
fn admission(function: &Function) -> Option<&'static str> {
    if !function.explicit_id {
        return Some(REASON_AUTOMATIC_IDENTITY);
    }
    if !function.type_parameters.is_empty() {
        return Some(REASON_GENERIC_FUNCTION);
    }
    if !function.effects.is_empty() {
        return Some(REASON_DECLARED_EFFECTS);
    }
    for param in &function.params {
        if param.mode != ParamMode::Value {
            return Some(REASON_UNSUPPORTED_PARAMETER_MODE);
        }
        if !matches!(param.ty, Type::I64 | Type::Bool) {
            return Some(REASON_UNSUPPORTED_PARAMETER_TYPE);
        }
    }
    if !matches!(function.return_type, Type::I64 | Type::Bool) {
        return Some(REASON_UNSUPPORTED_RESULT_TYPE);
    }
    None
}

fn ownership_text(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}

#[derive(Default)]
struct FunctionFacts {
    bindings: Vec<BindingFact>,
    /// `(binding id, enclosing statement/tail end)` per use event.
    uses: Vec<(String, usize)>,
    consumption_sites: Vec<ConsumptionSite>,
}

impl FunctionFacts {
    fn push_binding(&mut self, fact: BindingFact) {
        if !self.bindings.iter().any(|existing| existing.id == fact.id) {
            self.bindings.push(fact);
        }
    }

    fn push_pattern_binding(&mut self, binding: &ResolvedBinding) {
        self.push_binding(BindingFact {
            id: binding.id.as_str().to_owned(),
            name: binding.name.clone(),
            kind: KIND_PATTERN,
            mutable: false,
            ownership: ownership_text(binding.ownership).to_owned(),
            type_key: binding.ty.identity_key(),
            def_offset: binding.span.start,
            last_use_offset: binding.span.start,
            use_count: 0,
        });
    }
}

fn function_report(
    resolved: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<FunctionReport, Vec<Diagnostic>> {
    let mut facts = FunctionFacts::default();

    for param in &function.params {
        facts.push_binding(BindingFact {
            id: param.id.as_str().to_owned(),
            name: param.name.clone(),
            kind: KIND_PARAM,
            mutable: false,
            ownership: ownership_text(param.ownership).to_owned(),
            type_key: param.ty.identity_key(),
            def_offset: param.span.start,
            last_use_offset: param.span.start,
            use_count: 0,
        });
    }

    // Contract clauses execute with the body, so their places count as uses;
    // their enclosing boundary is the clause itself.
    for clause in function.requires.iter().chain(function.ensures.iter()) {
        collect_expr(clause, clause.span.end, resolved, &mut facts);
    }
    collect_expr(&function.body, function.body.span.end, resolved, &mut facts);

    for (id, scope_end) in facts.uses.drain(..) {
        if let Some(binding) = facts.bindings.iter_mut().find(|binding| binding.id == id) {
            binding.use_count += 1;
            binding.last_use_offset = binding.last_use_offset.max(scope_end);
        }
    }

    facts
        .bindings
        .sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));

    // Canonical greedy interval coloring in binding-id order: a binding joins
    // the lowest existing region whose members it does not overlap, else a
    // fresh region opens. Overlapping live ranges never share a region.
    let regions = derive_regions(&facts.bindings);

    facts.consumption_sites.sort_by(|left, right| {
        left.binding
            .as_bytes()
            .cmp(right.binding.as_bytes())
            .then_with(|| left.offset.cmp(&right.offset))
    });

    Ok(FunctionReport {
        stable_id: function.id.as_str().to_owned(),
        name: function.name.clone(),
        bindings: facts.bindings,
        regions,
        consumption_sites: facts.consumption_sites,
    })
}

/// Collect one expression tree under `scope_end`: the end offset of the
/// innermost statement or block tail containing it. Every `Place` root is a
/// use of its binding attributed to `scope_end`; calls into `own` callee
/// parameters record consumption sites; statements introduce bindings and
/// refine the boundary for their own parts.
fn collect_expr(
    expression: &ResolvedExpr,
    scope_end: usize,
    resolved: &ResolvedProgram,
    facts: &mut FunctionFacts,
) {
    match &expression.kind {
        ResolvedExprKind::Place(place) => {
            facts.uses.push((place.root.as_str().to_owned(), scope_end));
        }
        ResolvedExprKind::BorrowPlace { place, .. } => {
            facts.uses.push((place.root.as_str().to_owned(), scope_end));
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_expr(source, scope_end, resolved, facts);
            collect_expr(start, scope_end, resolved, facts);
            collect_expr(end, scope_end, resolved, facts);
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            args,
            ..
        } => {
            for (index, argument) in args.iter().enumerate() {
                collect_expr(argument, scope_end, resolved, facts);
                if let ResolvedExprKind::Place(place) = &argument.kind {
                    if let Some(target) = resolved.resolve_call_target(callee, instance.as_ref()) {
                        if target
                            .params
                            .get(index)
                            .is_some_and(|param| param.ownership == OwnershipMode::Own)
                        {
                            facts.consumption_sites.push(ConsumptionSite {
                                binding: place.root.as_str().to_owned(),
                                offset: argument.span.start,
                            });
                        }
                    }
                }
            }
        }
        ResolvedExprKind::NativeRustImportCall(native) => {
            for argument in &native.args {
                collect_expr(argument, scope_end, resolved, facts);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                collect_expr(argument, scope_end, resolved, facts);
            }
        }
        ResolvedExprKind::Unary { value, .. } => {
            collect_expr(value, scope_end, resolved, facts);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expr(left, scope_end, resolved, facts);
            collect_expr(right, scope_end, resolved, facts);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        span,
                    } => {
                        collect_expr(value, span.end, resolved, facts);
                        facts.push_binding(BindingFact {
                            id: binding.id.as_str().to_owned(),
                            name: binding.name.clone(),
                            kind: KIND_LOCAL,
                            mutable: *mutable,
                            ownership: ownership_text(binding.ownership).to_owned(),
                            type_key: binding.ty.identity_key(),
                            def_offset: binding.span.start,
                            last_use_offset: binding.span.start,
                            use_count: 0,
                        });
                    }
                    ResolvedStatement::Assign {
                        binding,
                        field: _,
                        value,
                        span,
                    } => {
                        collect_expr(value, span.end, resolved, facts);
                        facts.uses.push((binding.id.as_str().to_owned(), span.end));
                    }
                    ResolvedStatement::Unsafe { body, span, .. } => {
                        collect_expr(body, span.end, resolved, facts);
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        // The loop re-evaluates its condition and body every
                        // iteration; the region report records their last
                        // textual extent without inventing iteration counts.
                        collect_expr(condition, body.span.end, resolved, facts);
                        collect_expr(body, body.span.end, resolved, facts);
                    }
                }
            }
            collect_expr(tail, tail.span.end, resolved, facts);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expr(condition, scope_end, resolved, facts);
            collect_expr(then_branch, scope_end, resolved, facts);
            collect_expr(else_branch, scope_end, resolved, facts);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_expr(&field.value, scope_end, resolved, facts);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_expr(scrutinee, scope_end, resolved, facts);
            for arm in arms {
                collect_pattern_bindings(&arm.pattern, facts);
                collect_expr(&arm.value, scope_end, resolved, facts);
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            collect_expr(operand, scope_end, resolved, facts);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_expr(base, scope_end, resolved, facts);
            for field in fields {
                collect_expr(&field.value, scope_end, resolved, facts);
            }
        }
        ResolvedExprKind::Project { base, .. } => {
            collect_expr(base, scope_end, resolved, facts);
        }
        ResolvedExprKind::Upcast { source } => {
            collect_expr(source, scope_end, resolved, facts);
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
    }
}

fn collect_pattern_bindings(pattern: &ResolvedMatchPattern, facts: &mut FunctionFacts) {
    match pattern {
        ResolvedMatchPattern::Wildcard => {}
        ResolvedMatchPattern::Variant { fields, .. } => {
            for field in fields {
                facts.push_pattern_binding(&field.binding);
            }
        }
        ResolvedMatchPattern::Record { fields, .. } => {
            collect_record_pattern_fields(fields, facts);
        }
        // Refutable Match v1: binding arms introduce one fact; literals and
        // or-patterns introduce none.
        ResolvedMatchPattern::Binding(binding) => facts.push_pattern_binding(binding),
        ResolvedMatchPattern::Literal(_) | ResolvedMatchPattern::Or(_) => {}
    }
}

fn collect_record_pattern_fields(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
    facts: &mut FunctionFacts,
) {
    for field in fields {
        match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                facts.push_pattern_binding(binding);
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {}
            ResolvedRecordMatchFieldPattern::Record { fields: nested, .. } => {
                collect_record_pattern_fields(nested, facts)
            }
        }
    }
}

/// The canonical greedy clustering, shared by generation and replay.
fn derive_regions(facts: &[BindingFact]) -> Vec<Vec<String>> {
    let conflicts = |members: &[String], binding: &BindingFact| {
        members.iter().any(|member| {
            let other = facts
                .iter()
                .find(|candidate| candidate.id == *member)
                .expect("region members come from the binding list");
            binding.overlaps(other)
        })
    };
    let mut regions: Vec<Vec<String>> = Vec::new();
    for binding in facts {
        match regions
            .iter()
            .position(|members| !conflicts(members, binding))
        {
            Some(index) => regions[index].push(binding.id.clone()),
            None => regions.push(vec![binding.id.clone()]),
        }
    }
    regions
}

/// Maximal sets of at least two bindings whose effective ends coincide,
/// ordered by end offset; members stay in binding-id order.
fn derive_release_groups(facts: &[BindingFact]) -> Vec<(usize, Vec<String>)> {
    let mut ends: Vec<usize> = Vec::new();
    let mut groups: Vec<(usize, Vec<String>)> = Vec::new();
    for fact in facts {
        let end = fact.range_end();
        if ends.contains(&end) {
            continue;
        }
        ends.push(end);
        let group: Vec<String> = facts
            .iter()
            .filter(|other| other.range_end() == end)
            .map(|other| other.id.clone())
            .collect();
        if group.len() >= 2 {
            groups.push((end, group));
        }
    }
    groups
}

/// Distinct consumed bindings in site order. Callers pass sites already
/// sorted by (binding id, offset), so equal ids are adjacent.
fn derive_moved_bindings(sites: &[ConsumptionSite]) -> Vec<&str> {
    let mut moved: Vec<&str> = Vec::with_capacity(sites.len());
    for site in sites {
        if !moved.last().is_some_and(|last| *last == site.binding) {
            moved.push(&site.binding);
        }
    }
    moved
}

fn render_binding(binding: &BindingFact) -> String {
    bformat!(
        "{{\"id\":{},\"name\":{},\"kind\":\"{}\",\"mutable\":{},\"ownership\":\"{}\",\
\"type\":{},\"def_offset\":{},\"last_use_offset\":{},\"use_count\":{}}}",
        quote_json(&binding.id),
        quote_json(&binding.name),
        binding.kind,
        binding.mutable,
        binding.ownership,
        quote_json(&binding.type_key),
        binding.def_offset,
        binding.range_end(),
        binding.use_count,
    )
}

#[allow(clippy::too_many_arguments)]
fn render(
    path_text: &str,
    revision: &str,
    digest: &str,
    module_name: &str,
    max_bytes: usize,
    functions_total: usize,
    reports: &[FunctionReport],
    excluded: &[ExcludedFunction],
) -> String {
    let function_entries = reports
        .iter()
        .map(|report| {
            let bindings = report
                .bindings
                .iter()
                .map(render_binding)
                .collect::<Vec<_>>();
            let regions = report
                .regions
                .iter()
                .enumerate()
                .map(|(index, members)| {
                    let ids = members.iter().map(|id| quote_json(id)).collect::<Vec<_>>();
                    bformat!(
                        "{{\"index\":{},\"binding_ids\":[{}]}}",
                        index,
                        ids.budgeted_join(",")
                    )
                })
                .collect::<Vec<_>>();
            let sites = report
                .consumption_sites
                .iter()
                .map(|site| {
                    bformat!(
                        "{{\"binding\":{},\"offset\":{}}}",
                        quote_json(&site.binding),
                        site.offset
                    )
                })
                .collect::<Vec<_>>();
            let moved_json = derive_moved_bindings(&report.consumption_sites)
                .iter()
                .map(|id| quote_json(id))
                .collect::<Vec<_>>();
            let borrowed = report
                .bindings
                .iter()
                .filter(|binding| binding.kind == KIND_PARAM && binding.ownership == "borrow")
                .count();
            let shared = report
                .bindings
                .iter()
                .filter(|binding| binding.kind == KIND_PARAM && binding.ownership == "shared")
                .count();
            let borrows_total = borrowed + shared;
            let release_entries = derive_release_groups(&report.bindings)
                .iter()
                .map(|(end, members)| {
                    let ids = members.iter().map(|id| quote_json(id)).collect::<Vec<_>>();
                    bformat!(
                        "{{\"end_offset\":{},\"binding_ids\":[{}]}}",
                        end,
                        ids.budgeted_join(",")
                    )
                })
                .collect::<Vec<_>>();
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\
\"bindings_total\":{},\"bindings\":[{}],\
\"regions_total\":{},\"regions\":[{}],\
\"escape\":{{\"borrowed_parameters\":{},\"shared_parameters\":{},\
\"borrows_total\":{},\"non_escaping_borrows_total\":{},\
\"all_borrows_provably_non_escaping\":true,\
\"enforcing_check\":\"{}\",\"enforcing_check_summary\":{}}},\
\"moves\":{{\"consumption_sites\":[{}],\"moved_bindings\":[{}]}},\
\"release_groups\":[{}]}}",
                quote_json(&report.stable_id),
                quote_json(&report.name),
                report.bindings.len(),
                bindings.budgeted_join(","),
                report.regions.len(),
                regions.budgeted_join(","),
                borrowed,
                shared,
                borrows_total,
                borrows_total,
                ESCAPE_ENFORCING_CHECK,
                quote_json(ESCAPE_ENFORCING_CHECK_SUMMARY),
                sites.budgeted_join(","),
                moved_json.budgeted_join(","),
                release_entries.budgeted_join(","),
            )
        })
        .collect::<Vec<_>>();
    let exclusion_entries = excluded
        .iter()
        .map(|entry| {
            bformat!(
                "{{\"stable_id\":{},\"name\":{},\"reason\":\"{}\"}}",
                quote_json(&entry.stable_id),
                quote_json(&entry.name),
                entry.reason,
            )
        })
        .collect::<Vec<_>>();

    let payload = bformat!(
        "{{\"schema\":\"{}\",\"source\":{{\"path\":{},\"revision\":{},\"sha256\":{}}},\
\"limits\":{{\"max_bytes\":{}}},\
\"module\":{{\"name\":{},\"functions_total\":{},\"functions_admitted\":{},\"functions_excluded\":{}}},\
\"functions\":[{}],\"exclusions\":[{}],\
\"nonclaims\":[{}]}}",
        SCHEMA,
        quote_json(path_text),
        quote_json(revision),
        quote_json(digest),
        max_bytes,
        quote_json(module_name),
        functions_total,
        reports.len(),
        excluded.len(),
        function_entries.budgeted_join(","),
        exclusion_entries.budgeted_join(","),
        NONCLAIMS_JSON,
    );
    bformat!(
        "{{\"schema\":\"{}\",\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        SCHEMA,
        quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload,
    )
}

fn source_digest(source: &str) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Independently verify one envelope produced by [`generate`].
///
/// Recomputes the outer payload digest over the exact serialized payload
/// bytes, re-checks the declared byte count, replays the closed exclusion
/// vocabulary and counts, re-derives the complete greedy region clustering
/// from the reported live ranges, re-derives the escape facts, move facts,
/// and bulk-release groupings, and re-checks both canonical orderings.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedRegionReport, Diagnostic> {
    replay::verify_envelope(envelope)
}

/// Verify one envelope and additionally bind the current bytes of
/// `source_path` to the embedded source digest, failing closed on drift.
pub fn verify_envelope_against_source(
    envelope: &str,
    source_path: &Path,
) -> Result<VerifiedRegionReport, Diagnostic> {
    replay::verify_envelope_against_source(envelope, source_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_reject_out_of_bounds_values() {
        assert!(RegionReportOptions::new(512).is_err());
        assert!(RegionReportOptions::new(graph::MAX_AGENT_CONTEXT_BYTES + 1).is_err());
        assert!(RegionReportOptions::new(graph::MIN_AGENT_CONTEXT_BYTES).is_ok());
        assert_eq!(RegionReportOptions::default().max_bytes, DEFAULT_MAX_BYTES);
    }

    #[test]
    fn constants_are_canonical() {
        let parsed_reasons: serde_json::Value = serde_json::from_str(&format!(
            "[{}]",
            EXCLUSION_REASONS
                .iter()
                .map(|reason| format!("\"{reason}\""))
                .collect::<Vec<_>>()
                .join(",")
        ))
        .expect("exclusion reasons constant");
        assert_eq!(
            parsed_reasons,
            serde_json::json!([
                "automatic_identity",
                "generic_function",
                "declared_effects",
                "unsupported_parameter_mode",
                "unsupported_parameter_type",
                "unsupported_result_type"
            ])
        );
        let parsed: serde_json::Value =
            serde_json::from_str(&format!("[{NONCLAIMS_JSON}]")).expect("nonclaims constant");
        assert_eq!(
            parsed,
            serde_json::json!([
                "no_region_inference_implementation",
                "no_region_annotation_syntax",
                "no_arena_type",
                "no_bulk_release_runtime_behavior",
                "no_destructor_changes",
                "read_only_no_source_changes",
                "no_target_execution"
            ])
        );
    }

    #[test]
    fn domain_digest_is_domain_separated() {
        let first = domain_digest(SOURCE_DIGEST_DOMAIN, b"abc");
        let second = domain_digest(PAYLOAD_DIGEST_DOMAIN, b"abc");
        assert_ne!(first, second);
        assert_eq!(first, domain_digest(SOURCE_DIGEST_DOMAIN, b"abc"));
    }

    fn fact(id: &str, def: usize, end: usize) -> BindingFact {
        BindingFact {
            id: id.to_owned(),
            name: id.to_owned(),
            kind: KIND_LOCAL,
            mutable: false,
            ownership: "value".to_owned(),
            type_key: "scalar:i64".to_owned(),
            def_offset: def,
            last_use_offset: end,
            use_count: usize::from(end > def),
        }
    }

    #[test]
    fn overlapping_live_ranges_never_share_a_region() {
        // a [0,10] overlaps b [5,20]; c [30,40] is disjoint from both and
        // greedily reuses a's region.
        let facts = vec![fact("a", 0, 10), fact("b", 5, 20), fact("c", 30, 40)];
        let regions = derive_regions(&facts);
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0], vec!["a".to_owned(), "c".to_owned()]);
        assert_eq!(regions[1], vec!["b".to_owned()]);
    }

    #[test]
    fn disjoint_live_ranges_share_the_lowest_region() {
        // a ends at 10; b starts at 11 and ends at 20: disjoint.
        let facts = vec![fact("a", 0, 10), fact("b", 11, 20)];
        let regions = derive_regions(&facts);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0], vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn release_groups_collect_maximal_same_end_sets() {
        // a and b end together at 10; c ends later alone.
        let facts = vec![fact("a", 0, 10), fact("b", 2, 10), fact("c", 12, 30)];
        let groups = derive_release_groups(&facts);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, 10);
        assert_eq!(groups[0].1, vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn unused_bindings_do_not_form_spurious_groups() {
        let facts = vec![fact("a", 0, 0), fact("b", 5, 5)];
        assert!(derive_release_groups(&facts).is_empty());
        assert_eq!(derive_regions(&facts).len(), 1);
    }

    #[test]
    fn admission_mirrors_the_scalar_profile() {
        let source = r#"
module test.probe;

@id("probe.ok")
fn ok(value: i64) -> bool { value > 0 }

@id("probe.generic")
fn pick<T>(value: T) -> T { value }
"#;
        let path = std::env::temp_dir().join(format!(
            "semaprax-region-report-unit-{}.spx",
            std::process::id()
        ));
        std::fs::write(&path, source).unwrap();
        let program = parse(&std::fs::read_to_string(&path).unwrap(), &path).expect("parses");
        let mut functions = program.functions.iter().collect::<Vec<_>>();
        functions.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
        assert_eq!(admission(functions[0]), Some(REASON_GENERIC_FUNCTION));
        assert_eq!(admission(functions[1]), None);
        let _ = std::fs::remove_file(&path);
    }
}
