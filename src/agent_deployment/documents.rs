//! Canonical AgentDefinition v2 and AgentDeployment v1 documents.
//!
//! Both are compact UTF-8 JSON with exactly one terminal LF, closed objects,
//! and canonical key order. Neither document has a field that can hold a
//! credential, a secret, a token, or an environment reference: the schemas are
//! closed, so such a key is rejected as noncanonical rather than ignored.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::agent_definition::{render_models, render_plain_object, render_tools, LIMIT_KEYS};
use crate::diagnostic::{quote_json, Diagnostic};

use super::{
    definition_invariant, definition_malformed, deployment_invariant, deployment_malformed,
    DEFINITION_V2_SCHEMA, DEPLOYMENT_SCHEMA, MAX_DOCUMENT_BYTES, MAX_JSON_DEPTH, OPERATION_ROLES,
    TYPE_ROLES,
};

pub(crate) const LOCALITIES: [&str; 2] = ["local_only", "remote_allowed"];
pub(crate) const QUALITY_TIERS: [&str; 4] = ["basic", "standard", "advanced", "frontier"];

const REQUIREMENT_KEYS: [&str; 6] = [
    "required_locality",
    "minimum_quality_tier",
    "required_model_capabilities",
    "required_capabilities",
    "allowed_tool_ids",
    "required_target_features",
];

/// Source-owned agent semantics: identities, tool contracts, requirements and
/// budget ceilings. No provider, model, credential or target implementation.
#[derive(Clone)]
pub(crate) struct DefinitionV2 {
    pub(crate) agent_id: String,
    pub(crate) types: Vec<String>,
    pub(crate) operations: Vec<String>,
    pub(crate) tools: Value,
    pub(crate) required_locality: String,
    pub(crate) minimum_quality_tier: String,
    pub(crate) required_model_capabilities: Vec<String>,
    pub(crate) required_capabilities: Vec<String>,
    pub(crate) allowed_tool_ids: Vec<String>,
    pub(crate) required_target_features: Vec<String>,
    pub(crate) ceilings: Value,
}

/// Deployment-owned selection: concrete providers and models, the granted
/// subset of the source's capabilities and tools, available target features,
/// and effective limits at or below the source ceilings.
#[derive(Clone)]
pub(crate) struct Deployment {
    pub(crate) deployment_id: String,
    pub(crate) definition_digest: String,
    pub(crate) models: Vec<DeploymentModel>,
    pub(crate) allowed_provider_ids: Vec<String>,
    pub(crate) allowed_model_ids: Vec<String>,
    pub(crate) granted_capabilities: Vec<String>,
    pub(crate) allowed_tool_ids: Vec<String>,
    pub(crate) target_features: Vec<String>,
    pub(crate) limits: Value,
}

/// One deployment model field as admitted by the closed-key, canonical-byte
/// boundary. The document schema closes the key set but not every field type:
/// a wrongly typed value stays verbatim so binding still reports the exact
/// diagnostic, and at the exact point, that it did before typing.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ModelField<T> {
    Typed(T),
    Untyped(Value),
}

impl<T> ModelField<T> {
    pub(crate) fn typed(&self) -> Option<&T> {
        match self {
            Self::Typed(value) => Some(value),
            Self::Untyped(_) => None,
        }
    }
}

impl ModelField<String> {
    fn text(value: &Value) -> Self {
        value.as_str().map_or_else(
            || Self::Untyped(value.clone()),
            |text| Self::Typed(text.to_owned()),
        )
    }

    fn to_value(&self) -> Value {
        match self {
            Self::Typed(text) => Value::String(text.clone()),
            Self::Untyped(value) => value.clone(),
        }
    }
}

impl ModelField<u64> {
    fn count(value: &Value) -> Self {
        value
            .as_u64()
            .map_or_else(|| Self::Untyped(value.clone()), Self::Typed)
    }

    fn to_value(&self) -> Value {
        match self {
            Self::Typed(count) => Value::from(*count),
            Self::Untyped(value) => value.clone(),
        }
    }
}

impl ModelField<Vec<ModelField<String>>> {
    fn texts(value: &Value) -> Self {
        value.as_array().map_or_else(
            || Self::Untyped(value.clone()),
            |rows| Self::Typed(rows.iter().map(ModelField::<String>::text).collect()),
        )
    }

    fn to_value(&self) -> Value {
        match self {
            Self::Typed(rows) => {
                Value::Array(rows.iter().map(ModelField::<String>::to_value).collect())
            }
            Self::Untyped(value) => value.clone(),
        }
    }
}

/// The single private owner of one admitted deployment model row: every
/// supported field, in canonical key order, decoded once at admission.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DeploymentModel {
    pub(crate) provider_id: ModelField<String>,
    pub(crate) model_id: ModelField<String>,
    pub(crate) locality: ModelField<String>,
    pub(crate) quality_tier: ModelField<String>,
    pub(crate) tokenizer_id: ModelField<String>,
    pub(crate) max_context_tokens: ModelField<u64>,
    pub(crate) input_usd_microunits_per_million_tokens: ModelField<u64>,
    pub(crate) output_usd_microunits_per_million_tokens: ModelField<u64>,
    pub(crate) capabilities: ModelField<Vec<ModelField<String>>>,
}

/// A borrowed, read-only view of one fully typed admitted model row. It
/// copies no string and is never constructed from caller-supplied text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModelSelectionRef<'a> {
    provider_id: &'a str,
    model_id: &'a str,
    capabilities: &'a [ModelField<String>],
    max_context_tokens: u64,
}

impl<'a> ModelSelectionRef<'a> {
    pub(crate) fn provider_id(&self) -> &'a str {
        self.provider_id
    }

    pub(crate) fn model_id(&self) -> &'a str {
        self.model_id
    }

    pub(crate) fn capabilities(&self) -> impl Iterator<Item = &'a str> + 'a {
        self.capabilities
            .iter()
            .filter_map(|capability| capability.typed().map(String::as_str))
    }

    pub(crate) fn max_context_tokens(&self) -> u64 {
        self.max_context_tokens
    }
}

impl DeploymentModel {
    /// Decodes one row whose closed key set and order `render_models` has
    /// already admitted.
    fn decode(row: &Value) -> Result<Self, Diagnostic> {
        #[cfg(test)]
        model_decode_counter::record();
        let field = |key: &str| row.get(key).ok_or_else(deployment_malformed);
        Ok(Self {
            provider_id: ModelField::text(field("provider_id")?),
            model_id: ModelField::text(field("model_id")?),
            locality: ModelField::text(field("locality")?),
            quality_tier: ModelField::text(field("quality_tier")?),
            tokenizer_id: ModelField::text(field("tokenizer_id")?),
            max_context_tokens: ModelField::count(field("max_context_tokens")?),
            input_usd_microunits_per_million_tokens: ModelField::count(field(
                "input_usd_microunits_per_million_tokens",
            )?),
            output_usd_microunits_per_million_tokens: ModelField::count(field(
                "output_usd_microunits_per_million_tokens",
            )?),
            capabilities: ModelField::texts(field("capabilities")?),
        })
    }

    /// The row's position in `QUALITY_TIERS`, or `None` when its tier is not
    /// an admitted tier name.
    pub(crate) fn quality_rank(&self) -> Option<usize> {
        let tier = self.quality_tier.typed()?;
        QUALITY_TIERS
            .iter()
            .position(|candidate| *candidate == tier.as_str())
    }

    /// The borrowed selection view, present when every selection field is
    /// typed. A bound deployment's rows always are: Runtime v1 admitted them.
    pub(crate) fn selection(&self) -> Option<ModelSelectionRef<'_>> {
        let capabilities = self.capabilities.typed()?;
        if capabilities.iter().any(|value| value.typed().is_none()) {
            return None;
        }
        Some(ModelSelectionRef {
            provider_id: self.provider_id.typed()?,
            model_id: self.model_id.typed()?,
            capabilities,
            max_context_tokens: *self.max_context_tokens.typed()?,
        })
    }

    fn to_value(&self) -> Value {
        let mut row = Map::new();
        for (key, value) in [
            ("provider_id", self.provider_id.to_value()),
            ("model_id", self.model_id.to_value()),
            ("locality", self.locality.to_value()),
            ("quality_tier", self.quality_tier.to_value()),
            ("tokenizer_id", self.tokenizer_id.to_value()),
            ("max_context_tokens", self.max_context_tokens.to_value()),
            (
                "input_usd_microunits_per_million_tokens",
                self.input_usd_microunits_per_million_tokens.to_value(),
            ),
            (
                "output_usd_microunits_per_million_tokens",
                self.output_usd_microunits_per_million_tokens.to_value(),
            ),
            ("capabilities", self.capabilities.to_value()),
        ] {
            row.insert(key.to_owned(), value);
        }
        Value::Object(row)
    }
}

/// Admits a model array through the owning closed-key renderer, then decodes
/// each row exactly once into its typed owner. Order is preserved.
pub(crate) fn admit_models(value: &Value) -> Result<Vec<DeploymentModel>, Diagnostic> {
    render_models(value)?;
    value
        .as_array()
        .ok_or_else(deployment_malformed)?
        .iter()
        .map(DeploymentModel::decode)
        .collect()
}

/// The wire projection of admitted model rows, consumed by the owning
/// renderer and by the Runtime v1 compatibility projection.
pub(crate) fn models_value(models: &[DeploymentModel]) -> Value {
    Value::Array(models.iter().map(DeploymentModel::to_value).collect())
}

pub(crate) fn parse_definition_v2(source: &str) -> Result<DefinitionV2, Diagnostic> {
    let top = canonical_top(source, definition_malformed)?;
    if !exact_keys(
        &top,
        &[
            "schema",
            "agent_id",
            "types",
            "operations",
            "tools",
            "requirements",
            "ceilings",
        ],
    ) || string(&top, "schema", definition_malformed)? != DEFINITION_V2_SCHEMA
    {
        return Err(definition_malformed());
    }
    let agent_id = string(&top, "agent_id", definition_malformed)?.to_owned();
    if !canonical_identifier(&agent_id) {
        return Err(definition_invariant("agent_id"));
    }
    let types = role_ids(&top, "types", &TYPE_ROLES, None)?;
    let operations = role_ids(
        &top,
        "operations",
        &OPERATION_ROLES.map(|(role, _)| role),
        Some(&OPERATION_ROLES.map(|(_, kind)| kind)),
    )?;
    let mut semantic_ids = BTreeSet::from([agent_id.clone()]);
    if types
        .iter()
        .chain(operations.iter())
        .any(|stable_id| !semantic_ids.insert(stable_id.clone()))
    {
        return Err(definition_invariant("semantic_ids"));
    }

    let tools = top.get("tools").cloned().ok_or_else(definition_malformed)?;
    render_tools(&tools).map_err(|_| definition_malformed())?;
    let declared_tool_ids = tool_ids(&tools)?;

    let requirements = object(&top, "requirements", definition_malformed)?;
    if !exact_keys(&requirements, &REQUIREMENT_KEYS) {
        return Err(definition_malformed());
    }
    let required_locality = string(&requirements, "required_locality", definition_malformed)?;
    if !LOCALITIES.contains(&required_locality) {
        return Err(definition_invariant("requirements.required_locality"));
    }
    let minimum_quality_tier = string(&requirements, "minimum_quality_tier", definition_malformed)?;
    if !QUALITY_TIERS.contains(&minimum_quality_tier) {
        return Err(definition_invariant("requirements.minimum_quality_tier"));
    }
    let required_model_capabilities = identifier_list(
        &requirements,
        "required_model_capabilities",
        definition_invariant,
    )?;
    let required_capabilities =
        identifier_list(&requirements, "required_capabilities", definition_invariant)?;
    let allowed_tool_ids =
        identifier_list(&requirements, "allowed_tool_ids", definition_invariant)?;
    let required_target_features = identifier_list(
        &requirements,
        "required_target_features",
        definition_invariant,
    )?;
    if allowed_tool_ids
        .iter()
        .any(|tool_id| !declared_tool_ids.contains(tool_id))
    {
        return Err(definition_invariant("requirements.allowed_tool_ids"));
    }
    for capabilities in tool_capabilities(&tools)? {
        if capabilities
            .iter()
            .any(|capability| !required_capabilities.contains(capability))
        {
            return Err(definition_invariant("tools.required_capabilities"));
        }
    }

    let ceilings = top
        .get("ceilings")
        .cloned()
        .ok_or_else(definition_malformed)?;
    unsigned_limits(&ceilings).ok_or_else(|| definition_invariant("ceilings"))?;
    render_plain_object(&ceilings, &LIMIT_KEYS).map_err(|_| definition_malformed())?;

    let definition = DefinitionV2 {
        agent_id,
        types,
        operations,
        tools,
        required_locality: required_locality.to_owned(),
        minimum_quality_tier: minimum_quality_tier.to_owned(),
        required_model_capabilities,
        required_capabilities,
        allowed_tool_ids,
        required_target_features,
        ceilings,
    };
    if render_definition_v2(&definition) != source {
        return Err(definition_malformed());
    }
    Ok(definition)
}

pub(crate) fn render_definition_v2(definition: &DefinitionV2) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"agent_id\":{},\"types\":[",
        quote_json(DEFINITION_V2_SCHEMA),
        quote_json(&definition.agent_id)
    );
    for (index, (stable_id, role)) in definition.types.iter().zip(TYPE_ROLES).enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{}}}",
            quote_json(role),
            quote_json(stable_id)
        ));
    }
    output.push_str("],\"operations\":[");
    for (index, (stable_id, (role, kind))) in definition
        .operations
        .iter()
        .zip(OPERATION_ROLES)
        .enumerate()
    {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"stable_id\":{},\"kind\":{}}}",
            quote_json(role),
            quote_json(stable_id),
            quote_json(kind)
        ));
    }
    output.push_str("],\"tools\":");
    output.push_str(
        &render_tools(&definition.tools).expect("admitted v2 tool rows remain canonical"),
    );
    output.push_str(",\"requirements\":{\"required_locality\":");
    output.push_str(&quote_json(&definition.required_locality));
    output.push_str(",\"minimum_quality_tier\":");
    output.push_str(&quote_json(&definition.minimum_quality_tier));
    output.push_str(",\"required_model_capabilities\":");
    output.push_str(&render_list(&definition.required_model_capabilities));
    output.push_str(",\"required_capabilities\":");
    output.push_str(&render_list(&definition.required_capabilities));
    output.push_str(",\"allowed_tool_ids\":");
    output.push_str(&render_list(&definition.allowed_tool_ids));
    output.push_str(",\"required_target_features\":");
    output.push_str(&render_list(&definition.required_target_features));
    output.push_str("},\"ceilings\":");
    output.push_str(
        &render_plain_object(&definition.ceilings, &LIMIT_KEYS)
            .expect("admitted v2 ceilings remain canonical"),
    );
    output.push_str("}\n");
    output
}

pub(crate) fn parse_deployment(source: &str) -> Result<Deployment, Diagnostic> {
    let top = canonical_top(source, deployment_malformed)?;
    if !exact_keys(
        &top,
        &[
            "schema",
            "deployment_id",
            "definition_digest",
            "models",
            "selection",
            "grants",
            "limits",
        ],
    ) || string(&top, "schema", deployment_malformed)? != DEPLOYMENT_SCHEMA
    {
        return Err(deployment_malformed());
    }
    let deployment_id = string(&top, "deployment_id", deployment_malformed)?.to_owned();
    if !canonical_identifier(&deployment_id) {
        return Err(deployment_invariant("deployment_id"));
    }
    let definition_digest = string(&top, "definition_digest", deployment_malformed)?.to_owned();
    if !canonical_digest(&definition_digest) {
        return Err(deployment_invariant("definition_digest"));
    }

    let models = admit_models(top.get("models").ok_or_else(deployment_malformed)?)
        .map_err(|_| deployment_malformed())?;
    if models.is_empty() {
        return Err(deployment_invariant("models"));
    }

    let selection = object(&top, "selection", deployment_malformed)?;
    if !exact_keys(&selection, &["allowed_provider_ids", "allowed_model_ids"]) {
        return Err(deployment_malformed());
    }
    let allowed_provider_ids =
        identifier_list(&selection, "allowed_provider_ids", deployment_invariant)?;
    let allowed_model_ids = identifier_list(&selection, "allowed_model_ids", deployment_invariant)?;
    if allowed_provider_ids.is_empty() || allowed_model_ids.is_empty() {
        return Err(deployment_invariant("selection"));
    }

    let grants = object(&top, "grants", deployment_malformed)?;
    if !exact_keys(
        &grants,
        &[
            "granted_capabilities",
            "allowed_tool_ids",
            "target_features",
        ],
    ) {
        return Err(deployment_malformed());
    }
    let granted_capabilities =
        identifier_list(&grants, "granted_capabilities", deployment_invariant)?;
    let allowed_tool_ids = identifier_list(&grants, "allowed_tool_ids", deployment_invariant)?;
    let target_features = identifier_list(&grants, "target_features", deployment_invariant)?;

    let limits = top
        .get("limits")
        .cloned()
        .ok_or_else(deployment_malformed)?;
    unsigned_limits(&limits).ok_or_else(|| deployment_invariant("limits"))?;
    render_plain_object(&limits, &LIMIT_KEYS).map_err(|_| deployment_malformed())?;

    let deployment = Deployment {
        deployment_id,
        definition_digest,
        models,
        allowed_provider_ids,
        allowed_model_ids,
        granted_capabilities,
        allowed_tool_ids,
        target_features,
        limits,
    };
    if render_deployment(&deployment) != source {
        return Err(deployment_malformed());
    }
    Ok(deployment)
}

pub(crate) fn render_deployment(deployment: &Deployment) -> String {
    let mut output = format!(
        "{{\"schema\":{},\"deployment_id\":{},\"definition_digest\":{},\"models\":",
        quote_json(DEPLOYMENT_SCHEMA),
        quote_json(&deployment.deployment_id),
        quote_json(&deployment.definition_digest)
    );
    output.push_str(
        &render_models(&models_value(&deployment.models))
            .expect("admitted deployment models remain canonical"),
    );
    output.push_str(",\"selection\":{\"allowed_provider_ids\":");
    output.push_str(&render_list(&deployment.allowed_provider_ids));
    output.push_str(",\"allowed_model_ids\":");
    output.push_str(&render_list(&deployment.allowed_model_ids));
    output.push_str("},\"grants\":{\"granted_capabilities\":");
    output.push_str(&render_list(&deployment.granted_capabilities));
    output.push_str(",\"allowed_tool_ids\":");
    output.push_str(&render_list(&deployment.allowed_tool_ids));
    output.push_str(",\"target_features\":");
    output.push_str(&render_list(&deployment.target_features));
    output.push_str("},\"limits\":");
    output.push_str(
        &render_plain_object(&deployment.limits, &LIMIT_KEYS)
            .expect("admitted deployment limits remain canonical"),
    );
    output.push_str("}\n");
    output
}

/// Returns the declared `tool_id` of every admitted tool row.
pub(crate) fn tool_ids(tools: &Value) -> Result<Vec<String>, Diagnostic> {
    tools
        .as_array()
        .ok_or_else(definition_malformed)?
        .iter()
        .map(|row| {
            row.get("tool_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(definition_malformed)
        })
        .collect()
}

/// Returns the required capabilities of every admitted tool row, in order.
pub(crate) fn tool_capabilities(tools: &Value) -> Result<Vec<Vec<String>>, Diagnostic> {
    tools
        .as_array()
        .ok_or_else(definition_malformed)?
        .iter()
        .map(|row| {
            row.get("required_capabilities")
                .and_then(Value::as_array)
                .ok_or_else(definition_malformed)?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(definition_malformed)
                })
                .collect()
        })
        .collect()
}

/// Returns the exact unsigned value of each limit key in canonical order.
pub(crate) fn unsigned_limits(value: &Value) -> Option<Vec<u64>> {
    let object = value.as_object()?;
    if object.len() != LIMIT_KEYS.len() {
        return None;
    }
    LIMIT_KEYS
        .iter()
        .map(|key| object.get(*key).and_then(Value::as_u64))
        .collect()
}

pub(crate) fn render_list(values: &[String]) -> String {
    let mut output = String::from("[");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(value));
    }
    output.push(']');
    output
}

pub(crate) fn canonical_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 240
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn canonical_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        && value.to_ascii_lowercase() == value
}

fn canonical_top(
    source: &str,
    malformed: fn() -> Diagnostic,
) -> Result<Map<String, Value>, Diagnostic> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(malformed());
    }
    let Some(body) = source.strip_suffix('\n') else {
        return Err(malformed());
    };
    if body.is_empty() || body.contains('\n') || body.contains('\r') || body.starts_with('\u{feff}')
    {
        return Err(malformed());
    }
    let value: Value = serde_json::from_str(body).map_err(|_| malformed())?;
    if json_depth(&value) > MAX_JSON_DEPTH {
        return Err(malformed());
    }
    value.as_object().cloned().ok_or_else(malformed)
}

fn role_ids(
    top: &Map<String, Value>,
    key: &str,
    roles: &[&str],
    kinds: Option<&[&str]>,
) -> Result<Vec<String>, Diagnostic> {
    let values = top
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(definition_malformed)?;
    if values.len() != roles.len() {
        return Err(definition_invariant(key));
    }
    let mut ids = Vec::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        let row = value.as_object().ok_or_else(definition_malformed)?;
        let expected: &[&str] = match kinds {
            Some(_) => &["role", "stable_id", "kind"],
            None => &["role", "stable_id"],
        };
        if !exact_keys(row, expected) || string(row, "role", definition_malformed)? != roles[index]
        {
            return Err(definition_invariant(&format!("{key}.roles")));
        }
        if let Some(kinds) = kinds {
            if string(row, "kind", definition_malformed)? != kinds[index] {
                return Err(definition_invariant(&format!("{key}.roles")));
            }
        }
        let stable_id = string(row, "stable_id", definition_malformed)?.to_owned();
        if !canonical_identifier(&stable_id) {
            return Err(definition_invariant(&format!("{key}.stable_ids")));
        }
        ids.push(stable_id);
    }
    Ok(ids)
}

fn identifier_list(
    object: &Map<String, Value>,
    key: &str,
    invariant: fn(&str) -> Diagnostic,
) -> Result<Vec<String>, Diagnostic> {
    let values = object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| invariant(key))?;
    let mut list = Vec::with_capacity(values.len());
    for value in values {
        let text = value.as_str().ok_or_else(|| invariant(key))?;
        if !canonical_identifier(text) {
            return Err(invariant(key));
        }
        list.push(text.to_owned());
    }
    if list.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(invariant(key));
    }
    Ok(list)
}

fn object(
    top: &Map<String, Value>,
    key: &str,
    malformed: fn() -> Diagnostic,
) -> Result<Map<String, Value>, Diagnostic> {
    top.get(key)
        .and_then(Value::as_object)
        .cloned()
        .ok_or_else(malformed)
}

fn string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    malformed: fn() -> Diagnostic,
) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(malformed)
}

fn exact_keys(object: &Map<String, Value>, keys: &[&str]) -> bool {
    object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
}

fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => 1 + values.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(entries) => 1 + entries.values().map(json_depth).max().unwrap_or(0),
        _ => 1,
    }
}

#[cfg(test)]
pub(crate) mod model_decode_counter {
    use std::cell::Cell;

    thread_local! {
        static DECODES: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        DECODES.with(|count| count.set(count.get() + 1));
    }

    /// Model rows decoded from dynamic JSON on this thread.
    pub(crate) fn snapshot() -> usize {
        DECODES.with(Cell::get)
    }
}
