//! Program-level AST lowering.
//!
//! Entry point resolution, record layout validation, function and
//! function-template lowering, instance discovery, and type lowering.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

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
use super::monomorphize::materialize_function_template;
use super::nodes::{
    admitted_owned_byte_prelude_instance, OwnershipMode, ResolvedFunction,
    ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedImport, ResolvedImportFailure,
    ResolvedImportParameter, ResolvedImportResult, ResolvedImportResultKind, ResolvedInterface,
    ResolvedParam, ResolvedProgram, ResolvedResourceDrop, ResolvedResourceDropKind, ResolvedType,
    ResolvedTypeDeclaration, ResolvedTypeDeclarationKind, ResolvedTypeParameterDeclaration,
};
use super::{validate, Binding, Resolver};

impl Resolver<'_> {
    pub(super) fn resolve_call_type_argument(
        &self,
        caller: &FunctionExecutionId,
        argument: &Type,
        span: Span,
    ) -> Result<ResolvedType, Diagnostic> {
        if let FunctionExecutionId::Monomorphic(caller_id) = caller {
            if let Some(template) = self.program.functions.iter().find(|candidate| {
                candidate.stable_id == caller_id.as_str() && !candidate.type_parameters.is_empty()
            }) {
                return self.resolve_function_type(template, argument, span);
            }
        }
        self.resolve_type(argument, span)
    }

    pub(super) fn resolve_call_result(
        &self,
        caller: &FunctionExecutionId,
        result: &Type,
        span: Span,
    ) -> Result<(ResolvedType, OwnershipMode), Diagnostic> {
        let ty = self.resolve_call_type_argument(caller, result, span)?;
        let ownership =
            self.function_expression_ownership(caller, &ty, OwnershipMode::Own, span)?;
        Ok((ty, ownership))
    }

    pub(super) fn resolve_try_result_type(
        &self,
        function: &FunctionExecutionId,
        span: Span,
    ) -> Result<ResolvedType, Diagnostic> {
        let target = self.program.functions.iter().find(|candidate| matches!(function, FunctionExecutionId::Monomorphic(declaration) if candidate.stable_id == declaration.as_str()))
            .ok_or_else(|| self.error("SPX-H006", format!("resolved `?` has unknown enclosing function `{function}`"), span))?;
        self.resolve_function_type(target, &target.return_type, target.span)
    }

    pub(super) fn generic_function_arguments_are_admitted(
        &self,
        caller: &FunctionExecutionId,
        function: &crate::ast::Function,
        arguments: &[ResolvedType],
    ) -> Result<bool, Diagnostic> {
        if crate::vec_ops::source_wrapper(self.program, function).is_some()
            && matches!(arguments, [argument] if crate::vec_ops::resolved_element_is_admitted(argument))
        {
            return Ok(true);
        }
        if crate::box_ops::source_wrapper(self.program, function).is_some()
            && matches!(arguments, [argument] if crate::box_ops::resolved_element_is_admitted(argument))
        {
            return Ok(true);
        }
        if arguments.len() != function.type_parameters.len() {
            return Ok(false);
        }
        let caller_id = match caller {
            FunctionExecutionId::Monomorphic(id) => id,
            FunctionExecutionId::Generic(_) => return Ok(false),
        };
        let caller_parameter_count = self
            .program
            .functions
            .iter()
            .find(|candidate| candidate.stable_id == caller_id.as_str())
            .map_or(0, |candidate| candidate.type_parameters.len());
        let forwarded =
            super::generic_mapping::arguments(caller_id, caller_parameter_count, arguments);
        if forwarded {
            return Ok(true);
        }
        let result_type =
            self.resolve_function_type(function, &function.return_type, function.span)?;
        if super::generic_result::slot(
            &result_type,
            &DeclarationId::new(function.stable_id.clone()),
            function.type_parameters.len(),
        ) {
            return Ok(super::generic_result::arguments(&result_type, arguments));
        }
        if arguments.iter().any(|argument| {
            !super::type_reachability::nested_record_copy_scalar_is_admitted(argument)
        }) {
            return Ok(false);
        }
        let owner = DeclarationId::new(function.stable_id.clone());
        let return_type =
            self.resolve_function_type(function, &function.return_type, function.span)?;
        let owned_return = super::type_reachability::is_nested_owned_byte_record_template(
            &self.declarations,
            &return_type,
            &owner,
            function.type_parameters.len(),
        );
        let flat_owned_return = super::type_reachability::is_flat_owned_byte_record_template(
            &self.declarations,
            &return_type,
            &owner,
            function.type_parameters.len(),
        );
        let owned_parameters = function
            .params
            .iter()
            .filter(|parameter| parameter.mode == ParamMode::Own || parameter.ty == Type::String)
            .map(|parameter| self.resolve_function_type(function, &parameter.ty, parameter.span))
            .collect::<Result<Vec<_>, _>>()?;
        let nested = (owned_return && !flat_owned_return)
            || owned_parameters.iter().any(|ty| {
                super::type_reachability::is_nested_owned_byte_record_template(
                    &self.declarations,
                    ty,
                    &owner,
                    function.type_parameters.len(),
                ) && !super::type_reachability::is_flat_owned_byte_record_template(
                    &self.declarations,
                    ty,
                    &owner,
                    function.type_parameters.len(),
                )
            });
        let exact_owned_relay = matches!(owned_parameters.as_slice(), [parameter]
        if parameter == &return_type
            && super::type_reachability::is_nested_owned_byte_record_template(
                &self.declarations,
                parameter,
                &owner,
                function.type_parameters.len(),
            ));
        if arguments
            .iter()
            .all(|argument| matches!(argument, ResolvedType::I64 | ResolvedType::Bool))
            && !nested
        {
            return Ok(true);
        }
        if nested {
            Ok(exact_owned_relay)
        } else {
            Ok(flat_owned_return
                && owned_parameters.iter().any(|ty| {
                    super::type_reachability::is_flat_owned_byte_record_template(
                        &self.declarations,
                        ty,
                        &owner,
                        function.type_parameters.len(),
                    )
                }))
        }
    }

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
            .chain(crate::prelude::declarations_for_program(self.program))
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
            .collect::<Result<Vec<_>, _>>()?;
        let function_instances =
            self.discover_function_instances(&functions, &function_templates)?;
        let agents = self
            .program
            .agents
            .iter()
            .map(resolve_agent_declaration)
            .collect();
        let byte_slice_roots = derive_byte_slice_provenance(&functions, &self.declarations)?;
        let mut declarations = self.declarations;
        declarations.byte_slice_roots = byte_slice_roots;
        let mut resolved = ResolvedProgram {
            module: self.program.module.clone(),
            permits: self.program.permits.clone(),
            agents,
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
        let transparent_vec_wrapper = crate::vec_ops::source_wrapper(self.program, function);
        let transparent_box_wrapper = crate::box_ops::source_wrapper(self.program, function);
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
                let ownership = if let Some(op) = transparent_vec_wrapper {
                    op.param_ownership(index)
                } else if let Some(op) = transparent_box_wrapper {
                    op.param_ownership()
                } else if super::generic_result::slot(
                    &ty,
                    &function_id,
                    function.type_parameters.len(),
                ) || ty == ResolvedType::String
                    || super::type_reachability::is_nested_owned_byte_record_template(
                        &self.declarations,
                        &ty,
                        &function_id,
                        function.type_parameters.len(),
                    )
                {
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
                ownership: if (transparent_vec_wrapper.is_some() && return_type.is_uniquely_owned())
                    || (transparent_box_wrapper == Some(crate::box_ops::BoxOp::New))
                    || super::generic_result::slot(
                        &return_type,
                        &function_id,
                        function.type_parameters.len(),
                    )
                    || return_type == ResolvedType::String
                    || super::type_reachability::is_nested_owned_byte_record_template(
                        &self.declarations,
                        &return_type,
                        &function_id,
                        function.type_parameters.len(),
                    ) {
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
        functions: &[ResolvedFunction],
        templates: &[ResolvedFunctionTemplate],
    ) -> Result<Vec<ResolvedFunctionInstance>, Diagnostic> {
        const MAX_FUNCTION_INSTANCES: usize = 256;
        let mut calls = VecDeque::new();
        for function in functions {
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                super::visit_resolved_calls(expression, &mut |callee, instance, arguments| {
                    if let Some(instance) = instance {
                        calls.push_back((callee.clone(), arguments.to_vec(), instance.clone()));
                    }
                });
            }
        }

        let mut seen = BTreeSet::new();
        let mut instances = Vec::new();
        while let Some((template_id, type_arguments, id)) = calls.pop_front() {
            if FunctionInstanceId::derive(&template_id, &type_arguments) != id {
                return Err(super::hir_error(
                    "generic function call has an inconsistent instance identity",
                ));
            }
            let Some(template) = templates.iter().find(|template| template.id == template_id)
            else {
                return Err(super::hir_error(
                    "generic function call has no resolved template",
                ));
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            if instances.len() == MAX_FUNCTION_INSTANCES {
                return Err(super::hir_error(format!(
                    "generic function instance closure exceeds {MAX_FUNCTION_INSTANCES} entries"
                )));
            }
            let function = materialize_function_template(template, &type_arguments)?;
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                super::visit_resolved_calls(expression, &mut |callee, instance, arguments| {
                    if let Some(instance) = instance {
                        calls.push_back((callee.clone(), arguments.to_vec(), instance.clone()));
                    }
                });
            }
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
                        let instance = ResolvedType::Nominal {
                            declaration: declaration.clone(),
                            arguments: resolved.clone(),
                        };
                        let admitted_vec = declaration.as_str() == crate::prelude::VEC_ID
                            && matches!(resolved.as_slice(), [argument]
                                if crate::vec_ops::resolved_element_is_admitted(argument));
                        let admitted_box = declaration.as_str() == crate::prelude::BOX_ID
                            && matches!(resolved.as_slice(), [argument]
                                if crate::box_ops::resolved_element_is_admitted(argument));
                        if resolved.len() != parameters.len()
                            || (!admitted_vec
                                && !admitted_box

                && !admitted_owned_byte_prelude_instance(&declaration, &resolved)
                                && !crate::hir::type_reachability::is_flat_owned_byte_record(
                                    &self.declarations,
                                    &instance,
                                )
                                && !crate::hir::type_reachability::is_admitted_nested_owned_byte_record(
                                    &self.declarations,
                                    &instance,
                                )
                                && !crate::hir::type_reachability::is_admitted_concrete_owned_byte_variant(
                                    &self.declarations,
                                    &instance,
                                )
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

    pub(super) fn resolve_expression_type(
        &self,
        execution: &FunctionExecutionId,
        ty: &Type,
        span: Span,
    ) -> Result<ResolvedType, Diagnostic> {
        if let FunctionExecutionId::Monomorphic(owner) = execution {
            if let Some(function) = self
                .program
                .functions
                .iter()
                .find(|candidate| candidate.stable_id == owner.as_str())
            {
                return self.resolve_function_type(function, ty, span);
            }
        }
        self.resolve_type(ty, span)
    }

    pub(super) fn resolve_function_type(
        &self,
        function: &crate::ast::Function,
        ty: &Type,
        span: Span,
    ) -> Result<ResolvedType, Diagnostic> {
        let Type::Named { name, arguments } = ty else {
            return self.resolve_type(ty, span);
        };
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
        let declaration = self
            .declarations
            .type_id(name)
            .cloned()
            .ok_or_else(|| self.error("SPX-H001", format!("unresolved type `{name}`"), span))?;
        let resolved = arguments
            .iter()
            .map(|argument| self.resolve_function_type(function, argument, span))
            .collect::<Result<Vec<_>, _>>()?;
        let instance = ResolvedType::Nominal {
            declaration: declaration.clone(),
            arguments: resolved.clone(),
        };
        let owner = DeclarationId::new(function.stable_id.clone());
        let transparent_vec = crate::vec_ops::source_wrapper(self.program, function).is_some()
            && declaration.as_str() == crate::prelude::VEC_ID
            && matches!(resolved.as_slice(),
                [ResolvedType::TypeParameter { owner: parameter_owner, index: 0 }]
                    if parameter_owner == &owner);
        let specialized_vec_wrapper = crate::vec_ops::wrapper_by_id(function.stable_id.as_str())
            .is_some()
            && declaration.as_str() == crate::prelude::VEC_ID
            && matches!(resolved.as_slice(), [argument]
                if crate::vec_ops::resolved_element_is_admitted(argument));
        let transparent_box = crate::box_ops::source_wrapper(self.program, function).is_some()
            && declaration.as_str() == crate::prelude::BOX_ID
            && matches!(resolved.as_slice(),[ResolvedType::TypeParameter{owner:parameter_owner,index:0}] if parameter_owner==&owner);
        let specialized_box_wrapper = crate::box_ops::wrapper_by_id(function.stable_id.as_str())
            .is_some()
            && declaration.as_str() == crate::prelude::BOX_ID
            && matches!(resolved.as_slice(),[argument] if crate::box_ops::resolved_element_is_admitted(argument));
        if self
            .declarations
            .type_parameters(&declaration)
            .is_none_or(|parameters| parameters.len() != resolved.len())
            || (!transparent_vec
                && !specialized_vec_wrapper
                && !transparent_box
                && !specialized_box_wrapper
                && !super::generic_result::slot(&instance, &owner, function.type_parameters.len())
                && !admitted_owned_byte_prelude_instance(&declaration, &resolved)
                && !super::type_reachability::is_flat_owned_byte_record(
                    &self.declarations,
                    &instance,
                )
                && !super::type_reachability::is_admitted_nested_owned_byte_record(
                    &self.declarations,
                    &instance,
                )
                && !super::type_reachability::is_nested_owned_byte_record_template(
                    &self.declarations,
                    &instance,
                    &owner,
                    function.type_parameters.len(),
                )
                && !super::type_reachability::is_admitted_concrete_owned_byte_variant(
                    &self.declarations,
                    &instance,
                )
                && !resolved.is_empty()
                && resolved
                    .iter()
                    .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)))
        {
            return Err(self.error(
                "SPX-H006",
                format!("type `{declaration}` has invalid generic function arguments"),
                span,
            ));
        }
        Ok(instance)
    }
}

fn resolve_agent_declaration(
    declaration: &crate::ast::AgentDeclaration,
) -> super::ResolvedAgentDeclaration {
    super::ResolvedAgentDeclaration {
        stable_id: DeclarationId::new(declaration.stable_id.clone()),
        name: declaration.name.clone(),
        types: declaration
            .types
            .iter()
            .map(|role| super::ResolvedAgentTypeRole {
                role: match role.role {
                    crate::ast::AgentTypeRole::Task => super::ResolvedAgentTypeRoleKind::Task,
                    crate::ast::AgentTypeRole::State => super::ResolvedAgentTypeRoleKind::State,
                    crate::ast::AgentTypeRole::Observation => {
                        super::ResolvedAgentTypeRoleKind::Observation
                    }
                    crate::ast::AgentTypeRole::Proposal => {
                        super::ResolvedAgentTypeRoleKind::Proposal
                    }
                    crate::ast::AgentTypeRole::Outcome => super::ResolvedAgentTypeRoleKind::Outcome,
                    crate::ast::AgentTypeRole::Result => super::ResolvedAgentTypeRoleKind::Result,
                },
                stable_id: DeclarationId::new(role.stable_id.clone()),
            })
            .collect(),
        operations: declaration
            .operations
            .iter()
            .map(|operation| super::ResolvedAgentOperationRole {
                role: match operation.role {
                    crate::ast::AgentOperationRole::Initialize => {
                        super::ResolvedAgentOperationRoleKind::Initialize
                    }
                    crate::ast::AgentOperationRole::Observe => {
                        super::ResolvedAgentOperationRoleKind::Observe
                    }
                    crate::ast::AgentOperationRole::Propose => {
                        super::ResolvedAgentOperationRoleKind::Propose
                    }
                    crate::ast::AgentOperationRole::Authorize => {
                        super::ResolvedAgentOperationRoleKind::Authorize
                    }
                    crate::ast::AgentOperationRole::Execute => {
                        super::ResolvedAgentOperationRoleKind::Execute
                    }
                    crate::ast::AgentOperationRole::Reduce => {
                        super::ResolvedAgentOperationRoleKind::Reduce
                    }
                },
                kind: match operation.kind {
                    crate::ast::AgentOperationKind::Deterministic => {
                        super::ResolvedAgentOperationKind::Deterministic
                    }
                    crate::ast::AgentOperationKind::Model => {
                        super::ResolvedAgentOperationKind::Model
                    }
                    crate::ast::AgentOperationKind::Effect => {
                        super::ResolvedAgentOperationKind::Effect
                    }
                },
                stable_id: DeclarationId::new(operation.stable_id.clone()),
            })
            .collect(),
        runtime_v1_json: declaration.runtime_v1_json.clone(),
    }
}

#[cfg(test)]
mod tests;
