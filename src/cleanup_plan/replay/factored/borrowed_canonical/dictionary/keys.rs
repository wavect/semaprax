//! Exact derived key order, with debit at each evaluated field or byte.
use super::*;
use crate::cleanup_plan::replay::compact_stage::CopySource;
macro_rules! numeric {
    ($($ty:ty),+) => {$(impl Key for $ty {
        fn compare(&self, other: &Self, function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<Ordering, Diagnostic> {
            budget.charge(function, 1, "borrowed dictionary numeric key field")?;
            Ok(self.cmp(other))
        }
    })+};
}
numeric!(usize, u64);
impl<A: Key, B: Key> Key for (A, B) {
    fn compare(
        &self,
        other: &Self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        let first = self.0.compare(&other.0, function, budget)?;
        if first != Ordering::Equal {
            return Ok(first);
        }
        self.1.compare(&other.1, function, budget)
    }
}
impl<T: Key> Key for Vec<T> {
    fn compare(
        &self,
        other: &Self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        for (a, b) in self.iter().zip(other) {
            budget.charge(function, 1, "borrowed dictionary actual vector key member")?;
            let order = a.compare(b, function, budget)?;
            if order != Ordering::Equal {
                return Ok(order);
            }
        }
        budget.charge(function, 1, "borrowed dictionary vector key length")?;
        Ok(self.len().cmp(&other.len()))
    }
}
fn text(
    a: &str,
    b: &str,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    for (a, b) in a.bytes().zip(b.bytes()) {
        budget.charge(function, 1, "borrowed label actual identity byte")?;
        let order = a.cmp(&b);
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    budget.charge(function, 1, "borrowed label identity length")?;
    Ok(a.len().cmp(&b.len()))
}
fn ordinal(observation: &SkeletonObservation) -> u8 {
    match observation {
        SkeletonObservation::ReserveRenewal { .. } => 0,
        SkeletonObservation::Initialize { .. } => 1,
        SkeletonObservation::Transfer { .. } => 2,
        SkeletonObservation::CallCommit { .. } => 3,
        SkeletonObservation::Boolean { .. } => 4,
        SkeletonObservation::VariantCase { .. } => 5,
        SkeletonObservation::ArmSelected { .. } => 6,
        SkeletonObservation::Status { .. } => 7,
        SkeletonObservation::StageCopyResult(_) => 8,
        SkeletonObservation::LexicalExit { .. } => 9,
        SkeletonObservation::LoopLatch { .. } => 10,
    }
}
fn boolean(
    a: bool,
    b: bool,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Ordering, Diagnostic> {
    budget.charge(function, 1, "borrowed label boolean field")?;
    Ok(a.cmp(&b))
}
impl Key for Label {
    fn compare(
        &self,
        other: &Self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Ordering, Diagnostic> {
        #[cfg(test)]
        if budget.oracle_lease().is_some() {
            return crate::cleanup_plan::replay::factored::oracle_order::label(
                self, other, function, budget,
            );
        }
        budget.charge(function, 1, "borrowed label actual kind")?;
        let (a, b) = match (self, other) {
            (Label::End(a), Label::End(b)) => return Ok(a.cmp(b)),
            (Label::Observation(_), Label::End(_)) => return Ok(Ordering::Less),
            (Label::End(_), Label::Observation(_)) => return Ok(Ordering::Greater),
            (Label::Observation(a), Label::Observation(b)) => (a.as_ref(), b.as_ref()),
        };
        budget.charge(function, 1, "borrowed observation actual kind")?;
        let rank = ordinal(a).cmp(&ordinal(b));
        if rank != Ordering::Equal {
            return Ok(rank);
        }
        macro_rules! field {
            ($a:expr,$b:expr) => {
                let order = text($a, $b, function, budget)?;
                if order != Ordering::Equal {
                    return Ok(order);
                }
            };
        }
        match (a, b) {
            (
                SkeletonObservation::CallCommit {
                    call: a,
                    arguments: aa,
                },
                SkeletonObservation::CallCommit {
                    call: b,
                    arguments: ba,
                },
            ) => {
                field!(a.as_str(), b.as_str());
                budget.charge(function, 1, "borrowed empty owning argument vectors")?;
                if !aa.is_empty() || !ba.is_empty() {
                    return Err(replay_error(
                        function,
                        "borrowed canonical label carries owned arguments",
                    ));
                }
                Ok(Ordering::Equal)
            }
            (
                SkeletonObservation::Boolean {
                    expression: a,
                    value: av,
                },
                SkeletonObservation::Boolean {
                    expression: b,
                    value: bv,
                },
            ) => {
                field!(a.as_str(), b.as_str());
                boolean(*av, *bv, function, budget)
            }
            (
                SkeletonObservation::VariantCase {
                    scrutinee: a,
                    case: ac,
                    matches: av,
                },
                SkeletonObservation::VariantCase {
                    scrutinee: b,
                    case: bc,
                    matches: bv,
                },
            ) => {
                field!(a.as_str(), b.as_str());
                field!(ac.as_str(), bc.as_str());
                boolean(*av, *bv, function, budget)
            }
            (
                SkeletonObservation::ArmSelected {
                    scrutinee: a,
                    arm: aa,
                    selected: av,
                },
                SkeletonObservation::ArmSelected {
                    scrutinee: b,
                    arm: ba,
                    selected: bv,
                },
            ) => {
                field!(a.as_str(), b.as_str());
                budget.charge(function, 1, "borrowed label arm ordinal")?;
                let order = aa.cmp(ba);
                if order != Ordering::Equal {
                    return Ok(order);
                }
                boolean(*av, *bv, function, budget)
            }
            (
                SkeletonObservation::Status {
                    source: a,
                    success: av,
                },
                SkeletonObservation::Status {
                    source: b,
                    success: bv,
                },
            ) => {
                field!(a.expression.as_str(), b.expression.as_str());
                budget.charge(function, 1, "borrowed label status lane")?;
                let order = a.lane.cmp(&b.lane);
                if order != Ordering::Equal {
                    return Ok(order);
                }
                boolean(*av, *bv, function, budget)
            }
            (
                SkeletonObservation::StageCopyResult(CopySource::Body {
                    expression: a,
                    instance: at,
                }),
                SkeletonObservation::StageCopyResult(CopySource::Body {
                    expression: b,
                    instance: bt,
                }),
            ) => {
                field!(a.as_str(), b.as_str());
                budget.charge(function, 1, "borrowed label scalar result type")?;
                if !crate::hir::is_scalar_resolved_type(at)
                    || !crate::hir::is_scalar_resolved_type(bt)
                {
                    return Err(replay_error(
                        function,
                        "borrowed canonical label has a non-scalar result",
                    ));
                }
                Ok(at.cmp(bt))
            }
            _ => Err(replay_error(
                function,
                "borrowed canonical label is outside its complete profile",
            )),
        }
    }
}
