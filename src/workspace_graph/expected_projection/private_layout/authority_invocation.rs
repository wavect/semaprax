//! One invocation's exact top-level proof and reusable borrowed DFS backing.
use super::{
    debit, explicit_only, possible, refusal, reserve_builder_structure, Absence,
    AuthoredDeclaration, BTreeMap, Cell, Diagnostic, Frame, Lookup, ModuleUseKind, PrivateTypes,
    Program, TopLevel, DEPTH, WORK,
};
use std::cell::RefCell;

enum Held<'a> {
    Borrowed(TopLevel<'a>),
    Legacy(BTreeMap<&'a str, AuthoredDeclaration<'a>>),
}
impl<'a> Lookup<'a> for Held<'a> {
    fn get(&self, id: &str) -> Option<&AuthoredDeclaration<'a>> {
        match self {
            Self::Borrowed(v) => v.get(id),
            Self::Legacy(v) => v.get(id),
        }
    }
}
struct State<'a> {
    held: Option<Held<'a>>,
    count: usize,
    frames: Vec<Frame<'a>>,
    failed: bool,
}
/// No source or HIR verdict survives this borrowed invocation.
pub(in crate::workspace_graph) struct Invocation<'a> {
    programs: &'a [Program],
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    state: RefCell<State<'a>>,
}
impl<'a> Invocation<'a> {
    pub(in crate::workspace_graph) fn new(
        programs: &'a [Program],
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    ) -> Self {
        Self {
            programs,
            authored,
            state: RefCell::new(State {
                held: None,
                count: 0,
                frames: Vec::new(),
                failed: false,
            }),
        }
    }
    #[cfg(test)]
    pub(in crate::workspace_graph) fn derive_bound(
        &self,
        caller: &Program,
        programs: &[Program],
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    ) -> Result<PrivateTypes<'a>, Vec<Diagnostic>> {
        if !std::ptr::eq(self.programs, programs) || !std::ptr::eq(self.authored, authored) {
            self.state.try_borrow_mut().map_err(|_| refusal())?.failed = true;
            return Err(refusal());
        }
        self.derive(caller)
    }
    #[cfg(test)]
    pub(in crate::workspace_graph) fn derive(
        &self,
        caller: &Program,
    ) -> Result<PrivateTypes<'a>, Vec<Diagnostic>> {
        let mut state = self.state.try_borrow_mut().map_err(|_| refusal())?;
        if state.failed {
            return Err(refusal());
        }
        // Numeric discovery is followed by exact current pointer membership.
        let stride = std::mem::size_of::<Program>();
        let member = (caller as *const Program as usize)
            .checked_sub(self.programs.as_ptr() as usize)
            .filter(|offset| offset % stride == 0)
            .and_then(|offset| self.programs.get(offset / stride));
        if !member.is_some_and(|p| std::ptr::eq(p, caller)) {
            state.failed = true;
            return Err(refusal());
        }
        let result = self.derive_current(caller, &mut state);
        // A failed traversal must not retain any borrowed active DFS frames.
        state.frames.clear();
        if result.is_err() {
            state.failed = true;
        }
        result
    }
    #[cfg(test)]
    fn derive_current(
        &self,
        caller: &Program,
        state: &mut State<'a>,
    ) -> Result<PrivateTypes<'a>, Vec<Diagnostic>> {
        // Preserve the existing per-caller ceiling, not a new global ceiling.
        // Shared acquisition is charged to its first actual caller only,
        // under the existing WORK ceiling, never a fresh proof allowance.
        // All retained allocations use the enclosing cumulative builder ledger.
        let mut remaining = WORK;
        debit(&mut remaining, caller.module_uses.len())?;
        if !caller
            .module_uses
            .iter()
            .any(|u| matches!(u.kind, ModuleUseKind::Type | ModuleUseKind::Function))
            || !possible(caller, self.programs, &mut remaining)?
        {
            return Ok(PrivateTypes {
                roots: Vec::new(),
                remaining: Cell::new(remaining),
            });
        }
        if state.held.is_none() {
            debit(&mut remaining, self.programs.len())?;
            let count = self
                .programs
                .iter()
                .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
                .ok_or_else(refusal)?;
            let indexed = self
                .programs
                .iter()
                .try_fold(count, |n, p| n.checked_add(p.functions.len()))
                .ok_or_else(refusal)?;
            debit(&mut remaining, indexed)?;
            state.held = Some(
                match TopLevel::prove(
                    caller,
                    self.programs,
                    self.authored,
                    count,
                    indexed,
                    &mut remaining,
                )? {
                    Some(v) => Held::Borrowed(v),
                    None => Held::Legacy(legacy(self.programs, indexed)?),
                },
            );
            state.count = count;
        }
        let held = state.held.as_ref().ok_or_else(refusal)?;
        if explicit_only(caller, held, self.programs, &mut remaining)? != Absence::Possible {
            return Ok(PrivateTypes {
                roots: Vec::new(),
                remaining: Cell::new(remaining),
            });
        }
        if state.frames.capacity() == 0 {
            reserve_builder_structure(
                DEPTH
                    .checked_mul(std::mem::size_of::<Frame<'_>>())
                    .ok_or_else(refusal)?,
            )?;
            state
                .frames
                .try_reserve_exact(DEPTH)
                .map_err(|_| refusal())?;
            if state.frames.capacity() != DEPTH {
                return Err(refusal());
            }
        }
        PrivateTypes::finish_ready(
            caller,
            self.programs,
            held,
            state.count,
            remaining,
            &mut state.frames,
        )
    }
}
fn legacy<'a>(
    programs: &'a [Program],
    indexed: usize,
) -> Result<BTreeMap<&'a str, AuthoredDeclaration<'a>>, Vec<Diagnostic>> {
    let node = crate::cleanup_plan::replay_btree_node_upper::<&str, AuthoredDeclaration<'_>>()
        .ok_or_else(refusal)?;
    reserve_builder_structure(indexed.checked_mul(node).ok_or_else(refusal)?)?;
    let mut map = BTreeMap::new();
    for p in programs {
        for f in &p.functions {
            if map
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
            if map
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
    Ok(map)
}
#[cfg(test)]
#[path = "authority_invocation/tests.rs"]
mod tests;

#[path = "authority_invocation/scratch.rs"]
pub(in crate::workspace_graph) mod scratch;
