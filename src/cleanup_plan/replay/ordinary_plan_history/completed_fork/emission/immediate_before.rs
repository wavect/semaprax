//! Exact query-enabled pre-sticky immediate emitter generation, test-only.
use super::*;

pub(super) const PAYMENT: usize = 2 * size_of::<&mut ReplayBudget>()
    + 2 * size_of::<&ResolvedFunction>()
    + 2 * size_of::<&str>()
    + 2 * size_of::<usize>()
    + 2 * size_of::<Result<(), Diagnostic>>();

// Actual/formal owned input, borrowed suffix, work and returned owners. The
// per-path bank below is renewed before every partial output construction.
pub(in crate::cleanup_plan::replay) const ENTRY: usize = 2 * size_of::<Vec<ExprSkeletonPath>>()
    + 2 * size_of::<&[ExprSkeletonPath]>()
    + 2 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + size_of::<Emission>()
    + size_of::<Result<Emission, Diagnostic>>()
    + size_of::<Vec<ExprSkeletonPath>>()
    + 2 * size_of::<ExprSkeletonPath>()
    + size_of::<std::vec::IntoIter<ExprSkeletonPath>>()
    + size_of::<Option<ExprSkeletonPath>>()
    + 2 * size_of::<ClassifierLedger>()
    + 12 * size_of::<bool>()
    + PAYMENT;
// These are the actual emitter's move/clone/extension/publication roles, not
// the older arena Path/Chunk layout. Callee backing remains separately paid.
const STEP: usize = 2 * size_of::<ExprSkeletonPath>()
    + 2 * size_of::<Observations>()
    + 4 * size_of::<Option<Observations>>() // retained owner and owned expect actual/formal
    + 2 * size_of::<&Option<Observations>>() // as_ref actual/formal receiver
    + 2 * size_of::<&mut Option<Observations>>() // take actual/formal receiver
    + 4 * size_of::<Option<&Observations>>() // as_ref result/receive, expect actual/formal
    + 2 * size_of::<Option<CleanupPlace>>()
    + 2 * size_of::<lexical_skeleton::Continuation>()
    + 4 * size_of::<&ExprSkeletonPath>()
    + 4 * size_of::<&Observations>()
    + 2 * size_of::<&Option<CleanupPlace>>()
    + 2 * size_of::<&mut Vec<ExprSkeletonPath>>()
    + 2 * size_of::<Result<Observations, Diagnostic>>()
    + 2 * size_of::<Result<Option<CleanupPlace>, Diagnostic>>()
    + 6 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 2 * size_of::<std::iter::Enumerate<std::slice::Iter<'static, ExprSkeletonPath>>>()
    + 2 * size_of::<Option<(usize, &ExprSkeletonPath)>>()
    + 4 * size_of::<usize>()
    + 8 * size_of::<bool>()
    + 6 * size_of::<&str>()
    + 2 * size_of::<Result<(), Diagnostic>>() // push result and caller receive
    + 4 * size_of::<&lexical_skeleton::Continuation>() // sequenced actual/formal args
    + 2 * PAYMENT;
const FREEZE: usize = 2 * size_of::<Observations>()
    + 2 * size_of::<Option<Rc<ObservationDag>>>()
    + 2 * size_of::<Vec<Rc<SkeletonObservation>>>()
    + size_of::<ObservationDagKind>();
const VALIDATE: usize = 2 * size_of::<(&Observations, &mut SkeletonWork<'static, 'static>)>()
    + size_of::<Result<bool, Diagnostic>>()
    + size_of::<Vec<&ObservationDag>>()
    + 2 * size_of::<&mut Vec<&ObservationDag>>()
    + 2 * size_of::<&ObservationDag>()
    + 4 * size_of::<usize>()
    + 4 * size_of::<Option<&ObservationDag>>()
    + 2 * size_of::<&ObservationDagKind>()
    + size_of::<std::slice::Iter<'static, Rc<ObservationDag>>>()
    + 2 * size_of::<Option<&Rc<ObservationDag>>>()
    + 4 * size_of::<bool>()
    + PAYMENT;

// BEGIN_FROZEN_VALIDATION
#[rustfmt::skip]
mod validation {
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
        self.work.queries.selector(self.work.function, &mut *self.work.budget,
            crate::cleanup_plan::replay::skeleton_work::query_proof::SELECTOR,
            "shared sequence cold partition receivers")?;
        if matches!(self.work.budget.payer,
            crate::cleanup_plan::replay::allocation::AllocationPayer::ActiveBuilder
                | crate::cleanup_plan::replay::allocation::AllocationPayer::LegacyUnobserved)
            && self.work.budget.admission == crate::cleanup_plan::replay::AdmissionMode::Ordinary
            && !self.work.budget.lent_skeleton && !self.work.budget.merge_paths
            && self.work.function.cleanup_plan.lexical_control.is_none()
        {
            self.work.queries.preflight(self.work.function, &mut *self.work.budget,
                crate::cleanup_plan::replay::skeleton_work::query_proof::CLASSIFY_CALL,
                "shared sequence classifier call receivers")?;
            return crate::cleanup_plan::replay::skeleton_work::query_proof::classify_paid(root, self.work);
        }
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

}
// END_FROZEN_VALIDATION

// BEGIN_FROZEN_EMITTER
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn emit_prepaid(
    prefixes: Vec<ExprSkeletonPath>,
    suffixes: &[ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Emission, Diagnostic> {
    work.budget.reserve_bytes(
        work.function,
        ClassifierLedger::capture_into_controls()
            + ClassifierLedger::current_receiving_controls()
            + PAYMENT,
        "complete HIR fork exact ledger receivers",
    )?;
    let mut ledger = ClassifierLedger::empty_receiving();
    ledger.capture_current_into();
    if work.budget.admission != AdmissionMode::Ordinary
        || !matches!(
            work.budget.payer,
            AllocationPayer::LegacyUnobserved | AllocationPayer::ActiveBuilder
        )
        || work.budget.lent_skeleton
        || work.budget.merge_paths
        || work.function.cleanup_plan.lexical_control.is_some()
    {
        return Ok(Emission::Unsupported(prefixes));
    }
    work.budget.reserve_bytes(
        work.function,
        sequence::SHARE_CONTROLS,
        "complete HIR renewal selection receivers",
    )?;
    if !sequence::without_renewal(work)? {
        return Ok(Emission::Unsupported(prefixes));
    }
    work.budget.reserve_bytes(
        work.function,
        validation::CALL,
        "complete HIR validation scope receivers",
    )?;
    if !validation::inputs(&prefixes, suffixes, work)? {
        return Ok(Emission::Unsupported(prefixes));
    }
    let mut result = Vec::new();
    for mut prefix in prefixes {
        work.budget
            .reserve_bytes(work.function, STEP, "complete HIR fork prefix receivers")?;
        if prefix.failed || prefix.residual {
            if result.len() >= MAX_REPLAY_PATHS {
                return Err(work.budget.budget_refusal(
                    work.function,
                    "complete HIR fork exceeds finite path profile",
                    "",
                ));
            }
            work.push_expr_path(&mut result, prefix, "short-circuited skeleton path")?;
            continue;
        }
        if suffixes.is_empty() {
            continue;
        }
        // The immediate predecessor's first clone work wins before new node
        // metadata. Later suffixes each retain their original clone work too.
        work.charge(1, "skeleton prefix clone")?;
        if suffixes.len() > 1 && !prefix.observations.items.is_empty() {
            work.budget.reserve_bytes(
                work.function,
                sequence::SHARE_CONTROLS + FREEZE,
                "complete HIR immutable fork publication",
            )?;
            let Observations {
                prefix: previous,
                items,
            } = prefix.observations;
            let kind = ObservationDagKind::Seq(previous, items);
            prefix.observations = Observations {
                prefix: Some(sequence::node(work, kind)?),
                items: Vec::new(),
            };
        }
        let mut remaining = Some(prefix.observations);
        for (index, suffix) in suffixes.iter().enumerate() {
            if !ledger.current() {
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
            work.budget.reserve_bytes(
                work.function,
                STEP + ClassifierLedger::current_receiving_controls(),
                "complete HIR fork output receivers",
            )?;
            if index != 0 {
                work.charge(1, "skeleton prefix clone")?;
            }
            let observations = if index + 1 == suffixes.len() {
                note_skeleton_materialization();
                remaining
                    .take()
                    .expect("final suffix moves the unique remaining prefix")
            } else {
                let source = remaining
                    .as_ref()
                    .expect("nonfinal suffix retains prefix owner");
                work.budget
                    .reserve_clone(work.function, source, "skeleton prefix clone")?;
                note_skeleton_materialization();
                source.clone()
            };
            let observations = if suffix.observations.prefix.is_some() {
                // Exactly the original suffix event; the existing sequence
                // factory charges additional bound/release traversal as free
                // work on BOTH public and builder payers, never the held loan.
                work.charge(1, "skeleton suffix clone")?;
                note_skeleton_materialization();
                work.budget.reserve_bytes(
                    work.function,
                    sequence::EXTEND_CONTROLS,
                    "complete HIR shared suffix call receivers",
                )?;
                sequence::extend(work, observations, &suffix.observations)?
            } else {
                work.extend_owned_observations(
                    observations,
                    &suffix.observations,
                    "skeleton suffix clone",
                )?
            };
            let owned_source = work.clone_owned(
                &suffix.owned_source,
                "sequenced skeleton owned-source clone",
            )?;
            if result.len() >= MAX_REPLAY_PATHS {
                return Err(work.budget.budget_refusal(
                    work.function,
                    "complete HIR fork exceeds finite path profile",
                    "",
                ));
            }
            work.push_expr_path(
                &mut result,
                ExprSkeletonPath {
                    observations,
                    owned_source,
                    failed: suffix.failed,
                    residual: suffix.residual,
                    continuation: lexical_skeleton::Continuation::sequenced(
                        &prefix.continuation,
                        &suffix.continuation,
                    ),
                },
                "sequenced skeleton path",
            )?;
        }
    }
    Ok(Emission::Complete(result))
}

// END_FROZEN_EMITTER

#[test]
fn complete_frozen_query_enabled_validation_and_emitter_sources_remain_exact() {
    use sha2::{Digest, Sha256};
    let hash = |raw: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(raw)));
    let raw = include_str!("immediate_before.rs");
    let validation = raw.split_once("mod validation {\n").unwrap().1
        .split_once("\n}\n// END_FROZEN_VALIDATION").unwrap().0;
    let restored = validation.to_owned() + "\n#[cfg(test)]\nmod tests;\n";
    assert_eq!(restored.len(), 11246);
    assert_eq!(hash(restored.as_bytes()), "1f55c14d5d7fe41614e045fc680d5df8a93d4e29e4698e47bdb7e39ce8203c68");
    let body = raw.split_once("// BEGIN_FROZEN_EMITTER\n").unwrap().1
        .split_once("\n// END_FROZEN_EMITTER").unwrap().0;
    assert_eq!(body.len(), 6046);
    assert_eq!(hash(body.as_bytes()), "052a4b2557c4d7627068ca16adb882b5f02a83099d86fbb9a35688a4ef5660be");
    let start = raw.find("// Actual/formal owned input, borrowed suffix, work and returned owners.").unwrap();
    let end = raw.find("// BEGIN_FROZEN_VALIDATION").unwrap();
    assert_eq!(hash(raw[start..end].as_bytes()), "5b439b4b27c9199439d8eac853d582080c9ac63c4b847d426aa880cd97c03993");
}
