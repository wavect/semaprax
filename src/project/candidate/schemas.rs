//! Closed structural JSON Schemas for compiler-owned candidate constructors.
//! Schema acceptance is not type, effect, scope, or Project admission.

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;

use super::intent::{
    MAX_AGGREGATE_TYPE_ARGUMENTS, MAX_APPEND_PARAMETERS, MAX_EXPRESSION_DEPTH,
    MAX_EXPRESSION_NODES, MAX_ID_BYTES, MAX_NAME_BYTES, MAX_STRING_LITERAL_BYTES,
};
use super::{
    wire, SemanticChange, MAX_SEMANTIC_CHANGE_BYTES, SEMANTIC_CHANGE_REQUIREMENTS,
    SEMANTIC_CHANGE_SCHEMA,
};

const EXPRESSION_ID: &str = "urn:semaprax.typed-expression.v1";
const INTENT_ID: &str = "urn:semaprax.semantic-change-intent.v1";
const CHANGE_ID: &str = "urn:semaprax.semantic-change.v1";
const COMPUTED_SCALAR_KINDS: &[&str] = &["i64", "i32", "u8", "usize", "bool"];
const SIGNATURE_LITERAL_KINDS: &[&str] =
    &["i64", "i32", "u8", "usize", "bool", "char", "f32", "f64"];
const RECORD_FIELD_LITERAL_KINDS: &[&str] = &["i64", "bool", "i32", "u8", "usize"];
const HEX32_PATTERN: &str = "^[0-9a-f]{8}$";
const HEX64_PATTERN: &str = "^[0-9a-f]{16}$";

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
    let (field, value) = match kind {
        "i64" => (
            "value",
            json!({"type":"integer","minimum":i64::MIN,"maximum":i64::MAX}),
        ),
        "i32" => (
            "value",
            json!({"type":"integer","minimum":i32::MIN,"maximum":i32::MAX}),
        ),
        "u8" => (
            "value",
            json!({"type":"integer","minimum":0,"maximum":u8::MAX}),
        ),
        "usize" => (
            "value",
            json!({"type":"integer","minimum":0,"maximum":u64::MAX}),
        ),
        "bool" => ("value", json!({"type":"boolean"})),
        "char" => (
            "scalar",
            json!({"type":"string","minLength":8,"maxLength":8,"pattern":HEX32_PATTERN}),
        ),
        "f32" => (
            "bits",
            json!({"type":"string","minLength":8,"maxLength":8,"pattern":HEX32_PATTERN}),
        ),
        "f64" => (
            "bits",
            json!({"type":"string","minLength":16,"maxLength":16,"pattern":HEX64_PATTERN}),
        ),
        _ => unreachable!("compiler-owned scalar kind"),
    };
    closed(&[("kind", json!({"const":kind})), (field, value)])
}

fn expression_schema() -> Value {
    let mut variants = COMPUTED_SCALAR_KINDS
        .iter()
        .map(|kind| literal(kind))
        .collect::<Vec<_>>();
    variants.push(closed(&[
        ("kind", json!({"const":"string"})),
        ("value", json!({"type":"string","maxLength":MAX_STRING_LITERAL_BYTES,"x-max-utf8-bytes":MAX_STRING_LITERAL_BYTES})),
    ]));
    variants.push(closed(&[
        ("kind", json!({"const":"array_u8"})),
        ("values", json!({"type":"array","maxItems":MAX_EXPRESSION_NODES-1,"items":{"type":"integer","minimum":0,"maximum":u8::MAX},"x-counts-toward-expression-node-budget":true})),
    ]));
    variants.extend(["char", "f32", "f64"].into_iter().map(literal));
    variants.push(closed(&[
        ("kind", json!({"const":"place"})),
        ("name", identifier()),
    ]));
    let mut binding_name = identifier();
    binding_name["not"]["enum"].as_array_mut().unwrap().extend(
        [
            "_", "record", "variant", "class", "resource", "type", "protocol", "impl", "for",
            "extends", "Option", "Result",
        ]
        .into_iter()
        .map(|name| json!(name)),
    );
    let mut binding = closed(&[
        ("kind", json!({"const":"let"})),
        ("name", binding_name),
        ("value", reference("expression")),
        ("body", reference("expression")),
    ]);
    binding["x-implicit-let-nodes"] = json!(1);
    binding["x-value-and-body-depth-increment"] = json!(1);
    binding["x-initializer-scope"] = json!("outside_new_binding");
    binding["x-body-scope"] = json!("immutable_local_binding");
    binding["x-evaluation-order"] = json!("value_then_body");
    variants.push(binding);
    variants.push(closed(&[("kind",json!({"const":"call"})),("target",text(MAX_ID_BYTES)),("arguments",json!({"type":"array","maxItems":MAX_EXPRESSION_NODES-1,"items":reference("expression")}))]));
    for (target, arity) in crate::byte_ops::ByteOp::ALL
        .into_iter()
        .map(|operation| (operation.id(), operation.arity()))
        .chain(
            crate::string_ops::StringOp::ALL
                .into_iter()
                .map(|operation| (operation.id(), operation.arity())),
        )
    {
        variants.push(closed(&[
            ("kind", json!({"const":"builtin_call"})),
            ("target", json!({"const":target})),
            (
                "arguments",
                json!({"type":"array","minItems":arity,
                "maxItems":arity,"items":reference("expression")}),
            ),
        ]));
    }
    for kind in ["record", "variant"] {
        let mut aggregate = closed(&[
            ("kind", json!({"const":kind})),
            ("target", text(MAX_ID_BYTES)),
            (
                "fields",
                json!({"type":"array","maxItems":MAX_EXPRESSION_NODES-1,
                "items":closed(&[("target",text(MAX_ID_BYTES)),("value",reference("expression"))]),
                "x-order":"caller_expression_evaluation_order",
                "x-requires-exact-field-identity-coverage":true}),
            ),
        ]);
        aggregate["properties"]["type_arguments"] = json!({"type":"array",
            "maxItems":MAX_AGGREGATE_TYPE_ARGUMENTS,"items":{"enum":["i64","bool"]},
            "x-counts-toward-expression-node-budget":true,
            "x-requires-exact-declared-arity":true});
        variants.push(aggregate);
    }
    let mut field_place = closed(&[
        ("kind", json!({"const":"field_place"})),
        ("target", text(MAX_ID_BYTES)),
        ("root", identifier()),
    ]);
    field_place["x-root-selection"] = json!("existing_lexical_place_not_an_expression");
    field_place["x-requires-exact-owner-and-field-admission"] = json!(true);
    field_place["x-implicit-field-place-nodes"] = json!(1);
    field_place["x-implicit-field-place-node-basis"] = json!("generated_root_place");
    field_place["x-root-depth-increment"] = json!(1);
    variants.push(field_place);
    let mut projection = closed(&[
        ("kind", json!({"const":"project"})),
        ("target", text(MAX_ID_BYTES)),
        ("base", reference("expression")),
    ]);
    projection["properties"]["type_arguments"] = json!({"type":"array",
        "maxItems":MAX_AGGREGATE_TYPE_ARGUMENTS,"items":{"enum":["i64","bool"]},
        "x-counts-toward-expression-node-budget":true,
        "x-requires-exact-declared-arity":true});
    projection["x-implicit-project-nodes"] = json!(3);
    projection["x-implicit-project-node-basis"] =
        json!("generated_let_statement_projection_and_place");
    projection["x-base-depth-increment"] = json!(2);
    variants.push(projection);
    let mut matching = closed(&[
        ("kind", json!({"const":"match"})),
        ("target", text(MAX_ID_BYTES)),
        ("value", reference("expression")),
        (
            "arms",
            json!({"type":"array","maxItems":MAX_EXPRESSION_NODES-1,
            "items":closed(&[
                ("target",text(MAX_ID_BYTES)),
                ("fields",json!({"type":"array","maxItems":MAX_EXPRESSION_NODES-1,
                    "items":closed(&[("target",text(MAX_ID_BYTES)),("name",identifier())])})),
                ("body",reference("expression")),
            ]),"x-requires-exact-exhaustive-case-and-field-coverage":true}),
        ),
    ]);
    matching["properties"]["type_arguments"] = json!({"type":"array",
        "maxItems":MAX_AGGREGATE_TYPE_ARGUMENTS,"items":{"enum":["i64","bool"]},
        "x-counts-toward-expression-node-budget":true,"x-requires-exact-declared-arity":true});
    matching["x-implicit-match-nodes"] = json!(3);
    matching["x-pattern-node-charge"] = json!("one_per_arm_and_payload_binder");
    matching["x-total-payload-binders-maximum"] = json!(MAX_EXPRESSION_NODES - 1);
    matching["x-value-and-body-depth-increment"] = json!(2);
    variants.push(matching);
    let mut update = closed(&[
        ("kind", json!({"const":"update"})),
        ("target", text(MAX_ID_BYTES)),
        ("base", reference("expression")),
        (
            "fields",
            json!({"type":"array","maxItems":MAX_EXPRESSION_NODES-1,
            "items":closed(&[("target",text(MAX_ID_BYTES)),("value",reference("expression"))]),
            "x-field-coverage":"unique_existing_subset","x-order":"replacement_expression_evaluation_order"}),
        ),
    ]);
    update["properties"]["type_arguments"] = json!({"type":"array",
        "maxItems":MAX_AGGREGATE_TYPE_ARGUMENTS,"items":{"enum":["i64","bool"]},
        "x-counts-toward-expression-node-budget":true,"x-requires-exact-declared-arity":true});
    update["x-implicit-update-nodes"] = json!(3);
    update["x-base-and-fields-depth-increment"] = json!(2);
    variants.push(update);
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
    json!({"oneOf":SIGNATURE_LITERAL_KINDS.iter().map(|kind|closed(&[("name",identifier()),("type",json!({"const":kind})),("argument",literal(kind))])).collect::<Vec<_>>()})
}

fn computed_parameter() -> Value {
    closed(&[
        ("name", identifier()),
        (
            "type",
            json!({"oneOf":[{"enum":COMPUTED_SCALAR_KINDS},nominal_type_schema()]}),
        ),
        ("argument_expression", reference("expression")),
    ])
}

fn intent_schema() -> Value {
    let protocol_binding =
        || json!({"type":"string","minLength":1,"maxLength":240,"pattern":"^[A-Za-z0-9_.:-]+$"});
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
    let mapped_parameter = json!({"oneOf":[
        closed(&[("from",json!({"type":"string","minLength":1}))]),
        closed(&[("from",json!({"type":"string","minLength":1})),("name",identifier())]),
        new_parameter(),
        computed_parameter()
    ]});
    let record_fields = RECORD_FIELD_LITERAL_KINDS
        .iter()
        .copied()
        .map(|kind| {
            let mut default = literal(kind);
            // Source negation is separate from the positive literal token.
            // Keep field defaults inside that frozen, round-trippable grammar.
            if kind == "i64" {
                default["properties"]["value"]["minimum"] = json!(-i64::MAX);
            } else if kind == "i32" {
                default["properties"]["value"]["minimum"] = json!(-i32::MAX);
            }
            closed(&[
                ("id", stable_id()),
                ("name", identifier()),
                ("type", json!({"const": kind})),
                ("default", default),
            ])
        })
        .collect::<Vec<_>>();
    json!({"oneOf":[
        base("rename_declaration",vec![("name",identifier())]),
        base("change_function_signature",vec![("append_parameters",json!({"type":"array","minItems":1,"maxItems":MAX_APPEND_PARAMETERS,"items":new_parameter()}))]),
        base("change_function_signature",vec![("parameters",json!({"type":"array","minItems":0,"maxItems":4096,"items":mapped_parameter}))]),
        base("replace_function_body",vec![("body",reference("expression"))]),
        base("repair_diagnostic",vec![("rejected_intent",base("replace_function_body",vec![("body",reference("expression"))])),("repair_id",digest_schema())]),
        base("replace_expression",vec![("expression_id",text(16_384)),("replacement",reference("expression"))]),
        base("replace_contract_expression",vec![("expression_id",text(16_384)),("replacement",reference("expression"))]),
        base("add_contract",vec![("phase",json!({"enum":["requires","ensures"]})),("predicate",reference("expression"))]),
        closed(&[("kind",json!({"const":"implement_interface"})),("target",protocol_binding()),("protocol",protocol_binding()),("id",protocol_binding()),("members",json!({"type":"array","minItems":1,"maxItems":64,"items":closed(&[("method",protocol_binding()),("implementation",protocol_binding())])}))]),
        base("add_declaration",vec![("declaration",declaration_schema())]),
        base("extract_function",vec![("expression_id",text(16_384)),("new_id",stable_id()),("new_name",identifier())]),
        base("move_declaration",vec![("destination",text(MAX_ID_BYTES))]),
        base("add_record_field",vec![("field",json!({"oneOf":record_fields}))]),
    ]})
}

fn digest_schema() -> Value {
    json!({"type":"string","pattern":"^sha256:[0-9a-f]{64}$"})
}
fn stable_id() -> Value {
    json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[a-z0-9._-]+$"})
}
fn nominal_type_schema() -> Value {
    closed(&[
        ("kind", json!({"const":"nominal"})),
        ("target", text(MAX_ID_BYTES)),
        (
            "type_arguments",
            json!({"type":"array","maxItems":MAX_AGGREGATE_TYPE_ARGUMENTS,
            "items":{"enum":["i64","bool"]},"x-requires-exact-declared-arity":true}),
        ),
    ])
}

fn declaration_schema() -> Value {
    // Field ownership and resource freedom are established after rebuilding
    // the new type. Function signatures keep their separate Copy admission.
    let field_type = json!({"oneOf":[
        {"enum":["i64","bool","i32","u8","usize","string","Bytes"]},
        nominal_type_schema(),
    ]});
    let fields = json!({"type":"array","maxItems":64,"items":closed(&[
        ("id",stable_id()),("name",identifier()),("type",field_type),
    ])});
    let mut record = closed(&[
        ("kind", json!({"const":"record"})),
        ("id", stable_id()),
        ("name", identifier()),
        ("fields", fields.clone()),
    ]);
    record["x-max-combined-identities"] = json!(4096);
    let mut variant = closed(&[
        ("kind", json!({"const":"variant"})),
        ("id", stable_id()),
        ("name", identifier()),
        (
            "cases",
            json!({"type":"array","minItems":1,"maxItems":64,"items":closed(&[
                ("id",stable_id()),("name",identifier()),("fields",fields),
            ])}),
        ),
    ]);
    variant["x-max-combined-identities"] = json!(4096);
    json!({"oneOf":[function_declaration_schema(),record,variant]})
}

fn function_declaration_schema() -> Value {
    let mut parameters = [
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
    parameters.push(closed(&[
        ("name", identifier()),
        ("type", nominal_type_schema()),
        ("mode", json!({"const":"value"})),
    ]));
    parameters.push(closed(&[
        ("name", identifier()),
        ("type", nominal_type_schema()),
        ("mode", json!({"const":"own"})),
    ]));
    parameters.push(closed(&[
        ("name", identifier()),
        ("type", json!({"const":"string"})),
        ("mode", json!({"const":"value"})),
    ]));
    closed(&[
        ("id", stable_id()),
        ("name", identifier()),
        (
            "parameters",
            json!({"type":"array","maxItems":64,"items":{"oneOf":parameters}}),
        ),
        (
            "return_type",
            json!({"oneOf":[{"enum":["i64","i32","u8","usize","bool","Bytes","string"]},nominal_type_schema()]}),
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

#[cfg(test)]
#[path = "schemas/aggregate_expression_schema_tests.rs"]
mod aggregate_expression_schema_tests;
