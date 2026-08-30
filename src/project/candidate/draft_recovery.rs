//! Recovery of pending selectors over independently replayed valid history.
//! No placeholder source, serialized contexts, or host authority is imported.
use super::*;
use crate::project::ProjectRevision;

pub const PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA: &str =
    "semaprax.project-candidate-draft-recovery.v1";
pub const PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY: &str =
    "semaprax.project-candidate-draft-recovery-compatibility.v1";
pub const MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES: usize = 64 * 1024 * 1024;
const MAX_JSON_NODES: usize =
    super::super::recovery::MAX_JSON_NODES + 16 * MAX_PROJECT_CANDIDATE_HOLES + 32;
const DOMAIN: &[u8] = b"semaprax.project-candidate-draft-recovery.payload.v1\0";
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
        let mut payload = json!({"schema":PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA,
            "compiler":compiler(),"base_revision":self.last_valid.base_revision().project_revision(),
            "draft_schema":PROJECT_CANDIDATE_DRAFT_SCHEMA,"candidate_recovery":candidate_recovery,
            "holes":holes,"draft_digest":self.draft_digest()});
        let canonical = encode(payload.clone())?;
        payload["capsule_digest"] = json!(wire::digest(DOMAIN, canonical.as_bytes()));
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
        object(
            &capsule,
            &[
                "schema",
                "compiler",
                "base_revision",
                "draft_schema",
                "candidate_recovery",
                "holes",
                "draft_digest",
                "capsule_digest",
            ],
        )?;
        if capsule["schema"] != PROJECT_CANDIDATE_DRAFT_RECOVERY_SCHEMA
            || capsule["compiler"] != compiler()
            || capsule["draft_schema"] != PROJECT_CANDIDATE_DRAFT_SCHEMA
        {
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
        if wire::digest(DOMAIN, encode(capsule.clone())?.as_bytes()) != expected_capsule {
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

fn compiler() -> Value {
    json!({"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),"compatibility":PROJECT_CANDIDATE_DRAFT_RECOVERY_COMPATIBILITY})
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
