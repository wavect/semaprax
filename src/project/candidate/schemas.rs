//! Closed structural JSON Schemas for compiler-owned candidate constructors.
//! Schema acceptance is not type, effect, scope, or Project admission.

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;

use super::intent::{
    MAX_APPEND_PARAMETERS, MAX_EXPRESSION_DEPTH, MAX_EXPRESSION_NODES, MAX_ID_BYTES, MAX_NAME_BYTES,
};
use super::{
    wire, SemanticChange, MAX_SEMANTIC_CHANGE_BYTES, SEMANTIC_CHANGE_REQUIREMENTS,
    SEMANTIC_CHANGE_SCHEMA,
};

const EXPRESSION_ID: &str = "urn:semaprax.typed-expression.v1";
const INTENT_ID: &str = "urn:semaprax.semantic-change-intent.v1";
const CHANGE_ID: &str = "urn:semaprax.semantic-change.v1";
const SCALAR_KINDS: &[&str] = &["i64", "i32", "u8", "usize", "bool"];

impl SemanticChange {
    /// Self-contained structural constructor documents. Each document closes
    /// its object shapes and resolves recursion through local `$defs` only.
    /// Existing compiler constructors remain authoritative for semantic and
    /// lexical admission; this method changes no accepted source or errors.
    pub fn constructor_schemas() -> Result<String, Vec<Diagnostic>> {
        let expression = expression_schema();
        let intent = intent_schema();
        let definitions = json!({"expression":expression,"intent":intent});
        let expression_document = document(EXPRESSION_ID, "expression", definitions.clone());
        let intent_document = document(INTENT_ID, "intent", definitions.clone());
        let mut change_document = closed(&[
            ("schema", json!({"const":SEMANTIC_CHANGE_SCHEMA})),
            (
                "base_revision",
                json!({"type":"string","pattern":"^sha256:[0-9a-f]{64}$"}),
            ),
            ("intent", reference("intent")),
            (
                "requirements",
                json!({"const":SEMANTIC_CHANGE_REQUIREMENTS}),
            ),
        ]);
        change_document["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        change_document["$id"] = json!(CHANGE_ID);
        change_document["$defs"] = definitions.clone();
        let mut recovery_document = closed(&[
            (
                "schema",
                json!({"const":super::PROJECT_CANDIDATE_RECOVERY_SCHEMA}),
            ),
            ("compiler", json!({"const":super::recovery::compiler()})),
            ("base_revision", digest_schema()),
            ("change_schema", json!({"const":SEMANTIC_CHANGE_SCHEMA})),
            (
                "candidate_schema",
                json!({"const":super::PROJECT_CANDIDATE_SCHEMA}),
            ),
            (
                "changes",
                json!({"type":"array","maxItems":super::MAX_CHANGES,"items":reference("change")}),
            ),
            ("candidate_digest", digest_schema()),
            ("candidate_project_revision", digest_schema()),
            ("capsule_digest", digest_schema()),
        ]);
        let mut change_definition = change_document.clone();
        for key in ["$schema", "$id", "$defs"] {
            change_definition.as_object_mut().unwrap().remove(key);
        }
        let mut recovery_definitions = definitions;
        recovery_definitions["change"] = change_definition;
        recovery_document["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
        recovery_document["$id"] = json!("urn:semaprax.project-candidate-recovery.v1");
        recovery_document["$defs"] = recovery_definitions;
        wire::render(
            json!({
                "schema":"semaprax.candidate-constructor-schemas.v1",
                "documents":[expression_document,intent_document,change_document,recovery_document],
                "admission":"closed_structural_grammar_only",
                "requires_compiler_admission":true,
                "limits":{"max_change_bytes":MAX_SEMANTIC_CHANGE_BYTES,"max_json_value_nodes":8192,"max_json_value_depth":64,"max_expression_nodes":MAX_EXPRESSION_NODES,"max_expression_depth":MAX_EXPRESSION_DEPTH},
                "nonclaims":["not_type_scope_effect_ownership_or_target_admission","not_canonical_json_or_duplicate_key_validation","not_complete_semantic_response_schemas","no_source_or_commit_authority"],
            }),
            256 * 1024,
        )
    }
}

fn document(id: &str, root: &str, definitions: Value) -> Value {
    json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$id":id,"$ref":format!("#/$defs/{root}"),"$defs":definitions})
}

fn closed(fields: &[(&str, Value)]) -> Value {
    let properties = fields
        .iter()
        .map(|(name, schema)| ((*name).to_owned(), schema.clone()))
        .collect::<Map<_, _>>();
    json!({"type":"object","additionalProperties":false,"required":fields.iter().map(|(name,_)| *name).collect::<Vec<_>>(),"properties":properties})
}

fn reference(name: &str) -> Value {
    json!({"$ref":format!("#/$defs/{name}")})
}
fn text(max: usize) -> Value {
    json!({"type":"string","minLength":1,"maxLength":max,"x-max-utf8-bytes":max})
}

fn identifier() -> Value {
    json!({"type":"string","minLength":1,"maxLength":MAX_NAME_BYTES,"pattern":"^[A-Za-z_][A-Za-z0-9_]*$","not":{"enum":["module","use","fn","let","mut","if","else","while","match","true","false","requires","ensures","uses","permit","unsafe","return","own","borrow","shared","self","super"]}})
}

fn literal(kind: &str) -> Value {
    let value = match kind {
        "i64" => json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX}),
        "i32" => json!({"type":"integer","minimum":i32::MIN,"maximum":i32::MAX}),
        "u8" => json!({"type":"integer","minimum":0,"maximum":u8::MAX}),
        "usize" => json!({"type":"integer","minimum":0,"maximum":u64::MAX}),
        "bool" => json!({"type":"boolean"}),
        _ => unreachable!("compiler-owned scalar kind"),
    };
    closed(&[("kind", json!({"const":kind})), ("value", value)])
}

fn expression_schema() -> Value {
    let mut variants = SCALAR_KINDS
        .iter()
        .map(|kind| literal(kind))
        .collect::<Vec<_>>();
    variants.push(closed(&[
        ("kind", json!({"const":"place"})),
        ("name", identifier()),
    ]));
    variants.push(closed(&[("kind",json!({"const":"call"})),("target",text(MAX_ID_BYTES)),("arguments",json!({"type":"array","maxItems":MAX_EXPRESSION_NODES-1,"items":reference("expression")}))]));
    variants.push(closed(&[
        ("kind", json!({"const":"binary"})),
        (
            "op",
            json!({"enum":["+","-","*","/","%","==","!=","<","<=",">",">=","&&","||"]}),
        ),
        ("left", reference("expression")),
        ("right", reference("expression")),
    ]));
    variants.push(closed(&[
        ("kind", json!({"const":"unary"})),
        ("op", json!({"enum":["-","!"]})),
        ("value", reference("expression")),
    ]));
    variants.push(closed(&[
        ("kind", json!({"const":"if"})),
        ("condition", reference("expression")),
        ("then", reference("expression")),
        ("else", reference("expression")),
    ]));
    json!({"oneOf":variants,"x-max-expression-nodes":MAX_EXPRESSION_NODES,"x-max-expression-depth":MAX_EXPRESSION_DEPTH,"x-implicit-if-block-nodes":2})
}

fn new_parameter() -> Value {
    json!({"oneOf":SCALAR_KINDS.iter().map(|kind|closed(&[("name",identifier()),("type",json!({"const":kind})),("argument",literal(kind))])).collect::<Vec<_>>()})
}

fn intent_schema() -> Value {
    let base = |kind: &str, extra: Vec<(&str, Value)>| {
        let mut fields = vec![
            ("kind", json!({"const":kind})),
            ("target", text(MAX_ID_BYTES)),
        ];
        fields.extend(extra);
        closed(&fields)
    };
    // Existing parameter names come from the checked source; the ordinary
    // 128-byte constructor identifier restriction applies only to new names.
    let mapped_parameter = json!({"oneOf":[closed(&[("from",json!({"type":"string","minLength":1}))]),new_parameter()]});
    json!({"oneOf":[
        base("rename_declaration",vec![("name",identifier())]),
        base("change_function_signature",vec![("append_parameters",json!({"type":"array","minItems":1,"maxItems":MAX_APPEND_PARAMETERS,"items":new_parameter()}))]),
        base("change_function_signature",vec![("parameters",json!({"type":"array","minItems":0,"maxItems":4096,"items":mapped_parameter}))]),
        base("replace_function_body",vec![("body",reference("expression"))]),
        base("replace_expression",vec![("expression_id",text(16_384)),("replacement",reference("expression"))]),
        base("add_contract",vec![("phase",json!({"enum":["requires","ensures"]})),("predicate",reference("expression"))]),
        base("add_declaration",vec![("declaration",declaration_schema())]),
        base("extract_function",vec![("expression_id",text(16_384)),("new_id",stable_id()),("new_name",identifier())]),
    ]})
}

fn digest_schema() -> Value {
    json!({"type":"string","pattern":"^sha256:[0-9a-f]{64}$"})
}
fn stable_id() -> Value {
    json!({"type":"string","minLength":1,"maxLength":MAX_ID_BYTES,"pattern":"^[a-z0-9._-]+$"})
}
fn declaration_schema() -> Value {
    let parameters = [
        ("value", vec!["i64", "i32", "u8", "usize", "bool"]),
        ("own", vec!["Bytes"]),
        ("borrow", vec!["str", "Slice<u8>"]),
    ]
    .into_iter()
    .map(|(mode, types)| {
        closed(&[
            ("name", identifier()),
            ("type", json!({"enum":types})),
            ("mode", json!({"const":mode})),
        ])
    })
    .collect::<Vec<_>>();
    closed(&[
        ("id", stable_id()),
        ("name", identifier()),
        (
            "parameters",
            json!({"type":"array","maxItems":64,"items":{"oneOf":parameters}}),
        ),
        (
            "return_type",
            json!({"enum":["i64","i32","u8","usize","bool","Bytes"]}),
        ),
        (
            "effects",
            json!({"type":"array","maxItems":64,"uniqueItems":true,"items":text(128),"x-sorted":true}),
        ),
        (
            "requires",
            json!({"type":"array","maxItems":64,"items":reference("expression")}),
        ),
        (
            "ensures",
            json!({"type":"array","maxItems":64,"items":reference("expression")}),
        ),
        ("body", reference("expression")),
    ])
}
