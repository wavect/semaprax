//! One-use completeness reporting for the generic-instance key producer.
//! The legacy CappedString and every ordinary sink policy remain unchanged.
use super::{active, reserve_sink, Budget, ReservationRefusal, ReservationSite, ReservationStage};
use std::cell::{Cell, Ref, RefCell};
use std::fmt;
use std::mem::size_of;
use std::rc::Rc;

pub(crate) struct CheckedCappedString<'a> {
    bytes: String,
    budget: Option<Rc<Budget>>,
    complete: &'a mut bool,
}

impl<'a> CheckedCappedString<'a> {
    // Aggregate initializer, caller receiving owner, into_string consuming
    // owner; completion caller/parameter/field references and admission bool.
    pub(crate) const CONTROLS: usize =
        3 * size_of::<Self>() + 3 * size_of::<&mut bool>() + size_of::<bool>();

    // At most the first refused tracked write performs this transition. The
    // root completion flag is shared by every nominal writer in one comparison.
    // Inventory: active borrow/clone, selected Rc, required-refusal arguments,
    // record_refusal's first receipt/site construction and Cell receivers.
    pub(crate) const FIRST_FAILURE_CONTROLS: usize = 2 * size_of::<Option<Rc<Budget>>>()
        + size_of::<Rc<Budget>>()
        + size_of::<Ref<'static, Option<Rc<Budget>>>>()
        + 2 * size_of::<&RefCell<Option<Rc<Budget>>>>()
        + size_of::<&Ref<'static, Option<Rc<Budget>>>>()
        + size_of::<&Option<Rc<Budget>>>()
        + size_of::<&Rc<Budget>>()
        + size_of::<&Budget>()
        + 2 * size_of::<&Cell<Option<ReservationRefusal>>>()
        + size_of::<&Option<ReservationRefusal>>()
        + 3 * size_of::<Option<ReservationRefusal>>()
        + size_of::<ReservationRefusal>()
        + 2 * size_of::<Option<ReservationSite>>()
        + size_of::<ReservationSite>()
        + size_of::<&Cell<Option<ReservationSite>>>()
        + 2 * size_of::<&Cell<usize>>()
        + size_of::<&Cell<Option<ReservationStage>>>()
        + size_of::<Option<ReservationStage>>()
        + size_of::<&Cell<bool>>()
        + 3 * size_of::<bool>()
        + 3 * size_of::<usize>()
        + size_of::<&str>();

    // The checked formatter also replaces budgeted_format's direct reserve
    // writer. Its reserve_sink call has an independent active/captured payer
    // selection: caller/parameter captured Option and reference, length,
    // active return/receiving Option<Rc> values and Rc clone, borrow/Ref views,
    // Rc/Option references, as_deref/or_else/map receiver/return Options and
    // closure capture/receiver, plus the forwarding bool. Charge this selection
    // for EVERY checked callback; no guessed callback count or reused old bank.
    const SINK_SELECTION_CONTROLS: usize = 3 * size_of::<Option<Rc<Budget>>>()
        + size_of::<Rc<Budget>>()
        + size_of::<Ref<'static, Option<Rc<Budget>>>>()
        + 2 * size_of::<&RefCell<Option<Rc<Budget>>>>()
        + size_of::<&Ref<'static, Option<Rc<Budget>>>>()
        + 3 * size_of::<&Option<Rc<Budget>>>()
        + 2 * size_of::<&Rc<Budget>>()
        + 2 * size_of::<Option<&Rc<Budget>>>()
        + 3 * size_of::<Option<&Budget>>()
        + 3 * size_of::<&Option<&Rc<Budget>>>()
        + size_of::<usize>()
        + size_of::<bool>();

    // Successful extra required-entry admission, in addition to the surviving
    // original reserve_sink byte call: active borrow/clone and Option views,
    // outer overflow/admitted/caller bools (9), reserve overflow bools (3),
    // optional zero/and/length predicates (3), floor predicate/closure (3),
    // optional/forwarded returns (2). No byte-call receiver is reused as credit.
    // Positive nested reserve_optional still owns checked_add's two Option
    // values, remaining/floor/set Cell references and scalar receiving sites.
    // Its own failed admission is the explicit initial metadata ABI boundary;
    // no failed debit is described as having prepaid its own capture.
    const ENTRY_CONTROLS: usize = Self::SINK_SELECTION_CONTROLS
        + 2 * size_of::<Option<Rc<Budget>>>()
        + size_of::<Rc<Budget>>()
        + size_of::<Ref<'static, Option<Rc<Budget>>>>()
        + 2 * size_of::<&RefCell<Option<Rc<Budget>>>>()
        + size_of::<&Ref<'static, Option<Rc<Budget>>>>()
        + 3 * size_of::<&Option<Rc<Budget>>>()
        + 2 * size_of::<Option<&Rc<Budget>>>()
        + 2 * size_of::<&Rc<Budget>>()
        + 3 * size_of::<&Budget>()
        + 5 * size_of::<Option<&Budget>>()
        + 2 * size_of::<Option<usize>>()
        + 3 * size_of::<&Cell<usize>>()
        + 2 * size_of::<&Cell<bool>>()
        + 20 * size_of::<bool>()
        + 12 * size_of::<usize>();

    pub(crate) fn new(complete: &'a mut bool) -> Self {
        Self {
            bytes: String::new(),
            budget: active(),
            complete,
        }
    }

    #[cfg(test)]
    pub(crate) fn capture_write_frontier<T>(bytes: usize, body: impl FnOnce() -> T) -> (T, usize) {
        frontier_observer::capture(bytes, body)
    }

    pub(crate) fn into_string(self) -> String {
        self.bytes
    }
}

impl fmt::Write for CheckedCappedString<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        // No new predicate is repeatedly borrowed from an exhausted one-use
        // failure bank. Every actual callback admits its new entry controls.
        // Failure still returns Ok, preserving later formatter/key invocations.
        if !super::reserve_active_required(Self::ENTRY_CONTROLS) {
            *self.complete = false;
            return Ok(());
        }
        #[cfg(test)]
        frontier_observer::record(value.len());
        // Same original current-or-captured sink selection and byte request.
        if reserve_sink(self.budget.as_ref(), value.len()) {
            self.bytes.push_str(value);
        } else {
            // The existing absolute-refusal receipt wins. A silent reserved-
            // floor refusal is made required at the actual omitted write.
            // identity_key performs no budget switch/suspension: the active
            // payer is the same one selected by reserve_sink immediately above.
            super::refuse_active_required(value.len());
            *self.complete = false;
        }
        // Preserve every later formatter invocation and argument key producer.
        Ok(())
    }
}

// Diagnostic-only exact write frontier. No allocation, reservation, floor change,
// or production field; it observes after real ENTRY admission and before bytes.
#[cfg(test)]
pub(crate) mod frontier_observer {
    use std::cell::Cell;
    thread_local! {
        static TARGET: Cell<Option<usize>> = const { Cell::new(None) };
        static FIRST: Cell<Option<usize>> = const { Cell::new(None) };
    }
    pub(super) fn record(bytes: usize) {
        if TARGET.with(Cell::get) == Some(bytes) && FIRST.with(Cell::get).is_none() {
            FIRST.with(|first| first.set(crate::bounded_output::active_remaining()));
        }
    }
    pub(crate) fn capture<T>(bytes: usize, body: impl FnOnce() -> T) -> (T, usize) {
        struct Restore(Option<usize>, Option<usize>);
        impl Drop for Restore {
            fn drop(&mut self) {
                TARGET.with(|target| target.set(self.0));
                FIRST.with(|first| first.set(self.1));
            }
        }
        let restore = Restore(
            TARGET.with(|target| target.replace(Some(bytes))),
            FIRST.with(|first| first.replace(None)),
        );
        let value = body();
        let remaining = FIRST
            .with(Cell::get)
            .expect("the complete admitted write frontier was reached");
        drop(restore);
        (value, remaining)
    }
}
