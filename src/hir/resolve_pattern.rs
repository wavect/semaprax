//! Match pattern lowering and refutable-match admission.

use std::collections::BTreeMap;

use crate::ast::Span;
use crate::diagnostic::Diagnostic;

use super::expr_nodes::{
    PatternValue, ResolvedMatchPattern, ResolvedRecordMatchFieldPattern,
    ResolvedRecordMatchPatternField,
};
use super::ids::{DeclarationId, FunctionExecutionId, ValueId};
use super::monomorphize::substitute_type;
use super::nodes::{
    DeclarationKind, OwnershipMode, ResolvedBinding, ResolvedFieldDeclaration, ResolvedMatchMode,
    ResolvedType,
};
use super::{Binding, Resolver};

impl Resolver<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_record_match_pattern(
        &self,
        function: &FunctionExecutionId,
        expected: &ResolvedType,
        type_name: &str,
        fields: &[crate::ast::RecordMatchPatternField],
        bindings: &mut BTreeMap<String, Binding>,
        path: &str,
        span: Span,
        mode: ResolvedMatchMode,
    ) -> Result<ResolvedMatchPattern, Diagnostic> {
        enum Frame<'a> {
            Enter {
                expected: ResolvedType,
                type_name: &'a str,
                fields: &'a [crate::ast::RecordMatchPatternField],
                path: String,
                span: Span,
            },
            Fields {
                expected: ResolvedType,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                templates: &'a [ResolvedFieldDeclaration],
                fields: &'a [crate::ast::RecordMatchPatternField],
                index: usize,
                resolved: Vec<ResolvedRecordMatchPatternField>,
                path: String,
            },
            AfterNested {
                expected: ResolvedType,
                record: DeclarationId,
                arguments: Vec<ResolvedType>,
                templates: &'a [ResolvedFieldDeclaration],
                fields: &'a [crate::ast::RecordMatchPatternField],
                index: usize,
                resolved: Vec<ResolvedRecordMatchPatternField>,
                path: String,
                field: DeclarationId,
            },
        }
        let mut frames = vec![Frame::Enter {
            expected: expected.clone(),
            type_name,
            fields,
            path: path.to_owned(),
            span,
        }];
        let mut results = Vec::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter {
                    expected,
                    type_name,
                    fields,
                    path,
                    span,
                } => {
                    let ResolvedType::Nominal {
                        declaration: record,
                        arguments,
                    } = &expected
                    else {
                        return Err(self.error(
                            "SPX-H001",
                            "record pattern has a non-record concrete instance",
                            span,
                        ));
                    };
                    if self.declarations.type_id(type_name) != Some(record)
                        || self
                            .declarations
                            .declaration(record)
                            .is_none_or(|item| item.kind != DeclarationKind::Record)
                    {
                        return Err(self.error(
                            "SPX-H001",
                            format!("record pattern `{type_name}` does not match `{record}`"),
                            span,
                        ));
                    }
                    let templates = self.declarations.record_fields(record).ok_or_else(|| {
                        self.error("SPX-H006", "record pattern has no fields", span)
                    })?;
                    let record = record.clone();
                    let arguments = arguments.clone();
                    frames.push(Frame::Fields {
                        expected,
                        record,
                        arguments,
                        templates,
                        fields,
                        index: 0,
                        resolved: Vec::with_capacity(fields.len()),
                        path,
                    });
                }
                Frame::Fields {
                    expected,
                    record,
                    arguments,
                    templates,
                    fields,
                    index,
                    mut resolved,
                    path,
                } => {
                    let Some(field) = fields.get(index) else {
                        results.push(ResolvedMatchPattern::Record {
                            record,
                            instance: expected,
                            fields: resolved,
                        });
                        continue;
                    };
                    let field_id = self
                        .declarations
                        .field_id(&record, &field.name)
                        .cloned()
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H001",
                                format!(
                                    "unresolved record pattern field `{record}.{}`",
                                    field.name
                                ),
                                field.span,
                            )
                        })?;
                    let template = templates
                        .iter()
                        .find(|candidate| candidate.id == field_id)
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("record pattern field `{field_id}` has no template"),
                                field.span,
                            )
                        })?;
                    let field_ty = substitute_type(&template.ty, &record, &arguments)?;
                    let field_path = format!("{path}.field.{index}");
                    match &field.pattern {
                        crate::ast::RecordMatchFieldPattern::Binding { name, span } => {
                            let field_facts =
                                self.declarations.type_facts(&field_ty).ok_or_else(|| {
                                    self.error(
                                        "SPX-H006",
                                        "record pattern field has no authenticated type facts",
                                        *span,
                                    )
                                })?;
                            let ownership = if field_facts.needs_drop {
                                match mode {
                                    ResolvedMatchMode::Own => OwnershipMode::Own,
                                    ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                                    ResolvedMatchMode::Value => OwnershipMode::Value,
                                }
                            } else {
                                OwnershipMode::Value
                            };
                            let binding = ResolvedBinding {
                                id: ValueId::local(function, &format!("{field_path}.binding")),
                                name: name.clone(),
                                ownership,
                                ty: field_ty.clone(),
                                span: *span,
                            };
                            bindings.insert(
                                name.clone(),
                                Binding {
                                    id: binding.id.clone(),
                                    ty: field_ty,
                                    ownership,
                                    mutable: false,
                                },
                            );
                            resolved.push(ResolvedRecordMatchPatternField {
                                field: field_id,
                                pattern: ResolvedRecordMatchFieldPattern::Binding(binding),
                            });
                            frames.push(Frame::Fields {
                                expected,
                                record,
                                arguments,
                                templates,
                                fields,
                                index: index + 1,
                                resolved,
                                path,
                            });
                        }
                        crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {
                            resolved.push(ResolvedRecordMatchPatternField {
                                field: field_id,
                                pattern: ResolvedRecordMatchFieldPattern::Wildcard,
                            });
                            frames.push(Frame::Fields {
                                expected,
                                record,
                                arguments,
                                templates,
                                fields,
                                index: index + 1,
                                resolved,
                                path,
                            });
                        }
                        crate::ast::RecordMatchFieldPattern::Record {
                            type_name,
                            fields: nested,
                            span,
                            ..
                        } => {
                            frames.push(Frame::AfterNested {
                                expected,
                                record,
                                arguments,
                                templates,
                                fields,
                                index,
                                resolved,
                                path: path.clone(),
                                field: field_id,
                            });
                            frames.push(Frame::Enter {
                                expected: field_ty,
                                type_name,
                                fields: nested,
                                path: format!("{field_path}.record"),
                                span: *span,
                            });
                        }
                    }
                }
                Frame::AfterNested {
                    expected,
                    record,
                    arguments,
                    templates,
                    fields,
                    index,
                    mut resolved,
                    path,
                    field,
                } => {
                    let ResolvedMatchPattern::Record {
                        record: nested_record,
                        instance,
                        fields: nested_fields,
                    } = results.pop().expect("nested record result retained")
                    else {
                        unreachable!("nested resolver returns a record pattern")
                    };
                    resolved.push(ResolvedRecordMatchPatternField {
                        field,
                        pattern: ResolvedRecordMatchFieldPattern::Record {
                            record: nested_record,
                            instance,
                            fields: nested_fields,
                        },
                    });
                    frames.push(Frame::Fields {
                        expected,
                        record,
                        arguments,
                        templates,
                        fields,
                        index: index + 1,
                        resolved,
                        path,
                    });
                }
            }
        }
        Ok(results.pop().expect("root record pattern result retained"))
    }

    /// Refutable Match v1 admission over a Copy-scalar scrutinee: literal
    /// patterns must compare against exactly the scrutinee type
    /// (`SPX-T255`), or-patterns stay flat and same-typed (`SPX-M105`),
    /// aggregate patterns never mix with scalar scrutinees (`SPX-H001`), and
    /// a refutable match requires one trailing irrefutable guard-free
    /// catch-all arm (`SPX-T257`).
    pub(super) fn validate_refutable_match_admission(
        &self,
        scrutinee: &ResolvedType,
        arms: &[crate::ast::MatchArm],
    ) -> Result<(), Diagnostic> {
        for arm in arms {
            match &arm.pattern {
                crate::ast::MatchPattern::Wildcard { .. }
                | crate::ast::MatchPattern::Binding { .. } => {}
                crate::ast::MatchPattern::Literal { value, span } => {
                    if PatternValue::from_ast(*value).ty() != *scrutinee {
                        return Err(self.error(
                            "SPX-T255",
                            format!(
                                "literal pattern of type `{}` cannot match a `{}` scrutinee; \
                                 pattern literals compare against exactly their own type",
                                value.type_text(),
                                scrutinee.identity_key()
                            ),
                            *span,
                        ));
                    }
                }
                crate::ast::MatchPattern::Or { alternatives, span } => {
                    let mut alternative_type: Option<&'static str> = None;
                    for alternative in alternatives {
                        let crate::ast::MatchPattern::Literal { value, span } = alternative else {
                            return Err(self.error(
                                "SPX-M105",
                                "or-patterns admit only literal alternatives in v1",
                                alternative.span(),
                            ));
                        };
                        if PatternValue::from_ast(*value).ty() != *scrutinee {
                            return Err(self.error(
                                "SPX-T255",
                                format!(
                                    "literal pattern of type `{}` cannot match a `{}` scrutinee; \
                                     pattern literals compare against exactly their own type",
                                    value.type_text(),
                                    scrutinee.identity_key()
                                ),
                                *span,
                            ));
                        }
                        let type_text = value.type_text();
                        match alternative_type {
                            None => alternative_type = Some(type_text),
                            Some(seen) if seen == type_text => {}
                            Some(seen) => {
                                return Err(self.error(
                                    "SPX-M105",
                                    format!(
                                        "or-pattern mixes `{seen}` and `{type_text}` literal \
                                         alternatives; all alternatives must share one type"
                                    ),
                                    *span,
                                ));
                            }
                        }
                    }
                    if alternatives.is_empty() {
                        return Err(self.error(
                            "SPX-M105",
                            "or-pattern needs at least one literal alternative",
                            *span,
                        ));
                    }
                }
                crate::ast::MatchPattern::Variant { span, .. }
                | crate::ast::MatchPattern::Record { span, .. } => {
                    return Err(self.error(
                        "SPX-H001",
                        "aggregate pattern has a Copy-scalar scrutinee",
                        *span,
                    ));
                }
            }
        }
        let last = arms.last().expect("match always has arm syntax");
        let catch_all = matches!(
            &last.pattern,
            crate::ast::MatchPattern::Wildcard { .. } | crate::ast::MatchPattern::Binding { .. }
        );
        if !catch_all || last.guard.is_some() {
            return Err(self.error(
                "SPX-T257",
                "refutable match requires a trailing irrefutable catch-all arm \
                 (`_` or a binding) without a guard",
                last.span,
            ));
        }
        Ok(())
    }
}
