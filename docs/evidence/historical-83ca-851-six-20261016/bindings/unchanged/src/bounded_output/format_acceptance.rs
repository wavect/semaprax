//! Initial-report field acceptance on the actual current parent. This retains
//! no allowance and owns no output backing. Its inline controls are explicitly
//! ordinary caller-stack storage, NOT prepaid after an initial owner refusal.
use super::*;

#[derive(Clone, Copy)]
pub(crate) struct Field {
    pub(crate) complete: bool,
    pub(crate) bytes: usize,
}
struct Counter<'a> {
    budget: Option<&'a Budget>,
    bytes: usize,
    refused: bool,
}
impl Counter<'_> {
    fn piece(&mut self, bytes: usize) -> fmt::Result {
        if self.refused || !reserve(self.budget, bytes) {
            self.refused = true;
            return Err(fmt::Error);
        }
        self.bytes = self.bytes.checked_add(bytes).ok_or(fmt::Error)?;
        Ok(())
    }
    fn finish(self, result: fmt::Result) -> Field {
        if result.is_err() && !self.refused {
            match self.budget {
                Some(budget) => budget.overflowed.set(true),
                None => panic!("a formatting trait implementation returned an error"),
            }
        }
        Field {
            complete: result.is_ok() && !self.refused,
            bytes: self.bytes,
        }
    }
}
impl fmt::Write for Counter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.piece(value.len())
    }
}
pub(crate) fn formatted(arguments: fmt::Arguments<'_>) -> Field {
    let budget = active();
    let mut writer = Counter {
        budget: budget.as_deref(),
        bytes: 0,
        refused: false,
    };
    let result = writer.write_fmt(arguments);
    writer.finish(result)
}
/// Both inputs are complete, nonempty original String fields. The original
/// `{help} {evidence}` formatter writes precisely those three nonempty pieces.
/// Empty formatter pieces cannot change a successful intervening reservation.
/// This does not recursively replay the previously charged field fragments.
pub(crate) fn joined(help: usize, evidence: usize) -> Field {
    let budget = active();
    let mut writer = Counter {
        budget: budget.as_deref(),
        bytes: 0,
        refused: false,
    };
    let result = writer
        .piece(help)
        .and_then(|()| writer.piece(1))
        .and_then(|()| writer.piece(evidence));
    writer.finish(result)
}

#[cfg(test)]
mod tests;
