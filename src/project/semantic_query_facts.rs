//! Bounded checked-HIR fact projections for Universal Semantic Query v1.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::hir::{
    DeclarationId, OwnershipMode, Place, PlaceProjection, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedFunctionTemplate, ResolvedMatchPattern,
    ResolvedRecordMatchFieldPattern, ResolvedStatement, ResolvedType,
};

use super::semantic_query::{
    capacity, invalid, render, MAX_SEMANTIC_QUERY_CONSUMER_LIMIT, MAX_SEMANTIC_QUERY_RESULT_BYTES,
    SEMANTIC_QUERY_DECLARATION_CONSUMERS_SCHEMA, SEMANTIC_QUERY_OWNERSHIP_AT_EXPRESSION_SCHEMA,
};
use super::{ProjectRevision, SemanticWorkspaceSnapshot};

const MAX_FACT_WALK: usize = 65_536;

type Result<T> = std::result::Result<T, Vec<crate::diagnostic::Diagnostic>>;

pub(super) fn ownership_at_expression_payload(
    snapshot: &SemanticWorkspaceSnapshot,
    stable_id: &str,
    expression_id: &str,
) -> Result<String> {
    let revision = snapshot.generation().revision();
    let mut facts = Vec::new();
    for program in programs(revision) {
        for function in &program.functions {
            if function.id.as_str() == stable_id {
                facts.push(ownership_fact(
                    stable_id,
                    expression_id,
                    function_parts(function),
                )?);
            }
        }
        for template in &program.function_templates {
            if template.id.as_str() == stable_id {
                facts.push(ownership_fact(
                    stable_id,
                    expression_id,
                    template_parts(template),
                )?);
            }
        }
    }
    let Some(first) = facts.first() else {
        return Err(invalid(
            "semantic ownership query stable function declaration is unknown",
        ));
    };
    if facts.iter().skip(1).any(|fact| fact != first) {
        return Err(invalid(
            "semantic ownership query found conflicting retained HIR facts",
        ));
    }
    let generation = snapshot.generation();
    render(
        json!({
            "expression": first,
            "image_digest": generation.image().image_digest(),
            "limits": {"max_walk": MAX_FACT_WALK},
            "nonclaims": [
                "ownership_mode_is_a_checked_boundary_classification_not_flow_sensitive_availability",
                "loan_facts_are_static_checked_proof_not_runtime_liveness_or_permission",
                "expression_id_is_revision_scoped_within_the_selected_stable_declaration",
                "no_mutable_or_escaping_borrow_inference",
            ],
            "project_revision": generation.revision().project_revision(),
            "schema": SEMANTIC_QUERY_OWNERSHIP_AT_EXPRESSION_SCHEMA,
            "stable_id": stable_id,
            "workspace_revision": generation.workspace_revision(),
        }),
        MAX_SEMANTIC_QUERY_RESULT_BYTES,
        true,
    )
}

struct FunctionParts<'a> {
    requires: &'a [ResolvedExpr],
    body: &'a ResolvedExpr,
    ensures: &'a [ResolvedExpr],
    loan_plan: Option<&'a crate::loan_plan::LoanPlan>,
}

fn function_parts(function: &ResolvedFunction) -> FunctionParts<'_> {
    FunctionParts {
        requires: &function.requires,
        body: &function.body,
        ensures: &function.ensures,
        loan_plan: Some(&function.loan_plan),
    }
}

fn template_parts(function: &ResolvedFunctionTemplate) -> FunctionParts<'_> {
    FunctionParts {
        requires: &function.requires,
        body: &function.body,
        ensures: &function.ensures,
        loan_plan: None,
    }
}

fn ownership_fact(stable_id: &str, expression_id: &str, parts: FunctionParts<'_>) -> Result<Value> {
    let mut matches = Vec::new();
    let mut visited = 0usize;
    for root in parts
        .requires
        .iter()
        .chain(std::iter::once(parts.body))
        .chain(parts.ensures)
    {
        walk_expression(root, &mut visited, &mut |expression| {
            if expression.id.as_str() == expression_id {
                matches.push(expression);
            }
        })?;
    }
    let [expression] = matches.as_slice() else {
        return Err(invalid(
            "semantic ownership query expression is not uniquely owned by the selected declaration",
        ));
    };
    let direct_place = match &expression.kind {
        ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
            Some(place_value(place))
        }
        _ => None,
    };
    let loans = parts
        .loan_plan
        .map(|plan| {
            plan.loans
                .iter()
                .filter(|loan| loan.site.as_str() == expression_id)
                .map(loan_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({
        "expression_id": expression_id,
        "kind": expression_kind(&expression.kind),
        "loans": loans,
        "ownership_mode": ownership_name(expression.ownership),
        "place": direct_place,
        "stable_declaration": stable_id,
        "type_identity": expression.ty.identity_key(),
    }))
}

pub(super) fn declaration_consumers_payload(
    snapshot: &SemanticWorkspaceSnapshot,
    stable_id: &str,
    offset: usize,
    limit: usize,
) -> Result<String> {
    let revision = snapshot.generation().revision();
    if !declaration_exists(revision, stable_id) {
        return Err(invalid(
            "semantic consumer query stable declaration is unknown",
        ));
    }
    let mut consumers = BTreeMap::<String, Consumer>::new();
    let mut visited = 0usize;
    for program in programs(revision) {
        for function in &program.functions {
            index_consumer(
                &program.module,
                function.id.as_str(),
                &function.name,
                &function
                    .params
                    .iter()
                    .map(|param| &param.ty)
                    .collect::<Vec<_>>(),
                &function.return_type,
                function
                    .requires
                    .iter()
                    .chain(std::iter::once(&function.body))
                    .chain(&function.ensures),
                stable_id,
                &mut visited,
                &mut consumers,
            )?;
        }
        for function in &program.function_templates {
            index_consumer(
                &program.module,
                function.id.as_str(),
                &function.name,
                &function
                    .params
                    .iter()
                    .map(|param| &param.ty)
                    .collect::<Vec<_>>(),
                &function.return_type,
                function
                    .requires
                    .iter()
                    .chain(std::iter::once(&function.body))
                    .chain(&function.ensures),
                stable_id,
                &mut visited,
                &mut consumers,
            )?;
        }
    }
    let rows = consumers
        .into_values()
        .filter(|consumer| !consumer.use_kinds.is_empty())
        .map(|consumer| consumer.value(revision))
        .collect::<Vec<_>>();
    let end = offset.saturating_add(limit).min(rows.len());
    let page = if offset < rows.len() {
        rows[offset..end].to_vec()
    } else {
        Vec::new()
    };
    let generation = snapshot.generation();
    render(
        json!({
            "consumers": page,
            "image_digest": generation.image().image_digest(),
            "limit": limit,
            "limits": {
                "max_items_per_page": MAX_SEMANTIC_QUERY_CONSUMER_LIMIT,
                "max_walk": MAX_FACT_WALK,
            },
            "next_offset": (end < rows.len()).then_some(end),
            "nonclaims": [
                "direct_static_retained_HIR_uses_only",
                "no_transitive_runtime_path_feasibility_or_dynamic_dispatch_claim",
                "exported_means_direct_manifest_web_export_not_language_level_public_visibility",
                "no_cross_project_or_unloaded_source_consumers",
            ],
            "offset": offset,
            "project_revision": generation.revision().project_revision(),
            "schema": SEMANTIC_QUERY_DECLARATION_CONSUMERS_SCHEMA,
            "stable_id": stable_id,
            "total_consumers": rows.len(),
            "workspace_revision": generation.workspace_revision(),
        }),
        MAX_SEMANTIC_QUERY_RESULT_BYTES,
        true,
    )
}

struct Consumer {
    id: String,
    name: String,
    modules: BTreeSet<String>,
    use_kinds: BTreeSet<&'static str>,
}

impl Consumer {
    fn value(self, revision: &ProjectRevision) -> Value {
        let visibility = if revision.manifest().web_exports().contains(&self.id) {
            "exported"
        } else if self.modules.contains(revision.manifest().test_module()) {
            "test"
        } else {
            "local"
        };
        json!({
            "consumer_id": self.id,
            "modules": self.modules,
            "name": self.name,
            "use_kinds": self.use_kinds,
            "visibility": visibility,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn index_consumer<'a>(
    module: &str,
    id: &str,
    name: &str,
    params: &[&ResolvedType],
    result: &ResolvedType,
    roots: impl Iterator<Item = &'a ResolvedExpr>,
    target: &str,
    visited: &mut usize,
    consumers: &mut BTreeMap<String, Consumer>,
) -> Result<()> {
    let consumer = consumers.entry(id.to_owned()).or_insert_with(|| Consumer {
        id: id.to_owned(),
        name: name.to_owned(),
        modules: BTreeSet::new(),
        use_kinds: BTreeSet::new(),
    });
    if consumer.name != name {
        return Err(invalid(
            "semantic consumer query found conflicting retained function facts",
        ));
    }
    consumer.modules.insert(module.to_owned());
    if params.iter().any(|ty| type_references(ty, target)) || type_references(result, target) {
        consumer.use_kinds.insert("signature_type");
    }
    for root in roots {
        walk_expression(root, visited, &mut |expression| {
            collect_expression_uses(expression, target, &mut consumer.use_kinds)
        })?;
    }
    Ok(())
}

fn declaration_exists(revision: &ProjectRevision, target: &str) -> bool {
    let id = DeclarationId::new(target);
    programs(revision)
        .iter()
        .any(|program| program.declarations.declaration(&id).is_some())
}

fn programs(revision: &ProjectRevision) -> [&crate::hir::ResolvedProgram; 3] {
    [
        revision.entry_program(),
        revision.public_api_program(),
        revision.test_program(),
    ]
}

fn walk_expression<'a>(
    root: &'a ResolvedExpr,
    visited: &mut usize,
    visit: &mut impl FnMut(&'a ResolvedExpr),
) -> Result<()> {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        *visited = visited.saturating_add(1);
        if *visited > MAX_FACT_WALK {
            return Err(capacity("semantic fact query exceeds its HIR walk bound"));
        }
        visit(expression);
        push_children(expression, &mut pending);
    }
    Ok(())
}

fn push_children<'a>(expression: &'a ResolvedExpr, pending: &mut Vec<&'a ResolvedExpr>) {
    match &expression.kind {
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
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            pending.extend([end.as_ref(), start.as_ref(), source.as_ref()]);
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::NativeRustImportCall(crate::hir::ResolvedNativeRustImportCall {
            args,
            ..
        })
        | ResolvedExprKind::HostCommandCall(crate::hir::ResolvedHostCommandCall { args, .. }) => {
            pending.extend(args.iter().rev());
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => pending.push(value),
        ResolvedExprKind::Binary { left, right, .. } => pending.extend([right.as_ref(), left]),
        ResolvedExprKind::Block { statements, tail } => {
            pending.push(tail);
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
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
            pending.extend([
                else_branch.as_ref(),
                then_branch.as_ref(),
                condition.as_ref(),
            ]);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                pending.push(&arm.value);
                if let Some(guard) = &arm.guard {
                    pending.push(guard);
                }
            }
            pending.push(scrutinee);
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            pending.push(operand);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value));
            pending.push(base);
        }
    }
}

fn collect_expression_uses(
    expression: &ResolvedExpr,
    target: &str,
    uses: &mut BTreeSet<&'static str>,
) {
    if type_references(&expression.ty, target) {
        uses.insert("expression_type");
    }
    match &expression.kind {
        ResolvedExprKind::Place(place) => collect_place_uses(place, target, uses),
        ResolvedExprKind::BorrowPlace { operation, place } => {
            matches_id(operation, target, "borrow_operation", uses);
            collect_place_uses(place, target, uses);
        }
        ResolvedExprKind::ByteRange { operation, .. } => {
            matches_id(operation, target, "byte_range_operation", uses)
        }
        ResolvedExprKind::Call { callee, .. } => matches_id(callee, target, "direct_call", uses),
        ResolvedExprKind::NativeRustImportCall(call) => {
            matches_id(&call.import, target, "native_import_call", uses)
        }
        ResolvedExprKind::ConstructRecord { record, fields } => {
            matches_id(record, target, "construct_record", uses);
            collect_fields(
                fields.iter().map(|field| &field.field),
                target,
                "initialize_field",
                uses,
            );
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => {
            matches_id(variant, target, "construct_variant", uses);
            matches_id(case, target, "construct_case", uses);
            collect_fields(
                fields.iter().map(|field| &field.field),
                target,
                "initialize_field",
                uses,
            );
        }
        ResolvedExprKind::Match { arms, .. } => {
            for arm in arms {
                collect_pattern_uses(&arm.pattern, target, uses);
            }
        }
        ResolvedExprKind::Try {
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
            ..
        } => {
            for (id, kind) in [
                (result, "try_result"),
                (ok_case, "try_case"),
                (ok_field, "try_field"),
                (err_case, "try_case"),
                (err_field, "try_field"),
            ] {
                matches_id(id, target, kind, uses);
            }
            if type_references(residual_type, target) {
                uses.insert("expression_type");
            }
        }
        ResolvedExprKind::TryOption {
            option,
            some_case,
            some_field,
            none_case,
            residual_type,
            ..
        } => {
            for (id, kind) in [
                (option, "try_option"),
                (some_case, "try_case"),
                (some_field, "try_field"),
                (none_case, "try_case"),
            ] {
                matches_id(id, target, kind, uses);
            }
            if type_references(residual_type, target) {
                uses.insert("expression_type");
            }
        }
        ResolvedExprKind::UpdateRecord { record, fields, .. } => {
            matches_id(record, target, "update_record", uses);
            collect_fields(
                fields.iter().map(|field| &field.field),
                target,
                "update_field",
                uses,
            );
        }
        ResolvedExprKind::Project { field, .. } => matches_id(field, target, "project_field", uses),
        ResolvedExprKind::Block { statements, .. } => {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    field: Some(field), ..
                } = statement
                {
                    matches_id(field, target, "assign_field", uses);
                }
            }
        }
        _ => {}
    }
}

fn collect_pattern_uses(
    pattern: &ResolvedMatchPattern,
    target: &str,
    uses: &mut BTreeSet<&'static str>,
) {
    match pattern {
        ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => {
            matches_id(variant, target, "match_variant", uses);
            matches_id(case, target, "match_case", uses);
            collect_fields(
                fields.iter().map(|field| &field.field),
                target,
                "match_field",
                uses,
            );
        }
        ResolvedMatchPattern::Record {
            record,
            instance,
            fields,
        } => {
            matches_id(record, target, "match_record", uses);
            if type_references(instance, target) {
                uses.insert("expression_type");
            }
            collect_record_pattern_fields(fields, target, uses);
        }
        ResolvedMatchPattern::Or(patterns) => {
            for pattern in patterns {
                collect_pattern_uses(pattern, target, uses);
            }
        }
        ResolvedMatchPattern::Wildcard
        | ResolvedMatchPattern::Literal(_)
        | ResolvedMatchPattern::Binding(_) => {}
    }
}

fn collect_record_pattern_fields(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
    target: &str,
    uses: &mut BTreeSet<&'static str>,
) {
    for field in fields {
        matches_id(&field.field, target, "match_field", uses);
        if let ResolvedRecordMatchFieldPattern::Record {
            record,
            instance,
            fields,
        } = &field.pattern
        {
            matches_id(record, target, "match_record", uses);
            if type_references(instance, target) {
                uses.insert("expression_type");
            }
            collect_record_pattern_fields(fields, target, uses);
        }
    }
}

fn collect_place_uses(place: &Place, target: &str, uses: &mut BTreeSet<&'static str>) {
    for projection in &place.projections {
        match projection {
            PlaceProjection::Field(field) => matches_id(field, target, "place_field", uses),
            PlaceProjection::VariantField { case, field } => {
                matches_id(case, target, "place_case", uses);
                matches_id(field, target, "place_field", uses);
            }
        }
    }
}

fn collect_fields<'a>(
    fields: impl Iterator<Item = &'a DeclarationId>,
    target: &str,
    kind: &'static str,
    uses: &mut BTreeSet<&'static str>,
) {
    for field in fields {
        matches_id(field, target, kind, uses);
    }
}

fn matches_id(
    id: &DeclarationId,
    target: &str,
    kind: &'static str,
    uses: &mut BTreeSet<&'static str>,
) {
    if id.as_str() == target {
        uses.insert(kind);
    }
}

fn type_references(ty: &ResolvedType, target: &str) -> bool {
    match ty {
        ResolvedType::TypeParameter { owner, .. } => owner.as_str() == target,
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => {
            declaration.as_str() == target || arguments.iter().any(|ty| type_references(ty, target))
        }
        _ => false,
    }
}

fn place_value(place: &Place) -> Value {
    json!({
        "root": place.root.as_str(),
        "projections": place.projections.iter().map(|projection| match projection {
            PlaceProjection::Field(field) => json!({"kind":"field","stable_id":field.as_str()}),
            PlaceProjection::VariantField { case, field } => json!({"case_id":case.as_str(),"field_id":field.as_str(),"kind":"variant_payload"}),
        }).collect::<Vec<_>>(),
    })
}

fn loan_value(loan: &crate::loan_plan::Loan) -> Value {
    use crate::loan_plan::{LoanCause, LoanPointPhase};
    let point = |point: &crate::loan_plan::LoanProgramPoint| {
        json!({
            "expression_id": point.expression.as_str(),
            "phase": match point.phase { LoanPointPhase::Before => "before", LoanPointPhase::After => "after" },
        })
    };
    let cause = match &loan.cause {
        LoanCause::SliceView => json!({"kind":"slice_view"}),
        LoanCause::StrView => json!({"kind":"str_view"}),
        LoanCause::BorrowedCall { argument } => json!({"argument":argument,"kind":"borrowed_call"}),
        LoanCause::MatchBorrow { arm } => json!({"arm":arm,"kind":"match_borrow"}),
    };
    json!({
        "cause": cause,
        "end_edges": &loan.end_edges,
        "ends": loan.ends.iter().map(point).collect::<Vec<_>>(),
        "id": loan.id.0,
        "origin": place_value(&loan.origin),
        "parent": loan.parent.map(|parent| parent.0),
        "site": loan.site.as_str(),
        "start": point(&loan.start),
    })
}

fn ownership_name(mode: OwnershipMode) -> &'static str {
    match mode {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}

fn expression_kind(kind: &ResolvedExprKind) -> &'static str {
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
        ResolvedExprKind::BorrowPlace { .. } => "borrow_place",
        ResolvedExprKind::ByteRange { .. } => "byte_range",
        ResolvedExprKind::Call { .. } => "call",
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
