//! Closure-only memo copy: fixed slot bitmap records explicit empty success.
//! Ordinary Memo/RetainedMemo remain byte-exact independent owners.
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::mem::size_of;
const WIDTH: usize = 8;

struct Chunk<T> {
    values: Option<Box<[Option<T>; WIDTH]>>,
    empty: u8,
}
const _: () = assert!(WIDTH <= u8::BITS as usize);

pub(super) struct Memo<T> {
    chunks: Vec<Chunk<T>>,
    pub(super) binding: Option<Binding>,
    pub(super) failed: bool,
}
pub(super) struct Binding {
    pub(super) function: *const ResolvedFunction,
    pub(super) budget: *const ReplayBudget,
    pub(super) ledger: ClassifierLedger,
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
            let ledger_matches = binding.ledger.current_identity();
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
    // Identity-only preflight precedes acquiring a fresh invocation destination.
    // It neither asks a foreign payer nor constructs a diagnostic receiver.
    pub(super) fn preflight(&mut self, function: &ResolvedFunction, budget: &ReplayBudget) -> bool {
        if self.failed {
            return false;
        }
        if let Some(binding) = &self.binding {
            let ledger_matches = binding.ledger.current_identity();
            if binding.function != std::ptr::from_ref(function)
                || binding.budget != std::ptr::from_ref(budget)
                || !ledger_matches
            {
                self.failed = true;
            } else if budget.refusal_spent
                || (budget.pays_allocations() && crate::bounded_output::active_refused())
            {
                // Identity is already bound before reading this same account.
                // A warm poisoned owner must not consume its report or reset
                // work through a fresh zero-byte payer authentication.
                self.failed = true;
            }
            true
        } else {
            false
        }
    }
    // Canonical owned validation replaces its old retained-owner gateway with
    // the same arguments, bool result and receiving roles. A scope mismatch
    // refuses only the captured parent ledger, exactly as RetainedMemo and
    // ClosureReceiving::valid; it never reads or pays the foreign child.
    // Keep the separate identity-only API above unchanged for direct Memo users.
    pub(super) fn preflight_owned(
        &mut self,
        function: &ResolvedFunction,
        budget: &ReplayBudget,
    ) -> bool {
        if self.failed {
            return false;
        }
        if let Some(binding) = &self.binding {
            let ledger_matches = binding.ledger.current();
            if binding.function != std::ptr::from_ref(function)
                || binding.budget != std::ptr::from_ref(budget)
                || !ledger_matches
            {
                self.failed = true;
            } else if budget.refusal_spent
                || (budget.pays_allocations() && crate::bounded_output::active_refused())
            {
                // Identity is already bound before reading this same account.
                // A warm poisoned owner must not consume its report or reset
                // work through a fresh zero-byte payer authentication.
                self.failed = true;
            }
            true
        } else {
            false
        }
    }
    pub(super) fn unpublished(&self) -> bool {
        self.chunks.is_empty()
    }
    #[cfg(test)]
    pub(super) fn payload_chunks(&self) -> usize {
        self.chunks
            .iter()
            .filter(|chunk| chunk.values.is_some())
            .count()
    }
    pub(super) fn failed(&self) -> bool {
        self.failed
    }
    // Caller already holds the selected error; retain it unchanged while
    // invalidating views without querying any payer or allocating a report.
    pub(super) fn reject_after_error(&mut self) {
        self.failed = true;
    }
    pub(super) fn contains_key(&self, key: &usize) -> bool {
        self.state(key) != 0
    }
    // Replaces the old contains_key boolean result with a one-byte state code.
    // No owned Option/Rc/enum is transported by this borrowed metadata query.
    pub(super) fn state(&self, key: &usize) -> u8 {
        if self.failed {
            return 0;
        }
        match self.chunks.get(*key / WIDTH) {
            Some(chunk) => {
                if chunk
                    .values
                    .as_ref()
                    .is_some_and(|values| values[*key % WIDTH].is_some())
                {
                    2
                } else if chunk.empty & (1_u8 << (*key % WIDTH)) != 0 {
                    1
                } else {
                    0
                }
            }
            None => 0,
        }
    }
    pub(super) fn get(&self, key: &usize) -> Option<&T> {
        if self.failed {
            return None;
        }
        self.chunks
            .get(*key / WIDTH)?
            .values
            .as_ref()?
            .get(*key % WIDTH)?
            .as_ref()
    }
    #[cfg(test)]
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
    pub(super) fn insert_bound(
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
            // One actual inline slot initializer; receiving directory backing
            // is separately retained by reserve_vec above.
            budget.reserve_array::<Chunk<T>>(
                function,
                additional,
                "factored closure memo slot initializer",
            )?;
            self.chunks.resize_with(required, || Chunk {
                values: None,
                empty: 0,
            });
        }
        if self.chunks[index].values.is_none() {
            if budget.pays_allocations() {
                // Full array initializer and heap array, Box local and Some
                // initializer. No payload backing exists for empty-only slots.
                budget.reserve_bytes(
                    function,
                    2 * size_of::<[Option<T>; WIDTH]>() + 2 * size_of::<Box<[Option<T>; WIDTH]>>(),
                    "factored closure memo payload promotion",
                )?;
            }
            let values = Box::new(std::array::from_fn(|_| None));
            self.chunks[index].values = Some(values);
        }
        self.chunks[index].values.as_mut().expect("paid payload")[key % WIDTH] = Some(make());
        Ok(())
    }
    pub(super) fn insert_empty_bound(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
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
            // One actual inline slot initializer; receiving directory backing
            // is separately retained by reserve_vec above.
            budget.reserve_array::<Chunk<T>>(
                function,
                additional,
                "factored closure memo slot initializer",
            )?;
            self.chunks.resize_with(required, || Chunk {
                values: None,
                empty: 0,
            });
        }
        // The original empty bits survive later paid payload promotion.
        // Only unpublished keys reach this producer, exactly as before.
        self.chunks[index].empty |= 1_u8 << (key % WIDTH);
        Ok(())
    }
}

#[cfg(test)]
mod owned_tests;
