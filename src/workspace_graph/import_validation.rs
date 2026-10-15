//! Ordinary authored import checks with optional held private layout facts.
use super::expected_projection::private_layout::registry::LayoutRegistry;
use super::{
    diagnostics, type_contains_name_from, use_error, validate_imported_type, AuthoredDeclaration,
    AuthoredKind,
};
use crate::{
    ast::{ModuleUseKind, Program},
    diagnostic::Diagnostic,
    prelude,
};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn validate_uses_with_registry(
    programs: &[Program],
    modules: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    registry: Option<&LayoutRegistry<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    for program in programs {
        let mut function_aliases = BTreeSet::new();
        let mut type_aliases = BTreeSet::new();
        let mut protocol_aliases = BTreeSet::new();
        let mut imported_targets = BTreeSet::new();
        let local_functions = program
            .functions
            .iter()
            .map(|item| item.name.as_str())
            .chain(
                program
                    .interfaces
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .map(|import| import.name.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let local_types = program
            .types
            .iter()
            .map(|item| item.name.as_str())
            .chain(
                prelude::declarations()
                    .iter()
                    .map(|item| item.name.as_str()),
            )
            .collect::<BTreeSet<_>>();
        let local_protocols = program
            .protocols
            .iter()
            .map(|item| item.name.as_str())
            .collect::<BTreeSet<_>>();
        let workspace_type_aliases = program
            .module_uses
            .iter()
            .filter(|item| item.kind == ModuleUseKind::Type)
            .map(|item| item.alias.as_str())
            .collect::<BTreeSet<_>>();
        for interface in &program.interfaces {
            for import in &interface.imports {
                for param in &import.params {
                    if type_contains_name_from(&param.ty, &workspace_type_aliases) {
                        return Err(vec![Diagnostic::error(
                            "SPX-G172",
                            "workspace type aliases are not admitted in interface/import parameter carriers",
                            param.span,
                        )
                        .at_path(&program.path)]);
                    }
                }
            }
        }
        let mut private_layout = None;
        for module_use in &program.module_uses {
            if module_use
                .persistent_id
                .starts_with(crate::vec_ops::field_update::PREFIX)
            {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "compiler-owned scalar field update descriptors cannot be imported",
                )]);
            }
            if module_use
                .persistent_id
                .starts_with(crate::vec_ops::sort_keys::PREFIX)
            {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "compiler-owned explicit sort descriptors cannot be imported",
                )]);
            }
            if module_use
                .persistent_id
                .starts_with(crate::vec_ops::field_unique::PREFIX)
            {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "compiler-owned field uniqueness descriptors cannot be imported",
                )]);
            }
            if !imported_targets.insert((module_use.kind, module_use.persistent_id.as_str())) {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "the same workspace target is imported more than once",
                )]);
            }
            if module_use.target_module == program.module
                || !modules.contains_key(module_use.target_module.as_str())
            {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "target module is missing or equals the caller module",
                )]);
            }
            let alias_conflicts = match module_use.kind {
                ModuleUseKind::Function => {
                    !function_aliases.insert(module_use.alias.as_str())
                        || local_functions.contains(module_use.alias.as_str())
                        || module_use.alias == "main"
                        || crate::vec_ops::by_name(&module_use.alias).is_some()
                }
                ModuleUseKind::Type => {
                    !type_aliases.insert(module_use.alias.as_str())
                        || local_types.contains(module_use.alias.as_str())
                }
                ModuleUseKind::Protocol => {
                    !protocol_aliases.insert(module_use.alias.as_str())
                        || local_protocols.contains(module_use.alias.as_str())
                }
            };
            if alias_conflicts {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "alias is duplicated or shadows a local/prelude declaration",
                )]);
            }
            let target = authored
                .get(module_use.persistent_id.as_str())
                .ok_or_else(|| {
                    vec![diagnostics::unknown_import_error(
                        program, module_use, modules,
                    )]
                })?;
            let expected = match module_use.kind {
                ModuleUseKind::Function => AuthoredKind::Function,
                ModuleUseKind::Type => AuthoredKind::Type,
                ModuleUseKind::Protocol => AuthoredKind::Protocol,
            };
            if !target.explicit
                || target.module != module_use.target_module
                || target.kind != expected
            {
                return Err(vec![use_error(
                    program,
                    module_use,
                    "target module, declaration kind, or explicit identity disagrees",
                )]);
            }
            match module_use.kind {
                ModuleUseKind::Function => {
                    super::owned_function_import::validate_imported_function_with_registry(
                        program,
                        module_use,
                        target,
                        authored,
                        programs,
                        &mut private_layout,
                        registry,
                    )?
                }
                ModuleUseKind::Type => {
                    validate_imported_type(program, module_use, target, authored, programs)?;
                }
                ModuleUseKind::Protocol => {}
            }
        }
    }
    Ok(())
}
