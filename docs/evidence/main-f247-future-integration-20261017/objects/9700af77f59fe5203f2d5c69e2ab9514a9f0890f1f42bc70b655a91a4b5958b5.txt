//! Defining-source facts for demanded pure scalar helpers, including providers
//! with no literal inventory. Facts alone grant no replay authority.
use super::*;
use crate::hir::{
    OwnershipMode, ResolvedExprKind, ResolvedFunction, ResolvedStatement, ResolvedType,
};
use primitive::body_digest;
mod borrowed_records;
mod capture_directory;
#[cfg(test)]
mod checked_tests;
#[cfg(test)]
mod legacy;
const DEPTH: usize = 514;
const WORK: usize = 1_048_576;
pub(in crate::workspace_graph) struct Inventory {
    pub(in crate::workspace_graph) functions: Vec<Function>,
    revision: [u8; 32],
    pub(in crate::workspace_graph) capture_work: usize,
}
pub(in crate::workspace_graph) struct Function {
    pub(in crate::workspace_graph) id: hir::DeclarationId,
    pub(in crate::workspace_graph) explicit: bool,
    pub(in crate::workspace_graph) name: String,
    pub(in crate::workspace_graph) params: Vec<Parameter>,
    pub(in crate::workspace_graph) result: ResolvedType,
    digest: [u8; 32],
}
pub(in crate::workspace_graph) struct Parameter {
    pub(in crate::workspace_graph) id: hir::ValueId,
    pub(in crate::workspace_graph) name: String,
    pub(in crate::workspace_graph) ty: ResolvedType,
    pub(in crate::workspace_graph) ownership: OwnershipMode,
}
fn debit(work: &mut usize, units: usize) -> Result<(), Vec<Diagnostic>> {
    *work = work.checked_sub(units).ok_or_else(fail)?;
    Ok(())
}
fn pay_source_revision_work(
    path: &str,
    module: &str,
    source: &str,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    let units = source
        .len()
        .checked_add(path.len())
        .and_then(|n| n.checked_add(module.len()))
        .and_then(|n| n.checked_add(24 + b"semaprax.retained-primitive-source.v1\0".len()))
        .ok_or_else(fail)?;
    let Some(remaining) = work.checked_sub(units) else {
        return Ok(false);
    };
    *work = remaining;
    Ok(true)
}
/// Pure still includes checked arithmetic and failing pre/postconditions.
/// No builtin provider, lexical exits, mutable borrow or carrier is inferred.
pub(in crate::workspace_graph) fn pure(
    function: &ResolvedFunction,
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    pure_metered(function, work, &mut |_| Ok(()))
}
pub(in crate::workspace_graph) fn pure_metered(
    function: &ResolvedFunction,
    work: &mut usize,
    charge: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<bool, Vec<Diagnostic>> {
    pure_profile_metered(
        function,
        work,
        charge,
        hir::borrowed_scalar::option_u8::selected_profile_ready(),
        &[],
    )
}
#[cfg(test)]
pub(in crate::workspace_graph) fn pure_shape_metered(
    function: &ResolvedFunction,
    work: &mut usize,
    charge: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<bool, Vec<Diagnostic>> {
    pure_profile_metered(function, work, charge, true, &[])
}
pub(in crate::workspace_graph) fn pure_records_metered(
    function: &ResolvedFunction,
    records: &(impl crate::hir::items::ItemSource<hir::ResolvedTypeDeclaration> + ?Sized),
    work: &mut usize,
    charge: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<bool, Vec<Diagnostic>> {
    legacy_refusal(pure_records_checked(function, records, work, charge))
}
/// A purity refusal that never allocates on the classifier's own behalf. The
/// local work limit carries no payload; a callback refusal is returned exactly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workspace_graph) enum PurityRefusal<E> {
    LocalLimit,
    Charge(E),
}
/// The same closed classifier as `pure_records_metered`, with identical
/// traversal, charges, verdicts and first refusal, but no diagnostic is built.
pub(in crate::workspace_graph) fn pure_records_checked<E>(
    function: &ResolvedFunction,
    records: &(impl crate::hir::items::ItemSource<hir::ResolvedTypeDeclaration> + ?Sized),
    work: &mut usize,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, PurityRefusal<E>> {
    pure_profile_checked(
        function,
        work,
        charge,
        hir::borrowed_scalar::option_u8::selected_profile_ready(),
        records,
    )
}
// Legacy Vec contract: the local limit becomes the original SPX-G410 `fail()`
// and a callback Vec moves out unchanged, so its backing pointer is preserved.
fn legacy_refusal(
    result: Result<bool, PurityRefusal<Vec<Diagnostic>>>,
) -> Result<bool, Vec<Diagnostic>> {
    result.map_err(|refusal| match refusal {
        PurityRefusal::LocalLimit => fail(),
        PurityRefusal::Charge(errors) => errors,
    })
}
fn pure_profile_metered(
    function: &ResolvedFunction,
    work: &mut usize,
    charge: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
    option_shape: bool,
    records: &(impl crate::hir::items::ItemSource<hir::ResolvedTypeDeclaration> + ?Sized),
) -> Result<bool, Vec<Diagnostic>> {
    legacy_refusal(pure_profile_checked(
        function,
        work,
        charge,
        option_shape,
        records,
    ))
}
fn pure_profile_checked<E>(
    function: &ResolvedFunction,
    work: &mut usize,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
    option_shape: bool,
    records: &(impl crate::hir::items::ItemSource<hir::ResolvedTypeDeclaration> + ?Sized),
) -> Result<bool, PurityRefusal<E>> {
    let mut take = |units| -> Result<(), PurityRefusal<E>> {
        let remaining = work.checked_sub(units).ok_or(PurityRefusal::LocalLimit)?;
        charge(units).map_err(PurityRefusal::Charge)?;
        *work = remaining;
        Ok(())
    };
    take(
        function
            .params
            .len()
            .checked_add(1)
            .ok_or(PurityRefusal::LocalLimit)?,
    )?;
    if !function.effects.is_empty()
        || function.yields.is_some()
        || !(hir::borrowed_scalar::signature(function)
            || borrowed_records::signature(function, records, &mut take)?)
    {
        return Ok(false);
    }
    // The charged whitelist rejects every lexical statement and expression;
    // no separate unmetered lexical traversal is needed for this closed shape.
    let mut stack = [None; DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some(hir::lexical_exit::ChildCursor::new(root));
        let mut depth = 1;
        while depth != 0 {
            take(1)?;
            let mut cursor = stack[depth - 1].take().ok_or(PurityRefusal::LocalLimit)?;
            let expr = cursor.expression();
            if !cursor.is_started() {
                if (!option_shape
                    && (hir::borrowed_scalar::option_u8::is_type(&expr.ty)
                        || matches!(expr.kind, ResolvedExprKind::Match { .. })))
                    || !match borrowed_records::expression(function, expr, records, &mut take)? {
                        Some(admitted) => admitted,
                        None => hir::borrowed_scalar::expression(function, expr, &mut take)?,
                    }
                {
                    return Ok(false);
                }
                match &expr.kind {
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::Unary { .. }
                    | ResolvedExprKind::Binary { .. }
                    | ResolvedExprKind::If { .. } => {}
                    // The receiver above has authenticated both bare scalar roots and
                    // exact scalar Field projections of current Borrow records.
                    ResolvedExprKind::Place(_) => {}
                    ResolvedExprKind::Project { base, .. }
                        if matches!(base.ty, ResolvedType::Nominal { .. }) => {}
                    ResolvedExprKind::BorrowPlace { .. } => {}
                    ResolvedExprKind::Match { scrutinee, .. }
                        if option_shape
                            && hir::borrowed_scalar::option_u8::is_type(&scrutinee.ty) => {}
                    ResolvedExprKind::Call {
                        instance,
                        type_arguments,
                        ..
                    } if instance.is_none() && type_arguments.is_empty() => {}
                    ResolvedExprKind::Block { statements, .. } => {
                        take(statements.len())?;
                        for statement in statements {
                            match statement {
                                ResolvedStatement::Let { binding, .. }
                                | ResolvedStatement::Assign {
                                    binding,
                                    field: None,
                                    ..
                                } if binding.ownership == OwnershipMode::Value
                                    && (hir::is_scalar_resolved_type(&binding.ty)
                                        || (option_shape
                                            && hir::borrowed_scalar::option_u8::is_type(
                                                &binding.ty,
                                            ))) => {}
                                ResolvedStatement::While { .. } => {}
                                _ => return Ok(false),
                            }
                        }
                    }
                    _ => return Ok(false),
                }
            }
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Ok(false);
                }
                stack[depth - 1] = Some(cursor);
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(true)
}
pub(in crate::workspace_graph) fn capture(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    programs: &[Program],
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    sources: Option<&[WorkspaceSource]>,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    let Some(sources) = sources else {
        return Ok(None);
    };
    let mut work = WORK;
    // Demand is merely selection; each defining source is independently held.
    let mut demanded = false;
    for current in programs {
        debit(&mut work, 1)?;
        for import in &current.module_uses {
            debit(
                &mut work,
                import
                    .target_module
                    .len()
                    .checked_add(program.module.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
            if import.kind == crate::ast::ModuleUseKind::Function
                && import.target_module == program.module
            {
                demanded = true;
            }
        }
    }
    if !demanded {
        return Ok(None);
    }

    let mut selected = None;
    for source in sources {
        debit(
            &mut work,
            source
                .path
                .len()
                .checked_add(program.path.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(fail)?,
        )?;
        if source.path == program.path && selected.replace(source).is_some() {
            return Err(fail());
        }
    }
    let source = selected.ok_or_else(fail)?;
    // Capture is optional; a source that cannot fit the hash work retains no
    // proof. A selected receiver still requires and independently replays the
    // complete inventory. Decline before allocating scratch or hashing bytes.
    if !pay_source_revision_work(&program.path, &program.module, &source.source, &mut work)? {
        return Ok(None);
    }
    reserve_builder_structure(
        body_digest::scratch_bytes()
            .checked_add(
                DEPTH
                    .checked_mul(std::mem::size_of::<
                        Option<hir::lexical_exit::ChildCursor<'_>>,
                    >())
                    .ok_or_else(capacity)?,
            )
            .ok_or_else(capacity)?,
    )?;
    let revision = body_digest::source(&program.path, &program.module, &source.source);
    let candidates = capture_directory::Directory::new(program, resolved, &mut work)?;
    let count = candidates.count();
    if count == 0 {
        return Ok(None);
    }
    reserve_builder_structure(
        std::mem::size_of::<Inventory>()
            .checked_add(
                count
                    .checked_mul(std::mem::size_of::<Function>())
                    .ok_or_else(capacity)?,
            )
            .ok_or_else(capacity)?,
    )?;
    let mut functions = Vec::with_capacity(count);
    if functions.capacity() != count {
        return Err(capacity());
    }
    // One borrowed sorted view replaces a full authored-map scan per selected
    // function; a hit grants nothing until every unchanged guard below passes.
    let origins = capture_directory::Authored::new(authored, &mut work)?;
    for (ordinal, source) in program.functions.iter().enumerate() {
        debit(&mut work, 1)?;
        let Some(function) = candidates.selected(ordinal) else {
            continue;
        };
        let origin = origins.find(&source.stable_id, &mut work)?;
        debit(
            &mut work,
            origin
                .module
                .len()
                .checked_add(program.module.len())
                .and_then(|n| n.checked_add(origin.path.len()))
                .and_then(|n| n.checked_add(program.path.len()))
                .and_then(|n| n.checked_add(source.name.len()))
                .and_then(|n| n.checked_add(function.name.len()))
                .and_then(|n| n.checked_add(1))
                .ok_or_else(fail)?,
        )?;
        for (source, current) in source.params.iter().zip(&function.params) {
            debit(
                &mut work,
                source
                    .name
                    .len()
                    .checked_add(current.name.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
        }
        if origin.module != program.module
            || origin.path != program.path
            || !origin.explicit
            || !origin.function.is_some_and(|f| std::ptr::eq(f, source))
            || function.name != source.name
            || source.params.len() != function.params.len()
            || !primitive::records::source_type_matches(
                program,
                &source.return_type,
                &function.return_type,
                &mut work,
            )?
        {
            return Err(fail());
        }
        for (source, current) in source.params.iter().zip(&function.params) {
            if source.name != current.name
                || match source.mode {
                    crate::ast::ParamMode::Value => current.ownership != OwnershipMode::Value,
                    crate::ast::ParamMode::Borrow => current.ownership != OwnershipMode::Borrow,
                    _ => true,
                }
                || !primitive::records::source_type_matches(
                    program,
                    &source.ty,
                    &current.ty,
                    &mut work,
                )?
            {
                return Err(fail());
            }
        }
        reserve_builder_structure(
            source
                .stable_id
                .len()
                .checked_add(source.name.len())
                .and_then(|n| {
                    n.checked_add(
                        function
                            .params
                            .len()
                            .checked_mul(std::mem::size_of::<Parameter>())?,
                    )
                })
                .ok_or_else(capacity)?,
        )?;
        debit(
            &mut work,
            source
                .stable_id
                .len()
                .checked_add(source.name.len())
                .and_then(|n| n.checked_add(function.params.len()))
                .ok_or_else(fail)?,
        )?;
        let mut params = Vec::with_capacity(function.params.len());
        if params.capacity() != function.params.len() {
            return Err(capacity());
        }
        for param in &function.params {
            debit(&mut work, param.name.len().checked_add(1).ok_or_else(fail)?)?;
            reserve_builder_structure(
                param
                    .id
                    .as_str()
                    .len()
                    .checked_add(param.name.len())
                    .ok_or_else(capacity)?,
            )?;
            debit(
                &mut work,
                primitive::records::type_capacity_bytes(&param.ty)
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
            params.push(Parameter {
                id: param.id.clone(),
                name: param.name.clone(),
                ty: primitive::records::clone_type(&param.ty)?,
                ownership: param.ownership,
            });
        }
        let digest = body_digest::typed_attempt_metered(
            function,
            &revision,
            Some(&mut |units| debit(&mut work, units).ok()),
        )
        .0
        .ok_or_else(fail)?;
        functions.push(Function {
            id: function.id.clone(),
            explicit: source.explicit_id,
            name: function.name.clone(),
            params,
            result: function.return_type.clone(),
            digest,
        });
    }
    Ok(Some(Box::new(Inventory {
        functions,
        revision,
        capture_work: WORK.checked_sub(work).ok_or_else(fail)?,
    })))
}
pub(in crate::workspace_graph) fn matches_metered(
    inventory: &Inventory,
    fact: &Function,
    function: &ResolvedFunction,
    debit: &mut dyn FnMut(usize) -> Option<()>,
) -> bool {
    body_digest::typed_attempt_metered(function, &inventory.revision, Some(debit)).0
        == Some(fact.digest)
}
