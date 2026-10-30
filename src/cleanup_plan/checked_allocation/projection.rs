//! Atomic private projection of an ordinary-validated source subject.
//! No executor receives a staged attachment. Complete independent replay must
//! finish for every held function before the read-only validated owner exists.
use super::*;
use crate::cleanup::ReplayContext;

pub(crate) struct Validated<'source> {
    source: &'source Program,
    program: ResolvedProgram,
    entry: DeclarationId,
    // Moved old attachments retain the ordinary proof input without cloning HIR.
    ordinary_plans: Vec<crate::cleanup_plan::CleanupPlan>,
}
impl<'source> Validated<'source> {
    pub(crate) fn source(&self) -> &'source Program {
        self.source
    }
    pub(crate) fn program(&self) -> &ResolvedProgram {
        &self.program
    }
    pub(crate) fn entry(&self) -> &DeclarationId {
        &self.entry
    }
    /// Independent engine receiving remains required; this is neither an
    /// allocator capability nor a release lease nor an escaped-result grant.
    pub(crate) fn ordinary_plan(&self, index: usize) -> Option<&crate::cleanup_plan::CleanupPlan> {
        self.ordinary_plans.get(index)
    }
    /// Each receiver repeats the complete projected replay on this immutable
    /// owner instead of trusting construction history or a copied verdict.
    pub(crate) fn replay(&self) -> Result<(), Diagnostic> {
        crate::cleanup_plan::validate::scratch_ledger::validate_program_context(&Projected {
            program: &self.program,
        })
    }
}

pub(super) fn receive<'source>(
    mut prepared: Prepared<'source>,
) -> Result<Validated<'source>, Refusal> {
    // Uncatalogued materialization refuses before any candidate is built, so
    // no checked closure retains a fatal allocation path.
    crate::cleanup_plan::replay::allocation_coverage::validate(&prepared.program)
        .map_err(Refusal::Coverage)?;
    let count = prepared
        .program
        .functions
        .len()
        .checked_add(prepared.program.function_instances.len())
        .ok_or(Refusal::Storage)?;
    let bytes = std::alloc::Layout::array::<crate::cleanup_plan::CleanupPlan>(count)
        .map_err(|_| Refusal::Storage)?
        .size();
    if !crate::bounded_output::reserve_active_required(bytes) {
        return Err(Refusal::Storage);
    }
    let mut plans = Vec::new();
    if plans.try_reserve_exact(count).is_err() {
        crate::bounded_output::refuse_active_required(bytes);
        return Err(Refusal::Storage);
    }
    if plans.capacity() != count {
        let excess = plans
            .capacity()
            .saturating_sub(count)
            .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupPlan>())
            .ok_or(Refusal::Storage)?;
        let _ = crate::bounded_output::reserve_active_required(excess);
        crate::bounded_output::refuse_active_required(bytes);
        return Err(Refusal::Storage);
    }
    // All candidates are constructed while every ordinary attachment remains
    // unchanged. Failure drops private data; there is no partial publication.
    for function in prepared.program.functions.iter().chain(
        prepared
            .program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        let current = Function {
            program: &prepared.program,
            function,
        };
        plans.push(
            crate::cleanup_plan::build::checked_allocation::candidate(current)
                .map_err(Refusal::Projection)?,
        );
    }
    for (function, plan) in prepared
        .program
        .functions
        .iter_mut()
        .chain(
            prepared
                .program
                .function_instances
                .iter_mut()
                .map(|instance| &mut instance.function),
        )
        .zip(plans.iter_mut())
    {
        // Exchange two owned plans; this does not clone any body or attachment.
        std::mem::swap(&mut function.cleanup_plan, plan);
    }
    let context = Projected {
        program: &prepared.program,
    };
    crate::cleanup_plan::validate::scratch_ledger::validate_program_context(&context)
        .map_err(Refusal::Projection)?;
    Ok(Validated {
        source: prepared.source,
        program: prepared.program,
        entry: prepared.entry,
        ordinary_plans: plans,
    })
}

/// The constructor is private to this source-owned phase. Downstream contexts
/// cannot mint a selection from a decoded schema or copied function.
struct Projected<'a> {
    program: &'a ResolvedProgram,
}
impl ReplayContext for Projected<'_> {
    fn declarations(&self) -> &hir::DeclarationIndex {
        self.program.declarations()
    }
    fn types(&self) -> hir::items::View<'_, hir::ResolvedTypeDeclaration> {
        self.program.types()
    }
    fn interfaces(&self) -> &[hir::ResolvedInterface] {
        self.program.interfaces()
    }
    fn functions(&self) -> hir::items::View<'_, ResolvedFunction> {
        self.program.functions()
    }
    fn function_templates(&self) -> &[hir::ResolvedFunctionTemplate] {
        self.program.function_templates()
    }
    fn function_instances(&self) -> &[hir::ResolvedFunctionInstance] {
        self.program.function_instances()
    }
    fn full_program(&self) -> Option<&ResolvedProgram> {
        Some(self.program)
    }
    fn allocation_function<'a>(&'a self, function: &'a ResolvedFunction) -> Option<Function<'a>> {
        self.program
            .functions
            .iter()
            .chain(
                self.program
                    .function_instances
                    .iter()
                    .map(|instance| &instance.function),
            )
            .any(|member| std::ptr::eq(member, function))
            .then_some(Function {
                program: self.program,
                function,
            })
    }
}

#[cfg(test)]
#[path = "projection/tests.rs"]
mod tests;
