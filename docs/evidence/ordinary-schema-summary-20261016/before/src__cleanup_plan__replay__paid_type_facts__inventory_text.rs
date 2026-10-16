//! Raw ordinary diagnostic text on the held replay work and backing payer.
//! This carrier reserves real replacement storage and counts actual UTF-8 writes;
//! it does not turn reserved heap bytes into work or introduce a separate quota.
use super::*;
use std::fmt::{self, Write};

/// The new funded constructor API never treats a detached ordinary budget
/// as backing authority. The complete old unfunded compatibility API remains.
pub(super) fn current(payer: &mut Payer<'_>) -> Result<(), Diagnostic> {
    payer.current()?;
    if !payer.budget.pays_allocations() {
        return Err(Diagnostic::io_static(
            "SPX-H006",
            "ordinary inventory constructors require a live backing payer",
        ));
    }
    payer.budget.reserve_bytes(
        payer.function,
        0,
        "inventory held paid constructor authentication",
    )?;
    Ok(())
}

struct Text<'a, 'b> {
    payer: &'a mut Payer<'b>,
    bytes: String,
    first: Option<Diagnostic>,
}
impl Text<'_, '_> {
    fn append(&mut self, value: &str) -> Result<(), Diagnostic> {
        current(self.payer)?;
        let function = self.payer.function;
        self.payer
            .budget
            .charge(function, 1, "inventory raw formatter chunk")?;
        self.payer
            .budget
            .charge(function, value.len(), "inventory raw UTF-8 bytes copied")?;
        let required = self.bytes.len().checked_add(value.len()).ok_or_else(|| {
            self.payer
                .construction_refusal("inventory diagnostic string length overflowed")
        })?;
        if required > self.bytes.capacity() {
            let doubled = self.bytes.capacity().checked_mul(2).ok_or_else(|| {
                self.payer
                    .construction_refusal("inventory diagnostic string capacity overflowed")
            })?;
            let target = required.max(doubled).max(8);
            // Replacement may copy the complete initialized old prefix. Pay
            // that real copy bound separately; heap capacity is not work.
            self.payer.budget.charge(
                function,
                self.bytes.len(),
                "inventory diagnostic retained bytes recopied",
            )?;
            // Pay the complete replacement while the previous backing is live.
            self.payer.budget.reserve_bytes(
                function,
                target,
                "inventory raw diagnostic backing",
            )?;
            self.bytes
                .try_reserve_exact(target - self.bytes.len())
                .map_err(|_| {
                    self.payer
                        .construction_refusal("inventory diagnostic string allocation failed")
                })?;
            if self.bytes.capacity() != target {
                return Err(self.payer.construction_refusal(
                    "inventory diagnostic capacity disagrees with its reservation",
                ));
            }
        }
        self.bytes.push_str(value);
        Ok(())
    }
}
impl Write for Text<'_, '_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.first.is_some() {
            return Err(fmt::Error);
        }
        match self.append(value) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.first = Some(error);
                Err(fmt::Error)
            }
        }
    }
}

pub(super) fn raw(
    payer: &mut Payer<'_>,
    arguments: fmt::Arguments<'_>,
) -> Result<String, Diagnostic> {
    current(payer)?;
    inventory_metadata::step(payer, 2)?; // carrier construction and receiving String release
    payer.bytes(std::mem::size_of::<Text<'_, '_>>())?;
    let mut text = Text {
        payer,
        bytes: String::new(),
        first: None,
    };
    let formatted = text.write_fmt(arguments);
    if let Some(error) = text.first {
        return Err(error);
    }
    // Match the complete original formatter's contract for a trait formatter
    // which returns fmt::Error without a typed acquisition/work refusal.
    formatted.expect(
        "a formatting trait implementation returned an error when the underlying stream did not",
    );
    Ok(text.bytes)
}

pub(super) fn owner_matches(
    actual: &DeclarationId,
    owner: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    same_bytes(actual.as_str(), owner.as_str(), payer)
}

pub(super) fn same_bytes(
    actual: &str,
    expected: &str,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    current(payer)?;
    let function = payer.function;
    payer
        .budget
        .charge(function, 2, "inventory held identity length inspection")?;
    if actual.len() != expected.len() {
        return Ok(false);
    }
    // Equal-length identity equality compares these held UTF-8 bytes. This
    // explicit comparison bound is independent of any backing reservation.
    payer.budget.charge(
        function,
        actual.len(),
        "inventory held identity bytes compared",
    )?;
    Ok(actual == expected)
}
