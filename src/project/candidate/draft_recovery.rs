//! Recovery of pending selectors over independently replayed valid history.
//! No placeholder source, serialized contexts, or host authority is imported.
use super::*;
use crate::project::ProjectRevision;

pub const PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA: &str =
    "semaprax.project-candidate-draft-recovery.v1";
pub const PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_SCHEMA: &str =
    "semaprax.project-candidate-draft-recovery.v2";
pub const PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY: &str =
    "semaprax.project-candidate-draft-recovery-compatibility.v1";
pub const PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_COMPATIBILITY: &str =
    "semaprax.project-candidate-draft-recovery-compatibility.v2";
pub const MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES: usize = 64 * 1024 * 1024;
const MAX_JSON_NODES: usize = super::super::recovery::MAX_JSON_NODES
    + 16 * MAX_PROJECT_CANDIDATE_HOLES
    + 32 * MAX_PROJECT_CANDIDATE_DRAFT_LINEAGE
    + 32;
const DOMAIN: &[u8] = b"semaprax.project-candidate-draft-recovery.payload.v1\0";
const LINEAGE_DOMAIN: &[u8] = b"semaprax.project-candidate-draft-recovery.payload.v2\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

impl ProjectCandidateDraft {
    /// Export the prior valid history and unresolved selectors, never a proof
    /// that the unfinished draft is a checked or materializable candidate.
    pub fn recovery_capsule(&self) -> Result<String> {
        let history = self.last_valid.recovery_capsule()?;
        let candidate_recovery: Value = serde_json::from_str(&history)
            .map_err(|_| grammar("retained candidate recovery is invalid"))?;
        let mut holes = Vec::new();
        for (hole_id, target) in &self.holes {
            holes.push(json!({"kind":"function_body","hole_id":hole_id,"target":target}));
        }
        for (hole_id, (target, expression_id)) in &self.expression_holes {
            holes.push(json!({"kind":"expression","hole_id":hole_id,"target":target,"expression_id":expression_id}));
        }
        for (hole_id, (target, expression_id)) in &self.contract_expression_holes {
            holes.push(json!({"kind":"contract_expression","hole_id":hole_id,"target":target,"expression_id":expression_id}));
        }
        holes.sort_by(|left, right| left["hole_id"].as_str().cmp(&right["hole_id"].as_str()));
        let filled_hole_lineage = self
            .filled_holes
            .values()
            .map(FilledHoleLineage::json)
            .collect::<Vec<_>>();
        let branch_ancestry = self
            .ancestry
            .iter()
            .map(DraftAncestry::json)
            .collect::<Vec<_>>();
        let lineage = !self.filled_holes.is_empty() || !self.ancestry.is_empty();
        let mut payload = json!({"schema":PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
            "compiler":compiler(),"base_revision":self.last_valid.base_revision().project_revision(),
            "draft_schema":PROJECT_CANDIDATE_DRAFT_SCHEMA,"candidate_recovery":candidate_recovery,
            "holes":holes,"draft_digest":self.draft_digest()});
        let domain = if lineage {
            payload["schema"] = json!(PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_SCHEMA);
            payload["compiler"] = lineage_compiler();
            payload["draft_schema"] = json!(PROJECT_CANDIDATE_DRAFT_LINEAGE_SCHEMA);
            payload["filled_hole_lineage"] = json!(filled_hole_lineage);
            payload["branch_ancestry"] = json!(branch_ancestry);
            LINEAGE_DOMAIN
        } else {
            DOMAIN
        };
        let canonical = encode(payload.clone())?;
        payload["capsule_digest"] = json!(wire::digest(domain, canonical.as_bytes()));
        let encoded = encode(payload)?;
        preflight(encoded.as_bytes())?;
        Ok(encoded)
    }

    /// Rebuild the nested valid candidate from its original admitted base, then
    /// recreate every pending hole through the ordinary selection/overlap APIs.
    pub fn restore(base: Arc<ProjectRevision>, expected_base: &str, bytes: &[u8]) -> Result<Self> {
        check_digest(expected_base)?;
        if expected_base != base.project_revision() {
            return Err(stale(
                "draft recovery requires its exact admitted original base",
            ));
        }
        if bytes.len() > MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES {
            return Err(capacity("draft recovery exceeds its byte bound"));
        }
        preflight(bytes)?;
        let mut capsule: Value = serde_json::from_slice(bytes)
            .map_err(|_| grammar("draft recovery must be canonical JSON"))?;
        let lineage = capsule["schema"] == PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_SCHEMA;
        object(
            &capsule,
            if lineage {
                &[
                    "schema",
                    "compiler",
                    "base_revision",
                    "draft_schema",
                    "candidate_recovery",
                    "holes",
                    "filled_hole_lineage",
                    "branch_ancestry",
                    "draft_digest",
                    "capsule_digest",
                ][..]
            } else {
                &[
                    "schema",
                    "compiler",
                    "base_revision",
                    "draft_schema",
                    "candidate_recovery",
                    "holes",
                    "draft_digest",
                    "capsule_digest",
                ][..]
            },
        )?;
        let schema_valid = if lineage {
            capsule["compiler"] == lineage_compiler()
                && capsule["draft_schema"] == PROJECT_CANDIDATE_DRAFT_LINEAGE_SCHEMA
        } else {
            capsule["schema"] == PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA
                && capsule["compiler"] == compiler()
                && capsule["draft_schema"] == PROJECT_CANDIDATE_DRAFT_SCHEMA
        };
        if !schema_valid {
            return Err(grammar(
                "draft recovery compiler compatibility or schema does not match",
            ));
        }
        let original_base = digest(&capsule, "base_revision")?;
        if original_base != expected_base {
            return Err(stale(
                "draft recovery original base disagrees with the admitted revision",
            ));
        }
        let expected_draft = digest(&capsule, "draft_digest")?.to_owned();
        let expected_capsule = digest(&capsule, "capsule_digest")?.to_owned();
        if !capsule["candidate_recovery"].is_object() {
            return Err(grammar(
                "draft recovery requires a candidate recovery object",
            ));
        }
        let holes = capsule["holes"]
            .as_array()
            .ok_or_else(|| grammar("draft recovery holes must be an ordered array"))?;
        if holes.len() > MAX_PROJECT_CANDIDATE_HOLES {
            return Err(capacity("draft recovery exceeds its pending hole bound"));
        }
        let filled_holes = if lineage {
            parse_filled_lineage(&capsule["filled_hole_lineage"])?
        } else {
            BTreeMap::new()
        };
        let ancestry = if lineage {
            parse_ancestry(&capsule["branch_ancestry"])?
        } else {
            Vec::new()
        };
        let mut prior = None;
        for hole in holes {
            match text(hole, "kind")? {
                "function_body" => object(hole, &["kind", "hole_id", "target"])?,
                "expression" | "contract_expression" => {
                    object(hole, &["kind", "hole_id", "target", "expression_id"])?;
                    selector(text(hole, "expression_id")?)?;
                }
                _ => return Err(grammar("draft recovery hole kind is unsupported")),
            }
            let id = text(hole, "hole_id")?;
            validate_id(id)?;
            if prior.is_some_and(|prior| prior >= id) {
                return Err(grammar(
                    "draft recovery holes must have unique IDs in canonical order",
                ));
            }
            prior = Some(id);
            selector(text(hole, "target")?)?;
        }
        if encode(capsule.clone())?.as_bytes() != bytes {
            return Err(grammar("draft recovery requires exact canonical bytes"));
        }
        capsule
            .as_object_mut()
            .expect("closed object checked")
            .remove("capsule_digest");
        let domain = if lineage { LINEAGE_DOMAIN } else { DOMAIN };
        if wire::digest(domain, encode(capsule.clone())?.as_bytes()) != expected_capsule {
            return Err(stale("draft recovery content digest does not match"));
        }
        // All outer grammar, compatibility and digest checks precede replay.
        // The nested capsule still passes its own unchanged byte/node limits.
        let candidate_bytes = wire::render(
            capsule["candidate_recovery"].clone(),
            super::super::recovery::MAX_PROJECT_CANDIDATE_RECOVERY_BYTES,
        )?;
        let candidate = ProjectCandidate::restore(base, expected_base, candidate_bytes.as_bytes())?;
        let mut draft = Self::open(Arc::new(candidate))?;
        for hole in capsule["holes"].as_array().expect("array checked") {
            let expected = draft.draft_digest().to_owned();
            draft = match text(hole, "kind")? {
                "function_body" => draft.with_body_hole(
                    &expected,
                    text(hole, "target")?,
                    text(hole, "hole_id")?,
                )?,
                "expression" => draft.with_expression_hole(
                    &expected,
                    text(hole, "target")?,
                    text(hole, "expression_id")?,
                    text(hole, "hole_id")?,
                )?,
                "contract_expression" => draft.with_contract_expression_hole(
                    &expected,
                    text(hole, "target")?,
                    text(hole, "expression_id")?,
                    text(hole, "hole_id")?,
                )?,
                _ => unreachable!("closed kind checked"),
            };
        }
        draft = Self::finish(
            Arc::clone(&draft.last_valid),
            draft.holes,
            draft.expression_holes,
            draft.contract_expression_holes,
            filled_holes,
            ancestry,
        )?;
        if draft.draft_digest() != expected_draft {
            return Err(stale(
                "draft recovery replay disagrees with the final draft identity",
            ));
        }
        if draft.recovery_capsule()?.as_bytes() != bytes {
            return Err(stale("draft recovery exact capsule replay disagrees"));
        }
        Ok(draft)
    }
}

fn parse_filled_lineage(value: &Value) -> Result<BTreeMap<String, FilledHoleLineage>> {
    let rows = value
        .as_array()
        .ok_or_else(|| grammar("draft recovery filled-hole lineage must be an ordered array"))?;
    if rows.len() > MAX_PROJECT_CANDIDATE_DRAFT_LINEAGE {
        return Err(capacity(
            "draft recovery filled-hole lineage exceeds its bound",
        ));
    }
    let mut result = BTreeMap::new();
    let mut prior = None;
    for row in rows {
        object(
            row,
            &[
                "event_id",
                "hole_id",
                "kind",
                "target",
                "expression_id",
                "intent_digest",
                "history_ordinal",
                "origin_draft_digest",
            ],
        )?;
        let event_id = digest(row, "event_id")?;
        let intent_digest = digest(row, "intent_digest")?;
        let origin = digest(row, "origin_draft_digest")?;
        if prior.is_some_and(|prior| prior >= event_id) {
            return Err(grammar(
                "draft recovery filled-hole lineage must be uniquely ordered",
            ));
        }
        prior = Some(event_id);
        let hole_id = text(row, "hole_id")?;
        validate_id(hole_id)?;
        let kind = text(row, "kind")?;
        if !matches!(
            kind,
            "replace_function_body" | "replace_expression" | "replace_contract_expression"
        ) {
            return Err(grammar(
                "draft recovery filled-hole intention kind is unsupported",
            ));
        }
        let target = text(row, "target")?;
        selector(target)?;
        let expression_id = match &row["expression_id"] {
            Value::Null => None,
            Value::String(value) => {
                selector(value)?;
                Some(value.clone())
            }
            _ => {
                return Err(grammar(
                    "draft recovery lineage expression selector is invalid",
                ))
            }
        };
        if (kind == "replace_function_body") != expression_id.is_none() {
            return Err(grammar(
                "draft recovery filled-hole selector disagrees with its intention kind",
            ));
        }
        let history_ordinal = row["history_ordinal"]
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| grammar("draft recovery lineage history ordinal is invalid"))?;
        result.insert(
            event_id.to_owned(),
            FilledHoleLineage {
                event_id: event_id.to_owned(),
                hole_id: hole_id.to_owned(),
                kind: kind.to_owned(),
                target: target.to_owned(),
                expression_id,
                intent_digest: intent_digest.to_owned(),
                history_ordinal,
                origin_draft_digest: origin.to_owned(),
            },
        );
    }
    Ok(result)
}

fn parse_ancestry(value: &Value) -> Result<Vec<DraftAncestry>> {
    let rows = value
        .as_array()
        .ok_or_else(|| grammar("draft recovery branch ancestry must be an ordered array"))?;
    if rows.len() > MAX_PROJECT_CANDIDATE_DRAFT_LINEAGE {
        return Err(capacity("draft recovery branch ancestry exceeds its bound"));
    }
    let mut result = Vec::with_capacity(rows.len());
    for row in rows {
        object(row, &["operation", "parents", "onto_revision"])?;
        let operation = text(row, "operation")?;
        let parents = row["parents"]
            .as_array()
            .ok_or_else(|| grammar("draft recovery ancestry parents must be an array"))?;
        let expected_parents = match operation {
            "rebase" => 1,
            "merge" => 2,
            _ => return Err(grammar("draft recovery ancestry operation is unsupported")),
        };
        if parents.len() != expected_parents {
            return Err(grammar("draft recovery ancestry parent count disagrees"));
        }
        let mut parsed = Vec::with_capacity(parents.len());
        for parent in parents {
            let parent = parent
                .as_str()
                .ok_or_else(|| grammar("draft recovery ancestry parent must be a digest"))?;
            check_digest(parent)?;
            parsed.push(parent.to_owned());
        }
        let onto_revision = match &row["onto_revision"] {
            Value::Null if operation == "merge" => None,
            Value::String(value) if operation == "rebase" => {
                check_digest(value)?;
                Some(value.clone())
            }
            _ => return Err(grammar("draft recovery ancestry destination disagrees")),
        };
        result.push(DraftAncestry {
            operation: operation.to_owned(),
            parents: parsed,
            onto_revision,
        });
    }
    Ok(result)
}

fn compiler() -> Value {
    json!({"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),"compatibility":PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY})
}
fn lineage_compiler() -> Value {
    json!({"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),"compatibility":PROJECT_CANDIDATE_DRAFT_LINEAGE_RECOVERY_COMPATIBILITY})
}
fn encode(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES)
        .map_err(|_| capacity("draft recovery output exceeds its byte bound"))
}
fn check_digest(text: &str) -> Result<()> {
    if text.len() != 71
        || !text.starts_with("sha256:")
        || !text.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(grammar("draft recovery digest must be canonical SHA-256"));
    }
    Ok(())
}
fn digest<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    let text = text(value, field)?;
    check_digest(text)?;
    Ok(text)
}
fn selector(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 4096 || value.contains('\0') {
        return Err(grammar(
            "draft recovery selector must be a bounded stable identity",
        ));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| grammar("draft recovery field must be text"))
}
fn object(value: &Value, keys: &[&str]) -> Result<()> {
    let value = value
        .as_object()
        .ok_or_else(|| grammar("draft recovery value must be an object"))?;
    if value.len() != keys.len() || keys.iter().any(|key| !value.contains_key(*key)) {
        return Err(grammar("draft recovery has missing or unknown fields"));
    }
    Ok(())
}

// Bound potential Value allocations before JSON parsing. Strings, scalar
// tokens and containers each count once; syntax remains the parser's job.
fn preflight(bytes: &[u8]) -> Result<()> {
    let (mut string, mut escape, mut scalar) = (false, false, false);
    let (mut depth, mut nodes) = (0usize, 0usize);
    for byte in bytes {
        if string {
            if escape {
                escape = false;
            } else if *byte == b'\\' {
                escape = true;
            } else if *byte == b'"' {
                string = false;
            }
            continue;
        }
        match byte {
            b'"' => {
                string = true;
                scalar = false;
                nodes += 1;
            }
            b'{' | b'[' => {
                depth += 1;
                scalar = false;
                nodes += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                scalar = false;
            }
            b':' | b',' | b' ' | b'\t' | b'\r' | b'\n' => scalar = false,
            _ => {
                if !scalar {
                    scalar = true;
                    nodes += 1;
                }
            }
        }
        if depth > super::super::recovery::MAX_JSON_DEPTH || nodes > MAX_JSON_NODES {
            return Err(capacity(
                "draft recovery JSON exceeds its bounded history and hole inventory",
            ));
        }
    }
    Ok(())
}
