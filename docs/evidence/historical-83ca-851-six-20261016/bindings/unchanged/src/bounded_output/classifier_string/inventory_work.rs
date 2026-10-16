//! Work-charged counterpart of the classifier sink. Callbacks come from the
//! caller's budget; no quota, ledger or cap is created here, and a work or
//! payment failure is only ever published as `GrowthFailure::Payment`.
use super::*;

#[derive(Clone, Copy)]
struct Mode {
    strict: bool,
    logical: bool,
}

struct Io<'a, C, P, W> {
    context: &'a mut C,
    pay: &'a mut P,
    work: &'a mut W,
}
impl<C, E, P, W> Io<'_, C, P, W>
where
    P: FnMut(usize, &mut C) -> Result<(), E>,
    W: FnMut(usize, &mut C) -> Result<(), E>,
{
    fn charge_pay(&mut self, bytes: usize) -> Result<(), GrowthFailure<E>> {
        (self.pay)(bytes, &mut *self.context).map_err(GrowthFailure::Payment)
    }
    fn charge_work(&mut self, units: usize) -> Result<(), GrowthFailure<E>> {
        (self.work)(units, &mut *self.context).map_err(GrowthFailure::Payment)
    }
}

impl ClassifierString {
    pub(crate) fn acquire_with_work<C, E, P, W>(
        context: &mut C,
        pay: &mut P,
        work: &mut W,
    ) -> Result<Self, GrowthFailure<E>>
    where
        P: FnMut(usize, &mut C) -> Result<(), E>,
        W: FnMut(usize, &mut C) -> Result<(), E>,
    {
        let mut io = Io { context, pay, work };
        io.charge_work(1)?;
        io.charge_pay(size_of::<Self>())?;
        Ok(Self {
            bytes: String::new(),
            logical: active(),
        })
    }

    fn append_with_work<C, E, P, W>(
        &mut self,
        value: &str,
        io: &mut Io<'_, C, P, W>,
        mode: Mode,
    ) -> Result<bool, GrowthFailure<E>>
    where
        P: FnMut(usize, &mut C) -> Result<(), E>,
        W: FnMut(usize, &mut C) -> Result<(), E>,
    {
        // One visit per chunk, before the logical frontier is consulted.
        io.charge_work(1)?;
        if mode.logical {
            let accepted = if mode.strict {
                reserve(self.logical.as_deref(), value.len())
            } else {
                reserve_sink(self.logical.as_ref(), value.len())
            };
            if !accepted {
                return Ok(false);
            }
        }
        // Copied bytes are charged only after logical admission, before the copy.
        if !value.is_empty() {
            io.charge_work(value.len())?;
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
            // Initialized old bytes are recopied into the replacement, which is
            // paid in full while the old backing is still retained.
            if !self.bytes.is_empty() {
                io.charge_work(self.bytes.len())?;
            }
            io.charge_pay(target)?;
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

    /// Same logical behavior as `capped`, with work charged through the caller.
    pub(crate) fn capped_with_work<C, E, P, W>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        context: &mut C,
        pay: &mut P,
        work: &mut W,
    ) -> Result<(), GrowthFailure<E>>
    where
        P: FnMut(usize, &mut C) -> Result<(), E>,
        W: FnMut(usize, &mut C) -> Result<(), E>,
    {
        let mut io = Io { context, pay, work };
        let mode = Mode {
            strict: false,
            logical: true,
        };
        self.write_with_work(arguments, &mut io, mode).map(|_| ())
    }

    /// Same logical behavior as `strict`; a work failure never clears or
    /// marks overflow, only a logical decline or formatter error does.
    pub(crate) fn strict_with_work<C, E, P, W>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        context: &mut C,
        pay: &mut P,
        work: &mut W,
    ) -> Result<(), GrowthFailure<E>>
    where
        P: FnMut(usize, &mut C) -> Result<(), E>,
        W: FnMut(usize, &mut C) -> Result<(), E>,
    {
        let mut io = Io { context, pay, work };
        let mode = Mode {
            strict: true,
            logical: true,
        };
        match self.write_with_work(arguments, &mut io, mode) {
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

    /// Same as `raw`: no logical debit, physical and work charges only.
    pub(crate) fn raw_with_work<C, E, P, W>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        context: &mut C,
        pay: &mut P,
        work: &mut W,
    ) -> Result<(), GrowthFailure<E>>
    where
        P: FnMut(usize, &mut C) -> Result<(), E>,
        W: FnMut(usize, &mut C) -> Result<(), E>,
    {
        let mut io = Io { context, pay, work };
        let mode = Mode {
            strict: false,
            logical: false,
        };
        self.write_with_work(arguments, &mut io, mode).map(|_| ())
    }

    fn write_with_work<C, E, P, W>(
        &mut self,
        arguments: fmt::Arguments<'_>,
        io: &mut Io<'_, C, P, W>,
        mode: Mode,
    ) -> Result<bool, GrowthFailure<E>>
    where
        P: FnMut(usize, &mut C) -> Result<(), E>,
        W: FnMut(usize, &mut C) -> Result<(), E>,
    {
        // Two lifetimes keep the Io borrow short; `Io<'b, ..>` is invariant.
        struct Writer<'a, 'b, C, E, P, W> {
            output: &'a mut ClassifierString,
            io: &'a mut Io<'b, C, P, W>,
            mode: Mode,
            logical_refusal: bool,
            physical: Option<GrowthFailure<E>>,
        }
        impl<C, E, P, W> fmt::Write for Writer<'_, '_, C, E, P, W>
        where
            P: FnMut(usize, &mut C) -> Result<(), E>,
            W: FnMut(usize, &mut C) -> Result<(), E>,
        {
            fn write_str(&mut self, value: &str) -> fmt::Result {
                if self.physical.is_some() || (self.mode.strict && self.logical_refusal) {
                    return Err(fmt::Error);
                }
                match self
                    .output
                    .append_with_work(value, &mut *self.io, self.mode)
                {
                    Ok(true) => Ok(()),
                    Ok(false) => {
                        self.logical_refusal = true;
                        if self.mode.strict {
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
        // One formatter visit before any chunk action.
        io.charge_work(1)?;
        let mut writer = Writer {
            output: self,
            io,
            mode,
            logical_refusal: false,
            physical: None,
        };
        let outcome = fmt::write(&mut writer, arguments);
        let Writer {
            logical_refusal,
            physical,
            ..
        } = writer;
        if let Some(error) = physical {
            return Err(error);
        }
        if mode.strict && logical_refusal {
            return Ok(false);
        }
        outcome.map_err(|_| GrowthFailure::Formatter)?;
        Ok(true)
    }
}
