//! Rare source-derived signatures for module-local primitive lexical replay.
//! These are captured from the authoritative AST, never from a cleanup schema.
use super::*;
use crate::ast::{ParamMode, Type};
pub(super) mod body_digest;
mod function_matching;
pub(in crate::workspace_graph) mod records;

pub(in crate::workspace_graph) struct Inventory {
    pub(in crate::workspace_graph) functions: Vec<FunctionFact>,
    pub(in crate::workspace_graph) records: Option<Box<records::Records>>,
    source_revision: [u8; 32],
    pub(in crate::workspace_graph) capture_hash_work: usize,
    pub(in crate::workspace_graph) imported: bool,
}
pub(in crate::workspace_graph) struct FunctionFact {
    pub(in crate::workspace_graph) id: hir::DeclarationId,
    pub(in crate::workspace_graph) explicit: bool,
    pub(in crate::workspace_graph) name: String,
    pub(in crate::workspace_graph) params: Vec<ParameterFact>,
    pub(in crate::workspace_graph) result: hir::ResolvedType,
    body: [u8; 32],
}
pub(in crate::workspace_graph) struct ParameterFact {
    pub(in crate::workspace_graph) name: String,
    pub(in crate::workspace_graph) ownership: hir::OwnershipMode,
    pub(in crate::workspace_graph) ty: hir::ResolvedType,
}

pub(super) fn ty(ty: &Type) -> Option<hir::ResolvedType> {
    use hir::ResolvedType as R;
    Some(match ty {
        Type::I64 => R::I64,
        Type::I32 => R::I32,
        Type::Char => R::Char,
        Type::U8 => R::U8,
        Type::Usize => R::Usize,
        Type::ArrayU8(size) => R::ArrayU8(*size),
        Type::F32 => R::F32,
        Type::F64 => R::F64,
        Type::Bool => R::Bool,
        Type::String => R::String,
        Type::Bytes => R::Bytes,
        Type::Str => R::Str,
        Type::SliceU8 => R::SliceU8,
        Type::StringMap => R::StringMap,
        Type::OnceFunction => R::OnceFunction,
        Type::OnceFunctionI64 => R::OnceFunctionI64,
        Type::OnceFunctionI64Pair => R::OnceFunctionI64Pair,
        Type::MutFunctionI64 => R::MutFunctionI64,
        Type::Never | Type::Named { .. } | Type::Function { .. } => return None,
    })
}
fn ownership(param: &crate::ast::Param, ty: &hir::ResolvedType) -> Option<hir::OwnershipMode> {
    use hir::{OwnershipMode as O, ResolvedType as R};
    if param.mode == ParamMode::Shared {
        return None;
    }
    Some(
        if param.mode == ParamMode::Borrow && matches!(ty, R::Bytes | R::StringMap) {
            O::Borrow
        } else if matches!(
            ty,
            R::Bytes
                | R::String
                | R::StringMap
                | R::OnceFunction
                | R::OnceFunctionI64
                | R::OnceFunctionI64Pair
        ) {
            O::Own
        } else if matches!(ty, R::Str | R::SliceU8) {
            if param.mode != ParamMode::Borrow {
                return None;
            }
            O::Borrow
        } else {
            match param.mode {
                ParamMode::Value => O::Value,
                ParamMode::Own => O::Own,
                ParamMode::Borrow => O::Borrow,
                ParamMode::Shared => return None,
            }
        },
    )
}

/// Unsupported shapes keep the retained lexical route closed. They do not
/// change ordinary source checking, retention or direct cleanup replay.
pub(super) fn capture(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    authenticated_sources: Option<&[WorkspaceSource]>,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    capture_with_matching::<false>(program, resolved, authored, authenticated_sources)
}
fn capture_with_matching<const LEGACY: bool>(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    authenticated_sources: Option<&[WorkspaceSource]>,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    if program.module_uses.iter().any(|u| {
        !matches!(
            u.kind,
            crate::ast::ModuleUseKind::Function | crate::ast::ModuleUseKind::Type
        )
    }) || !program.agents.is_empty()
        || !program.interfaces.is_empty()
        || !program.protocols.is_empty()
        || !program.implementations.is_empty()
        || !program.permits.is_empty()
    {
        return Ok(None);
    }
    let mut work = WORK;
    if !records::preflight(program, resolved, &mut work)? {
        return Ok(None);
    }
    for function in &program.functions {
        work = work
            .checked_sub(function.params.len().checked_add(1).ok_or_else(fail)?)
            .ok_or_else(fail)?;
        // Named-type lookups scan the bounded local declaration inventory.
        work = work
            .checked_sub(
                function
                    .params
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(program.types.len()))
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        if !function.type_parameters.is_empty()
            || !function.effects.is_empty()
            || function.yields.is_some()
            || function.follows.is_some()
            || !records::source_type_admitted(program, &function.return_type, &mut work)?
        {
            return Ok(None);
        }
        for param in &function.params {
            if !records::source_type_admitted(program, &param.ty, &mut work)? {
                return Ok(None);
            }
            if !matches!(param.ty, Type::Named { .. })
                && ownership(param, &ty(&param.ty).expect("primitive")).is_none()
            {
                return Ok(None);
            }
        }
    }
    // This factory is reachable only while the checked authoritative AST and
    // its checked resolved program are both held. Freeze a complete semantic
    // typed-body guard; never accept an expected digest from a candidate.
    let Some(sources) = authenticated_sources else {
        return Ok(None);
    };
    work.checked_sub(sources.len()).ok_or_else(fail)?;
    let mut current_sources = sources.iter().filter(|source| source.path == program.path);
    let source = current_sources.next().ok_or_else(fail)?;
    if current_sources.next().is_some() {
        return Err(fail());
    }
    reserve_builder_structure(body_digest::scratch_bytes())?;
    let mut capture_hash_work = source
        .source
        .len()
        .checked_add(program.path.len())
        .and_then(|n| n.checked_add(program.module.len()))
        .and_then(|n| n.checked_add(24 + b"semaprax.retained-primitive-source.v1\0".len()))
        .ok_or_else(fail)?;
    let source_revision = body_digest::source(&program.path, &program.module, &source.source);
    let mut body_work = WORK;
    // One held borrowed index serves both full source/signature checks and
    // final fact construction. It is lookup scratch, never namespace authority.
    let matching = if LEGACY {
        None
    } else {
        Some(function_matching::Index::new(
            &resolved.functions,
            &mut body_work,
        )?)
    };
    for source in &program.functions {
        let origin = authored.get(source.stable_id.as_str()).ok_or_else(fail)?;
        if origin.path != program.path
            || origin.module != program.module
            || origin.function.is_none()
        {
            return Err(fail());
        }
        let function = function_matching::lookup(
            matching.as_ref(),
            &resolved.functions,
            &source.stable_id,
            &mut body_work,
        )?;
        work = work
            .checked_sub(
                source
                    .params
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(program.types.len()))
                    .ok_or_else(fail)?,
            )
            .ok_or_else(fail)?;
        if !records::source_type_matches(
            program,
            &source.return_type,
            &function.return_type,
            &mut body_work,
        )? || source.params.len() != function.params.len()
        {
            return Err(fail());
        }
        for (source, current) in source.params.iter().zip(&function.params) {
            if !records::source_type_matches(program, &source.ty, &current.ty, &mut body_work)?
                || if matches!(source.ty, Type::Named { .. }) {
                    match source.mode {
                        ParamMode::Own => current.ownership != hir::OwnershipMode::Own,
                        ParamMode::Borrow => current.ownership != hir::OwnershipMode::Borrow,
                        ParamMode::Value => current.ownership != hir::OwnershipMode::Value,
                        ParamMode::Shared => true,
                    }
                } else {
                    ownership(source, &current.ty) != Some(current.ownership)
                }
            {
                return Err(fail());
            }
        }
        if body_digest::typed_static_attempt_metered(
            function,
            &source_revision,
            Some(&mut |units| {
                capture_hash_work = capture_hash_work.checked_add(units)?;
                Some(())
            }),
        )
        .0
        .is_none()
        {
            return Ok(None);
        }
    }
    reserve_builder_structure(
        std::mem::size_of::<Inventory>()
            .checked_add(
                program
                    .functions
                    .len()
                    .checked_mul(std::mem::size_of::<FunctionFact>())
                    .ok_or_else(capacity)?,
            )
            .ok_or_else(capacity)?,
    )?;
    let records = records::capture(program, resolved, &mut work)?;
    let mut functions = Vec::with_capacity(program.functions.len());
    for function in &program.functions {
        reserve_builder_structure(
            function
                .params
                .len()
                .checked_mul(std::mem::size_of::<ParameterFact>())
                .ok_or_else(capacity)?,
        )?;
        let mut params = Vec::with_capacity(function.params.len());
        let current = function_matching::lookup(
            matching.as_ref(),
            &resolved.functions,
            &function.stable_id,
            &mut body_work,
        )?;
        for (param, current) in function.params.iter().zip(&current.params) {
            let resolved = records::clone_type(&current.ty)?;
            params.push(ParameterFact {
                name: crate::bounded_output::budgeted_clone(&param.name),
                ownership: current.ownership,
                ty: resolved,
            });
        }
        reserve_builder_structure(function.stable_id.len())?;
        functions.push(FunctionFact {
            id: hir::DeclarationId::new(function.stable_id.as_str()),
            explicit: function.explicit_id,
            name: crate::bounded_output::budgeted_clone(&function.name),
            params,
            result: records::clone_type(&current.return_type)?,
            body: body_digest::typed_static_attempt_metered(
                current,
                &source_revision,
                Some(&mut |units| {
                    capture_hash_work = capture_hash_work.checked_add(units)?;
                    Some(())
                }),
            )
            .0
            .ok_or_else(fail)?,
        });
    }
    if crate::bounded_output::active_refused() {
        return Err(capacity());
    }
    capture_hash_work = capture_hash_work
        .checked_add(WORK.checked_sub(work).ok_or_else(fail)?)
        .and_then(|n| n.checked_add(WORK.checked_sub(body_work)?))
        .ok_or_else(fail)?;
    let inventory = Inventory {
        functions,
        records,
        source_revision,
        capture_hash_work,
        imported: !program.module_uses.is_empty(),
    };
    // Validate the complete retained backing arithmetic after the independently
    // reserved exact-capacity carriers have been constructed.
    inventory.owned_capacity_bytes().ok_or_else(capacity)?;
    Ok(Some(Box::new(inventory)))
}
impl Inventory {
    pub(in crate::workspace_graph) fn owned_capacity_bytes(&self) -> Option<usize> {
        self.functions
            .iter()
            .try_fold(
                self.functions
                    .capacity()
                    .checked_mul(std::mem::size_of::<FunctionFact>())?,
                |total, function| {
                    let params = function.params.iter().try_fold(
                        function
                            .params
                            .capacity()
                            .checked_mul(std::mem::size_of::<ParameterFact>())?,
                        |sum, param| {
                            sum.checked_add(param.name.capacity())?
                                .checked_add(records::type_capacity_bytes(&param.ty)?)
                        },
                    )?;
                    total
                        .checked_add(function.id.as_str().len())?
                        .checked_add(function.name.capacity())?
                        .checked_add(params)?
                        .checked_add(records::type_capacity_bytes(&function.result)?)
                },
            )?
            .checked_add(
                self.records
                    .as_ref()
                    .map_or(Some(0), |records| records.owned_capacity_bytes())?,
            )
    }
}

pub(in crate::workspace_graph) fn body_matches(
    inventory: &Inventory,
    source: &FunctionFact,
    function: &hir::ResolvedFunction,
) -> bool {
    body_digest::typed_static_attempt_metered(function, &inventory.source_revision, None).0
        == Some(source.body)
}
pub(in crate::workspace_graph) fn body_matches_metered(
    inventory: &Inventory,
    source: &FunctionFact,
    function: &hir::ResolvedFunction,
    debit: &mut dyn FnMut(usize) -> Option<()>,
) -> bool {
    body_digest::typed_static_attempt_metered(function, &inventory.source_revision, Some(debit)).0
        == Some(source.body)
}
pub(in crate::workspace_graph) fn digest_scratch_bytes() -> usize {
    body_digest::scratch_bytes()
}
