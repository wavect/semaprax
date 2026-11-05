//! Lazy numeric closure memo. Sparse high keys still require gap directory slots.
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::mem::size_of;
const WIDTH: usize = 8;

pub(super) struct Memo<T> {
    chunks: Vec<Option<Box<[Option<T>; WIDTH]>>>,
    binding: Option<Binding>,
    failed: bool,
}
struct Binding {
    function: *const ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
}
impl<T> Memo<T> {
    pub(super) fn new() -> Self {
        Self {
            chunks: Vec::new(),
            binding: None,
            failed: false,
        }
    }
    pub(super) fn authenticate(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        if let Some(binding) = &self.binding {
            // Read no foreign payer, even to construct a refusal report. The
            // exact caller keeps both borrowed receivers at stable addresses.
            let ledger_matches = binding.ledger.current();
            if binding.function != std::ptr::from_ref(function)
                || binding.budget != std::ptr::from_ref(budget)
                || !ledger_matches
            {
                self.failed = true;
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
        }
        if let Err(error) =
            budget.reserve_bytes(function, 0, "factored closure memo authentication")
        {
            self.failed = true;
            return Err(error);
        }
        if budget.pays_allocations() {
            if let Err(error) = budget.reserve_bytes(
                function,
                size_of::<Result<(), Diagnostic>>(),
                "factored closure memo authorization receipt",
            ) {
                self.failed = true;
                return Err(error);
            }
        }
        if self.binding.is_none() {
            // One live Memo receiver (including its binding slot) and the
            // separately named Option<Binding> initializer below.
            // The old Canonicalizer constructor created only allocation-free
            // inert fields; activation pays its new inline receiver once.
            if budget.pays_allocations() {
                if let Err(error) = budget.reserve_bytes(
                    function,
                    size_of::<Self>() + size_of::<Option<Binding>>(),
                    "factored closure memo controls",
                ) {
                    self.failed = true;
                    return Err(error);
                }
            }
            let binding = Some(Binding {
                function: std::ptr::from_ref(function),
                budget: std::ptr::from_ref(budget),
                ledger: ClassifierLedger::capture(),
            });
            self.binding = binding;
        }
        Ok(())
    }
    // Caller already holds the selected error; retain it unchanged while
    // invalidating views without querying any payer or allocating a report.
    pub(super) fn reject_after_error(&mut self) {
        self.failed = true;
    }
    pub(super) fn contains_key(&self, key: &usize) -> bool {
        self.get(key).is_some()
    }
    pub(super) fn get(&self, key: &usize) -> Option<&T> {
        if self.failed {
            return None;
        }
        self.chunks
            .get(*key / WIDTH)?
            .as_ref()?
            .get(*key % WIDTH)?
            .as_ref()
    }
    pub(super) fn insert(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
        make: impl FnOnce() -> T,
    ) -> Result<(), Diagnostic> {
        self.authenticate(function, budget)?;
        let result = self.insert_bound(function, key, budget, make);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn insert_bound(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
        make: impl FnOnce() -> T,
    ) -> Result<(), Diagnostic> {
        let index = key / WIDTH;
        // WIDTH > 1, so quotient + 1 cannot overflow usize.
        let required = index + 1;
        if self.chunks.len() < required {
            let additional = required - self.chunks.len();
            budget.reserve_vec(
                function,
                &mut self.chunks,
                additional,
                "factored closure memo directory",
            )?;
            self.chunks.resize_with(required, || None);
        }
        if self.chunks[index].is_none() {
            if budget.pays_allocations() {
                // Named array initializer + its simultaneously allocated boxed
                // array backing + named Box pointer. The directory's receiving
                // Option<Box> slot was already paid by reserve_vec, not again.
                budget.reserve_bytes(
                    function,
                    2 * size_of::<[Option<T>; WIDTH]>() + size_of::<Box<[Option<T>; WIDTH]>>(),
                    "factored closure memo chunk",
                )?;
            }
            let values: [Option<T>; WIDTH] = std::array::from_fn(|_| None);
            let chunk = Box::new(values);
            self.chunks[index] = Some(chunk);
        }
        self.chunks[index].as_mut().expect("paid chunk")[key % WIDTH] = Some(make());
        Ok(())
    }
}
