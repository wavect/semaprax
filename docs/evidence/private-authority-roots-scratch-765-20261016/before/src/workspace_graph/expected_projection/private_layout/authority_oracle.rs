//! Saved pre-#765 private authority: the complete owned top-level
//! function/type map rebuilt for each caller. Test-only differential oracle for
//! roots, verdicts and remaining work; it never consults the borrowed view or
//! the held authored index and reports no authority-index request.
use super::{
    debit, explicit_only, possible, refusal, reserve_builder_structure, visit, Absence,
    AuthoredDeclaration, BTreeMap, Diagnostic, Frame, ModuleUseKind, Program, DEPTH, WORK,
};

pub(super) type Saved<'a> = (Vec<(&'a str, &'a str)>, usize);

pub(super) fn saved_roots<'a>(
    caller: &Program,
    programs: &'a [Program],
) -> Result<Saved<'a>, Vec<Diagnostic>> {
    let mut remaining = WORK;
    debit(&mut remaining, caller.module_uses.len())?;
    if !caller
        .module_uses
        .iter()
        .any(|u| matches!(u.kind, ModuleUseKind::Type | ModuleUseKind::Function))
    {
        return Ok((Vec::new(), remaining));
    }
    if !possible(caller, programs, &mut remaining)? {
        return Ok((Vec::new(), remaining));
    }
    debit(&mut remaining, programs.len())?;
    let count = programs
        .iter()
        .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
        .ok_or_else(refusal)?;
    let function_count = programs
        .iter()
        .try_fold(0usize, |n, p| n.checked_add(p.functions.len()))
        .ok_or_else(refusal)?;
    let indexed = count.checked_add(function_count).ok_or_else(refusal)?;
    debit(&mut remaining, indexed)?;
    let node = crate::cleanup_plan::replay_btree_node_upper::<&str, AuthoredDeclaration<'_>>()
        .ok_or_else(refusal)?;
    reserve_builder_structure(indexed.checked_mul(node).ok_or_else(refusal)?)?;
    let mut authored = BTreeMap::new();
    for p in programs {
        for f in &p.functions {
            if authored
                .insert(
                    f.stable_id.as_str(),
                    AuthoredDeclaration {
                        path: &p.path,
                        module: &p.module,
                        explicit: f.explicit_id,
                        kind: crate::workspace_graph::AuthoredKind::Function,
                        function: Some(f),
                        ty: None,
                    },
                )
                .is_some()
            {
                return Err(refusal());
            }
        }
        for t in &p.types {
            if authored
                .insert(
                    t.stable_id.as_str(),
                    AuthoredDeclaration {
                        path: &p.path,
                        module: &p.module,
                        explicit: t.explicit_id,
                        kind: crate::workspace_graph::AuthoredKind::Type,
                        function: None,
                        ty: Some(t),
                    },
                )
                .is_some()
            {
                return Err(refusal());
            }
        }
    }
    if explicit_only(caller, &authored, programs, &mut remaining)? != Absence::Possible {
        return Ok((Vec::new(), remaining));
    }
    reserve_builder_structure(
        DEPTH
            .checked_mul(std::mem::size_of::<Frame<'_>>())
            .ok_or_else(refusal)?,
    )?;
    let mut frames = Vec::new();
    frames.try_reserve_exact(DEPTH).map_err(|_| refusal())?;
    if frames.capacity() != DEPTH {
        return Err(refusal());
    }
    reserve_builder_structure(
        count
            .checked_mul(std::mem::size_of::<(&str, &str)>())
            .ok_or_else(refusal)?,
    )?;
    let mut roots: Vec<(&str, &str)> = Vec::new();
    roots.try_reserve_exact(count).map_err(|_| refusal())?;
    if roots.capacity() != count {
        return Err(refusal());
    }
    visit(
        caller,
        &authored,
        programs,
        &mut frames,
        &mut remaining,
        |module, declaration, work| {
            for (m, id) in &roots {
                debit(
                    work,
                    module
                        .len()
                        .checked_add(m.len())
                        .and_then(|n| n.checked_add(id.len()))
                        .and_then(|n| n.checked_add(declaration.stable_id.len()))
                        .ok_or_else(refusal)?,
                )?;
                if *m == module && *id == declaration.stable_id {
                    return Ok(());
                }
            }
            if roots.len() == count {
                return Err(refusal());
            }
            roots.push((module, declaration.stable_id.as_str()));
            Ok(())
        },
    )?;
    Ok((roots, remaining))
}
