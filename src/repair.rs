//! Read-only, targeted diagnostic repair discovery and instantiation.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::ast::{Expr, ExprKind, ParamMode, Program, Type};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::hir::{
    self, DeclarationId, ExpressionId, IdentityOrigin, ResolvedBinding, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, ResolvedStatement, ValueId,
};
use crate::{format, graph, parse, patch};

const REPORT_SCHEMA: &str = "semaprax.diagnostic-repair.v1";
const PREVIEW_SCHEMA: &str = "semaprax.diagnostic-repair-preview.v1";
const PATCH_SCHEMA: &str = "semaprax.semantic-patch.v3";
const REPAIR_ID_DOMAIN: &[u8] = b"semaprax.diagnostic-repair-id.v1\0";
const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.diagnostic-repair.source-digest.v1\0";
const PATCH_DIGEST_DOMAIN: &[u8] = b"semaprax.diagnostic-repair.patch-digest.v1\0";
const DERIVED_REBASE_DOMAIN: &[u8] = b"semaprax.diagnostic-repair.derived-rebase.v1\0";
const MIN_PERSISTENT_ID_BYTES: usize = 1;
const MAX_PERSISTENT_ID_BYTES: usize = 255;
pub(crate) const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_FUNCTIONS: usize = 1024;
const MAX_CALL_SITES: usize = 65_536;
const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const RESERVED_ID_PREFIXES: [&str; 7] = [
    "auto:",
    "core.",
    "semaprax.",
    "declaration:",
    "function-execution:",
    "parameter:",
    "nominal:",
];
const RESERVED_ID_VALUES: [&str; 2] = ["bool", "i64"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticRepairQuery {
    automatic_function_id: String,
}

impl DiagnosticRepairQuery {
    pub fn assign_function_id(
        automatic_function_id: impl Into<String>,
    ) -> Result<Self, Diagnostic> {
        let automatic_function_id = automatic_function_id.into();
        if automatic_function_id.is_empty() || automatic_function_id.contains(char::is_whitespace) {
            return Err(repair_query_error(
                "assign-function-id target must be one nonempty automatic function ID",
            ));
        }
        Ok(Self {
            automatic_function_id,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistentDeclarationId(String);

impl PersistentDeclarationId {
    pub fn new(value: impl Into<String>) -> Result<Self, Diagnostic> {
        let value = value.into();
        if !valid_persistent_id_syntax(&value) {
            return Err(repair_input_error(format!(
                "persistent_id must be 1..={MAX_PERSISTENT_ID_BYTES} ASCII bytes matching [A-Za-z0-9][A-Za-z0-9._:-]*"
            )));
        }
        if RESERVED_ID_PREFIXES
            .iter()
            .any(|prefix| value.starts_with(prefix))
            || RESERVED_ID_VALUES.contains(&value.as_str())
        {
            return Err(repair_input_error(
                "persistent_id uses a reserved compiler identity prefix",
            ));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn query(source_path: &Path, query: &DiagnosticRepairQuery) -> Result<String, Vec<Diagnostic>> {
    query_with_hook(source_path, query, |_, _| Ok(()))
}

pub fn instantiate(
    source_path: &Path,
    repair_id: &str,
    persistent_id: &PersistentDeclarationId,
) -> Result<String, Vec<Diagnostic>> {
    instantiate_with_hook(source_path, repair_id, persistent_id, |_, _| Ok(()))
}

fn query_with_hook(
    source_path: &Path,
    query: &DiagnosticRepairQuery,
    mut before_final_check: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let (canonical_path, snapshot, program, resolved, diagnostics, base_revision, usage) =
        read_eligible_source(source_path)?;
    let target = eligible_target(
        &program,
        &resolved,
        &diagnostics,
        &query.automatic_function_id,
    )?;
    let repair_id = repair_id(&base_revision, &target.stable_id);
    let report = render_report(
        &base_revision,
        snapshot.source(),
        target,
        &repair_id,
        &diagnostics,
        usage,
    )?;
    before_final_check(&canonical_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("diagnostic repair final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged_bounded(
        &canonical_path,
        source_path,
        &snapshot,
        &base_revision,
        MAX_SOURCE_BYTES,
    )?;
    Ok(report)
}

fn instantiate_with_hook(
    source_path: &Path,
    selected_repair_id: &str,
    persistent_id: &PersistentDeclarationId,
    mut before_final_check: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let (canonical_path, snapshot, program, resolved, diagnostics, base_revision, usage) =
        read_eligible_source(source_path)?;
    if resolved
        .declarations
        .declaration(&DeclarationId::new(persistent_id.as_str()))
        .is_some()
    {
        return Err(vec![repair_input_error(format!(
            "persistent_id `{}` is already present in the declaration table",
            persistent_id.as_str()
        ))]);
    }

    let mut selected = None;
    for function in &program.functions {
        if repair_id(&base_revision, &function.stable_id) != selected_repair_id {
            continue;
        }
        if selected.is_some() {
            return Err(vec![repair_query_error(
                "diagnostic repair ID matched more than one target",
            )]);
        }
        selected = Some(eligible_target(
            &program,
            &resolved,
            &diagnostics,
            &function.stable_id,
        )?);
    }
    let target = selected.ok_or_else(|| {
        vec![repair_query_error(
            "diagnostic repair ID is unknown or stale for the current source",
        )]
    })?;
    let candidate = validate_one_edit_rebase(
        snapshot.source(),
        source_path,
        &program,
        &resolved,
        target,
        persistent_id.as_str(),
    )?;
    let identity_rebase = identity_rebase_evidence(target, persistent_id.as_str(), &candidate);
    let patch_source = format!(
        "schema {PATCH_SCHEMA}\nbase {base_revision}\nassign-function-id repair {selected_repair_id} diagnostic SPX-S103 target {} name {} to {}\n",
        target.stable_id,
        target.name,
        persistent_id.as_str()
    );
    let preview = render_preview(PreviewRender {
        base_revision: &base_revision,
        source: snapshot.source(),
        target,
        repair_id: selected_repair_id,
        persistent_id,
        patch_source: &patch_source,
        proof: &candidate,
        identity_rebase: &identity_rebase,
        diagnostics: &diagnostics,
        usage,
    })?;
    before_final_check(&canonical_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("diagnostic repair final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged_bounded(
        &canonical_path,
        source_path,
        &snapshot,
        &base_revision,
        MAX_SOURCE_BYTES,
    )?;
    Ok(preview)
}

type EligibleSource = (
    std::path::PathBuf,
    patch::SourceSnapshot,
    Program,
    hir::ResolvedProgram,
    Vec<Diagnostic>,
    String,
    WorkUsage,
);

#[derive(Clone, Copy, Debug)]
struct WorkUsage {
    source_bytes: usize,
    functions: usize,
    call_sites: usize,
}

struct CallGraph {
    edges: BTreeMap<DeclarationId, BTreeSet<DeclarationId>>,
    call_sites: usize,
}

pub(crate) struct AssignmentCandidate {
    candidate: Program,
    canonical_candidate: String,
    candidate_revision: String,
    identity_rebase: IdentityRebaseEvidence,
}

impl AssignmentCandidate {
    pub(crate) fn into_parts(self) -> (Program, String, String, IdentityRebaseEvidence) {
        (
            self.candidate,
            self.canonical_candidate,
            self.candidate_revision,
            self.identity_rebase,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdentityRebaseCaller {
    id: String,
    identity_origin: IdentityOrigin,
    site_count: usize,
}

impl IdentityRebaseCaller {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn identity_origin(&self) -> IdentityOrigin {
        self.identity_origin
    }

    pub(crate) fn site_count(&self) -> usize {
        self.site_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdentityRebaseEvidence {
    before_id: String,
    after_id: String,
    name: String,
    direct_callers: Vec<IdentityRebaseCaller>,
    derived_id_count: usize,
    derived_id_digest: String,
}

impl IdentityRebaseEvidence {
    pub(crate) fn before_id(&self) -> &str {
        &self.before_id
    }

    pub(crate) fn after_id(&self) -> &str {
        &self.after_id
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn direct_callers(&self) -> &[IdentityRebaseCaller] {
        &self.direct_callers
    }

    pub(crate) fn derived_id_count(&self) -> usize {
        self.derived_id_count
    }

    pub(crate) fn derived_id_digest(&self) -> &str {
        &self.derived_id_digest
    }
}

pub(crate) struct PatchAssignmentInput<'a> {
    pub(crate) source: &'a str,
    pub(crate) source_path: &'a Path,
    pub(crate) before: &'a Program,
    pub(crate) before_resolved: &'a hir::ResolvedProgram,
    pub(crate) base_revision: &'a str,
    pub(crate) repair_id: &'a str,
    pub(crate) target_id: &'a str,
    pub(crate) target_name: &'a str,
    pub(crate) persistent_id: &'a str,
}

pub(crate) fn preflight_patch_assignment(
    input: PatchAssignmentInput<'_>,
) -> Result<AssignmentCandidate, Vec<Diagnostic>> {
    if input.source.len() > MAX_SOURCE_BYTES {
        return Err(vec![repair_query_error(format!(
            "diagnostic repair source exceeds {MAX_SOURCE_BYTES} bytes"
        ))]);
    }
    validate_closed_program(input.before, input.before_resolved)?;
    let analysis = hir::analyze(input.before);
    let target = eligible_target(
        input.before,
        input.before_resolved,
        &analysis.diagnostics,
        input.target_id,
    )?;
    if target.name != input.target_name {
        return Err(vec![repair_query_error(
            "assign-function-id name selector does not match the target declaration",
        )]);
    }
    if repair_id(input.base_revision, input.target_id) != input.repair_id {
        return Err(vec![repair_query_error(
            "assign-function-id repair selector is unknown or stale",
        )]);
    }
    let persistent_id =
        PersistentDeclarationId::new(input.persistent_id).map_err(|error| vec![error])?;
    if input
        .before_resolved
        .declarations
        .declaration(&DeclarationId::new(persistent_id.as_str()))
        .is_some()
    {
        return Err(vec![repair_input_error(format!(
            "persistent_id `{}` is already present in the declaration table",
            persistent_id.as_str()
        ))]);
    }
    let proof = validate_one_edit_rebase(
        input.source,
        input.source_path,
        input.before,
        input.before_resolved,
        target,
        persistent_id.as_str(),
    )?;
    let identity_rebase = identity_rebase_evidence(target, persistent_id.as_str(), &proof);
    Ok(AssignmentCandidate {
        candidate: proof.candidate,
        canonical_candidate: proof.canonical_candidate,
        candidate_revision: proof.candidate_revision,
        identity_rebase,
    })
}

fn identity_rebase_evidence(
    target: &crate::ast::Function,
    persistent_id: &str,
    proof: &CandidateProof,
) -> IdentityRebaseEvidence {
    IdentityRebaseEvidence {
        before_id: target.stable_id.clone(),
        after_id: persistent_id.to_owned(),
        name: target.name.clone(),
        direct_callers: proof
            .direct_callers
            .iter()
            .map(|(id, caller)| IdentityRebaseCaller {
                id: id.clone(),
                identity_origin: caller.identity_origin,
                site_count: caller.site_count,
            })
            .collect(),
        derived_id_count: proof.derived.len(),
        derived_id_digest: derived_rebase_digest(&proof.derived),
    }
}

fn read_eligible_source(source_path: &Path) -> Result<EligibleSource, Vec<Diagnostic>> {
    let canonical_path = patch::canonical_source_path(source_path)?;
    let snapshot =
        patch::read_source_snapshot_bounded(&canonical_path, MAX_SOURCE_BYTES, "SPX-R101")?;
    if snapshot.source().len() > MAX_SOURCE_BYTES {
        return Err(vec![repair_query_error(format!(
            "diagnostic repair source exceeds {MAX_SOURCE_BYTES} bytes"
        ))]);
    }
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    precheck_program(&program)?;
    let base_revision = graph::revision(&program);
    let analysis = hir::analyze(&program);
    let resolved = analysis.resolved.ok_or(analysis.diagnostics.clone())?;
    hir::validate(&resolved).map_err(|error| vec![error])?;
    let call_sites = validate_closed_program(&program, &resolved)?;
    let usage = WorkUsage {
        source_bytes: snapshot.source().len(),
        functions: program.functions.len(),
        call_sites,
    };
    Ok((
        canonical_path,
        snapshot,
        program,
        resolved,
        analysis.diagnostics,
        base_revision,
        usage,
    ))
}

pub(crate) fn precheck_program(program: &Program) -> Result<(), Vec<Diagnostic>> {
    if program.functions.len() > MAX_FUNCTIONS {
        return Err(vec![repair_query_error(format!(
            "diagnostic repair program exceeds {MAX_FUNCTIONS} functions"
        ))]);
    }
    let mut expressions = Vec::new();
    for function in &program.functions {
        expressions.extend(function.requires.iter());
        expressions.extend(function.ensures.iter());
        expressions.push(&function.body);
    }
    let mut call_sites = 0usize;
    while let Some(expression) = expressions.pop() {
        match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => {}
            ExprKind::Call { args, .. } => {
                call_sites = call_sites.saturating_add(1);
                if call_sites > MAX_CALL_SITES {
                    return Err(vec![repair_query_error(format!(
                        "diagnostic repair program exceeds {MAX_CALL_SITES} call sites"
                    ))]);
                }
                expressions.extend(args);
            }
            ExprKind::Unary { value, .. } => expressions.push(value),
            ExprKind::Binary { left, right, .. } => {
                expressions.push(right);
                expressions.push(left);
            }
            ExprKind::Block { statements, tail } => {
                expressions.push(tail);
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            expressions.push(child);
                        }
                    }
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                expressions.push(else_branch);
                expressions.push(then_branch);
                expressions.push(condition);
            }
            ExprKind::ConstructRecord { fields, .. }
            | ExprKind::ConstructVariant { fields, .. } => {
                expressions.extend(fields.iter().map(|field| &field.value));
            }
            ExprKind::Match { scrutinee, arms } => {
                expressions.push(scrutinee);
                expressions.extend(arms.iter().map(|arm| &arm.value));
            }
            ExprKind::Try { operand }
            | ExprKind::UpdateRecord { base: operand, .. }
            | ExprKind::Project { base: operand, .. } => expressions.push(operand),
            ExprKind::MethodCall { receiver, args, .. } => {
                expressions.push(receiver);
                expressions.extend(args);
            }
            ExprKind::SuperMethod { args, .. } => expressions.extend(args),
        }
        if let ExprKind::UpdateRecord { fields, .. } = &expression.kind {
            expressions.extend(fields.iter().map(|field| &field.value));
        }
    }
    Ok(())
}

fn validate_closed_program(
    program: &Program,
    resolved: &hir::ResolvedProgram,
) -> Result<usize, Vec<Diagnostic>> {
    if program.functions.len() > MAX_FUNCTIONS {
        return Err(vec![repair_query_error(format!(
            "diagnostic repair program exceeds {MAX_FUNCTIONS} functions"
        ))]);
    }
    let closed = program.types.is_empty()
        && program.interfaces.is_empty()
        && program.permits.is_empty()
        && resolved.function_templates.is_empty()
        && resolved.function_instances.is_empty()
        && graph::graph_schema(resolved) == "semaprax.graph.v10"
        && program.functions.iter().all(|function| {
            function.type_parameters.is_empty()
                && function.effects.is_empty()
                && function.requires.is_empty()
                && function.ensures.is_empty()
                && function.params.iter().all(|parameter| {
                    parameter.mode == ParamMode::Value && scalar_type(&parameter.ty)
                })
                && scalar_type(&function.return_type)
                && scalar_expr(&function.body)
        });
    let call_graph = call_graph(resolved)?;
    if !closed || has_call_cycle(&call_graph.edges) {
        return Err(vec![repair_query_error(
            "assign-function-id v1 requires an acyclic, effect-free, contract-free monomorphic scalar Graph-v10 program",
        )]);
    }
    Ok(call_graph.call_sites)
}

fn eligible_target<'a>(
    program: &'a Program,
    resolved: &hir::ResolvedProgram,
    diagnostics: &[Diagnostic],
    target: &str,
) -> Result<&'a crate::ast::Function, Vec<Diagnostic>> {
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == target)
        .ok_or_else(|| {
            vec![repair_query_error(format!(
                "automatic function target `{target}` was not found"
            ))]
        })?;
    if function.name == "main"
        || function.explicit_id
        || resolved.entrypoint.as_str() == function.stable_id
        || resolved
            .declarations
            .declaration(&DeclarationId::new(function.stable_id.clone()))
            .map(|declaration| declaration.identity_origin)
            != Some(IdentityOrigin::Automatic)
        || !diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "SPX-S103" && diagnostic.span == Some(function.name_span)
        })
    {
        return Err(vec![repair_query_error(format!(
            "function `{target}` is not an available SPX-S103 repair target"
        ))]);
    }
    Ok(function)
}

fn scalar_type(ty: &Type) -> bool {
    matches!(ty, Type::I64 | Type::Bool)
}

fn scalar_expr(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => true,
        ExprKind::Call {
            type_arguments,
            args,
            ..
        } => type_arguments.is_empty() && args.iter().all(scalar_expr),
        ExprKind::Unary { value, .. } => scalar_expr(value),
        ExprKind::Binary { left, right, .. } => scalar_expr(left) && scalar_expr(right),
        ExprKind::Block { statements, tail } => {
            statements.iter().all(|statement| {
                (0..statement.child_count())
                    .all(|index| statement.child(index).is_some_and(scalar_expr))
            }) && scalar_expr(tail)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => scalar_expr(condition) && scalar_expr(then_branch) && scalar_expr(else_branch),
        ExprKind::ConstructRecord { .. }
        | ExprKind::ConstructVariant { .. }
        | ExprKind::Match { .. }
        | ExprKind::Try { .. }
        | ExprKind::UpdateRecord { .. }
        | ExprKind::Project { .. }
        | ExprKind::MethodCall { .. }
        | ExprKind::SuperMethod { .. } => false,
    }
}

fn call_graph(program: &hir::ResolvedProgram) -> Result<CallGraph, Vec<Diagnostic>> {
    let known = program
        .functions
        .iter()
        .map(|function| function.id.clone())
        .collect::<BTreeSet<_>>();
    let mut graph = BTreeMap::new();
    let mut call_sites = 0usize;
    for function in &program.functions {
        let mut calls = BTreeSet::new();
        collect_calls(&function.body, &known, &mut calls, &mut call_sites);
        if call_sites > MAX_CALL_SITES {
            return Err(vec![repair_query_error(format!(
                "diagnostic repair program exceeds {MAX_CALL_SITES} call sites"
            ))]);
        }
        graph.insert(function.id.clone(), calls);
    }
    Ok(CallGraph {
        edges: graph,
        call_sites,
    })
}

fn has_call_cycle(graph: &BTreeMap<DeclarationId, BTreeSet<DeclarationId>>) -> bool {
    let mut indegree = graph
        .keys()
        .cloned()
        .map(|id| (id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for callees in graph.values() {
        for callee in callees {
            if let Some(count) = indegree.get_mut(callee) {
                *count += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some(id) = ready.pop() {
        visited += 1;
        if let Some(callees) = graph.get(&id) {
            for callee in callees {
                let count = indegree
                    .get_mut(callee)
                    .expect("call graph contains known functions only");
                *count -= 1;
                if *count == 0 {
                    ready.push(callee.clone());
                }
            }
        }
    }
    visited != graph.len()
}

fn collect_calls(
    expression: &ResolvedExpr,
    known: &BTreeSet<DeclarationId>,
    calls: &mut BTreeSet<DeclarationId>,
    call_sites: &mut usize,
) {
    match &expression.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::Place(_) => {}
        ResolvedExprKind::Call { callee, args, .. } => {
            *call_sites = call_sites.saturating_add(1);
            if known.contains(callee) {
                calls.insert(callee.clone());
            }
            for argument in args {
                collect_calls(argument, known, calls, call_sites);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_calls(argument, known, calls, call_sites);
            }
        }
        ResolvedExprKind::Unary { value, .. } => collect_calls(value, known, calls, call_sites),
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_calls(left, known, calls, call_sites);
            collect_calls(right, known, calls, call_sites);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                collect_calls(statement.value(), known, calls, call_sites);
            }
            collect_calls(tail, known, calls, call_sites);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_calls(condition, known, calls, call_sites);
            collect_calls(then_branch, known, calls, call_sites);
            collect_calls(else_branch, known, calls, call_sites);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_calls(&field.value, known, calls, call_sites);
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            collect_calls(scrutinee, known, calls, call_sites);
            for arm in arms {
                collect_calls(&arm.value, known, calls, call_sites);
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            collect_calls(operand, known, calls, call_sites);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_calls(base, known, calls, call_sites);
            for field in fields {
                collect_calls(&field.value, known, calls, call_sites);
            }
        }
        ResolvedExprKind::Project { base, .. } => collect_calls(base, known, calls, call_sites),
        ResolvedExprKind::Upcast { source } => collect_calls(source, known, calls, call_sites),
    }
}

#[derive(Debug)]
struct CandidateProof {
    candidate: Program,
    canonical_candidate: String,
    candidate_revision: String,
    candidate_source_digest: String,
    derived: Vec<RebaseEntry>,
    direct_callers: BTreeMap<String, DirectCaller>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct RebaseEntry {
    kind: &'static str,
    before: String,
    after: String,
}

#[derive(Clone, Debug)]
struct DirectCaller {
    identity_origin: IdentityOrigin,
    site_count: usize,
}

struct StructuralRebase<'a> {
    before_target: &'a str,
    after_target: &'a str,
    selected_function: bool,
    current_function: String,
    derived: BTreeSet<RebaseEntry>,
    reverse_ids: BTreeMap<String, String>,
    direct_callers: BTreeMap<String, DirectCaller>,
    before_program: &'a hir::ResolvedProgram,
}

fn validate_one_edit_rebase(
    source: &str,
    source_path: &Path,
    before: &Program,
    before_resolved: &hir::ResolvedProgram,
    target: &crate::ast::Function,
    persistent_id: &str,
) -> Result<CandidateProof, Vec<Diagnostic>> {
    let mut expected = before.clone();
    let expected_target = expected
        .functions
        .iter_mut()
        .find(|function| function.stable_id == target.stable_id)
        .expect("eligible target remains in cloned AST");
    expected_target.stable_id = persistent_id.to_owned();
    expected_target.explicit_id = true;
    let expected_source = format::canonical(&expected);

    let mut edited = source.to_owned();
    edited.insert_str(target.span.start, &format!("@id(\"{persistent_id}\")\n"));
    let edited_candidate = parse(&edited, source_path).map_err(|error| vec![error])?;
    if format::canonical(&edited_candidate) != expected_source {
        return Err(vec![repair_delta_error(
            "assign-function-id did not produce exactly one canonical identity annotation edit",
        )]);
    }
    let candidate = parse(&expected_source, source_path).map_err(|error| vec![error])?;
    let candidate_analysis = hir::analyze(&candidate);
    let candidate_resolved = candidate_analysis
        .resolved
        .ok_or(candidate_analysis.diagnostics.clone())?;
    hir::validate(&candidate_resolved).map_err(|error| vec![error])?;
    validate_closed_program(&candidate, &candidate_resolved)?;
    let mut structural = StructuralRebase {
        before_target: &target.stable_id,
        after_target: persistent_id,
        selected_function: false,
        current_function: String::new(),
        derived: BTreeSet::new(),
        reverse_ids: BTreeMap::from([(persistent_id.to_owned(), target.stable_id.clone())]),
        direct_callers: BTreeMap::new(),
        before_program: before_resolved,
    };
    structural.compare_program(before_resolved, &candidate_resolved)?;
    validate_normalized_graph(before, &candidate, persistent_id, &structural.reverse_ids)?;
    let candidate_revision = graph::revision(&candidate);
    let candidate_source_digest = domain_digest(SOURCE_DIGEST_DOMAIN, expected_source.as_bytes());
    Ok(CandidateProof {
        candidate,
        canonical_candidate: expected_source,
        candidate_revision,
        candidate_source_digest,
        derived: structural.derived.into_iter().collect(),
        direct_callers: structural.direct_callers,
    })
}

fn validate_normalized_graph(
    before: &Program,
    after: &Program,
    after_target: &str,
    reverse_ids: &BTreeMap<String, String>,
) -> Result<(), Vec<Diagnostic>> {
    let before_graph = graph::to_json(before)?;
    let after_graph = graph::to_json(after)?;
    let before_value: serde_json::Value =
        serde_json::from_str(&before_graph).map_err(|_| rebase_mismatch())?;
    let mut after_value: serde_json::Value =
        serde_json::from_str(&after_graph).map_err(|_| rebase_mismatch())?;
    after_value["revision"] = before_value["revision"].clone();
    let selected = after_value["nodes"]
        .as_array_mut()
        .and_then(|nodes| {
            nodes
                .iter_mut()
                .find(|node| node["id"].as_str() == Some(after_target))
        })
        .ok_or_else(rebase_mismatch)?;
    selected["identity_origin"] = serde_json::Value::String("automatic".to_owned());
    selected["persistent"] = serde_json::Value::Bool(false);
    normalize_identity_strings(&mut after_value, None, reverse_ids);
    if before_value != after_value {
        return Err(rebase_mismatch());
    }
    Ok(())
}

impl StructuralRebase<'_> {
    fn compare_program(
        &mut self,
        before: &hir::ResolvedProgram,
        after: &hir::ResolvedProgram,
    ) -> Result<(), Vec<Diagnostic>> {
        if before.module != after.module
            || before.permits != after.permits
            || before.types.len() != after.types.len()
            || before.interfaces.len() != after.interfaces.len()
            || before.function_templates.len() != after.function_templates.len()
            || before.function_instances.len() != after.function_instances.len()
            || before.functions.len() != after.functions.len()
            || before.types != after.types
            || before.interfaces != after.interfaces
            || before.function_templates != after.function_templates
            || before.function_instances != after.function_instances
            || before.entrypoint != after.entrypoint
        {
            return Err(rebase_mismatch());
        }
        for (left, right) in before.functions.iter().zip(&after.functions) {
            self.compare_function(left, right)?;
        }
        for left in &before.functions {
            let before_decl = before
                .declarations
                .declaration(&left.id)
                .ok_or_else(rebase_mismatch)?;
            let expected_id = if left.id.as_str() == self.before_target {
                DeclarationId::new(self.after_target)
            } else {
                left.id.clone()
            };
            let after_decl = after
                .declarations
                .declaration(&expected_id)
                .ok_or_else(rebase_mismatch)?;
            let expected_origin = if left.id.as_str() == self.before_target {
                IdentityOrigin::Explicit
            } else {
                before_decl.identity_origin
            };
            if before_decl.name != after_decl.name
                || before_decl.kind != after_decl.kind
                || before_decl.owner != after_decl.owner
                || after_decl.identity_origin != expected_origin
                || before.declarations.function_id(&left.name) != Some(&left.id)
                || after.declarations.function_id(&left.name) != Some(&expected_id)
            {
                return Err(rebase_mismatch());
            }
        }
        if after
            .declarations
            .declaration(&DeclarationId::new(self.before_target))
            .is_some()
        {
            return Err(rebase_mismatch());
        }
        Ok(())
    }

    fn compare_function(
        &mut self,
        before: &ResolvedFunction,
        after: &ResolvedFunction,
    ) -> Result<(), Vec<Diagnostic>> {
        self.selected_function = before.id.as_str() == self.before_target;
        self.current_function = before.id.as_str().to_owned();
        let expected_id = if self.selected_function {
            self.after_target
        } else {
            before.id.as_str()
        };
        if after.id.as_str() != expected_id
            || before.name != after.name
            || before.return_type != after.return_type
            || before.effects != after.effects
            || before.requires.len() != after.requires.len()
            || before.ensures.len() != after.ensures.len()
            || before.params.len() != after.params.len()
            || before.cleanup != after.cleanup
        {
            return Err(rebase_mismatch());
        }
        for (left, right) in before.params.iter().zip(&after.params) {
            if left.name != right.name || left.ownership != right.ownership || left.ty != right.ty {
                return Err(rebase_mismatch());
            }
            self.compare_value_id("parameter", &left.id, &right.id)?;
        }
        self.compare_value_id("result", &before.result_id, &after.result_id)?;
        for (left, right) in before.requires.iter().zip(&after.requires) {
            self.compare_expr(left, right)?;
        }
        self.compare_expr(&before.body, &after.body)?;
        for (left, right) in before.ensures.iter().zip(&after.ensures) {
            self.compare_expr(left, right)?;
        }
        let before_plan: serde_json::Value = serde_json::from_str(
            &crate::graph_cleanup::cleanup_plan_json(&before.cleanup_plan),
        )
        .map_err(|_| rebase_mismatch())?;
        let mut after_plan: serde_json::Value = serde_json::from_str(
            &crate::graph_cleanup::cleanup_plan_json(&after.cleanup_plan),
        )
        .map_err(|_| rebase_mismatch())?;
        normalize_identity_strings(&mut after_plan, None, &self.reverse_ids);
        if before_plan != after_plan {
            return Err(rebase_mismatch());
        }
        Ok(())
    }

    fn compare_value_id(
        &mut self,
        kind: &'static str,
        before: &ValueId,
        after: &ValueId,
    ) -> Result<(), Vec<Diagnostic>> {
        self.compare_derived_id(kind, before.as_str(), after.as_str())
    }

    fn compare_expression_id(
        &mut self,
        before: &ExpressionId,
        after: &ExpressionId,
    ) -> Result<(), Vec<Diagnostic>> {
        self.compare_derived_id("expression", before.as_str(), after.as_str())
    }

    fn compare_value_reference(
        &self,
        before: &ValueId,
        after: &ValueId,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.selected_function {
            if self.reverse_ids.get(after.as_str()).map(String::as_str) != Some(before.as_str()) {
                return Err(rebase_mismatch());
            }
        } else if before != after {
            return Err(rebase_mismatch());
        }
        Ok(())
    }

    fn compare_derived_id(
        &mut self,
        kind: &'static str,
        before: &str,
        after: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.selected_function {
            if before == after {
                return Err(rebase_mismatch());
            }
            if let Some(existing) = self.reverse_ids.get(after) {
                if existing != before {
                    return Err(rebase_mismatch());
                }
            } else {
                self.reverse_ids.insert(after.to_owned(), before.to_owned());
            }
            self.derived.insert(RebaseEntry {
                kind,
                before: before.to_owned(),
                after: after.to_owned(),
            });
        } else if before != after {
            return Err(rebase_mismatch());
        }
        Ok(())
    }

    fn compare_binding(
        &mut self,
        before: &ResolvedBinding,
        after: &ResolvedBinding,
    ) -> Result<(), Vec<Diagnostic>> {
        if before.name != after.name || before.ownership != after.ownership || before.ty != after.ty
        {
            return Err(rebase_mismatch());
        }
        self.compare_value_id("binding", &before.id, &after.id)
    }

    fn compare_expr(
        &mut self,
        before: &ResolvedExpr,
        after: &ResolvedExpr,
    ) -> Result<(), Vec<Diagnostic>> {
        if before.ty != after.ty || before.ownership != after.ownership {
            return Err(rebase_mismatch());
        }
        self.compare_expression_id(&before.id, &after.id)?;
        match (&before.kind, &after.kind) {
            (ResolvedExprKind::Int(left), ResolvedExprKind::Int(right)) if left == right => {}
            (ResolvedExprKind::Bool(left), ResolvedExprKind::Bool(right)) if left == right => {}
            (ResolvedExprKind::Place(left), ResolvedExprKind::Place(right)) => {
                if left.projections != right.projections {
                    return Err(rebase_mismatch());
                }
                self.compare_value_reference(&left.root, &right.root)?;
            }
            (
                ResolvedExprKind::Call {
                    callee: left_callee,
                    type_arguments: left_types,
                    instance: left_instance,
                    args: left_args,
                },
                ResolvedExprKind::Call {
                    callee: right_callee,
                    type_arguments: right_types,
                    instance: right_instance,
                    args: right_args,
                },
            ) => {
                if left_types != right_types
                    || left_instance != right_instance
                    || left_args.len() != right_args.len()
                {
                    return Err(rebase_mismatch());
                }
                if left_callee.as_str() == self.before_target {
                    if right_callee.as_str() != self.after_target {
                        return Err(rebase_mismatch());
                    }
                    let declaration = self
                        .before_program
                        .declarations
                        .declaration(&DeclarationId::new(&self.current_function))
                        .ok_or_else(rebase_mismatch)?;
                    let caller = self
                        .direct_callers
                        .entry(self.current_function.clone())
                        .or_insert(DirectCaller {
                            identity_origin: declaration.identity_origin,
                            site_count: 0,
                        });
                    caller.site_count += 1;
                } else if left_callee != right_callee {
                    return Err(rebase_mismatch());
                }
                for (left, right) in left_args.iter().zip(right_args) {
                    self.compare_expr(left, right)?;
                }
            }
            (
                ResolvedExprKind::Unary {
                    op: left_op,
                    value: left,
                },
                ResolvedExprKind::Unary {
                    op: right_op,
                    value: right,
                },
            ) if left_op == right_op => self.compare_expr(left, right)?,
            (
                ResolvedExprKind::Binary {
                    op: left_op,
                    left: left_lhs,
                    right: left_rhs,
                },
                ResolvedExprKind::Binary {
                    op: right_op,
                    left: right_lhs,
                    right: right_rhs,
                },
            ) if left_op == right_op => {
                self.compare_expr(left_lhs, right_lhs)?;
                self.compare_expr(left_rhs, right_rhs)?;
            }
            (
                ResolvedExprKind::Block {
                    statements: left_statements,
                    tail: left_tail,
                },
                ResolvedExprKind::Block {
                    statements: right_statements,
                    tail: right_tail,
                },
            ) => {
                if left_statements.len() != right_statements.len() {
                    return Err(rebase_mismatch());
                }
                for (left, right) in left_statements.iter().zip(right_statements) {
                    match (left, right) {
                        (
                            ResolvedStatement::Let {
                                binding: left_binding,
                                value: left_value,
                                ..
                            },
                            ResolvedStatement::Let {
                                binding: right_binding,
                                value: right_value,
                                ..
                            },
                        ) => {
                            self.compare_binding(left_binding, right_binding)?;
                            self.compare_expr(left_value, right_value)?;
                        }
                        (
                            ResolvedStatement::Assign {
                                binding: left_binding,
                                field: left_field,
                                value: left_value,
                                ..
                            },
                            ResolvedStatement::Assign {
                                binding: right_binding,
                                field: right_field,
                                value: right_value,
                                ..
                            },
                        ) => {
                            self.compare_binding(left_binding, right_binding)?;
                            if left_field != right_field {
                                return Err(rebase_mismatch());
                            }
                            self.compare_expr(left_value, right_value)?;
                        }
                        (
                            ResolvedStatement::Unsafe {
                                audit: left_audit,
                                body: left_body,
                                ..
                            },
                            ResolvedStatement::Unsafe {
                                audit: right_audit,
                                body: right_body,
                                ..
                            },
                        ) => {
                            if left_audit != right_audit {
                                return Err(rebase_mismatch());
                            }
                            self.compare_expr(left_body, right_body)?;
                        }
                        _ => return Err(rebase_mismatch()),
                    }
                }
                self.compare_expr(left_tail, right_tail)?;
            }
            (
                ResolvedExprKind::If {
                    condition: left_condition,
                    then_branch: left_then,
                    else_branch: left_else,
                },
                ResolvedExprKind::If {
                    condition: right_condition,
                    then_branch: right_then,
                    else_branch: right_else,
                },
            ) => {
                self.compare_expr(left_condition, right_condition)?;
                self.compare_expr(left_then, right_then)?;
                self.compare_expr(left_else, right_else)?;
            }
            _ => return Err(rebase_mismatch()),
        }
        Ok(())
    }
}

fn normalize_identity_strings(
    value: &mut serde_json::Value,
    field: Option<&str>,
    reverse: &BTreeMap<String, String>,
) {
    match value {
        serde_json::Value::String(text) => {
            if identity_string_field(field) {
                if let Some(before) = reverse.get(text) {
                    *text = before.clone();
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                normalize_identity_strings(value, field, reverse);
            }
        }
        serde_json::Value::Object(values) => {
            for (field, value) in values {
                normalize_identity_strings(value, Some(field), reverse);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {}
    }
}

fn identity_string_field(field: Option<&str>) -> bool {
    matches!(
        field,
        Some(
            "id" | "owner"
                | "entrypoint"
                | "type_id"
                | "return_type_id"
                | "result_id"
                | "callee"
                | "root"
                | "expression"
                | "call"
                | "value_expression"
                | "scrutinee"
                | "function"
                | "template"
                | "record"
                | "variant"
                | "case"
                | "field"
                | "lifecycle_id"
                | "calls"
                | "fields"
                | "cases"
                | "projections"
        )
    )
}

fn rebase_mismatch() -> Vec<Diagnostic> {
    vec![repair_delta_error(
        "candidate HIR exceeds the selected function identity rebase",
    )]
}

fn render_report(
    base_revision: &str,
    source: &str,
    target: &crate::ast::Function,
    repair_id: &str,
    diagnostics: &[Diagnostic],
    usage: WorkUsage,
) -> Result<String, Vec<Diagnostic>> {
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == "SPX-S103" && diagnostic.span == Some(target.name_span)
        })
        .ok_or_else(|| {
            vec![repair_query_error(
                "eligible target has no exact SPX-S103 diagnostic",
            )]
        })?;
    render_with_budget(usage, |budget| {
        format!(
            "{{\"schema\":\"{REPORT_SCHEMA}\",\"source_graph_schema\":\"semaprax.graph.v10\",\"base_revision\":{},\"source\":{{\"digest\":{}}},\"limits\":{},\"budget\":{},\"query\":{{\"kind\":\"assign_function_id\",\"target\":{}}},\"diagnostic\":{},\"repair\":{{\"id\":{},\"kind\":\"assign_function_id\",\"classification\":\"breaking_identity_rebase\",\"applicability\":\"requires_input\",\"input\":{{\"name\":\"persistent_id\",\"type\":\"persistent_declaration_id\",\"required\":true,\"constraints\":{{\"min_bytes\":1,\"max_bytes\":255,\"pattern\":\"[A-Za-z0-9][A-Za-z0-9._:-]*\",\"forbidden_prefixes\":[\"auto:\",\"core.\",\"semaprax.\",\"declaration:\",\"function-execution:\",\"parameter:\",\"nominal:\"],\"forbidden_values\":[\"bool\",\"i64\"]}}}},\"operation\":{{\"schema\":\"{PATCH_SCHEMA}\",\"kind\":\"assign_function_id\",\"repair_id\":{},\"diagnostic\":\"SPX-S103\",\"target\":{},\"name\":{},\"to\":{{\"input\":\"persistent_id\"}}}}}}}}",
            quote_json(base_revision),
            quote_json(&domain_digest(SOURCE_DIGEST_DOMAIN, source.as_bytes())),
            limits_json(),
            budget,
            quote_json(&target.stable_id),
            diagnostic.json(),
            quote_json(repair_id),
            quote_json(repair_id),
            quote_json(&target.stable_id),
            quote_json(&target.name),
        )
    })
}

struct PreviewRender<'a> {
    base_revision: &'a str,
    source: &'a str,
    target: &'a crate::ast::Function,
    repair_id: &'a str,
    persistent_id: &'a PersistentDeclarationId,
    patch_source: &'a str,
    proof: &'a CandidateProof,
    identity_rebase: &'a IdentityRebaseEvidence,
    diagnostics: &'a [Diagnostic],
    usage: WorkUsage,
}

fn render_preview(input: PreviewRender<'_>) -> Result<String, Vec<Diagnostic>> {
    let diagnostic = input
        .diagnostics
        .iter()
        .find(|diagnostic| {
            diagnostic.code == "SPX-S103" && diagnostic.span == Some(input.target.name_span)
        })
        .ok_or_else(|| {
            vec![repair_query_error(
                "eligible target has no exact SPX-S103 diagnostic",
            )]
        })?;
    let callers = input
        .identity_rebase
        .direct_callers()
        .iter()
        .map(|caller| {
            format!(
                "{{\"id\":{},\"identity_origin\":{},\"site_count\":{}}}",
                quote_json(caller.id()),
                quote_json(caller.identity_origin().text()),
                caller.site_count()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    render_with_budget(input.usage, |budget| {
        format!(
            "{{\"schema\":\"{PREVIEW_SCHEMA}\",\"source_graph_schema\":\"semaprax.graph.v10\",\"base_revision\":{},\"candidate_revision\":{},\"source\":{{\"digest\":{}}},\"candidate_source\":{{\"digest\":{}}},\"limits\":{},\"budget\":{},\"query\":{{\"kind\":\"assign_function_id\",\"target\":{}}},\"diagnostic\":{},\"repair\":{{\"id\":{},\"kind\":\"assign_function_id\",\"classification\":\"breaking_identity_rebase\",\"input\":{{\"persistent_id\":{}}}}},\"patch\":{{\"schema\":\"{PATCH_SCHEMA}\",\"digest\":{},\"source\":{}}},\"identity_rebase\":{{\"before_id\":{},\"after_id\":{},\"name\":{},\"direct_callers\":[{}],\"derived_id_count\":{},\"derived_id_digest\":{}}}}}",
            quote_json(input.base_revision),
            quote_json(&input.proof.candidate_revision),
            quote_json(&domain_digest(
                SOURCE_DIGEST_DOMAIN,
                input.source.as_bytes()
            )),
            quote_json(&input.proof.candidate_source_digest),
            limits_json(),
            budget,
            quote_json(&input.target.stable_id),
            diagnostic.json(),
            quote_json(input.repair_id),
            quote_json(input.persistent_id.as_str()),
            quote_json(&domain_digest(
                PATCH_DIGEST_DOMAIN,
                input.patch_source.as_bytes()
            )),
            quote_json(input.patch_source),
            quote_json(input.identity_rebase.before_id()),
            quote_json(input.identity_rebase.after_id()),
            quote_json(input.identity_rebase.name()),
            callers,
            input.identity_rebase.derived_id_count(),
            quote_json(input.identity_rebase.derived_id_digest()),
        )
    })
}

fn limits_json() -> String {
    format!(
        "{{\"max_source_bytes\":{MAX_SOURCE_BYTES},\"max_functions\":{MAX_FUNCTIONS},\"max_call_sites\":{MAX_CALL_SITES},\"max_output_bytes\":{MAX_OUTPUT_BYTES}}}"
    )
}

fn render_with_budget(
    usage: WorkUsage,
    render: impl Fn(&str) -> String,
) -> Result<String, Vec<Diagnostic>> {
    let mut used_output_bytes = 0usize;
    for _ in 0..4 {
        let budget = format!(
            "{{\"used_source_bytes\":{},\"used_functions\":{},\"used_call_sites\":{},\"used_output_bytes\":{}}}",
            usage.source_bytes, usage.functions, usage.call_sites, used_output_bytes
        );
        let output = render(&budget);
        if output.len() == used_output_bytes {
            return bounded_output(output);
        }
        used_output_bytes = output.len();
    }
    Err(vec![repair_delta_error(
        "diagnostic repair output byte accounting did not converge",
    )])
}

fn bounded_output(output: String) -> Result<String, Vec<Diagnostic>> {
    if output.len() > MAX_OUTPUT_BYTES {
        Err(vec![repair_query_error(format!(
            "diagnostic repair output exceeds {MAX_OUTPUT_BYTES} bytes"
        ))])
    } else {
        Ok(output)
    }
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

fn derived_rebase_digest(entries: &[RebaseEntry]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DERIVED_REBASE_DOMAIN);
    for entry in entries {
        hash_text(&mut hasher, entry.kind);
        hash_text(&mut hasher, &entry.before);
        hash_text(&mut hasher, &entry.after);
    }
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn repair_id(base_revision: &str, target: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(REPAIR_ID_DOMAIN);
    hash_text(&mut hasher, base_revision);
    hash_text(&mut hasher, "SPX-S103");
    hash_text(&mut hasher, "function");
    hash_text(&mut hasher, target);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn hash_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn valid_persistent_id_syntax(value: &str) -> bool {
    if !(MIN_PERSISTENT_ID_BYTES..=MAX_PERSISTENT_ID_BYTES).contains(&value.len()) {
        return false;
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && bytes
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

fn repair_query_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-R101", message)
}

fn repair_input_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-R102", message)
}

fn repair_delta_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G112", message)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn fixture(source: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-repair-unit-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("module.spx");
        std::fs::write(&path, source).unwrap();
        (directory, path)
    }

    #[test]
    fn query_rejects_same_byte_identity_replacement_at_final_check() {
        let source = "module repair.unit;\nfn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let (directory, path) = fixture(source);
        let query = DiagnosticRepairQuery::assign_function_id("auto:repair.unit.helper").unwrap();
        let error = query_with_hook(&path, &query, |canonical, _| {
            let replacement = directory.join("replacement.spx");
            std::fs::write(&replacement, source)?;
            std::fs::rename(replacement, canonical)
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn instantiate_rejects_source_byte_drift_at_final_check() {
        let source = "module repair.unit;\nfn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let (directory, path) = fixture(source);
        let request = DiagnosticRepairQuery::assign_function_id("auto:repair.unit.helper").unwrap();
        let report = query(&path, &request).unwrap();
        let value: serde_json::Value = serde_json::from_str(&report).unwrap();
        let repair_id = value["repair"]["id"].as_str().unwrap();
        let persistent = PersistentDeclarationId::new("repair.unit.helper").unwrap();
        let error = instantiate_with_hook(&path, repair_id, &persistent, |canonical, _| {
            std::fs::write(canonical, source.replace("{1}", "{2}"))
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn instantiate_rejects_growth_beyond_the_final_read_bound() {
        let source = "module repair.unit;\nfn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let (directory, path) = fixture(source);
        let request = DiagnosticRepairQuery::assign_function_id("auto:repair.unit.helper").unwrap();
        let report = query(&path, &request).unwrap();
        let value: serde_json::Value = serde_json::from_str(&report).unwrap();
        let repair_id = value["repair"]["id"].as_str().unwrap();
        let persistent = PersistentDeclarationId::new("repair.unit.helper").unwrap();
        let error = instantiate_with_hook(&path, repair_id, &persistent, |canonical, _| {
            std::fs::write(canonical, vec![b'x'; MAX_SOURCE_BYTES + 1])
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
