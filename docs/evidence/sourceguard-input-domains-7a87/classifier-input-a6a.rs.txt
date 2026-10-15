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
impl ClassifierLedger {
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
