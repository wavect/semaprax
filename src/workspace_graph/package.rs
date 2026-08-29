use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;
use crate::package_report_v2::ScalarPackageInterface;

use super::{build_owned, hir, WorkspaceSource};

pub(crate) struct PackageWorkspaceLink {
    pub(crate) program: hir::ResolvedProgram,
    pub(crate) modules: Vec<PackageWorkspaceModule>,
    pub(crate) imports: Vec<PackageWorkspaceImport>,
    pub(crate) linked_function_ids: Vec<String>,
    pub(crate) root_exports: Vec<String>,
}

pub(crate) struct PackageWorkspaceModule {
    pub(crate) package: String,
    pub(crate) interface: ScalarPackageInterface,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct PackageWorkspaceImport {
    pub(crate) dependent: String,
    pub(crate) dependency: String,
    pub(crate) target: String,
    pub(crate) alias: String,
    pub(crate) ordinal: usize,
}

pub(crate) fn build_package_scalar_sources(
    sources: Vec<WorkspaceSource>,
    root_package: &str,
) -> Result<PackageWorkspaceLink, Vec<Diagnostic>> {
    let build = build_owned(sources)?;
    if !build.contains_module(root_package) {
        return Err(vec![package_error(
            "package-source root module is absent from authenticated sources",
        )]);
    }
    let paths_to_modules = build
        .hir
        .module_paths
        .iter()
        .map(|(module, path)| (path.clone(), module.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut imports = Vec::new();
    for edge in &build.edges {
        if edge.kind == "type_import" {
            return Err(vec![package_profile_error(
                "package-source scalar profile does not admit type imports",
            )]);
        }
        if edge.kind != "function_import" {
            continue;
        }
        let dependent = paths_to_modules.get(&edge.caller_path).ok_or_else(|| {
            vec![package_error(
                "package-source import caller path is not authenticated",
            )]
        })?;
        let dependency = paths_to_modules.get(&edge.target_path).ok_or_else(|| {
            vec![package_error(
                "package-source import target path is not authenticated",
            )]
        })?;
        imports.push(PackageWorkspaceImport {
            dependent: dependent.clone(),
            dependency: dependency.clone(),
            target: edge.target.clone(),
            alias: edge.alias.clone(),
            ordinal: edge.ordinal,
        });
    }
    imports.sort();

    let mut reachable = BTreeSet::from([root_package.to_owned()]);
    let mut pending = BTreeSet::from([root_package.to_owned()]);
    while let Some(module) = pending.pop_first() {
        for import in imports.iter().filter(|value| value.dependent == module) {
            if reachable.insert(import.dependency.clone()) {
                pending.insert(import.dependency.clone());
            }
        }
    }
    if reachable.len() != build.hir.modules.len() {
        return Err(vec![package_error(
            "package-source selected module inventory is not wholly reachable from the root",
        )]);
    }

    let mut modules = Vec::with_capacity(build.hir.modules.len());
    let mut root_exports = Vec::new();
    for module in &build.hir.modules {
        if !module.permits.is_empty()
            || !module.types.is_empty()
            || !module.interfaces.is_empty()
            || !module.function_templates.is_empty()
            || !module.function_instances.is_empty()
            || module.functions.iter().any(|function| {
                !function.effects.is_empty()
                    || function.params.iter().any(|parameter| {
                        parameter.ownership != hir::OwnershipMode::Value
                            || !matches!(
                                parameter.ty,
                                hir::ResolvedType::I64 | hir::ResolvedType::Bool
                            )
                    })
                    || !matches!(
                        function.return_type,
                        hir::ResolvedType::I64 | hir::ResolvedType::Bool
                    )
            })
        {
            return Err(vec![package_profile_error(
                "package-source module is outside the effect-free scalar profile",
            )]);
        }
        let explicit = module
            .functions
            .iter()
            .filter(|function| {
                build
                    .hir
                    .declarations
                    .get(function.id.as_str())
                    .is_some_and(|fact| fact.origin == hir::IdentityOrigin::Explicit)
            })
            .collect::<Vec<_>>();
        let interface =
            crate::package_report_v2::scalar_interface_from_resolved(&module.module, &explicit)
                .map_err(|error| vec![error])?;
        if module.module == root_package {
            root_exports = interface
                .functions
                .iter()
                .map(|function| function.stable_id.clone())
                .collect();
        }
        modules.push(PackageWorkspaceModule {
            package: module.module.clone(),
            interface,
        });
    }
    modules.sort_by(|left, right| left.package.as_bytes().cmp(right.package.as_bytes()));
    let program = build.linked_scalar_program(root_package)?;
    let mut linked_function_ids = program
        .functions
        .iter()
        .map(|function| function.id.as_str().to_owned())
        .collect::<Vec<_>>();
    linked_function_ids.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    if linked_function_ids
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(vec![package_error(
            "package-source linked function identities are duplicated",
        )]);
    }
    root_exports.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    if root_exports.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(vec![package_error(
            "package-source root export identities are duplicated",
        )]);
    }
    Ok(PackageWorkspaceLink {
        program,
        modules,
        imports,
        linked_function_ids,
        root_exports,
    })
}

fn package_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS503", message.into())
}

fn package_profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS504", message.into())
}
