//! Read-only, exact-session eligibility for one bounded checked-i64 fold.
//! No alternate scheduler is dispatched by this module.

use serde_json::json;

use crate::assurance_manifest::law_set::{native_proof, LawSet};
use crate::ast::BinaryOp;
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;

use super::{
    kernel_ir::KernelExpr, BufferHandle, CpuReferenceSession, KernelArtifact, KernelShape, Scalar,
    ScalarKind, MAX_BUFFER_ELEMENTS,
};

pub const REDUCTION_ELIGIBILITY_SCHEMA: &str = "semaprax.law-reduction-eligibility.v1";

/// Every input element must lie in this closed nonnegative range. The count
/// bound and maximum element are checked in i128 so every partial sum in any
/// grouping is representable as an i64.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReductionDomain {
    pub minimum: i64,
    pub maximum: i64,
    pub maximum_elements: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReductionSchedule {
    /// The scheduler may change parentheses but keeps element order.
    Regroup,
    /// The scheduler may also change element order; commutativity is needed.
    Reorder,
}

impl CpuReferenceSession {
    /// Analyze one live fold artifact and buffer pair. This never dispatches
    /// a parallel fold or applies a rewrite. Only an exact `acc + element`
    /// checked-i64 body, an installed proof of zero identity, disjoint live
    /// buffers and a total nonnegative arithmetic domain can be eligible.
    pub fn checked_add_reduction_eligibility(
        &self,
        revision: &ProjectRevision,
        artifact: &KernelArtifact,
        input: BufferHandle,
        output: BufferHandle,
        laws: &LawSet,
        identity_law_id: &str,
        identity_proof: &native_proof::VerifiedLawProof,
        domain: ReductionDomain,
        schedule: ReductionSchedule,
    ) -> Result<String, Vec<Diagnostic>> {
        self.check_artifact(revision.entry_program(), artifact)
            .map_err(|error| vec![error.diagnostic()])?;
        native_proof::require_checked_i64_add_zero_identity(
            revision,
            laws,
            identity_law_id,
            identity_proof,
        )?;
        let (input_kind, values) = self.live(input).map_err(|error| vec![error.diagnostic()])?;
        let (output_kind, output_values) = self
            .live(output)
            .map_err(|error| vec![error.diagnostic()])?;
        let reason = if artifact.shape != KernelShape::SequentialFold {
            Some("not_a_sequential_fold")
        } else if !pure_add_body(&artifact.ir.body)
            || artifact.ir.params != [ScalarKind::I64, ScalarKind::I64]
            || artifact.ir.result != ScalarKind::I64
        {
            Some("operation_is_not_exact_checked_i64_addition")
        } else if input == output || input_kind != ScalarKind::I64 || output_kind != ScalarKind::I64
            || output_values.len() != 1
        {
            Some("buffer_alias_or_type_outside_admitted_fold")
        } else if domain.minimum < 0
            || domain.maximum < domain.minimum
            || domain.maximum_elements == 0
            || domain.maximum_elements > MAX_BUFFER_ELEMENTS
            || values.len() > domain.maximum_elements
        {
            Some("nonnegative_bounded_domain_unavailable")
        } else if i128::from(domain.maximum) * (domain.maximum_elements as i128)
            > i128::from(i64::MAX)
        {
            Some("some_grouping_may_overflow_checked_i64")
        } else if values.iter().any(|value| {
            !matches!(value, Scalar::I64(number) if *number >= domain.minimum && *number <= domain.maximum)
        }) {
            Some("current_input_outside_proved_domain")
        } else {
            None
        };
        let report = json!({
            "schema": REDUCTION_ELIGIBILITY_SCHEMA,
            "project_revision": revision.project_revision(),
            "operation_id": artifact.declaration(),
            "artifact_fingerprint": artifact.fingerprint(),
            "law_set_digest": laws.digest(),
            "identity_law_id": identity_law_id,
            "identity_law_semantic_digest": laws.semantic_digest(identity_law_id),
            "numeric_semantics": "checked_i64",
            "domain": {"minimum":domain.minimum,"maximum":domain.maximum,"maximum_elements":domain.maximum_elements},
            "schedule": match schedule { ReductionSchedule::Regroup => "regroup_preserving_order", ReductionSchedule::Reorder => "reorder_elements" },
            "associativity": if reason.is_none() { "checked_by_exact_nonnegative_sum_bound" } else { "unavailable" },
            "identity": "proved_by_installed_native_law",
            "commutativity_required": schedule == ReductionSchedule::Reorder,
            "commutativity": if reason.is_none() && schedule == ReductionSchedule::Reorder { "checked_by_exact_integer_addition_bound" } else { "not_used" },
            "eligible": reason.is_none(),
            "reason": reason,
            "transformation_applied": false,
            "parallel_execution_occurred": false,
            "existing_fold_order": "sequential_left_to_right",
            "tcb": ["checked_project_hir_and_cpu_reference_binding", "installed_native_law_proof", "compiler_arithmetic_bound_checker"],
        });
        serde_json::to_string(&report).map_err(|error| {
            vec![Diagnostic::io(
                "SPX-GC014",
                format!("reduction report rendering: {error}"),
            )]
        })
    }
}

fn pure_add_body(body: &KernelExpr) -> bool {
    match body {
        KernelExpr::Block { lets, tail } if lets.is_empty() => pure_add_body(tail),
        KernelExpr::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } => {
            matches!(left.as_ref(), KernelExpr::Slot(0))
                && matches!(right.as_ref(), KernelExpr::Slot(1))
        }
        _ => false,
    }
}
