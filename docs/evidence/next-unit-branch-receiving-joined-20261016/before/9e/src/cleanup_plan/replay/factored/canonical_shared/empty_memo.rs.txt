//! Closure-only memo copy: fixed slot bitmap records explicit empty success.
//! Ordinary Memo/RetainedMemo remain byte-exact independent owners.
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::mem::size_of;
const WIDTH: usize = 8;

struct Chunk<T> {
    values: [Option<T>; WIDTH],
    empty: u8,
}
const _: () = assert!(WIDTH <= u8::BITS as usize);

pub(super) struct Memo<T> {
    chunks: Vec<Option<Box<Chunk<T>>>>,
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
        self.state(key) != 0
    }
    // Replaces the old contains_key boolean result with a one-byte state code.
    // No owned Option/Rc/enum is transported by this borrowed metadata query.
    pub(super) fn state(&self, key: &usize) -> u8 {
        if self.failed {
            return 0;
        }
        match self.chunks.get(*key / WIDTH).and_then(Option::as_ref) {
            Some(chunk) => {
                if chunk.values[*key % WIDTH].is_some() {
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
            .as_ref()?
            .values
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
                // Inline Chunk initializer + boxed Chunk backing, the distinct
                // from_fn array initializer, and named Box pointer. Directory
                // Option<Box> receiving storage is paid by reserve_vec.
                budget.reserve_bytes(
                    function,
                    2 * size_of::<Chunk<T>>()
                        + size_of::<[Option<T>; WIDTH]>()
                        + size_of::<Box<Chunk<T>>>(),
                    "factored closure memo chunk",
                )?;
            }
            let chunk = Box::new(Chunk {
                values: std::array::from_fn(|_| None),
                empty: 0,
            });
            self.chunks[index] = Some(chunk);
        }
        self.chunks[index].as_mut().expect("paid chunk").values[key % WIDTH] = Some(make());
        Ok(())
    }
    fn insert_empty_bound(
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
            self.chunks.resize_with(required, || None);
        }
        if self.chunks[index].is_none() {
            if budget.pays_allocations() {
                // Inline Chunk initializer + boxed Chunk backing, the distinct
                // from_fn array initializer, and named Box pointer. Directory
                // Option<Box> receiving storage is paid by reserve_vec.
                budget.reserve_bytes(
                    function,
                    2 * size_of::<Chunk<T>>()
                        + size_of::<[Option<T>; WIDTH]>()
                        + size_of::<Box<Chunk<T>>>(),
                    "factored closure memo chunk",
                )?;
            }
            let chunk = Box::new(Chunk {
                values: std::array::from_fn(|_| None),
                empty: 0,
            });
            self.chunks[index] = Some(chunk);
        }
        // Only !contains_key nodes reach publication in the postorder producer;
        // no value is overwritten and no extra assertion lookup is constructed.
        self.chunks[index].as_mut().expect("paid chunk").empty |= 1_u8 << (key % WIDTH);
        Ok(())
    }
}

#[path = "empty_memo/retained.rs"]
mod retained;
pub(super) use retained::RetainedMemo;
