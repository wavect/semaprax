//! Record, variant, and record-update construction frames: per-field
//! preparation and resumption.

use crate::ast::{
    Expr, FieldDeclaration, ParamMode, Type, TypeDeclaration, VariantCaseDeclaration,
};
use crate::diagnostic::Diagnostic;
use crate::source_verify::binding::CheckedValue;
use crate::source_verify::diagnostics::{error, reject_native_unit_value};
use crate::source_verify::loans::mark_value_sources_moved;
use crate::source_verify::place::source_place;
use crate::source_verify::scope::VerifierFrame;
use crate::source_verify::type_table::{effective_record_fields, TypeTable};
use crate::source_verify::IterativeVerifier;
use std::collections::HashSet;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_prepare_record_field(
        &mut self,
        expression: &'p Expr,
        type_name: &'p str,
        type_arguments: &'p [Type],
        fields: &'p [crate::ast::FieldInitializer],
        declared_fields: Option<&'p [FieldDeclaration]>,
        scope: usize,
        index: usize,
        mut supplied: HashSet<&'p str>,
    ) -> Result<(), Diagnostic> {
        let field = &fields[index];
        let declared = self
            .types
            .declared_field(type_name, &field.name)
            .or_else(|| {
                declared_fields.and_then(|declared| {
                    declared
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                })
            });
        if !supplied.insert(field.name.as_str()) || declared.is_none() {
            self.diagnostics.push(error(
                self.program,
                "SPX-T212",
                format!(
                    "unknown or duplicate field `{}` in `{type_name}` construction",
                    field.name
                ),
                field.span,
            ));
        }
        self.frames.push(VerifierFrame::ResumeRecordField {
            expression,
            type_name,
            type_arguments,
            fields,
            declared_fields,
            scope,
            index,
            supplied,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: &field.value,
            scope,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_resume_record_field(
        &mut self,
        expression: &'p Expr,
        type_name: &'p str,
        type_arguments: &'p [Type],
        fields: &'p [crate::ast::FieldInitializer],
        declared_fields: Option<&'p [FieldDeclaration]>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'p str>,
    ) -> Result<(), Diagnostic> {
        let actual = self.values.pop().unwrap_or(None);
        let field = &fields[index];
        let declared = self
            .types
            .declared_field(type_name, &field.name)
            .or_else(|| {
                declared_fields.and_then(|declared| {
                    declared
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                })
            });
        if let (Some(declared), Some(actual)) = (declared, actual) {
            reject_native_unit_value(self.program, &field.value, &actual, self.diagnostics);
            let expected = self
                .types
                .declaration(type_name)
                .and_then(|declaration| {
                    TypeTable::substitute_variant_type(declaration, type_arguments, &declared.ty)
                })
                .unwrap_or_else(|| declared.ty.clone());
            if actual.ty != expected {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T215",
                    format!(
                        "field `{}.{}` expects {}, received {}",
                        type_name, field.name, expected, actual.ty
                    ),
                    field.value.span,
                ));
            }
            if self.types.needs_drop(&declared.ty) && actual.mode == ParamMode::Own {
                if self.allow_moves {
                    mark_value_sources_moved(
                        self.program,
                        &field.value,
                        &mut self.scopes[scope].bindings,
                        self.types,
                        self.diagnostics,
                    );
                } else {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-O105",
                        "contract expression cannot transfer an owned record field",
                        field.value.span,
                    ));
                }
            } else if self.types.needs_drop(&declared.ty)
                && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-O108",
                    "cannot move an owned field through a borrowed or shared record",
                    field.value.span,
                ));
            }
        }
        let next = index + 1;
        if fields.get(next).is_some() {
            self.frames.push(VerifierFrame::PrepareRecordField {
                expression,
                type_name,
                type_arguments,
                fields,
                declared_fields,
                scope,
                index: next,
                supplied,
            });
        } else {
            if let Some(declared_fields) = declared_fields {
                for field in declared_fields {
                    if !supplied.contains(field.name.as_str()) {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T213",
                            format!(
                                "record `{type_name}` construction is missing field `{}`",
                                field.name
                            ),
                            expression.span,
                        ));
                    }
                }
                let instance = Type::Named {
                    name: type_name.to_owned(),
                    arguments: type_arguments.to_vec(),
                };
                self.values.push(Some(CheckedValue::returned(
                    instance.clone(),
                    self.types.needs_drop(&instance),
                )));
            } else {
                self.values.push(None);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_prepare_variant_field(
        &mut self,
        expression: &'p Expr,
        type_name: &'p str,
        type_arguments: &'p [Type],
        case_name: &'p str,
        fields: &'p [crate::ast::FieldInitializer],
        declaration: Option<&'p TypeDeclaration>,
        case: Option<&'p VariantCaseDeclaration>,
        scope: usize,
        index: usize,
        mut supplied: HashSet<&'p str>,
    ) -> Result<(), Diagnostic> {
        let field = &fields[index];
        let declared = case.and_then(|case| {
            case.fields
                .iter()
                .find(|candidate| candidate.name == field.name)
        });
        if !supplied.insert(field.name.as_str()) || (case.is_some() && declared.is_none()) {
            self.diagnostics.push(error(self.program, "SPX-T212", format!("unknown or duplicate payload field `{}` in `{type_name}::{case_name}` construction", field.name), field.span));
        }
        self.frames.push(VerifierFrame::ResumeVariantField {
            expression,
            type_name,
            type_arguments,
            case_name,
            fields,
            declaration,
            case,
            scope,
            index,
            supplied,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: &field.value,
            scope,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_resume_variant_field(
        &mut self,
        expression: &'p Expr,
        type_name: &'p str,
        type_arguments: &'p [Type],
        case_name: &'p str,
        fields: &'p [crate::ast::FieldInitializer],
        declaration: Option<&'p TypeDeclaration>,
        case: Option<&'p VariantCaseDeclaration>,
        scope: usize,
        index: usize,
        supplied: HashSet<&'p str>,
    ) -> Result<(), Diagnostic> {
        let actual = self.values.pop().unwrap_or(None);
        let field = &fields[index];
        let declared = case.and_then(|case| {
            case.fields
                .iter()
                .find(|candidate| candidate.name == field.name)
        });
        if let (Some(declaration), Some(declared), Some(actual)) = (declaration, declared, actual) {
            reject_native_unit_value(self.program, &field.value, &actual, self.diagnostics);
            let expected =
                TypeTable::substitute_variant_type(declaration, type_arguments, &declared.ty)
                    .unwrap_or_else(|| declared.ty.clone());
            if actual.ty != expected {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T215",
                    format!(
                        "payload `{}::{}.{}` expects {}, received {}",
                        type_name, case_name, field.name, expected, actual.ty
                    ),
                    field.value.span,
                ));
            }
            if self.types.needs_drop(&expected) && actual.mode == ParamMode::Own {
                if self.allow_moves {
                    mark_value_sources_moved(
                        self.program,
                        &field.value,
                        &mut self.scopes[scope].bindings,
                        self.types,
                        self.diagnostics,
                    );
                } else {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-O105",
                        "contract expression cannot transfer an owned variant payload",
                        field.value.span,
                    ));
                }
            } else if self.types.needs_drop(&expected)
                && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-O108",
                    "cannot move an owned variant payload through a borrowed or shared value",
                    field.value.span,
                ));
            }
        }
        let next = index + 1;
        if fields.get(next).is_some() {
            self.frames.push(VerifierFrame::PrepareVariantField {
                expression,
                type_name,
                type_arguments,
                case_name,
                fields,
                declaration,
                case,
                scope,
                index: next,
                supplied,
            });
        } else {
            if let Some(case) = case {
                for field in &case.fields {
                    if !supplied.contains(field.name.as_str()) {
                        self.diagnostics.push(error(self.program, "SPX-T213", format!("variant construction `{type_name}::{case_name}` is missing payload field `{}`", field.name), expression.span));
                    }
                }
                let instance = Type::Named {
                    name: type_name.to_owned(),
                    arguments: type_arguments.to_vec(),
                };
                self.values.push(Some(CheckedValue::returned(
                    instance.clone(),
                    self.types.needs_drop(&instance),
                )));
            } else if declaration.is_some() {
                let instance = Type::Named {
                    name: type_name.to_owned(),
                    arguments: type_arguments.to_vec(),
                };
                self.values.push(Some(CheckedValue::returned(
                    instance.clone(),
                    self.types.needs_drop(&instance),
                )));
            } else {
                self.values.push(None);
            }
        }
        Ok(())
    }

    pub(super) fn frame_resume_update_base(
        &mut self,
        expression: &'p Expr,
        base: &'p Expr,
        fields: &'p [crate::ast::FieldInitializer],
        scope: usize,
    ) -> Result<(), Diagnostic> {
        let Some(base_value) = self.values.pop().flatten() else {
            self.values.push(None);
            return Ok(());
        };
        reject_native_unit_value(self.program, base, &base_value, self.diagnostics);
        let Some(declared_fields) = effective_record_fields(self.types, &base_value.ty) else {
            self.diagnostics.push(error(
                self.program,
                "SPX-T215",
                format!(
                    "record update requires a record base, received {}",
                    base_value.ty
                ),
                base.span,
            ));
            self.values.push(None);
            return Ok(());
        };
        let nested_update = self.types.is_nested_owned_byte_record(&base_value.ty)
            && !self.types.is_flat_owned_byte_record(&base_value.ty);
        if nested_update
            && source_place(base, &self.scopes[scope].bindings, self.types)
                .is_none_or(|place| !place.projections.is_empty())
        {
            self.diagnostics.push(error(
                self.program,
                "SPX-O117",
                "nested owned-record update requires an exact named owned base place",
                expression.span,
            ));
        }
        if self.types.needs_drop(&base_value.ty) {
            match base_value.mode {
                ParamMode::Own if self.allow_moves => mark_value_sources_moved(
                    self.program,
                    base,
                    &mut self.scopes[scope].bindings,
                    self.types,
                    self.diagnostics,
                ),
                ParamMode::Own => self.diagnostics.push(error(
                    self.program,
                    "SPX-O105",
                    "contract expression cannot transfer an owned record update base",
                    base.span,
                )),
                ParamMode::Borrow | ParamMode::Shared => self.diagnostics.push(error(
                    self.program,
                    "SPX-O108",
                    "cannot update an owned record through a borrowed or shared base",
                    base.span,
                )),
                ParamMode::Value => {}
            }
        }
        if !fields.is_empty() {
            self.frames.push(VerifierFrame::PrepareUpdateField {
                expression,
                base_type: base_value.ty,
                fields,
                declared_fields,
                scope,
                index: 0,
                supplied: HashSet::new(),
            });
        } else {
            self.values.push(Some(CheckedValue::returned(
                base_value.ty.clone(),
                self.types.needs_drop(&base_value.ty),
            )));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_prepare_update_field(
        &mut self,
        expression: &'p Expr,
        base_type: Type,
        fields: &'p [crate::ast::FieldInitializer],
        declared_fields: &'p [FieldDeclaration],
        scope: usize,
        index: usize,
        mut supplied: HashSet<&'p str>,
    ) -> Result<(), Diagnostic> {
        let field = &fields[index];
        let declared = declared_fields
            .iter()
            .find(|candidate| candidate.name == field.name);
        if !supplied.insert(field.name.as_str()) || declared.is_none() {
            self.diagnostics.push(error(
                self.program,
                "SPX-T212",
                format!(
                    "unknown or duplicate field `{}` in `{}` update",
                    field.name, base_type
                ),
                field.span,
            ));
        }
        self.frames.push(VerifierFrame::ResumeUpdateField {
            expression,
            base_type,
            fields,
            declared_fields,
            scope,
            index,
            supplied,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: &field.value,
            scope,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_resume_update_field(
        &mut self,
        expression: &'p Expr,
        base_type: Type,
        fields: &'p [crate::ast::FieldInitializer],
        declared_fields: &'p [FieldDeclaration],
        scope: usize,
        index: usize,
        supplied: HashSet<&'p str>,
    ) -> Result<(), Diagnostic> {
        let actual = self.values.pop().unwrap_or(None);
        let field = &fields[index];
        let declared = declared_fields
            .iter()
            .find(|candidate| candidate.name == field.name);
        if let (Some(declared), Some(actual)) = (declared, actual) {
            reject_native_unit_value(self.program, &field.value, &actual, self.diagnostics);
            let expected = self
                .types
                .record_field_type(&base_type, declared)
                .unwrap_or_else(|| declared.ty.clone());
            if actual.ty != expected {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-T215",
                    format!(
                        "field `{}.{}` expects {}, received {}",
                        base_type, field.name, expected, actual.ty
                    ),
                    field.value.span,
                ));
            }
            if self.types.needs_drop(&expected) && actual.mode == ParamMode::Own {
                if self.allow_moves {
                    mark_value_sources_moved(
                        self.program,
                        &field.value,
                        &mut self.scopes[scope].bindings,
                        self.types,
                        self.diagnostics,
                    );
                } else {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-O105",
                        "contract expression cannot transfer an owned record replacement",
                        field.value.span,
                    ));
                }
            } else if self.types.needs_drop(&expected)
                && matches!(actual.mode, ParamMode::Borrow | ParamMode::Shared)
            {
                self.diagnostics.push(error(
                    self.program,
                    "SPX-O108",
                    "cannot move an owned replacement through a borrowed or shared value",
                    field.value.span,
                ));
            }
        }
        let next = index + 1;
        if fields.get(next).is_some() {
            self.frames.push(VerifierFrame::PrepareUpdateField {
                expression,
                base_type,
                fields,
                declared_fields,
                scope,
                index: next,
                supplied,
            });
        } else {
            self.values.push(Some(CheckedValue::returned(
                base_type.clone(),
                self.types.needs_drop(&base_type),
            )));
        }
        Ok(())
    }
}
