//! Explicit-payer funded twin of `owner`. Candidate order, the `validate_owner`
//! predicate and the first-duplicate ambiguity stop are the original's. Nothing
//! builds the discarded legacy diagnostics: a missing or ambiguous owner is the
//! plain `OwnerVerdict`, whose `diagnostic()` rebuilds the original text only
//! for a caller that actually needs it. Each candidate execution ID is paid on
//! the caller's payer before it is constructed: a monomorphic declaration ID is
//! a shared identity (0 payload bytes); a generic instance ID owns its full
//! identity text. A payer refusal is returned unchanged, never read as a
//! missing owner.
use super::FunctionExecutionId;
#[cfg(test)]
use crate::diagnostic::Diagnostic;
use crate::hir::funded::{self, FundedError, Reserve};
use crate::hir::ResolvedFunction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OwnerVerdict {
    Missing,
    Ambiguous,
}

impl OwnerVerdict {
    #[cfg(test)]
    pub(crate) fn diagnostic(self) -> Diagnostic {
        match self {
            Self::Ambiguous => Diagnostic::io(
                "SPX-H006",
                "lexical function execution owner is ambiguous",
            ),
            Self::Missing => Diagnostic::io(
                "SPX-H006",
                "unregistered lexical function requires an explicit authenticated execution context",
            ),
        }
    }
}

pub(crate) fn owner_funded<E>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<Result<FunctionExecutionId, OwnerVerdict>, FundedError<E>> {
    funded::charge(reserve, 0)?;
    let mut found = None;
    for declaration in program.functions() {
        funded::charge(reserve, 0)?;
        let candidate = FunctionExecutionId::Monomorphic(declaration.id.clone());
        if function.result_id.matches_result(&candidate)
            && function.body.id.matches(&candidate, "body")
            && found.replace(candidate).is_some()
        {
            return Ok(Err(OwnerVerdict::Ambiguous));
        }
    }
    for instance in program.function_instances() {
        funded::charge(reserve, instance.id.as_str().len())?;
        let candidate = FunctionExecutionId::Generic(instance.id.clone());
        if function.result_id.matches_result(&candidate)
            && function.body.id.matches(&candidate, "body")
            && found.replace(candidate).is_some()
        {
            return Ok(Err(OwnerVerdict::Ambiguous));
        }
    }
    Ok(found.ok_or(OwnerVerdict::Missing))
}

/// Funded `requires`: the scan is a fixed finite stack with no owning frontier,
/// so its real cursor layout is paid first. The only allocation reachable is
/// the original depth-overflow diagnostic text, paid at its full length before
/// the scan can build it (an upper bound; unused on the normal route).
pub(crate) fn requires_funded<E>(
    function: &ResolvedFunction,
    reserve: Reserve<'_, E>,
) -> Result<bool, FundedError<E>> {
    const DEPTH_TEXT: &str = "lexical control scan exceeds admitted expression depth";
    funded::charge(
        reserve,
        std::mem::size_of::<[Option<super::ChildCursor<'_>>; super::walk::PRESENCE_DEPTH]>(),
    )?;
    funded::charge(reserve, DEPTH_TEXT.len())?;
    Ok(super::requires(function).unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funded_owner_matches_the_original_and_refuses_unchanged() {
        let checked = crate::check(
            "module owners; @id(\"main\") fn main()->i64 {0}",
            "owner-funded.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = &program.functions[0];
        let old = super::super::owner(&program, function).unwrap();
        let mut paid = 0usize;
        let mut pay = |bytes: usize| -> Result<(), &'static str> {
            paid += bytes;
            Ok(())
        };
        assert_eq!(owner_funded(&program, function, &mut pay), Ok(Ok(old)));
        let mut refuse = |_: usize| -> Result<(), &'static str> { Err("refused") };
        assert_eq!(
            owner_funded(&program, function, &mut refuse),
            Err(FundedError::Refused("refused"))
        );
        let mut pay = |_: usize| -> Result<(), &'static str> { Ok(()) };
        assert_eq!(requires_funded(function, &mut pay), Ok(false));
        assert_eq!(
            OwnerVerdict::Missing.diagnostic().message,
            "unregistered lexical function requires an explicit authenticated execution context"
        );

        // Keep the complete plain-owner control above. Exercise the actual
        // terminated guard and its already-admitted unterminated receiver with
        // the separate existing work callback as well as the byte payer.
        #[derive(Debug)]
        struct Limit {
            remaining: usize,
            used: usize,
            refusal: Option<&'static str>,
            label: &'static str,
        }
        impl Limit {
            fn new(remaining: usize, label: &'static str) -> Self {
                Self {
                    remaining,
                    used: 0,
                    refusal: None,
                    label,
                }
            }
            fn charge(&mut self, amount: usize) -> Result<(), &'static str> {
                if let Some(first) = self.refusal {
                    return Err(first);
                }
                let Some(remaining) = self.remaining.checked_sub(amount) else {
                    self.refusal = Some(self.label);
                    return Err(self.label);
                };
                self.remaining = remaining;
                self.used = self.used.checked_add(amount).expect("bounded test debit");
                Ok(())
            }
        }
        let mut debit_pairs = Vec::new();
        for terminator in ["", ";"] {
            let source = format!("module guard.bounds; @id(\"main\") fn main()->i64 {{ let mut i=0; while i<3 {{ i=i+1; if i==2 {{continue;}} else {{0}}{terminator} i=i+0; 0 }} i }}");
            let checked = crate::check(&source, "guard-bounds.spx").unwrap();
            let program = crate::hir::resolve(&checked).unwrap();
            let function = &program.functions[0];
            let original_owner = super::super::owner(&program, function).unwrap();
            let original_requires = super::super::requires(function).unwrap();
            assert!(original_requires, "real lexical guard required");
            let run = |bytes: &mut Limit, work: &mut Limit| {
                let mut reserve = |amount| bytes.charge(amount);
                let mut visit = |amount| work.charge(amount);
                let owner = owner_funded_with_work(&program, function, &mut reserve, &mut visit)?;
                let required = requires_funded_with_work(function, &mut reserve, &mut visit)?;
                Ok::<_, FundedError<&'static str>>((owner, required))
            };
            // Derive exact cumulative charges from the real complete receiver;
            // no guessed quota or production allowance is introduced.
            let mut bytes = Limit::new(usize::MAX, "byte refusal");
            let mut work = Limit::new(usize::MAX, "work refusal");
            assert_eq!(
                run(&mut bytes, &mut work),
                Ok((Ok(original_owner.clone()), original_requires))
            );
            assert!(bytes.used > 0 && work.used > 0);
            let exact = (bytes.used, work.used);
            debit_pairs.push(exact);
            let mut bytes = Limit::new(exact.0, "byte refusal");
            let mut work = Limit::new(exact.1, "work refusal");
            let ((), observed) = crate::test_allocation_observer::measure(|| {
                let result = run(&mut bytes, &mut work);
                assert_eq!(result, Ok((Ok(original_owner.clone()), original_requires)));
                // The receiving execution ID drops inside the observation.
                drop(result);
            });
            assert!(!observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
            assert!(observed.allocated <= bytes.used && observed.highwater <= bytes.used);
            assert_eq!((bytes.remaining, work.remaining), (0, 0));
            assert_eq!((bytes.used, work.used), exact);
            assert!(bytes.refusal.is_none() && work.refusal.is_none());
            for (byte_limit, work_limit, selected) in [
                (0, exact.1, "byte refusal"),
                (exact.0 - 1, exact.1, "byte refusal"),
                (exact.0, 0, "work refusal"),
                (exact.0, exact.1 - 1, "work refusal"),
            ] {
                let mut bytes = Limit::new(byte_limit, "byte refusal");
                let mut work = Limit::new(work_limit, "work refusal");
                let ((), observed) = crate::test_allocation_observer::measure(|| {
                    let result = run(&mut bytes, &mut work);
                    assert_eq!(result, Err(FundedError::Refused(selected)));
                    drop(result);
                });
                assert!(!observed.invalid && observed.live == 0);
                assert_eq!(observed.allocated, observed.freed);
                assert!(observed.allocated <= bytes.used && observed.highwater <= bytes.used);
                let used = (bytes.used, work.used);
                if selected == "byte refusal" {
                    assert_eq!(bytes.charge(0), Err(selected));
                } else {
                    assert_eq!(work.charge(0), Err(selected));
                }
                assert_eq!(
                    (bytes.used, work.used),
                    used,
                    "sticky refusal never resets or refunds"
                );
                assert_eq!(
                    super::super::owner(&program, function).unwrap(),
                    original_owner
                );
                assert_eq!(super::super::requires(function).unwrap(), original_requires);
            }
        }
        assert_eq!(
            debit_pairs[0], debit_pairs[1],
            "same canonical guard has identical charged work and storage"
        );
    }
}

#[cfg(test)]
mod work;
#[cfg(test)]
pub(crate) use work::{owner_funded_with_work, requires_funded_with_work};
