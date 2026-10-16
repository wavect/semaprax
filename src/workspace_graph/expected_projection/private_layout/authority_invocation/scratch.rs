//! Invocation-paid backing only. Every lease rebuilds roots and logical WORK.
use super::super::{graph_error, visit};
use super::{
    debit, explicit_only, legacy, possible, refusal, reserve_builder_structure, Absence,
    AuthoredDeclaration, BTreeMap, Cell, Diagnostic, Frame, Held, Invocation, Lookup,
    ModuleUseKind, PrivateTypes, Program, State, TopLevel, DEPTH, WORK,
};
use crate::bounded_output::invocation_binding::InvocationBinding;
use std::cell::{RefCell, RefMut};
use std::mem::size_of;

type Roots<'a> = Vec<(&'a str, &'a str)>;
/// Complete predecessor owner stays independent; only this receiver retains roots.
pub(in crate::workspace_graph) struct ScratchInvocation<'a> {
    base: Invocation<'a>,
    roots: RefCell<Roots<'a>>,
    binding: InvocationBinding,
}
/// Guard holds both exclusive borrows through the consumer's final comparison.
/// No mutable backing, ticket, source verdict or detached authority is exposed.
pub(in crate::workspace_graph) struct Lease<'lease, 'a> {
    state: RefMut<'lease, State<'a>>,
    roots: RefMut<'lease, Roots<'a>>,
    remaining: Cell<usize>,
    binding: &'lease InvocationBinding,
    complete: bool,
}
impl Drop for Lease<'_, '_> {
    fn drop(&mut self) {
        if !self.complete {
            self.state.failed = true;
        }
        self.roots.clear();
        self.state.frames.clear();
    }
}
/// Ordinary fallback owns its complete original roots; leased roots cannot escape
/// their invocation. Lease fields and backing mutation stay private.
pub(in crate::workspace_graph) enum Current<'lease, 'a, 'owned> {
    Owned(PrivateTypes<'owned>),
    Leased(Lease<'lease, 'a>),
}
impl<'lease, 'a, 'owned> Current<'lease, 'a, 'owned> {
    pub(in crate::workspace_graph) fn derive(
        owner: Option<&'lease ScratchInvocation<'a>>,
        caller: &Program,
        programs: &'owned [Program],
        authored: &'owned BTreeMap<&'owned str, AuthoredDeclaration<'owned>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let controls = size_of::<Self>()
            .checked_add(size_of::<Result<Self, Vec<Diagnostic>>>())
            .and_then(|n| {
                n.checked_add(size_of::<(
                    Option<&ScratchInvocation<'_>>,
                    &Program,
                    &[Program],
                    &BTreeMap<&str, AuthoredDeclaration<'_>>,
                )>())
            })
            .ok_or_else(refusal)?;
        required(controls)?;
        match owner {
            Some(owner) => owner
                .lease_bound(caller, programs, authored)
                .map(Self::Leased),
            None => PrivateTypes::derive(caller, programs, authored).map(Self::Owned),
        }
    }
    pub(in crate::workspace_graph) fn contains(
        &self,
        id: &str,
        module: Option<&str>,
    ) -> Result<bool, Vec<Diagnostic>> {
        match self {
            Self::Owned(old) => old.contains(id, module),
            Self::Leased(lease) => lease.view()?.contains(id, module),
        }
    }
    pub(in crate::workspace_graph) fn validate_signatures(
        &self,
        matches: impl FnMut(&str, &str) -> bool,
    ) -> Result<(), Vec<Diagnostic>> {
        match self {
            Self::Owned(old) => old.validate_signatures(matches),
            Self::Leased(lease) => lease.view()?.validate_signatures(matches),
        }
    }
}
impl Lease<'_, '_> {
    fn view(&self) -> Result<View<'_, '_>, Vec<Diagnostic>> {
        if !self.binding.current() || !crate::bounded_output::reserve_active_required(0) {
            return Err(refusal());
        }
        Ok(View {
            roots: &self.roots,
            remaining: &self.remaining,
        })
    }
}
struct View<'lease, 'a> {
    roots: &'lease [(&'a str, &'a str)],
    remaining: &'lease Cell<usize>,
}
impl View<'_, '_> {
    pub(in crate::workspace_graph) fn contains(
        &self,
        id: &str,
        module: Option<&str>,
    ) -> Result<bool, Vec<Diagnostic>> {
        let mut work = self.remaining.get();
        let result = (|| {
            for (provider, root) in self.roots {
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
        for (module, id) in self.roots {
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
impl<'a> ScratchInvocation<'a> {
    /// None is a bootstrap funding refusal, translated at the existing outer
    /// diagnostic boundary. No failure-report allocation occurs in this owner.
    pub(in crate::workspace_graph) fn new(
        programs: &'a [Program],
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    ) -> Option<Self> {
        let controls = size_of::<Self>()
            .checked_add(size_of::<Option<Self>>())?
            .checked_add(size_of::<Result<Self, Vec<Diagnostic>>>())?
            .checked_add(size_of::<Invocation<'_>>())?
            .checked_add(size_of::<(
                &[Program],
                &BTreeMap<&str, AuthoredDeclaration<'_>>,
            )>())?
            .checked_add(size_of::<InvocationBinding>())?
            .checked_add(size_of::<Option<usize>>())?;
        if !crate::bounded_output::reserve_active_required(controls) {
            return None;
        }
        Some(Self {
            base: Invocation::new(programs, authored),
            roots: RefCell::new(Vec::new()),
            binding: InvocationBinding::capture(),
        })
    }
    pub(in crate::workspace_graph) fn lease_bound(
        &self,
        caller: &Program,
        programs: &[Program],
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    ) -> Result<Lease<'_, 'a>, Vec<Diagnostic>> {
        // Identity and first refusal precede every reuse, even an empty lease.
        if !self.binding.current() || !crate::bounded_output::reserve_active_required(0) {
            return Err(refusal());
        }
        let controls = lease_controls().ok_or_else(refusal)?;
        required(controls)?;
        let mut state = self.base.state.try_borrow_mut().map_err(|_| refusal())?;
        if state.failed {
            return Err(refusal());
        }
        if !std::ptr::eq(self.base.programs, programs)
            || !std::ptr::eq(self.base.authored, authored)
        {
            state.failed = true;
            return Err(refusal());
        }
        let stride = size_of::<Program>();
        let member = (caller as *const Program as usize)
            .checked_sub(self.base.programs.as_ptr() as usize)
            .filter(|offset| offset % stride == 0)
            .and_then(|offset| self.base.programs.get(offset / stride));
        if !member.is_some_and(|p| std::ptr::eq(p, caller)) {
            state.failed = true;
            return Err(refusal());
        }
        let roots = self.roots.try_borrow_mut().map_err(|_| refusal())?;
        if !roots.is_empty() || !state.frames.is_empty() {
            state.failed = true;
            return Err(refusal());
        }
        // Construct the cleanup guard before any traversal can fail or unwind.
        let mut lease = Lease {
            state,
            roots,
            remaining: Cell::new(0),
            binding: &self.binding,
            complete: false,
        };
        match self.prepare(caller, &mut lease.state, &mut lease.roots) {
            Ok(remaining) => {
                lease.remaining.set(remaining);
                lease.complete = true;
                Ok(lease)
            }
            Err(error) => {
                lease.state.failed = true;
                Err(error)
            }
        }
    }
    fn prepare(
        &self,
        caller: &Program,
        state: &mut State<'a>,
        roots: &mut Roots<'a>,
    ) -> Result<usize, Vec<Diagnostic>> {
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
            || !possible(caller, self.base.programs, &mut remaining)?
        {
            return Ok(remaining);
        }
        if state.held.is_none() {
            debit(&mut remaining, self.base.programs.len())?;
            let count = self
                .base
                .programs
                .iter()
                .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
                .ok_or_else(refusal)?;
            let indexed = self
                .base
                .programs
                .iter()
                .try_fold(count, |n, p| n.checked_add(p.functions.len()))
                .ok_or_else(refusal)?;
            debit(&mut remaining, indexed)?;
            state.held = Some(
                match TopLevel::prove(
                    caller,
                    self.base.programs,
                    self.base.authored,
                    count,
                    indexed,
                    &mut remaining,
                )? {
                    Some(v) => Held::Borrowed(v),
                    None => Held::Legacy(legacy(self.base.programs, indexed)?),
                },
            );
            state.count = count;
        }
        let held = state.held.as_ref().ok_or_else(refusal)?;
        if explicit_only(caller, held, self.base.programs, &mut remaining)? != Absence::Possible {
            return Ok(remaining);
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
        self.fill(
            caller,
            held,
            state.count,
            &mut remaining,
            &mut state.frames,
            roots,
        )?;
        Ok(remaining)
    }
    fn fill<A: Lookup<'a>>(
        &self,
        caller: &Program,
        authored: &A,
        count: usize,
        remaining: &mut usize,
        frames: &mut Vec<Frame<'a>>,
        roots: &mut Roots<'a>,
    ) -> Result<(), Vec<Diagnostic>> {
        if roots.capacity() == 0 && count != 0 {
            let layout = std::alloc::Layout::array::<(&str, &str)>(count).map_err(|_| refusal())?;
            required(layout.size())?;
            roots.try_reserve_exact(count).map_err(|_| refusal())?;
            if roots.capacity() != count {
                if roots.capacity() > count {
                    let excess =
                        std::alloc::Layout::array::<(&str, &str)>(roots.capacity() - count)
                            .map_err(|_| refusal())?;
                    required(excess.size())?;
                }
                return Err(refusal());
            }
        }
        visit(
            caller,
            authored,
            self.base.programs,
            frames,
            remaining,
            |module, declaration, work| {
                for (m, id) in roots.iter() {
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

        Ok(())
    }
}
// Named concrete slots, not a flat allowance: exclusive borrow locals, final
// guard and result, query-view/reborrow receiver, preparation result, lexical
// arguments, full-census allocation/layout receivers and root callback captures.
fn lease_controls() -> Option<usize> {
    size_of::<Lease<'_, '_>>()
        .checked_add(size_of::<RefMut<'_, State<'_>>>())?
        .checked_add(size_of::<RefMut<'_, Roots<'_>>>())?
        .checked_add(size_of::<View<'_, '_>>())?
        .checked_add(size_of::<Result<View<'_, '_>, Vec<Diagnostic>>>())?
        .checked_add(size_of::<Result<Lease<'_, '_>, Vec<Diagnostic>>>())?
        .checked_add(size_of::<(
            &ScratchInvocation<'_>,
            &Program,
            &[Program],
            &BTreeMap<&str, AuthoredDeclaration<'_>>,
        )>())?
        .checked_add(size_of::<Result<usize, Vec<Diagnostic>>>())?
        .checked_add(size_of::<(
            &ScratchInvocation<'_>,
            &Program,
            &mut State<'_>,
            &mut Roots<'_>,
        )>())?
        .checked_add(size_of::<(
            &ScratchInvocation<'_>,
            &Program,
            &Held<'_>,
            usize,
            &mut usize,
            &mut Vec<Frame<'_>>,
            &mut Roots<'_>,
        )>())?
        .checked_add(size_of::<(&mut Roots<'_>, usize)>())?
        .checked_add(size_of::<Result<(), Vec<Diagnostic>>>())?
        .checked_add(size_of::<std::alloc::Layout>())?
        .checked_add(size_of::<Result<std::alloc::Layout, std::alloc::LayoutError>>())?
        .checked_add(size_of::<(usize, usize)>())?
        .checked_add(size_of::<InvocationBinding>())
}
fn required(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        reserve_builder_structure(0)
    }
}
#[cfg(test)]
#[path = "scratch/tests.rs"]
mod tests;
