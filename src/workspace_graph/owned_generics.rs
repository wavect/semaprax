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
use crate::hir;

use super::{graph_error, WorkspaceDeclarationFact, WorkspaceResolvedModule};

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
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();
        for linked in functions {
            visit_call_sites(&linked.function, &mut |_, instance, _| {
                if let Some(instance) = instance {
                    if seen.insert(instance.clone()) {
                        ordered.push(instance.clone());
                    }
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
