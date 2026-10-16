//! Complete ordinary inventory validation with only retained summary state.
//! Every root finishes its old shape/profile validation before publication.
use super::inventory_metadata as metadata;
use super::*;
use crate::hir::{
    OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedStatement,
    ResolvedTypeDeclaration, ResolvedTypeDeclarationKind,
};
mod collector;
mod generic;
mod profile;
mod shape;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub(in crate::cleanup_plan::replay) struct Summary {
    pub(in crate::cleanup_plan::replay) has_nested_owned_bytes: bool,
    pub(in crate::cleanup_plan::replay) has_variant: bool,
    pub(in crate::cleanup_plan::replay) has_iterator: bool,
    pub(in crate::cleanup_plan::replay) storage_count: usize,
    pub(in crate::cleanup_plan::replay) flag_count: usize,
}
struct Builder<'a, 'b> {
    program: &'a dyn crate::cleanup::ReplayContext,
    payer: &'a mut Payer<'b>,
    summary: Summary,
    shape_frames: Vec<shape::Frame<'a>>,
}
fn error(message: &'static str) -> Diagnostic {
    Diagnostic::io_static("SPX-H006", message)
}
impl<'s> Builder<'s, '_> {
    fn needs_drop(&mut self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        inventory_drop::needs_drop(self.program, ty, self.payer)
    }
    fn leaf(
        &mut self,
        id: &str,
        depth: usize,
        profile: &mut profile::Tracker,
    ) -> Result<(), Diagnostic> {
        metadata::step(self.payer, 1)?;
        u32::try_from(self.summary.flag_count)
            .map_err(|_| error("too many cleanup liveness flags"))?;
        self.summary.flag_count += 1;
        self.summary.has_iterator |=
            inventory_text::same_bytes(id, crate::cleanup::ITER_DROP_LIFECYCLE_ID, self.payer)?;
        profile.leaf(id, depth, self.payer)
    }
    fn find<'a>(
        &mut self,
        id: &DeclarationId,
        program: &'a dyn crate::cleanup::ReplayContext,
    ) -> Result<Option<&'a ResolvedTypeDeclaration>, Diagnostic> {
        for item in program.types() {
            metadata::step(self.payer, 1)?;
            if inventory_text::owner_matches(&item.id, id, self.payer)? {
                return Ok(Some(item));
            }
        }
        Ok(None)
    }
    fn unsupported(&mut self, ty: &ResolvedType) -> Result<Diagnostic, Diagnostic> {
        let key = inventory_raw_key::identity(ty, self.payer)?;
        Ok(Diagnostic::io(
            "SPX-H006",
            inventory_text::raw(
                self.payer,
                format_args!("droppable type `{key}` has unsupported generic arguments"),
            )?,
        ))
    }
    fn unknown(&mut self, id: &DeclarationId) -> Result<Diagnostic, Diagnostic> {
        Ok(Diagnostic::io(
            "SPX-H006",
            inventory_text::raw(self.payer, format_args!("unknown cleanup type `{id}`"))?,
        ))
    }
    fn add_slot(&mut self, ty: &'s ResolvedType) -> Result<bool, Diagnostic> {
        metadata::step(self.payer, 1)?;
        u32::try_from(self.summary.storage_count)
            .map_err(|_| error("too many cleanup storage candidates"))?;
        let (profile, variant) = self.shape(ty)?;
        self.summary.storage_count += 1;
        self.summary.has_nested_owned_bytes |= profile.has_nested_owned_bytes;
        self.summary.has_variant |= variant;
        // The old conditional_entry is queried only for this freshly returned
        // slot. No externally supplied storage ID can enter this collector.
        Ok(variant)
    }
}

/// Publication occurs only after all original signature/contracts/body/ensures
/// validation succeeds. Every fresh slot already finished its complete immutable
/// shape profile, so the old final second pass cannot discover new semantics.
pub(super) fn collect_with_payer<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    payer: &'a mut Payer<'_>,
) -> Result<Summary, Diagnostic> {
    inventory_text::current(payer)?;
    metadata::step(payer, 1)?;
    payer.bytes(std::mem::size_of::<Builder<'_, '_>>())?;
    // The retained frontier header is included in Builder; pay its actual
    // construction and eventual receiving-buffer release before publication.
    metadata::step(payer, 2)?;
    let mut builder = Builder {
        program,
        payer,
        summary: Summary::default(),
        shape_frames: Vec::new(),
    };
    for (parameter_index, parameter) in function.params.iter().enumerate() {
        metadata::step(builder.payer, 1)?;
        if parameter.ownership == OwnershipMode::Own && builder.needs_drop(&parameter.ty)? {
            u32::try_from(parameter_index).map_err(|_| error("too many function parameters"))?;
            builder.add_slot(&parameter.ty)?;
        }
    }
    for expression in &function.requires {
        builder.collect_expression(expression)?;
    }
    builder.collect_expression(&function.body)?;
    if builder.needs_drop(&function.return_type)? {
        builder.add_slot(&function.return_type)?;
    }
    for expression in &function.ensures {
        builder.collect_expression(expression)?;
    }
    Ok(builder.summary)
}

#[cfg(test)]
mod tests;

pub(in crate::cleanup_plan::replay) fn collect(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Summary, Diagnostic> {
    let mut payer = Payer {
        function,
        budget,
        ledger: crate::bounded_output::classifier_string::ClassifierLedger::capture(),
    };
    collect_with_payer(program, function, &mut payer)
}

#[cfg(test)]
mod frontier_tests;
#[cfg(test)]
mod shape_original;
