//! Closed structural JSON Schemas for compiler-owned candidate constructors.
//! Schema acceptance is not type, effect, scope, or Project admission.

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;

use super::intent::{
    MAX_AGGREGATE_TYPE_ARGUMENTS, MAX_APPEND_PARAMETERS, MAX_EXPRESSION_DEPTH,
    MAX_EXPRESSION_NODES, MAX_ID_BYTES, MAX_NAME_BYTES,
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
    json!({"oneOf":SCALAR_KINDS.iter().map(|kind|closed(&[("name",identifier()),("type",json!({"const":kind})),("argument",literal(kind))])).collect::<Vec<_>>()})
}

fn computed_parameter() -> Value {
    closed(&[
        ("name", identifier()),
        ("type", json!({"enum":SCALAR_KINDS})),
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
    let record_fields = ["i64", "bool"]
        .into_iter()
        .map(|kind| {
            closed(&[
                ("id", stable_id()),
                ("name", identifier()),
                ("type", json!({"const": kind})),
                ("default", literal(kind)),
            ])
        })
        .collect::<Vec<_>>();
    json!({"oneOf":[
        base("rename_declaration",vec![("name",identifier())]),
        base("change_function_signature",vec![("append_parameters",json!({"type":"array","minItems":1,"maxItems":MAX_APPEND_PARAMETERS,"items":new_parameter()}))]),
        base("change_function_signature",vec![("parameters",json!({"type":"array","minItems":0,"maxItems":4096,"items":mapped_parameter}))]),
        base("replace_function_body",vec![("body",reference("expression"))]),
        base("repair_diagnostic",vec![("rejected_intent",base("replace_function_body",vec![("body",json!({"oneOf":[literal("i64"),literal("i32"),literal("u8"),literal("usize")]}))])),("repair_id",digest_schema())]),
        base("replace_expression",vec![("expression_id",text(16_384)),("replacement",reference("expression"))]),
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
    let fields = json!({"type":"array","maxItems":64,"items":closed(&[
        ("id",stable_id()),("name",identifier()),("type",json!({"enum":["i64","bool"]})),
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
    closed(&[
        ("id", stable_id()),
        ("name", identifier()),
        (
            "parameters",
            json!({"type":"array","maxItems":64,"items":{"oneOf":parameters}}),
        ),
        (
            "return_type",
            json!({"oneOf":[{"enum":["i64","i32","u8","usize","bool","Bytes"]},nominal_type_schema()]}),
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
mod aggregate_expression_schema_tests {
    use super::*;

    #[test]
    fn computed_signature_arguments_are_a_separate_recursive_mapping_only_form() {
        let schema = intent_schema();
        let forms = schema["oneOf"].as_array().unwrap();
        let append = forms
            .iter()
            .find(|form| form["properties"].get("append_parameters").is_some())
            .unwrap();
        let literal_items = append["properties"]["append_parameters"]["items"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(literal_items.len(), 5);
        for literal in literal_items {
            assert_eq!(literal["additionalProperties"], false);
            assert_eq!(literal["required"], json!(["name", "type", "argument"]));
            assert!(literal["properties"].get("argument_expression").is_none());
            assert_eq!(
                literal["properties"]["type"]["const"],
                literal["properties"]["argument"]["properties"]["kind"]["const"]
            );
        }
        let mapped = forms
            .iter()
            .find(|form| form["properties"].get("parameters").is_some())
            .unwrap();
        let choices = mapped["properties"]["parameters"]["items"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(choices.len(), 4);
        assert_eq!(choices[0]["required"], json!(["from"]));
        assert_eq!(choices[1]["required"], json!(["from", "name"]));
        assert_eq!(choices[2]["oneOf"].as_array().unwrap(), literal_items);
        let computed = &choices[3];
        assert_eq!(computed["additionalProperties"], false);
        assert_eq!(
            computed["required"],
            json!(["name", "type", "argument_expression"])
        );
        assert_eq!(computed["properties"].as_object().unwrap().len(), 3);
        assert_eq!(computed["properties"]["type"]["enum"], json!(SCALAR_KINDS));
        assert_eq!(
            computed["properties"]["argument_expression"],
            reference("expression")
        );
        for excluded in ["from", "argument", "source", "mode"] {
            assert!(computed["properties"].get(excluded).is_none());
        }
    }

    #[test]
    fn lexical_binding_closes_scope_and_recursive_children_without_type_authority() {
        let schema = expression_schema();
        let binding = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|variant| variant["properties"]["kind"]["const"] == "let")
            .unwrap();
        assert_eq!(binding["additionalProperties"], false);
        assert_eq!(
            binding["required"],
            json!(["kind", "name", "value", "body"])
        );
        assert_eq!(binding["properties"].as_object().unwrap().len(), 4);
        assert_eq!(binding["properties"]["value"], reference("expression"));
        assert_eq!(binding["properties"]["body"], reference("expression"));
        let name = &binding["properties"]["name"];
        assert_eq!(name["maxLength"], MAX_NAME_BYTES);
        assert_eq!(name["pattern"], "^[A-Za-z_][A-Za-z0-9_]*$");
        let reserved = name["not"]["enum"].as_array().unwrap();
        for token in [
            "let", "mut", "_", "record", "variant", "class", "resource", "type", "protocol",
            "impl", "for", "extends", "Option", "Result",
        ] {
            assert!(reserved.contains(&json!(token)));
        }
        assert_eq!(binding["x-implicit-let-nodes"], 1);
        assert_eq!(binding["x-value-and-body-depth-increment"], 1);
        assert_eq!(binding["x-initializer-scope"], "outside_new_binding");
        assert_eq!(binding["x-body-scope"], "immutable_local_binding");
        assert_eq!(binding["x-evaluation-order"], "value_then_body");
        for field in ["type", "mutable", "source", "declared_type"] {
            assert!(binding["properties"].get(field).is_none());
        }
    }

    #[test]
    fn declaration_nominal_types_are_closed_and_only_value_parameters() {
        let declaration = function_declaration_schema();
        let parameters = declaration["properties"]["parameters"]["items"]["oneOf"]
            .as_array()
            .unwrap();
        assert_eq!(parameters.len(), 4);
        assert_eq!(
            parameters[0]["properties"]["type"]["enum"],
            json!(SCALAR_KINDS)
        );
        assert_eq!(
            parameters[1]["properties"]["type"]["enum"],
            json!(["Bytes"])
        );
        assert_eq!(
            parameters[2]["properties"]["type"]["enum"],
            json!(["str", "Slice<u8>"])
        );
        assert_eq!(parameters[3]["properties"]["mode"]["const"], "value");
        let nominal = &parameters[3]["properties"]["type"];
        assert_eq!(nominal["additionalProperties"], false);
        assert_eq!(
            nominal["required"],
            json!(["kind", "target", "type_arguments"])
        );
        assert_eq!(
            nominal["properties"]["type_arguments"]["maxItems"],
            MAX_AGGREGATE_TYPE_ARGUMENTS
        );
        assert_eq!(
            nominal["properties"]["type_arguments"]["items"]["enum"],
            json!(["i64", "bool"])
        );
        assert!(nominal["properties"]["type_arguments"]
            .get("minItems")
            .is_none());
        assert_eq!(
            &declaration["properties"]["return_type"]["oneOf"][1],
            nominal
        );
    }

    #[test]
    fn type_declarations_close_members_and_preserve_function_shape() {
        let schema = declaration_schema();
        let forms = schema["oneOf"].as_array().unwrap();
        assert_eq!(forms.len(), 3);
        assert_eq!(forms[0], function_declaration_schema());
        assert!(forms[0]["properties"].get("kind").is_none());
        assert_eq!(
            forms[1]["required"],
            json!(["kind", "id", "name", "fields"])
        );
        assert_eq!(forms[2]["required"], json!(["kind", "id", "name", "cases"]));
        for form in &forms[1..] {
            assert_eq!(form["additionalProperties"], false);
            assert_eq!(form["x-max-combined-identities"], 4096);
            assert!(form["properties"].get("type_parameters").is_none());
        }
        let cases = &forms[2]["properties"]["cases"];
        assert_eq!(cases["minItems"], 1);
        assert_eq!(cases["maxItems"], 64);
        assert_eq!(cases["items"]["additionalProperties"], false);
        assert_eq!(cases["items"]["required"], json!(["id", "name", "fields"]));
        for fields in [
            &forms[1]["properties"]["fields"],
            &cases["items"]["properties"]["fields"],
        ] {
            assert_eq!(fields["maxItems"], 64);
            assert!(fields.get("minItems").is_none());
            assert_eq!(fields["items"]["additionalProperties"], false);
            assert_eq!(fields["items"]["required"], json!(["id", "name", "type"]));
            assert_eq!(
                fields["items"]["properties"]["type"]["enum"],
                json!(["i64", "bool"])
            );
        }
    }

    #[test]
    fn aggregate_constructors_are_closed_recursive_identity_selected_shapes() {
        let schema = expression_schema();
        let variants = schema["oneOf"].as_array().unwrap();
        let kinds = variants
            .iter()
            .map(|variant| variant["properties"]["kind"]["const"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            kinds,
            [
                "i64", "i32", "u8", "usize", "bool", "place", "call", "binary", "unary", "if",
                "record", "variant", "project", "match", "update", "let"
            ]
            .into_iter()
            .collect()
        );
        for kind in ["record", "variant"] {
            let variant = variants
                .iter()
                .find(|variant| variant["properties"]["kind"]["const"] == kind)
                .unwrap();
            assert_eq!(variant["additionalProperties"], false);
            assert_eq!(variant["required"], json!(["kind", "target", "fields"]));
            assert_eq!(variant["properties"].as_object().unwrap().len(), 4);
            let fields = &variant["properties"]["fields"];
            assert_eq!(fields["maxItems"], MAX_EXPRESSION_NODES - 1);
            assert_eq!(fields["items"]["additionalProperties"], false);
            assert_eq!(fields["items"]["required"], json!(["target", "value"]));
            assert_eq!(
                fields["items"]["properties"]["value"],
                reference("expression")
            );
            let arguments = &variant["properties"]["type_arguments"];
            assert_eq!(arguments["type"], "array");
            assert_eq!(arguments["maxItems"], MAX_AGGREGATE_TYPE_ARGUMENTS);
            assert_eq!(arguments["items"]["enum"], json!(["i64", "bool"]));
            assert!(arguments.get("minItems").is_none());
            assert!(!variant["required"]
                .as_array()
                .unwrap()
                .contains(&json!("type_arguments")));
            assert!(variant["properties"].get("source").is_none());
        }
    }

    #[test]
    fn projection_recurses_through_one_base_and_keeps_owner_selection_compiler_owned() {
        let schema = expression_schema();
        let projection = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["properties"]["kind"]["const"] == "project")
            .unwrap();
        assert_eq!(projection["additionalProperties"], false);
        assert_eq!(projection["required"], json!(["kind", "target", "base"]));
        assert_eq!(projection["properties"].as_object().unwrap().len(), 4);
        assert_eq!(projection["properties"]["base"], reference("expression"));
        assert_eq!(
            projection["properties"]["type_arguments"]["items"]["enum"],
            json!(["i64", "bool"])
        );
        assert_eq!(
            projection["properties"]["type_arguments"]["maxItems"],
            MAX_AGGREGATE_TYPE_ARGUMENTS
        );
        assert_eq!(projection["x-implicit-project-nodes"], 3);
        assert_eq!(projection["x-base-depth-increment"], 2);
        assert!(projection["properties"].get("owner").is_none());
        assert!(projection["properties"].get("name").is_none());
        assert!(projection["properties"].get("source").is_none());
    }

    #[test]
    fn exhaustive_match_schema_closes_case_payload_bindings_and_recursive_bodies() {
        let schema = expression_schema();
        let matching = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["properties"]["kind"]["const"] == "match")
            .unwrap();
        assert_eq!(matching["additionalProperties"], false);
        assert_eq!(
            matching["required"],
            json!(["kind", "target", "value", "arms"])
        );
        assert_eq!(matching["properties"].as_object().unwrap().len(), 5);
        assert_eq!(matching["properties"]["value"], reference("expression"));
        let arm = &matching["properties"]["arms"]["items"];
        assert_eq!(arm["additionalProperties"], false);
        assert_eq!(arm["required"], json!(["target", "fields", "body"]));
        assert_eq!(arm["properties"]["body"], reference("expression"));
        let field = &arm["properties"]["fields"]["items"];
        assert_eq!(field["additionalProperties"], false);
        assert_eq!(field["required"], json!(["target", "name"]));
        assert_eq!(field["properties"]["name"], identifier());
        assert!(arm["properties"].get("guard").is_none());
        assert!(matching["properties"].get("mode").is_none());
        assert_eq!(matching["x-implicit-match-nodes"], 3);
        assert_eq!(matching["x-value-and-body-depth-increment"], 2);
        assert!(!matching["required"]
            .as_array()
            .unwrap()
            .contains(&json!("type_arguments")));
    }

    #[test]
    fn record_update_schema_closes_ordered_field_subset_and_recursive_base() {
        let schema = expression_schema();
        let update = schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["properties"]["kind"]["const"] == "update")
            .unwrap();
        assert_eq!(update["additionalProperties"], false);
        assert_eq!(
            update["required"],
            json!(["kind", "target", "base", "fields"])
        );
        assert_eq!(update["properties"].as_object().unwrap().len(), 5);
        assert_eq!(update["properties"]["base"], reference("expression"));
        let fields = &update["properties"]["fields"];
        assert_eq!(fields["maxItems"], MAX_EXPRESSION_NODES - 1);
        assert!(fields.get("minItems").is_none());
        assert_eq!(fields["items"]["additionalProperties"], false);
        assert_eq!(fields["items"]["required"], json!(["target", "value"]));
        assert_eq!(
            fields["items"]["properties"]["value"],
            reference("expression")
        );
        assert_eq!(update["x-implicit-update-nodes"], 3);
        assert_eq!(update["x-base-and-fields-depth-increment"], 2);
        assert!(update["properties"].get("owner").is_none());
        assert!(update["properties"].get("name").is_none());
    }
}
