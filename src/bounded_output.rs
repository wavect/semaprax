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

    use super::{budgeted_join, with_limit, with_limit_usage, CappedString};

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
}
