use std::cell::{Cell, RefCell};
use std::fmt::{self, Write as _};
use std::ops::Deref;
use std::rc::Rc;

struct Budget {
    initial: usize,
    remaining: Cell<usize>,
    floor: Cell<usize>,
    overflowed: Cell<bool>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Rc<Budget>>> = const { RefCell::new(None) };
}

pub(crate) fn with_limit<T>(limit: usize, operation: impl FnOnce() -> T) -> (T, bool) {
    let (value, overflowed, _) = with_limit_usage(limit, operation);
    (value, overflowed)
}

pub(crate) fn with_limit_usage<T>(limit: usize, operation: impl FnOnce() -> T) -> (T, bool, usize) {
    struct Restore {
        previous: Option<Rc<Budget>>,
        current: Rc<Budget>,
        initial: usize,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            let consumed = self.initial.saturating_sub(self.current.remaining.get());
            let previous = self.previous.take();
            ACTIVE.with(|active| {
                active.replace(previous.clone());
            });
            if let Some(parent) = previous {
                if !reserve(Some(&parent), consumed) {
                    // A refused reservation normally means nothing was
                    // written, but here the child already emitted the bytes:
                    // they cannot be un-spent. Charge the parent anyway, so
                    // `consumed` and `active_remaining` stay truthful, and
                    // report the refusal.
                    let remaining = parent.remaining.get();
                    parent.remaining.set(remaining.saturating_sub(consumed));
                    parent.overflowed.set(true);
                }
            }
        }
    }
    let parent = active();
    let effective_limit = parent
        .as_ref()
        .map_or(limit, |budget| limit.min(budget.remaining.get()));
    let budget = Rc::new(Budget {
        initial: effective_limit,
        remaining: Cell::new(effective_limit),
        floor: Cell::new(0),
        overflowed: Cell::new(false),
    });
    let previous = ACTIVE.with(|active| active.replace(Some(Rc::clone(&budget))));
    let restore = Restore {
        previous,
        current: Rc::clone(&budget),
        initial: effective_limit,
    };
    let value = operation();
    let overflowed = budget.overflowed.get();
    let consumed = effective_limit.saturating_sub(budget.remaining.get());
    drop(restore);
    (value, overflowed, consumed)
}

fn active() -> Option<Rc<Budget>> {
    ACTIVE.with(|active| active.borrow().clone())
}

fn reserve(budget: Option<&Budget>, length: usize) -> bool {
    let Some(budget) = budget else {
        return true;
    };
    let remaining = budget.remaining.get();
    if length > remaining {
        budget.overflowed.set(true);
        return false;
    }
    if length
        .checked_add(budget.floor.get())
        .is_none_or(|required| required > remaining)
    {
        return false;
    }
    budget.remaining.set(remaining - length);
    true
}

pub(crate) fn reserve_active(length: usize) -> bool {
    let budget = active();
    reserve(budget.as_deref(), length)
}

pub(crate) fn reserve_active_preserving(length: usize, floor: usize) -> bool {
    let budget = active();
    let Some(budget) = budget.as_deref() else {
        return true;
    };
    let floor = floor.max(budget.floor.get());
    let Some(required) = length.checked_add(floor) else {
        return false;
    };
    if required > budget.remaining.get() {
        return false;
    }
    reserve(Some(budget), length)
}

pub(crate) fn set_active_floor(floor: usize) -> bool {
    let Some(budget) = active() else {
        return true;
    };
    if floor > budget.remaining.get() {
        return false;
    }
    budget.floor.set(floor);
    true
}

pub(crate) fn clear_active_floor() {
    if let Some(budget) = active() {
        budget.floor.set(0);
    }
}

pub(crate) fn active_remaining() -> Option<usize> {
    active().map(|budget| budget.remaining.get())
}

pub(crate) fn active_limit() -> Option<usize> {
    active().map(|budget| budget.initial)
}

fn reserve_sink(captured: Option<&Rc<Budget>>, length: usize) -> bool {
    let current = active();
    reserve(
        current.as_deref().or_else(|| captured.map(Rc::as_ref)),
        length,
    )
}

pub(crate) fn budgeted_format(arguments: fmt::Arguments<'_>) -> String {
    struct Counter(usize);
    impl fmt::Write for Counter {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.0 = self.0.saturating_add(value.len());
            Ok(())
        }
    }
    let mut counter = Counter(0);
    let _ = counter.write_fmt(arguments);
    let budget = active();
    if !reserve(budget.as_deref(), counter.0) {
        return String::new();
    }
    let mut output = String::with_capacity(counter.0);
    let _ = output.write_fmt(arguments);
    output
}

pub(crate) fn budgeted_join(values: impl IntoIterator<Item = String>, separator: &str) -> String {
    let values = values.into_iter().collect::<Vec<_>>();
    join_slices(&values, separator)
}

pub(crate) fn budgeted_clone(value: &str) -> String {
    let budget = active();
    if !reserve(budget.as_deref(), value.len()) {
        return String::new();
    }
    value.to_owned()
}

pub(crate) trait BudgetedJoin {
    fn budgeted_join(&self, separator: &str) -> String;
}

impl<T: AsRef<str>> BudgetedJoin for [T] {
    fn budgeted_join(&self, separator: &str) -> String {
        join_slices(self, separator)
    }
}

fn join_slices<T: AsRef<str>>(values: &[T], separator: &str) -> String {
    let length = values
        .iter()
        .try_fold(0usize, |length, value| {
            length.checked_add(value.as_ref().len())
        })
        .and_then(|length| {
            separator
                .len()
                .checked_mul(values.len().saturating_sub(1))
                .and_then(|separators| length.checked_add(separators))
        });
    let budget = active();
    let Some(length) = length else {
        if let Some(budget) = budget {
            budget.overflowed.set(true);
        }
        return String::new();
    };
    if !reserve(budget.as_deref(), length) {
        return String::new();
    }
    let mut output = String::with_capacity(length);
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str(separator);
        }
        output.push_str(value.as_ref());
    }
    output
}

pub(crate) struct CappedString {
    bytes: String,
    budget: Option<Rc<Budget>>,
}

impl CappedString {
    pub(crate) fn new() -> Self {
        Self {
            bytes: String::new(),
            budget: active(),
        }
    }

    #[cfg(test)]
    pub(crate) fn allocated_capacity(&self) -> usize {
        self.bytes.capacity()
    }

    pub(crate) fn push_str(&mut self, value: &str) {
        if reserve_sink(self.budget.as_ref(), value.len()) {
            self.bytes.push_str(value);
        }
    }

    pub(crate) fn push(&mut self, value: char) {
        if reserve_sink(self.budget.as_ref(), value.len_utf8()) {
            self.bytes.push(value);
        }
    }

    pub(crate) fn into_string(self) -> String {
        self.bytes
    }
}

impl fmt::Write for CappedString {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.push_str(value);
        Ok(())
    }
}

pub(crate) struct CappedVec {
    bytes: Vec<u8>,
    budget: Option<Rc<Budget>>,
}

impl CappedVec {
    pub(crate) fn new() -> Self {
        Self {
            bytes: Vec::new(),
            budget: active(),
        }
    }

    pub(crate) fn from_slice(value: &[u8]) -> Self {
        let mut output = Self::new();
        output.extend_from_slice(value);
        output
    }

    pub(crate) fn push(&mut self, value: u8) {
        if reserve_sink(self.budget.as_ref(), 1) {
            self.bytes.push(value);
        }
    }

    pub(crate) fn extend<const N: usize>(&mut self, values: [u8; N]) {
        self.extend_from_slice(&values);
    }

    pub(crate) fn extend_from_slice(&mut self, values: &[u8]) {
        if reserve_sink(self.budget.as_ref(), values.len()) {
            self.bytes.extend_from_slice(values);
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.bytes
    }
}

impl Deref for CappedVec {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    use super::{
        active_limit, active_remaining, budgeted_clone, budgeted_format, budgeted_join,
        clear_active_floor, reserve_active, reserve_active_preserving, set_active_floor,
        with_limit, with_limit_usage, BudgetedJoin as _, CappedString, CappedVec,
    };

    #[test]
    fn exact_limit_succeeds_and_over_limit_fails_closed() {
        let (exact, overflowed) = with_limit(4, || {
            let mut output = CappedString::new();
            output.push_str("four");
            output.into_string()
        });
        assert_eq!(exact, "four");
        assert!(!overflowed);

        let (over, overflowed) = with_limit(3, || {
            let mut output = CappedString::new();
            output.push_str("four");
            output.into_string()
        });
        assert_eq!(over, "");
        assert!(overflowed);
    }

    #[test]
    fn join_length_is_checked_and_budgeted() {
        let (joined, overflowed) =
            with_limit(5, || budgeted_join(["ab".to_owned(), "cd".to_owned()], ","));
        assert_eq!(joined, "ab,cd");
        assert!(!overflowed);

        let (joined, overflowed) =
            with_limit(4, || budgeted_join(["ab".to_owned(), "cd".to_owned()], ","));
        assert_eq!(joined, "");
        assert!(overflowed);
    }

    #[test]
    fn nested_budget_debits_parent_and_parent_sink_uses_child() {
        let (_, parent_overflowed) = with_limit(5, || {
            let mut parent_sink = CappedString::new();
            let (_, child_overflowed) = with_limit(3, || parent_sink.push_str("four"));
            assert!(child_overflowed);
            parent_sink.push_str("five!");
            assert_eq!(parent_sink.into_string(), "five!");
        });
        assert!(!parent_overflowed);

        let (_, parent_overflowed) = with_limit(5, || {
            let (_, child_overflowed) = with_limit(4, || {
                let mut output = CappedString::new();
                output.push_str("four");
            });
            assert!(!child_overflowed);
            let mut output = CappedString::new();
            output.push_str("xx");
        });
        assert!(parent_overflowed);
    }

    #[test]
    fn panic_restores_and_debits_parent_budget() {
        let (_, parent_overflowed) = with_limit(5, || {
            let result = catch_unwind(AssertUnwindSafe(|| {
                let _ = with_limit(4, || {
                    let mut output = CappedString::new();
                    output.push_str("four");
                    panic!("injected");
                });
            }));
            assert!(result.is_err());
            let mut output = CappedString::new();
            output.push_str("xx");
        });
        assert!(parent_overflowed);
    }

    #[test]
    fn usage_reports_exact_debits_and_preserves_nested_restoration() {
        let ((child_overflowed, child_used), parent_overflowed, parent_used) =
            with_limit_usage(7, || {
                let (_, child_overflowed, child_used) = with_limit_usage(4, || {
                    let mut output = CappedString::new();
                    output.push_str("abc");
                });
                let mut output = CappedString::new();
                output.push_str("defg");
                (child_overflowed, child_used)
            });
        assert!(!child_overflowed);
        assert_eq!(child_used, 3);
        assert!(!parent_overflowed);
        assert_eq!(parent_used, 7);

        let (_, overflowed, used) = with_limit_usage(2, || {
            let mut output = CappedString::new();
            output.push_str("abc");
        });
        assert!(overflowed);
        assert_eq!(used, 0);

        let (_, overflowed, used) = with_limit_usage(3, || {
            let mut output = CappedString::new();
            output.push_str("xyz");
        });
        assert!(!overflowed);
        assert_eq!(used, 3);
    }

    #[test]
    fn zero_and_one_byte_budgets_admit_only_what_fits() {
        // A zero budget still admits an empty write: nothing is spent, so
        // nothing overflows. The first byte is what fails.
        let (empty, overflowed, used) = with_limit_usage(0, || {
            let mut output = CappedString::new();
            output.push_str("");
            output.into_string()
        });
        assert_eq!(empty, "");
        assert!(!overflowed);
        assert_eq!(used, 0);

        let (nothing, overflowed) = with_limit(0, || {
            let mut output = CappedString::new();
            output.push('a');
            output.into_string()
        });
        assert_eq!(nothing, "");
        assert!(overflowed);

        let (one, overflowed, used) = with_limit_usage(1, || {
            let mut output = CappedString::new();
            output.push('a');
            output.into_string()
        });
        assert_eq!(one, "a");
        assert!(!overflowed);
        assert_eq!(used, 1);

        let (over, overflowed) = with_limit(1, || {
            let mut output = CappedString::new();
            output.push_str("ab");
            output.into_string()
        });
        assert_eq!(over, "");
        assert!(overflowed);
    }

    #[test]
    fn budgets_count_bytes_and_never_split_a_code_point() {
        // `é` is two bytes and `€` is three: a three-byte budget admits the
        // first and must refuse the second whole rather than write a prefix
        // of its encoding.
        let (output, overflowed) = with_limit(3, || {
            let mut output = CappedString::new();
            output.push('é');
            output.push('€');
            output.into_string()
        });
        assert_eq!(output, "é");
        assert_eq!(output.len(), 2);
        assert!(overflowed);
        assert_eq!(output.chars().count(), 1);

        // Two characters, four bytes: the budget is spent in bytes, so a
        // three-byte budget refuses the string outright.
        let (output, overflowed) = with_limit(3, || {
            let mut output = CappedString::new();
            output.push_str("éé");
            output.into_string()
        });
        assert_eq!(output, "");
        assert!(overflowed);

        // A four-byte code point does not fit a three-byte budget at all.
        let (output, overflowed) = with_limit(3, || {
            let mut output = CappedString::new();
            output.push('\u{1f600}');
            output.into_string()
        });
        assert_eq!(output, "");
        assert!(overflowed);

        let (output, overflowed, used) = with_limit_usage(4, || {
            let mut output = CappedString::new();
            output.push('\u{1f600}');
            output.into_string()
        });
        assert_eq!(output, "\u{1f600}");
        assert!(!overflowed);
        assert_eq!(used, 4);
    }

    #[test]
    fn capped_vec_writes_are_all_or_nothing_and_leave_the_budget_usable() {
        let (bytes, overflowed) = with_limit(5, || {
            let mut output = CappedVec::new();
            // Six bytes into five: the whole slice is refused, not a prefix.
            output.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
            assert!(output.is_empty());
            // The refusal spent nothing, so the budget still admits writes.
            output.extend([7, 8]);
            output.push(9);
            assert_eq!(output.len(), 3);
            assert_eq!(&output[..], &[7u8, 8, 9][..]);
            output.into_vec()
        });
        assert_eq!(bytes, vec![7, 8, 9]);
        assert!(overflowed);

        let (bytes, overflowed, used) =
            with_limit_usage(3, || CappedVec::from_slice(&[1, 2, 3]).into_vec());
        assert_eq!(bytes, vec![1, 2, 3]);
        assert!(!overflowed);
        assert_eq!(used, 3);

        let (bytes, overflowed) = with_limit(2, || CappedVec::from_slice(&[1, 2, 3]).into_vec());
        assert!(bytes.is_empty());
        assert!(overflowed);
    }

    #[test]
    fn a_floor_reserves_a_trailer_lane_without_reporting_overflow() {
        let (_, overflowed, used) = with_limit_usage(10, || {
            assert!(set_active_floor(6));
            // Five bytes fit the remaining ten but would eat into the six
            // reserved for the trailer, so the reservation is refused. That
            // refusal is a lane boundary, not a budget overflow.
            assert!(!reserve_active(5));
            assert_eq!(active_remaining(), Some(10));
            assert!(reserve_active(4));
            assert_eq!(active_remaining(), Some(6));
            assert!(!reserve_active(1));

            clear_active_floor();
            assert!(reserve_active(6));
            assert_eq!(active_remaining(), Some(0));
        });
        assert!(!overflowed);
        assert_eq!(used, 10);

        // A floor larger than what remains is refused and leaves the previous
        // floor in place, so the caller cannot silently arm an unsatisfiable
        // reservation.
        let (_, overflowed) = with_limit(10, || {
            assert!(!set_active_floor(11));
            assert!(reserve_active(10));
        });
        assert!(!overflowed);
    }

    #[test]
    fn preserving_reservations_check_a_trailer_without_arming_the_floor() {
        let (_, overflowed) = with_limit(7, || {
            // Three bytes plus a five-byte trailer exceed seven, so the write
            // is refused and nothing is spent.
            assert!(!reserve_active_preserving(3, 5));
            assert_eq!(active_remaining(), Some(7));
            assert!(reserve_active_preserving(2, 5));
            assert_eq!(active_remaining(), Some(5));
            // The declared trailer was a per-call check, not a persistent
            // floor: an ordinary reservation may still spend the remainder.
            assert!(reserve_active(5));
            assert_eq!(active_remaining(), Some(0));
        });
        assert!(!overflowed);
    }

    #[test]
    fn reservations_outside_any_budget_always_succeed() {
        assert_eq!(active_remaining(), None);
        assert_eq!(active_limit(), None);
        assert!(reserve_active(usize::MAX));
        assert!(reserve_active_preserving(usize::MAX, usize::MAX));
        assert!(set_active_floor(usize::MAX));
        clear_active_floor();
        assert_eq!(budgeted_clone("unbounded"), "unbounded");
    }

    #[test]
    fn a_nested_limit_is_clamped_to_what_the_parent_has_left() {
        let (inner, overflowed) = with_limit(5, || {
            let mut output = CappedString::new();
            output.push_str("ab");
            // The child asks for a hundred bytes but may not exceed the three
            // its parent still holds.
            with_limit(100, || (active_limit(), active_remaining())).0
        });
        assert_eq!(inner, (Some(3), Some(3)));
        assert!(!overflowed);

        // The clamp is real, not merely reported: the child cannot emit more
        // than the parent's remainder, and its refusal is reported to the
        // child's own caller without spending parent bytes.
        let ((child, child_overflowed), overflowed, used) = with_limit_usage(5, || {
            let mut parent = CappedString::new();
            parent.push_str("ab");
            with_limit(100, || {
                let mut output = CappedString::new();
                output.push_str("cdef");
                output.into_string()
            })
        });
        assert_eq!(child, "");
        assert!(child_overflowed);
        assert!(!overflowed);
        assert_eq!(used, 2);
    }

    #[test]
    fn a_child_that_would_consume_the_parents_floor_fails_the_parent_closed() {
        let (_, overflowed) = with_limit(10, || {
            assert!(set_active_floor(8));
            let (_, child_overflowed) = with_limit(4, || {
                let mut output = CappedString::new();
                output.push_str("abcd");
            });
            // The child stayed inside its own limit, so it reports no
            // overflow; the parent debits the child on scope exit, discovers
            // the bytes would eat its reserved trailer, and fails closed.
            assert!(!child_overflowed);
        });
        assert!(overflowed);
    }

    #[test]
    fn a_child_that_breaches_the_parents_floor_is_still_charged_to_the_parent() {
        // Failing closed is only half the accounting. The child has already
        // emitted its bytes by the time the parent debits them, so a refused
        // debit cannot un-spend them: the parent must lose them as well as
        // overflow, or `consumed` undercounts and `active_remaining` overstates
        // what a caller may still write.
        let (_, overflowed, used) = with_limit_usage(10, || {
            assert!(set_active_floor(8));
            let (emitted, child_overflowed, child_used) = with_limit_usage(4, || {
                let mut output = CappedString::new();
                output.push_str("abcd");
                output.into_string()
            });
            assert_eq!(emitted, "abcd");
            assert!(!child_overflowed);
            assert_eq!(child_used, 4);
            assert_eq!(active_remaining(), Some(6));
        });
        assert!(overflowed);
        assert_eq!(used, 4);

        // The debited remainder is real rather than a bookkeeping entry: once
        // the trailer lane is released the parent may spend exactly the six
        // bytes it has left and not one more.
        let (_, overflowed, used) = with_limit_usage(10, || {
            assert!(set_active_floor(8));
            let (_, child_overflowed) = with_limit(4, || {
                let mut output = CappedString::new();
                output.push_str("abcd");
            });
            assert!(!child_overflowed);
            clear_active_floor();
            assert!(!reserve_active(7));
            assert!(reserve_active(6));
            assert_eq!(active_remaining(), Some(0));
        });
        assert!(overflowed);
        assert_eq!(used, 10);
    }

    #[test]
    fn identical_input_and_budget_produce_identical_output_after_an_overflow() {
        fn render(limit: usize) -> (String, bool) {
            with_limit(limit, || {
                let mut output = CappedString::new();
                output.push_str(&budgeted_format(format_args!("{}-{}", "café", 42)));
                output.push('!');
                output.into_string()
            })
        }

        let first = render(20);
        let (partial, flagged) = render(3);
        assert!(flagged);
        assert!(partial.len() <= 3, "{partial:?} exceeded its budget");
        // A prior overflow must not leave thread-local budget state behind.
        assert_eq!(active_remaining(), None);
        let second = render(20);
        assert_eq!(first.0.as_bytes(), second.0.as_bytes());
        assert_eq!(first.1, second.1);
        assert!(!first.1);
    }

    #[test]
    fn formatting_and_cloning_charge_exact_byte_lengths() {
        // `café-42` is eight bytes: the accented byte counts, and the exact
        // budget succeeds while one byte less refuses the whole string.
        let (formatted, overflowed, used) =
            with_limit_usage(8, || budgeted_format(format_args!("{}-{}", "café", 42)));
        assert_eq!(formatted, "café-42");
        assert!(!overflowed);
        assert_eq!(used, 8);

        let (formatted, overflowed) =
            with_limit(7, || budgeted_format(format_args!("{}-{}", "café", 42)));
        assert_eq!(formatted, "");
        assert!(overflowed);

        let (cloned, overflowed, used) = with_limit_usage(3, || budgeted_clone("abc"));
        assert_eq!(cloned, "abc");
        assert!(!overflowed);
        assert_eq!(used, 3);

        let (cloned, overflowed) = with_limit(2, || budgeted_clone("abc"));
        assert_eq!(cloned, "");
        assert!(overflowed);
    }

    #[test]
    fn joins_charge_one_separator_fewer_than_their_elements() {
        let (joined, overflowed, used) =
            with_limit_usage(0, || budgeted_join(Vec::<String>::new(), ",,,,"));
        assert_eq!(joined, "");
        assert!(!overflowed);
        assert_eq!(used, 0);

        // A single element pays for no separator at all.
        let (joined, overflowed, used) =
            with_limit_usage(3, || budgeted_join(["abc".to_owned()], ",,,,"));
        assert_eq!(joined, "abc");
        assert!(!overflowed);
        assert_eq!(used, 3);

        let (joined, overflowed, used) = with_limit_usage(4, || ["a", "b"].budgeted_join("--"));
        assert_eq!(joined, "a--b");
        assert!(!overflowed);
        assert_eq!(used, 4);

        let (joined, overflowed) = with_limit(3, || ["a", "b"].budgeted_join("--"));
        assert_eq!(joined, "");
        assert!(overflowed);
    }
}
