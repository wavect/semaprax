//! Proof over actual provider bodies, never over the generated prototype.
use crate::ast::{
    Function, ModuleUseKind, ParamMode, Program, Type, TypeDeclaration, TypeDeclarationKind,
};
use crate::workspace_graph::{AuthoredDeclaration, AuthoredKind};
use std::collections::BTreeMap;

fn provider<'a>(
    function: &'a Function,
    module: &str,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<&'a Program> {
    let fact = authored.get(function.stable_id.as_str())?;
    if !fact.explicit
        || fact.kind != AuthoredKind::Function
        || fact.module != module
        || !std::ptr::eq(fact.function?, function)
    {
        return None;
    }
    let mut owners = programs.iter().filter(|program| program.module == module);
    let owner = owners.next()?;
    if owners.next().is_some()
        || owner
            .functions
            .iter()
            .filter(|candidate| candidate.stable_id == function.stable_id)
            .count()
            != 1
    {
        return None;
    }
    if !owner
        .functions
        .iter()
        .any(|candidate| std::ptr::eq(candidate, function))
    {
        return None;
    }
    Some(owner)
}
fn lookup<'a>(
    owner: &'a Program,
    name: &str,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<(&'a Program, &'a Function)> {
    let mut local = owner
        .functions
        .iter()
        .filter(|function| function.name == name);
    if let Some(function) = local.next() {
        if local.next().is_some() {
            return None;
        }
        return Some((
            provider(function, &owner.module, authored, programs)?,
            function,
        ));
    }
    let mut imports = owner
        .module_uses
        .iter()
        .filter(|import| import.kind == ModuleUseKind::Function && import.alias == name);
    let import = imports.next()?;
    if imports.next().is_some() {
        return None;
    }
    let fact = authored.get(import.persistent_id.as_str())?;
    if fact.module != import.target_module {
        return None;
    }
    let function = fact.function?;
    Some((
        provider(function, fact.module, authored, programs)?,
        function,
    ))
}
fn record<'a>(
    owner: &Program,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Option<&'a TypeDeclaration> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let defining = programs
        .iter()
        .find(|program| program.module == owner.module)?;
    let local = defining
        .types
        .iter()
        .find(|declaration| declaration.name == *name);
    let (identity, fact) = if let Some(declaration) = local {
        (
            declaration.stable_id.as_str(),
            authored.get(declaration.stable_id.as_str())?,
        )
    } else {
        let mut imports = defining
            .module_uses
            .iter()
            .filter(|import| import.kind == ModuleUseKind::Type && import.alias == *name);
        let import = imports.next()?;
        if imports.next().is_some() {
            return None;
        }
        let fact = authored.get(import.persistent_id.as_str())?;
        if fact.module != import.target_module {
            return None;
        }
        (import.persistent_id.as_str(), fact)
    };
    let declaration = fact.ty?;
    if declaration.stable_id != identity
        || !fact.explicit
        || fact.kind != AuthoredKind::Type
        || !declaration.type_parameters.is_empty()
        || !matches!(declaration.kind, TypeDeclarationKind::Record { .. })
    {
        return None;
    }
    let mut owners = programs
        .iter()
        .filter(|program| program.module == fact.module);
    let provider = owners.next()?;
    if owners.next().is_some()
        || provider
            .types
            .iter()
            .filter(|item| item.stable_id == declaration.stable_id)
            .count()
            != 1
        || !provider
            .types
            .iter()
            .any(|item| std::ptr::eq(item, declaration))
    {
        return None;
    }
    Some(declaration)
}
/// A borrowed builtin Vec needs its current imported element declaration,
/// not the name-only local classifier. This supplies no provider body proof.
fn borrowed_vector_record<'a>(
    owner: &Program,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> bool {
    let Type::Named { name, arguments } = ty else {
        return false;
    };
    if name != "Vec"
        || owner
            .types
            .iter()
            .any(|declaration| declaration.name == "Vec")
        || owner
            .module_uses
            .iter()
            .any(|import| import.kind == ModuleUseKind::Type && import.alias == "Vec")
    {
        return false;
    }
    let [element] = arguments.as_slice() else {
        return false;
    };
    record(owner, element, authored, programs)
        .is_some_and(super::super::type_rewrite::record_element)
}
fn parameter<'a>(
    owner: &Program,
    mode: ParamMode,
    ty: &Type,
    authored: &BTreeMap<&str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> bool {
    match mode {
        ParamMode::Own => *ty == Type::Bytes,
        ParamMode::Value => {
            crate::source_verify::is_scalar_source_type(ty)
                || crate::loop_calls::ast_copy_variant(owner, ty)
                || record(owner, ty, authored, programs)
                    .is_some_and(crate::source_verify::copy_record_declaration_admitted)
        }
        ParamMode::Borrow => {
            crate::loop_calls::ast_param_admitted(owner, mode, ty)
                || record(owner, ty, authored, programs).is_some()
                || borrowed_vector_record(owner, ty, authored, programs)
        }
        ParamMode::Shared => false,
    }
}

pub(in crate::workspace_graph::expected_projection) struct ProviderProofs<'a> {
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
    proofs: crate::loop_calls::byte_writer::SourceProofs<'a>,
    carrier_charged: bool,
}
impl<'a> ProviderProofs<'a> {
    pub(in crate::workspace_graph::expected_projection) fn new(
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
    ) -> Self {
        Self {
            authored,
            programs,
            proofs: Default::default(),
            carrier_charged: false,
        }
    }
    pub(in crate::workspace_graph::expected_projection) fn authorized(
        &mut self,
        function: &'a Function,
        module: &str,
    ) -> Result<bool, crate::diagnostic::Diagnostic> {
        if let Some(error) = self.proofs.failure() {
            return Err(error);
        }
        let authored = self.authored;
        let programs = self.programs;
        if function.return_type != Type::Bytes
            || function
                .params
                .iter()
                .filter(|param| param.mode == ParamMode::Own && param.ty == Type::Bytes)
                .count()
                != 1
        {
            return Ok(false);
        }
        let Some(owner) = provider(function, module, authored, programs) else {
            return Ok(false);
        };
        let Some(lookup_work) = programs.iter().try_fold(programs.len(), |work, program| {
            work.checked_add(program.functions.len())?
                .checked_add(program.types.len())?
                .checked_add(program.module_uses.len())
        }) else {
            return Err(crate::diagnostic::Diagnostic::io(
                "SPX-G171",
                "byte helper provider proof lookup work overflowed",
            ));
        };
        if !self.carrier_charged {
            self.proofs.reserve_carrier(
                std::mem::size_of::<Self>()
                    - std::mem::size_of::<crate::loop_calls::byte_writer::SourceProofs<'a>>(),
            )?;
            self.carrier_charged = true;
        }
        self.proofs.prove(
            owner,
            function,
            |owner, name| {
                let crate::ast::program_source::Subject::Owned(owner) = owner.subject() else {
                    return None;
                };
                lookup(owner, name, authored, programs)
                    .map(|(p, f)| (p as &dyn crate::ast::program_source::ProgramSource, f))
            },
            |owner, mode, ty| {
                let crate::ast::program_source::Subject::Owned(owner) = owner.subject() else {
                    return false;
                };
                parameter(owner, mode, ty, authored, programs)
            },
            |owner, ty| {
                let crate::ast::program_source::Subject::Owned(owner) = owner.subject() else {
                    return false;
                };
                crate::source_verify::is_scalar_source_type(ty)
                    || crate::loop_calls::ast_copy_variant(owner, ty)
                    || record(owner, ty, authored, programs)
                        .is_some_and(crate::source_verify::copy_record_declaration_admitted)
            },
            lookup_work,
        )
    }
}

// PROVIDER_PARTS_BEGIN
impl<'a> ProviderProofs<'a> {
    pub(super) fn from_backing(
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
        backing: crate::loop_calls::byte_writer::SourceBacking<'a>,
        carrier_charged: bool,
    ) -> Result<Self, crate::diagnostic::Diagnostic> {
        Ok(Self {
            authored,
            programs,
            proofs: crate::loop_calls::byte_writer::SourceProofs::from_backing(backing)?,
            carrier_charged,
        })
    }
    pub(super) fn into_parts(
        self,
    ) -> (
        crate::loop_calls::byte_writer::SourceBacking<'a>,
        bool,
        bool,
    ) {
        let Self {
            authored: _,
            programs: _,
            proofs,
            carrier_charged,
        } = self;
        let (backing, failed) = proofs.into_backing();
        (backing, carrier_charged, failed)
    }
    #[cfg(test)]
    pub(super) fn observation(&self) -> (usize, usize, usize, usize) {
        self.proofs.scope_observation()
    }
}
// PROVIDER_PARTS_END

#[cfg(test)]
#[path = "byte_helper/tests.rs"]
#[allow(
    clippy::duplicate_mod,
    reason = "Run the same controls against independent receivers."
)]
mod tests;
