//! Program-level AST lowering.
//!
//! Entry point resolution, record layout validation, function and
//! function-template lowering, instance discovery, and type lowering.

use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{
    ImportFailure, ParamMode, ResourceLifecycleKind, Span, Type, TypeDeclarationKind,
};
use crate::cleanup::CleanupInventory;
use crate::cleanup_plan::CleanupPlan;
use crate::diagnostic::Diagnostic;
use crate::loan_plan::LoanPlan;

use super::byte_capacity::analyze_byte_data_capacity;
use super::byte_slice_provenance::derive_byte_slice_provenance;
use super::ids::{DeclarationId, FunctionExecutionId, FunctionInstanceId, ValueId};
use super::monomorphize::specialize_source_function;
use super::nodes::{
    admitted_owned_byte_prelude_instance, OwnershipMode, ResolvedFunction,
    ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedImport, ResolvedImportFailure,
    ResolvedImportParameter, ResolvedImportResult, ResolvedImportResultKind, ResolvedInterface,
    ResolvedParam, ResolvedProgram, ResolvedResourceDrop, ResolvedResourceDropKind, ResolvedType,
    ResolvedTypeDeclaration, ResolvedTypeDeclarationKind, ResolvedTypeParameterDeclaration,
};
use super::{validate, Binding, Resolver};

impl Resolver<'_> {
    pub(super) fn resolve(
        mut self,
    ) -> Result<(ResolvedProgram, super::FunctionResolutionWork), Diagnostic> {
        let entrypoint = self
            .program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .map(|function| DeclarationId::new(function.stable_id.clone()))
            .ok_or_else(|| {
                self.error(
                    "SPX-H005",
                    "verified program has no resolved entry point",
                    Span::default(),
                )
            })?;
        self.validate_record_layouts()?;
        let types = self
            .program
            .types
            .iter()
            .chain(crate::prelude::declarations())
            .map(|declaration| {
                let id = DeclarationId::new(declaration.stable_id.clone());
                let kind = match &declaration.kind {
                    TypeDeclarationKind::Resource { lifecycles } => {
                        let lifecycle = lifecycles.first().ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("resource `{id}` has no resolved lifecycle"),
                                declaration.span,
                            )
                        })?;
                        let lifecycle_id = DeclarationId::new(
                            lifecycle.stable_id.clone().ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("resource `{id}` lifecycle has no identity"),
                                    lifecycle.span,
                                )
                            })?,
                        );
                        let drop_kind = match &lifecycle.kind {
                            ResourceLifecycleKind::Trivial => ResolvedResourceDropKind::Trivial,
                            ResourceLifecycleKind::Imported { import_key } => {
                                let import = self
                                    .declarations
                                    .import_id(import_key)
                                    .cloned()
                                    .ok_or_else(|| {
                                        self.error(
                                            "SPX-H006",
                                            format!(
                                                "resource `{id}` lifecycle references unknown import key `{import_key}`"
                                            ),
                                            lifecycle.span,
                                        )
                                    })?;
                                ResolvedResourceDropKind::Imported {
                                    import,
                                    import_key: import_key.clone(),
                                }
                            }
                        };
                        ResolvedTypeDeclarationKind::Resource {
                            drop: ResolvedResourceDrop {
                                id: lifecycle_id,
                                kind: drop_kind,
                            },
                        }
                    }
                    TypeDeclarationKind::Record { .. } => {
                        let fields = self
                            .declarations
                            .record_fields(&id)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("record `{id}` has no resolved fields"),
                                    declaration.span,
                                )
                            })?
                            .to_vec();
                        ResolvedTypeDeclarationKind::Record { fields }
                    }
                    TypeDeclarationKind::Class { methods, .. } => {
                        let fields = self
                            .declarations
                            .record_fields(&id)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("class `{id}` has no resolved fields"),
                                    declaration.span,
                                )
                            })?
                            .to_vec();
                        let methods = methods
                            .iter()
                            .map(|method| DeclarationId::new(method.stable_id.clone()))
                            .collect();
                        ResolvedTypeDeclarationKind::Class { fields, methods }
                    }
                    TypeDeclarationKind::Variant { .. } => {
                        let cases = self
                            .declarations
                            .variant_cases(&id)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("variant `{id}` has no resolved cases"),
                                    declaration.span,
                                )
                            })?
                            .to_vec();
                        ResolvedTypeDeclarationKind::Variant { cases }
                    }
                };
                Ok(ResolvedTypeDeclaration {
                    type_parameters: self
                        .declarations
                        .type_parameters(&id)
                        .ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("type `{id}` has no parameter metadata"),
                                declaration.span,
                            )
                        })?
                        .to_vec(),
                    id,
                    name: declaration.name.clone(),
                    kind,
                    span: declaration.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let interfaces = self
            .program
            .interfaces
            .iter()
            .map(|interface| {
                let interface_id = DeclarationId::new(interface.stable_id.clone());
                let imports = interface
                    .imports
                    .iter()
                    .map(|import| {
                        let parameters = import
                            .params
                            .iter()
                            .map(|param| {
                                Ok(ResolvedImportParameter {
                                    name: param.name.clone(),
                                    ty: self.resolve_type(&param.ty, param.span)?,
                                    ownership: param.mode.into(),
                                    consumes_on_failure: param.name == import.consumes,
                                })
                            })
                            .collect::<Result<Vec<_>, Diagnostic>>()?;
                        let failure = match &import.failure {
                            ImportFailure::Infallible => ResolvedImportFailure::Infallible,
                            ImportFailure::Status { domain_id } => ResolvedImportFailure::Status {
                                domain_id: domain_id.clone(),
                                normalization: "semaprax.status.v1",
                            },
                        };
                        Ok(ResolvedImport {
                            id: DeclarationId::new(import.stable_id.clone()),
                            name: import.name.clone(),
                            interface: interface_id.clone(),
                            import_key: import.stable_id.clone(),
                            native_rust: import.native_rust,
                            parameters,
                            result: ResolvedImportResult {
                                kind: match import.result {
                                    crate::ast::ImportResult::Unit => {
                                        ResolvedImportResultKind::Unit
                                    }
                                    crate::ast::ImportResult::I64 => ResolvedImportResultKind::I64,
                                    crate::ast::ImportResult::Bool => {
                                        ResolvedImportResultKind::Bool
                                    }
                                },
                                ownership: OwnershipMode::Value,
                                producer: "callee",
                                out_slot_initialization: "success_only",
                                ownership_transfer: "final_zero_status_commit",
                            },
                            effects: import.effects.clone(),
                            required_authority: import.effects.clone(),
                            failure,
                            span: import.span,
                        })
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?;
                Ok(ResolvedInterface {
                    id: interface_id,
                    name: interface.name.clone(),
                    permits: interface.permits.clone(),
                    imports,
                    span: interface.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let mut functions = Vec::new();
        for function in self
            .program
            .functions
            .iter()
            .filter(|function| function.type_parameters.is_empty())
        {
            let (resolved, cost, reused) = self.resolve_or_reuse_function(function)?;
            if reused {
                self.function_work.reused += 1;
            }
            self.function_work
                .costs
                .insert(function.stable_id.clone(), cost);
            functions.push(resolved);
        }
        for decl in &self.program.types {
            if let TypeDeclarationKind::Class { methods, .. } = &decl.kind {
                for method in methods {
                    if method.type_parameters.is_empty() {
                        // Class declarations are part of the exact environment,
                        // but the first function-granular lane deliberately
                        // resolves their methods afresh.
                        functions.push(self.resolve_function(method)?);
                    }
                }
            }
        }
        let function_templates = self
            .program
            .functions
            .iter()
            .filter(|function| !function.type_parameters.is_empty())
            .map(|function| self.resolve_function_template(function))
            .collect::<Result<_, _>>()?;
        let function_instances = self.discover_function_instances()?;
        let byte_slice_roots = derive_byte_slice_provenance(&functions, &self.declarations)?;
        let mut declarations = self.declarations;
        declarations.byte_slice_roots = byte_slice_roots;
        let mut resolved = ResolvedProgram {
            module: self.program.module.clone(),
            permits: self.program.permits.clone(),
            entrypoint,
            declarations,
            types,
            interfaces,
            function_templates,
            functions,
            function_instances,
        };
        analyze_byte_data_capacity(&resolved)?;
        let loan_plans = resolved
            .functions
            .iter()
            .map(|function| crate::loan_plan::build_plan(&resolved, function))
            .collect::<Result<Vec<_>, _>>()?;
        for (function, loan_plan) in resolved.functions.iter_mut().zip(loan_plans) {
            function.loan_plan = loan_plan;
        }
        let instance_loan_plans = resolved
            .function_instances
            .iter()
            .map(|instance| crate::loan_plan::build_plan(&resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()?;
        for (instance, loan_plan) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_loan_plans)
        {
            instance.function.loan_plan = loan_plan;
        }
        let inventories = resolved
            .functions
            .iter()
            .map(|function| crate::cleanup::build_inventory(&resolved, function))
            .collect::<Result<Vec<_>, _>>()?;
        for (function, inventory) in resolved.functions.iter_mut().zip(inventories) {
            function.cleanup = inventory;
        }
        let instance_inventories = resolved
            .function_instances
            .iter()
            .map(|instance| crate::cleanup::build_inventory(&resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()?;
        for (instance, inventory) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_inventories)
        {
            instance.function.cleanup = inventory;
        }
        let cleanup_plans = resolved
            .functions
            .iter()
            .map(|function| crate::cleanup_plan::build_plan(&resolved, function))
            .collect::<Result<Vec<_>, _>>()?;
        for (function, cleanup_plan) in resolved.functions.iter_mut().zip(cleanup_plans) {
            function.cleanup_plan = cleanup_plan;
        }
        let instance_cleanup_plans = resolved
            .function_instances
            .iter()
            .map(|instance| crate::cleanup_plan::build_plan(&resolved, &instance.function))
            .collect::<Result<Vec<_>, _>>()?;
        for (instance, cleanup_plan) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_cleanup_plans)
        {
            instance.function.cleanup_plan = cleanup_plan;
        }
        validate(&resolved)?;
        Ok((resolved, self.function_work))
    }

    fn resolve_or_reuse_function(
        &self,
        function: &crate::ast::Function,
    ) -> Result<(ResolvedFunction, usize, bool), Diagnostic> {
        if let Some(reuse) = &self.reuse {
            let previous = reuse
                .program
                .functions
                .iter()
                .find(|previous| previous.stable_id == function.stable_id);
            let resolved = reuse
                .resolved
                .functions
                .iter()
                .find(|previous| previous.id.as_str() == function.stable_id);
            let cost = reuse.costs.get(&function.stable_id).copied();
            if let (Some(previous), Some(resolved), Some(cost)) = (previous, resolved, cost) {
                if previous == function {
                    if !crate::bounded_output::reserve_active(cost) {
                        return Err(self.error(
                            "SPX-H006",
                            "function reuse exceeds the active builder budget",
                            function.span,
                        ));
                    }
                    return Ok((resolved.clone(), cost, true));
                }
            }
        }
        let before = crate::bounded_output::active_remaining();
        let resolved = self.resolve_function(function)?;
        let cost = before
            .zip(crate::bounded_output::active_remaining())
            .map_or(0, |(before, after)| before.saturating_sub(after));
        Ok((resolved, cost, false))
    }

    pub(super) fn validate_record_layouts(&self) -> Result<(), Diagnostic> {
        for declaration in &self.program.types {
            if !matches!(
                &declaration.kind,
                TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. }
            ) {
                continue;
            }
            if !declaration.type_parameters.is_empty() {
                continue;
            }
            let ty = ResolvedType::Nominal {
                declaration: DeclarationId::new(declaration.stable_id.clone()),
                arguments: Vec::new(),
            };
            if self.declarations.type_facts(&ty).is_none() {
                return Err(self.error(
                    "SPX-T217",
                    format!(
                        "record `{}` has an illegal by-value recursive layout",
                        declaration.name
                    ),
                    declaration.span,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn resolve_function(
        &self,
        function: &crate::ast::Function,
    ) -> Result<ResolvedFunction, Diagnostic> {
        let template_id = DeclarationId::new(function.stable_id.clone());
        let function_scope = FunctionExecutionId::Monomorphic(template_id.clone());
        self.resolve_function_in_scope(function, &function_scope, template_id)
    }

    pub(super) fn resolve_function_template(
        &self,
        function: &crate::ast::Function,
    ) -> Result<ResolvedFunctionTemplate, Diagnostic> {
        let function_id = DeclarationId::new(function.stable_id.clone());
        let function_scope = FunctionExecutionId::Monomorphic(function_id.clone());
        let type_parameters = function
            .type_parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                Ok(ResolvedTypeParameterDeclaration {
                    name: parameter.name.clone(),
                    index: u32::try_from(index).map_err(|_| {
                        self.error(
                            "SPX-H006",
                            format!("function `{}` has too many type parameters", function.name),
                            parameter.span,
                        )
                    })?,
                    span: parameter.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let mut bindings = BTreeMap::new();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let ty = self.resolve_function_type(function, &param.ty, param.span)?;
                let id = ValueId::parameter(&function_scope, index);
                // Type parameters range over Copy i64/bool; concrete String
                // slots use the same implicit ownership as ordinary functions.
                let ownership = if ty == ResolvedType::String {
                    OwnershipMode::Own
                } else {
                    OwnershipMode::Value
                };
                bindings.insert(
                    param.name.clone(),
                    Binding {
                        id: id.clone(),
                        ty: ty.clone(),
                        ownership,
                        mutable: false,
                    },
                );
                Ok(ResolvedParam {
                    id,
                    name: param.name.clone(),
                    ownership,
                    ty,
                    span: param.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let return_type =
            self.resolve_function_type(function, &function.return_type, function.span)?;
        let result_id = ValueId::result(&function_scope);
        let requires = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    &function_scope,
                    expression,
                    &bindings,
                    &format!("requires.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        let body = self.resolve_expr(&function_scope, &function.body, &bindings, "body")?;
        let mut ensures_bindings = bindings;
        ensures_bindings.insert(
            "result".to_owned(),
            Binding {
                id: result_id.clone(),
                ty: return_type.clone(),
                ownership: if return_type == ResolvedType::String {
                    OwnershipMode::Own
                } else {
                    OwnershipMode::Value
                },
                mutable: false,
            },
        );
        let ensures = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    &function_scope,
                    expression,
                    &ensures_bindings,
                    &format!("ensures.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        Ok(ResolvedFunctionTemplate {
            id: function_id,
            name: function.name.clone(),
            type_parameters,
            params,
            result_id,
            return_type,
            effects: function.effects.clone(),
            requires,
            ensures,
            body,
            span: function.span,
        })
    }

    pub(super) fn discover_function_instances(
        &self,
    ) -> Result<Vec<ResolvedFunctionInstance>, Diagnostic> {
        let mut calls = Vec::new();
        for function in self
            .program
            .functions
            .iter()
            .filter(|function| function.type_parameters.is_empty())
        {
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                expression.visit_call_instances(&mut |name, arguments, span| {
                    calls.push((name.to_owned(), arguments.to_vec(), span));
                });
            }
        }

        let mut seen = BTreeSet::new();
        let mut instances = Vec::new();
        for (name, source_arguments, span) in calls {
            let Some(template) = self
                .program
                .functions
                .iter()
                .find(|function| function.name == name && !function.type_parameters.is_empty())
            else {
                continue;
            };
            let type_arguments = source_arguments
                .iter()
                .map(|argument| self.resolve_type(argument, span))
                .collect::<Result<Vec<_>, _>>()?;
            let template_id = DeclarationId::new(template.stable_id.clone());
            let id = FunctionInstanceId::derive(&template_id, &type_arguments);
            if !seen.insert(id.clone()) {
                continue;
            }
            let specialized =
                specialize_source_function(template, &source_arguments).ok_or_else(|| {
                    self.error(
                        "SPX-H006",
                        format!("generic function `{}` specialization failed", template.name),
                        span,
                    )
                })?;
            let execution = FunctionExecutionId::Generic(id.clone());
            let function =
                self.resolve_function_in_scope(&specialized, &execution, template_id.clone())?;
            instances.push(ResolvedFunctionInstance {
                id,
                template: template_id,
                type_arguments,
                function,
            });
        }
        Ok(instances)
    }

    pub(super) fn resolve_function_in_scope(
        &self,
        function: &crate::ast::Function,
        function_scope: &FunctionExecutionId,
        function_id: DeclarationId,
    ) -> Result<ResolvedFunction, Diagnostic> {
        let mut bindings = BTreeMap::new();
        let params = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| {
                let ty = self.resolve_type(&param.ty, param.span)?;
                let id = ValueId::parameter(function_scope, index);
                // `borrow Bytes` is the one admitted synchronous borrowed
                // owner carrier. Other uniquely-owned values, including
                // strings and source-value parameters, retain the established
                // implicit-Own normalization.
                let ownership = if ty == ResolvedType::Bytes && param.mode == ParamMode::Borrow {
                    OwnershipMode::Borrow
                } else if ty.is_uniquely_owned() {
                    OwnershipMode::Own
                } else if matches!(ty, ResolvedType::Str | ResolvedType::SliceU8) {
                    if param.mode != ParamMode::Borrow {
                        return Err(self.error(
                            "SPX-H006",
                            "resolved borrowed-view parameter must have borrow ownership",
                            param.span,
                        ));
                    }
                    OwnershipMode::Borrow
                } else {
                    param.mode.into()
                };
                bindings.insert(
                    param.name.clone(),
                    Binding {
                        id: id.clone(),
                        ty: ty.clone(),
                        ownership,
                        mutable: false,
                    },
                );
                Ok(ResolvedParam {
                    id,
                    name: param.name.clone(),
                    ownership,
                    ty,
                    span: param.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let return_type = self.resolve_type(&function.return_type, function.span)?;
        if return_type == ResolvedType::Str {
            return Err(self.error(
                "SPX-H006",
                "borrowed `str` cannot escape through a function result",
                function.span,
            ));
        }
        if return_type == ResolvedType::SliceU8 {
            return Err(self.error(
                "SPX-H006",
                "borrowed `Slice<u8>` cannot escape through a function result",
                function.span,
            ));
        }
        let result_id = ValueId::result(function_scope);

        let requires = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    function_scope,
                    expression,
                    &bindings,
                    &format!("requires.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;
        let body = self.resolve_expr(function_scope, &function.body, &bindings, "body")?;

        let mut ensures_bindings = bindings;
        ensures_bindings.insert(
            "result".to_owned(),
            Binding {
                id: result_id.clone(),
                ty: return_type.clone(),
                ownership: self.expression_ownership(
                    &return_type,
                    OwnershipMode::Own,
                    function.span,
                )?,
                mutable: false,
            },
        );
        let ensures = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, expression)| {
                self.resolve_expr(
                    function_scope,
                    expression,
                    &ensures_bindings,
                    &format!("ensures.{index}"),
                )
            })
            .collect::<Result<_, _>>()?;

        Ok(ResolvedFunction {
            id: function_id,
            name: function.name.clone(),
            params,
            result_id,
            return_type,
            effects: function.effects.clone(),
            requires,
            ensures,
            body,
            cleanup: CleanupInventory::unresolved(),
            cleanup_plan: CleanupPlan::unresolved(),
            loan_plan: LoanPlan::unresolved(),
            span: function.span,
        })
    }

    pub(super) fn resolve_type(&self, ty: &Type, span: Span) -> Result<ResolvedType, Diagnostic> {
        enum Frame<'a> {
            Enter(&'a Type),
            Arguments {
                declaration: DeclarationId,
                arguments: &'a [Type],
                index: usize,
                resolved: Vec<ResolvedType>,
            },
        }
        let mut frames = vec![Frame::Enter(ty)];
        let mut result = None;
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(Type::I64) => result = Some(ResolvedType::I64),
                Frame::Enter(Type::I32) => result = Some(ResolvedType::I32),
                Frame::Enter(Type::Char) => result = Some(ResolvedType::Char),
                Frame::Enter(Type::U8) => result = Some(ResolvedType::U8),
                Frame::Enter(Type::Usize) => result = Some(ResolvedType::Usize),
                Frame::Enter(Type::ArrayU8(length)) => {
                    result = Some(ResolvedType::ArrayU8(*length));
                }
                Frame::Enter(Type::F32) => result = Some(ResolvedType::F32),
                Frame::Enter(Type::F64) => result = Some(ResolvedType::F64),
                Frame::Enter(Type::Bool) => result = Some(ResolvedType::Bool),
                Frame::Enter(Type::String) => result = Some(ResolvedType::String),
                Frame::Enter(Type::Bytes) => result = Some(ResolvedType::Bytes),
                Frame::Enter(Type::Str) => result = Some(ResolvedType::Str),
                Frame::Enter(Type::SliceU8) => result = Some(ResolvedType::SliceU8),
                Frame::Enter(Type::Named { name, arguments }) => {
                    let declaration =
                        self.declarations.type_id(name).cloned().ok_or_else(|| {
                            self.error("SPX-H001", format!("unresolved type `{name}`"), span)
                        })?;
                    frames.push(Frame::Arguments {
                        declaration,
                        arguments,
                        index: 0,
                        resolved: Vec::with_capacity(arguments.len()),
                    });
                }
                Frame::Arguments {
                    declaration,
                    arguments,
                    index,
                    mut resolved,
                } => {
                    if index != 0 {
                        resolved.push(result.take().expect("resolved child type retained"));
                    }
                    if let Some(argument) = arguments.get(index) {
                        frames.push(Frame::Arguments {
                            declaration,
                            arguments,
                            index: index + 1,
                            resolved,
                        });
                        frames.push(Frame::Enter(argument));
                    } else {
                        let parameters = self
                            .declarations
                            .type_parameters(&declaration)
                            .ok_or_else(|| {
                                self.error(
                                    "SPX-H006",
                                    format!("type `{declaration}` has no parameter metadata"),
                                    span,
                                )
                            })?;
                        if resolved.len() != parameters.len()
                            || (!admitted_owned_byte_prelude_instance(&declaration, &resolved)
                                && !resolved.is_empty()
                                && resolved.iter().any(|argument| {
                                    !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                }))
                        {
                            return Err(self.error(
                                "SPX-H006",
                                format!("type `{declaration}` has invalid concrete arguments"),
                                span,
                            ));
                        }
                        result = Some(ResolvedType::Nominal {
                            declaration,
                            arguments: resolved,
                        });
                    }
                }
            }
        }
        Ok(result.expect("root type resolution produces a value"))
    }

    pub(super) fn resolve_function_type(
        &self,
        function: &crate::ast::Function,
        ty: &Type,
        span: Span,
    ) -> Result<ResolvedType, Diagnostic> {
        if let Type::Named { name, arguments } = ty {
            if arguments.is_empty() {
                if let Some(index) = function
                    .type_parameters
                    .iter()
                    .position(|parameter| parameter.name == *name)
                {
                    return Ok(ResolvedType::TypeParameter {
                        owner: DeclarationId::new(function.stable_id.clone()),
                        index: u32::try_from(index).map_err(|_| {
                            self.error(
                                "SPX-H006",
                                format!(
                                    "function `{}` type parameter index does not fit u32",
                                    function.name
                                ),
                                span,
                            )
                        })?,
                    });
                }
            }
        }
        self.resolve_type(ty, span)
    }
}

#[cfg(test)]
mod tests;
