//! Draft-bound expression discovery from the existing last-valid source owner.
//! This does not release the retained candidate or validate a proposed hole.
use serde_json::{json, Value};

use super::{capacity, grammar, wire, ProjectCandidateDraft};
use crate::diagnostic::Diagnostic;

pub const PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA: &str =
    "semaprax.project-draft-expression-catalog.v1";
pub const MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES: usize = 1024 * 1024;

impl ProjectCandidateDraft {
    /// Discover current body selections after authenticating this exact draft.
    /// Rows describe its last valid source, not owned-value liveness or
    /// nonoverlap with pending holes. `with_expression_hole` owns admission.
    pub fn expression_catalog(
        &self,
        expected_draft: &str,
        target: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        self.draft_expression_catalog(expected_draft, target, false)
    }

    /// Discover current requires/ensures selections without exposing a
    /// materializable candidate. Ordinary contract-hole admission still applies.
    pub fn contract_expression_catalog(
        &self,
        expected_draft: &str,
        target: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        self.draft_expression_catalog(expected_draft, target, true)
    }

    fn draft_expression_catalog(
        &self,
        expected: &str,
        target: &str,
        contract: bool,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        let bytes = if contract {
            self.last_valid.contract_expression_catalog(target)?
        } else {
            self.last_valid.expression_catalog(target)?
        };
        if bytes.len() > MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES {
            return Err(capacity(
                "draft expression catalogue exceeds its source owner bound",
            ));
        }
        let value: Value = serde_json::from_str(&bytes)
            .map_err(|_| grammar("last-valid expression catalogue is invalid compiler JSON"))?;
        let mut fields = match value {
            Value::Object(fields) => fields,
            _ => return Err(grammar("last-valid expression catalogue is not an object")),
        };
        let keys = [
            "schema",
            "candidate_digest",
            "project_revision",
            "target",
            "source",
            "declared_effect_budget",
            "expressions",
            "limits",
            "nonclaims",
        ];
        let schema = if contract {
            "semaprax.project-contract-expression-catalog.v1"
        } else {
            "semaprax.project-expression-catalog.v1"
        };
        if fields.len() != keys.len()
            || keys.iter().any(|key| !fields.contains_key(*key))
            || fields["schema"] != schema
            || fields["candidate_digest"] != self.last_valid.candidate_digest()
            || fields["project_revision"] != self.last_valid.revision().project_revision()
            || fields["target"] != target
        {
            return Err(grammar("last-valid expression catalogue bindings disagree"));
        }
        let mut expressions = fields.remove("expressions").unwrap();
        let rows = expressions
            .as_array_mut()
            .ok_or_else(|| grammar("last-valid expression inventory is unavailable"))?;
        if rows
            .iter()
            .any(|row| !matches!(row["phase"].as_str(), Some("body" | "requires" | "ensures")))
        {
            return Err(grammar(
                "last-valid expression inventory has an unknown region",
            ));
        }
        rows.retain(|row| {
            if contract {
                row["phase"] != "body"
            } else {
                row["phase"] == "body"
            }
        });
        let mut nonclaims = fields.remove("nonclaims").unwrap();
        nonclaims
            .as_array_mut()
            .ok_or_else(|| grammar("last-valid expression nonclaims are unavailable"))?
            .extend([
                json!("not_pending_hole_nonoverlap_admission"),
                json!("query_does_not_register_last_valid_candidate"),
                json!("no_draft_completion_or_source_authority"),
            ]);
        wire::render(
            json!({
                "schema":PROJECT_DRAFT_EXPRESSION_CATALOG_SCHEMA,
                "draft_revision":self.draft_digest(),
                "last_valid_revision":self.last_valid.revision().project_revision(),
                "last_valid_candidate_digest":self.last_valid.candidate_digest(),
                "target":target,"region":if contract {"contract"} else {"body"},
                "source":fields.remove("source").unwrap(),
                "declared_effect_budget":fields.remove("declared_effect_budget").unwrap(),
                "expressions":expressions,"limits":fields.remove("limits").unwrap(),
                "materializable":false,"source_authority":false,
                "validation":"pending_fill_full_source_replay",
                "evidence_class":"last_valid_expression_inventory_not_draft_validation",
                "selection_admission":"requires_hole_open_validation","nonclaims":nonclaims,
            }),
            MAX_PROJECT_DRAFT_EXPRESSION_CATALOG_BYTES,
        )
        .map_err(|_| capacity("draft expression catalogue wrapper exceeds its byte bound"))
    }
}
