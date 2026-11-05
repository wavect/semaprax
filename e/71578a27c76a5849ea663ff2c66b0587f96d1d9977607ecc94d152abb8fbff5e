//! Compact private observation storage; the canonical cleanup source stays intact.
//! The common body source stays inline. Only residual carriers need a second
//! allocation, acquired from the same live payer before that allocation exists.
#[cfg(test)]
use super::{
    CleanupPlace, CleanupTerminator, CleanupTransition, DeclarationId, EdgeCondition,
    ExitContinuation, Observations, ResolvedProgram, SkeletonObservation, SkeletonPath,
    SkeletonTerminal, StatusSourceId, VecDeque, lexical_cert, physical_clone, plan_skeleton_paths,
    replay_error, skeleton_clone, skeleton_materializations, skeleton_push, skeleton_queue_push,
};
use super::{
    Diagnostic, ExpressionId, ReplayBudget, ResolvedFunction, ResolvedType, StagedCopyResultSource,
};

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

impl Residual {
    pub(super) fn canonical(&self) -> &StagedCopyResultSource {
        &self.0
    }
}

impl CopySource {
    // These are actual caller/query/payment controls, admitted before the
    // source-kind match or the new function arguments. Initial failure remains
    // the ordinary unreceived admission boundary, not a donated owner.
    pub(super) const QUERY_CONTROLS: usize = 2 * std::mem::size_of::<Result<(), Diagnostic>>()
        + 3 * std::mem::size_of::<&ResolvedFunction>()
        + 3 * std::mem::size_of::<&mut ReplayBudget>()
        + 2 * std::mem::size_of::<&str>()
        + 2 * std::mem::size_of::<usize>()
        + std::mem::size_of::<&StagedCopyResultSource>();
    pub(super) const BODY_CONTROLS: usize = 2 * std::mem::size_of::<Result<Self, Diagnostic>>()
        + 2 * std::mem::size_of::<Self>()
        + std::mem::size_of::<StagedCopyResultSource>()
        + std::mem::size_of::<ExpressionId>()
        + std::mem::size_of::<ResolvedType>();
    pub(super) const RESIDUAL_CONTROLS: usize = 2 * std::mem::size_of::<Result<Self, Diagnostic>>()
        + 2 * std::mem::size_of::<Self>()
        + 3 * std::mem::size_of::<StagedCopyResultSource>()
        + 2 * std::mem::size_of::<Residual>()
        + 2 * std::mem::size_of::<Box<StagedCopyResultSource>>()
        + std::mem::size_of::<Result<(), Diagnostic>>()
        + std::mem::size_of::<&ResolvedFunction>()
        + std::mem::size_of::<&mut ReplayBudget>()
        + std::mem::size_of::<&str>()
        + std::mem::size_of::<usize>();

    #[track_caller]
    pub(super) fn new(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        source: StagedCopyResultSource,
    ) -> Result<Self, Diagnostic> {
        // The producer already owns `source` and has performed its complete
        // original clone census. This move earns no credit or refund.
        // All real callers use prepare! before evaluating these arguments.
        // The independent Main constructor/archive remains in the old domain.
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

// Two real admissions: common query/next-payment controls, then the selected
// constructor layout. No Box or residual binding is priced on the Body route.
macro_rules! prepare {
    ($function:expr, $budget:expr, $source:expr) => {{
        $budget.reserve_bytes(
            $function,
            $crate::cleanup_plan::replay::compact_stage::CopySource::QUERY_CONTROLS,
            "compact staged-source caller query controls",
        )?;
        match $source {
            $crate::cleanup_plan::StagedCopyResultSource::Body { .. } => {
                $budget.reserve_bytes(
                    $function,
                    $crate::cleanup_plan::replay::compact_stage::CopySource::BODY_CONTROLS,
                    "compact staged-source body receiving controls",
                )?;
            }
            _ => {
                $budget.reserve_bytes(
                    $function,
                    $crate::cleanup_plan::replay::compact_stage::CopySource::RESIDUAL_CONTROLS,
                    "compact staged-source residual receiving controls",
                )?;
            }
        }
    }};
    ($function:expr, $budget:expr, $source:expr, unwrap) => {{
        $budget
            .reserve_bytes(
                $function,
                $crate::cleanup_plan::replay::compact_stage::CopySource::QUERY_CONTROLS,
                "compact staged-source caller query controls",
            )
            .unwrap();
        match $source {
            $crate::cleanup_plan::StagedCopyResultSource::Body { .. } => {
                $budget
                    .reserve_bytes(
                        $function,
                        $crate::cleanup_plan::replay::compact_stage::CopySource::BODY_CONTROLS,
                        "compact staged-source body receiving controls",
                    )
                    .unwrap();
            }
            _ => {
                $budget
                    .reserve_bytes(
                        $function,
                        $crate::cleanup_plan::replay::compact_stage::CopySource::RESIDUAL_CONTROLS,
                        "compact staged-source residual receiving controls",
                    )
                    .unwrap();
            }
        }
    }};
}
pub(super) use prepare;

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

#[cfg(test)]
#[path = "compact_stage/source_custody_main8a.rs"]
pub(super) mod source_custody_main8a;

#[cfg(test)]
#[path = "compact_stage/source_custody_maina3.rs"]
pub(super) mod source_custody_maina3;

#[cfg(test)]
#[path = "compact_stage/source_custody_main44.rs"]
pub(super) mod source_custody_main44;

#[cfg(test)]
mod main_f247_source_custody;
