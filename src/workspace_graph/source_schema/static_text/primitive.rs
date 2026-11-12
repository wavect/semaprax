//! Independent source-signature and current namespace authentication for the
//! module-local primitive retained lexical replay context.
use super::*;
mod helpers;
mod records;
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::{SelectedReplayRefusal, SelectedReplayReport, SelectedReplayReportForm};
// Every authenticator on the qualified replay route refuses with this Copy
// value; none constructs a Diagnostic, Vec or String before the transfer.
use crate::cleanup_plan::SelectedReplayRefused as Refused;

/// Test-only legacy rendering of an inline refusal as the public G410 Vec,
/// for owners outside the qualified route that still return `Vec`.
#[cfg(test)]
fn legacy(_: Refused) -> Vec<Diagnostic> {
    fail()
}
use crate::hir::{
    ResolvedFunction, ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedInterface,
    ResolvedTypeDeclaration,
};

pub(super) struct Context<'a> {
    index: hir::DeclarationIndex,
    module: &'a WorkspaceResolvedModule,
    /// Retained modules hold only authored types. The canonical compiler-owned
    /// Vec, rebuilt with its index metadata on the same preparation, follows
    /// them in the receiving current directory; `types()` lends the exact module-local view.
    vector: Option<ResolvedTypeDeclaration>,
}
impl ReplayContext for Context<'_> {
    fn declarations(&self) -> &hir::DeclarationIndex {
        &self.index
    }
    fn types(&self) -> crate::hir::items::View<'_, ResolvedTypeDeclaration> {
        crate::hir::items::ItemSource::view(&self.module.types)
    }
    fn type_count(&self) -> usize {
        self.module.types.len() + usize::from(self.vector.is_some())
    }
    fn type_at(&self, index: usize) -> Option<&ResolvedTypeDeclaration> {
        self.module.types.get(index).or_else(|| {
            self.vector
                .as_ref()
                .filter(|_| index == self.module.types.len())
        })
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &[]
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        crate::hir::items::ItemSource::view(&self.module.functions)
    }
    fn function_templates(&self) -> &[ResolvedFunctionTemplate] {
        &[]
    }
    fn function_instances(&self) -> &[ResolvedFunctionInstance] {
        &[]
    }
    fn retained_static_byte_iterators(&self) -> bool {
        self.vector.is_some()
    }
}

#[cfg(test)]
pub(super) fn authenticate<'a>(
    module: &'a WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    physical: &mut crate::cleanup_plan::ReplayPhysicalReservation,
) -> Result<Context<'a>, Vec<Diagnostic>> {
    let inventory = module
        .static_text
        .as_deref()
        .and_then(|i| i.lexical.as_deref())
        .ok_or_else(fail)?;
    if !module.interfaces.is_empty()
        || !module.permits.is_empty()
        || !module.agents.is_empty()
        || !module.function_templates.is_empty()
        || !module.function_instances.is_empty()
        || module.functions.len() != inventory.functions.len()
    {
        return Err(fail());
    }
    physical
        .reserve(crate::workspace_graph::static_text::primitive::digest_scratch_bytes())
        .map_err(|_| fail())?;
    let mut work = WORK;
    for (source, function) in inventory.functions.iter().zip(&module.functions) {
        let declaration = facts.get(source.id.as_str()).ok_or_else(fail)?;
        if declaration.kind != hir::DeclarationKind::Function
            || declaration.owner.is_some()
            || declaration.origin
                != if source.explicit {
                    hir::IdentityOrigin::Explicit
                } else {
                    hir::IdentityOrigin::Automatic
                }
            || declaration.path.as_deref() != Some(module.path.as_str())
            || declaration.module.as_deref() != Some(module.module.as_str())
        {
            return Err(fail());
        }
        if function.id != source.id
            || !crate::workspace_graph::static_text::primitive::body_matches(
                inventory, source, function,
            )
            || function.name != source.name
            || !function.effects.is_empty()
            || function.yields.is_some()
            || function.return_type != source.result
            || function.params.len() != source.params.len()
        {
            return Err(fail());
        }
        work = work.checked_sub(source.params.len()).ok_or_else(fail)?;
        for (source, parameter) in source.params.iter().zip(&function.params) {
            if parameter.name != source.name
                || parameter.ownership != source.ownership
                || parameter.ty != source.ty
            {
                return Err(fail());
            }
        }
    }
    // Exact authored declaration order is part of this rare private inventory.
    // Reconstruct from current typed functions only after the complete source
    // body guard and current namespace/signature comparison, with no searches.
    let origins = module
        .functions
        .iter()
        .zip(&inventory.functions)
        .map(|(function, source)| {
            (
                function,
                if source.explicit {
                    hir::IdentityOrigin::Explicit
                } else {
                    hir::IdentityOrigin::Automatic
                },
            )
        });
    let mut index =
        hir::DeclarationIndex::primitive_replay(origins, physical).map_err(|e| vec![e])?;
    records::authenticate(
        module,
        inventory.records.as_deref(),
        facts,
        &mut index,
        physical,
    )?;
    // This legacy test owner reconstructs no Vec index metadata, so its
    // current directory lends none either.
    Ok(Context {
        index,
        module,
        vector: None,
    })
}

/// Static literal sites have already been independently authenticated by the
/// caller. Rebuild primitive declarations, then run complete typed lexical
/// replay with the same cumulative reservation for every selected function.
///
/// The actual caller selects one fixed outward `form`. Its paid carrier is
/// acquired on this module's ledger before preparation and is the only
/// outward report: setup, authentication, constructor and Session refusals
/// transfer it without allocation. Authentication refuses with the Copy
/// `Refused`; a Session refusal moves its first typed engine receipt into the
/// failed owner (or retains an unmigrated Owned Diagnostic), never the report (the public G410
/// literal is a fixed remapping). A refused carrier returns the rejected
/// ledger and Copy reason; only the caller's named `Unpaid` boundary may turn
/// that into an owned, unfunded Diagnostic.
pub(in crate::workspace_graph::source_schema) fn replay_paid(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    form: SelectedReplayReportForm,
) -> Result<bool, SelectedReplayRefusal> {
    let Some(inventory) = module.static_text.as_deref() else {
        return Ok(false);
    };
    if inventory.lexical.is_none() {
        return Ok(false);
    }
    let physical = crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES);
    let acquired = SelectedReplayReport::try_acquire(physical, form)?;
    if inventory
        .lexical
        .as_deref()
        .is_some_and(|inventory| inventory.imported)
    {
        helpers::replay(module, modules, facts, acquired)?;
        return Ok(true);
    }
    let Some(anchor) = module.functions.first() else {
        return Err(acquired.refuse_unprepared());
    };
    let mut preparation = acquired.prepare(anchor)?;
    let (context, lexical) = match members(module, modules, facts, &mut preparation, anchor) {
        Ok(members) => members,
        Err(refused) => return Err(preparation.refuse(refused)),
    };
    let mut session = preparation.into_paid_session(&context, anchor)?;
    let mut found = false;
    for function in lexical {
        // The first member refusal stops the route: no failed Session is
        // validated or finished again, so no legacy retry Diagnostic exists.
        if let Err(error) = session.validate_paid(function) {
            return Err(session.into_failed_preparation().refuse_engine(error));
        }
        found = true;
    }
    if !found {
        return Err(session.into_failed_preparation().refuse(Refused::MISMATCH));
    }
    drop(session.finish_paid()?);
    Ok(true)
}

/// Local authentication and lexical member selection on the held preparation.
/// Every refusal is the inline Copy `Refused`; the held preparation keeps the
/// failed ledger and any first paid budget receipt.
fn members<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    preparation: &mut crate::cleanup_plan::SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(Context<'a>, Vec<&'a ResolvedFunction>), Refused> {
    current_module(module, modules, preparation, anchor)?;
    preparation.try_reserve_array::<Context<'_>>(anchor, 1, "retained lexical context")?;
    let context = helpers::local::authenticate(module, facts, preparation, anchor)?;
    preparation.try_reserve_array::<&ResolvedFunction>(
        anchor,
        module.functions.len(),
        "retained lexical member references",
    )?;
    let mut lexical = Vec::with_capacity(module.functions.len());
    if lexical.capacity() != module.functions.len() {
        preparation.fail();
        return Err(Refused::MISMATCH);
    }
    for function in &module.functions {
        if helpers::local::requires(function, preparation, anchor)? {
            preparation.try_charge(anchor, 1, "retained lexical member insertion")?;
            lexical.push(function);
        }
    }
    Ok((context, lexical))
}

// Test callers acquire the real held-source context; no metadata is projected away.
#[cfg(test)]
pub(in crate::workspace_graph) fn with_test_context<T>(
    module: &WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    apply: impl FnOnce(&dyn ReplayContext) -> T,
) -> Result<T, Vec<Diagnostic>> {
    let mut physical =
        crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES);
    let context = authenticate(module, facts, &mut physical)?;
    Ok(apply(&context))
}

/// Tests hand in the sole owned preparation, and receive it back only with
/// the current source-authenticated context held for the callback's duration.
#[cfg(test)]
pub(in crate::workspace_graph) fn with_prepared_test_context<T>(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    mut preparation: crate::cleanup_plan::SelectedReplayPreparation,
    apply: impl FnOnce(&dyn ReplayContext, crate::cleanup_plan::SelectedReplayPreparation) -> T,
) -> Result<T, Vec<Diagnostic>> {
    let anchor = module.functions.first().ok_or_else(fail)?;
    let imported = module
        .static_text
        .as_ref()
        .and_then(|i| i.lexical.as_ref())
        .is_some_and(|i| i.imported);
    if imported {
        let current =
            current_module_receipt(module, modules, &mut preparation, anchor).map_err(legacy)?;
        let context =
            helpers::authenticate_current(current, facts, &mut preparation).map_err(legacy)?;
        return Ok(apply(&context, preparation));
    }
    current_module(module, modules, &mut preparation, anchor).map_err(legacy)?;
    preparation
        .try_reserve_array::<Context<'_>>(anchor, 1, "held test source context")
        .map_err(legacy)?;
    let context =
        helpers::local::authenticate(module, facts, &mut preparation, anchor).map_err(legacy)?;
    Ok(apply(&context, preparation))
}

/// Exact object membership in the live module slice, with a unique path and
/// namespace. Shared by every retained source receiver.
/// An invocation-local exact module/slice membership and namespace receipt.
/// Only the charged complete scan constructs it; it carries no wire authority.
/// Its borrowed pointers cannot be transplanted to an equal or changed input.
pub(in crate::workspace_graph::source_schema) struct CurrentModule<'a> {
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    owner: *const crate::cleanup_plan::SelectedReplayPreparation,
}
pub(in crate::workspace_graph::source_schema) fn current_module(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    preparation: &mut crate::cleanup_plan::SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Refused> {
    let mut exact = 0usize;
    let mut namespace = 0usize;
    for current in modules {
        preparation.try_charge(
            anchor,
            current
                .path
                .len()
                .checked_add(module.path.len())
                .and_then(|n| n.checked_add(current.module.len()))
                .and_then(|n| n.checked_add(module.module.len()))
                .and_then(|n| n.checked_add(1))
                .ok_or(Refused::MISMATCH)?,
            "retained current module membership",
        )?;
        if std::ptr::eq(current, module) {
            exact += 1;
        }
        if current.path == module.path || current.module == module.module {
            namespace += 1;
        }
    }
    if exact != 1 || namespace != 1 {
        preparation.fail();
        return Err(Refused::MISMATCH);
    }
    Ok(())
}

fn current_module_receipt<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    preparation: &mut crate::cleanup_plan::SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<CurrentModule<'a>, Refused> {
    current_module(module, modules, preparation, anchor)?;
    preparation.try_reserve_array::<CurrentModule<'_>>(
        anchor,
        1,
        "retained current module membership receipt",
    )?;
    Ok(CurrentModule {
        module,
        modules,
        owner: preparation,
    })
}

// Whole independently compiled original owned-Diagnostic caller; no admission use.
#[cfg(test)]
pub(in crate::workspace_graph) fn replay_owned_before(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    form: SelectedReplayReportForm,
) -> Result<bool, SelectedReplayRefusal> {
    let Some(inventory) = module.static_text.as_deref() else {
        return Ok(false);
    };
    if inventory.lexical.is_none() {
        return Ok(false);
    }
    let physical = crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES);
    let acquired = SelectedReplayReport::try_acquire(physical, form)?;
    if inventory
        .lexical
        .as_deref()
        .is_some_and(|inventory| inventory.imported)
    {
        helpers::replay(module, modules, facts, acquired)?;
        return Ok(true);
    }
    let Some(anchor) = module.functions.first() else {
        return Err(acquired.refuse_unprepared());
    };
    let mut preparation = acquired.prepare(anchor)?;
    let (context, lexical) = match members(module, modules, facts, &mut preparation, anchor) {
        Ok(members) => members,
        Err(refused) => return Err(preparation.refuse(refused)),
    };
    let mut session = preparation.into_paid_session(&context, anchor)?;
    let mut found = false;
    for function in lexical {
        // The first member refusal stops the route: no failed Session is
        // validated or finished again, so no legacy retry Diagnostic exists.
        if let Err(error) = session.validate(function) {
            return Err(session.into_failed_preparation().refuse_session(error));
        }
        found = true;
    }
    if !found {
        return Err(session.into_failed_preparation().refuse(Refused::MISMATCH));
    }
    drop(session.finish_paid()?);
    Ok(true)
}
