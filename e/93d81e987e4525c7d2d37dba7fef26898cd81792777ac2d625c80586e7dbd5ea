//! Physical backing counterpart of the original classifier's logical sink.
//! Logical decline keeps the original partial/suppressed output; only physical
//! acquisition publishes a typed failure. No allowance or authority is created.
use super::{active, reserve, reserve_sink, Budget};
use std::fmt;
use std::mem::size_of;
use std::rc::Rc;

#[derive(Debug)]
pub(crate) enum GrowthFailure<E> {
    Payment(E),
    Arithmetic,
    Allocation,
    Capacity,
    Formatter,
}

pub(crate) struct ClassifierString {
    bytes: String,
    logical: Option<Rc<Budget>>,
}
impl ClassifierString {
    pub(crate) fn acquire<E>(
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, GrowthFailure<E>> {
        pay(size_of::<Self>()).map_err(GrowthFailure::Payment)?;
        Ok(Self {
            bytes: String::new(),
            logical: active(),
        })
    }

    fn append<E>(
        &mut self,
        value: &str,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
        logical: bool,
        strict: bool,
    ) -> Result<bool, GrowthFailure<E>> {
        // Keep the original reserve_sink frontier BEFORE any physical request.
        // A reserved-floor decline is not necessarily sticky physical refusal.
        if logical {
            // Strict budgeted_format reserves against its captured budget;
            // CappedString uses reserve_sink's current-or-captured rule.
            let accepted = if strict {
                reserve(self.logical.as_deref(), value.len())
            } else {
                reserve_sink(self.logical.as_ref(), value.len())
            };
            if !accepted {
                return Ok(false);
            }
        }
        let required = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or(GrowthFailure::Arithmetic)?;
        if required > self.bytes.capacity() {
            let doubled = self
                .bytes
                .capacity()
                .checked_mul(2)
                .ok_or(GrowthFailure::Arithmetic)?;
            let target = required.max(doubled).max(8);
            // The whole new requested buffer is paid while the prior buffer
            // remains charged. Dropping either carrier never refunds the payer.
            pay(target).map_err(GrowthFailure::Payment)?;
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| GrowthFailure::Allocation)?;
            if self.bytes.capacity() != target {
                return Err(GrowthFailure::Capacity);
            }
        }
        self.bytes.push_str(value);
        Ok(true)
    }

    /// Exact CappedString semantics: declined logical appends are suppressed,
    /// later formatters still run, and the accepted partial bytes are retained.
    pub(crate) fn capped<E>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), GrowthFailure<E>> {
        self.write(arguments, pay, false, true).map(|_| ())
    }

    /// Exact budgeted_format semantics: the first logical decline stops its
    /// formatter and discards the whole string, without inventing sticky state.
    pub(crate) fn strict<E>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), GrowthFailure<E>> {
        match self.write(arguments, pay, true, true) {
            Ok(true) => {}
            Ok(false) => self.bytes = String::new(),
            Err(GrowthFailure::Formatter) => match &self.logical {
                Some(budget) => {
                    budget.overflowed.set(true);
                    self.bytes = String::new();
                }
                None => panic!("a formatting trait implementation returned an error"),
            },
            Err(error) => return Err(error),
        }
        Ok(())
    }

    /// Original writes to an ordinary String do not debit logical sink text.
    /// They still acquire actual physical replacement backing before growth.
    pub(crate) fn raw<E>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), GrowthFailure<E>> {
        self.write(arguments, pay, false, false).map(|_| ())
    }

    fn write<E>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        pay: &mut impl FnMut(usize) -> Result<(), E>,
        strict: bool,
        logical: bool,
    ) -> Result<bool, GrowthFailure<E>> {
        struct Writer<'a, E, P> {
            output: &'a mut ClassifierString,
            pay: &'a mut P,
            strict: bool,
            logical: bool,
            logical_refusal: bool,
            physical: Option<GrowthFailure<E>>,
        }
        impl<E, P: FnMut(usize) -> Result<(), E>> fmt::Write for Writer<'_, E, P> {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                if self.physical.is_some() || (self.strict && self.logical_refusal) {
                    return Err(fmt::Error);
                }
                match self
                    .output
                    .append(value, self.pay, self.logical, self.strict)
                {
                    Ok(true) => Ok(()),
                    Ok(false) => {
                        self.logical_refusal = true;
                        if self.strict {
                            Err(fmt::Error)
                        } else {
                            Ok(())
                        }
                    }
                    Err(error) => {
                        self.physical = Some(error);
                        Err(fmt::Error)
                    }
                }
            }
        }
        let mut writer = Writer {
            output: self,
            pay,
            strict,
            logical,
            logical_refusal: false,
            physical: None,
        };
        let outcome = fmt::write(&mut writer, arguments);
        if let Some(error) = writer.physical {
            return Err(error);
        }
        if writer.strict && writer.logical_refusal {
            return Ok(false);
        }
        outcome.map_err(|_| GrowthFailure::Formatter)?;
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str {
        &self.bytes
    }
    pub(crate) fn into_string(self) -> String {
        self.bytes
    }
}

/// Exact invocation binding only; this scalar/Rc carrier grants no allowance,
/// cached admission or source authority. Its caller pays its containing control.
pub(crate) struct ClassifierLedger(Option<Rc<Budget>>);

/// Borrowed identity, never allowance: the containing ledger must stay alive.
/// No TLS loan is retained and every use reads ACTIVE again.
pub(crate) struct StorageParent<'a>(&'a Budget);
impl StorageParent<'_> {
    pub(crate) const ACQUISITION_CONTROLS: usize = {
        let method = 2 * size_of::<&ClassifierLedger>();
        let pattern = size_of::<&Option<Rc<Budget>>>() + size_of::<&Rc<Budget>>();
        // Rc::deref actual/formal and return, then the witness field loan.
        let field = 2 * size_of::<&Rc<Budget>>() + 2 * size_of::<&Budget>();
        let constructed = size_of::<Self>();
        let returned = 2 * size_of::<Option<Self>>();
        let received_pattern = size_of::<Self>();
        method + pattern + field + constructed + returned + received_pattern
    };

    /// One successful-call bank; the immediately returning mismatch uses the
    /// separate terminal bank, prepaid ONCE by the containing continuation.
    pub(crate) const CURRENT_CONTROLS: usize = {
        use std::cell::{Cell, Ref, RefCell};
        use std::thread::LocalKey;
        type Active = RefCell<Option<Rc<Budget>>>;
        type Loan = Ref<'static, Option<Rc<Budget>>>;
        let helper = 2 * size_of::<&Self>() + 3 * size_of::<bool>();
        let tls = 2 * size_of::<&LocalKey<Active>>() + 2 * size_of::<&Self>() + size_of::<bool>();
        let callback = 2 * size_of::<&Active>() + size_of::<bool>();
        let borrow = 2 * size_of::<&Active>() + size_of::<Loan>();
        let option_loan = 2 * size_of::<&Loan>() + size_of::<&Option<Rc<Budget>>>();
        let pattern = size_of::<&Option<Rc<Budget>>>() + size_of::<&Rc<Budget>>();
        // Current Rc dereference, then ptr::eq actual/formal pairs and guard.
        // The held operand is already a Budget loan; no held Rc is dereferenced.
        let identity = 2 * size_of::<&Rc<Budget>>()
            + size_of::<&Budget>()
            + 4 * size_of::<&Budget>()
            + size_of::<bool>();
        let sticky = 2 * size_of::<&Cell<bool>>() + 2 * size_of::<bool>();
        helper + tls + callback + borrow + option_loan + pattern + identity + sticky
    };

    /// At most one reached mismatch in the whole continuation. A false result
    /// must immediately exit; this bank cannot fund a second mismatch call.
    pub(crate) const TERMINAL_CONTROLS: usize = {
        use std::cell::Cell;
        type Receipt = Option<super::ReservationRefusal>;
        type Site = Option<&'static std::panic::Location<'static>>;
        type Stage = Option<super::ReservationStage>;
        let mismatch = 2 * size_of::<&Budget>()
            + 2 * size_of::<usize>()
            + 2 * size_of::<&Cell<bool>>()
            + 2 * size_of::<bool>()
            + size_of::<bool>();
        let first_refusal_get = 2 * size_of::<&Cell<Receipt>>() + size_of::<Receipt>();
        let first_refusal_test = 2 * size_of::<&Receipt>() + size_of::<bool>();
        let first_site = size_of::<&'static std::panic::Location<'static>>()
            + 2 * size_of::<Site>()
            + 2 * size_of::<&Cell<Site>>();
        let receipt_fields = 4 * size_of::<&Cell<usize>>()
            + 2 * size_of::<usize>()
            + 2 * size_of::<&Cell<Stage>>()
            + size_of::<Stage>();
        let first_receipt = size_of::<super::ReservationRefusal>()
            + 2 * size_of::<Receipt>()
            + 2 * size_of::<&Cell<Receipt>>();
        let terminal = mismatch
            + first_refusal_get
            + first_refusal_test
            + first_site
            + receipt_fields
            + first_receipt;
        // The complete original guard bank stays an independent target-layout
        // upper bound; no original caller's fee is reduced by this successor.
        assert!(
            Self::CURRENT_CONTROLS + terminal <= ClassifierLedger::STORAGE_PARENT_CURRENT_CONTROLS
        );
        terminal
    };

    pub(crate) fn current(&self) -> bool {
        super::ACTIVE.with(|active| match &*active.borrow() {
            Some(current) if std::ptr::eq(self.0, &**current) => !self.0.overflowed.get(),
            _ => {
                super::record_refusal(self.0, 0);
                self.0.overflowed.set(true);
                false
            }
        })
    }
}
impl ClassifierLedger {
    /// Acquisition does not authenticate ACTIVE or spend allowance. The caller
    /// prepays it on the known parent and later uses fresh current() checks.
    #[allow(
        clippy::manual_map,
        clippy::explicit_auto_deref,
        reason = "Keep the explicit match and Rc dereference aligned with the complete acquisition invoice; a map closure introduces different receiving roles."
    )]
    pub(crate) fn storage_parent(&self) -> Option<StorageParent<'_>> {
        match &self.0 {
            Some(budget) => Some(StorageParent(&**budget)),
            None => None,
        }
    }
    /// Empty field initializer for an already prepaid receiving owner. The
    /// caller must pay capture_into_controls before filling the real binding.
    pub(crate) fn empty_receiving() -> Self {
        Self(None)
    }
    /// A test-account caller authenticates its own invocation separately. This
    /// detached marker contains no ACTIVE handle and grants no ordinary payer.
    #[cfg(test)]
    pub(crate) fn oracle_marker() -> Self {
        Self(None)
    }

    pub(crate) fn capture() -> Self {
        Self(active())
    }
    /// Immutable diagnostic data from this held invocation. It grants neither
    /// permission to spend nor authority to publish under another ACTIVE owner.
    pub(crate) fn first_refusal(&self) -> Option<super::ReservationRefusal> {
        self.0
            .as_ref()
            .and_then(|budget| budget.first_refusal.get())
    }
    /// One-time fill of an already prepaid receiving slot after its acquiring
    /// child restores. No returned ledger/Result is constructed by this gateway.
    pub(crate) fn capture_into_controls() -> usize {
        std::mem::size_of::<&mut Self>()
            + std::mem::size_of::<std::cell::Ref<'_, Option<Rc<Budget>>>>()
            + 2 * std::mem::size_of::<&Option<Rc<Budget>>>()
            + std::mem::size_of::<&mut Option<Rc<Budget>>>()
            + std::mem::size_of::<&std::cell::RefCell<Option<Rc<Budget>>>>()
            + std::mem::size_of::<Option<Rc<Budget>>>()
    }
    pub(crate) fn capture_current_into(&mut self) {
        super::ACTIVE.with(|active| self.0.clone_from(&*active.borrow()));
    }
    pub(crate) fn storage_terminal_receipt_controls() -> usize {
        // Exactly two possible terminal owning sites: parent association and
        // acquisition rejection. Each unchanged record_refusal may return the
        // first Option, initialize the record, initialize Some, receive it in
        // Cell::set, and return the stage Option. No old allowance is spent.
        2 * (3 * std::mem::size_of::<Option<super::ReservationRefusal>>()
            + std::mem::size_of::<super::ReservationRefusal>()
            + std::mem::size_of::<Option<super::ReservationStage>>())
    }
    pub(crate) fn reject_storage(&self) {
        if let Some(budget) = &self.0 {
            super::record_refusal(budget, 0);
            budget.overflowed.set(true);
        }
    }
    pub(crate) fn storage_authentication_controls() -> usize {
        // Parent identity by borrow, then immutable acquisition evidence.
        2 * std::mem::size_of::<&Self>()
            + std::mem::size_of::<&std::cell::RefCell<Option<Rc<Budget>>>>()
            + std::mem::size_of::<std::cell::Ref<'_, Option<Rc<Budget>>>>()
            + 2 * std::mem::size_of::<&Option<Rc<Budget>>>()
            + 5 * std::mem::size_of::<&Rc<Budget>>()
            + std::mem::size_of::<Option<&Rc<Budget>>>()
            + 2 * std::mem::size_of::<bool>()
    }
    /// Retained storage evidence only: this never grants access to remaining
    /// allowance and deliberately does not require the acquiring child current.
    pub(crate) fn successful_storage_acquisition(&self) -> bool {
        self.0
            .as_ref()
            .is_some_and(|budget| !budget.overflowed.get())
    }
    /// Identity and sticky state by borrow, without cloning a receiving Rc.
    /// The containing paid storage owner supplies this fixed control context.
    pub(crate) fn storage_parent_current(&self) -> bool {
        super::ACTIVE.with(|active| match (&self.0, &*active.borrow()) {
            (None, None) => true,
            (Some(held), Some(current)) if Rc::ptr_eq(held, current) => !held.overflowed.get(),
            _ => {
                if let Some(held) = &self.0 {
                    super::record_refusal(held, 0);
                    held.overflowed.set(true);
                }
                false
            }
        })
    }
    /// Complete fresh borrowed parent check. Every named term belongs to an
    /// actual lexical boundary above; no Option<&Rc<Budget>> is constructed.
    /// The known parent prepays both arms BEFORE the callback. The later check
    /// never spends ACTIVE funds, including when ACTIVE is a foreign ledger.
    pub(crate) const STORAGE_PARENT_CURRENT_CONTROLS: usize = {
        use std::cell::{Cell, Ref, RefCell};
        use std::thread::LocalKey;
        type Active = RefCell<Option<Rc<Budget>>>;
        type Loan = Ref<'static, Option<Rc<Budget>>>;
        // Caller/formal self and helper return/caller boolean, plus caller !.
        let helper = 2 * size_of::<&Self>() + 3 * size_of::<bool>();
        // The closure captures only self. Its actual/formal environments each
        // contain that one borrow; with returns the closure's boolean.
        let tls = 2 * size_of::<&LocalKey<Active>>() + 2 * size_of::<&Self>() + size_of::<bool>();
        // with supplies active to the closure: actual/formal active, then the
        // distinct actual/formal RefCell::borrow receiver and owned Ref result.
        let callback = 2 * size_of::<&Active>() + size_of::<bool>();
        let borrow = 2 * size_of::<&Active>() + size_of::<Loan>();
        // &*active.borrow(): Ref::deref actual/formal and returned option loan.
        let option_loan = 2 * size_of::<&Loan>() + size_of::<&Option<Rc<Budget>>>();
        // &self.0, the actual two-field match tuple, and the successful Some
        // bindings are separate roles, not an Option::as_ref return.
        let pattern = size_of::<&Option<Rc<Budget>>>()
            + size_of::<(&Option<Rc<Budget>>, &Option<Rc<Budget>>)>()
            + 2 * size_of::<&Rc<Budget>>();
        let pointer_test = 4 * size_of::<&Rc<Budget>>() + size_of::<bool>();
        // held.overflowed: Rc::deref actual/formal/output, Cell::get receiver
        // actual/formal, returned bool and its lexical negation.
        let successful = 2 * size_of::<&Rc<Budget>>()
            + size_of::<&Budget>()
            + 2 * size_of::<&Cell<bool>>()
            + 2 * size_of::<bool>();
        // The mismatch if-let has its own option loan and held binding. Its two
        // Rc dereferences belong respectively to record_refusal's coercion and
        // the subsequent overflowed field; neither borrows the success bank.
        let mismatch = size_of::<&Option<Rc<Budget>>>()
            + size_of::<&Rc<Budget>>()
            + 4 * size_of::<&Rc<Budget>>()
            + 2 * size_of::<&Budget>()
            + 2 * size_of::<&Budget>()
            + 2 * size_of::<usize>()
            + 2 * size_of::<&Cell<bool>>()
            + 2 * size_of::<bool>();
        // One reached record_refusal body, not the insertion owner's unrelated
        // two-call bank: get/is_none precede the optional first sticky receipt.
        type Receipt = Option<super::ReservationRefusal>;
        type Site = Option<&'static std::panic::Location<'static>>;
        type Stage = Option<super::ReservationStage>;
        let first_refusal_get = 2 * size_of::<&Cell<Receipt>>() + size_of::<Receipt>();
        let first_refusal_test = 2 * size_of::<&Receipt>() + size_of::<bool>();
        // Location::caller output, Some(site) actual and Cell::set value formal,
        // and that distinct set's actual/formal receiver loans.
        let first_site = size_of::<&'static std::panic::Location<'static>>()
            + 2 * size_of::<Site>()
            + 2 * size_of::<&Cell<Site>>();
        // remaining/floor get actual/formal receivers and returned usize values;
        // stage get has its own actual/formal receiver and Option return.
        let receipt_fields = 4 * size_of::<&Cell<usize>>()
            + 2 * size_of::<usize>()
            + 2 * size_of::<&Cell<Stage>>()
            + size_of::<Stage>();
        // Complete struct construction, Some(receipt) actual and set formal,
        // plus the first_refusal.set actual/formal receiver loans.
        let first_receipt = size_of::<super::ReservationRefusal>()
            + 2 * size_of::<Receipt>()
            + 2 * size_of::<&Cell<Receipt>>();
        // The empty-parent true and mismatch false are actual distinct literal
        // producers. Unit returns at record_refusal/Cell::set have zero width.
        let terminal_literals = 2 * size_of::<bool>();
        helper
            + tls
            + callback
            + borrow
            + option_loan
            + pattern
            + pointer_test
            + successful
            + mismatch
            + first_refusal_get
            + first_refusal_test
            + first_site
            + receipt_fields
            + first_receipt
            + terminal_literals
    };

    /// Actual new callers of current() prepay its cloned ACTIVE return/local,
    /// match tuple and arm receiver, TLS borrow, borrowed predicates and bools.
    /// Original current() below is unchanged; this inventory grants no authority.
    pub(crate) fn current_receiving_controls() -> usize {
        use std::cell::{Ref, RefCell};
        use std::mem::size_of;
        size_of::<&Self>()
            + 2 * size_of::<Option<Rc<Budget>>>()
            + size_of::<(&Option<Rc<Budget>>, Option<Rc<Budget>>)>()
            + size_of::<Rc<Budget>>()
            + size_of::<Ref<'_, Option<Rc<Budget>>>>()
            + size_of::<&RefCell<Option<Rc<Budget>>>>()
            + size_of::<&Option<Rc<Budget>>>()
            + 4 * size_of::<&Rc<Budget>>()
            + 2 * size_of::<bool>()
    }
    /// The insertion owner has two terminal call sites on the SAME held
    /// budget: current mismatch and reject_storage. Each may read first_refusal;
    /// only one can initialize its first receipt/site. No live payer is queried.
    pub(crate) fn insertion_terminal_controls() -> usize {
        use std::cell::Cell;
        use std::mem::size_of;
        type Receipt = Option<super::ReservationRefusal>;
        type Site = Option<&'static std::panic::Location<'static>>;
        size_of::<&Self>()
            + 2 * (size_of::<&Option<Rc<Budget>>>() + size_of::<&Rc<Budget>>()
                + size_of::<&Budget>() + size_of::<&Cell<bool>>())
            + 4 * size_of::<Receipt>() // two gets, one Some, one Cell::set receiver
            + size_of::<super::ReservationRefusal>()
            + size_of::<Option<super::ReservationStage>>()
            + size_of::<&'static std::panic::Location<'static>>()
            + 2 * size_of::<Site>()
            + 3 * size_of::<&Cell<Receipt>>()
            + size_of::<&Cell<Site>>()
            + 2 * size_of::<&Cell<usize>>()
            + size_of::<&Cell<Option<super::ReservationStage>>>()
    }
    /// Identity probe only: mismatch grants nothing and mutates neither ledger.
    /// A strict subset of the same-site current() receiving shapes; no new role.
    /// Real acquisition still authenticates its payer after this probe succeeds.
    pub(crate) fn current_identity(&self) -> bool {
        let current = active();
        match (&self.0, current) {
            (None, None) => true,
            (Some(held), Some(current)) => Rc::ptr_eq(held, &current),
            _ => false,
        }
    }
    pub(crate) fn current(&self) -> bool {
        let current = active();
        let same = match (&self.0, current) {
            (None, None) => true,
            (Some(held), Some(current)) => Rc::ptr_eq(held, &current),
            _ => false,
        };
        if !same {
            if let Some(held) = &self.0 {
                super::record_refusal(held, 0);
                held.overflowed.set(true);
            }
        }
        same
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod explicit;

mod inventory_work;

#[cfg(test)]
mod oracle_work;
