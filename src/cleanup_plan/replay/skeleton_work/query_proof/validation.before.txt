//! Invocation-local immutable classification DATA, never content authority.
use super::{
    lexical_skeleton, size_of, ClassifierLedger, Diagnostic, ExprSkeletonPath, ObservationDag,
    ObservationDagKind, Observations, Rc, SkeletonWork, PAYMENT,
};

struct Entry {
    root: Rc<ObservationDag>,
    visits: usize,
}

// The exclusive real work loan binds both function and budget without a raw
// address capability. An entry owns its immutable Rc, preventing ABA/mutation.
struct Validation<'scope, 'function, 'budget> {
    work: &'scope mut SkeletonWork<'function, 'budget>,
    ledger: ClassifierLedger,
    last: Option<Entry>,
}
type Scope = Validation<'static, 'static, 'static>;
type Pending = Vec<&'static ObservationDag>;
type Paths = std::iter::Chain<
    std::slice::Iter<'static, ExprSkeletonPath>,
    std::slice::Iter<'static, ExprSkeletonPath>,
>;

// Actual/formal loans, live/moved/returned owners, setup iterators and complete
// error/Drop receivers. Dynamic Vec backing is separately acquired below.
pub(super) const CALL: usize = 8 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * size_of::<Result<bool, Diagnostic>>()
    + 2 * size_of::<Scope>()
    + 2 * size_of::<ClassifierLedger>()
    + 3 * size_of::<Option<Entry>>()
    + 12 * size_of::<std::slice::Iter<'static, ExprSkeletonPath>>() // two iterator results, chain/IntoIterator owners
    + 2 * size_of::<Paths>()
    + ClassifierLedger::capture_into_controls()
    + AUTH
    + PAYMENT;
// CALL prepays the first guard. Every successful reserve renews the next
// guard's borrowed receivers and minimal foreign-error return before use.
const AUTH: usize = ClassifierLedger::current_receiving_controls()
    + 2 * size_of::<Diagnostic>()
    + 2 * size_of::<String>()
    + 4 * size_of::<&str>()
    + 2 * size_of::<Result<bool, Diagnostic>>();
const INPUT_STEP: usize = 2 * size_of::<&mut Paths>()
    + 2 * size_of::<Option<&ExprSkeletonPath>>()
    + 2 * size_of::<&ExprSkeletonPath>()
    + 2 * size_of::<bool>()
    + 2 * PAYMENT;
const CHECK: usize = 2 * size_of::<&mut Scope>()
    + 2 * size_of::<&Observations>()
    + 2 * size_of::<&Option<Rc<ObservationDag>>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 2 * size_of::<&Rc<ObservationDag>>()
    + 2 * size_of::<&Option<Entry>>()
    + 2 * size_of::<&Entry>()
    + 4 * size_of::<&Rc<ObservationDag>>() // ptr_eq actual/formal pair
    + 2 * size_of::<Result<bool, Diagnostic>>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<bool>()
    + 2 * PAYMENT;
const REPLAY: usize = 2 * size_of::<usize>() + 2 * size_of::<bool>() + 2 * PAYMENT;
const COLD: usize = 2 * size_of::<&mut Scope>()
    + 4 * size_of::<&Rc<ObservationDag>>()
    + 3 * size_of::<Pending>()
    + 4 * size_of::<&mut Pending>()
    + 2 * size_of::<&ObservationDag>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Result<(bool, usize), Diagnostic>>()
    + 2 * PAYMENT; // macro reservation and initial pending backing reservation
const POP: usize = 6 * size_of::<&mut Pending>()
    + 2 * size_of::<Option<&ObservationDag>>()
    + 4 * size_of::<&ObservationDag>()
    + 2 * size_of::<&ObservationDagKind>()
    + 2 * size_of::<&Option<Rc<ObservationDag>>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<&Rc<ObservationDag>>()
    + 4 * size_of::<&Vec<Rc<ObservationDag>>>()
    + 4 * size_of::<&[Rc<ObservationDag>]>()
    + 2 * size_of::<std::slice::Iter<'static, Rc<ObservationDag>>>()
    + 4 * size_of::<usize>()
    + 2 * size_of::<Option<usize>>()
    + 2 * size_of::<bool>()
    + 3 * PAYMENT;
const CHILD: usize = 2 * size_of::<&mut std::slice::Iter<'static, Rc<ObservationDag>>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<&Rc<ObservationDag>>()
    + 2 * size_of::<&ObservationDag>()
    + 2 * size_of::<&mut Pending>()
    + PAYMENT;
const PUBLISH: usize = 2 * size_of::<Entry>()
    + 2 * size_of::<Option<Entry>>() // incoming Some and publication receiver
    + 2 * size_of::<Rc<ObservationDag>>()
    + 2 * size_of::<&Rc<ObservationDag>>()
    + 2 * size_of::<usize>()
    + PAYMENT;

// Lexical expansion avoids an unpaid payment-helper argument/Result boundary.
// Foreign lenders are rejected before any reserve/diagnostic renderer on them.
macro_rules! pay {
    ($scope:ident, $bytes:expr, $phase:literal) => {{
        if !$scope.ledger.current() {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        $scope
            .work
            .budget
            .reserve_bytes($scope.work.function, $bytes + AUTH, $phase)?;
    }};
}

pub(super) fn inputs(
    prefixes: &[ExprSkeletonPath],
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<bool, Diagnostic> {
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    let mut scope = Validation {
        work,
        ledger,
        last: None,
    };
    let mut paths = prefixes.iter().chain(suffixes.iter());
    loop {
        pay!(scope, INPUT_STEP, "complete HIR memo input receivers");
        let Some(path) = paths.next() else { break };
        scope
            .work
            .budget
            .charge(scope.work.function, 1, "complete HIR fork input selection")?;
        if !matches!(path.continuation, lexical_skeleton::Continuation::Normal) {
            return Ok(false);
        }
        pay!(scope, CHECK, "complete HIR memo validation receivers");
        if !scope.check(&path.observations)? {
            return Ok(false);
        }
    }
    // Scope/Rc owners drop before the caller consumes any prefix owner.
    Ok(true)
}

impl Validation<'_, '_, '_> {
    fn check(&mut self, observations: &Observations) -> Result<bool, Diagnostic> {
        let Some(root) = &observations.prefix else {
            return Ok(true);
        };
        if let Some(entry) = &self.last {
            // One fixed slot deliberately forgoes nonadjacent reuse. This
            // bounds all-disjoint lookup work linearly, without an admission
            // threshold or alternative semantic route.
            self.work
                .budget
                .charge(self.work.function, 1, "complete HIR memo root lookup")?;
            if Rc::ptr_eq(&entry.root, root) {
                #[cfg(test)]
                note(false);
                let mut left = entry.visits;
                loop {
                    pay!(self, REPLAY, "complete HIR memo replay receivers");
                    if left == 0 {
                        break;
                    }
                    self.work.budget.charge(
                        self.work.function,
                        1,
                        "complete HIR sequence validation",
                    )?;
                    left -= 1;
                }
                return Ok(true);
            }
        }
        pay!(self, COLD, "complete HIR memo cold receivers");
        #[cfg(test)]
        note(true);
        let (valid, visits) = self.classify(root)?;
        if !valid {
            return Ok(false);
        }
        pay!(self, PUBLISH, "complete HIR memo publication receivers");
        // The old root remains owned through full cold classification and
        // successful publication payment. Replacing it drops exactly one Rc;
        // caller inputs still own every source until this scope is destroyed.
        self.last = Some(Entry {
            root: root.clone(),
            visits,
        });
        Ok(true)
    }

    fn classify(&mut self, root: &Rc<ObservationDag>) -> Result<(bool, usize), Diagnostic> {
        let mut pending = Vec::new();
        let mut visits = 0usize;
        self.work.budget.reserve_vec(
            self.work.function,
            &mut pending,
            1,
            "complete HIR sequence validation backing",
        )?;
        pending.push(root.as_ref());
        loop {
            pay!(self, POP, "complete HIR memo cold step receivers");
            let Some(node) = pending.pop() else { break };
            self.work
                .budget
                .charge(self.work.function, 1, "complete HIR sequence validation")?;
            visits = visits.checked_add(1).ok_or_else(|| {
                self.work.budget.budget_refusal(
                    self.work.function,
                    "complete HIR validation visit count overflowed",
                    "",
                )
            })?;
            match &node.kind {
                ObservationDagKind::Alt(_) => return Ok((false, visits)),
                ObservationDagKind::Seq(prefix, _) => {
                    if let Some(prefix) = prefix {
                        self.work.budget.reserve_vec(
                            self.work.function,
                            &mut pending,
                            1,
                            "complete HIR sequence ancestor backing",
                        )?;
                        pending.push(prefix.as_ref());
                    }
                }
                ObservationDagKind::Cat(children) => {
                    self.work.budget.reserve_vec(
                        self.work.function,
                        &mut pending,
                        children.len(),
                        "complete HIR sequence concatenation backing",
                    )?;
                    let mut children = children.iter();
                    loop {
                        pay!(self, CHILD, "complete HIR memo child receivers");
                        let Some(child) = children.next() else { break };
                        pending.push(child.as_ref());
                    }
                }
            }
        }
        Ok((true, visits))
    }
}

// Read-only test observations; never consulted by classification/admission.
#[cfg(test)]
thread_local! {
    static COUNTS: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
}
#[cfg(test)]
fn note(cold: bool) {
    COUNTS.with(|counts| {
        let (classify, hits) = counts.get();
        counts.set((classify + usize::from(cold), hits + usize::from(!cold)));
    });
}

#[cfg(test)]
mod tests;
