//! One proof-gated, source-derived optimization. This produces an ordinary
//! immutable Project candidate; it never edits or publishes authoritative
//! source, and the candidate's full Project admission runs after the rewrite.

use serde_json::json;

use crate::assurance_manifest::law_set::{native_proof, LawSet};
use crate::ast::BinaryOp;
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExprKind, ResolvedType};

use super::{expression, parse_revision, ProjectCandidate, SemanticChange};

impl ProjectCandidate {
    /// Replace one authored `i64` place expression `x + 0` with `x`, after a
    /// real native proof of the exact assumption-free scalar identity has been
    /// rebound to this candidate's Project and law inventory. The left place
    /// is still read once, at the same evaluation position. Checked addition
    /// by zero cannot overflow, so no trap or effect is removed.
    pub fn propose_checked_i64_add_zero(
        &self,
        expected_candidate: &str,
        target: &str,
        expression_id: &str,
        laws: &LawSet,
        law_id: &str,
        proof: &native_proof::VerifiedLawProof,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.require_candidate(expected_candidate)?;
        native_proof::require_checked_i64_add_zero_identity(&self.revision, laws, law_id, proof)?;
        let programs = parse_revision(&self.revision)?;
        let selected =
            expression::authored_selection(&self.revision, &programs, target, expression_id)?;
        let ResolvedExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = &selected.expression.kind
        else {
            return Err(refused());
        };
        let ResolvedExprKind::Place(place) = &left.kind else {
            return Err(refused());
        };
        if !place.projections.is_empty()
            || !matches!(right.kind, ResolvedExprKind::Int(0))
            || selected.expression.ty != ResolvedType::I64
            || left.ty != ResolvedType::I64
            || right.ty != ResolvedType::I64
        {
            return Err(refused());
        }
        let binding = selected
            .scope
            .iter()
            .find(|binding| binding.id == place.root.as_str())
            .ok_or_else(refused)?;
        let change = SemanticChange::new(
            self.revision.project_revision(),
            &json!({
                "kind":"replace_expression",
                "target":target,
                "expression_id":expression_id,
                "replacement":{"kind":"place","name":binding.name},
            }),
        )?;
        self.apply(expected_candidate, &change)
    }
}

fn refused() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G225",
        "law optimization admits only an authored i64 place plus literal zero".to_owned(),
    )]
}
