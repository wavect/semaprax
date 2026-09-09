//! Exact generic linking for the Project owned-data API profiles.
//!
//! The owned-data linker selects the exact union of the entry closure and the
//! selected public roots. A generic call reaches its meaning through two
//! separate authenticated declarations: the authored template that owns the
//! `@id` identity, and the checked monomorphic instance the resolver already
//! materialized for one exact type-argument vector. This module keeps both
//! inventories, resolves each call site to exactly one instance, and refuses
//! to link a template whose call site has no authenticated instance.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::{ast::Program, hir};

use super::{
    graph_error, retained_loan_plan_bytes, AuthoredDeclaration, WorkspaceDeclarationFact,
    WorkspaceResolvedModule, GRAPH_ACCOUNTED_RESOLVED_FUNCTION_INSTANCE_BYTES,
};

/// Retain only authored nominal declarations reached by an already selected
/// scalar function closure. This is needed when an otherwise scalar public
/// root constructs and consumes an owned aggregate entirely inside its body;
/// function-declaration ownership is not evidence of that expression type.
pub(super) fn reachable_scalar_types(
    modules: &[WorkspaceResolvedModule],
    functions: &[hir::LinkedScalarFunction],
    function_instances: &[hir::ResolvedFunctionInstance],
) -> Result<Vec<hir::ResolvedTypeDeclaration>, Vec<Diagnostic>> {
    let mut available = BTreeMap::new();
    for declaration in modules.iter().flat_map(|module| &module.types) {
        if available
            .insert(declaration.id.clone(), declaration.clone())
            .is_some()
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "workspace scalar type identity is duplicated",
            )]);
        }
    }
    hir::reachable_authored_types(functions, function_instances, &[], &available)
        .map_err(|error| vec![error])
}

pub(super) fn program_imports_vec_wrapper(program: &Program, programs: &[Program]) -> bool {
    program
        .module_uses
        .iter()
        .any(|module_use| imported_vec_wrapper(programs, module_use).is_some())
}
pub(super) fn program_imports_box_wrapper(program: &Program, programs: &[Program]) -> bool {
    program
        .module_uses
        .iter()
        .any(|module_use| imported_box_wrapper(programs, module_use).is_some())
}
fn imported_box_wrapper(
    programs: &[Program],
    module_use: &crate::ast::ModuleUse,
) -> Option<crate::box_ops::BoxOp> {
    if module_use.kind != crate::ast::ModuleUseKind::Function {
        return None;
    }
    let provider = programs
        .iter()
        .find(|provider| provider.module == module_use.target_module)?;
    let function = provider
        .functions
        .iter()
        .find(|function| function.stable_id == module_use.persistent_id)?;
    crate::box_ops::source_wrapper(provider, function)
}

fn imported_vec_wrapper(
    programs: &[Program],
    module_use: &crate::ast::ModuleUse,
) -> Option<crate::vec_ops::VecOp> {
    if module_use.kind != crate::ast::ModuleUseKind::Function {
        return None;
    }
    let provider = programs
        .iter()
        .find(|provider| provider.module == module_use.target_module)?;
    let function = provider
        .functions
        .iter()
        .find(|function| function.stable_id == module_use.persistent_id)?;
    crate::vec_ops::source_wrapper(provider, function)
}

pub(super) fn retain_module_instances(
    program: &Program,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    instances: Vec<hir::ResolvedFunctionInstance>,
) -> Result<
    (
        Vec<hir::ResolvedFunctionInstance>,
        Vec<hir::ResolvedFunctionInstance>,
    ),
    Vec<Diagnostic>,
> {
    let retained = super::filter_owned_vec_accounted(
        instances,
        GRAPH_ACCOUNTED_RESOLVED_FUNCTION_INSTANCE_BYTES,
        |item| retained_loan_plan_bytes(&item.function.loan_plan),
        |item| {
            authored
                .get(item.template.as_str())
                .is_some_and(|owner| owner.module == program.module)
                || program.module_uses.iter().any(|module_use| {
                    module_use.persistent_id == item.template.as_str()
                        && (imported_vec_wrapper(programs, module_use).is_some()
                            || imported_box_wrapper(programs, module_use).is_some())
                })
        },
    )?;
    Ok(retained.into_iter().partition(|item| {
        authored
            .get(item.template.as_str())
            .is_some_and(|owner| owner.module == program.module)
    }))
}

pub(super) fn merge_imported_vec_instances(
    retained: &mut BTreeMap<hir::FunctionInstanceId, hir::ResolvedFunctionInstance>,
    imported: Vec<hir::ResolvedFunctionInstance>,
) -> Result<(), Vec<Diagnostic>> {
    for instance in imported {
        match retained.entry(instance.id.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(instance);
            }
            std::collections::btree_map::Entry::Occupied(entry) if entry.get() == &instance => {}
            std::collections::btree_map::Entry::Occupied(_) => {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "imported vector wrapper instance meaning is not canonical",
                )]);
            }
        }
    }
    Ok(())
}

pub(super) fn attach_imported_vec_instances(
    modules: &mut [WorkspaceResolvedModule],
    retained: BTreeMap<hir::FunctionInstanceId, hir::ResolvedFunctionInstance>,
) -> Result<(), Vec<Diagnostic>> {
    if retained.is_empty() {
        return Ok(());
    }
    for instance in retained.into_values() {
        let module = if crate::box_ops::wrapper_by_id(instance.template.as_str()).is_some() {
            crate::box_ops::MODULE
        } else {
            crate::vec_ops::MODULE
        };
        let provider = modules
            .iter_mut()
            .find(|candidate| candidate.module == module)
            .ok_or_else(|| {
                vec![graph_error(
                    "SPX-G173",
                    "imported wrapper instances have no authenticated provider module",
                )]
            })?;
        if !provider
            .function_templates
            .iter()
            .any(|template| template.id == instance.template)
        {
            return Err(vec![graph_error(
                "SPX-G173",
                "imported wrapper instance has no authenticated provider template",
            )]);
        }
        provider.function_instances.push(instance);
    }
    Ok(())
}

/// The authenticated generic inventory of one Phase-A workspace build.
pub(super) struct OwnedGenericInventory {
    templates: BTreeMap<hir::DeclarationId, hir::ResolvedFunctionTemplate>,
    instances: BTreeMap<hir::FunctionInstanceId, hir::ResolvedFunctionInstance>,
}

/// The exact retained closure over ordinary functions, generic templates, and
/// materialized generic instances.
pub(super) struct OwnedGenericClosure {
    pub(super) functions: BTreeSet<hir::DeclarationId>,
    pub(super) templates: BTreeSet<hir::DeclarationId>,
    instances: BTreeSet<hir::FunctionInstanceId>,
}

impl OwnedGenericInventory {
    /// Collect every authenticated template and instance. Identities are
    /// rejected for duplication exactly like the ordinary function inventory,
    /// and each template must agree with its Phase-A declaration fact.
    pub(super) fn collect(
        modules: &[WorkspaceResolvedModule],
        declarations: &BTreeMap<String, WorkspaceDeclarationFact>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut templates = BTreeMap::new();
        let mut instances = BTreeMap::new();
        for module in modules {
            for template in &module.function_templates {
                let Some(fact) = declarations.get(template.id.as_str()) else {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data generic template is absent from declaration facts",
                    )]);
                };
                if fact.kind != hir::DeclarationKind::Function
                    || fact.path.as_deref() != Some(module.path.as_str())
                    || fact.module.as_deref() != Some(module.module.as_str())
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data generic template facts disagree with its retained body",
                    )]);
                }
                if templates
                    .insert(template.id.clone(), template.clone())
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data generic template identity is duplicated",
                    )]);
                }
            }
            for instance in &module.function_instances {
                if instance.id
                    != hir::FunctionInstanceId::derive(&instance.template, &instance.type_arguments)
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data generic instance identity is not canonical",
                    )]);
                }
                if instances
                    .insert(instance.id.clone(), instance.clone())
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data generic instance identity is duplicated",
                    )]);
                }
            }
        }
        for instance in instances.values() {
            if !templates.contains_key(&instance.template) {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!(
                        "workspace owned-data generic instance `{}` has no authenticated template",
                        instance.id.as_str()
                    ),
                )]);
            }
        }
        Ok(Self {
            templates,
            instances,
        })
    }

    fn template(&self, id: &hir::DeclarationId) -> Option<&hir::ResolvedFunctionTemplate> {
        self.templates.get(id)
    }

    /// Retain the exact templates the closure selected, in canonical identity
    /// order.
    pub(super) fn retained_templates(
        &self,
        selected: &BTreeSet<hir::DeclarationId>,
    ) -> Result<Vec<hir::ResolvedFunctionTemplate>, Vec<Diagnostic>> {
        selected
            .iter()
            .map(|id| {
                self.templates.get(id).cloned().ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        format!(
                            "owned-data closure names an unauthenticated generic template `{id}`"
                        ),
                    )]
                })
            })
            .collect()
    }

    /// Materialize the retained instances in exactly the order an independent
    /// replay of the retained monomorphic bodies discovers them. Canonical HIR
    /// validation reconstructs that sequence from the linked functions alone,
    /// so a retained instance reachable only from another instance body is not
    /// representable and fails closed here rather than downstream.
    pub(super) fn retained_instances(
        &self,
        functions: &[hir::LinkedScalarFunction],
        closure: &OwnedGenericClosure,
    ) -> Result<Vec<hir::ResolvedFunctionInstance>, Vec<Diagnostic>> {
        const MAX_FUNCTION_INSTANCES: usize = 256;
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();
        let mut pending = std::collections::VecDeque::new();
        for linked in functions {
            visit_call_sites(&linked.function, &mut |_, instance, _| {
                if let Some(instance) = instance {
                    pending.push_back(instance.clone());
                }
            });
        }
        while let Some(id) = pending.pop_front() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if ordered.len() == MAX_FUNCTION_INSTANCES {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!(
                        "owned-data generic instance closure exceeds {MAX_FUNCTION_INSTANCES} entries"
                    ),
                )]);
            }
            ordered.push(id.clone());
            let Some(instance) = self.instances.get(&id) else {
                continue;
            };
            visit_call_sites(&instance.function, &mut |_, instance, _| {
                if let Some(instance) = instance {
                    pending.push_back(instance.clone());
                }
            });
        }
        if seen != closure.instances {
            return Err(vec![graph_error(
                "SPX-G173",
                "owned-data closure retains a generic instance that is not reachable from an authored function body",
            )]);
        }
        ordered
            .into_iter()
            .map(|id| {
                self.instances.get(&id).cloned().ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        format!(
                            "owned-data closure names an unauthenticated generic instance `{}`",
                            id.as_str()
                        ),
                    )]
                })
            })
            .collect()
    }
}

pub(super) struct RetainedScalarParts {
    pub(super) types: Vec<hir::ResolvedTypeDeclaration>,
    pub(super) function_templates: Vec<hir::ResolvedFunctionTemplate>,
    pub(super) function_instances: Vec<hir::ResolvedFunctionInstance>,
}

pub(super) fn retained_scalar_generics(
    modules: &[WorkspaceResolvedModule],
    declarations: &BTreeMap<String, WorkspaceDeclarationFact>,
    functions: &[hir::LinkedScalarFunction],
    types: Vec<hir::ResolvedTypeDeclaration>,
) -> Result<RetainedScalarParts, Vec<Diagnostic>> {
    let available = functions
        .iter()
        .map(|linked| {
            (
                linked.function.id.clone(),
                hir::LinkedScalarFunction {
                    function: linked.function.clone(),
                    origin: linked.origin,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let roots = available.keys().cloned().collect();
    let generics = OwnedGenericInventory::collect(modules, declarations)?;
    let closure = close_owned_data_closure(&available, &generics, roots)?;
    Ok(RetainedScalarParts {
        types,
        function_templates: generics.retained_templates(&closure.templates)?,
        function_instances: generics.retained_instances(functions, &closure)?,
    })
}

pub(super) fn select_scalar_generic_closure(
    modules: &[WorkspaceResolvedModule],
    declarations: &BTreeMap<String, WorkspaceDeclarationFact>,
    available: &BTreeMap<hir::DeclarationId, hir::LinkedScalarFunction>,
    roots: BTreeSet<hir::DeclarationId>,
) -> Result<OwnedGenericClosure, Vec<Diagnostic>> {
    let generics = OwnedGenericInventory::collect(modules, declarations)?;
    close_owned_data_closure(available, &generics, roots)
}

pub(super) fn select_scalar_generic_roots(
    modules: &[WorkspaceResolvedModule],
    declarations: &BTreeMap<String, WorkspaceDeclarationFact>,
    available: &BTreeMap<hir::DeclarationId, hir::LinkedScalarFunction>,
    additional_roots: &[String],
    entrypoint: hir::DeclarationId,
) -> Result<OwnedGenericClosure, Vec<Diagnostic>> {
    let mut roots = additional_roots
        .iter()
        .map(|root| hir::DeclarationId::new(root.clone()))
        .collect::<BTreeSet<_>>();
    roots.insert(entrypoint);
    select_scalar_generic_closure(modules, declarations, available, roots)
}

pub(super) fn retained_scalar_parts(
    modules: &[WorkspaceResolvedModule],
    declarations: &BTreeMap<String, WorkspaceDeclarationFact>,
    functions: &[hir::LinkedScalarFunction],
    closure: &OwnedGenericClosure,
    base_types: Vec<hir::ResolvedTypeDeclaration>,
) -> Result<RetainedScalarParts, Vec<Diagnostic>> {
    let generics = OwnedGenericInventory::collect(modules, declarations)?;
    let templates = generics.retained_templates(&closure.templates)?;
    let instances = generics.retained_instances(functions, closure)?;
    let types = if functions.iter().any(|linked| {
        declarations
            .get(linked.function.id.as_str())
            .is_some_and(|fact| fact.owner.is_some())
    }) {
        base_types
    } else {
        reachable_scalar_types(modules, functions, &instances)?
    };
    Ok(RetainedScalarParts {
        types,
        function_templates: templates,
        function_instances: instances,
    })
}

/// Walk the entry-plus-roots closure over `(callee, type arguments)` call
/// sites. A call that names a generic template selects exactly the instance
/// its authenticated type-argument vector derives; the walk then continues
/// through that instance body's own callees.
pub(super) fn close_owned_data_closure(
    available: &BTreeMap<hir::DeclarationId, hir::LinkedScalarFunction>,
    generics: &OwnedGenericInventory,
    roots: BTreeSet<hir::DeclarationId>,
) -> Result<OwnedGenericClosure, Vec<Diagnostic>> {
    let mut closure = OwnedGenericClosure {
        functions: BTreeSet::new(),
        templates: BTreeSet::new(),
        instances: BTreeSet::new(),
    };
    let mut pending_functions = roots;
    let mut pending_instances = BTreeSet::<hir::FunctionInstanceId>::new();
    loop {
        let body = if let Some(function_id) = pending_functions.pop_first() {
            let Some(linked) = available.get(&function_id) else {
                return Err(vec![Diagnostic::io(
                    "SPX-W115",
                    format!(
                        "selected Project Web export identity `{function_id}` does not name an authenticated function"
                    ),
                )]);
            };
            if !closure.functions.insert(function_id) {
                continue;
            }
            if closure.functions.len() > crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
                return Err(vec![graph_error(
                    "SPX-G172",
                    format!(
                        "workspace owned-data linked inventory exceeds {} functions",
                        crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS
                    ),
                )]);
            }
            &linked.function
        } else if let Some(instance_id) = pending_instances.pop_first() {
            let Some(instance) = generics.instances.get(&instance_id) else {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!(
                        "owned-data closure names an unauthenticated generic instance `{}`",
                        instance_id.as_str()
                    ),
                )]);
            };
            if !closure.instances.insert(instance_id) {
                continue;
            }
            if closure.instances.len() > crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!(
                        "workspace owned-data linked inventory exceeds {} generic instances",
                        crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS
                    ),
                )]);
            }
            &instance.function
        } else {
            break;
        };
        let mut sites = Vec::new();
        visit_call_sites(body, &mut |callee, instance, type_arguments| {
            sites.push((callee.clone(), instance.cloned(), type_arguments.to_vec()));
        });
        for (callee, instance, type_arguments) in sites {
            if available.contains_key(&callee) {
                if instance.is_some() || !type_arguments.is_empty() {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        format!(
                            "owned-data closure calls monomorphic function `{callee}` with generic arguments"
                        ),
                    )]);
                }
                if !closure.functions.contains(&callee) {
                    pending_functions.insert(callee);
                }
                continue;
            }
            if let Some(template) = generics.template(&callee) {
                let derived = hir::FunctionInstanceId::derive(&callee, &type_arguments);
                if template.type_parameters.len() != type_arguments.len()
                    || instance.is_some_and(|attached| attached != derived)
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        format!(
                            "owned-data closure calls generic function `{callee}` with a non-canonical instance selection"
                        ),
                    )]);
                }
                if !generics.instances.contains_key(&derived) {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        format!(
                            "owned-data closure calls generic function `{callee}` with no authenticated instance"
                        ),
                    )]);
                }
                closure.templates.insert(callee);
                if !closure.instances.contains(&derived) {
                    pending_instances.insert(derived);
                }
                continue;
            }
            if crate::string_ops::by_id(callee.as_str()).is_none()
                && crate::str_ops::by_id(callee.as_str()).is_none()
                && crate::byte_ops::by_id(callee.as_str()).is_none()
                && crate::vec_ops::by_id(callee.as_str()).is_none()
                && crate::box_ops::by_id(callee.as_str()).is_none()
                && crate::iterator_ops::by_id(callee.as_str()).is_none()
                && crate::host_io_ops::by_id(callee.as_str()).is_none()
                && crate::command_io_ops::by_id(callee.as_str()).is_none()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data closure calls unauthenticated function `{callee}`"),
                )]);
            }
        }
    }
    Ok(closure)
}

fn visit_call_sites(
    function: &hir::ResolvedFunction,
    visit: &mut impl FnMut(&hir::DeclarationId, Option<&hir::FunctionInstanceId>, &[hir::ResolvedType]),
) {
    for requirement in &function.requires {
        hir::visit_resolved_calls(requirement, visit);
    }
    hir::visit_resolved_calls(&function.body, visit);
    for postcondition in &function.ensures {
        hir::visit_resolved_calls(postcondition, visit);
    }
}

/// Internal scalar-root closures may retain exact owned generic carriers.
/// This predicate is never used to admit selected public ABI declarations.
pub(super) fn private_signature(
    workspace: &super::ValidatedWorkspaceHir,
    module: &WorkspaceResolvedModule,
    function: &hir::ResolvedFunction,
) -> bool {
    private_callable_signature(workspace, module, function)
        || hir::generic_result::concrete_signature(function)
        || hir::generic_collection::concrete_signature(function)
        || (hir::generic_variant::concrete_signature(&module.types, function)
            && function
                .params
                .iter()
                .map(|p| &p.ty)
                .chain(std::iter::once(&function.return_type))
                .all(|ty| {
                    let hir::ResolvedType::Nominal { declaration, .. } = ty else {
                        return true;
                    };
                    workspace
                        .declarations
                        .get(declaration.as_str())
                        .is_some_and(|fact| {
                            fact.kind == hir::DeclarationKind::Variant
                                && fact.origin == hir::IdentityOrigin::Explicit
                        })
                }))
}

fn private_callable_signature(
    workspace: &super::ValidatedWorkspaceHir,
    module: &WorkspaceResolvedModule,
    function: &hir::ResolvedFunction,
) -> bool {
    if !hir::function_value::private_helper_signature(function) {
        return false;
    }
    let mut foreign = false;
    for caller_module in &workspace.modules {
        if caller_module.module == module.module {
            continue;
        }
        for caller in caller_module.functions.iter().chain(
            caller_module
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ) {
            visit_call_sites(caller, &mut |callee, _, _| {
                foreign |= *callee == function.id;
            });
        }
    }
    !foreign
}

/// Ephemeral linker admission derived from checked module ownership and calls.
pub(super) fn private_callable_link_ids(
    functions: &[hir::LinkedScalarFunction],
    declarations: &BTreeMap<String, super::WorkspaceDeclarationFact>,
) -> BTreeSet<hir::DeclarationId> {
    functions
        .iter()
        .filter_map(|linked| {
            let function = &linked.function;
            let fact = declarations.get(function.id.as_str())?;
            if fact.owner.is_some()
                || fact.module.is_none()
                || !hir::function_value::private_helper_signature(function)
            {
                return None;
            }
            let mut foreign = false;
            for caller in functions {
                if declarations
                    .get(caller.function.id.as_str())
                    .and_then(|fact| fact.module.as_ref())
                    == fact.module.as_ref()
                {
                    continue;
                }
                visit_call_sites(&caller.function, &mut |callee, _, _| {
                    foreign |= *callee == function.id;
                });
            }
            (!foreign).then(|| function.id.clone())
        })
        .collect()
}

// The same authenticated owned-data linker serves ordinary roots and the
// explicit Agent role/type roots. Existing callers retain their empty type set.
impl super::WorkspaceGraphBuild {
    pub(super) fn linked_owned_data_api_program_with_roots(
        &self,
        entry_module: &str,
        additional_roots: &[String],
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        self.linked_owned_data_with_type_roots(entry_module, additional_roots, &[])
    }

    pub(crate) fn linked_agent_role_program(
        &self,
        entry_module: &str,
        functions: &[String],
        types: &[hir::DeclarationId],
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        let mut program = self.linked_owned_data_with_type_roots(entry_module, functions, types)?;
        self.attach_project_agents(&mut program)?;
        hir::validate(&program).map_err(|error| vec![error])?;
        Ok(program)
    }

    fn linked_owned_data_with_type_roots(
        &self,
        entry_module: &str,
        additional_roots: &[String],
        type_roots: &[hir::DeclarationId],
    ) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        use super::*;
        validate_entry_module(entry_module)?;
        let mut available = BTreeMap::<hir::DeclarationId, hir::LinkedScalarFunction>::new();
        let mut entrypoints = Vec::new();
        for module in &self.hir.modules {
            for function in &module.functions {
                let Some(fact) = self.hir.declarations.get(function.id.as_str()) else {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data function is absent from declaration facts",
                    )]);
                };
                if fact.kind != hir::DeclarationKind::Function
                    || fact.path.as_deref() != Some(module.path.as_str())
                    || fact.module.as_deref() != Some(module.module.as_str())
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data function facts disagree with its retained body",
                    )]);
                }
                if module.module == entry_module && function.name == "main" {
                    if !function.params.is_empty() || function.return_type != hir::ResolvedType::I64
                    {
                        return Err(vec![graph_error(
                            "SPX-G172",
                            "workspace owned-data entry must have the exact signature fn main() -> i64",
                        )]);
                    }
                    entrypoints.push((function.id.clone(), fact.origin));
                }
                if available
                    .insert(
                        function.id.clone(),
                        hir::LinkedScalarFunction {
                            function: function.clone(),
                            origin: fact.origin,
                        },
                    )
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data function identity is duplicated",
                    )]);
                }
            }
        }
        let [(entrypoint, hir::IdentityOrigin::Explicit)] = entrypoints.as_slice() else {
            return Err(vec![graph_error(
                "SPX-G172",
                "workspace owned-data entry module must declare exactly one explicit authored `main` function",
            )]);
        };
        let entrypoint = entrypoint.clone();
        let mut roots = BTreeSet::from([entrypoint.clone()]);
        roots.extend(
            additional_roots
                .iter()
                .map(|root| hir::DeclarationId::new(root.clone())),
        );
        let generics = owned_generics::OwnedGenericInventory::collect(
            &self.hir.modules,
            &self.hir.declarations,
        )?;
        let closure = owned_generics::close_owned_data_closure(&available, &generics, roots)?;
        let functions = closure
            .functions
            .iter()
            .map(|id| {
                available
                    .get(id)
                    .map(|linked| hir::LinkedScalarFunction {
                        function: linked.function.clone(),
                        origin: linked.origin,
                    })
                    .ok_or_else(|| {
                        vec![graph_error(
                            "SPX-G173",
                            "owned-data closure names an unauthenticated function",
                        )]
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let function_templates = generics.retained_templates(&closure.templates)?;
        let function_instances = generics.retained_instances(&functions, &closure)?;

        let referenced_imports = functions
            .iter()
            .flat_map(|linked| resolved_function_imports(&linked.function))
            .collect::<BTreeSet<_>>();
        let mut imports =
            BTreeMap::<hir::DeclarationId, (&hir::ResolvedInterface, &hir::ResolvedImport)>::new();
        for module in &self.hir.modules {
            for interface in &module.interfaces {
                for import in &interface.imports {
                    if import.interface != interface.id
                        || imports
                            .insert(import.id.clone(), (interface, import))
                            .is_some()
                    {
                        return Err(vec![graph_error(
                            "SPX-G173",
                            "workspace owned-data import inventory is ambiguous",
                        )]);
                    }
                }
            }
        }
        let mut selected_imports =
            BTreeMap::<hir::DeclarationId, BTreeSet<hir::DeclarationId>>::new();
        for import_id in referenced_imports {
            let Some((interface, _)) = imports.get(&import_id) else {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data closure references unknown import `{import_id}`"),
                )]);
            };
            selected_imports
                .entry(interface.id.clone())
                .or_default()
                .insert(import_id);
        }
        let interfaces = selected_imports
            .into_iter()
            .map(|(interface_id, selected)| {
                let interface = imports
                    .values()
                    .find_map(|(interface, _)| (interface.id == interface_id).then_some(*interface))
                    .ok_or_else(|| {
                        vec![graph_error(
                            "SPX-G173",
                            "owned-data interface selection lost its authenticated owner",
                        )]
                    })?;
                let mut interface = interface.clone();
                interface
                    .imports
                    .retain(|import| selected.contains(&import.id));
                Ok(interface)
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;

        let mut available_types = BTreeMap::new();
        for module in &self.hir.modules {
            for declaration in &module.types {
                if available_types
                    .insert(declaration.id.clone(), declaration.clone())
                    .is_some()
                {
                    return Err(vec![graph_error(
                        "SPX-G173",
                        "workspace owned-data type identity is duplicated",
                    )]);
                }
            }
        }
        let types = hir::reachable_authored_types_with_roots(
            &functions,
            &function_instances,
            &interfaces,
            &available_types,
            type_roots,
        )
        .map_err(|error| vec![error])?;

        fn retain_fact(
            authenticated: &BTreeMap<String, WorkspaceDeclarationFact>,
            selected: &mut BTreeMap<hir::DeclarationId, hir::LinkedDeclarationFact>,
            id: &hir::DeclarationId,
            kind: hir::DeclarationKind,
            owner: Option<&hir::DeclarationId>,
        ) -> Result<(), Vec<Diagnostic>> {
            let Some(fact) = authenticated.get(id.as_str()) else {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data declaration `{id}` has no Phase-A fact"),
                )]);
            };
            if fact.kind != kind || fact.owner.as_deref() != owner.map(hir::DeclarationId::as_str) {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data declaration `{id}` disagrees with its Phase-A fact"),
                )]);
            }
            if selected
                .insert(
                    id.clone(),
                    hir::LinkedDeclarationFact {
                        kind: fact.kind,
                        origin: fact.origin,
                        owner: owner.cloned(),
                    },
                )
                .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    format!("owned-data declaration `{id}` is selected more than once"),
                )]);
            }
            Ok(())
        }

        let mut declaration_facts = BTreeMap::new();
        for linked in &functions {
            retain_fact(
                &self.hir.declarations,
                &mut declaration_facts,
                &linked.function.id,
                hir::DeclarationKind::Function,
                None,
            )?;
        }
        for template in &function_templates {
            retain_fact(
                &self.hir.declarations,
                &mut declaration_facts,
                &template.id,
                hir::DeclarationKind::Function,
                None,
            )?;
        }
        for declaration in &types {
            let kind = match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
                hir::ResolvedTypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
                hir::ResolvedTypeDeclarationKind::Class { .. }
                | hir::ResolvedTypeDeclarationKind::Resource { .. } => {
                    return Err(vec![graph_error(
                        "SPX-G172",
                        "owned-data type projection escaped the record/variant profile",
                    )]);
                }
            };
            retain_fact(
                &self.hir.declarations,
                &mut declaration_facts,
                &declaration.id,
                kind,
                None,
            )?;
            match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Record { fields } => {
                    for field in fields {
                        retain_fact(
                            &self.hir.declarations,
                            &mut declaration_facts,
                            &field.id,
                            hir::DeclarationKind::Field,
                            Some(&declaration.id),
                        )?;
                    }
                }
                hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        retain_fact(
                            &self.hir.declarations,
                            &mut declaration_facts,
                            &case.id,
                            hir::DeclarationKind::VariantCase,
                            Some(&declaration.id),
                        )?;
                        for field in &case.fields {
                            retain_fact(
                                &self.hir.declarations,
                                &mut declaration_facts,
                                &field.id,
                                hir::DeclarationKind::CaseField,
                                Some(&case.id),
                            )?;
                        }
                    }
                }
                hir::ResolvedTypeDeclarationKind::Class { .. }
                | hir::ResolvedTypeDeclarationKind::Resource { .. } => {
                    unreachable!("rejected above")
                }
            }
        }
        for interface in &interfaces {
            retain_fact(
                &self.hir.declarations,
                &mut declaration_facts,
                &interface.id,
                hir::DeclarationKind::Interface,
                None,
            )?;
            for import in &interface.imports {
                retain_fact(
                    &self.hir.declarations,
                    &mut declaration_facts,
                    &import.id,
                    hir::DeclarationKind::Import,
                    Some(&interface.id),
                )?;
            }
        }
        let permits = functions
            .iter()
            .flat_map(|linked| linked.function.effects.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        hir::link_owned_data_api_workspace(
            entry_module.to_owned(),
            entrypoint,
            functions,
            hir::LinkedOwnedDataParts {
                permits,
                types,
                interfaces,
                declaration_facts,
                function_templates,
                function_instances,
            },
        )
        .map_err(|error| vec![error])
    }

    /// Consume one Phase-A graph after deriving both requested closures.
    pub(crate) fn into_linked_scalar_programs(
        self,
        entry_module: &str,
        test_module: &str,
    ) -> Result<(hir::ResolvedProgram, hir::ResolvedProgram), Vec<Diagnostic>> {
        self.validate_entire_scalar_workspace(entry_module, test_module)?;
        let entry = self.linked_scalar_program(entry_module)?;
        let test = self.linked_scalar_program(test_module)?;
        Ok((entry, test))
    }
}
