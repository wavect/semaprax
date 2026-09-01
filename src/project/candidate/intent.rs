//! Closed source-independent intentions over an already authenticated AST set.
//!
//! This is candidate construction, never admission: the caller must format,
//! reparse, rebuild and verify the complete Project before exposing a result.
//! Legacy append migration preserves each existing argument in place. Ordered
//! Copy-parameter mapping stages every original argument left-to-right. No
//! type-conversion guess is performed; names resolve through stable IDs.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::ast::{
    BinaryOp, Expr, ExprKind, FieldInitializer, Function, MatchArm, MatchMode, MatchPattern,
    MatchPatternField, ModuleUseKind, Param, ParamMode, Program, Span, Statement, Type,
    TypeDeclarationKind, UnaryOp,
};
use crate::diagnostic::Diagnostic;

pub(super) const MAX_NAME_BYTES: usize = 128;
pub(super) const MAX_ID_BYTES: usize = 4096;
pub(super) const MAX_APPEND_PARAMETERS: usize = 16;
pub(super) const MAX_EXPRESSION_DEPTH: usize = 64;
pub(super) const MAX_EXPRESSION_NODES: usize = 4096;
pub(super) const MAX_STRING_LITERAL_BYTES: usize = 16_384;
const MAX_WALK_DEPTH: usize = 256;
const MAX_WALK_NODES: usize = 1_048_576;

#[path = "aggregate.rs"]
mod aggregate;
#[path = "builtin.rs"]
mod builtin;
#[path = "field_place.rs"]
mod field_place;
#[path = "signature.rs"]
mod signature;

pub(super) use aggregate::{
    aggregate_constructors, aggregate_dependency_fingerprint,
    aggregate_match_dependency_fingerprint, aggregate_matches,
    aggregate_projection_dependency_fingerprint, aggregate_projections, aggregate_updates,
    field_place_dependency_fingerprint, field_places, nominal_type_dependency_fingerprint,
    nominal_type_plan, nominal_types, validate_nominal_ast, MAX_AGGREGATE_TYPE_ARGUMENTS,
};
pub(super) use builtin::{
    builtin_constructors, builtin_dependency_fingerprint, by_id as builtin_operation_by_id,
    validate_builtin_namespace, BuiltinOp,
};
pub(super) use field_place::{parameter_nominal_scope, NominalScope};
pub(super) use signature::{ordered_signature_parameters, validate_computed_signature};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub(super) struct IntentSummary {
    pub(super) target_id: String,
    pub(super) kind: String,
    pub(super) migrated_calls: usize,
}

/// Mutates invocation-local candidate ASTs only. On failure the caller discards
/// them; this internal routine supplies neither rollback nor source authority.
#[cfg(test)]
pub(super) fn apply(programs: &mut [Program], intent: &Value) -> Result<IntentSummary> {
    apply_inner(None, programs, intent)
}

pub(super) fn apply_with_revision(
    revision: &crate::project::ProjectRevision,
    programs: &mut [Program],
    intent: &Value,
) -> Result<IntentSummary> {
    apply_inner(Some(revision), programs, intent)
}

fn apply_inner(
    revision: Option<&crate::project::ProjectRevision>,
    programs: &mut [Program],
    intent: &Value,
) -> Result<IntentSummary> {
    let kind = text(intent, "kind")?;
    let target = text(intent, "target")?;
    if target.is_empty() || target.len() > MAX_ID_BYTES {
        return Err(grammar(
            "candidate intention target is not a bounded stable ID",
        ));
    }
    let mut selected = None;
    for (program_index, program) in programs.iter().enumerate() {
        for (function_index, function) in program.functions.iter().enumerate() {
            if function.stable_id == target
                && selected.replace((program_index, function_index)).is_some()
            {
                return Err(grammar("candidate intention target is ambiguous"));
            }
        }
    }
    let (owner, function_index) = selected
        .ok_or_else(|| grammar("candidate intention requires an existing top-level function"))?;
    let function = &programs[owner].functions[function_index];
    if !function.explicit_id || !function.type_parameters.is_empty() || function.name == "main" {
        return Err(grammar(
            "candidate intention requires an explicit monomorphic non-main function",
        ));
    }
    let owner_module = programs[owner].module.clone();
    let old_name = function.name.clone();
    let mut migrated_calls = 0;
    match kind {
        "rename_declaration" => {
            object(intent, &["kind", "target", "name"])?;
            let name = identifier(text(intent, "name")?)?;
            if name == old_name {
                return Err(grammar(
                    "candidate declaration rename must change its display name",
                ));
            }
            let bindings = call_bindings(&programs[owner])?;
            if bindings.contains_key(name) {
                return Err(grammar(
                    "candidate declaration name conflicts with a call binding",
                ));
            }
            let mut nodes = 0;
            walk_program(&mut programs[owner], &mut nodes, &mut |expression| {
                if let ExprKind::Call { name: call, .. } = &mut expression.kind {
                    if bindings.get(call).is_some_and(|id| id == target) {
                        *call = name.to_owned();
                        migrated_calls += 1;
                    }
                }
                Ok(())
            })?;
            programs[owner].functions[function_index].name = name.to_owned();
        }
        "change_function_signature" if intent.get("parameters").is_some() => {
            migrated_calls = signature::apply(revision, programs, intent, owner, function_index)?;
        }
        "change_function_signature" => {
            object(intent, &["kind", "target", "append_parameters"])?;
            let additions = array(intent, "append_parameters")?;
            if additions.is_empty() || additions.len() > MAX_APPEND_PARAMETERS {
                return Err(capacity(
                    "candidate signature requires one to sixteen appended parameters",
                ));
            }
            let old_arity = function.params.len();
            let mut names = function
                .params
                .iter()
                .map(|p| p.name.clone())
                .collect::<BTreeSet<_>>();
            let mut params = Vec::with_capacity(additions.len());
            let mut arguments = Vec::with_capacity(additions.len());
            for addition in additions {
                object(addition, &["name", "type", "argument"])?;
                let name = identifier(text(addition, "name")?)?;
                if !names.insert(name.to_owned()) {
                    return Err(grammar(
                        "candidate signature parameter names must remain unique",
                    ));
                }
                let ty = scalar_type(text(addition, "type")?)?;
                let argument = member(addition, "argument")?;
                if text(argument, "kind")? != text(addition, "type")? {
                    return Err(grammar(
                        "appended argument must be an exact typed scalar literal",
                    ));
                }
                let expression = literal(argument)?;
                params.push(Param {
                    name: name.to_owned(),
                    mode: ParamMode::Value,
                    ty,
                    span: Span::default(),
                });
                arguments.push(expression);
            }
            let mut nodes = 0;
            for program in programs.iter_mut() {
                // Existing imports select both provider identity and module;
                // an alias is never inferred from a provider's display name.
                for import in &program.module_uses {
                    if import.kind == ModuleUseKind::Function
                        && import.persistent_id == target
                        && import.target_module != owner_module
                    {
                        return Err(grammar(
                            "candidate call provider module does not match its stable ID",
                        ));
                    }
                }
                let bindings = call_bindings(program)?;
                walk_program(program, &mut nodes, &mut |expression| {
                    if let ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } = &mut expression.kind
                    {
                        if bindings.get(name).is_some_and(|id| id == target) {
                            if !type_arguments.is_empty() || args.len() != old_arity {
                                return Err(grammar(
                                    "candidate call migration has an unsupported signature",
                                ));
                            }
                            args.extend(arguments.iter().cloned());
                            migrated_calls += 1;
                        }
                    }
                    Ok(())
                })?;
            }
            programs[owner].functions[function_index]
                .params
                .extend(params);
        }
        "replace_function_body" => {
            object(intent, &["kind", "target", "body"])?;
            let bindings = call_bindings(&programs[owner])?;
            let params = function
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect::<BTreeSet<_>>();
            let mut constructor = Constructor {
                bindings: &bindings,
                params: &params,
                nodes: 0,
                next_projection: 0,
                arm_bindings: BTreeSet::new(),
                reserved_bindings: BTreeSet::new(),
                generated_bindings: BTreeSet::new(),
                builtin_identities: None,
                nominal_scope: match revision {
                    Some(revision) => field_place::parameter_nominal_scope(
                        revision,
                        &programs[owner],
                        &function.params,
                        member(intent, "body")?,
                    )?,
                    None => BTreeMap::new(),
                },
                field_enabled: field_place::requested(member(intent, "body")?),
                field_work: 0,
                revision,
                program: &programs[owner],
            };
            let body = constructor.expression(member(intent, "body")?, 0)?;
            programs[owner].functions[function_index].body = body;
        }
        "add_contract" => {
            object(intent, &["kind", "target", "phase", "predicate"])?;
            let phase = text(intent, "phase")?;
            if !matches!(phase, "requires" | "ensures") {
                return Err(grammar("contract phase must be requires or ensures"));
            }
            if function
                .requires
                .len()
                .saturating_add(function.ensures.len())
                >= 1024
            {
                return Err(capacity("candidate contract inventory exceeds its limit"));
            }
            let mut places = function
                .params
                .iter()
                .map(|p| p.name.clone())
                .collect::<BTreeSet<_>>();
            if phase == "ensures" {
                places.insert("result".to_owned());
            }
            let mut nominal_scope = match revision {
                Some(revision) => parameter_nominal_scope(
                    revision,
                    &programs[owner],
                    &function.params,
                    member(intent, "predicate")?,
                )?,
                None => BTreeMap::new(),
            };
            if phase == "ensures" && field_place::requested(member(intent, "predicate")?) {
                if let Some(revision) = revision {
                    field_place::insert_ast_type(
                        revision,
                        &programs[owner],
                        &mut nominal_scope,
                        "result",
                        &function.return_type,
                    )?;
                }
            }
            let predicate = construct_expression_inner(
                revision,
                &programs[owner],
                &places,
                nominal_scope,
                member(intent, "predicate")?,
            )?;
            let function = &mut programs[owner].functions[function_index];
            // Append exactly one predicate. Never alter, delete, reorder or
            // infer a replacement for an existing contract.
            if phase == "requires" {
                function.requires.push(predicate);
            } else {
                function.ensures.push(predicate);
            }
        }
        _ => return Err(grammar("unsupported candidate intention kind")),
    }
    Ok(IntentSummary {
        target_id: target.to_owned(),
        kind: kind.to_owned(),
        migrated_calls,
    })
}

pub(super) fn uses_field_places(value: &Value) -> bool {
    field_place::requested(value)
}

/// Construct an expression from a caller-authenticated lexical scope. This
/// helper grants no admission: complete source replay owns all type, effect,
/// ownership, contract and target checks.
pub(super) fn construct_expression_with_scope(
    revision: &crate::project::ProjectRevision,
    program: &Program,
    scope_names: &BTreeSet<String>,
    nominal_scope: NominalScope,
    value: &Value,
) -> Result<Expr> {
    construct_expression_inner(Some(revision), program, scope_names, nominal_scope, value)
}

pub(super) fn insert_nominal_type(
    revision: &crate::project::ProjectRevision,
    program: &Program,
    scope: &mut NominalScope,
    name: &str,
    ty: &Type,
) -> Result<()> {
    field_place::insert_ast_type(revision, program, scope, name, ty)
}

fn construct_expression_inner(
    revision: Option<&crate::project::ProjectRevision>,
    program: &Program,
    scope_names: &BTreeSet<String>,
    nominal_scope: NominalScope,
    value: &Value,
) -> Result<Expr> {
    let bindings = call_bindings(program)?;
    let params = scope_names
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    Constructor {
        bindings: &bindings,
        params: &params,
        nodes: 0,
        next_projection: 0,
        arm_bindings: BTreeSet::new(),
        reserved_bindings: BTreeSet::new(),
        generated_bindings: BTreeSet::new(),
        builtin_identities: None,
        nominal_scope,
        field_enabled: field_place::requested(value),
        field_work: 0,
        revision,
        program,
    }
    .expression(value, 0)
}

pub(super) fn call_bindings(program: &Program) -> Result<BTreeMap<String, String>> {
    let mut bindings = BTreeMap::new();
    for (name, id) in program
        .functions
        .iter()
        .map(|f| (&f.name, &f.stable_id))
        .chain(
            program
                .module_uses
                .iter()
                .filter(|u| u.kind == ModuleUseKind::Function)
                .map(|u| (&u.alias, &u.persistent_id)),
        )
    {
        if bindings.insert(name.clone(), id.clone()).is_some() {
            return Err(grammar("candidate function bindings are ambiguous"));
        }
    }
    Ok(bindings)
}

struct Constructor<'a> {
    bindings: &'a BTreeMap<String, String>,
    params: &'a BTreeSet<&'a str>,
    nodes: usize,
    next_projection: usize,
    arm_bindings: BTreeSet<String>,
    reserved_bindings: BTreeSet<String>,
    generated_bindings: BTreeSet<String>,
    builtin_identities: Option<BTreeSet<String>>,
    nominal_scope: NominalScope,
    field_enabled: bool,
    field_work: usize,
    revision: Option<&'a crate::project::ProjectRevision>,
    program: &'a Program,
}

struct PreparedMatchArm<'a> {
    case_name: String,
    fields: Vec<MatchPatternField>,
    bindings: Vec<String>,
    body: &'a Value,
}

impl Constructor<'_> {
    fn infer_nominal(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<std::sync::Arc<crate::hir::ResolvedType>>> {
        if !self.field_enabled {
            return Ok(None);
        }
        let revision = self
            .revision
            .ok_or_else(|| grammar("field places require an authenticated Project revision"))?;
        field_place::infer(
            revision,
            self.program,
            self.bindings,
            &self.nominal_scope,
            expression,
            &mut self.field_work,
            0,
        )
    }
    fn match_binder(&self, name: &str) -> Result<()> {
        identifier(name)?;
        if matches!(
            name,
            "_" | "record"
                | "variant"
                | "class"
                | "resource"
                | "type"
                | "protocol"
                | "impl"
                | "for"
                | "extends"
        ) || self.params.contains(name)
            || self.arm_bindings.contains(name)
            || self.generated_bindings.contains(name)
            || self.bindings.contains_key(name)
            || self
                .program
                .module_uses
                .iter()
                .any(|binding| binding.alias == name)
            || self.program.types.iter().any(|ty| ty.name == name)
            || matches!(name, "Option" | "Result")
        {
            return Err(grammar("match payload binder is reserved or collides with an existing lexical, call, type or generated binding"));
        }
        Ok(())
    }

    fn match_expression(&mut self, value: &Value, depth: usize) -> Result<Expr> {
        if value.get("type_arguments").is_some() {
            object(
                value,
                &["kind", "target", "value", "arms", "type_arguments"],
            )?;
        } else {
            object(value, &["kind", "target", "value", "arms"])?;
        }
        // The already charged root becomes a block; count its generated let,
        // match and scrutinee variable as additional AST nodes.
        self.nodes += 3;
        if self.nodes > MAX_EXPRESSION_NODES || depth + 2 > MAX_EXPRESSION_DEPTH {
            return Err(capacity("match lowering exceeds its node or depth bound"));
        }
        if let Some(arguments) = value.get("type_arguments") {
            let arguments = arguments
                .as_array()
                .ok_or_else(|| grammar("aggregate type_arguments must be an explicit array"))?;
            if arguments.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
                || arguments.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes)
            {
                return Err(capacity(
                    "match type arguments exceed the remaining node bound",
                ));
            }
            self.nodes += arguments.len();
        }
        let revision = self
            .revision
            .ok_or_else(|| grammar("match requires a retained checked Project revision"))?;
        let plan = aggregate::match_plan(
            revision,
            self.program,
            text(value, "target")?,
            value.get("type_arguments"),
        )?;
        let requested = array(value, "arms")?;
        if requested.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes) {
            return Err(capacity("match arms exceed the remaining node bound"));
        }
        if requested.len() != plan.cases.len() {
            return Err(grammar("match must cover every exact variant case once"));
        }
        let mut seen = BTreeSet::new();
        let mut prepared = Vec::new();
        for arm in requested {
            object(arm, &["target", "fields", "body"])?;
            let target = text(arm, "target")?;
            let case = plan
                .cases
                .get(target)
                .ok_or_else(|| grammar("match selects a foreign variant case"))?;
            if !seen.insert(target) {
                return Err(grammar("match repeats a variant case"));
            }
            let requested_fields = array(arm, "fields")?;
            let charge = 1usize.saturating_add(requested_fields.len());
            if charge > MAX_EXPRESSION_NODES.saturating_sub(self.nodes) {
                return Err(capacity(
                    "match patterns and payload bindings exceed the node bound",
                ));
            }
            self.nodes += charge;
            if requested_fields.len() != case.fields.len() {
                return Err(grammar(
                    "match must bind every exact case payload field once",
                ));
            }
            let mut field_ids = BTreeSet::new();
            let mut names = BTreeSet::new();
            let mut fields = Vec::new();
            let mut bindings = Vec::new();
            for field in requested_fields {
                object(field, &["target", "name"])?;
                let target = text(field, "target")?;
                let field_name = case
                    .fields
                    .get(target)
                    .ok_or_else(|| grammar("match selects a foreign case payload field"))?;
                if !field_ids.insert(target) {
                    return Err(grammar("match repeats a case payload field"));
                }
                let name = text(field, "name")?;
                self.match_binder(name)?;
                if !names.insert(name) {
                    return Err(grammar(
                        "match payload binders must be unique within their arm",
                    ));
                }
                bindings.push(name.to_owned());
                fields.push(MatchPatternField {
                    name: field_name.clone(),
                    name_span: Span::default(),
                    binding: name.to_owned(),
                    binding_span: Span::default(),
                    span: Span::default(),
                });
            }
            // Reservation affects generated names only, never place lookup.
            // Thus sibling-arm binders cannot become visible to expressions.
            self.reserved_bindings.extend(bindings.iter().cloned());
            prepared.push(PreparedMatchArm {
                case_name: case.name.clone(),
                fields,
                bindings,
                body: member(arm, "body")?,
            });
        }
        let name = self.projection_name()?;
        // No arm names are active while the original scrutinee is constructed.
        let scrutinee = self.expression(member(value, "value")?, depth + 2)?;
        let scrutinee_type = self.infer_nominal(&scrutinee)?;
        let mut arms = Vec::new();
        for arm in prepared {
            for binding in &arm.bindings {
                self.arm_bindings.insert(binding.clone());
            }
            if let Some(root) = &scrutinee_type {
                for field in &arm.fields {
                    if let Some(ty) = field_place::field_type(
                        revision,
                        root,
                        Some(&arm.case_name),
                        &field.name,
                        &mut self.field_work,
                    )? {
                        self.nominal_scope.insert(field.binding.clone(), ty);
                    }
                }
            }
            let body = self.expression(arm.body, depth + 2);
            for binding in &arm.bindings {
                self.arm_bindings.remove(binding);
                self.nominal_scope.remove(binding);
            }
            arms.push(MatchArm {
                pattern: MatchPattern::Variant {
                    type_name: plan.type_name.clone(),
                    type_span: Span::default(),
                    case_name: arm.case_name,
                    case_span: Span::default(),
                    fields: arm.fields,
                    span: Span::default(),
                },
                guard: None,
                value: body?,
                span: Span::default(),
            });
        }
        Ok(Expr {
            kind: ExprKind::Block {
                statements: vec![Statement::Let {
                    name: name.clone(),
                    name_span: Span::default(),
                    mutable: false,
                    declared: Some(plan.owner_type),
                    value: scrutinee,
                    span: Span::default(),
                }],
                tail: Box::new(Expr {
                    kind: ExprKind::Match {
                        mode: MatchMode::Value,
                        scrutinee: Box::new(Expr {
                            kind: ExprKind::Var(name),
                            span: Span::default(),
                        }),
                        arms,
                    },
                    span: Span::default(),
                }),
            },
            span: Span::default(),
        })
    }

    fn projection_name(&mut self) -> Result<String> {
        // Every attempt either consumes one occupied source/reserved lexical
        // name or selects a new monotonically increasing generated name.
        // Match payload and let names are reserved before constructing their
        // values; nested staging locals cannot capture those bindings.
        let limit = self
            .params
            .len()
            .saturating_add(self.bindings.len())
            .saturating_add(self.program.module_uses.len())
            .saturating_add(self.program.types.len())
            .saturating_add(self.reserved_bindings.len())
            .saturating_add(MAX_EXPRESSION_NODES);
        while self.next_projection <= limit {
            let name = format!("spx_project_{}", self.next_projection);
            self.next_projection += 1;
            if !self.params.contains(name.as_str())
                && !self.reserved_bindings.contains(&name)
                && !self.bindings.contains_key(&name)
                && !self
                    .program
                    .module_uses
                    .iter()
                    .any(|binding| binding.alias == name)
                && !self.program.types.iter().any(|ty| ty.name == name)
            {
                self.generated_bindings.insert(name.clone());
                return Ok(name);
            }
        }
        Err(capacity(
            "projection temporary name inventory exceeds its bound",
        ))
    }

    fn expression(&mut self, value: &Value, depth: usize) -> Result<Expr> {
        self.nodes += 1;
        if depth > MAX_EXPRESSION_DEPTH || self.nodes > MAX_EXPRESSION_NODES {
            return Err(capacity(
                "candidate expression constructor exceeds its depth or node bound",
            ));
        }
        let kind = match text(value, "kind")? {
            "i64" | "i32" | "u8" | "usize" | "bool" | "char" | "f32" | "f64" => {
                let expression = literal(value)?;
                let added = literal_nodes(&expression).saturating_sub(1);
                if added != 0 {
                    self.nodes = self.nodes.saturating_add(added);
                    if self.nodes > MAX_EXPRESSION_NODES || depth + 1 > MAX_EXPRESSION_DEPTH {
                        return Err(capacity(
                            "signed float literal exceeds the expression node or depth bound",
                        ));
                    }
                }
                return Ok(expression);
            }
            "string" => {
                object(value, &["kind", "value"])?;
                let contents = text(value, "value")?;
                if contents.len() > MAX_STRING_LITERAL_BYTES {
                    return Err(capacity(
                        "candidate string literal exceeds its UTF-8 byte bound",
                    ));
                }
                ExprKind::String(contents.to_owned())
            }
            "array_u8" => {
                object(value, &["kind", "values"])?;
                let values = array(value, "values")?;
                // The root is already charged. Literal payload entries share
                // the same budget with every surrounding constructor node.
                if values.len() > MAX_EXPRESSION_NODES - self.nodes {
                    return Err(capacity(
                        "candidate byte-array literal exceeds the expression node bound",
                    ));
                }
                self.nodes += values.len();
                let bytes = values
                    .iter()
                    .map(|item| {
                        item.as_u64()
                            .and_then(|number| u8::try_from(number).ok())
                            .ok_or_else(|| {
                                grammar("candidate byte-array literal requires integer bytes")
                            })
                    })
                    .collect::<Result<Vec<_>>>()?;
                ExprKind::ArrayU8(bytes)
            }
            "place" => {
                object(value, &["kind", "name"])?;
                let name = identifier(text(value, "name")?)?;
                if !self.params.contains(name) && !self.arm_bindings.contains(name) {
                    return Err(grammar(
                        "candidate place must identify an existing parameter",
                    ));
                }
                ExprKind::Var(name.to_owned())
            }
            "let" => {
                object(value, &["kind", "name", "value", "body"])?;
                let name = text(value, "name")?;
                self.match_binder(name)?;
                // The charged expression root becomes a block. Its inferred,
                // immutable let statement is one additional AST node.
                self.nodes += 1;
                if self.nodes > MAX_EXPRESSION_NODES || depth + 1 > MAX_EXPRESSION_DEPTH {
                    return Err(capacity("let lowering exceeds its node or depth bound"));
                }
                // Reserve before the initializer, but activate only for the
                // body. Initializer staging must not capture the future name.
                self.reserved_bindings.insert(name.to_owned());
                let initializer = self.expression(member(value, "value")?, depth + 1)?;
                let nominal = self.infer_nominal(&initializer)?;
                let body_request = member(value, "body")?;
                self.arm_bindings.insert(name.to_owned());
                if let Some(ty) = nominal {
                    self.nominal_scope.insert(name.to_owned(), ty);
                }
                let body = self.expression(body_request, depth + 1);
                self.arm_bindings.remove(name);
                self.nominal_scope.remove(name);
                ExprKind::Block {
                    statements: vec![Statement::Let {
                        name: name.to_owned(),
                        name_span: Span::default(),
                        mutable: false,
                        declared: None,
                        value: initializer,
                        span: Span::default(),
                    }],
                    tail: Box::new(body?),
                }
            }
            "match" => return self.match_expression(value, depth),
            "field_place" => {
                object(value, &["kind", "target", "root"])?;
                let root = identifier(text(value, "root")?)?;
                if !self.params.contains(root) && !self.arm_bindings.contains(root) {
                    return Err(grammar(
                        "field place root must be an existing lexical named binding",
                    ));
                }
                let ty = self.nominal_scope.get(root).ok_or_else(|| {
                    grammar("field place root has no exact authenticated nominal type fact")
                })?;
                let revision = self.revision.ok_or_else(|| {
                    grammar("field places require an authenticated Project revision")
                })?;
                let (field, _) = aggregate::field_place_plan(
                    revision,
                    self.program,
                    text(value, "target")?,
                    ty,
                )?;
                self.nodes += 1;
                if self.nodes > MAX_EXPRESSION_NODES || depth + 1 > MAX_EXPRESSION_DEPTH {
                    return Err(capacity(
                        "field place projection exceeds its node or depth bound",
                    ));
                }
                ExprKind::Project {
                    base: Box::new(Expr {
                        kind: ExprKind::Var(root.to_owned()),
                        span: Span::default(),
                    }),
                    field,
                    field_span: Span::default(),
                }
            }
            "project" => {
                if value.get("type_arguments").is_some() {
                    object(value, &["kind", "target", "base", "type_arguments"])?;
                } else {
                    object(value, &["kind", "target", "base"])?;
                }
                // The wire node becomes a block. Charge the generated let
                // statement, projection and variable as three further nodes.
                self.nodes += 3;
                if self.nodes > MAX_EXPRESSION_NODES || depth + 2 > MAX_EXPRESSION_DEPTH {
                    return Err(capacity(
                        "projection lowering exceeds its node or depth bound",
                    ));
                }
                if let Some(arguments) = value.get("type_arguments") {
                    let arguments = arguments.as_array().ok_or_else(|| {
                        grammar("aggregate type_arguments must be an explicit array")
                    })?;
                    if arguments.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
                        || arguments.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes)
                    {
                        return Err(capacity(
                            "projection type arguments exceed the remaining node bound",
                        ));
                    }
                    self.nodes += arguments.len();
                }
                let revision = self.revision.ok_or_else(|| {
                    grammar("projection requires a retained checked Project revision")
                })?;
                let plan = aggregate::projection_plan(
                    revision,
                    self.program,
                    text(value, "target")?,
                    value.get("type_arguments"),
                )?;
                let name = self.projection_name()?;
                let base = self.expression(member(value, "base")?, depth + 2)?;
                ExprKind::Block {
                    statements: vec![Statement::Let {
                        name: name.clone(),
                        name_span: Span::default(),
                        mutable: false,
                        declared: Some(plan.owner_type),
                        value: base,
                        span: Span::default(),
                    }],
                    tail: Box::new(Expr {
                        kind: ExprKind::Project {
                            base: Box::new(Expr {
                                kind: ExprKind::Var(name),
                                span: Span::default(),
                            }),
                            field: plan.field_name,
                            field_span: Span::default(),
                        },
                        span: Span::default(),
                    }),
                }
            }
            "update" => {
                if value.get("type_arguments").is_some() {
                    object(
                        value,
                        &["kind", "target", "base", "fields", "type_arguments"],
                    )?;
                } else {
                    object(value, &["kind", "target", "base", "fields"])?;
                }
                // Root block plus generated let, update and base variable.
                self.nodes += 3;
                if self.nodes > MAX_EXPRESSION_NODES || depth + 2 > MAX_EXPRESSION_DEPTH {
                    return Err(capacity("update lowering exceeds its node or depth bound"));
                }
                if let Some(arguments) = value.get("type_arguments") {
                    let arguments = arguments.as_array().ok_or_else(|| {
                        grammar("aggregate type_arguments must be an explicit array")
                    })?;
                    if arguments.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
                        || arguments.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes)
                    {
                        return Err(capacity(
                            "update type arguments exceed the remaining node bound",
                        ));
                    }
                    self.nodes += arguments.len();
                }
                let revision = self.revision.ok_or_else(|| {
                    grammar("update requires a retained checked Project revision")
                })?;
                let plan = aggregate::plan(
                    revision,
                    self.program,
                    "record",
                    text(value, "target")?,
                    value.get("type_arguments"),
                )?;
                let requested = array(value, "fields")?;
                if requested.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes) {
                    return Err(capacity(
                        "update fields exceed the remaining expression node bound",
                    ));
                }
                let mut seen = BTreeSet::new();
                // A subset is intentional, including the source-admitted empty
                // update; membership is checked before constructing any child.
                for field in requested {
                    object(field, &["target", "value"])?;
                    let target = text(field, "target")?;
                    if !plan.fields.contains_key(target) || !seen.insert(target) {
                        return Err(grammar(
                            "update repeats or selects a foreign record field identity",
                        ));
                    }
                }
                let name = self.projection_name()?;
                let base = self.expression(member(value, "base")?, depth + 2)?;
                let mut fields = Vec::with_capacity(requested.len());
                for field in requested {
                    fields.push(FieldInitializer {
                        name: plan.fields[text(field, "target")?].clone(),
                        name_span: Span::default(),
                        value: self.expression(member(field, "value")?, depth + 2)?,
                        span: Span::default(),
                    });
                }
                ExprKind::Block {
                    statements: vec![Statement::Let {
                        name: name.clone(),
                        name_span: Span::default(),
                        mutable: false,
                        declared: Some(Type::Named {
                            name: plan.type_name,
                            arguments: plan.type_arguments,
                        }),
                        value: base,
                        span: Span::default(),
                    }],
                    tail: Box::new(Expr {
                        kind: ExprKind::UpdateRecord {
                            base: Box::new(Expr {
                                kind: ExprKind::Var(name),
                                span: Span::default(),
                            }),
                            fields,
                        },
                        span: Span::default(),
                    }),
                }
            }
            "record" | "variant" => {
                if value.get("type_arguments").is_some() {
                    object(value, &["kind", "target", "fields", "type_arguments"])?;
                } else {
                    object(value, &["kind", "target", "fields"])?;
                }
                let kind = text(value, "kind")?;
                let target = text(value, "target")?;
                let revision = self.revision.ok_or_else(|| {
                    grammar("aggregate constructor requires a retained checked Project revision")
                })?;
                if let Some(arguments) = value.get("type_arguments") {
                    let arguments = arguments.as_array().ok_or_else(|| {
                        grammar("aggregate type_arguments must be an explicit array")
                    })?;
                    if arguments.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
                        || arguments.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes)
                    {
                        return Err(capacity(
                            "aggregate type arguments exceed the remaining constructor node bound",
                        ));
                    }
                    self.nodes += arguments.len();
                }
                let plan = aggregate::plan(
                    revision,
                    self.program,
                    kind,
                    target,
                    value.get("type_arguments"),
                )?;
                let requested = array(value, "fields")?;
                if requested.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes) {
                    return Err(capacity(
                        "aggregate constructor fields exceed the remaining expression node bound",
                    ));
                }
                if requested.len() != plan.fields.len() {
                    return Err(grammar(
                        "aggregate constructor must initialize every exact field once",
                    ));
                }
                let mut seen = BTreeSet::new();
                // Validate the complete inventory before constructing any child.
                for field in requested {
                    object(field, &["target", "value"])?;
                    let target = text(field, "target")?;
                    if !plan.fields.contains_key(target) || !seen.insert(target) {
                        return Err(grammar(
                            "aggregate constructor repeats or selects a foreign field identity",
                        ));
                    }
                }
                let mut fields = Vec::with_capacity(requested.len());
                for field in requested {
                    fields.push(FieldInitializer {
                        name: plan.fields[text(field, "target")?].clone(),
                        name_span: Span::default(),
                        value: self.expression(member(field, "value")?, depth + 1)?,
                        span: Span::default(),
                    });
                }
                if let Some(case_name) = plan.case_name {
                    ExprKind::ConstructVariant {
                        type_name: plan.type_name,
                        type_span: Span::default(),
                        type_arguments: plan.type_arguments,
                        case_name,
                        case_span: Span::default(),
                        fields,
                    }
                } else {
                    ExprKind::ConstructRecord {
                        type_name: plan.type_name,
                        type_span: Span::default(),
                        type_arguments: plan.type_arguments,
                        fields,
                    }
                }
            }
            "builtin_call" => {
                object(value, &["kind", "target", "arguments"])?;
                let target = text(value, "target")?;
                if builtin::by_id(target).is_none() {
                    return Err(grammar(
                        "builtin call target is not an admitted compiler-owned operation",
                    ));
                }
                let revision = self.revision.ok_or_else(|| {
                    grammar("builtin calls require an authenticated Project revision")
                })?;
                if self.builtin_identities.is_none() {
                    self.builtin_identities = Some(builtin::source_identities(revision)?);
                }
                let identities = self
                    .builtin_identities
                    .as_ref()
                    .ok_or_else(|| grammar("builtin source identity inventory is unavailable"))?;
                let op = builtin::plan(identities, self.program, target)?;
                if self.params.contains(op.name())
                    || self.arm_bindings.contains(op.name())
                    || self.generated_bindings.contains(op.name())
                {
                    return Err(grammar(
                        "builtin call spelling is shadowed by a lexical binding",
                    ));
                }
                let arguments = array(value, "arguments")?;
                if arguments.len() != op.arity() {
                    return Err(grammar(
                        "builtin call requires its compiler-owned exact arity",
                    ));
                }
                if arguments.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes) {
                    return Err(capacity(
                        "builtin call arguments exceed the constructor node bound",
                    ));
                }
                let args = arguments
                    .iter()
                    .map(|argument| self.expression(argument, depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                ExprKind::Call {
                    name: op.name().to_owned(),
                    type_arguments: Vec::new(),
                    args,
                }
            }
            "call" => {
                object(value, &["kind", "target", "arguments"])?;
                let target = text(value, "target")?;
                let mut names = self.bindings.iter().filter(|(_, id)| id.as_str() == target);
                let name = names.next().map(|(name, _)| name.clone()).ok_or_else(|| {
                    grammar("candidate call target has no existing local or import binding")
                })?;
                if names.next().is_some() {
                    return Err(grammar("candidate call target has multiple aliases"));
                }
                let arguments = array(value, "arguments")?;
                if arguments.len() > MAX_EXPRESSION_NODES.saturating_sub(self.nodes) {
                    return Err(capacity(
                        "candidate call arguments exceed the constructor node bound",
                    ));
                }
                let args = arguments
                    .iter()
                    .map(|argument| self.expression(argument, depth + 1))
                    .collect::<Result<Vec<_>>>()?;
                ExprKind::Call {
                    name,
                    type_arguments: Vec::new(),
                    args,
                }
            }
            "binary" => {
                object(value, &["kind", "op", "left", "right"])?;
                let op = match text(value, "op")? {
                    "+" => BinaryOp::Add,
                    "-" => BinaryOp::Sub,
                    "*" => BinaryOp::Mul,
                    "/" => BinaryOp::Div,
                    "%" => BinaryOp::Rem,
                    "==" => BinaryOp::Eq,
                    "!=" => BinaryOp::Ne,
                    "<" => BinaryOp::Lt,
                    "<=" => BinaryOp::Le,
                    ">" => BinaryOp::Gt,
                    ">=" => BinaryOp::Ge,
                    "&&" => BinaryOp::And,
                    "||" => BinaryOp::Or,
                    _ => return Err(grammar("unsupported candidate binary operator")),
                };
                let left = Box::new(self.expression(member(value, "left")?, depth + 1)?);
                let right = Box::new(self.expression(member(value, "right")?, depth + 1)?);
                ExprKind::Binary { op, left, right }
            }
            "unary" => {
                object(value, &["kind", "op", "value"])?;
                let op = match text(value, "op")? {
                    "-" => UnaryOp::Neg,
                    "!" => UnaryOp::Not,
                    _ => return Err(grammar("unsupported candidate unary operator")),
                };
                ExprKind::Unary {
                    op,
                    value: Box::new(self.expression(member(value, "value")?, depth + 1)?),
                }
            }
            "if" => {
                object(value, &["kind", "condition", "then", "else"])?;
                let condition = Box::new(self.expression(member(value, "condition")?, depth + 1)?);
                // Source `if` branches are blocks. The constructor does not
                // accept textual blocks, so synthesize empty-statement blocks
                // and charge their nodes to the same bounded budget.
                self.nodes += 2;
                if self.nodes > MAX_EXPRESSION_NODES {
                    return Err(capacity(
                        "candidate branch blocks exceed the constructor node bound",
                    ));
                }
                let then_value = self.expression(member(value, "then")?, depth + 1)?;
                let else_value = self.expression(member(value, "else")?, depth + 1)?;
                let then_branch = Box::new(block(then_value));
                let else_branch = Box::new(block(else_value));
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                }
            }
            _ => return Err(grammar("unsupported candidate expression constructor")),
        };
        Ok(Expr {
            kind,
            span: Span::default(),
        })
    }
}

fn block(tail: Expr) -> Expr {
    Expr {
        kind: ExprKind::Block {
            statements: Vec::new(),
            tail: Box::new(tail),
        },
        span: Span::default(),
    }
}

fn literal(value: &Value) -> Result<Expr> {
    let kind = match text(value, "kind")? {
        "i64" => {
            object(value, &["kind", "value"])?;
            ExprKind::Int(
                member(value, "value")?
                    .as_i64()
                    .ok_or_else(|| grammar("candidate i64 literal is out of range"))?,
            )
        }
        "i32" => {
            object(value, &["kind", "value"])?;
            ExprKind::Int32(
                member(value, "value")?
                    .as_i64()
                    .and_then(|n| i32::try_from(n).ok())
                    .ok_or_else(|| grammar("candidate i32 literal is out of range"))?,
            )
        }
        "u8" => {
            object(value, &["kind", "value"])?;
            ExprKind::Uint8(
                member(value, "value")?
                    .as_u64()
                    .and_then(|n| u8::try_from(n).ok())
                    .ok_or_else(|| grammar("candidate u8 literal is out of range"))?,
            )
        }
        "usize" => {
            object(value, &["kind", "value"])?;
            ExprKind::Usize(
                member(value, "value")?
                    .as_u64()
                    .ok_or_else(|| grammar("candidate usize literal is out of range"))?,
            )
        }
        "bool" => {
            object(value, &["kind", "value"])?;
            ExprKind::Bool(
                member(value, "value")?
                    .as_bool()
                    .ok_or_else(|| grammar("candidate bool literal is invalid"))?,
            )
        }
        "char" => {
            object(value, &["kind", "scalar"])?;
            let scalar = exact_hex(value, "scalar", 8)? as u32;
            if char::from_u32(scalar).is_none() {
                return Err(grammar(
                    "candidate char literal is not a Unicode scalar value",
                ));
            }
            ExprKind::Char(scalar)
        }
        "f32" => {
            object(value, &["kind", "bits"])?;
            let bits = exact_hex(value, "bits", 8)? as u32;
            if !f32::from_bits(bits).is_finite() {
                return Err(grammar(
                    "candidate f32 literal bits are not source-representable finite data",
                ));
            }
            return Ok(float_literal(bits & 0x7fff_ffff, bits & 0x8000_0000 != 0));
        }
        "f64" => {
            object(value, &["kind", "bits"])?;
            let bits = exact_hex(value, "bits", 16)?;
            if !f64::from_bits(bits).is_finite() {
                return Err(grammar(
                    "candidate f64 literal bits are not source-representable finite data",
                ));
            }
            return Ok(float64_literal(
                bits & 0x7fff_ffff_ffff_ffff,
                bits & 0x8000_0000_0000_0000 != 0,
            ));
        }
        _ => {
            return Err(grammar(
                "candidate migration argument must be a scalar literal",
            ))
        }
    };
    Ok(Expr {
        kind,
        span: Span::default(),
    })
}

fn exact_hex(value: &Value, key: &str, digits: usize) -> Result<u64> {
    let text = text(value, key)?;
    if text.len() != digits
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(grammar(
            "candidate scalar bit pattern must be fixed-width lowercase hexadecimal",
        ));
    }
    u64::from_str_radix(text, 16)
        .map_err(|_| grammar("candidate scalar bit pattern is out of range"))
}

fn float_literal(magnitude: u32, negative: bool) -> Expr {
    let value = Expr {
        kind: ExprKind::Float32(magnitude),
        span: Span::default(),
    };
    signed_float(value, negative)
}

fn float64_literal(magnitude: u64, negative: bool) -> Expr {
    let value = Expr {
        kind: ExprKind::Float64(magnitude),
        span: Span::default(),
    };
    signed_float(value, negative)
}

fn signed_float(value: Expr, negative: bool) -> Expr {
    if negative {
        Expr {
            kind: ExprKind::Unary {
                op: UnaryOp::Neg,
                value: Box::new(value),
            },
            span: Span::default(),
        }
    } else {
        value
    }
}

fn literal_nodes(expression: &Expr) -> usize {
    usize::from(matches!(&expression.kind, ExprKind::Unary { .. })) + 1
}

fn scalar_type(name: &str) -> Result<Type> {
    match name {
        "i64" => Ok(Type::I64),
        "i32" => Ok(Type::I32),
        "u8" => Ok(Type::U8),
        "usize" => Ok(Type::Usize),
        "char" => Ok(Type::Char),
        "f32" => Ok(Type::F32),
        "f64" => Ok(Type::F64),
        "bool" => Ok(Type::Bool),
        _ => Err(grammar(
            "candidate appended parameter type must be an admitted Copy scalar",
        )),
    }
}

pub(super) fn identifier(name: &str) -> Result<&str> {
    if name.is_empty()
        || name.len() > MAX_NAME_BYTES
        || !name.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_alphabetic() || (index != 0 && byte.is_ascii_digit())
        })
        || matches!(
            name,
            "module"
                | "use"
                | "fn"
                | "let"
                | "mut"
                | "if"
                | "else"
                | "while"
                | "match"
                | "true"
                | "false"
                | "requires"
                | "ensures"
                | "uses"
                | "permit"
                | "unsafe"
                | "return"
                | "own"
                | "borrow"
                | "shared"
                | "self"
                | "super"
        )
    {
        return Err(grammar(
            "candidate name must be a bounded ordinary identifier",
        ));
    }
    Ok(name)
}

fn member<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value
        .get(key)
        .ok_or_else(|| grammar("candidate intention is missing a required field"))
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    member(value, key)?
        .as_str()
        .ok_or_else(|| grammar("candidate intention field must be text"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value]> {
    member(value, key)?
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| grammar("candidate intention field must be an array"))
}

fn object<'a>(value: &'a Value, keys: &[&str]) -> Result<&'a Map<String, Value>> {
    let object = value
        .as_object()
        .ok_or_else(|| grammar("candidate intention must be an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(grammar("candidate intention has missing or unknown fields"));
    }
    Ok(object)
}

fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G226", message)]
}

pub(super) fn walk_program(
    program: &mut Program,
    nodes: &mut usize,
    visit: &mut impl FnMut(&mut Expr) -> Result<()>,
) -> Result<()> {
    for function in &mut program.functions {
        walk_function(function, nodes, visit)?;
    }
    for declaration in &mut program.types {
        if let TypeDeclarationKind::Class { methods, .. } = &mut declaration.kind {
            for method in methods {
                walk_function(method, nodes, visit)?;
            }
        }
    }
    Ok(())
}

pub(super) fn walk_function(
    function: &mut Function,
    nodes: &mut usize,
    visit: &mut impl FnMut(&mut Expr) -> Result<()>,
) -> Result<()> {
    for expression in function
        .requires
        .iter_mut()
        .chain(function.ensures.iter_mut())
        .chain(std::iter::once(&mut function.body))
    {
        walk(expression, 0, nodes, visit)?;
    }
    Ok(())
}

/// Exhaustive child traversal; generic bodies, contracts, guards and loops are
/// included. Unknown future AST variants cause a compiler error, not omission.
fn walk(
    expression: &mut Expr,
    depth: usize,
    nodes: &mut usize,
    visit: &mut impl FnMut(&mut Expr) -> Result<()>,
) -> Result<()> {
    *nodes += 1;
    if depth > MAX_WALK_DEPTH || *nodes > MAX_WALK_NODES {
        return Err(capacity(
            "candidate call migration exceeds its traversal bound",
        ));
    }
    let next = depth + 1;
    match &mut expression.kind {
        ExprKind::Call { args, .. } | ExprKind::SuperMethod { args, .. } => {
            for arg in args {
                walk(arg, next, nodes, visit)?;
            }
        }
        ExprKind::MethodCall { receiver, args, .. } => {
            walk(receiver, next, nodes, visit)?;
            for arg in args {
                walk(arg, next, nodes, visit)?;
            }
        }
        ExprKind::Unary { value, .. }
        | ExprKind::Try { operand: value }
        | ExprKind::Project { base: value, .. } => walk(value, next, nodes, visit)?,
        ExprKind::Binary { left, right, .. } => {
            walk(left, next, nodes, visit)?;
            walk(right, next, nodes, visit)?;
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    Statement::Let { value, .. } | Statement::Assign { value, .. } => {
                        walk(value, next, nodes, visit)?
                    }
                    Statement::Unsafe { body, .. } => walk(body, next, nodes, visit)?,
                    Statement::While {
                        condition, body, ..
                    } => {
                        walk(condition, next, nodes, visit)?;
                        walk(body, next, nodes, visit)?;
                    }
                }
            }
            walk(tail, next, nodes, visit)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            walk(condition, next, nodes, visit)?;
            walk(then_branch, next, nodes, visit)?;
            walk(else_branch, next, nodes, visit)?;
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                walk(&mut field.value, next, nodes, visit)?;
            }
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            walk(scrutinee, next, nodes, visit)?;
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    walk(guard, next, nodes, visit)?;
                }
                walk(&mut arm.value, next, nodes, visit)?;
            }
        }
        ExprKind::UpdateRecord { base, fields } => {
            walk(base, next, nodes, visit)?;
            for field in fields {
                walk(&mut field.value, next, nodes, visit)?;
            }
        }
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
        | ExprKind::Var(_) => {}
    }
    let previous_arity = match &expression.kind {
        ExprKind::Call { args, .. } => args.len(),
        _ => 0,
    };
    visit(expression)?;
    if let ExprKind::Call { args, .. } = &expression.kind {
        // The sole growth admitted by migration is direct literal arguments;
        // charge them even though traversal deliberately did not revisit them.
        let added = args.get(previous_arity..).ok_or_else(|| {
            grammar("candidate migration unexpectedly reduced a caller argument inventory")
        })?;
        *nodes += added.iter().map(literal_nodes).sum::<usize>();
        if *nodes > MAX_WALK_NODES {
            return Err(capacity(
                "candidate migrated arguments exceed the node bound",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{format, parser::Parser};
    use serde_json::json;
    use std::path::Path;

    fn parse(source: &str) -> Program {
        Parser::new(source, Path::new("fixture.spx"))
            .unwrap()
            .parse()
            .unwrap()
    }

    fn programs() -> Vec<Program> {
        vec![
            parse(
                r#"module image.core;
@id("image.add") fn add(a: i64, b: i64) -> i64 { a + b }
@id("image.local") fn local(x: i64) -> i64
requires add(x, 0) >= 0
ensures add(result, 0) >= 0
{
    let mut n = add(x, 1);
    while add(n, 0) > 0 { n = add(n, -1); n > 0 }
    add(add(x, 1), 2)
}
"#,
            ),
            parse(
                r#"module image.consumer;
use function @id("image.add") from image.core as plus;
@id("image.consumer.main") fn main() -> i64 { plus(3, 4) }
"#,
            ),
        ]
    }

    fn append() -> Value {
        json!({"kind":"change_function_signature","target":"image.add","append_parameters":[
            {"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}
        ]})
    }

    fn data_expression(request: &Value) -> Result<Expr> {
        construct_expression_inner(
            None,
            &programs()[0],
            &BTreeSet::new(),
            NominalScope::new(),
            request,
        )
    }

    #[test]
    fn string_literals_preserve_decoded_data_and_bound_utf8_before_rendering() {
        for contents in ["", "\0\u{7f}\n\r\t\\\"é🦀", "{kind:call,target:ambient}"] {
            let expression = data_expression(&json!({"kind":"string","value":contents})).unwrap();
            assert!(matches!(&expression.kind, ExprKind::String(actual) if actual == contents));
            let mut program =
                parse("module image.data; @id(\"image.data.text\") fn text() -> string { \"\" }");
            program.functions[0].body = expression;
            let source = format::canonical(&program);
            let reparsed = parse(&source);
            assert_eq!(format::canonical(&reparsed), source);
            let ExprKind::Block { tail, .. } = &reparsed.functions[0].body.kind else {
                panic!("function body must remain a block");
            };
            assert!(matches!(&tail.kind, ExprKind::String(actual) if actual == contents));
        }
        let limit = "é".repeat(MAX_STRING_LITERAL_BYTES / 2);
        assert!(data_expression(&json!({"kind":"string","value":limit})).is_ok());
        let over = format!("{limit}a");
        let errors = data_expression(&json!({"kind":"string","value":over})).unwrap_err();
        assert_eq!(errors[0].code, "SPX-G226");
    }

    #[test]
    fn byte_array_literals_charge_payloads_to_the_shared_expression_budget() {
        let limit = json!({"kind":"array_u8","values":vec![255u8; MAX_EXPRESSION_NODES - 1]});
        let expression = data_expression(&limit).unwrap();
        assert!(
            matches!(expression.kind, ExprKind::ArrayU8(values) if values.len() == MAX_EXPRESSION_NODES - 1)
        );
        assert!(
            matches!(data_expression(&json!({"kind":"array_u8","values":[]})).unwrap().kind, ExprKind::ArrayU8(values) if values.is_empty())
        );
        let nested = json!({"kind":"let","name":"first",
            "value":{"kind":"array_u8","values":vec![0u8; 2047]},
            "body":{"kind":"array_u8","values":vec![1u8; 2047]}});
        for request in [
            json!({"kind":"array_u8","values":vec![0u8; MAX_EXPRESSION_NODES]}),
            nested,
        ] {
            assert_eq!(data_expression(&request).unwrap_err()[0].code, "SPX-G226");
        }
    }

    #[test]
    fn data_literal_grammar_does_not_expand_scalar_migration_defaults() {
        for request in [
            json!({"kind":"array_u8","values":[-1]}),
            json!({"kind":"array_u8","values":[256]}),
            json!({"kind":"array_u8","values":[1.0]}),
            json!({"kind":"array_u8","values":[true]}),
            json!({"kind":"array_u8","values":["1"]}),
            json!({"kind":"array_u8","values":[{"kind":"u8","value":1}]}),
            json!({"kind":"string","value":null}),
            json!({"kind":"string","value":"text","source":"ambient()"}),
            json!({"kind":"repeat_array_u8","value":0,"count":1}),
        ] {
            assert_eq!(data_expression(&request).unwrap_err()[0].code, "SPX-G225");
        }
        for argument in [
            json!({"kind":"string","value":""}),
            json!({"kind":"array_u8","values":[]}),
        ] {
            let mut intention = append();
            intention["append_parameters"][0]["argument"] = argument;
            code(apply(&mut programs(), &intention), "SPX-G225");
        }
    }

    #[test]
    fn widened_copy_literals_preserve_scalar_bits_and_signed_node_shape() {
        let character = data_expression(&json!({"kind":"char","scalar":"0001f600"})).unwrap();
        assert!(matches!(character.kind, ExprKind::Char(0x1f600)));
        let positive = data_expression(&json!({"kind":"f64","bits":"0000000000000001"})).unwrap();
        assert!(matches!(positive.kind, ExprKind::Float64(1)));
        let negative_zero = data_expression(&json!({"kind":"f32","bits":"80000000"})).unwrap();
        assert_eq!(literal_nodes(&negative_zero), 2);
        assert!(
            matches!(negative_zero.kind, ExprKind::Unary { op: UnaryOp::Neg, value }
            if matches!(value.kind, ExprKind::Float32(0)))
        );
        for invalid in [
            json!({"kind":"char","scalar":"0000d800"}),
            json!({"kind":"char","scalar":"0001F600"}),
            json!({"kind":"f32","bits":"7f800000"}),
            json!({"kind":"f32","bits":"7fc00000"}),
            json!({"kind":"f64","bits":"fff0000000000000"}),
            json!({"kind":"f64","bits":"000000000000000"}),
        ] {
            assert_eq!(data_expression(&invalid).unwrap_err()[0].code, "SPX-G225");
        }
        assert_eq!(scalar_type("char").unwrap(), Type::Char);
        assert_eq!(scalar_type("f32").unwrap(), Type::F32);
        assert_eq!(scalar_type("f64").unwrap(), Type::F64);
    }

    #[test]
    fn append_migrates_nested_contract_loop_and_import_calls_without_reordering() {
        let mut programs = programs();
        let summary = apply(&mut programs, &append()).unwrap();
        assert_eq!(summary.target_id, "image.add");
        assert_eq!(summary.kind, "change_function_signature");
        assert_eq!(summary.migrated_calls, 8);
        let source = format::canonical(&programs[0]);
        assert!(source.contains("fn add(a: i64, b: i64, offset: i64)"));
        assert!(source.contains("requires add(x, 0, 0) >= 0"));
        assert!(source.contains("ensures add(result, 0, 0) >= 0"));
        assert!(source.contains("while add(n, 0, 0) > 0"));
        assert!(source.contains("add(add(x, 1, 0), 2, 0)"));
        let consumer = format::canonical(&programs[1]);
        assert!(consumer.contains("plus(3, 4, 0)"));
        assert!(consumer.contains("from image.core as plus"));
        for program in &programs {
            let canonical = format::canonical(program);
            assert_eq!(format::canonical(&parse(&canonical)), canonical);
        }
    }

    #[test]
    fn rename_keeps_import_alias_and_identity_and_body_uses_stable_id_calls() {
        let mut programs = programs();
        let consumer = format::canonical(&programs[1]);
        let summary = apply(
            &mut programs,
            &json!({
                "kind":"rename_declaration","target":"image.add","name":"sum"
            }),
        )
        .unwrap();
        assert_eq!(summary.migrated_calls, 7);
        assert_eq!(format::canonical(&programs[1]), consumer);
        let source = format::canonical(&programs[0]);
        assert!(source.contains("@id(\"image.add\")\nfn sum("));
        assert!(source.contains("sum(sum(x, 1), 2)"));
        apply(&mut programs, &json!({
            "kind":"replace_function_body","target":"image.local",
            "body":{"kind":"if",
                "condition":{"kind":"binary","op":">=","left":{"kind":"place","name":"x"},"right":{"kind":"i64","value":0}},
                "then":{"kind":"call","target":"image.add","arguments":[{"kind":"place","name":"x"},{"kind":"i64","value":1}]},
                "else":{"kind":"unary","op":"-","value":{"kind":"place","name":"x"}}
            }
        })).unwrap();
        let source = format::canonical(&programs[0]);
        assert!(source.contains("if x >= 0 { sum(x, 1) } else { -x }"));
        assert_eq!(format::canonical(&parse(&source)), source);
    }

    fn code(result: Result<IntentSummary>, expected: &str) {
        match result {
            Ok(_) => panic!("expected {expected}"),
            Err(errors) => assert!(
                errors.iter().any(|error| error.code == expected),
                "{errors:?}"
            ),
        }
    }

    #[test]
    fn unsupported_or_effectful_migrations_and_unbound_body_nodes_fail_closed() {
        // The deeply nested unary chain exercises the constructor depth guard
        // without relying on the default test-thread stack size, which can be
        // as small as 2 MiB on some runners.
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(|| {
                let invalid = [
            json!({"kind":"change_function_signature","target":"image.add","parameters":[{"from":"missing"}]}),
            json!({"kind":"change_function_signature","target":"image.add","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"call","target":"image.add","arguments":[]}}]}),
            json!({"kind":"change_function_signature","target":"image.add","append_parameters":[{"name":"a","type":"i64","argument":{"kind":"i64","value":0}}]}),
            json!({"kind":"replace_function_body","target":"image.add","body":{"kind":"place","name":"missing"}}),
            json!({"kind":"replace_function_body","target":"image.add","body":{"kind":"call","target":"not.imported","arguments":[]}}),
            json!({"kind":"replace_function_body","target":"image.add","body":{"kind":"i64","value":0,"source":"ambient()"}}),
            json!({"kind":"rename_declaration","target":"image.add","name":"local"}),
            json!({"kind":"rename_declaration","target":"image.consumer.main","name":"entry"}),
            json!({"kind":"rename_declaration","target":"image.add","name":"sum() { 1 }"}),
        ];
        for intention in invalid {
            code(apply(&mut programs(), &intention), "SPX-G225");
        }
        let mut nested = json!({"kind":"i64","value":0});
        for _ in 0..=MAX_EXPRESSION_DEPTH {
            nested = json!({"kind":"unary","op":"-","value":nested});
        }
        code(
            apply(
                &mut programs(),
                &json!({"kind":"replace_function_body","target":"image.add","body":nested}),
            ),
            "SPX-G226",
        );
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
