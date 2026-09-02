use super::*;
use crate::{format, parser::Parser};
use serde_json::json;
use std::path::Path;

fn parse(source: &str) -> Program {
    Parser::new(source, Path::new("fixture.spx"))
        .unwrap()
        .parse()
        .unwrap()
}

fn programs() -> Vec<Program> {
    vec![
        parse(
            r#"module image.core;
@id("image.add") fn add(a: i64, b: i64) -> i64 { a + b }
@id("image.local") fn local(x: i64) -> i64
requires add(x, 0) >= 0
ensures add(result, 0) >= 0
{
let mut n = add(x, 1);
while add(n, 0) > 0 { n = add(n, -1); n > 0 }
add(add(x, 1), 2)
}
"#,
        ),
        parse(
            r#"module image.consumer;
use function @id("image.add") from image.core as plus;
@id("image.consumer.main") fn main() -> i64 { plus(3, 4) }
"#,
        ),
    ]
}

fn append() -> Value {
    json!({"kind":"change_function_signature","target":"image.add","append_parameters":[
        {"name":"offset","type":"i64","argument":{"kind":"i64","value":0}}
    ]})
}

fn data_expression(request: &Value) -> Result<Expr> {
    construct_expression_inner(
        None,
        &programs()[0],
        &BTreeSet::new(),
        NominalScope::new(),
        request,
    )
}

#[test]
fn string_literals_preserve_decoded_data_and_bound_utf8_before_rendering() {
    for contents in ["", "\0\u{7f}\n\r\t\\\"é🦀", "{kind:call,target:ambient}"] {
        let expression = data_expression(&json!({"kind":"string","value":contents})).unwrap();
        assert!(matches!(&expression.kind, ExprKind::String(actual) if actual == contents));
        let mut program =
            parse("module image.data; @id(\"image.data.text\") fn text() -> string { \"\" }");
        program.functions[0].body = expression;
        let source = format::canonical(&program);
        let reparsed = parse(&source);
        assert_eq!(format::canonical(&reparsed), source);
        let ExprKind::Block { tail, .. } = &reparsed.functions[0].body.kind else {
            panic!("function body must remain a block");
        };
        assert!(matches!(&tail.kind, ExprKind::String(actual) if actual == contents));
    }
    let limit = "é".repeat(MAX_STRING_LITERAL_BYTES / 2);
    assert!(data_expression(&json!({"kind":"string","value":limit})).is_ok());
    let over = format!("{limit}a");
    let errors = data_expression(&json!({"kind":"string","value":over})).unwrap_err();
    assert_eq!(errors[0].code, "SPX-G226");
}

#[test]
fn byte_array_literals_charge_payloads_to_the_shared_expression_budget() {
    let limit = json!({"kind":"array_u8","values":vec![255u8; MAX_EXPRESSION_NODES - 1]});
    let expression = data_expression(&limit).unwrap();
    assert!(
        matches!(expression.kind, ExprKind::ArrayU8(values) if values.len() == MAX_EXPRESSION_NODES - 1)
    );
    assert!(
        matches!(data_expression(&json!({"kind":"array_u8","values":[]})).unwrap().kind, ExprKind::ArrayU8(values) if values.is_empty())
    );
    let nested = json!({"kind":"let","name":"first",
        "value":{"kind":"array_u8","values":vec![0u8; 2047]},
        "body":{"kind":"array_u8","values":vec![1u8; 2047]}});
    for request in [
        json!({"kind":"array_u8","values":vec![0u8; MAX_EXPRESSION_NODES]}),
        nested,
    ] {
        assert_eq!(data_expression(&request).unwrap_err()[0].code, "SPX-G226");
    }
}

#[test]
fn data_literal_grammar_does_not_expand_scalar_migration_defaults() {
    for request in [
        json!({"kind":"array_u8","values":[-1]}),
        json!({"kind":"array_u8","values":[256]}),
        json!({"kind":"array_u8","values":[1.0]}),
        json!({"kind":"array_u8","values":[true]}),
        json!({"kind":"array_u8","values":["1"]}),
        json!({"kind":"array_u8","values":[{"kind":"u8","value":1}]}),
        json!({"kind":"string","value":null}),
        json!({"kind":"string","value":"text","source":"ambient()"}),
        json!({"kind":"repeat_array_u8","value":0,"count":1}),
    ] {
        assert_eq!(data_expression(&request).unwrap_err()[0].code, "SPX-G225");
    }
    for argument in [
        json!({"kind":"string","value":""}),
        json!({"kind":"array_u8","values":[]}),
    ] {
        let mut intention = append();
        intention["append_parameters"][0]["argument"] = argument;
        code(apply(&mut programs(), &intention), "SPX-G225");
    }
}

#[test]
fn widened_copy_literals_preserve_scalar_bits_and_signed_node_shape() {
    let character = data_expression(&json!({"kind":"char","scalar":"0001f600"})).unwrap();
    assert!(matches!(character.kind, ExprKind::Char(0x1f600)));
    let positive = data_expression(&json!({"kind":"f64","bits":"0000000000000001"})).unwrap();
    assert!(matches!(positive.kind, ExprKind::Float64(1)));
    let negative_zero = data_expression(&json!({"kind":"f32","bits":"80000000"})).unwrap();
    assert_eq!(literal_nodes(&negative_zero), 2);
    assert!(
        matches!(negative_zero.kind, ExprKind::Unary { op: UnaryOp::Neg, value }
        if matches!(value.kind, ExprKind::Float32(0)))
    );
    for invalid in [
        json!({"kind":"char","scalar":"0000d800"}),
        json!({"kind":"char","scalar":"0001F600"}),
        json!({"kind":"f32","bits":"7f800000"}),
        json!({"kind":"f32","bits":"7fc00000"}),
        json!({"kind":"f64","bits":"fff0000000000000"}),
        json!({"kind":"f64","bits":"000000000000000"}),
    ] {
        assert_eq!(data_expression(&invalid).unwrap_err()[0].code, "SPX-G225");
    }
    assert_eq!(scalar_type("char").unwrap(), Type::Char);
    assert_eq!(scalar_type("f32").unwrap(), Type::F32);
    assert_eq!(scalar_type("f64").unwrap(), Type::F64);
}

#[test]
fn append_migrates_nested_contract_loop_and_import_calls_without_reordering() {
    let mut programs = programs();
    let summary = apply(&mut programs, &append()).unwrap();
    assert_eq!(summary.target_id, "image.add");
    assert_eq!(summary.kind, "change_function_signature");
    assert_eq!(summary.migrated_calls, 8);
    let source = format::canonical(&programs[0]);
    assert!(source.contains("fn add(a: i64, b: i64, offset: i64)"));
    assert!(source.contains("requires add(x, 0, 0) >= 0"));
    assert!(source.contains("ensures add(result, 0, 0) >= 0"));
    assert!(source.contains("while add(n, 0, 0) > 0"));
    assert!(source.contains("add(add(x, 1, 0), 2, 0)"));
    let consumer = format::canonical(&programs[1]);
    assert!(consumer.contains("plus(3, 4, 0)"));
    assert!(consumer.contains("from image.core as plus"));
    for program in &programs {
        let canonical = format::canonical(program);
        assert_eq!(format::canonical(&parse(&canonical)), canonical);
    }
}

#[test]
fn rename_keeps_import_alias_and_identity_and_body_uses_stable_id_calls() {
    let mut programs = programs();
    let consumer = format::canonical(&programs[1]);
    let summary = apply(
        &mut programs,
        &json!({
            "kind":"rename_declaration","target":"image.add","name":"sum"
        }),
    )
    .unwrap();
    assert_eq!(summary.migrated_calls, 7);
    assert_eq!(format::canonical(&programs[1]), consumer);
    let source = format::canonical(&programs[0]);
    assert!(source.contains("@id(\"image.add\")\nfn sum("));
    assert!(source.contains("sum(sum(x, 1), 2)"));
    apply(&mut programs, &json!({
        "kind":"replace_function_body","target":"image.local",
        "body":{"kind":"if",
            "condition":{"kind":"binary","op":">=","left":{"kind":"place","name":"x"},"right":{"kind":"i64","value":0}},
            "then":{"kind":"call","target":"image.add","arguments":[{"kind":"place","name":"x"},{"kind":"i64","value":1}]},
            "else":{"kind":"unary","op":"-","value":{"kind":"place","name":"x"}}
        }
    })).unwrap();
    let source = format::canonical(&programs[0]);
    assert!(source.contains("if x >= 0 { sum(x, 1) } else { -x }"));
    assert_eq!(format::canonical(&parse(&source)), source);
}

fn code(result: Result<IntentSummary>, expected: &str) {
    match result {
        Ok(_) => panic!("expected {expected}"),
        Err(errors) => assert!(
            errors.iter().any(|error| error.code == expected),
            "{errors:?}"
        ),
    }
}

#[test]
fn unsupported_or_effectful_migrations_and_unbound_body_nodes_fail_closed() {
    // The deeply nested unary chain exercises the constructor depth guard
    // without relying on the default test-thread stack size, which can be
    // as small as 2 MiB on some runners.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let invalid = [
        json!({"kind":"change_function_signature","target":"image.add","parameters":[{"from":"missing"}]}),
        json!({"kind":"change_function_signature","target":"image.add","append_parameters":[{"name":"offset","type":"i64","argument":{"kind":"call","target":"image.add","arguments":[]}}]}),
        json!({"kind":"change_function_signature","target":"image.add","append_parameters":[{"name":"a","type":"i64","argument":{"kind":"i64","value":0}}]}),
        json!({"kind":"replace_function_body","target":"image.add","body":{"kind":"place","name":"missing"}}),
        json!({"kind":"replace_function_body","target":"image.add","body":{"kind":"call","target":"not.imported","arguments":[]}}),
        json!({"kind":"replace_function_body","target":"image.add","body":{"kind":"i64","value":0,"source":"ambient()"}}),
        json!({"kind":"rename_declaration","target":"image.add","name":"local"}),
        json!({"kind":"rename_declaration","target":"image.consumer.main","name":"entry"}),
        json!({"kind":"rename_declaration","target":"image.add","name":"sum() { 1 }"}),
    ];
    for intention in invalid {
        code(apply(&mut programs(), &intention), "SPX-G225");
    }
    let mut nested = json!({"kind":"i64","value":0});
    for _ in 0..=MAX_EXPRESSION_DEPTH {
        nested = json!({"kind":"unary","op":"-","value":nested});
    }
    code(
        apply(
            &mut programs(),
            &json!({"kind":"replace_function_body","target":"image.add","body":nested}),
        ),
        "SPX-G226",
    );
        })
        .unwrap()
        .join()
        .unwrap();
}
