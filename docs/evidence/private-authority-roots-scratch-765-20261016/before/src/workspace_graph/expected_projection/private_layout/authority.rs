//! Current-source private type authority, independent of synthetic aliases.
use super::{
    debit, explicit_only, graph_error, private_without_work, refusal, reserve_builder_structure,
    visit, Absence, AuthoredDeclaration, BTreeMap, Diagnostic, Frame, ModuleUseKind, Program,
    DEPTH, WORK,
};
use std::cell::Cell;
#[path = "authority_view.rs"]
pub(super) mod view;
use view::{Lookup, TopLevel};
#[path = "authority_invocation.rs"]
pub(in crate::workspace_graph) mod invocation;

pub(in crate::workspace_graph) struct PrivateTypes<'a> {
    roots: Vec<(&'a str, &'a str)>,
    remaining: Cell<usize>,
}
impl<'a> PrivateTypes<'a> {
    /// `authored` is the caller-held `index_authored(programs)`. It is reused
    /// only through the proven borrowed view; otherwise the old owned map is
    /// rebuilt exactly as before.
    pub(in crate::workspace_graph) fn derive(
        caller: &Program,
        programs: &'a [Program],
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut remaining = WORK;
        debit(&mut remaining, caller.module_uses.len())?;
        if !caller
            .module_uses
            .iter()
            .any(|u| matches!(u.kind, ModuleUseKind::Type | ModuleUseKind::Function))
        {
            return Ok(Self {
                roots: Vec::new(),
                remaining: Cell::new(remaining),
            });
        }
        if !possible(caller, programs, &mut remaining)? {
            return Ok(Self {
                roots: Vec::new(),
                remaining: Cell::new(remaining),
            });
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
        match TopLevel::prove(caller, programs, authored, count, indexed, &mut remaining)? {
            Some(borrowed) => {
                #[cfg(test)]
                view::record_legacy_calculated(indexed);
                Self::finish(caller, programs, &borrowed, count, remaining)
            }
            None => Self::legacy(caller, programs, count, indexed, remaining),
        }
    }
    // The old owned top-level map, retained only where a class method shares
    // the function kind in the held index.
    fn legacy(
        caller: &Program,
        programs: &'a [Program],
        count: usize,
        indexed: usize,
        remaining: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        let node = crate::cleanup_plan::replay_btree_node_upper::<&str, AuthoredDeclaration<'_>>()
            .ok_or_else(refusal)?;
        let index_bytes = indexed.checked_mul(node).ok_or_else(refusal)?;
        #[cfg(test)]
        crate::workspace_graph::forecast_observation::authority_index(
            &caller.path,
            indexed,
            node,
            index_bytes,
            crate::bounded_output::active_remaining(),
        );
        reserve_builder_structure(index_bytes)?;
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
        Self::finish(caller, programs, &authored, count, remaining)
    }
    fn finish<A: Lookup<'a>>(
        caller: &Program,
        programs: &'a [Program],
        authored: &A,
        count: usize,
        mut remaining: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        if explicit_only(caller, authored, programs, &mut remaining)? != Absence::Possible {
            return Ok(Self {
                roots: Vec::new(),
                remaining: Cell::new(remaining),
            });
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
        Self::finish_ready(caller, programs, authored, count, remaining, &mut frames)
    }
    fn finish_ready<A: Lookup<'a>>(
        caller: &Program,
        programs: &'a [Program],
        authored: &A,
        count: usize,
        mut remaining: usize,
        frames: &mut Vec<Frame<'a>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        // A type may be reached by several authenticated roots. Retain each
        // borrowed identity once; no copied name, declaration, or layout fact.
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
            authored,
            programs,
            frames,
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
        Ok(Self {
            roots,
            remaining: Cell::new(remaining),
        })
    }
    pub(in crate::workspace_graph) fn contains(
        &self,
        id: &str,
        module: Option<&str>,
    ) -> Result<bool, Vec<Diagnostic>> {
        let mut work = self.remaining.get();
        let result = (|| {
            for (provider, root) in &self.roots {
                debit(
                    &mut work,
                    root.len()
                        .checked_add(id.len())
                        .and_then(|n| n.checked_add(provider.len()))
                        .and_then(|n| n.checked_add(module.map_or(0, str::len)))
                        .ok_or_else(refusal)?,
                )?;
                if *root == id && Some(*provider) == module {
                    return Ok(true);
                }
            }
            Ok(false)
        })();
        self.remaining.set(work);
        result
    }
    pub(in crate::workspace_graph) fn validate_signatures(
        &self,
        mut matches: impl FnMut(&str, &str) -> bool,
    ) -> Result<(), Vec<Diagnostic>> {
        for (module, id) in &self.roots {
            let mut work = self.remaining.get();
            let result = debit(
                &mut work,
                module.len().checked_add(id.len()).ok_or_else(refusal)?,
            );
            self.remaining.set(work);
            result?;
            if !matches(module, id) {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "private workspace type signature disagrees with current provider HIR authority",
                )]);
            }
        }
        Ok(())
    }
}

// Complete borrowed absence discovery precedes any directory or frame backing.
// Exhaustion produces no inferred authority; ordinary import checks still run.
fn possible(
    caller: &Program,
    programs: &[Program],
    work: &mut usize,
) -> Result<bool, Vec<Diagnostic>> {
    for import in caller
        .module_uses
        .iter()
        .filter(|u| matches!(u.kind, ModuleUseKind::Type | ModuleUseKind::Function))
    {
        if work.checked_sub(programs.len()).is_none() {
            *work = 0;
            return Ok(false);
        }
        debit(work, programs.len())?;
        let provider = programs
            .iter()
            .find(|p| p.module == import.target_module)
            .ok_or_else(refusal)?;
        let scan = provider
            .types
            .len()
            .checked_add(provider.functions.len())
            .ok_or_else(refusal)?;
        if work.checked_sub(scan).is_none() {
            *work = 0;
            return Ok(false);
        }
        debit(work, scan)?;
        let explicit = match import.kind {
            ModuleUseKind::Type => provider
                .types
                .iter()
                .find(|t| t.stable_id == import.persistent_id)
                .map(|t| t.explicit_id),
            ModuleUseKind::Function => provider
                .functions
                .iter()
                .find(|f| f.stable_id == import.persistent_id)
                .map(|f| f.explicit_id),
            _ => None,
        };
        if explicit != Some(true) {
            return Err(refusal());
        }
        for ty in &provider.types {
            if work.checked_sub(caller.module_uses.len()).is_none() {
                *work = 0;
                return Ok(false);
            }
            debit(work, caller.module_uses.len())?;
            if private_without_work(caller, &provider.module, &ty.stable_id) {
                return Ok(true);
            }
        }
        if work.checked_sub(provider.module_uses.len()).is_none() {
            *work = 0;
            return Ok(false);
        }
        debit(work, provider.module_uses.len())?;
        for u in provider
            .module_uses
            .iter()
            .filter(|u| u.kind == ModuleUseKind::Type)
        {
            if work.checked_sub(caller.module_uses.len()).is_none() {
                *work = 0;
                return Ok(false);
            }
            debit(work, caller.module_uses.len())?;
            if !caller
                .module_uses
                .iter()
                .any(|c| c.kind == ModuleUseKind::Type && c.persistent_id == u.persistent_id)
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
#[path = "authority_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "authority_oracle.rs"]
mod oracle;

#[cfg(test)]
#[path = "authority_view_tests.rs"]
mod view_tests;
