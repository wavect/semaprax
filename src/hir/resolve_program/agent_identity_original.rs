//! Complete frozen 5ff resolver receiver and agent builder, compiled by parity controls.
use super::*;
use crate::hir::{record_invariants, DeclarationIndex, FunctionResolutionWork, FunctionReuse};
use crate::Program;

impl Resolver<'_, '_> {
    pub(in crate::hir) fn resolve_agent_identity_original(
        mut self,
    ) -> Result<(ResolvedProgram, crate::hir::FunctionResolutionWork), Diagnostic> {
        let _lowering_stage = reservation_stage_guard(ReservationStage::HirLowering);
        let entrypoint = self
            .program
            .functions()
            .iter()
            .find(|function| function.name == "main")
            .map(|function| self.declarations.held_identity(&function.stable_id))
            .ok_or_else(|| {
                self.error(
                    "SPX-H005",
                    "verified program has no resolved entry point",
                    Span::default(),
                )
            })?;
        self.validate_record_layouts()?;
        let types = self.program.types()
            .iter()
            .chain(crate::prelude::declarations_for_program(self.program))
            .map(|declaration| {
                let id = self.declarations.held_identity(&declaration.stable_id);
                let kind = match &declaration.kind {
                    TypeDeclarationKind::Resource { lifecycles } => {
                        let lifecycle = lifecycles.first().ok_or_else(|| {
                            self.error(
                                "SPX-H006",
                                format!("resource `{id}` has no resolved lifecycle"),
                                declaration.span,
                            )
                        })?;
                        let lifecycle_id = self.declarations.held_identity(
                            lifecycle.stable_id.as_deref().ok_or_else(|| {
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
                            .to_vec().into();
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
                            .to_vec().into();
                        let methods = methods
                            .iter()
                            .map(|method| self.declarations.held_identity(&method.stable_id))
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
                            .to_vec().into();
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
                        .to_vec().into(),
                    id,
                    name: declaration.name.clone(),
                    kind,
                    span: declaration.span,
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let interfaces = self
            .program
            .interfaces()
            .iter()
            .map(|interface| {
                let interface_id = self.declarations.held_identity(&interface.stable_id);
                let imports = interface
                    .imports
                    .iter()
                    .map(|import| {
                        if import.index_selected
                            && (import.selected_signature.is_none()
                                || import.selected_index_digest.is_none())
                        {
                            return Err(Diagnostic::error(
                                "SPX-B147",
                                "selected Rust import requires an authenticated API index",
                                import.span,
                            ));
                        }
                        let parameters = import
                            .params
                            .iter()
                            .map(|param| {
                                Ok(ResolvedImportParameter {
                                    name: param.name.clone(),
                                    ty: self.resolve_type(&param.ty, param.span)?,
                                    ownership: param.mode.into(),
                                    consumes_on_failure: param.name == import.consumes
                                        || (import.native_rust
                                            && param.mode == crate::ast::ParamMode::Own),
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
                            id: self.declarations.held_identity(&import.stable_id),
                            name: import.name.clone(),
                            interface: interface_id.clone(),
                            import_key: import.stable_id.clone(),
                            native_rust: import.native_rust,
                            index_selected: import.index_selected,
                            selected_index_digest: import.selected_index_digest.clone(),
                            selected_receiver: import.selected_receiver.clone(),
                            rust_path: import.rust_path.clone(),
                            parameters,
                            result: ResolvedImportResult {
                                kind: self
                                    .resolve_native_rust_result(&import.result, import.span)?
                                    .0,
                                ownership: if import.result.is_owned() {
                                    OwnershipMode::Own
                                } else if import.result.is_borrowed() {
                                    OwnershipMode::Borrow
                                } else {
                                    OwnershipMode::Value
                                },
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
        // Retention can move this exact carrier without creating a second Vec
        // when the synthetic module contains only authored local functions.
        let function_count =
            self.program
                .functions()
                .iter()
                .chain(self.program.types().iter().flat_map(
                    |declaration| match &declaration.kind {
                        TypeDeclarationKind::Class { methods, .. } => methods.as_slice(),
                        _ => &[],
                    },
                ))
                .filter(|function| function.type_parameters.is_empty())
                .count();
        let mut functions = Vec::with_capacity(function_count);
        for (_, function) in self.function_cells[..self.program.functions().len()]
            .iter()
            .filter(|(_, function)| function.payload().type_parameters.is_empty())
        {
            let (resolved, cost, reused) = self.resolve_or_reuse_function(function)?;
            if reused {
                self.function_work.reused += 1;
            }
            self.function_work
                .costs
                .insert(function.payload().stable_id.clone(), cost);
            functions.push(resolved);
        }
        for decl in self.program.types() {
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
        if let Some(error) = self.byte_helpers.failure() {
            return Err(error);
        }
        // The exact signature environment permits monomorphic body reuse in
        // generic modules, but grants no previous template/instance authority.
        // Every template is resolved from the current source before calls in
        // either fresh or reused functions seed the current reachable closure.
        let function_templates = self
            .program
            .functions()
            .iter()
            .filter(|function| !function.type_parameters.is_empty())
            .map(|function| self.resolve_function_template(function))
            .collect::<Result<Vec<_>, _>>()?;
        let function_instances =
            self.discover_function_instances(&functions, &function_templates)?;
        for agent in self.program.agents() {
            agent
                .validate_execution_metadata(self.program)
                .map_err(|message| Diagnostic::io("SPX-G559", message))?;
        }
        let agents = self
            .program
            .agents()
            .iter()
            .map(|declaration| resolve_agent_declaration(declaration, self.program.module()))
            .collect();
        let byte_slice_roots = derive_byte_slice_provenance(&functions, &self.declarations)?;
        let mut declarations = self.declarations;
        declarations.byte_slice_roots = byte_slice_roots;
        let mut resolved = ResolvedProgram {
            module: self.program.module().to_owned(),
            permits: self.program.permits().to_vec(),
            agents,
            entrypoint,
            declarations,
            types,
            interfaces,
            function_templates,
            functions,
            function_instances,
        };
        if let Some(error) = self.byte_helpers.failure() {
            return Err(error);
        }
        crate::hir::selected_fused_candidate::resolution::capacity(
            self.program,
            self.fused_producer,
            &resolved,
        )?;
        initial_loan_plans::attach(&mut resolved)?;
        let inventories = staged!(
            HirCleanupInventories,
            resolved
                .functions
                .iter()
                .map(|function| crate::cleanup::build_inventory(&resolved, function))
                .collect::<Result<Vec<_>, _>>()
        )?;
        for (function, inventory) in resolved.functions.iter_mut().zip(inventories) {
            function.cleanup = inventory;
        }
        let instance_inventories = staged!(
            HirInstanceCleanupInventories,
            resolved
                .function_instances
                .iter()
                .map(|instance| crate::cleanup::build_inventory(&resolved, &instance.function))
                .collect::<Result<Vec<_>, _>>()
        )?;
        for (instance, inventory) in resolved
            .function_instances
            .iter_mut()
            .zip(instance_inventories)
        {
            instance.function.cleanup = inventory;
        }
        staged!(
            HirCleanupPlans,
            crate::cleanup_plan::attach_program_plans(&mut resolved)
        )?;
        staged!(
            HirYieldProfiles,
            (|| {
                for function in &resolved.functions {
                    if let Some(yields) = &function.yields {
                        if crate::cleanup_plan::owned_frame_parameter(
                            &resolved.declarations,
                            &function.params,
                        ) {
                            crate::cleanup_plan::owned_frame_liveness(
                                &resolved.declarations,
                                function,
                            )?;
                        } else if crate::cleanup_plan::owned_frame_v2_parameter(
                            &resolved.declarations,
                            &function.params,
                        ) {
                            crate::cleanup_plan::owned_frame_v2_liveness(
                                &resolved.declarations,
                                function,
                            )?;
                        } else {
                            crate::cleanup_plan::admit_owned_bytes_profile(function)?;
                        }
                        if crate::hir::yield_aggregate::has_bytes_leaf(
                            &resolved.declarations,
                            &yields.request_type,
                        ) || crate::hir::yield_aggregate::has_bytes_leaf(
                            &resolved.declarations,
                            &yields.response_type,
                        ) {
                            crate::cleanup_plan::admit_sequential_aggregate_bytes_profile(
                                function,
                            )?;
                        }
                    }
                }
                Ok::<(), Diagnostic>(())
            })()
        )?;
        self.function_work.static_text = staged!(
            HirValidation,
            if self.partition_profile {
                crate::hir::byte_partition::validate_private(&resolved)
            } else {
                crate::hir::selected_fused_candidate::resolution::validation(
                    self.program,
                    self.fused_producer,
                    &resolved,
                )
            }
        )?;
        Ok((resolved, self.function_work))
    }
}

fn resolve_agent_declaration(
    declaration: &crate::ast::AgentDeclaration,
    module: &str,
) -> crate::hir::ResolvedAgentDeclaration {
    crate::hir::ResolvedAgentDeclaration {
        stable_id: DeclarationId::new(declaration.stable_id.clone()),
        name: declaration.name.clone(),
        types: declaration
            .types
            .iter()
            .map(|role| crate::hir::ResolvedAgentTypeRole {
                role: match role.role {
                    crate::ast::AgentTypeRole::Task => crate::hir::ResolvedAgentTypeRoleKind::Task,
                    crate::ast::AgentTypeRole::State => {
                        crate::hir::ResolvedAgentTypeRoleKind::State
                    }
                    crate::ast::AgentTypeRole::Observation => {
                        crate::hir::ResolvedAgentTypeRoleKind::Observation
                    }
                    crate::ast::AgentTypeRole::Proposal => {
                        crate::hir::ResolvedAgentTypeRoleKind::Proposal
                    }
                    crate::ast::AgentTypeRole::Outcome => {
                        crate::hir::ResolvedAgentTypeRoleKind::Outcome
                    }
                    crate::ast::AgentTypeRole::Result => {
                        crate::hir::ResolvedAgentTypeRoleKind::Result
                    }
                },
                stable_id: DeclarationId::new(role.stable_id.clone()),
            })
            .collect(),
        operations: declaration
            .operations
            .iter()
            .map(|operation| crate::hir::ResolvedAgentOperationRole {
                role: match operation.role {
                    crate::ast::AgentOperationRole::Initialize => {
                        crate::hir::ResolvedAgentOperationRoleKind::Initialize
                    }
                    crate::ast::AgentOperationRole::Observe => {
                        crate::hir::ResolvedAgentOperationRoleKind::Observe
                    }
                    crate::ast::AgentOperationRole::Propose => {
                        crate::hir::ResolvedAgentOperationRoleKind::Propose
                    }
                    crate::ast::AgentOperationRole::Authorize => {
                        crate::hir::ResolvedAgentOperationRoleKind::Authorize
                    }
                    crate::ast::AgentOperationRole::Execute => {
                        crate::hir::ResolvedAgentOperationRoleKind::Execute
                    }
                    crate::ast::AgentOperationRole::Reduce => {
                        crate::hir::ResolvedAgentOperationRoleKind::Reduce
                    }
                },
                kind: match operation.kind {
                    crate::ast::AgentOperationKind::Deterministic => {
                        crate::hir::ResolvedAgentOperationKind::Deterministic
                    }
                    crate::ast::AgentOperationKind::Model => {
                        crate::hir::ResolvedAgentOperationKind::Model
                    }
                    crate::ast::AgentOperationKind::Effect => {
                        crate::hir::ResolvedAgentOperationKind::Effect
                    }
                },
                stable_id: DeclarationId::new(operation.stable_id.clone()),
                embedded: operation.embedded_function_index.is_some(),
            })
            .collect(),
        source_association: None,
        runtime_v1_json: declaration.runtime_v1_json.clone(),
        model_wait: declaration.model_wait.as_ref().map(|binding| {
            Box::new(crate::hir::ResolvedAgentModelWaitBinding {
                helper_id: crate::hir::DeclarationId::new(binding.helper_id.clone()),
            })
        }),
    }
    .bind_source_association(module)
}

pub(in crate::hir) fn resolve_verified_original(
    program: &Program,
    reuse: Option<FunctionReuse<'_>>,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Diagnostic> {
    let clause_program = record_invariants::clause_program(program);
    let inventory_program = clause_program.as_ref().unwrap_or(program);
    let resolved = (Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(
            inventory_program,
        )),
        program: inventory_program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(inventory_program),
        declarations: DeclarationIndex::from_verified(inventory_program)?,
        reuse: if clause_program.is_some() {
            None
        } else {
            reuse
        },
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve_agent_identity_original()?;
    let Some(desugared) = record_invariants::desugar(program, &resolved.0) else {
        return Ok(resolved);
    };
    (Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(&desugared)),
        program: &desugared,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(&desugared),
        declarations: DeclarationIndex::from_verified(&desugared)?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve_agent_identity_original()
}
