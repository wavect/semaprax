// Independent allocating capture oracle retained verbatim for tests only.
use super::{
    body_digest, capacity, debit, fail, hir, primitive, pure, reserve_builder_structure,
    AuthoredDeclaration, BTreeMap, Diagnostic, Function, Inventory, OwnershipMode, Parameter,
    Program, WorkspaceSource, DEPTH, WORK,
};
pub(super) fn capture(
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
    debit(
        &mut work,
        source
            .source
            .len()
            .checked_add(program.path.len())
            .and_then(|n| n.checked_add(program.module.len()))
            .and_then(|n| n.checked_add(24 + b"semaprax.retained-primitive-source.v1\0".len()))
            .ok_or_else(fail)?,
    )?;
    let revision = body_digest::source(&program.path, &program.module, &source.source);
    let mut count = 0usize;
    for source in &program.functions {
        debit(
            &mut work,
            source.params.len().checked_add(1).ok_or_else(fail)?,
        )?;
        if !source.explicit_id
            || !source.type_parameters.is_empty()
            || source.follows.is_some()
            || source.params.iter().any(|p| {
                !matches!(
                    p.mode,
                    crate::ast::ParamMode::Value | crate::ast::ParamMode::Borrow
                )
            })
        {
            continue;
        }
        let mut current = None;
        for function in &resolved.functions {
            debit(
                &mut work,
                function
                    .id
                    .as_str()
                    .len()
                    .checked_add(source.stable_id.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
            if function.id.as_str() == source.stable_id && current.replace(function).is_some() {
                return Err(fail());
            }
        }
        let function = current.ok_or_else(fail)?;
        if source.explicit_id
            && source.type_parameters.is_empty()
            && source.follows.is_none()
            && source.params.iter().all(|p| {
                matches!(
                    p.mode,
                    crate::ast::ParamMode::Value | crate::ast::ParamMode::Borrow
                )
            })
            && pure(function, &mut work)?
        {
            count = count.checked_add(1).ok_or_else(capacity)?;
        }
    }
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
    for source in &program.functions {
        debit(
            &mut work,
            source.params.len().checked_add(1).ok_or_else(fail)?,
        )?;
        if !source.explicit_id
            || !source.type_parameters.is_empty()
            || source.follows.is_some()
            || source.params.iter().any(|p| {
                !matches!(
                    p.mode,
                    crate::ast::ParamMode::Value | crate::ast::ParamMode::Borrow
                )
            })
        {
            continue;
        }
        let mut current = None;
        for function in &resolved.functions {
            debit(
                &mut work,
                function
                    .id
                    .as_str()
                    .len()
                    .checked_add(source.stable_id.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
            if function.id.as_str() == source.stable_id && current.replace(function).is_some() {
                return Err(fail());
            }
        }
        let function = current.ok_or_else(fail)?;
        if !source.explicit_id
            || !source.type_parameters.is_empty()
            || source.follows.is_some()
            || source.params.iter().any(|p| {
                !matches!(
                    p.mode,
                    crate::ast::ParamMode::Value | crate::ast::ParamMode::Borrow
                )
            })
            || !pure(function, &mut work)?
        {
            continue;
        }
        let mut selected = None;
        for (id, origin) in authored {
            debit(
                &mut work,
                id.len()
                    .checked_add(source.stable_id.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
            if *id == source.stable_id {
                selected = Some(origin);
                break;
            }
        }
        let origin = selected.ok_or_else(fail)?;
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
            || primitive::ty(&source.return_type).as_ref() != Some(&function.return_type)
            || source
                .params
                .iter()
                .zip(&function.params)
                .any(|(source, current)| {
                    source.name != current.name
                        || match source.mode {
                            crate::ast::ParamMode::Value => {
                                current.ownership != OwnershipMode::Value
                            }
                            crate::ast::ParamMode::Borrow => {
                                current.ownership != OwnershipMode::Borrow
                            }
                            _ => true,
                        }
                        || primitive::ty(&source.ty).as_ref() != Some(&current.ty)
                })
        {
            return Err(fail());
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
            params.push(Parameter {
                id: param.id.clone(),
                name: param.name.clone(),
                ty: param.ty.clone(),
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
