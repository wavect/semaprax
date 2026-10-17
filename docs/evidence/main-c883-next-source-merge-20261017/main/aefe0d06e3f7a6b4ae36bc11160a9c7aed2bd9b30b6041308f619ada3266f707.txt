//! Explicitly financed test comparison hook. Ordinary unopted callers retain
//! the complete original enumerating/factored receiver. Owning tests opt into
//! one independent finite account without consuming their live builder ledger.
use super::*;
use crate::cleanup_plan::replay::allocation::{enter, OracleLease};

/// Explicit private-test dispatcher. No compiler or resolver call chooses it.
pub(super) fn cross_check(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) {
    match super::super::allocation::opted() {
        Ok(None) => super::cross_check(program, function),
        Ok(Some(lease)) => cross_check_oracle(program, function, lease),
        Err(refusal) => panic!("opted oracle lease is not live: {refusal:?}"),
    }
}

/// Ordinary storage sharing changes backing only; explicitly opted oracle
/// routes keep their complete independent account and never borrow this lease.
pub(super) fn cross_check_with_storage<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
    storage: Option<&mut continuation_storage::Storage<'hir>>,
) -> Result<(), Diagnostic> {
    match super::super::allocation::opted() {
        Ok(None) => match storage {
            Some(storage) if storage.has_receiving_owner() => {
                storage.authenticate(program, function, budget)?;
                super::continuation_receiver::cross_check(program, function, storage);
            }
            _ => super::cross_check(program, function),
        },
        Ok(Some(lease)) => cross_check_oracle(program, function, lease),
        Err(refusal) => panic!("opted oracle lease is not live: {refusal:?}"),
    }
    Ok(())
}

/// Restores the materialization instrumentation on every exit, including
/// unwind. It only restores the saved counter; nothing is normalized.
struct Restore(usize);

impl Drop for Restore {
    fn drop(&mut self) {
        SKELETON_MATERIALIZATIONS.with(|count| count.set(self.0));
    }
}

/// One receiver's `start`/`complete` pair. `finish` completes visibly on the
/// normal and error paths; `Drop` completes during unwind only.
struct Run {
    lease: OracleLease,
    done: bool,
}

impl Run {
    fn begin(lease: OracleLease, function: &ResolvedFunction) -> Self {
        if let Err(refusal) = lease.start() {
            panic!(
                "oracle account refused to start a comparison for `{}`: {refusal:?}",
                function.id
            );
        }
        Self { lease, done: false }
    }

    fn finish(mut self, function: &ResolvedFunction) {
        self.done = true;
        if let Err(refusal) = self.lease.complete() {
            panic!(
                "oracle account could not complete a comparison for `{}`: {refusal:?}",
                function.id
            );
        }
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if !self.done {
            // Unwind: `Unbalanced` would mean the account was replaced and its
            // own receipt already shows that.
            let _ = self.lease.complete();
        }
    }
}

/// Runs one fresh receiver under its own invocation authority and balanced
/// start/complete pair, then makes any account, work or physical refusal
/// visible before the verdict is read.
fn receiver<T>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    lease: OracleLease,
    name: &str,
    body: impl FnOnce() -> T,
) -> T {
    let run = Run::begin(lease, function);
    let invocation = match enter(program, function, lease) {
        Ok(invocation) => invocation,
        Err(refusal) => panic!(
            "oracle invocation authority refused `{}`: {refusal:?}",
            function.id
        ),
    };
    let verdict = body();
    if let Err(refusal) = invocation.exit() {
        panic!(
            "oracle invocation authority was unbalanced for `{}`: {refusal:?}",
            function.id
        );
    }
    run.finish(function);
    assert!(
        !lease.refused(),
        "oracle account refused the {name} comparison for `{}` (physical: {:?})",
        function.id,
        lease.physical_failure()
    );
    verdict
}

/// Stable sort scratch is at most one element slot per path; pay that upper
/// bound on the account before the sort can allocate it.
fn paid_sort(
    _lease: OracleLease,
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    paths: &mut [SkeletonPath],
) -> Result<(), Diagnostic> {
    super::oracle_order::sort(paths, function, budget)
}
fn same_paths(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    a: &[SkeletonPath],
    b: &[SkeletonPath],
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "oracle path vector length")?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (x, y) in a.iter().zip(b) {
        budget.charge(function, 1, "oracle path vector member")?;
        if super::oracle_order::path(x, y, function, budget)? != std::cmp::Ordering::Equal {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn cross_check_oracle(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    lease: OracleLease,
) {
    let _restore = Restore(SKELETON_MATERIALIZATIONS.with(Cell::get));
    let enumerated = receiver(program, function, lease, "enumerating", || {
        let mut enumerating =
            ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
        hir_skeleton_paths(program, function, &mut enumerating).and_then(|mut hir| {
            let mut plan = plan_skeleton_paths(function, &mut enumerating)?;
            paid_sort(lease, &mut enumerating, function, &mut hir)?;
            paid_sort(lease, &mut enumerating, function, &mut plan)?;
            same_paths(function, &mut enumerating, &hir, &plan)
        })
    });
    // A fresh factored receiver runs regardless of the enumeration verdict.
    let factored = receiver(program, function, lease, "factored", || {
        let mut factored = ReplayBudget::with_skeleton_limit_oracle(0, lease);
        compare(program, function, &mut factored)
    });
    if let Ok(true) = enumerated {
        require_decided(function, &factored);
        assert!(
            matches!(factored, Ok(Some(true))),
            "funded factored skeleton comparison disagrees with enumeration for `{}`: {:?}",
            function.id,
            factored.map_err(|error| error.message)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup_plan::replay::allocation::{
        authority, opt_in, InvocationRefusal, OracleReceipt, OracleRefusal, OracleScope,
    };

    const SOURCE: &str =
        "module hook; @id(\"main\") fn main()->i64 {let a=if 1==1 {1} else {2}; a}";
    /// `SOURCE` resolves to this one function.
    const PROGRAM_FUNCTIONS: usize = 1;

    type Outcome = (
        std::thread::Result<()>,
        OracleReceipt,
        Result<(), OracleRefusal>,
    );

    fn run(allowance: usize, work: usize) -> Outcome {
        let program =
            crate::hir::resolve(&crate::check(SOURCE, "oracle-hook.spx").unwrap()).unwrap();
        let scope = OracleScope::open_with_work(allowance, work).unwrap();
        let lease = scope.lease();
        let opt = opt_in(lease).unwrap();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            for function in &program.functions {
                cross_check(&program, function);
            }
        }));
        // Unwind restored the invocation: the next subject is ordinary again.
        for function in &program.functions {
            assert_eq!(authority(&program, function), Ok(None));
        }
        drop(opt);
        let retry = lease.charge(0);
        (outcome, scope.finish(), retry)
    }

    #[test]
    fn pending_receiver_runs_both_comparisons_cumulatively_and_refuses_visibly() {
        let (outcome, receipt, retry) = run(1 << 26, 1 << 26);
        assert!(outcome.is_ok() && retry.is_ok());
        assert!(
            receipt.charged > 0,
            "the oracle account paid the comparison"
        );
        // Exclusive dispatch: exactly the enumerating and the factored
        // receiver per function, never the old ACTIVE pair as well.
        assert_eq!(receipt.started, 2 * PROGRAM_FUNCTIONS);
        assert_eq!(receipt.started, receipt.completed);
        assert!(receipt.physical_failure.is_none());
        assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none());
        let (bytes, work) = (receipt.charged, receipt.work);
        // Exact allowance repeats the identical cumulative charge.
        let (outcome, exact, _) = run(bytes, work);
        assert!(outcome.is_ok());
        assert_eq!(
            (exact.charged, exact.work, exact.calls),
            (bytes, work, receipt.calls)
        );
        // One byte short and zero refuse, visibly and stickily.
        for allowance in [bytes - 1, 0] {
            let (outcome, short, retry) = run(allowance, work);
            assert!(
                outcome.is_err(),
                "account refusal is not a skipped comparison"
            );
            assert!(short.first_refusal.is_some());
            assert_eq!(retry, Err(OracleRefusal::Sticky));
            assert!(short.charged <= allowance);
            assert_eq!(short.started, short.completed);
        }
        if work > 0 {
            let (outcome, short, retry) = run(bytes, work - 1);
            assert!(outcome.is_err());
            assert!(short.first_work_refusal.is_some());
            assert_eq!(retry, Err(OracleRefusal::Sticky));
        }
    }

    #[test]
    fn physical_failure_is_sticky_with_bytes_left_and_keeps_the_first_reason() {
        let scope = OracleScope::open_with_work(1 << 20, 1 << 20).unwrap();
        let lease = scope.lease();
        assert_eq!(lease.charge(8), Ok(()));
        lease.fail_physical("first");
        lease.fail_physical("second");
        assert!(lease.refused());
        assert_eq!(lease.physical_failure(), Some("first"));
        assert_eq!(lease.charge(1), Err(OracleRefusal::Sticky));
        assert_eq!(lease.charge_work(1), Err(OracleRefusal::Sticky));
        assert_eq!(lease.start(), Err(OracleRefusal::Sticky));
        let receipt = scope.finish();
        // Logical receipts are untouched: a physical failure is not a refusal.
        assert!(receipt.first_refusal.is_none() && receipt.first_work_refusal.is_none());
        assert_eq!(receipt.charged, 8);
    }

    #[test]
    fn hook_without_opt_in_stays_ordinary_and_expired_opt_in_is_refused() {
        let program =
            crate::hir::resolve(&crate::check(SOURCE, "oracle-hook.spx").unwrap()).unwrap();
        for function in &program.functions {
            cross_check(&program, function);
        }
        let scope = OracleScope::open(1 << 20).unwrap();
        let opt = opt_in(scope.lease()).unwrap();
        assert_eq!(opt_in(scope.lease()).err(), Some(InvocationRefusal::Nested));
        drop(scope);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cross_check(&program, &program.functions[0]);
        }));
        assert!(outcome.is_err(), "an expired opted lease is never skipped");
        drop(opt);
    }
}

// Classification of an undecided result carries no comparison authority.
fn require_decided(function: &ResolvedFunction, factored: &Result<Option<bool>, Diagnostic>) {
    assert!(
        !matches!(factored, Ok(None)),
        "funded factored skeleton comparison is undecided for `{}`",
        function.id,
    );
}

#[cfg(test)]
mod closure_controls;
