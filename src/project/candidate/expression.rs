//! Revision-bound expression discovery and source-authoritative replacement.
//! HIR IDs select meaning; compiler-derived, uniquely joined AST paths select
//! candidate nodes. Neither spans nor source text are accepted from callers.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::ast::{self, Expr, ExprKind, Span, Statement};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedMatchPattern, ResolvedRecordMatchFieldPattern, ResolvedStatement,
};
use crate::project::{ProjectRevision, ProjectSource};

use super::{intent, parse_revision, wire, ProjectCandidate};

const MAX_EXPRESSIONS: usize = 4096;
const MAX_DEPTH: usize = 256;
const MAX_SCOPE_FACTS: usize = 16_384;
const MAX_CATALOG_BYTES: usize = 1024 * 1024;
const SCHEMA: &str = "semaprax.project-expression-catalog.v1";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
type SpanKey = (usize, usize, usize, usize);

#[derive(Clone, Copy)]
struct Binding<'a> {
    name: &'a str,
    id: &'a str,
    ty: &'a hir::ResolvedType,
    ownership: OwnershipMode,
    mutable: bool,
}

type Scope<'a> = BTreeMap<&'a str, Binding<'a>>;

struct Fact<'a> {
    expression: &'a ResolvedExpr,
    phase: &'static str,
    scope: Scope<'a>,
}

struct AstFact<'a> {
    expression: &'a Expr,
    phase: &'static str,
    path: Vec<usize>,
}

struct Subject<'a> {
    source: &'a ProjectSource,
    module: &'a str,
    function: &'a ResolvedFunction,
}

/// A narrow authenticated handoff for compiler-owned structural operations.
/// The body path and lexical binding identities are derived, never user input.
pub(super) struct AuthoredExpression<'a> {
    pub(super) owner: usize,
    pub(super) function_index: usize,
    pub(super) phase: &'static str,
    pub(super) path: Vec<usize>,
    pub(super) expression: &'a ResolvedExpr,
    pub(super) scope: Vec<LexicalBinding<'a>>,
    pub(super) effects: &'a [String],
}

pub(super) struct LexicalBinding<'a> {
    pub(super) name: &'a str,
    pub(super) id: &'a str,
    pub(super) ty: &'a hir::ResolvedType,
    pub(super) ownership: OwnershipMode,
    pub(super) mutable: bool,
}

pub(super) fn authored_selection<'a>(
    revision: &'a ProjectRevision,
    programs: &[ast::Program],
    target: &str,
    expression_id: &str,
) -> Result<AuthoredExpression<'a>> {
    authored_region_selection(revision, programs, target, expression_id, false)
}

pub(super) fn authored_contract_selection<'a>(
    revision: &'a ProjectRevision,
    programs: &[ast::Program],
    target: &str,
    expression_id: &str,
) -> Result<AuthoredExpression<'a>> {
    authored_region_selection(revision, programs, target, expression_id, true)
}

/// Direct callees of the complete original predicate containing the selected
/// subtree. HIR inventory bounds apply before this borrowed scan; stable
/// template callees remain distinct from their revision-local instances.
pub(super) fn contract_call_targets(
    revision: &ProjectRevision,
    target: &str,
    expression_id: &str,
) -> Result<BTreeSet<String>> {
    let programs = parse_revision(revision)?;
    let selected = authored_contract_selection(revision, &programs, target, expression_id)?;
    let subject = subject(revision, target)?;
    let ordinal = *selected
        .path
        .first()
        .ok_or_else(|| invalid("contract selection has no predicate ordinal"))?;
    let predicates = if selected.phase == "requires" {
        &subject.function.requires
    } else {
        &subject.function.ensures
    };
    let root = predicates
        .get(ordinal)
        .ok_or_else(|| invalid("contract predicate is unavailable"))?;
    let facts = hir_facts(subject.function)?;
    let mut targets = BTreeSet::new();
    for fact in facts {
        if fact.phase == selected.phase
            && fact.expression.span.start >= root.span.start
            && fact.expression.span.end <= root.span.end
        {
            if let ResolvedExprKind::Call { callee, .. } = &fact.expression.kind {
                targets.insert(callee.as_str().to_owned());
            }
        }
    }
    Ok(targets)
}

fn authored_region_selection<'a>(
    revision: &'a ProjectRevision,
    programs: &[ast::Program],
    target: &str,
    expression_id: &str,
    contract: bool,
) -> Result<AuthoredExpression<'a>> {
    let subject = subject(revision, target)?;
    let (owner, function_index) = source_function(programs, &subject, target)?;
    let facts = hir_facts(subject.function)?;
    let fact = selected(&facts, expression_id)?;
    if !region_admitted(fact.phase, contract) {
        return Err(invalid(if contract {
            "contract selection requires an authored predicate expression"
        } else {
            "structural extraction requires an authored body expression"
        }));
    }
    let ast = ast_facts(&programs[owner].functions[function_index])?;
    let spans = hir_span_counts(&facts);
    let joined = join(fact, &ast, &spans, subject.source.source())
        .ok_or_else(|| invalid("expression has no unique authenticated authored AST origin"))?;
    if !contract {
        reject_unsafe_ancestor(
            &programs[owner].functions[function_index].body,
            &joined.path,
        )?;
    }
    Ok(AuthoredExpression {
        owner,
        function_index,
        phase: fact.phase,
        path: joined.path.clone(),
        expression: fact.expression,
        scope: fact
            .scope
            .values()
            .map(|binding| LexicalBinding {
                name: binding.name,
                id: binding.id,
                ty: binding.ty,
                ownership: binding.ownership,
                mutable: binding.mutable,
            })
            .collect(),
        effects: &subject.function.effects,
    })
}

// Extraction cannot move a nested expression out of its original audit owner.
fn reject_unsafe_ancestor(mut node: &Expr, path: &[usize]) -> Result<()> {
    for index in path {
        if let ExprKind::Block { statements, .. } = &node.kind {
            let mut offset = 0;
            for statement in statements {
                let end = offset + statement.child_count();
                if *index >= offset
                    && *index < end
                    && matches!(statement, ast::Statement::Unsafe { .. })
                {
                    return Err(invalid("extraction cannot cross an unsafe audit boundary"));
                }
                offset = end;
            }
        }
        node = ast_children(node)
            .get(*index)
            .copied()
            .ok_or_else(|| invalid("authenticated extraction path is unavailable"))?;
    }
    Ok(())
}

pub(super) fn authored_slot<'a>(
    programs: &'a mut [ast::Program],
    selection: &AuthoredExpression<'_>,
) -> Result<&'a mut Expr> {
    if selection.phase != "body" {
        return Err(invalid("structural extraction requires a body selection"));
    }
    ast_at_mut(
        &mut programs[selection.owner].functions[selection.function_index].body,
        &selection.path,
    )
}

impl ProjectCandidate {
    /// Authenticated HIR expression identities with lexical context. Visibility
    /// is not a proof that an owned binding remains live at the selected point.
    pub fn expression_catalog(&self, target: &str) -> Result<String> {
        self.expression_region_catalog(target, false)
    }

    /// Existing predicate regions only; this does not widen the legacy body
    /// expression catalogue or accept source paths supplied by callers.
    pub fn contract_expression_catalog(&self, target: &str) -> Result<String> {
        self.expression_region_catalog(target, true)
    }

    fn expression_region_catalog(&self, target: &str, contract: bool) -> Result<String> {
        let subject = subject(&self.revision, target)?;
        let programs = parse_revision(&self.revision)?;
        let (owner, function_index) = source_function(&programs, &subject, target)?;
        let function = &programs[owner].functions[function_index];
        let ast = ast_facts(function)?;
        let facts = hir_facts(subject.function)?;
        let spans = hir_span_counts(&facts);
        let expressions = facts
            .iter()
            .filter(|fact| !contract || region_admitted(fact.phase, true))
            .map(|fact| {
                let joined = join(fact, &ast, &spans, subject.source.source());
                let reason = if !region_admitted(fact.phase, contract) {
                    "contract_region_is_read_only"
                } else if joined.is_none() {
                    "no_unique_authored_ast_origin"
                } else {
                    "requires_typed_constructor_and_full_project_revalidation"
                };
                json!({
                    "expression_id":fact.expression.id.as_str(), "phase":fact.phase,
                    "kind":hir_kind(&fact.expression.kind),
                    "expected_type":fact.expression.ty.identity_key(),
                    "ownership":ownership(fact.expression.ownership),
                    "source_span":span_json(fact.expression.span),
                    "replaceable":region_admitted(fact.phase, contract) && joined.is_some(), "reason":reason,
                    "scope":fact.scope.values().map(binding_json).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        wire::render(
            json!({
                "schema":if contract {"semaprax.project-contract-expression-catalog.v1"} else {SCHEMA}, "candidate_digest":self.candidate_digest(),
                "project_revision":self.revision.project_revision(), "target":target,
                "source":{"path":subject.source.path(),"module":subject.module,
                    "source_revision":subject.source.source_revision(),"source_digest":subject.source.source_digest()},
                "declared_effect_budget":subject.function.effects,
                "expressions":expressions,
                "limits":{"max_expressions":MAX_EXPRESSIONS,"max_depth":MAX_DEPTH,"max_scope_facts":MAX_SCOPE_FACTS,"max_bytes":MAX_CATALOG_BYTES},
                "nonclaims":["lexical_scope_is_not_owned_value_liveness","declared_effect_budget_is_not_expression_effect_inference","expression_ids_are_revision_scoped",if contract {"no_body_replacement_or_contract_truth_proof"} else {"no_contract_replacement"},"no_source_or_commit_authority"],
            }),
            MAX_CATALOG_BYTES,
        )
    }
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [ast::Program],
    request: &Value,
) -> Result<intent::IntentSummary> {
    apply_region(revision, programs, request, false)
}

pub(super) fn apply_contract(
    revision: &ProjectRevision,
    programs: &mut [ast::Program],
    request: &Value,
) -> Result<intent::IntentSummary> {
    apply_region(revision, programs, request, true)
}

fn apply_region(
    revision: &ProjectRevision,
    programs: &mut [ast::Program],
    request: &Value,
    contract: bool,
) -> Result<intent::IntentSummary> {
    validate_region_request(request, contract)?;
    let target = text(request, "target")?;
    let expression_id = text(request, "expression_id")?;
    let subject = subject(revision, target)?;
    let (owner, function_index) = source_function(programs, &subject, target)?;
    let facts = hir_facts(subject.function)?;
    let fact = selected(&facts, expression_id)?;
    if !region_admitted(fact.phase, contract) {
        return Err(invalid(if contract {
            "contract replacement requires a requires or ensures expression"
        } else {
            "contract expressions are read-only in replace_expression"
        }));
    }
    let ast = ast_facts(&programs[owner].functions[function_index])?;
    let spans = hir_span_counts(&facts);
    let joined = join(fact, &ast, &spans, subject.source.source())
        .ok_or_else(|| invalid("expression has no unique authenticated authored AST origin"))?;
    let path = joined.path.clone();
    let scope = fact.scope.keys().map(|name| (*name).to_owned()).collect();
    let nominal_scope = if intent::uses_field_places(&request["replacement"]) {
        fact.scope
            .values()
            .map(|binding| {
                (
                    binding.name.to_owned(),
                    std::sync::Arc::new(binding.ty.clone()),
                )
            })
            .collect()
    } else {
        BTreeMap::new()
    };
    let replacement = intent::construct_expression_with_scope(
        revision,
        &programs[owner],
        &scope,
        nominal_scope,
        &request["replacement"],
    )?;
    let slot = region_at_mut(
        &mut programs[owner].functions[function_index],
        fact.phase,
        &path,
    )?;
    // Keep the required block category for function and branch bodies. The
    // typed constructors produce expressions, never caller-authored blocks.
    *slot = if matches!(slot.kind, ExprKind::Block { .. }) {
        Expr {
            kind: ExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(replacement),
            },
            span: Span::default(),
        }
    } else {
        replacement
    };
    Ok(intent::IntentSummary {
        target_id: target.to_owned(),
        kind: if contract {
            "replace_contract_expression"
        } else {
            "replace_expression"
        }
        .to_owned(),
        migrated_calls: 0,
    })
}

/// Validate the replacement's actual type and ownership after full Project
/// admission. Locate its new source span through the old compiler-derived AST
/// parent path; old revision-scoped expression IDs/spans are never assumed to
/// survive replacement, formatting or lowering.
pub(super) fn validate_replacement(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<()> {
    remapped_expression(before, after, request).map(|_| ())
}

pub(super) fn validate_contract_replacement(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<()> {
    validate_region_request(request, true)?;
    remap_contract_selection(
        before,
        after,
        text(request, "target")?,
        text(request, "expression_id")?,
    )?;
    // Reconstruct the complete expected projection independently. Contract
    // counts alone cannot prove that another predicate/body/signature survived.
    let mut programs = parse_revision(before)?;
    apply_contract(before, &mut programs, request)?;
    let sources = super::materialize(&programs)?;
    if sources.len() != after.sources().len()
        || sources
            .iter()
            .zip(after.sources())
            .any(|(expected, actual)| {
                expected.path != actual.path() || expected.source != actual.source()
            })
    {
        return Err(invalid(
            "contract replacement changed source outside its authenticated subtree",
        ));
    }
    Ok(())
}

/// Rebase calls this only after independently rejecting competing body edits.
/// This helper authenticates the corresponding new selector, not merge safety.
pub(super) fn rebase_intent(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<Value> {
    let id = remapped_expression(before, after, request)?;
    let mut remapped = request.clone();
    remapped["expression_id"] = Value::String(id);
    Ok(remapped)
}

pub(super) fn rebase_contract_intent(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<Value> {
    validate_region_request(request, true)?;
    let id = remap_contract_selection(
        before,
        after,
        text(request, "target")?,
        text(request, "expression_id")?,
    )?;
    let mut remapped = request.clone();
    remapped["expression_id"] = json!(id);
    Ok(remapped)
}

fn remapped_expression(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<String> {
    validate_request(request)?;
    let target = text(request, "target")?;
    remap_selection(before, after, target, text(request, "expression_id")?)
}

/// Re-authenticate a surviving compiler-owned selection after a disjoint edit.
/// Callers must separately prove the selected subtree was not overwritten.
pub(super) fn remap_selection(
    before: &ProjectRevision,
    after: &ProjectRevision,
    target: &str,
    expression_id: &str,
) -> Result<String> {
    remap_region_selection(before, after, target, expression_id, false)
}

pub(super) fn remap_contract_selection(
    before: &ProjectRevision,
    after: &ProjectRevision,
    target: &str,
    expression_id: &str,
) -> Result<String> {
    remap_region_selection(before, after, target, expression_id, true)
}

fn remap_region_selection(
    before: &ProjectRevision,
    after: &ProjectRevision,
    target: &str,
    expression_id: &str,
    contract: bool,
) -> Result<String> {
    let old_subject = subject(before, target)?;
    let old_programs = parse_revision(before)?;
    let (owner, function_index) = source_function(&old_programs, &old_subject, target)?;
    let old_ast = ast_facts(&old_programs[owner].functions[function_index])?;
    let old_facts = hir_facts(old_subject.function)?;
    let selected = selected(&old_facts, expression_id)?;
    if !region_admitted(selected.phase, contract) {
        return Err(invalid(if contract {
            "contract selection cannot be remapped across source regions"
        } else {
            "contract expressions cannot be replaced"
        }));
    }
    let old_spans = hir_span_counts(&old_facts);
    let old_node = join(selected, &old_ast, &old_spans, old_subject.source.source())
        .ok_or_else(|| invalid("original replacement expression lost its source origin"))?;
    let new_subject = subject(after, target)?;
    let new_programs = parse_revision(after)?;
    let (owner, function_index) = source_function(&new_programs, &new_subject, target)?;
    let new_ast = ast_facts(&new_programs[owner].functions[function_index])?;
    let new_node = new_ast
        .iter()
        .find(|node| node.phase == selected.phase && node.path == old_node.path)
        .ok_or_else(|| {
            invalid("replacement AST parent path did not survive canonical source projection")
        })?;
    let new_facts = hir_facts(new_subject.function)?;
    let new_spans = hir_span_counts(&new_facts);
    let mut matches = new_facts.iter().filter(|fact| {
        fact.phase == selected.phase
            && fact.expression.span == new_node.expression.span
            && join(fact, &new_ast, &new_spans, new_subject.source.source()).is_some()
    });
    let new_fact = matches
        .next()
        .ok_or_else(|| invalid("replacement has no unique independently admitted HIR origin"))?;
    if matches.next().is_some()
        || selected.expression.ty != new_fact.expression.ty
        || selected.expression.ownership != new_fact.expression.ownership
    {
        return Err(invalid(
            "replacement changed the selected expression's expected type or ownership",
        ));
    }
    Ok(new_fact.expression.id.as_str().to_owned())
}

fn subject<'a>(revision: &'a ProjectRevision, target: &str) -> Result<Subject<'a>> {
    if target.is_empty() || target.len() > 4096 || target.contains('\0') {
        return Err(invalid(
            "expression discovery requires a bounded function stable ID",
        ));
    }
    let mut found = None;
    for module in revision.semantic.image_modules() {
        for function in module
            .functions()
            .iter()
            .filter(|function| function.id.as_str() == target)
        {
            if found.is_some() {
                return Err(invalid("expression function identity is ambiguous"));
            }
            let source = revision
                .sources()
                .iter()
                .find(|source| source.path() == module.path())
                .ok_or_else(|| invalid("expression module source is absent"))?;
            if source.source_revision() != module.source_revision()
                || source.source_digest() != module.source_digest()
            {
                return Err(invalid("expression module source provenance disagrees"));
            }
            found = Some(Subject {
                source,
                module: module.module(),
                function,
            });
        }
    }
    found.ok_or_else(|| {
        invalid("expression function is not an admitted monomorphic source function")
    })
}

fn source_function(
    programs: &[ast::Program],
    subject: &Subject<'_>,
    target: &str,
) -> Result<(usize, usize)> {
    let mut found = None;
    for (owner, program) in programs.iter().enumerate() {
        if program.path != subject.source.path() || program.module != subject.module {
            continue;
        }
        for (index, function) in program
            .functions
            .iter()
            .enumerate()
            .filter(|(_, function)| function.stable_id == target)
        {
            if found.is_some()
                || !function.explicit_id
                || !function.type_parameters.is_empty()
                || function.span != subject.function.span
                || function.name != subject.function.name
            {
                return Err(invalid(
                    "expression target lacks an exact explicit top-level source function",
                ));
            }
            found = Some((owner, index));
        }
    }
    found.ok_or_else(|| invalid("expression target is not an explicit top-level source function"))
}

fn selected<'a, 'b>(facts: &'a [Fact<'b>], id: &str) -> Result<&'a Fact<'b>> {
    if id.is_empty() || id.len() > 16_384 || id.contains('\0') {
        return Err(invalid("expression identity is invalid"));
    }
    let mut selected = facts
        .iter()
        .filter(|fact| fact.expression.id.as_str() == id);
    let fact = selected
        .next()
        .ok_or_else(|| invalid("expression identity is not part of this function revision"))?;
    if selected.next().is_some() {
        return Err(invalid("expression identity is ambiguous"));
    }
    Ok(fact)
}

fn span_key(span: Span) -> SpanKey {
    (span.start, span.end, span.line, span.column)
}

fn hir_span_counts(facts: &[Fact<'_>]) -> BTreeMap<SpanKey, usize> {
    let mut spans = BTreeMap::new();
    for fact in facts {
        *spans.entry(span_key(fact.expression.span)).or_insert(0) += 1;
    }
    spans
}

fn join<'a, 'b>(
    fact: &Fact<'_>,
    ast: &'a [AstFact<'b>],
    spans: &BTreeMap<SpanKey, usize>,
    source: &str,
) -> Option<&'a AstFact<'b>> {
    let span = fact.expression.span;
    if span.start >= span.end
        || source.get(span.start..span.end).is_none()
        || spans.get(&span_key(span)) != Some(&1)
    {
        return None;
    }
    if matches!(
        fact.expression.kind,
        ResolvedExprKind::Upcast { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::ByteRange { .. }
            | ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::HostCommandCall(_)
    ) {
        return None;
    }
    let mut nodes = ast
        .iter()
        .filter(|node| node.phase == fact.phase && node.expression.span == span);
    let node = nodes.next()?;
    if nodes.next().is_some() {
        return None;
    }
    let authored = ast_kind(&node.expression.kind);
    let resolved = hir_kind(&fact.expression.kind);
    if authored != resolved && !(authored == "project" && resolved == "place") {
        return None;
    }
    Some(node)
}

fn hir_facts(function: &ResolvedFunction) -> Result<Vec<Fact<'_>>> {
    let mut scope = Scope::new();
    for param in &function.params {
        scope.insert(
            param.name.as_str(),
            Binding {
                name: &param.name,
                id: param.id.as_str(),
                ty: &param.ty,
                ownership: param.ownership,
                mutable: false,
            },
        );
    }
    let mut facts = Vec::new();
    let mut scope_count = 0;
    for expression in &function.requires {
        visit_hir(
            expression,
            "requires",
            &scope,
            0,
            &mut facts,
            &mut scope_count,
        )?;
    }
    visit_hir(
        &function.body,
        "body",
        &scope,
        0,
        &mut facts,
        &mut scope_count,
    )?;
    scope.insert(
        "result",
        Binding {
            name: "result",
            id: function.result_id.as_str(),
            ty: &function.return_type,
            ownership: function.body.ownership,
            mutable: false,
        },
    );
    for expression in &function.ensures {
        visit_hir(
            expression,
            "ensures",
            &scope,
            0,
            &mut facts,
            &mut scope_count,
        )?;
    }
    let mut ids = BTreeMap::new();
    for fact in &facts {
        if ids.insert(fact.expression.id.as_str(), ()).is_some() {
            return Err(invalid("retained expression IDs are not unique"));
        }
    }
    Ok(facts)
}

fn visit_hir<'a>(
    expression: &'a ResolvedExpr,
    phase: &'static str,
    scope: &Scope<'a>,
    depth: usize,
    facts: &mut Vec<Fact<'a>>,
    scope_count: &mut usize,
) -> Result<()> {
    if depth > MAX_DEPTH || facts.len() >= MAX_EXPRESSIONS {
        return Err(limit("expression catalogue traversal exceeds its bound"));
    }
    *scope_count = scope_count
        .checked_add(scope.len())
        .ok_or_else(|| limit("expression scope accounting overflow"))?;
    if *scope_count > MAX_SCOPE_FACTS {
        return Err(limit(
            "expression catalogue lexical scope inventory exceeds its bound",
        ));
    }
    facts.push(Fact {
        expression,
        phase,
        scope: scope.clone(),
    });
    match &expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            let mut local = scope.clone();
            for statement in statements {
                match statement {
                    ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        ..
                    } => {
                        visit_hir(value, phase, &local, depth + 1, facts, scope_count)?;
                        insert_binding(&mut local, binding, *mutable);
                    }
                    ResolvedStatement::Assign { value, .. } => {
                        visit_hir(value, phase, &local, depth + 1, facts, scope_count)?
                    }
                    ResolvedStatement::Unsafe { body, .. } => {
                        visit_hir(body, phase, &local, depth + 1, facts, scope_count)?
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        visit_hir(condition, phase, &local, depth + 1, facts, scope_count)?;
                        visit_hir(body, phase, &local, depth + 1, facts, scope_count)?;
                    }
                }
            }
            visit_hir(tail, phase, &local, depth + 1, facts, scope_count)?;
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            visit_hir(scrutinee, phase, scope, depth + 1, facts, scope_count)?;
            for arm in arms {
                let mut local = scope.clone();
                pattern_bindings(&arm.pattern, &mut local, 0)?;
                if let Some(guard) = &arm.guard {
                    visit_hir(guard, phase, &local, depth + 1, facts, scope_count)?;
                }
                visit_hir(&arm.value, phase, &local, depth + 1, facts, scope_count)?;
            }
        }
        _ => {
            let mut children = Vec::new();
            hir::push_resolved_expression_children_in_authored_order(expression, &mut children);
            while let Some(child) = children.pop() {
                visit_hir(child, phase, scope, depth + 1, facts, scope_count)?;
            }
        }
    }
    Ok(())
}

fn insert_binding<'a>(scope: &mut Scope<'a>, binding: &'a ResolvedBinding, mutable: bool) {
    scope.insert(
        &binding.name,
        Binding {
            name: &binding.name,
            id: binding.id.as_str(),
            ty: &binding.ty,
            ownership: binding.ownership,
            mutable,
        },
    );
}

fn pattern_bindings<'a>(
    pattern: &'a ResolvedMatchPattern,
    scope: &mut Scope<'a>,
    depth: usize,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(limit("expression match binding depth exceeds its bound"));
    }
    match pattern {
        ResolvedMatchPattern::Binding(binding) => insert_binding(scope, binding, false),
        ResolvedMatchPattern::Variant { fields, .. } => {
            for field in fields {
                insert_binding(scope, &field.binding, false);
            }
        }
        ResolvedMatchPattern::Record { fields, .. } => {
            for field in fields {
                record_bindings(&field.pattern, scope, depth + 1)?;
            }
        }
        ResolvedMatchPattern::Or(alternatives) => {
            // Verified Or patterns contain literals only; rejecting bindings
            // avoids inventing one alternative's lexical identity as shared.
            if alternatives
                .iter()
                .any(|item| !matches!(item, ResolvedMatchPattern::Literal(_)))
            {
                return Err(invalid(
                    "expression catalogue cannot authenticate Or-pattern bindings",
                ));
            }
        }
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
    }
    Ok(())
}

fn record_bindings<'a>(
    pattern: &'a ResolvedRecordMatchFieldPattern,
    scope: &mut Scope<'a>,
    depth: usize,
) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(limit("expression record binding depth exceeds its bound"));
    }
    match pattern {
        ResolvedRecordMatchFieldPattern::Binding(binding) => insert_binding(scope, binding, false),
        ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
            for field in fields {
                record_bindings(&field.pattern, scope, depth + 1)?;
            }
        }
        ResolvedRecordMatchFieldPattern::Wildcard => {}
    }
    Ok(())
}

fn ast_facts(function: &ast::Function) -> Result<Vec<AstFact<'_>>> {
    let mut facts = Vec::new();
    for (index, root) in function.requires.iter().enumerate() {
        visit_ast(root, "requires", &mut vec![index], &mut facts)?;
    }
    visit_ast(&function.body, "body", &mut Vec::new(), &mut facts)?;
    for (index, root) in function.ensures.iter().enumerate() {
        visit_ast(root, "ensures", &mut vec![index], &mut facts)?;
    }
    Ok(facts)
}

fn visit_ast<'a>(
    expression: &'a Expr,
    phase: &'static str,
    path: &mut Vec<usize>,
    facts: &mut Vec<AstFact<'a>>,
) -> Result<()> {
    if path.len() > MAX_DEPTH || facts.len() >= MAX_EXPRESSIONS {
        return Err(limit("expression source AST exceeds catalogue bounds"));
    }
    facts.push(AstFact {
        expression,
        phase,
        path: path.clone(),
    });
    for (index, child) in ast_children(expression).into_iter().enumerate() {
        path.push(index);
        visit_ast(child, phase, path, facts)?;
        path.pop();
    }
    Ok(())
}

fn ast_at_mut<'a>(expression: &'a mut Expr, path: &[usize]) -> Result<&'a mut Expr> {
    if path.is_empty() {
        return Ok(expression);
    }
    let child = ast_children_mut(expression)
        .into_iter()
        .nth(path[0])
        .ok_or_else(|| invalid("authenticated expression AST path is unavailable"))?;
    ast_at_mut(child, &path[1..])
}

fn ast_children(expression: &Expr) -> Vec<&Expr> {
    match &expression.kind {
        ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => args.iter().collect(),
        ExprKind::MethodCall { receiver, args, .. } => {
            std::iter::once(receiver.as_ref()).chain(args).collect()
        }
        ExprKind::Unary { value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Project { base: value, .. } => vec![value],
        ExprKind::Binary { left, right, .. } => vec![left, right],
        ExprKind::Block { statements, tail } => {
            let mut children = Vec::new();
            for statement in statements {
                for index in 0..statement.child_count() {
                    children.push(statement.child(index).expect("declared AST child"));
                }
            }
            children.push(tail);
            children
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => vec![condition, then_branch, else_branch],
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            fields.iter().map(|field| &field.value).collect()
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            let mut children = vec![scrutinee.as_ref()];
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    children.push(guard);
                }
                children.push(&arm.value);
            }
            children
        }
        ExprKind::UpdateRecord { base, fields } => std::iter::once(base.as_ref())
            .chain(fields.iter().map(|field| &field.value))
            .collect(),
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => Vec::new(),
    }
}

fn ast_children_mut(expression: &mut Expr) -> Vec<&mut Expr> {
    match &mut expression.kind {
        ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
            args.iter_mut().collect()
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            std::iter::once(receiver.as_mut()).chain(args).collect()
        }
        ExprKind::Unary { value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Project { base: value, .. } => vec![value],
        ExprKind::Binary { left, right, .. } => vec![left, right],
        ExprKind::Block { statements, tail } => {
            let mut children = Vec::new();
            for statement in statements {
                match statement {
                    Statement::Let { value, .. } | Statement::Assign { value, .. } => {
                        children.push(value)
                    }
                    Statement::Unsafe { body, .. } => children.push(body.as_mut()),
                    Statement::While {
                        condition, body, ..
                    } => {
                        children.push(condition.as_mut());
                        children.push(body.as_mut());
                    }
                }
            }
            children.push(tail);
            children
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => vec![condition, then_branch, else_branch],
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            fields.iter_mut().map(|field| &mut field.value).collect()
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            let mut children = vec![scrutinee.as_mut()];
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    children.push(guard.as_mut());
                }
                children.push(&mut arm.value);
            }
            children
        }
        ExprKind::UpdateRecord { base, fields } => std::iter::once(base.as_mut())
            .chain(fields.iter_mut().map(|field| &mut field.value))
            .collect(),
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => Vec::new(),
    }
}

fn ast_kind(kind: &ExprKind) -> &'static str {
    match kind {
        ExprKind::Int(_) => "i64",
        ExprKind::Int32(_) => "i32",
        ExprKind::Char(_) => "char",
        ExprKind::Uint8(_) => "u8",
        ExprKind::Usize(_) => "usize",
        ExprKind::ArrayU8(_) => "array_u8",
        ExprKind::RepeatArrayU8 { .. } => "repeat_array_u8",
        ExprKind::Float32(_) => "f32",
        ExprKind::Float64(_) => "f64",
        ExprKind::Bool(_) => "bool",
        ExprKind::String(_) => "string",
        ExprKind::Var(_) => "place",
        ExprKind::Call { .. } => "call",
        ExprKind::MethodCall { .. } => "method_call",
        ExprKind::SuperMethod { .. } => "super_method",
        ExprKind::Unary { .. } => "unary",
        ExprKind::Binary { .. } => "binary",
        ExprKind::Block { .. } => "block",
        ExprKind::If { .. } => "if",
        ExprKind::ConstructRecord { .. } => "record",
        ExprKind::ConstructVariant { .. } => "variant",
        ExprKind::Match { .. } => "match",
        ExprKind::Try { .. } => "try",
        ExprKind::UpdateRecord { .. } => "record_update",
        ExprKind::Project { .. } => "project",
    }
}

fn hir_kind(kind: &ResolvedExprKind) -> &'static str {
    match kind {
        ResolvedExprKind::Int(_) => "i64",
        ResolvedExprKind::Int32(_) => "i32",
        ResolvedExprKind::Char(_) => "char",
        ResolvedExprKind::Uint8(_) => "u8",
        ResolvedExprKind::Usize(_) => "usize",
        ResolvedExprKind::ArrayU8(_) => "array_u8",
        ResolvedExprKind::RepeatArrayU8 { .. } => "repeat_array_u8",
        ResolvedExprKind::Float32(_) => "f32",
        ResolvedExprKind::Float64(_) => "f64",
        ResolvedExprKind::Bool(_) => "bool",
        ResolvedExprKind::String(_) => "string",
        ResolvedExprKind::Place(_) => "place",
        ResolvedExprKind::Call { .. } => "call",
        ResolvedExprKind::BorrowPlace { .. } => "borrow_place",
        ResolvedExprKind::ByteRange { .. } => "byte_range",
        ResolvedExprKind::NativeRustImportCall(_) => "native_import_call",
        ResolvedExprKind::HostCommandCall(_) => "host_command_call",
        ResolvedExprKind::Unary { .. } => "unary",
        ResolvedExprKind::Binary { .. } => "binary",
        ResolvedExprKind::Block { .. } => "block",
        ResolvedExprKind::If { .. } => "if",
        ResolvedExprKind::ConstructRecord { .. } => "record",
        ResolvedExprKind::ConstructVariant { .. } => "variant",
        ResolvedExprKind::Match { .. } => "match",
        ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => "try",
        ResolvedExprKind::UpdateRecord { .. } => "record_update",
        ResolvedExprKind::Project { .. } => "project",
        ResolvedExprKind::Upcast { .. } => "upcast",
    }
}

fn ownership(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}
fn binding_json(binding: &Binding<'_>) -> Value {
    json!({"name":binding.name,"value_id":binding.id,"type":binding.ty.identity_key(),"ownership":ownership(binding.ownership),"mutable":binding.mutable})
}
fn span_json(span: Span) -> Value {
    json!({"start":span.start,"end":span.end,"line":span.line,"column":span.column})
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("expression intention requires a text field"))
}
fn region_admitted(phase: &str, contract: bool) -> bool {
    if contract {
        matches!(phase, "requires" | "ensures")
    } else {
        phase == "body"
    }
}

fn region_at_mut<'a>(
    function: &'a mut ast::Function,
    phase: &str,
    path: &[usize],
) -> Result<&'a mut Expr> {
    if phase == "body" {
        return ast_at_mut(&mut function.body, path);
    }
    let (ordinal, children) = path
        .split_first()
        .ok_or_else(|| invalid("contract expression has no predicate ordinal"))?;
    let predicates = match phase {
        "requires" => &mut function.requires,
        "ensures" => &mut function.ensures,
        _ => return Err(invalid("unknown authenticated expression region")),
    };
    let root = predicates
        .get_mut(*ordinal)
        .ok_or_else(|| invalid("contract predicate ordinal is unavailable"))?;
    ast_at_mut(root, children)
}

fn validate_request(value: &Value) -> Result<()> {
    validate_region_request(value, false)
}

fn validate_region_request(value: &Value, contract: bool) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("expression intention must be an object"))?;
    if object.len() != 4
        || ["kind", "target", "expression_id", "replacement"]
            .iter()
            .any(|key| !object.contains_key(*key))
        || text(value, "kind")?
            != if contract {
                "replace_contract_expression"
            } else {
                "replace_expression"
            }
    {
        return Err(invalid(
            "expression intention has missing or unknown fields",
        ));
    }
    Ok(())
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}
fn limit(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G226", message)]
}

#[cfg(test)]
mod extraction_audit_tests {
    use super::*;

    #[test]
    fn extraction_does_not_escape_an_enclosing_unsafe_statement() {
        let source = r#"module audit.test;
permit { unsafe }
@id("audit.function")
fn audited(value: i64) -> i64 {
    @audit("keep this audit owner") unsafe { value + 1 }
    value
}
"#;
        let program = crate::parse(source, std::path::Path::new("audit.spx")).unwrap();
        let function = &program.functions[0];
        let facts = ast_facts(function).unwrap();
        let inside = facts
            .iter()
            .find(|fact| {
                source.get(fact.expression.span.start..fact.expression.span.end)
                    == Some("value + 1")
            })
            .unwrap();
        let errors = reject_unsafe_ancestor(&function.body, &inside.path).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-G225"));
        let tail = facts
            .iter()
            .find(|fact| {
                source.get(fact.expression.span.start..fact.expression.span.end) == Some("value")
                    && fact.expression.span.start > inside.expression.span.end
            })
            .unwrap();
        reject_unsafe_ancestor(&function.body, &tail.path).unwrap();
    }
}
