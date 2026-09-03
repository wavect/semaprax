//! Exact retained-instance guard for a generic declaration display rename.
//!
//! The generic template and every concrete retained instance are authenticated
//! before source mutation. After ordinary full-Project replay, only the display
//! name may differ. Source spans are normalized because a width-changing
//! display rename shifts positions; stable structural HIR identities are not.
//! Instance identities, exact type arguments, checked bodies, contracts,
//! ownership, cleanup and loan plans must otherwise remain exactly equal.

use crate::ast::{Program, Span};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedFunctionInstance,
    ResolvedFunctionTemplate, ResolvedMatchPattern, ResolvedRecordMatchFieldPattern,
    ResolvedStatement,
};

use super::ProjectRevision;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

const MAX_RETAINED_INSTANCES: usize = 4096;

#[derive(Clone)]
pub(super) struct GenericRenamePlan {
    target: String,
    path: String,
    module: String,
    template: ResolvedFunctionTemplate,
    instances: Vec<ResolvedFunctionInstance>,
}

pub(super) fn plan(
    revision: &ProjectRevision,
    programs: &[Program],
    intent: &serde_json::Value,
) -> Result<Option<GenericRenamePlan>> {
    if intent.get("kind").and_then(serde_json::Value::as_str) != Some("rename_declaration") {
        return Ok(None);
    }
    let Some(target) = intent.get("target").and_then(serde_json::Value::as_str) else {
        return Ok(None);
    };
    let mut source = None;
    for program in programs {
        for function in &program.functions {
            if function.stable_id == target
                && !function.type_parameters.is_empty()
                && source.replace((program, function)).is_some()
            {
                return Err(invalid("generic rename source identity is ambiguous"));
            }
        }
    }
    let Some((program, function)) = source else {
        return Ok(None);
    };
    if !function.explicit_id || function.name == "main" {
        return Err(invalid(
            "generic rename requires an explicit non-entry template identity",
        ));
    }

    let mut retained = None;
    for owner in revision.semantic.image_modules() {
        for template in owner
            .function_templates()
            .iter()
            .filter(|template| template.id.as_str() == target)
        {
            if retained.replace((owner, template)).is_some() {
                return Err(invalid("retained generic template identity is ambiguous"));
            }
        }
    }
    let Some((owner, template)) = retained else {
        return Err(invalid(
            "generic rename requires an authenticated retained template",
        ));
    };
    if owner.path() != program.path
        || owner.module() != program.module
        || template.name != function.name
        || template.type_parameters.len() != function.type_parameters.len()
    {
        return Err(invalid(
            "generic source declaration disagrees with its retained template",
        ));
    }
    let (template, instances) = normalized(owner, template, target)?;
    Ok(Some(GenericRenamePlan {
        target: target.to_owned(),
        path: owner.path().to_owned(),
        module: owner.module().to_owned(),
        template,
        instances,
    }))
}

pub(super) fn validate(revision: &ProjectRevision, plan: &GenericRenamePlan) -> Result<()> {
    let mut retained = None;
    for owner in revision.semantic.image_modules() {
        for template in owner
            .function_templates()
            .iter()
            .filter(|template| template.id.as_str() == plan.target)
        {
            if retained.replace((owner, template)).is_some() {
                return Err(stale(
                    "candidate generic template identity became ambiguous",
                ));
            }
        }
    }
    let Some((owner, template)) = retained else {
        return Err(stale("candidate lost the retained generic template"));
    };
    if owner.path() != plan.path || owner.module() != plan.module {
        return Err(stale("candidate moved the retained generic template"));
    }
    let (candidate_template, candidate_instances) = normalized(owner, template, &plan.target)?;
    if candidate_template != plan.template || candidate_instances != plan.instances {
        return Err(stale(
            "generic rename changed the checked template or concrete instance inventory",
        ));
    }
    Ok(())
}

fn normalized(
    owner: &crate::workspace_graph::WorkspaceGraphProjectionModule,
    template: &ResolvedFunctionTemplate,
    target: &str,
) -> Result<(ResolvedFunctionTemplate, Vec<ResolvedFunctionInstance>)> {
    let mut normalized_template = template.clone();
    normalized_template.name.clear();
    normalize_template_spans(&mut normalized_template);
    let mut instances = owner
        .function_instances()
        .iter()
        .filter(|instance| instance.template.as_str() == target)
        .cloned()
        .collect::<Vec<_>>();
    if instances.len() > MAX_RETAINED_INSTANCES {
        return Err(capacity(
            "generic rename concrete instance inventory exceeds its bound",
        ));
    }
    for instance in &mut instances {
        if instance.function.id.as_str() != target
            || instance.type_arguments.len() != template.type_parameters.len()
        {
            return Err(invalid(
                "retained concrete instance disagrees with its generic template",
            ));
        }
        instance.function.name.clear();
        instance.function.span = Span::default();
        for parameter in &mut instance.function.params {
            parameter.span = Span::default();
        }
        for expression in &mut instance.function.requires {
            normalize_expression_spans(expression);
        }
        normalize_expression_spans(&mut instance.function.body);
        for expression in &mut instance.function.ensures {
            normalize_expression_spans(expression);
        }
    }
    instances.sort_by(|left, right| left.id.cmp(&right.id));
    if instances.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err(invalid("retained concrete instance identity is duplicated"));
    }
    Ok((normalized_template, instances))
}

fn normalize_template_spans(template: &mut ResolvedFunctionTemplate) {
    template.span = Span::default();
    for parameter in &mut template.type_parameters {
        parameter.span = Span::default();
    }
    for parameter in &mut template.params {
        parameter.span = Span::default();
    }
    for expression in &mut template.requires {
        normalize_expression_spans(expression);
    }
    normalize_expression_spans(&mut template.body);
    for expression in &mut template.ensures {
        normalize_expression_spans(expression);
    }
}

fn normalize_expression_spans(expression: &mut ResolvedExpr) {
    expression.span = Span::default();
    match &mut expression.kind {
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
            normalize_expression_spans(source);
            normalize_expression_spans(start);
            normalize_expression_spans(end);
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::NativeRustImportCall(crate::hir::ResolvedNativeRustImportCall {
            args,
            ..
        })
        | ResolvedExprKind::HostCommandCall(crate::hir::ResolvedHostCommandCall { args, .. }) => {
            for argument in args {
                normalize_expression_spans(argument);
            }
        }
        ResolvedExprKind::Unary { value, .. } | ResolvedExprKind::Upcast { source: value } => {
            normalize_expression_spans(value);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            normalize_expression_spans(left);
            normalize_expression_spans(right);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                normalize_statement_spans(statement);
            }
            normalize_expression_spans(tail);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            normalize_expression_spans(condition);
            normalize_expression_spans(then_branch);
            normalize_expression_spans(else_branch);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                normalize_expression_spans(&mut field.value);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            normalize_expression_spans(scrutinee);
            for arm in arms {
                arm.span = Span::default();
                normalize_pattern_spans(&mut arm.pattern);
                if let Some(guard) = &mut arm.guard {
                    normalize_expression_spans(guard);
                }
                normalize_expression_spans(&mut arm.value);
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            normalize_expression_spans(operand);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            normalize_expression_spans(base);
            for field in fields {
                normalize_expression_spans(&mut field.value);
            }
        }
        ResolvedExprKind::Project { base, .. } => normalize_expression_spans(base),
    }
}

fn normalize_statement_spans(statement: &mut ResolvedStatement) {
    match statement {
        ResolvedStatement::Let {
            binding,
            value,
            span,
            ..
        }
        | ResolvedStatement::Assign {
            binding,
            value,
            span,
            ..
        } => {
            normalize_binding_span(binding);
            *span = Span::default();
            normalize_expression_spans(value);
        }
        ResolvedStatement::Unsafe { body, span, .. } => {
            *span = Span::default();
            normalize_expression_spans(body);
        }
        ResolvedStatement::While {
            condition,
            body,
            span,
        } => {
            *span = Span::default();
            normalize_expression_spans(condition);
            normalize_expression_spans(body);
        }
    }
}

fn normalize_pattern_spans(pattern: &mut ResolvedMatchPattern) {
    match pattern {
        ResolvedMatchPattern::Variant { fields, .. } => {
            for field in fields {
                normalize_binding_span(&mut field.binding);
            }
        }
        ResolvedMatchPattern::Record { fields, .. } => {
            for field in fields {
                normalize_record_pattern_spans(&mut field.pattern);
            }
        }
        ResolvedMatchPattern::Binding(binding) => normalize_binding_span(binding),
        ResolvedMatchPattern::Or(patterns) => {
            for pattern in patterns {
                normalize_pattern_spans(pattern);
            }
        }
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
    }
}

fn normalize_record_pattern_spans(pattern: &mut ResolvedRecordMatchFieldPattern) {
    match pattern {
        ResolvedRecordMatchFieldPattern::Binding(binding) => normalize_binding_span(binding),
        ResolvedRecordMatchFieldPattern::Record { fields, .. } => {
            for field in fields {
                normalize_record_pattern_spans(&mut field.pattern);
            }
        }
        ResolvedRecordMatchFieldPattern::Wildcard => {}
    }
}

fn normalize_binding_span(binding: &mut ResolvedBinding) {
    binding.span = Span::default();
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G503", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G504", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G505", message)]
}
