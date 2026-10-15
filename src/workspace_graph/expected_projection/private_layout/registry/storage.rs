//! A physical storage owner, acquired in one real preparation child. Its old
//! ledger is evidence of acquisition, never a source of remaining allowance.
use super::super::{ledger, names, owner_index, refusal, AuthoredDeclaration, Frame};
use crate::ast::Program;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::diagnostic::Diagnostic;
use std::{collections::BTreeMap, mem::size_of, rc::Rc};

pub(in crate::workspace_graph::expected_projection::private_layout) struct Storage<'a> {
    programs: &'a [Program],
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    work: Rc<ledger::Ledger>,
    acquisition: ClassifierLedger,
    parent: ClassifierLedger,
    bound: bool,
    entry_paid: bool,
    rejection: Vec<Diagnostic>,
    pub(in crate::workspace_graph::expected_projection::private_layout) census:
        Option<names::Census<'a>>,
    pub(in crate::workspace_graph::expected_projection::private_layout) frames: Vec<Frame<'a>>,
    pub(in crate::workspace_graph::expected_projection::private_layout) owners:
        owner_index::Scratch<'a>,
}
impl<'a> Storage<'a> {
    pub(super) fn acquire(
        programs: &'a [Program],
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        work: &Rc<ledger::Ledger>,
    ) -> Result<Self, Vec<Diagnostic>> {
        // The required child payer exists: this call is inside registry.prepare's actual
        // child. Headers and all capture/return controls precede construction.
        super::super::reserve_builder_structure(
            Self::entry_controls()
                + ClassifierLedger::storage_terminal_receipt_controls()
                + size_of::<Vec<Diagnostic>>()
                + 2 * size_of::<Diagnostic>()
                + 3 * size_of::<Vec<Diagnostic>>()
                + size_of::<&mut Self>() // one terminal take_rejection receiver
                + size_of::<Self>()
                + size_of::<Result<Self, Vec<Diagnostic>>>()
                + 2 * size_of::<Option<Self>>()
                + 2 * size_of::<std::cell::RefMut<'_, Option<Self>>>()
                + size_of::<std::cell::Ref<'_, Option<Self>>>()
                + size_of::<Option<&Self>>()
                + size_of::<&Self>()
                + size_of::<Result<(), Vec<Diagnostic>>>()
                + 2 * size_of::<ClassifierLedger>()
                + size_of::<Rc<ledger::Ledger>>()
                + size_of::<&Rc<ledger::Ledger>>() // Rc::clone's receiving argument
                + size_of::<Vec<Frame<'a>>>()
                + owner_index::Scratch::<'a>::initializer_controls()
                + ClassifierLedger::capture_into_controls(),
        )?;
        let mut rejection = Vec::with_capacity(1);
        if rejection.capacity() != 1 {
            return Err(refusal());
        }
        rejection.push(Diagnostic::io_static(
            "SPX-G172",
            "private layout closure exceeds its independently bounded nominal profile",
        ));
        Ok(Self {
            programs,
            authored,
            work: Rc::clone(work),
            acquisition: ClassifierLedger::capture(),
            parent: ClassifierLedger::capture(),
            bound: false,
            entry_paid: true,
            rejection,
            census: None,
            frames: Vec::new(),
            owners: owner_index::Scratch::empty(),
        })
    }
    pub(super) fn acquisition_arguments() -> usize {
        // This frontier is paid before the caller evaluates the NEW acquire
        // arguments, not retrospectively by acquire or the future entry bank.
        size_of::<Result<(), Vec<Diagnostic>>>()
            + size_of::<std::cell::Ref<'_, Option<Rc<ledger::Ledger>>>>()
            + size_of::<Option<&Rc<ledger::Ledger>>>()
            + 2 * size_of::<&Rc<ledger::Ledger>>()
            + size_of::<&[Program]>()
            + size_of::<&BTreeMap<&str, AuthoredDeclaration<'_>>>()
    }
    pub(super) fn entry_controls() -> usize {
        // Every receiver below is specific to one future cold entry (and its
        // possible first rejection), paid by the preceding actual child.
        size_of::<std::cell::RefMut<'_, Option<Self>>>()
            + 2 * size_of::<Option<&mut Self>>() // entry and post-restore binding
            + 4 * size_of::<&mut Self>() // their initialized/receiving references
            + size_of::<std::cell::Ref<'_, Option<Rc<ledger::Ledger>>>>()
            + 2 * size_of::<Option<&Rc<ledger::Ledger>>>()
            + 4 * size_of::<&Rc<ledger::Ledger>>() // work predicate and ptr_eq arguments
            + size_of::<&[Program]>()
            + size_of::<&BTreeMap<&str, AuthoredDeclaration<'_>>>()
            + 2 * size_of::<bool>()
            + ClassifierLedger::storage_authentication_controls()
            + size_of::<Result<(), Vec<Diagnostic>>>()
            + size_of::<std::cell::Ref<'_, Option<Self>>>()
            + size_of::<std::cell::RefMut<'_, Option<Self>>>()
    }
    pub(super) fn lease_controls() -> usize {
        size_of::<Result<(), Vec<Diagnostic>>>()
            + size_of::<std::cell::RefMut<'_, Option<Self>>>()
            + size_of::<Option<&mut Self>>()
            + size_of::<&mut Self>()
    }
    pub(super) fn renew_entry(&mut self) {
        // First acquisition or this child's successful entry_controls reserve
        // is the only caller. This field is never an allowance counter.
        self.entry_paid = true;
    }
    pub(super) fn take_rejection(&mut self) -> Vec<Diagnostic> {
        std::mem::take(&mut self.rejection)
    }
    /// Actual receiving storage and this one-time overwrite were paid by the
    /// acquisition before the child restored. No returned ledger is constructed.
    pub(super) fn bind_restored_parent(&mut self) {
        if !self.bound {
            self.parent.capture_current_into();
            self.bound = true;
        }
    }
    pub(super) fn reject(&self) {
        self.acquisition.reject_storage();
    }
    pub(super) fn enter(
        &mut self,
        programs: &[Program],
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
        work: Option<&Rc<ledger::Ledger>>,
    ) -> bool {
        assert!(self.entry_paid, "one-shot entry controls cannot be reused");
        self.entry_paid = false;
        // Source and work identity precede the parent check. Rc identity is
        // stable across registry moves; no registry stack address is authority.
        self.bound
            && std::ptr::eq(programs, self.programs)
            && std::ptr::eq(authored, self.authored)
            && work.is_some_and(|work| Rc::ptr_eq(work, &self.work))
            && !self.work.failed.get()
            && self.acquisition.successful_storage_acquisition()
            && self.parent.storage_parent_current()
    }
    #[cfg(test)]
    pub(super) fn state(&self) -> ([usize; 3], [usize; 2], [usize; 2], bool) {
        let census = self.census.as_ref().expect("eligible census").state();
        let owners = self.owners.state();
        (
            census.0,
            [self.frames.as_ptr() as usize, self.frames.capacity()],
            owners.0,
            census.4 && self.frames.is_empty() && owners.2,
        )
    }
}
