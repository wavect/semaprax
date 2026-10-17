//! Exact current constructor subject, authenticated in the original order.
use super::super::function_lookup::original as function_lookup;
use super::super::generic_template::original_call_graph as generic_template;
use super::original_intrinsics as box_intrinsic;
use super::*;
use crate::hir::inspection::validate_nul_free_identities_original as validate_nul_free_identities;

pub(in crate::hir) struct ConstructorSeed<'a> {
    pub(super) program: &'a ResolvedProgram,
    pub(super) functions: function_lookup::Lookup<'a>,
}
impl<'a> ConstructorSeed<'a> {
    pub(in crate::hir) fn authenticate(
        program: &'a ResolvedProgram,
        directory: Option<&'a crate::hir::function_directory::Directory<'a>>,
    ) -> Result<Self, Diagnostic> {
        validate_nul_free_identities(program)?;
        stdin_stream::reject_sealed_escape(program)?;
        string_intrinsic::reject_reserved_identities(program)?;
        box_intrinsic::reject_reserved_identities(program)?;
        crate::hir::closure::once::reject_reserved_identities(program)?;
        generic_template::validate_call_graph(program)?;
        for declaration in program.declarations.declarations() {
            if crate::host_io_ops::by_id(declaration.id.as_str()).is_some()
                || crate::command_io_ops::by_id(declaration.id.as_str()).is_some()
            {
                return Err(hir_error(format!(
                    "resolved {:?} declaration `{}` aliases a compiler-owned host I/O operation",
                    declaration.kind, declaration.id
                )));
            }
        }
        let functions = function_lookup::authenticate(program, directory)?;
        for template in &program.function_templates {
            if crate::host_io_ops::by_id(template.id.as_str()).is_some()
                || crate::command_io_ops::by_id(template.id.as_str()).is_some()
            {
                return Err(hir_error(format!(
                    "resolved function template `{}` aliases a compiler-owned host I/O operation",
                    template.id
                )));
            }
        }
        Ok(Self { program, functions })
    }
    pub(in crate::hir) fn program(&self) -> &'a ResolvedProgram {
        self.program
    }
}
