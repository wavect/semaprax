//! Explicit test-account receiver. The complete captured-ledger receiver in the
//! parent remains unchanged. No ACTIVE ledger is read, captured or reserved here.
use super::*;

impl ClassifierString {
    pub(crate) fn acquire_explicit<E>(
        pay: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, GrowthFailure<E>> {
        pay(size_of::<Self>()).map_err(GrowthFailure::Payment)?;
        Ok(Self {
            bytes: String::new(),
            logical: None,
        })
    }

    pub(crate) fn capped_explicit<S, E>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        storage: &mut S,
        pay: &mut impl FnMut(usize, &mut S) -> Result<(), E>,
        reserve: &mut impl FnMut(usize, &mut S) -> Result<bool, E>,
    ) -> Result<(), GrowthFailure<E>> {
        self.write_explicit(arguments, storage, pay, reserve, false)
            .map(|_| ())
    }

    /// Raw has no logical reservation: a successful work callback cannot
    /// silently decline a chunk. The original complete raw receiver is kept.
    pub(crate) fn raw_explicit<S, E>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        storage: &mut S,
        pay: &mut impl FnMut(usize, &mut S) -> Result<(), E>,
        work: &mut impl FnMut(usize, &mut S) -> Result<(), E>,
    ) -> Result<(), GrowthFailure<E>> {
        self.write_explicit(
            arguments,
            storage,
            pay,
            &mut |bytes, state| work(bytes, state).map(|()| true),
            false,
        )
        .map(|_| ())
    }

    pub(crate) fn strict_explicit<S, E>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        storage: &mut S,
        pay: &mut impl FnMut(usize, &mut S) -> Result<(), E>,
        reserve: &mut impl FnMut(usize, &mut S) -> Result<bool, E>,
    ) -> Result<(), GrowthFailure<E>> {
        match self.write_explicit(arguments, storage, pay, reserve, true)? {
            true => {}
            false => self.bytes = String::new(),
        }
        Ok(())
    }

    fn append_explicit<S, E>(
        &mut self,
        value: &str,
        storage: &mut S,
        pay: &mut impl FnMut(usize, &mut S) -> Result<(), E>,
        reserve: &mut impl FnMut(usize, &mut S) -> Result<bool, E>,
    ) -> Result<bool, GrowthFailure<E>> {
        // Preserve the original logical-before-physical append frontier. A
        // physical/account error propagates; logical floor suppression may only
        // be represented by an explicit successful false callback result.
        if !reserve(value.len(), storage).map_err(GrowthFailure::Payment)? {
            return Ok(false);
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
            pay(target, storage).map_err(GrowthFailure::Payment)?;
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

    fn write_explicit<S, E, P, L>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        storage: &mut S,
        pay: &mut P,
        reserve: &mut L,
        strict: bool,
    ) -> Result<bool, GrowthFailure<E>>
    where
        P: FnMut(usize, &mut S) -> Result<(), E>,
        L: FnMut(usize, &mut S) -> Result<bool, E>,
    {
        struct Writer<'a, S, E, P, L> {
            output: &'a mut ClassifierString,
            storage: &'a mut S,
            pay: &'a mut P,
            reserve: &'a mut L,
            strict: bool,
            logical_refusal: bool,
            physical: Option<GrowthFailure<E>>,
        }
        impl<S, E, P, L> fmt::Write for Writer<'_, S, E, P, L>
        where
            P: FnMut(usize, &mut S) -> Result<(), E>,
            L: FnMut(usize, &mut S) -> Result<bool, E>,
        {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                if self.physical.is_some() || (self.strict && self.logical_refusal) {
                    return Err(fmt::Error);
                }
                match self
                    .output
                    .append_explicit(value, self.storage, self.pay, self.reserve)
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
            storage,
            pay,
            reserve,
            strict,
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
}

#[cfg(test)]
mod tests;
