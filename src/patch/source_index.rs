use std::collections::BTreeMap;

use crate::ast::{
    Expr, ExprKind, MatchPattern, Program, RecordMatchFieldPattern, Span, Statement,
    TypeDeclarationKind,
};
use crate::hir::{
    PlaceProjection, ResolvedExpr, ResolvedExprKind, ResolvedMatchPattern, ResolvedProgram,
    ResolvedRecordMatchFieldPattern, ResolvedStatement, ResolvedType,
};
use crate::lexer::{Token, TokenKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MemberSite {
    pub span: Span,
    pub shorthand_binding: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CallSite {
    pub expression: String,
    pub template: String,
    pub instance: Option<String>,
    pub type_arguments: Vec<ResolvedType>,
    pub type_argument_spans: Vec<Span>,
}

#[derive(Default)]
pub(super) struct SemanticSourceIndex {
    pub members: BTreeMap<(String, String), Vec<MemberSite>>,
    pub cases: BTreeMap<(String, String), Vec<Span>>,
    pub calls: BTreeMap<String, CallSite>,
    member_owners: BTreeMap<String, String>,
}

impl SemanticSourceIndex {
    pub fn build(program: &Program, resolved: &ResolvedProgram, tokens: &[Token]) -> Option<Self> {
        let mut index = Self::default();
        for declaration in &program.types {
            match &declaration.kind {
                TypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        index.member(
                            &declaration.stable_id,
                            &field.stable_id,
                            field.name_span,
                            None,
                        );
                    }
                }
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        index.case(&declaration.stable_id, &case.stable_id, case.name_span);
                        for field in &case.fields {
                            index.member(&case.stable_id, &field.stable_id, field.name_span, None);
                        }
                    }
                }
                TypeDeclarationKind::Resource { .. } => {}
            }
        }

        for function in &program.functions {
            let (requires, ensures, body) = if function.type_parameters.is_empty() {
                let resolved = resolved
                    .functions
                    .iter()
                    .find(|candidate| candidate.id.as_str() == function.stable_id)?;
                (&resolved.requires, &resolved.ensures, &resolved.body)
            } else {
                let resolved = resolved
                    .function_templates
                    .iter()
                    .find(|candidate| candidate.id.as_str() == function.stable_id)?;
                (&resolved.requires, &resolved.ensures, &resolved.body)
            };
            if function.requires.len() != requires.len() || function.ensures.len() != ensures.len()
            {
                return None;
            }
            for (source, resolved) in function.requires.iter().zip(requires) {
                index.expr(source, resolved, tokens)?;
            }
            for (source, resolved) in function.ensures.iter().zip(ensures) {
                index.expr(source, resolved, tokens)?;
            }
            index.expr(&function.body, body, tokens)?;
        }
        Some(index)
    }

    fn member(&mut self, owner: &str, field: &str, span: Span, shorthand_binding: Option<String>) {
        self.member_owners
            .entry(field.to_owned())
            .or_insert_with(|| owner.to_owned());
        self.members
            .entry((owner.to_owned(), field.to_owned()))
            .or_default()
            .push(MemberSite {
                span,
                shorthand_binding,
            });
    }

    fn case(&mut self, owner: &str, case: &str, span: Span) {
        self.cases
            .entry((owner.to_owned(), case.to_owned()))
            .or_default()
            .push(span);
    }

    fn expr(&mut self, source: &Expr, resolved: &ResolvedExpr, tokens: &[Token]) -> Option<()> {
        match (&source.kind, &resolved.kind) {
            (
                ExprKind::Call {
                    type_arguments,
                    args,
                    ..
                },
                ResolvedExprKind::Call {
                    callee,
                    type_arguments: resolved_arguments,
                    instance,
                    args: resolved_args,
                },
            ) => {
                let type_argument_spans =
                    call_type_argument_spans(source.span, type_arguments.len(), tokens)?;
                let expression = resolved.id.as_str().to_owned();
                if self
                    .calls
                    .insert(
                        expression.clone(),
                        CallSite {
                            expression,
                            template: callee.as_str().to_owned(),
                            instance: instance.as_ref().map(|value| value.as_str().to_owned()),
                            type_arguments: resolved_arguments.clone(),
                            type_argument_spans,
                        },
                    )
                    .is_some()
                {
                    return None;
                }
                self.expr_pairs(args, resolved_args, tokens)?;
            }
            (
                ExprKind::Unary { value, .. },
                ResolvedExprKind::Unary {
                    value: resolved_value,
                    ..
                },
            )
            | (
                ExprKind::Try { operand: value },
                ResolvedExprKind::Try {
                    operand: resolved_value,
                    ..
                },
            )
            | (
                ExprKind::Try { operand: value },
                ResolvedExprKind::TryOption {
                    operand: resolved_value,
                    ..
                },
            ) => self.expr(value, resolved_value, tokens)?,
            (
                ExprKind::Binary { left, right, .. },
                ResolvedExprKind::Binary {
                    left: resolved_left,
                    right: resolved_right,
                    ..
                },
            ) => {
                self.expr(left, resolved_left, tokens)?;
                self.expr(right, resolved_right, tokens)?;
            }
            (
                ExprKind::Block { statements, tail },
                ResolvedExprKind::Block {
                    statements: resolved_statements,
                    tail: resolved_tail,
                },
            ) => {
                if statements.len() != resolved_statements.len() {
                    return None;
                }
                for (statement, resolved_statement) in statements.iter().zip(resolved_statements) {
                    match (statement, resolved_statement) {
                        (
                            Statement::Let { value, .. },
                            ResolvedStatement::Let {
                                value: resolved_value,
                                ..
                            },
                        ) => self.expr(value, resolved_value, tokens)?,
                    }
                }
                self.expr(tail, resolved_tail, tokens)?;
            }
            (
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                },
                ResolvedExprKind::If {
                    condition: resolved_condition,
                    then_branch: resolved_then,
                    else_branch: resolved_else,
                },
            ) => {
                self.expr(condition, resolved_condition, tokens)?;
                self.expr(then_branch, resolved_then, tokens)?;
                self.expr(else_branch, resolved_else, tokens)?;
            }
            (
                ExprKind::ConstructRecord { fields, .. },
                ResolvedExprKind::ConstructRecord {
                    record,
                    fields: resolved_fields,
                },
            ) => {
                if fields.len() != resolved_fields.len() {
                    return None;
                }
                for (field, resolved_field) in fields.iter().zip(resolved_fields) {
                    self.member(
                        record.as_str(),
                        resolved_field.field.as_str(),
                        field.name_span,
                        None,
                    );
                    self.expr(&field.value, &resolved_field.value, tokens)?;
                }
            }
            (
                ExprKind::ConstructVariant {
                    case_span, fields, ..
                },
                ResolvedExprKind::ConstructVariant {
                    variant,
                    case,
                    fields: resolved_fields,
                },
            ) => {
                self.case(variant.as_str(), case.as_str(), *case_span);
                if fields.len() != resolved_fields.len() {
                    return None;
                }
                for (field, resolved_field) in fields.iter().zip(resolved_fields) {
                    self.member(
                        case.as_str(),
                        resolved_field.field.as_str(),
                        field.name_span,
                        None,
                    );
                    self.expr(&field.value, &resolved_field.value, tokens)?;
                }
            }
            (
                ExprKind::Match { scrutinee, arms },
                ResolvedExprKind::Match {
                    scrutinee: resolved_scrutinee,
                    arms: resolved_arms,
                },
            ) => {
                self.expr(scrutinee, resolved_scrutinee, tokens)?;
                if arms.len() != resolved_arms.len() {
                    return None;
                }
                for (arm, resolved_arm) in arms.iter().zip(resolved_arms) {
                    self.pattern(&arm.pattern, &resolved_arm.pattern)?;
                    self.expr(&arm.value, &resolved_arm.value, tokens)?;
                }
            }
            (
                ExprKind::UpdateRecord { base, fields },
                ResolvedExprKind::UpdateRecord {
                    base: resolved_base,
                    record,
                    fields: resolved_fields,
                },
            ) => {
                self.expr(base, resolved_base, tokens)?;
                if fields.len() != resolved_fields.len() {
                    return None;
                }
                for (field, resolved_field) in fields.iter().zip(resolved_fields) {
                    self.member(
                        record.as_str(),
                        resolved_field.field.as_str(),
                        field.name_span,
                        None,
                    );
                    self.expr(&field.value, &resolved_field.value, tokens)?;
                }
            }
            (
                ExprKind::Project {
                    base, field_span, ..
                },
                ResolvedExprKind::Project {
                    base: resolved_base,
                    field,
                },
            ) => {
                self.expr(base, resolved_base, tokens)?;
                let owner = resolved_base.ty.nominal_id()?;
                self.member(owner.as_str(), field.as_str(), *field_span, None);
            }
            (ExprKind::Project { .. }, ResolvedExprKind::Place(place)) => {
                let mut spans = Vec::new();
                collect_project_spans(source, &mut spans);
                if spans.len() != place.projections.len() {
                    return None;
                }
                for (span, projection) in spans.into_iter().zip(&place.projections) {
                    let PlaceProjection::Field(field) = projection else {
                        return None;
                    };
                    let owner = self.member_owners.get(field.as_str())?.clone();
                    self.member(&owner, field.as_str(), span, None);
                }
            }
            (ExprKind::Int(_), ResolvedExprKind::Int(_))
            | (ExprKind::Bool(_), ResolvedExprKind::Bool(_))
            | (ExprKind::Var(_), ResolvedExprKind::Place(_)) => {}
            _ => return None,
        }
        Some(())
    }

    fn expr_pairs(
        &mut self,
        source: &[Expr],
        resolved: &[ResolvedExpr],
        tokens: &[Token],
    ) -> Option<()> {
        if source.len() != resolved.len() {
            return None;
        }
        for (source, resolved) in source.iter().zip(resolved) {
            self.expr(source, resolved, tokens)?;
        }
        Some(())
    }

    fn pattern(&mut self, source: &MatchPattern, resolved: &ResolvedMatchPattern) -> Option<()> {
        match (source, resolved) {
            (
                MatchPattern::Variant {
                    case_span, fields, ..
                },
                ResolvedMatchPattern::Variant {
                    variant,
                    case,
                    fields: resolved_fields,
                },
            ) => {
                self.case(variant.as_str(), case.as_str(), *case_span);
                if fields.len() != resolved_fields.len() {
                    return None;
                }
                for (field, resolved_field) in fields.iter().zip(resolved_fields) {
                    let shorthand =
                        (field.name_span == field.binding_span).then(|| field.binding.clone());
                    self.member(
                        case.as_str(),
                        resolved_field.field.as_str(),
                        field.name_span,
                        shorthand,
                    );
                }
            }
            (
                MatchPattern::Record { fields, .. },
                ResolvedMatchPattern::Record {
                    record,
                    fields: resolved_fields,
                    ..
                },
            ) => self.record_pattern(record.as_str(), fields, resolved_fields)?,
            (MatchPattern::Wildcard { .. }, ResolvedMatchPattern::Wildcard) => {}
            _ => return None,
        }
        Some(())
    }

    fn record_pattern(
        &mut self,
        owner: &str,
        source: &[crate::ast::RecordMatchPatternField],
        resolved: &[crate::hir::ResolvedRecordMatchPatternField],
    ) -> Option<()> {
        if source.len() != resolved.len() {
            return None;
        }
        for (field, resolved_field) in source.iter().zip(resolved) {
            let shorthand = match &field.pattern {
                RecordMatchFieldPattern::Binding { name, span } if *span == field.name_span => {
                    Some(name.clone())
                }
                _ => None,
            };
            self.member(
                owner,
                resolved_field.field.as_str(),
                field.name_span,
                shorthand,
            );
            if let (
                RecordMatchFieldPattern::Record { fields, .. },
                ResolvedRecordMatchFieldPattern::Record {
                    record,
                    fields: resolved_fields,
                    ..
                },
            ) = (&field.pattern, &resolved_field.pattern)
            {
                self.record_pattern(record.as_str(), fields, resolved_fields)?;
            }
        }
        Some(())
    }
}

fn collect_project_spans(expression: &Expr, spans: &mut Vec<Span>) {
    if let ExprKind::Project {
        base, field_span, ..
    } = &expression.kind
    {
        collect_project_spans(base, spans);
        spans.push(*field_span);
    }
}

fn call_type_argument_spans(span: Span, count: usize, tokens: &[Token]) -> Option<Vec<Span>> {
    if count == 0 {
        return Some(Vec::new());
    }
    let inside: Vec<_> = tokens
        .iter()
        .filter(|token| token.span.start >= span.start && token.span.end <= span.end)
        .collect();
    let start = inside
        .iter()
        .position(|token| matches!(token.kind, TokenKind::Lt))?;
    let mut depth = 0usize;
    let mut spans = Vec::new();
    for token in inside.into_iter().skip(start) {
        match token.kind {
            TokenKind::Lt => depth += 1,
            TokenKind::Gt => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    break;
                }
            }
            TokenKind::Ident(_) if depth == 1 => spans.push(token.span),
            _ => {}
        }
    }
    (spans.len() == count).then_some(spans)
}
