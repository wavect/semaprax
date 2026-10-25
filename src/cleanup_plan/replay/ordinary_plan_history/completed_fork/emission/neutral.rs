//! A neutral prefix has no observation, owned source or continuation to move.
//! Keep the old validation/query owner and work, but never construct the general
//! emitter's temporary Observations, Failure/Emission or swap receivers.
use super::*;

// This bank precedes every new length/field/Option/continuation inspection.
// Existing outer DISPATCH already owns the actual payer/lexical-domain test.
pub(in crate::cleanup_plan::replay) const SELECT: usize = 4 * size_of::<&Vec<ExprSkeletonPath>>()
    + 4 * size_of::<&[ExprSkeletonPath]>()
    + 4 * size_of::<&ExprSkeletonPath>()
    + 4 * size_of::<&Vec<Rc<crate::cleanup_plan::replay::SkeletonObservation>>>()
    + 4 * size_of::<&Option<Rc<ObservationDag>>>()
    + 4 * size_of::<&Option<crate::cleanup_plan::CleanupPlace>>()
    + 4 * size_of::<&lexical_skeleton::Continuation>()
    + 8 * size_of::<usize>()
    + 32 * size_of::<bool>()
    + PAYMENT;
// All work/clone calls retain their original implementation and payment. This
// bank pays only the new lexical output loans/owners and complete errors. It
// does not discount any bank whose helper remains on this branch.
pub(in crate::cleanup_plan::replay) const OUTPUT: usize = 2 * size_of::<Vec<ExprSkeletonPath>>()
    + 2 * size_of::<Result<Vec<ExprSkeletonPath>, Diagnostic>>()
    + 4 * size_of::<&mut Vec<ExprSkeletonPath>>()
    + 12 * size_of::<&[ExprSkeletonPath]>()
    + 4 * size_of::<&Option<Rc<ObservationDag>>>()
    + 4 * size_of::<&mut ExprSkeletonPath>()
    + 4 * size_of::<&ExprSkeletonPath>()
    + 4 * size_of::<&Rc<ObservationDag>>()
    + 2 * size_of::<Rc<ObservationDag>>()
    + 2 * size_of::<Option<Rc<ObservationDag>>>()
    + 2 * size_of::<Option<crate::cleanup_plan::CleanupPlace>>()
    + 4 * size_of::<&Option<crate::cleanup_plan::CleanupPlace>>()
    + 2 * size_of::<Result<Option<crate::cleanup_plan::CleanupPlace>, Diagnostic>>()
    + 12 * size_of::<&mut SkeletonWork<'static, 'static>>()
    + 12 * size_of::<&str>()
    + 12 * size_of::<usize>()
    + 8 * size_of::<Result<(), Diagnostic>>()
    + 2 * size_of::<ClassifierLedger>()
    + ClassifierLedger::capture_into_controls()
    + ClassifierLedger::current_receiving_controls()
    + 2 * size_of::<Diagnostic>()
    + 2 * size_of::<String>()
    + 2 * size_of::<std::borrow::Cow<'static, str>>()
    + 8 * size_of::<bool>()
    + 3 * PAYMENT;

macro_rules! dispatch {
    ($prefixes:ident, $suffixes:ident, $work:ident) => {{
        use $crate::bounded_output::classifier_string::ClassifierLedger;
        use $crate::cleanup_plan::replay::ordinary_plan_history::completed_fork::emission::{
            self, neutral, validation,
        };
        use $crate::cleanup_plan::replay::skeleton_work::enumerated_prefix as sequence;
        use $crate::cleanup_plan::replay::{lexical_skeleton, note_skeleton_materialization};
        // A test can execute the entire immediate caller by omitting just this
        // new lexical branch. No forwarding emitter frame pads its old invoice.
        'neutral_dispatch: {
            #[cfg(test)]
            if neutral::before_selected() {
                break 'neutral_dispatch match emission::emit!($prefixes, $suffixes, $work)? {
                    emission::Emission::Complete(paths) => return Ok(paths),
                    emission::Emission::Unsupported(paths) => paths,
                };
            }
            $work.budget.reserve_bytes(
                $work.function,
                neutral::SELECT,
                "neutral HIR prefix selector receivers",
            )?;
            let eligible = if $prefixes.len() == 1 && $suffixes.len() == 1 {
                let prefix = &$prefixes[0];
                let suffix = &$suffixes[0];
                !prefix.failed
                    && !prefix.residual
                    && prefix.observations.prefix.is_none()
                    && prefix.observations.items.is_empty()
                    && prefix.owned_source.is_none()
                    && matches!(&prefix.continuation, lexical_skeleton::Continuation::Normal)
                    && suffix.observations.items.is_empty()
                    && suffix.owned_source.is_none()
                    && matches!(&suffix.continuation, lexical_skeleton::Continuation::Normal)
            } else {
                false
            };
            if eligible {
                $work.budget.reserve_bytes(
                    $work.function,
                    neutral::OUTPUT + sequence::SHARE_CONTROLS,
                    "neutral HIR prefix output and renewal receivers",
                )?;
                let suffix = &$suffixes[0];
                let mut ledger = ClassifierLedger::empty_receiving();
                ledger.capture_current_into();
                // Keep the entire original renewal scan, including early false.
                // Unsupported inputs go directly to the caller's original fallback;
                // repeating the old emitter here would duplicate the work scan.
                if sequence::without_renewal($work)? {
                    $work.budget.reserve_bytes(
                        $work.function,
                        validation::CALL,
                        "complete HIR validation scope receivers",
                    )?;
                    if validation::inputs(&$prefixes, $suffixes, $work)? {
                        $work.charge(1, "skeleton prefix clone")?;
                        if !ledger.current() {
                            return Err($crate::diagnostic::Diagnostic::io(
                                "SPX-H006",
                                String::new(),
                            ));
                        }
                        note_skeleton_materialization();
                        $work.charge(1, "skeleton suffix clone")?;
                        note_skeleton_materialization();
                        // The original clone_owned call (including its zero-byte
                        // reservation and materialization) remains executable.
                        let owned_source = $work.clone_owned(
                            &suffix.owned_source,
                            "sequenced skeleton owned-source clone",
                        )?;
                        $work.charge(1, "sequenced skeleton path")?;
                        note_skeleton_materialization();
                        let mut paths = $prefixes;
                        let path = &mut paths[0];
                        path.observations.prefix = suffix.observations.prefix.clone();
                        path.owned_source = owned_source;
                        path.failed = suffix.failed;
                        path.residual = suffix.residual;
                        return Ok(paths);
                    }
                }
                $prefixes
            } else {
                match emission::emit!($prefixes, $suffixes, $work)? {
                    emission::Emission::Complete(paths) => return Ok(paths),
                    emission::Emission::Unsupported(paths) => paths,
                }
            }
        }
    }};
}
pub(in crate::cleanup_plan::replay) use dispatch;

#[cfg(test)]
thread_local! {
    static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn before_selected() -> bool {
    BEFORE.with(|value| value.get())
}
#[cfg(test)]
fn before<T>(body: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(BEFORE.with(|value| value.replace(true)));
    body()
}
#[cfg(test)]
mod tests;
