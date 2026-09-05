//! Deterministic, authority-free clients derived from Proposal Schema v1.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::{quote_json, Diagnostic};

use super::shape::{CaseRow, FieldRow, Representation, Shape};
use super::{CompiledAgentProposalSchema, PROPOSAL_SCHEMA, SCHEMA_V1};

pub const AGENT_PROPOSAL_CLIENT_BUNDLE_SCHEMA: &str = "semaprax.agent-proposal-client-bundle.v1";
pub const MAX_AGENT_PROPOSAL_CLIENT_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_AGENT_PROPOSAL_CLIENT_BUNDLE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_AGENT_PROPOSAL_CLIENT_MANIFEST_BYTES: usize = 64 * 1024;

const BUNDLE_DOMAIN: &[u8] = b"semaprax.agent-proposal-client-bundle.digest.v1\0";
const SOURCE_DOMAIN: &[u8] = b"semaprax.agent-proposal-client.source.digest.v1\0";
const ARTIFACTS: [(&str, &str); 4] = [
    ("structured_output_json_schema", "proposal.schema.json"),
    ("typescript", "proposal.ts"),
    ("python", "proposal.py"),
    ("rust", "proposal.rs"),
];
const NONCLAIMS: [&str; 6] = [
    "additive_projection_bound_to_frozen_agent_proposal_schema_v1",
    "generated_sources_grant_no_provider_tool_filesystem_network_or_process_authority",
    "generated_encoders_create_no_authorized_value_or_publication_token",
    "structured_output_schema_is_provider_neutral",
    "structured_output_schema_does_not_replace_exact_decoder_range_validation",
    "no_generated_source_compilation_execution_packaging_or_publication_claim",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentProposalClientBundle {
    bundle_digest: String,
    manifest_json: String,
    structured_output_schema: String,
    typescript_source: String,
    python_source: String,
    rust_source: String,
}

impl CompiledAgentProposalSchema {
    pub fn generate_clients(&self) -> Result<AgentProposalClientBundle> {
        AgentProposalClientBundle::derive(self)
    }
}

impl AgentProposalClientBundle {
    pub fn derive(compiled: &CompiledAgentProposalSchema) -> Result<Self> {
        let structured_output_schema = structured_schema(compiled);
        let typescript_source = typescript(compiled);
        let python_source = python(compiled);
        let rust_source = rust(compiled);
        let sources = [
            structured_output_schema.as_str(),
            typescript_source.as_str(),
            python_source.as_str(),
            rust_source.as_str(),
        ];
        let total = sources.iter().try_fold(0usize, |total, source| {
            if source.len() > MAX_AGENT_PROPOSAL_CLIENT_SOURCE_BYTES {
                return Err(invalid("generated proposal client exceeds its byte limit"));
            }
            total
                .checked_add(source.len())
                .ok_or_else(|| invalid("generated proposal client byte count overflowed"))
        })?;
        if total > MAX_AGENT_PROPOSAL_CLIENT_BUNDLE_BYTES {
            return Err(invalid(
                "generated proposal client bundle exceeds its byte limit",
            ));
        }
        let artifacts = ARTIFACTS
            .iter()
            .zip(sources)
            .map(|((kind, path), source)| {
                json!({
                    "bytes": source.len(),
                    "digest": source_digest(kind, source.as_bytes()),
                    "kind": kind,
                    "path": path,
                })
            })
            .collect::<Vec<_>>();
        let payload = json!({
            "artifacts": artifacts,
            "limits": {
                "max_bundle_bytes": MAX_AGENT_PROPOSAL_CLIENT_BUNDLE_BYTES,
                "max_manifest_bytes": MAX_AGENT_PROPOSAL_CLIENT_MANIFEST_BYTES,
                "max_source_bytes": MAX_AGENT_PROPOSAL_CLIENT_SOURCE_BYTES,
            },
            "nonclaims": NONCLAIMS,
            "proposal_schema_digest": compiled.schema().digest(),
            "proposal_schema_identity": SCHEMA_V1,
            "proposal_type_revision": compiled.schema().proposal_type_revision(),
            "schema": AGENT_PROPOSAL_CLIENT_BUNDLE_SCHEMA,
        });
        let bundle_digest = digest(BUNDLE_DOMAIN, canonical(payload.clone())?.as_bytes());
        let manifest_json = canonical(with_field(payload, "bundle_digest", json!(bundle_digest)))?;
        if manifest_json.len() > MAX_AGENT_PROPOSAL_CLIENT_MANIFEST_BYTES {
            return Err(invalid("generated proposal client manifest is too large"));
        }
        Ok(Self {
            bundle_digest,
            manifest_json,
            structured_output_schema,
            typescript_source,
            python_source,
            rust_source,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        compiled: &CompiledAgentProposalSchema,
        expected_bundle_digest: &str,
        manifest_bytes: &[u8],
        structured_output_schema_bytes: &[u8],
        typescript_bytes: &[u8],
        python_bytes: &[u8],
        rust_bytes: &[u8],
    ) -> Result<Self> {
        validate_digest(expected_bundle_digest)?;
        let sources = [
            structured_output_schema_bytes,
            typescript_bytes,
            python_bytes,
            rust_bytes,
        ];
        submitted_bounds(manifest_bytes, &sources)?;
        let manifest = std::str::from_utf8(manifest_bytes)
            .map_err(|_| invalid("proposal client manifest is not UTF-8"))?;
        let value: Value = serde_json::from_str(manifest)
            .map_err(|_| invalid("proposal client manifest is not JSON"))?;
        if canonical(value.clone())?.as_bytes() != manifest_bytes {
            return Err(invalid(
                "proposal client manifest is not exact canonical JSON",
            ));
        }
        validate_manifest(&value)?;
        let derived = Self::derive(compiled)?;
        if expected_bundle_digest != derived.bundle_digest()
            || manifest_bytes != derived.manifest_json().as_bytes()
            || sources
                != [
                    derived.structured_output_schema().as_bytes(),
                    derived.typescript_source().as_bytes(),
                    derived.python_source().as_bytes(),
                    derived.rust_source().as_bytes(),
                ]
        {
            return Err(stale(
                "proposal client bundle failed exact schema-bound replay",
            ));
        }
        Ok(derived)
    }

    pub fn bundle_digest(&self) -> &str {
        &self.bundle_digest
    }
    pub fn manifest_json(&self) -> &str {
        &self.manifest_json
    }
    pub fn structured_output_schema(&self) -> &str {
        &self.structured_output_schema
    }
    pub fn typescript_source(&self) -> &str {
        &self.typescript_source
    }
    pub fn python_source(&self) -> &str {
        &self.python_source
    }
    pub fn rust_source(&self) -> &str {
        &self.rust_source
    }
}

#[allow(clippy::too_many_arguments)]
pub fn verify_agent_proposal_client_bundle(
    compiled: &CompiledAgentProposalSchema,
    expected_bundle_digest: &str,
    manifest_bytes: &[u8],
    structured_output_schema_bytes: &[u8],
    typescript_bytes: &[u8],
    python_bytes: &[u8],
    rust_bytes: &[u8],
) -> Result<()> {
    AgentProposalClientBundle::replay(
        compiled,
        expected_bundle_digest,
        manifest_bytes,
        structured_output_schema_bytes,
        typescript_bytes,
        python_bytes,
        rust_bytes,
    )?;
    Ok(())
}

fn structured_schema(compiled: &CompiledAgentProposalSchema) -> String {
    let value = match &compiled.shape {
        Shape::Record { fields } => json!({
            "additionalProperties": false,
            "properties": {"fields": fields_schema(fields)},
            "required": ["fields"],
            "type": "object",
        }),
        Shape::Variant { cases } => json!({
            "oneOf": cases.iter().map(case_schema).collect::<Vec<_>>(),
        }),
    };
    canonical(json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "additionalProperties": false,
        "properties": {
            "agent_id": {"const": compiled.schema().agent_id()},
            "proposal_schema_digest": {"const": compiled.schema().digest()},
            "schema": {"const": PROPOSAL_SCHEMA},
            "value": value,
        },
        "required": ["schema", "agent_id", "proposal_schema_digest", "value"],
        "type": "object",
    }))
    .expect("generated schema is bounded")
}

fn fields_schema(fields: &[FieldRow]) -> Value {
    let properties = fields
        .iter()
        .map(|field| (field.stable_id.clone(), scalar_schema(field.representation)))
        .collect::<Map<_, _>>();
    json!({
        "additionalProperties": false,
        "properties": properties,
        "required": fields.iter().map(|field| field.stable_id.as_str()).collect::<Vec<_>>(),
        "type": "object",
    })
}

fn case_schema(case: &CaseRow) -> Value {
    json!({
        "additionalProperties": false,
        "properties": {
            "case": {"const": case.stable_id},
            "fields": fields_schema(&case.fields),
        },
        "required": ["case", "fields"],
        "type": "object",
    })
}

fn scalar_schema(representation: Representation) -> Value {
    match representation {
        Representation::Bool => json!({"type": "boolean"}),
        Representation::Text => json!({"maxLength": 4096, "type": "string"}),
        Representation::I32 | Representation::I64 => {
            json!({"pattern": "^(0|-[1-9][0-9]*|[1-9][0-9]*)$", "type": "string"})
        }
        Representation::U8 | Representation::U64 => {
            json!({"pattern": "^(0|[1-9][0-9]*)$", "type": "string"})
        }
    }
}

fn typescript(compiled: &CompiledAgentProposalSchema) -> String {
    let mut output = header("TypeScript", compiled);
    output.push_str("export const AGENT_ID = ");
    output.push_str(&quote_json(compiled.schema().agent_id()));
    output.push_str(" as const;\nexport const PROPOSAL_SCHEMA_DIGEST = ");
    output.push_str(&quote_json(compiled.schema().digest()));
    output.push_str(" as const;\nexport type ExactInteger = bigint;\n");
    match &compiled.shape {
        Shape::Record { fields } => ts_record(&mut output, fields),
        Shape::Variant { cases } => ts_variant(&mut output, cases),
    }
    output.push_str("\nfunction integer(value: bigint, low: bigint, high: bigint): string { if (value < low || value > high) throw new RangeError(\"proposal integer\"); return value.toString(10); }\nfunction text(value: string): string { if (new TextEncoder().encode(value).length > 4096) throw new RangeError(\"proposal string\"); return value; }\nfunction envelope(value: object): string { return JSON.stringify({schema: \"semaprax.agent-proposal.v1\", agent_id: AGENT_ID, proposal_schema_digest: PROPOSAL_SCHEMA_DIGEST, value}) + \"\\n\"; }\n");
    output
}

fn ts_record(output: &mut String, fields: &[FieldRow]) {
    ts_fields_type(output, "ProposalFields", fields);
    output.push_str("export function encodeProposal(fields: ProposalFields): string { return envelope({fields: {");
    ts_fields(output, fields, "fields");
    output.push_str("}}); }\n");
}

fn ts_variant(output: &mut String, cases: &[CaseRow]) {
    for (index, case) in cases.iter().enumerate() {
        ts_fields_type(output, &format!("Case{index}Fields"), &case.fields);
    }
    output.push_str("export type ProposalValue =\n");
    for (index, case) in cases.iter().enumerate() {
        output.push_str("  | {case: ");
        output.push_str(&quote_json(&case.stable_id));
        output.push_str(&format!("; fields: Case{index}Fields}}\n"));
    }
    output.push_str(
        ";\nexport function encodeProposal(value: ProposalValue): string { switch (value.case) {\n",
    );
    for case in cases {
        output.push_str("case ");
        output.push_str(&quote_json(&case.stable_id));
        output.push_str(": return envelope({case: value.case, fields: {");
        ts_fields(output, &case.fields, "value.fields");
        output.push_str("}});\n");
    }
    output.push_str("} throw new Error(\"proposal case\"); }\n");
}

fn ts_fields_type(output: &mut String, name: &str, fields: &[FieldRow]) {
    output.push_str("export type ");
    output.push_str(name);
    output.push_str(" = {");
    for field in fields {
        output.push_str("readonly ");
        output.push_str(&quote_json(&field.stable_id));
        output.push(':');
        output.push_str(match field.representation {
            Representation::Bool => "boolean;",
            Representation::Text => "string;",
            _ => "ExactInteger;",
        });
    }
    output.push_str("};\n");
}

fn ts_fields(output: &mut String, fields: &[FieldRow], base: &str) {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(&field.stable_id));
        output.push(':');
        let access = format!("{base}[{}]", quote_json(&field.stable_id));
        output.push_str(&wire_call(field.representation, &access, "typescript"));
    }
}

fn python(compiled: &CompiledAgentProposalSchema) -> String {
    let mut output = header("Python", compiled);
    output.push_str("import json\nfrom typing import Literal, TypedDict, Union\nAGENT_ID = ");
    output.push_str(&quote_json(compiled.schema().agent_id()));
    output.push_str("\nPROPOSAL_SCHEMA_DIGEST = ");
    output.push_str(&quote_json(compiled.schema().digest()));
    output.push_str("\ndef _integer(value: int, low: int, high: int) -> str:\n    if isinstance(value, bool) or value < low or value > high: raise ValueError(\"proposal integer\")\n    return str(value)\ndef _text(value: str) -> str:\n    if len(value.encode(\"utf-8\")) > 4096: raise ValueError(\"proposal string\")\n    return value\ndef _envelope(value: object) -> str:\n    return json.dumps({\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":AGENT_ID,\"proposal_schema_digest\":PROPOSAL_SCHEMA_DIGEST,\"value\":value}, ensure_ascii=False, separators=(\",\",\":\")) + \"\\n\"\n");
    match &compiled.shape {
        Shape::Record { fields } => py_record(&mut output, fields),
        Shape::Variant { cases } => py_variant(&mut output, cases),
    }
    output
}

fn py_record(output: &mut String, fields: &[FieldRow]) {
    py_fields_type(output, "ProposalFields", fields);
    output.push_str(
        "def encode_proposal(fields: ProposalFields) -> str:\n    return _envelope({\"fields\":{",
    );
    py_fields(output, fields, "fields");
    output.push_str("}})\n");
}

fn py_variant(output: &mut String, cases: &[CaseRow]) {
    for (index, case) in cases.iter().enumerate() {
        py_fields_type(output, &format!("Case{index}Fields"), &case.fields);
        output.push_str(&format!("Case{index} = TypedDict(\"Case{index}\", {{\"case\":Literal[{}],\"fields\":Case{index}Fields}})\n", quote_json(&case.stable_id)));
    }
    output.push_str("ProposalValue = Union[");
    for index in 0..cases.len() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!("Case{index}"));
    }
    output.push_str(
        "]\ndef encode_proposal(value: ProposalValue) -> str:\n    case = value[\"case\"]\n",
    );
    for (index, case) in cases.iter().enumerate() {
        output.push_str(if index == 0 { "    if " } else { "    elif " });
        output.push_str("case == ");
        output.push_str(&quote_json(&case.stable_id));
        output.push_str(": fields = {");
        py_fields(output, &case.fields, "value[\"fields\"]");
        output.push_str("}\n");
    }
    output.push_str("    else: raise ValueError(\"proposal case\")\n    return _envelope({\"case\":case,\"fields\":fields})\n");
}

fn py_fields_type(output: &mut String, name: &str, fields: &[FieldRow]) {
    output.push_str(name);
    output.push_str(" = TypedDict(");
    output.push_str(&quote_json(name));
    output.push_str(",{");
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(&field.stable_id));
        output.push(':');
        output.push_str(match field.representation {
            Representation::Bool => "bool",
            Representation::Text => "str",
            _ => "int",
        });
    }
    output.push_str("})\n");
}

fn py_fields(output: &mut String, fields: &[FieldRow], base: &str) {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&quote_json(&field.stable_id));
        output.push(':');
        let access = format!("{base}[{}]", quote_json(&field.stable_id));
        output.push_str(&wire_call(field.representation, &access, "python"));
    }
}

fn rust(compiled: &CompiledAgentProposalSchema) -> String {
    let mut output = header("Rust", compiled);
    output.push_str("pub const AGENT_ID: &str = ");
    output.push_str(&quote_json(compiled.schema().agent_id()));
    output.push_str(";\npub const PROPOSAL_SCHEMA_DIGEST: &str = ");
    output.push_str(&quote_json(compiled.schema().digest()));
    output.push_str(";\nfn quoted(value: &str) -> Result<String,&'static str> { serde_json::to_string(value).map_err(|_| \"proposal string\") }\n");
    match &compiled.shape {
        Shape::Record { fields } => rust_record(&mut output, fields),
        Shape::Variant { cases } => rust_variant(&mut output, cases),
    }
    output
}

fn rust_record(output: &mut String, fields: &[FieldRow]) {
    rust_struct(output, "ProposalFields", fields);
    output.push_str("pub fn encode_proposal(value: &ProposalFields) -> Result<String,&'static str> { let mut fields=String::from(\"{\");");
    rust_fields(output, fields, "value");
    output.push_str("fields.push('}'); Ok(format!(\"{{\\\"schema\\\":\\\"semaprax.agent-proposal.v1\\\",\\\"agent_id\\\":{},\\\"proposal_schema_digest\\\":{},\\\"value\\\":{{\\\"fields\\\":{}}}}}\\n\",quoted(AGENT_ID)?,quoted(PROPOSAL_SCHEMA_DIGEST)?,fields))}\n");
}

fn rust_variant(output: &mut String, cases: &[CaseRow]) {
    for (index, case) in cases.iter().enumerate() {
        rust_struct(output, &format!("Case{index}Fields"), &case.fields);
    }
    output.push_str("pub enum ProposalValue {");
    for index in 0..cases.len() {
        output.push_str(&format!("Case{index}(Case{index}Fields),"));
    }
    output.push_str("}\npub fn encode_proposal(value: &ProposalValue) -> Result<String,&'static str> { let (case,fields)=match value {");
    for (index, case) in cases.iter().enumerate() {
        output.push_str(&format!(
            "ProposalValue::Case{index}(value)=>{{let mut fields=String::from(\"{{\");"
        ));
        rust_fields(output, &case.fields, "value");
        output.push_str("fields.push('}');(");
        output.push_str(&quote_json(&case.stable_id));
        output.push_str(",fields)},");
    }
    output.push_str("}; Ok(format!(\"{{\\\"schema\\\":\\\"semaprax.agent-proposal.v1\\\",\\\"agent_id\\\":{},\\\"proposal_schema_digest\\\":{},\\\"value\\\":{{\\\"case\\\":{},\\\"fields\\\":{}}}}}\\n\",quoted(AGENT_ID)?,quoted(PROPOSAL_SCHEMA_DIGEST)?,quoted(case)?,fields))}\n");
}

fn rust_struct(output: &mut String, name: &str, fields: &[FieldRow]) {
    output.push_str("pub struct ");
    output.push_str(name);
    output.push('{');
    for (index, field) in fields.iter().enumerate() {
        output.push_str("/// Stable ID: ");
        output.push_str(&field.stable_id);
        output.push_str(&format!(
            "\npub field_{index}: {},",
            rust_type(field.representation)
        ));
    }
    output.push_str("}\n");
}

fn rust_fields(output: &mut String, fields: &[FieldRow], base: &str) {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            output.push_str("fields.push(',');");
        }
        output.push_str("fields.push_str(");
        output.push_str(&quote_json(&format!("{}:", quote_json(&field.stable_id))));
        output.push_str(");fields.push_str(&");
        let access = format!("{base}.field_{index}");
        match field.representation {
            Representation::Bool => output.push_str(&format!("{access}.to_string()")),
            Representation::Text => output.push_str(&format!("{{if {access}.as_bytes().len()>4096{{return Err(\"proposal string\")}}quoted(&{access})?}}")),
            _ => output.push_str(&format!("quoted(&{access}.to_string())?")),
        }
        output.push_str(");");
    }
}

fn rust_type(representation: Representation) -> &'static str {
    match representation {
        Representation::Bool => "bool",
        Representation::I32 => "i32",
        Representation::I64 => "i64",
        Representation::U8 => "u8",
        Representation::U64 => "u64",
        Representation::Text => "String",
    }
}

fn wire_call(representation: Representation, access: &str, language: &str) -> String {
    match representation {
        Representation::Bool => access.to_owned(),
        Representation::Text if language == "typescript" => format!("text({access})"),
        Representation::Text => format!("_text({access})"),
        integer => {
            let (low, high) = integer.bounds().expect("integer bounds");
            if language == "typescript" {
                format!("integer({access},{low}n,{high}n)")
            } else {
                format!("_integer({access},{low},{high})")
            }
        }
    }
}

fn header(language: &str, compiled: &CompiledAgentProposalSchema) -> String {
    let comment = if language == "Python" { "#" } else { "//" };
    format!(
        "{comment} Generated {language} Agent Proposal client.\n{comment} Proposal schema digest: {}\n{comment} Authority-free: validates and encodes data only.\n",
        compiled.schema().digest()
    )
}

fn submitted_bounds(manifest: &[u8], sources: &[&[u8]]) -> Result<()> {
    if manifest.len() > MAX_AGENT_PROPOSAL_CLIENT_MANIFEST_BYTES
        || sources
            .iter()
            .any(|source| source.len() > MAX_AGENT_PROPOSAL_CLIENT_SOURCE_BYTES)
    {
        return Err(invalid("submitted proposal client exceeds a byte limit"));
    }
    let total = sources.iter().try_fold(0usize, |total, source| {
        total
            .checked_add(source.len())
            .ok_or_else(|| invalid("submitted proposal client byte count overflowed"))
    })?;
    if total > MAX_AGENT_PROPOSAL_CLIENT_BUNDLE_BYTES {
        return Err(invalid(
            "submitted proposal client bundle exceeds its byte limit",
        ));
    }
    Ok(())
}

fn validate_manifest(value: &Value) -> Result<()> {
    let object = exact_object(value)?;
    exact_fields(
        object,
        &[
            "artifacts",
            "bundle_digest",
            "limits",
            "nonclaims",
            "proposal_schema_digest",
            "proposal_schema_identity",
            "proposal_type_revision",
            "schema",
        ],
    )?;
    if value["schema"] != AGENT_PROPOSAL_CLIENT_BUNDLE_SCHEMA
        || value["proposal_schema_identity"] != SCHEMA_V1
        || value["limits"]
            != json!({
                "max_bundle_bytes": MAX_AGENT_PROPOSAL_CLIENT_BUNDLE_BYTES,
                "max_manifest_bytes": MAX_AGENT_PROPOSAL_CLIENT_MANIFEST_BYTES,
                "max_source_bytes": MAX_AGENT_PROPOSAL_CLIENT_SOURCE_BYTES,
            })
        || value["nonclaims"] != json!(NONCLAIMS)
    {
        return Err(invalid("proposal client manifest fixed fields are invalid"));
    }
    for field in [
        "bundle_digest",
        "proposal_schema_digest",
        "proposal_type_revision",
    ] {
        validate_digest(
            object[field]
                .as_str()
                .ok_or_else(|| invalid("proposal client manifest digest is invalid"))?,
        )?;
    }
    let artifacts = value["artifacts"]
        .as_array()
        .ok_or_else(|| invalid("proposal client artifact inventory is invalid"))?;
    if artifacts.len() != ARTIFACTS.len() {
        return Err(invalid("proposal client artifact inventory is invalid"));
    }
    for (artifact, (kind, path)) in artifacts.iter().zip(ARTIFACTS) {
        let row = exact_object(artifact)?;
        exact_fields(row, &["bytes", "digest", "kind", "path"])?;
        if artifact["kind"] != kind
            || artifact["path"] != path
            || artifact["bytes"].as_u64().is_none()
        {
            return Err(invalid("proposal client artifact descriptor is invalid"));
        }
        validate_digest(
            row["digest"]
                .as_str()
                .ok_or_else(|| invalid("proposal client artifact digest is invalid"))?,
        )?;
    }
    let identity = digest(
        BUNDLE_DOMAIN,
        canonical(without_field(value, "bundle_digest")?)?.as_bytes(),
    );
    if value["bundle_digest"] != identity {
        return Err(invalid(
            "proposal client bundle digest does not authenticate its manifest",
        ));
    }
    Ok(())
}

fn canonical(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut source = serde_json::to_string(&value)
        .map_err(|_| invalid("proposal client JSON cannot be rendered"))?;
    source.push('\n');
    Ok(source)
}

fn exact_object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| invalid("proposal client manifest object is invalid"))
}

fn exact_fields(object: &Map<String, Value>, fields: &[&str]) -> Result<()> {
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid(
            "proposal client manifest has unknown or missing fields",
        ));
    }
    Ok(())
}

fn with_field(value: Value, key: &str, field: Value) -> Value {
    let mut object = value
        .as_object()
        .expect("bundle payload is an object")
        .clone();
    object.insert(key.to_owned(), field);
    Value::Object(object)
}

fn without_field(value: &Value, key: &str) -> Result<Value> {
    let mut object = exact_object(value)?.clone();
    if object.remove(key).is_none() {
        return Err(invalid("proposal client bundle identity is missing"));
    }
    Ok(Value::Object(object))
}

fn source_digest(kind: &str, bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(SOURCE_DOMAIN);
    hash.update((kind.len() as u64).to_le_bytes());
    hash.update(kind.as_bytes());
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid("proposal client digest is invalid"));
    }
    Ok(())
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G560", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G561", message)]
}
