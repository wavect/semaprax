use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};

use crate::ast::{Param, ParamMode, Type};
use crate::diagnostic::Diagnostic;
use crate::package_report_v2::ScalarPackageInterface;

use super::{build_owned, hir, WorkspaceGraphBuild, WorkspaceSource};

thread_local! {
    /// Set only for the duration of one package-source workspace build, which
    /// authenticates the closed cross-package owner-view boundary. Every
    /// ordinary Project, draft, and candidate build keeps the narrower profile.
    static ACTIVE_PACKAGE_SOURCE_BUILD: Cell<bool> = const { Cell::new(false) };
}

/// Build one package-source workspace: the ordinary owned build plus exactly
/// one additional admitted import shape, the whole `own Bytes` argument the
/// package owner-view boundary authenticates. The flag lives only for this
/// call, so no other build can observe the wider profile.
pub(super) fn build_owned_package_sources(
    sources: Vec<WorkspaceSource>,
) -> Result<WorkspaceGraphBuild, Vec<Diagnostic>> {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE_PACKAGE_SOURCE_BUILD.with(|active| active.set(self.0));
        }
    }
    let restore = Restore(ACTIVE_PACKAGE_SOURCE_BUILD.with(|active| active.replace(true)));
    let result = build_owned(sources);
    drop(restore);
    result
}

/// The byte parameters an imported function may declare. A borrowed view is
/// admitted everywhere; the whole owned transfer only inside a package-source
/// build. Both carry no lifetime out, so the caller still requires a
/// non-borrowing scalar result.
pub(super) fn admitted_byte_parameter(param: &Param) -> bool {
    (param.mode == ParamMode::Borrow && param.ty == Type::SliceU8)
        || (ACTIVE_PACKAGE_SOURCE_BUILD.with(Cell::get)
            && param.mode == ParamMode::Own
            && param.ty == Type::Bytes)
}

/// The exact refusal text for the import profile the active build admits.
pub(super) fn import_profile_refusal() -> &'static str {
    if ACTIVE_PACKAGE_SOURCE_BUILD.with(Cell::get) {
        "function target must be monomorphic with admitted value parameters, or byte parameters and a scalar return"
    } else {
        "function target must be monomorphic with admitted value parameters, or borrowed byte-slice parameters and a scalar return"
    }
}

pub(crate) struct PackageWorkspaceLink {
    build: super::WorkspaceGraphBuild,
    root_package: String,
    pub(crate) modules: Vec<PackageWorkspaceModule>,
    pub(crate) imports: Vec<PackageWorkspaceImport>,
    pub(crate) root_exports: Vec<String>,
}

impl PackageWorkspaceLink {
    /// Actual independently checked cross-package call edges over every source
    /// function/contract, including callers outside the linked export closure.
    pub(crate) fn call_facts(&self) -> Result<Vec<PackageWorkspaceCall>, Diagnostic> {
        // The ordinary workspace builder already bounds all edges to 65,536.
        // This graph-only clone additionally bounds its retained text before
        // allocating any call row; old package builds never request this view.
        let mut retained_bytes = 0usize;
        for edge in self.build.edges.iter().filter(|edge| edge.kind == "call") {
            retained_bytes = retained_bytes
                .saturating_add(std::mem::size_of::<PackageWorkspaceCall>())
                .saturating_add(edge.caller.len())
                .saturating_add(edge.target.len())
                .saturating_add(edge.expression.len())
                .saturating_add(edge.ast_path.len())
                .saturating_add(edge.alias.len())
                .saturating_add(1024);
            if retained_bytes > 16 * 1024 * 1024 {
                return Err(Diagnostic::io(
                    "SPX-PS603",
                    "package graph call fact retention exceeds its byte bound",
                ));
            }
        }
        let paths = self
            .build
            .hir
            .module_paths
            .iter()
            .map(|(module, path)| (path.as_str(), module.as_str()))
            .collect::<BTreeMap<_, _>>();
        self.build
            .edges
            .iter()
            .filter(|edge| edge.kind == "call")
            .map(|edge| {
                let caller_package = paths
                    .get(edge.caller_path.as_str())
                    .ok_or_else(|| package_error("package call caller source is absent"))?;
                let target_package = paths
                    .get(edge.target_path.as_str())
                    .ok_or_else(|| package_error("package call target source is absent"))?;
                if caller_package == target_package {
                    return Err(package_error("package call edge is not cross-package"));
                }
                Ok(PackageWorkspaceCall {
                    caller_package: (*caller_package).to_owned(),
                    target_package: (*target_package).to_owned(),
                    caller: edge.caller.clone(),
                    target: edge.target.clone(),
                    site: edge.site,
                    expression: edge.expression.clone(),
                    ast_path: edge.ast_path.clone(),
                    alias: edge.alias.clone(),
                    ordinal: edge.ordinal,
                })
            })
            .collect()
    }
    /// Link the exact authenticated root export set and its transitive scalar
    /// callees. Package roots deliberately do not inherit Project's authored
    /// `main` display-name requirement; the byte-lowest selected `fn() -> i64`
    /// export is the internal HIR anchor and remains an ordinary package export.
    pub(crate) fn link_root_exports(&self) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
        self.build
            .linked_package_scalar_exports(&self.root_package, &self.root_exports)
    }
}

pub(crate) struct PackageWorkspaceCall {
    pub(crate) caller_package: String,
    pub(crate) target_package: String,
    pub(crate) caller: String,
    pub(crate) target: String,
    pub(crate) site: &'static str,
    pub(crate) expression: String,
    pub(crate) ast_path: String,
    pub(crate) alias: String,
    pub(crate) ordinal: usize,
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
    let build = build_owned_package_sources(sources)?;
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
                    || function
                        .params
                        .iter()
                        .any(|parameter| !package_parameter(parameter))
                    || !hir::package_scalar_type(&function.return_type)
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
    root_exports.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    if root_exports.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(vec![package_error(
            "package-source root export identities are duplicated",
        )]);
    }
    Ok(PackageWorkspaceLink {
        build,
        root_package: root_package.to_owned(),
        modules,
        imports,
        root_exports,
    })
}

fn package_parameter(parameter: &hir::ResolvedParam) -> bool {
    matches!(
        (&parameter.ty, parameter.ownership),
        (
            hir::ResolvedType::I64
                | hir::ResolvedType::I32
                | hir::ResolvedType::F32
                | hir::ResolvedType::F64
                | hir::ResolvedType::Bool
                | hir::ResolvedType::Char
                | hir::ResolvedType::U8
                | hir::ResolvedType::Usize,
            hir::OwnershipMode::Value
        ) | (hir::ResolvedType::Bytes, hir::OwnershipMode::Own)
            | (hir::ResolvedType::SliceU8, hir::OwnershipMode::Borrow)
    )
}

fn package_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS503", message.into())
}

fn package_profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS504", message.into())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn source(path: &str, text: &str) -> WorkspaceSource {
        let program = crate::parse(text, Path::new(path)).expect("package fixture parses");
        WorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&program),
        }
    }

    fn provider() -> WorkspaceSource {
        source(
            "lib.spx",
            r#"
module lib.math;

@id("lib.answer")
fn answer() -> i64 { 41 }

@id("lib.unused")
fn unused() -> i64 { 99 }
"#,
        )
    }

    #[test]
    fn package_link_uses_non_main_anchor_and_retains_uncalled_selected_export() {
        let root = source(
            "app.spx",
            r#"
module app.main;
use function @id("lib.answer") from lib.math as answer;

@id("app.anchor")
fn boot() -> i64 { answer() }

@id("app.uncalled")
fn inspect(value: bool) -> bool { value }

@id("app.z_anchor")
fn alternate() -> i64 { 7 }

@id("workspace.synthetic.main.authored")
fn prefixed(value: bool) -> bool { value }
"#,
        );
        let package = build_package_scalar_sources(vec![root, provider()], "app.main")
            .expect("package workspace");
        let linked = package.link_root_exports().expect("package-only link");
        assert_eq!(linked.entrypoint.as_str(), "app.anchor");
        assert!(linked
            .functions
            .iter()
            .all(|function| function.name != "main"));
        assert_eq!(
            linked
                .functions
                .iter()
                .map(|function| function.id.as_str())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "app.anchor",
                "app.uncalled",
                "app.z_anchor",
                "lib.answer",
                "workspace.synthetic.main.authored",
            ])
        );
        assert!(!linked
            .functions
            .iter()
            .any(|function| function.id.as_str() == "lib.unused"));
    }

    #[test]
    fn package_link_rejects_root_without_noarg_i64_anchor() {
        let root = source(
            "app.spx",
            r#"
module app.main;
use function @id("lib.answer") from lib.math as answer;

@id("app.only_bool")
fn inspect() -> bool { answer() == 41 }
"#,
        );
        let package = build_package_scalar_sources(vec![root, provider()], "app.main")
            .expect("package workspace");
        assert_eq!(
            package.link_root_exports().unwrap_err()[0].code,
            "SPX-PS504"
        );
    }

    #[test]
    fn package_link_rejects_noncanonical_export_root_order() {
        let root = source(
            "app.spx",
            r#"
module app.main;
use function @id("lib.answer") from lib.math as answer;

@id("app.a")
fn first() -> i64 { answer() }

@id("app.b")
fn second() -> i64 { 2 }
"#,
        );
        let mut package = build_package_scalar_sources(vec![root, provider()], "app.main")
            .expect("package workspace");
        package.root_exports.reverse();
        assert_eq!(
            package.link_root_exports().unwrap_err()[0].code,
            "SPX-PS503"
        );
    }
}
