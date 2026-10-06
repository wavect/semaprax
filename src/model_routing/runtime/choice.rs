//! Runtime agent and tool selection (MR-11, runtime half).
//!
//! `choice-select/v1` (`engine::choice`, `engine::choice_select`) decides one
//! finite choice over caller-supplied options. This module is the runtime's
//! side of that contract: it derives the candidate set from authority the host
//! already holds, and turns a typed selection back into an authorized dispatch.
//!
//! * [`granted_tool_options`] reads an Agent Runtime Profile v1 (for example
//!   `BoundAgentDeployment::runtime_v1_profile()`) and offers exactly the
//!   tools its policy allows, with the contract's effects, required
//!   capabilities and argument/result schema types. A tool the deployment does
//!   not grant is never a candidate.
//! * [`SpecialistRegistry`] is a configured specialist registry over an
//!   [`ApprovedProfileSet`]. It yields both the `agent` options and the
//!   [`SpecialistGrant`]s a [`super::RoutedSession`] dispatches through, so the
//!   option set and the dispatch allowlist come from one document.
//!
//! The core screens these options (kind, types, privacy, budget, effects,
//! capabilities) before any inference. A [`ChoiceSelection`] is advisory: the
//! authorize stage ([`authorize_tool_choice`], [`authorize_specialist_choice`])
//! re-derives the candidate set from the live deployment or registry, rechecks
//! the selection against it and, for a tool, validates the arguments against
//! the tool's closed argument schema. Only the resulting [`AuthorizedTool`] or
//! [`AuthorizedSpecialist`] (no public constructors) names something to run,
//! and the existing runtime boundary (Runtime v1 tool policy, session
//! specialist allowlist) checks it once more. Nothing here executes anything.
//!
//! Contract: the `choice-select/v1` section of `docs/HARNESS-DECISION-V1.md`
//! and `docs/RUNTIME-MODEL-ROUTING-V1.md`.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::error::RuntimeRoutingError;
use super::profiles::ApprovedProfileSet;
use super::session::SpecialistGrant;
use crate::model_routing::engine::choice::{description_ok, stable_id_ok};
use crate::model_routing::engine::{
    json, ChoiceInputs, ChoiceOption, ChoiceSelection, Confidentiality, DestinationKind,
};

/// Schema id of a configured specialist registry document.
pub const SPECIALIST_REGISTRY_SCHEMA: &str = "semaprax.runtime-specialist-registry.v1";
/// Bound on registry entries (the core sends at most 16 admitted options).
pub const MAX_SPECIALISTS: usize = 32;

const DISPATCH: &str = "SPX-HPJ026";

fn choice_error(code: &str, message: impl Into<String>) -> RuntimeRoutingError {
    RuntimeRoutingError::Choice {
        code: code.to_owned(),
        message: message.into(),
    }
}

fn config(message: impl Into<String>) -> RuntimeRoutingError {
    RuntimeRoutingError::InvalidConfig(message.into())
}

/// The stable type id of a closed tool schema: `schema/<16 hex>` over its
/// canonical JSON. Two tools have the same type exactly when their schemas
/// are identical, so the question's input/output type can be computed from
/// the schema the caller will actually produce or consume.
pub fn tool_schema_type(schema: &Value) -> String {
    let digest = json::digest("semaprax.runtime-tool-schema-type.v1", schema);
    format!("schema/{}", &digest["sha256:".len().."sha256:".len() + 16])
}

fn strings(v: &Value) -> Option<BTreeSet<String>> {
    v.as_array()?
        .iter()
        .map(|x| x.as_str().map(str::to_owned))
        .collect()
}

/// One tool contract of a runtime profile, as the choice task sees it.
struct ToolContract<'a> {
    id: &'a str,
    raw: &'a Map<String, Value>,
}

fn profile_tools(
    profile: &Value,
) -> Result<(Vec<ToolContract<'_>>, BTreeSet<String>), RuntimeRoutingError> {
    let tools = profile["tools"]
        .as_array()
        .ok_or_else(|| config("runtime profile has no `tools` array"))?;
    let allowed = strings(&profile["policy"]["allowed_tool_ids"])
        .ok_or_else(|| config("runtime profile has no `policy.allowed_tool_ids`"))?;
    let granted = strings(&profile["policy"]["granted_capabilities"])
        .ok_or_else(|| config("runtime profile has no `policy.granted_capabilities`"))?;
    let mut out = Vec::new();
    for t in tools {
        let raw = t
            .as_object()
            .ok_or_else(|| config("runtime profile tool is not an object"))?;
        let id = raw["tool_id"]
            .as_str()
            .ok_or_else(|| config("runtime profile tool has no `tool_id`"))?;
        if allowed.contains(id) {
            out.push(ToolContract { id, raw });
        }
    }
    Ok((out, granted))
}

fn tool_option(t: &ToolContract<'_>) -> ChoiceOption {
    let set = |k: &str| strings(&t.raw[k]).unwrap_or_default();
    ChoiceOption {
        id: t.id.to_owned(),
        kind: DestinationKind::Tool,
        description: t.raw["description"].as_str().unwrap_or("").to_owned(),
        input_type: tool_schema_type(&t.raw["arguments_schema"]),
        output_type: tool_schema_type(&t.raw["result_schema"]),
        // A registered tool runs behind the caller's own host boundary.
        destination: crate::model_routing::engine::Destination::Local,
        max_confidentiality: Confidentiality::Project,
        // Runtime v1 tool contracts declare no price: unknown, never zero.
        est_cost_micros: None,
        effects: set("effects"),
        requires: set("required_capabilities"),
    }
}

/// The tool options an Agent Runtime Profile v1 grants: one `tool` option per
/// tool contract whose id is in `policy.allowed_tool_ids`, in profile order.
/// The second value is the profile's granted capability set, for
/// `ChoiceQuestion::granted`. A malformed description or id is kept here and
/// rejected by the core's screen (`invalid_option`), never repaired.
pub fn granted_tool_options(
    runtime_profile: &str,
) -> Result<(Vec<ChoiceOption>, BTreeSet<String>), RuntimeRoutingError> {
    let profile: Value =
        serde_json::from_str(runtime_profile).map_err(|_| config("runtime profile is not JSON"))?;
    let (tools, granted) = profile_tools(&profile)?;
    Ok((tools.iter().map(tool_option).collect(), granted))
}

/// Validate `arguments` against a closed Runtime v1 argument schema
/// (`{type: object, fields: [{name, type, required, max_bytes}],
/// additional_properties: false}`) and return its canonical rendering.
fn closed_arguments(schema: &Value, arguments: &Value) -> Result<String, String> {
    let fields = schema["fields"]
        .as_array()
        .ok_or("the tool's argument schema has no fields")?;
    let object = arguments
        .as_object()
        .ok_or("arguments must be a JSON object")?;
    for key in object.keys() {
        if !fields.iter().any(|f| f["name"] == *key) {
            return Err(format!("argument `{key}` is not declared by the tool"));
        }
    }
    for f in fields {
        let name = f["name"].as_str().ok_or("schema field without a name")?;
        let max = f["max_bytes"].as_u64().unwrap_or(0);
        let Some(v) = object.get(name) else {
            if f["required"] == true {
                return Err(format!("required argument `{name}` is missing"));
            }
            continue;
        };
        let ok = match (f["type"].as_str(), v) {
            (Some("string"), Value::String(s)) => s.len() as u64 <= max,
            (Some("integer"), Value::Number(n)) => n.as_i64().is_some(),
            (Some("boolean"), Value::Bool(_)) => true,
            _ => false,
        };
        if !ok {
            return Err(format!(
                "argument `{name}` does not match its declared type or bound"
            ));
        }
    }
    Ok(json::canonical(arguments))
}

/// A tool selection the authorize stage admitted, with its checked
/// arguments. No public constructor: only [`authorize_tool_choice`] makes one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedTool {
    tool_id: String,
    arguments_json: String,
}

impl AuthorizedTool {
    pub fn tool_id(&self) -> &str {
        &self.tool_id
    }
    /// Canonical JSON of the checked arguments.
    pub fn arguments_json(&self) -> &str {
        &self.arguments_json
    }
}

/// The authorize stage for a tool selection. Before any effect it rechecks
/// that the selection is still a supplied, unchanged, admitted option of the
/// live inputs (`SPX-HPJ024`), that the live runtime profile still grants
/// exactly that tool with the same declaration, and that `arguments` satisfy
/// the tool's closed argument schema (`SPX-HPJ026`).
pub fn authorize_tool_choice(
    selection: &ChoiceSelection,
    live: &ChoiceInputs,
    runtime_profile: &str,
    arguments: &Value,
) -> Result<AuthorizedTool, RuntimeRoutingError> {
    let option = selection
        .recheck(live)
        .map_err(|d| choice_error(d.code, d.message))?;
    if option.kind != DestinationKind::Tool {
        return Err(choice_error(DISPATCH, "the selection is not a tool"));
    }
    let profile: Value =
        serde_json::from_str(runtime_profile).map_err(|_| config("runtime profile is not JSON"))?;
    let (tools, granted) = profile_tools(&profile)?;
    let contract = tools.iter().find(|t| t.id == option.id).ok_or_else(|| {
        choice_error(
            DISPATCH,
            format!("tool `{}` is not granted by the live deployment", option.id),
        )
    })?;
    if tool_option(contract) != *option || !option.requires.is_subset(&granted) {
        return Err(choice_error(
            DISPATCH,
            format!(
                "tool `{}` changed its contract or grant since the decision",
                option.id
            ),
        ));
    }
    let arguments_json = closed_arguments(&contract.raw["arguments_schema"], arguments)
        .map_err(|why| choice_error(DISPATCH, format!("tool `{}`: {why}", option.id)))?;
    Ok(AuthorizedTool {
        tool_id: option.id.clone(),
        arguments_json,
    })
}

/// One configured specialist: a stable choice id bound to one approved
/// profile, with the host description and types the choice task screens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecialistEntry {
    pub id: String,
    /// The approved profile this specialist runs as.
    pub profile: String,
    pub description: String,
    pub input_type: String,
    pub output_type: String,
    pub max_confidentiality: Confidentiality,
    pub effects: BTreeSet<String>,
    pub requires: BTreeSet<String>,
    /// Whether child-agent work may be delegated to it.
    pub delegable: bool,
}

/// A configured specialist registry: the `agent` candidate set and the
/// session's specialist allowlist, from one host document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecialistRegistry {
    entries: Vec<SpecialistEntry>,
}

fn closed<'a>(
    v: &'a Value,
    what: &str,
    keys: &[&str],
) -> Result<&'a Map<String, Value>, RuntimeRoutingError> {
    let m = v
        .as_object()
        .ok_or_else(|| config(format!("{what} must be an object")))?;
    if let Some(k) = m.keys().find(|k| !keys.contains(&k.as_str())) {
        return Err(config(format!("{what} has unknown member `{k}`")));
    }
    Ok(m)
}

impl SpecialistRegistry {
    /// Strict parse of a `semaprax.runtime-specialist-registry.v1` document:
    /// `{schema, specialists: [{id, profile, description, input_type,
    /// output_type, max_confidentiality, effects, requires, delegable}]}`.
    /// Ids, types, effects and capabilities must be stable ids and the
    /// description a bounded host description; nothing is repaired.
    pub fn parse(document: &str) -> Result<Self, RuntimeRoutingError> {
        let v: Value = serde_json::from_str(document)
            .map_err(|_| config("specialist registry is not JSON"))?;
        let top = closed(&v, "specialist registry", &["schema", "specialists"])?;
        if top.get("schema").and_then(Value::as_str) != Some(SPECIALIST_REGISTRY_SCHEMA) {
            return Err(config(format!(
                "specialist registry schema must be `{SPECIALIST_REGISTRY_SCHEMA}`"
            )));
        }
        let rows = top
            .get("specialists")
            .and_then(Value::as_array)
            .filter(|a| !a.is_empty() && a.len() <= MAX_SPECIALISTS)
            .ok_or_else(|| {
                config(format!(
                    "`specialists` must hold 1..={MAX_SPECIALISTS} entries"
                ))
            })?;
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for row in rows {
            let m = closed(
                row,
                "specialist",
                &[
                    "id",
                    "profile",
                    "description",
                    "input_type",
                    "output_type",
                    "max_confidentiality",
                    "effects",
                    "requires",
                    "delegable",
                ],
            )?;
            let s = |k: &str| {
                m.get(k)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| config(format!("specialist `{k}` must be a string")))
            };
            let ids = |k: &str| {
                strings(m.get(k).unwrap_or(&Value::Null))
                    .filter(|set| set.iter().all(|x| stable_id_ok(x)))
                    .ok_or_else(|| config(format!("specialist `{k}` must be stable ids")))
            };
            let entry = SpecialistEntry {
                id: s("id")?,
                profile: s("profile")?,
                description: s("description")?,
                input_type: s("input_type")?,
                output_type: s("output_type")?,
                max_confidentiality: Confidentiality::parse(&s("max_confidentiality")?)
                    .ok_or_else(|| config("specialist `max_confidentiality` is out of its set"))?,
                effects: ids("effects")?,
                requires: ids("requires")?,
                delegable: m
                    .get("delegable")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| config("specialist `delegable` must be a boolean"))?,
            };
            if ![&entry.id, &entry.input_type, &entry.output_type]
                .iter()
                .all(|x| stable_id_ok(x))
                || !description_ok(&entry.description)
            {
                return Err(config(format!(
                    "specialist `{}` needs stable ids and a bounded host description",
                    entry.id.chars().take(64).collect::<String>()
                )));
            }
            if !seen.insert(entry.id.clone()) {
                return Err(config(format!("specialist `{}` is duplicated", entry.id)));
            }
            entries.push(entry);
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[SpecialistEntry] {
        &self.entries
    }

    pub fn entry(&self, id: &str) -> Option<&SpecialistEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// The `agent` options over `set`. Every entry must name an approved
    /// profile (a registry that points outside the approved set is a
    /// configuration error, not a silently dropped option). Destination and
    /// declared cost come from the profile's own logical metadata.
    pub fn options(
        &self,
        set: &ApprovedProfileSet,
    ) -> Result<Vec<ChoiceOption>, RuntimeRoutingError> {
        self.entries
            .iter()
            .map(|e| {
                let p = set.profile(&e.profile).ok_or_else(|| {
                    config(format!(
                        "specialist `{}` names profile `{}`, which is not approved",
                        e.id, e.profile
                    ))
                })?;
                Ok(ChoiceOption {
                    id: e.id.clone(),
                    kind: DestinationKind::Agent,
                    description: e.description.clone(),
                    input_type: e.input_type.clone(),
                    output_type: e.output_type.clone(),
                    destination: p.model().destination.clone(),
                    max_confidentiality: e.max_confidentiality,
                    est_cost_micros: Some(p.model().est_cost_micros),
                    effects: e.effects.clone(),
                    requires: e.requires.clone(),
                })
            })
            .collect()
    }

    /// The session specialist allowlist this registry configures
    /// (`SessionPolicy::specialists`).
    pub fn grants(&self) -> Vec<SpecialistGrant> {
        self.entries
            .iter()
            .map(|e| SpecialistGrant {
                id: e.id.clone(),
                profile: e.profile.clone(),
                delegable: e.delegable,
            })
            .collect()
    }
}

/// A specialist selection the authorize stage admitted. No public
/// constructor: only [`authorize_specialist_choice`] makes one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorizedSpecialist {
    id: String,
    profile: String,
    deployment_digest: String,
}

impl AuthorizedSpecialist {
    /// The specialist id to dispatch (`TurnRequest::specialist`).
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn profile(&self) -> &str {
        &self.profile
    }
    /// The bound deployment the specialist runs as.
    pub fn deployment_digest(&self) -> &str {
        &self.deployment_digest
    }
}

/// The authorize stage for an agent selection: recheck against the live
/// inputs (`SPX-HPJ024`), then against the live registry and approved set
/// (`SPX-HPJ026`) before the turn is dispatched.
pub fn authorize_specialist_choice(
    selection: &ChoiceSelection,
    live: &ChoiceInputs,
    registry: &SpecialistRegistry,
    set: &ApprovedProfileSet,
) -> Result<AuthorizedSpecialist, RuntimeRoutingError> {
    let option = selection
        .recheck(live)
        .map_err(|d| choice_error(d.code, d.message))?;
    if option.kind != DestinationKind::Agent {
        return Err(choice_error(DISPATCH, "the selection is not an agent"));
    }
    let entry = registry.entry(&option.id).ok_or_else(|| {
        choice_error(
            DISPATCH,
            format!("specialist `{}` is not in the live registry", option.id),
        )
    })?;
    let current = registry.options(set)?;
    if !current.iter().any(|o| o == option) {
        return Err(choice_error(
            DISPATCH,
            format!(
                "specialist `{}` changed its declaration since the decision",
                option.id
            ),
        ));
    }
    let profile = set.profile(&entry.profile).ok_or_else(|| {
        choice_error(
            DISPATCH,
            format!("profile `{}` is not approved", entry.profile),
        )
    })?;
    Ok(AuthorizedSpecialist {
        id: entry.id.clone(),
        profile: entry.profile.clone(),
        deployment_digest: profile.deployment_digest().to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn profile(desc: &str) -> String {
        json!({
            "tools": [
                {"tool_id": "kb.search", "description": desc,
                 "arguments_schema": {"type": "object", "fields": [{"name": "query", "type": "string", "required": true, "max_bytes": 8}], "additional_properties": false},
                 "result_schema": {"type": "object", "fields": [], "additional_properties": false},
                 "effects": ["read"], "required_capabilities": ["tool.read"]},
                {"tool_id": "kb.hidden", "description": "not granted",
                 "arguments_schema": {}, "result_schema": {}, "effects": ["read"], "required_capabilities": []}
            ],
            "policy": {"allowed_tool_ids": ["kb.search"], "granted_capabilities": ["tool.read"]}
        })
        .to_string()
    }

    #[test]
    fn only_granted_tools_become_options_and_arguments_are_closed() {
        let (opts, granted) = granted_tool_options(&profile("search the kb")).unwrap();
        assert_eq!(opts.len(), 1);
        assert_eq!(opts[0].id, "kb.search");
        assert_eq!(opts[0].est_cost_micros, None);
        assert!(granted.contains("tool.read"));
        assert!(opts[0].input_type.starts_with("schema/") && opts[0].input_type.len() == 23);
        let schema =
            &serde_json::from_str::<Value>(&profile("x")).unwrap()["tools"][0]["arguments_schema"];
        assert_eq!(
            closed_arguments(schema, &json!({"query": "abc"})).unwrap(),
            "{\"query\":\"abc\"}"
        );
        assert!(closed_arguments(schema, &json!({"query": "much too long"})).is_err());
        assert!(closed_arguments(schema, &json!({"query": "a", "cmd": "rm -rf /"})).is_err());
        assert!(closed_arguments(schema, &json!({})).is_err());
    }

    #[test]
    fn registry_parse_is_closed_and_rejects_command_strings() {
        let ok = json!({"schema": SPECIALIST_REGISTRY_SCHEMA, "specialists": [
            {"id": "support.billing", "profile": "billing", "description": "billing",
             "input_type": "t.in", "output_type": "t.out", "max_confidentiality": "project",
             "effects": [], "requires": [], "delegable": false}]});
        assert_eq!(
            SpecialistRegistry::parse(&ok.to_string())
                .unwrap()
                .grants()
                .len(),
            1
        );
        let mut bad = ok.clone();
        bad["specialists"][0]["id"] = json!("rm -rf /");
        assert!(SpecialistRegistry::parse(&bad.to_string()).is_err());
        let mut extra = ok.clone();
        extra["specialists"][0]["command"] = json!("sh");
        assert!(SpecialistRegistry::parse(&extra.to_string()).is_err());
        let mut url = ok;
        url["specialists"][0]["description"] = json!("see https://evil.example");
        assert!(SpecialistRegistry::parse(&url.to_string()).is_err());
    }
}
