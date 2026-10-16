//! Compact private observation storage; the canonical cleanup source stays intact.
//! The common body source stays inline. Only residual carriers need a second
//! allocation, acquired from the same live payer before that allocation exists.
use super::*;

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum CopySource {
    Body {
        expression: ExpressionId,
        instance: ResolvedType,
    },
    Residual(Residual),
}

/// Only this module can construct a non-body residual owner. Keeping the full
/// canonical source preserves every identity and both concrete result types.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct Residual(Box<StagedCopyResultSource>);

#[cfg(test)]
impl Residual {
    pub(super) fn canonical(&self) -> &StagedCopyResultSource {
        &self.0
    }
}

impl CopySource {
    fn receiving_bytes() -> usize {
        use std::mem::size_of;
        2 * size_of::<Result<Self, Diagnostic>>()
            + size_of::<Result<(), Diagnostic>>()
            + size_of::<&ResolvedFunction>()
            + size_of::<&mut ReplayBudget>()
            + size_of::<Residual>()
            + size_of::<usize>()
    }

    #[track_caller]
    pub(super) fn new(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        source: StagedCopyResultSource,
    ) -> Result<Self, Diagnostic> {
        // The producer already owns `source` and has performed its complete
        // original clone census. This move earns no credit or refund.
        budget.reserve_array::<u8>(
            function,
            Self::receiving_bytes(),
            "compact staged-source receiving layouts",
        )?;
        match source {
            StagedCopyResultSource::Body {
                expression,
                instance,
            } => Ok(Self::Body {
                expression,
                instance,
            }),
            source @ (StagedCopyResultSource::TryResidual { .. }
            | StagedCopyResultSource::TryOptionNone { .. }) => {
                budget.reserve_array::<StagedCopyResultSource>(
                    function,
                    1,
                    "compact staged-source residual backing",
                )?;
                Ok(Self::Residual(Residual(Box::new(source))))
            }
        }
    }

    #[cfg(test)]
    pub(super) fn matches_canonical(&self, source: &StagedCopyResultSource) -> bool {
        match (self, source) {
            (
                Self::Body {
                    expression: a,
                    instance: at,
                },
                StagedCopyResultSource::Body {
                    expression: b,
                    instance: bt,
                },
            ) => a == b && at == bt,
            (Self::Residual(source), other) => source.0.as_ref() == other,
            _ => false,
        }
    }
}

impl std::fmt::Debug for CopySource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Body {
                expression,
                instance,
            } => formatter
                .debug_struct("Body")
                .field("expression", expression)
                .field("instance", instance)
                .finish(),
            Self::Residual(source) => std::fmt::Debug::fmt(source.0.as_ref(), formatter),
        }
    }
}

#[cfg(test)]
pub(super) mod before;
#[cfg(test)]
pub(super) mod source_custody;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) fn original_paths(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<before::SkeletonPath>, Diagnostic> {
    before::plan_skeleton_paths(function, budget)
}

#[cfg(test)]
pub(super) fn paths_match(actual: &[SkeletonPath], expected: &[before::SkeletonPath]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.terminal == expected.terminal
                && actual.observations.prefix.is_none()
                && expected.observations.prefix.is_none()
                && actual.observations.items.len() == expected.observations.items.len()
                && actual
                    .observations
                    .items
                    .iter()
                    .zip(&expected.observations.items)
                    .all(|(a, b)| observations_match(a, b))
        })
}

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn observations_match(
    actual: &SkeletonObservation,
    expected: &before::SkeletonObservation,
) -> bool {
    use before::SkeletonObservation as Old;
    use SkeletonObservation as New;
    match (actual, expected) {
        (
            New::ReserveRenewal { at: a, binding: ap },
            Old::ReserveRenewal { at: b, binding: bp },
        ) => a == b && ap == bp,
        (
            New::Initialize {
                at: a,
                destination: ap,
            },
            Old::Initialize {
                at: b,
                destination: bp,
            },
        ) => a == b && ap == bp,
        (
            New::Transfer {
                at: a,
                source: ap,
                destination: aq,
            },
            Old::Transfer {
                at: b,
                source: bp,
                destination: bq,
            },
        ) => a == b && ap == bp && aq == bq,
        (
            New::CallCommit {
                call: a,
                arguments: ap,
            },
            Old::CallCommit {
                call: b,
                arguments: bp,
            },
        ) => a == b && ap == bp,
        (
            New::Boolean {
                expression: a,
                value: av,
            },
            Old::Boolean {
                expression: b,
                value: bv,
            },
        ) => a == b && av == bv,
        (
            New::VariantCase {
                scrutinee: a,
                case: ac,
                matches: av,
            },
            Old::VariantCase {
                scrutinee: b,
                case: bc,
                matches: bv,
            },
        ) => a == b && ac == bc && av == bv,
        (
            New::ArmSelected {
                scrutinee: a,
                arm: ac,
                selected: av,
            },
            Old::ArmSelected {
                scrutinee: b,
                arm: bc,
                selected: bv,
            },
        ) => a == b && ac == bc && av == bv,
        (
            New::Status {
                source: a,
                success: av,
            },
            Old::Status {
                source: b,
                success: bv,
            },
        ) => a == b && av == bv,
        (New::StageCopyResult(a), Old::StageCopyResult(b)) => a.matches_canonical(b),
        (
            New::LexicalExit {
                site: a,
                kind: ak,
                target: at,
            },
            Old::LexicalExit {
                site: b,
                kind: bk,
                target: bt,
            },
        ) => a == b && ak == bk && at == bt,
        (New::LoopLatch { target: a }, Old::LoopLatch { target: b }) => a == b,
        _ => false,
    }
}
