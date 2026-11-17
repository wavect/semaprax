//! Full commented formatting into a borrowed comparison, without final backing.
use super::{Comments, Placement};
use crate::ast::Program;

pub(crate) struct Comparison {
    pub(crate) bytes: Option<usize>,
    pub(crate) matches: bool,
}

struct Compare<'a> {
    expected: &'a [u8],
    offset: usize,
    total: Option<usize>,
    mismatch: bool,
}
impl<'a> Compare<'a> {
    fn new(expected: &'a str) -> Self {
        Self {
            expected: expected.as_bytes(),
            offset: 0,
            total: Some(0),
            mismatch: false,
        }
    }
    fn finish(self) -> Comparison {
        Comparison {
            bytes: self.total,
            matches: !self.mismatch && self.total == Some(self.expected.len()),
        }
    }
}
impl std::fmt::Write for Compare<'_> {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.total = self.total.and_then(|total| total.checked_add(text.len()));
        if let Some(end) = self.offset.checked_add(text.len()) {
            if self.expected.get(self.offset..end) != Some(text.as_bytes()) {
                self.mismatch = true;
            }
            self.offset = end;
        } else {
            self.mismatch = true;
        }
        // Mismatch/overflow are observations, never formatter cancellation.
        // Every remaining placement hook and formatter write must execute.
        Ok(())
    }
}

// The owning cold builder supplies both arms' exact current report controls.
#[cfg(test)]
thread_local! {
    static MATERIALIZED_CONTROL: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn with_materialized_control<T>(operation: impl FnOnce() -> T) -> (T, usize) {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            MATERIALIZED_CONTROL.set(None);
        }
    }
    assert!(MATERIALIZED_CONTROL.get().is_none());
    MATERIALIZED_CONTROL.set(Some(0));
    let reset = Reset;
    let output = operation();
    let calls = MATERIALIZED_CONTROL.get().unwrap();
    drop(reset);
    (output, calls)
}

pub(crate) fn canonical_comparison(
    program: &Program,
    comments: &Comments,
    expected: &str,
) -> Comparison {
    #[cfg(test)]
    if let Some(calls) = MATERIALIZED_CONTROL.get() {
        MATERIALIZED_CONTROL.set(Some(calls + 1));
        let output = super::canonical_with_comments(program, comments);
        return Comparison {
            bytes: Some(output.len()),
            matches: output == expected,
        };
    }
    let placement = Placement::new(program, comments);
    let mut output = Compare::new(expected);
    super::super::write_canonical_commented(program, &placement, &mut output);
    output.finish()
}

#[cfg(test)]
#[path = "compare/tests.rs"]
mod tests;
