//! Explicit test-account classifier sink with actual work and backing payer.
//! The complete ordinary and original explicit sinks remain independent. This
//! receiver never reads or debits ACTIVE, and never invents a private quota.
use super::*;

impl ClassifierString {
    pub(crate) fn acquire_oracle_work<C, E>(
        context: &mut C,
        pay: &mut impl FnMut(usize, &mut C) -> Result<(), E>,
        work: &mut impl FnMut(usize, &mut C) -> Result<(), E>,
    ) -> Result<Self, GrowthFailure<E>> {
        work(1, context).map_err(GrowthFailure::Payment)?;
        pay(size_of::<Self>(), context).map_err(GrowthFailure::Payment)?;
        Ok(Self {
            bytes: String::new(),
            logical: None,
        })
    }

    pub(crate) fn write_oracle_work<C, E, P, L, W>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        context: &mut C,
        pay: &mut P,
        logical: &mut L,
        work: &mut W,
        strict: bool,
    ) -> Result<(), GrowthFailure<E>>
    where
        P: FnMut(usize, &mut C) -> Result<(), E>,
        L: FnMut(usize, &mut C) -> Result<bool, E>,
        W: FnMut(usize, &mut C) -> Result<(), E>,
    {
        struct Writer<'a, C, E, P, L, W> {
            output: &'a mut ClassifierString,
            context: &'a mut C,
            pay: &'a mut P,
            logical: &'a mut L,
            work: &'a mut W,
            strict: bool,
            declined: bool,
            first: Option<GrowthFailure<E>>,
        }
        impl<C, E, P, L, W> Writer<'_, C, E, P, L, W>
        where
            P: FnMut(usize, &mut C) -> Result<(), E>,
            L: FnMut(usize, &mut C) -> Result<bool, E>,
            W: FnMut(usize, &mut C) -> Result<(), E>,
        {
            fn append(&mut self, value: &str) -> Result<bool, GrowthFailure<E>> {
                (self.work)(1, self.context).map_err(GrowthFailure::Payment)?;
                if !(self.logical)(value.len(), self.context).map_err(GrowthFailure::Payment)? {
                    return Ok(false);
                }
                (self.work)(value.len(), self.context).map_err(GrowthFailure::Payment)?;
                let required = self
                    .output
                    .bytes
                    .len()
                    .checked_add(value.len())
                    .ok_or(GrowthFailure::Arithmetic)?;
                if required > self.output.bytes.capacity() {
                    let doubled = self
                        .output
                        .bytes
                        .capacity()
                        .checked_mul(2)
                        .ok_or(GrowthFailure::Arithmetic)?;
                    let target = required.max(doubled).max(8);
                    (self.work)(self.output.bytes.len(), self.context)
                        .map_err(GrowthFailure::Payment)?;
                    if self.output.bytes.capacity() != 0 {
                        (self.work)(1, self.context).map_err(GrowthFailure::Payment)?;
                    }
                    (self.pay)(target, self.context).map_err(GrowthFailure::Payment)?;
                    self.output
                        .bytes
                        .try_reserve_exact(target - self.output.bytes.len())
                        .map_err(|_| GrowthFailure::Allocation)?;
                    if self.output.bytes.capacity() != target {
                        return Err(GrowthFailure::Capacity);
                    }
                }
                self.output.bytes.push_str(value);
                Ok(true)
            }
        }
        impl<C, E, P, L, W> fmt::Write for Writer<'_, C, E, P, L, W>
        where
            P: FnMut(usize, &mut C) -> Result<(), E>,
            L: FnMut(usize, &mut C) -> Result<bool, E>,
            W: FnMut(usize, &mut C) -> Result<(), E>,
        {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                if self.first.is_some() || (self.strict && self.declined) {
                    return Err(fmt::Error);
                }
                match self.append(value) {
                    Ok(true) => Ok(()),
                    Ok(false) => {
                        self.declined = true;
                        if self.strict {
                            Err(fmt::Error)
                        } else {
                            Ok(())
                        }
                    }
                    Err(error) => {
                        self.first = Some(error);
                        Err(fmt::Error)
                    }
                }
            }
        }
        work(1, context).map_err(GrowthFailure::Payment)?;
        let mut writer = Writer {
            output: self,
            context,
            pay,
            logical,
            work,
            strict,
            declined: false,
            first: None,
        };
        let formatted = fmt::write(&mut writer, arguments);
        let Writer {
            declined, first, ..
        } = writer;
        if let Some(error) = first {
            return Err(error);
        }
        if strict && declined {
            self.bytes = String::new();
            return Ok(());
        }
        formatted.map_err(|_| GrowthFailure::Formatter)
    }
}
