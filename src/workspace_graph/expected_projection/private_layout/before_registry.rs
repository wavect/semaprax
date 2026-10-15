//! One source-held directory and monotonic preparation receipt per invocation.
use super::{
    refusal,
    shape::{debit, explicit_only, Absence},
    AuthoredDeclaration, LayoutClosure, WORK,
};
use crate::{ast::Program, diagnostic::Diagnostic};
use std::{
    cell::{Cell, Ref, RefCell},
    collections::BTreeMap,
};

pub(in crate::workspace_graph) struct LayoutRegistry<'a> {
    programs: &'a [Program],
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    slots: RefCell<Vec<Option<LayoutClosure<'a>>>>,
    ledger: RefCell<Option<std::rc::Rc<super::ledger::Ledger>>>,
    empty: RefCell<LayoutClosure<'a>>,
    remaining: Cell<usize>,
    receipt: Cell<usize>,
    failed: Cell<bool>,
    #[cfg(test)]
    derivations: Cell<usize>,
}
impl<'a> LayoutRegistry<'a> {
    pub(in crate::workspace_graph) fn new(
        programs: &'a [Program],
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    ) -> Self {
        Self {
            programs,
            authored,
            slots: RefCell::new(Vec::new()),
            ledger: RefCell::new(None),
            empty: RefCell::new(LayoutClosure {
                entries: Vec::new(),
                owners: None,
            }),
            remaining: Cell::new(WORK),
            receipt: Cell::new(0),
            failed: Cell::new(false),
            #[cfg(test)]
            derivations: Cell::new(0),
        }
    }
    pub(in crate::workspace_graph) fn receipt(&self) -> usize {
        self.receipt.get()
    }

    fn fail(&self) {
        self.failed.set(true);
        self.remaining.set(0);
        if let Some(ledger) = self.ledger.borrow().as_ref() {
            ledger.failed.set(true);
            ledger.remaining.set(0);
        }
    }
    fn is_failed(&self) -> bool {
        self.failed.get()
            || self
                .ledger
                .borrow()
                .as_ref()
                .is_some_and(|ledger| ledger.failed.get())
    }
    fn work_remaining(&self) -> usize {
        self.ledger
            .borrow()
            .as_ref()
            .map_or(self.remaining.get(), |ledger| ledger.remaining.get())
    }
    fn set_remaining(&self, remaining: usize) {
        if let Some(ledger) = self.ledger.borrow().as_ref() {
            ledger.remaining.set(remaining)
        } else {
            self.remaining.set(remaining)
        }
    }
    fn prepare<T>(
        &self,
        operation: impl FnOnce() -> Result<T, Vec<Diagnostic>>,
    ) -> Result<T, Vec<Diagnostic>> {
        if self.is_failed() {
            return Err(refusal());
        }
        let available = super::super::super::active_builder_limit()
            .checked_sub(self.receipt.get())
            .ok_or_else(refusal)?;
        let (result, overflowed, consumed, _) =
            crate::bounded_output::with_limit_usage_refusal(available, operation);
        let total = self
            .receipt
            .get()
            .checked_add(consumed)
            .ok_or_else(refusal)?;
        self.receipt.set(total);
        if overflowed || result.is_err() {
            self.fail()
        }
        if overflowed {
            result.and_then(|_| {
                Err(vec![super::super::super::limit_error(
                    "builder_bytes",
                    super::super::super::active_builder_limit(),
                )])
            })
        } else {
            result
        }
    }

    pub(in crate::workspace_graph) fn get(
        &self,
        caller: &Program,
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
        programs: &[Program],
    ) -> Result<Ref<'_, LayoutClosure<'a>>, Vec<Diagnostic>> {
        let result =
            if std::ptr::eq(authored, self.authored) && std::ptr::eq(programs, self.programs) {
                self.lookup(caller)
            } else {
                Err(refusal())
            };
        if result.is_err() {
            self.fail()
        }
        result
    }
    /// This caller's already held closure, borrowed only to present the private
    /// copies appended from it. It derives nothing, debits no work, grants no
    /// lookup and allocates nothing; an unheld or explicit-only caller has none.
    pub(in crate::workspace_graph) fn held(
        &self,
        caller: &Program,
    ) -> Option<Ref<'_, LayoutClosure<'a>>> {
        let index = self.index(caller)?;
        Ref::filter_map(self.slots.borrow(), |slots| slots.get(index)?.as_ref()).ok()
    }
    fn index(&self, caller: &Program) -> Option<usize> {
        let address = caller as *const Program as usize;
        let base = self.programs.as_ptr() as usize;
        let stride = std::mem::size_of::<Program>();
        let index = address
            .checked_sub(base)
            .filter(|offset| offset % stride == 0)?
            / stride;
        self.programs
            .get(index)
            .is_some_and(|held| std::ptr::eq(held, caller))
            .then_some(index)
    }
    fn lookup(&self, caller: &Program) -> Result<Ref<'_, LayoutClosure<'a>>, Vec<Diagnostic>> {
        if self.is_failed() {
            return Err(refusal());
        }
        let mut work = self.work_remaining();
        debit(&mut work, 1)?;
        self.set_remaining(work);
        let index = self.index(caller).ok_or_else(refusal)?;
        if self.slots.borrow().get(index).is_some_and(Option::is_some) {
            return Ok(Ref::map(self.slots.borrow(), |slots| {
                slots[index].as_ref().expect("checked held entry")
            }));
        }
        let absence = explicit_only(caller, self.authored, self.programs, &mut work);
        self.set_remaining(work);
        if absence? != Absence::Possible {
            return Ok(self.empty.borrow());
        }
        self.prepare(|| {
            if self.slots.borrow().is_empty() {
                super::reserve_builder_structure(
                    std::mem::size_of::<Self>()
                        .checked_add(
                            self.programs
                                .len()
                                .checked_mul(std::mem::size_of::<Option<LayoutClosure<'a>>>())
                                .ok_or_else(refusal)?,
                        )
                        .ok_or_else(refusal)?,
                )?;
                let ledger = super::ledger::Ledger::new(self.work_remaining())?;
                *self.ledger.borrow_mut() = Some(ledger);
                let mut slots = Vec::new();
                slots
                    .try_reserve_exact(self.programs.len())
                    .map_err(|_| refusal())?;
                if slots.capacity() != self.programs.len() {
                    return Err(refusal());
                }
                slots.resize_with(self.programs.len(), || None);
                *self.slots.borrow_mut() = slots;
            }
            #[cfg(test)]
            {
                self.derivations.set(self.derivations.get() + 1);
                DERIVATIONS.with(|count| count.set(count.get() + 1));
            }
            let mut work = self.work_remaining();
            let layout = LayoutClosure::derive_before_scratch(
                caller,
                self.authored,
                self.programs,
                &mut work,
                self.ledger.borrow().as_ref(),
            );
            self.set_remaining(work);
            let layout = layout?;
            self.slots.borrow_mut()[index] = Some(layout);
            Ok(())
        })?;
        Ok(Ref::map(self.slots.borrow(), |slots| {
            slots[index].as_ref().expect("successful held derivation")
        }))
    }
}


#[cfg(test)]
thread_local! {static DERIVATIONS: Cell<usize> = const {Cell::new(0)};}
#[cfg(test)]
fn observe_derivations<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            DERIVATIONS.with(|count| count.set(self.0));
        }
    }
    let saved = Restore(DERIVATIONS.with(|count| count.replace(0)));
    let result = operation();
    let count = DERIVATIONS.with(Cell::get);
    drop(saved);
    (result, count)
}
