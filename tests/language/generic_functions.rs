use std::path::Path;

use semaprax::graph::{self, AgentContextFilter, AgentContextOptions};
use semaprax::hir::{self, DeclarationId, ResolvedExprKind, ResolvedType};
use semaprax::{codegen, format, parse, semantic_trace, verify, wasm};
use sha2::{Digest as _, Sha256};

#[test]
fn generic_string_templates_replay_owned_slots_and_reject_forged_intrinsics() {
    let source = r#"
module test.generic_strings;
@id("test.text") fn text<T>(value: T, text: string) -> string { text }
@id("test.measure") fn measure<T>(value: T) -> i64 {
    let text = string_from_char('\0');
    string_len_chars(text)
}
@id("app.main") fn main() -> i64 { measure<i64>(42) }
"#;
    let program = semaprax::check(source, "generic-strings.spx").unwrap();
    let canonical = format::canonical(&program);
    let reparsed = semaprax::check(&canonical, "generic-strings.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&reparsed).unwrap()
    );
    assert_eq!(resolved.function_instances.len(), 1);
    assert_eq!(
        resolved.function_templates[0].params[1].ownership,
        hir::OwnershipMode::Own
    );
    for forged in [hir::OwnershipMode::Value, hir::OwnershipMode::Borrow] {
        let mut hostile = resolved.clone();
        hostile.function_templates[0].params[1].ownership = forged;
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
        let mut hostile = resolved.clone();
        hostile.function_templates[0].body.ownership = forged;
        assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    }
    let mut hostile = resolved.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut hostile.function_templates[1].body.kind
    else {
        panic!("expected template block");
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("expected String binding");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut value.kind else {
        panic!("expected intrinsic");
    };
    *callee = DeclarationId::new("core.string.forged");
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
    let mut hostile = resolved;
    let ResolvedExprKind::Block { statements, .. } = &mut hostile.function_templates[1].body.kind
    else {
        panic!("expected template block");
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("expected String binding");
    };
    let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
        panic!("expected intrinsic");
    };
    args.clear();
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

const SOURCE: &str = r#"
module test.generic_functions;

@id("test.id")
fn id<T>(value: T) -> T
    requires value == value
    ensures result == value
{
    value
}

@id("test.first")
fn first<T, U>(left: T, right: U) -> T { left }

@id("test.unused")
fn unused<T>(value: T) -> T { value }

@id("app.main")
fn main() -> i64 {
    let number = id<i64>(40);
    let flag = id<bool>(true);
    number + first<i64, bool>(2, flag)
}
"#;

fn parse_source(source: &str) -> semaprax::ast::Program {
    parse(source, Path::new("generic-functions.spx")).unwrap()
}

fn error_codes(source: &str) -> Vec<&'static str> {
    verify::verify(&parse_source(source))
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn sha256_text(value: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
    )
}

#[test]
fn explicit_generic_functions_round_trip_and_materialize_only_reachable_instances() {
    let program = parse_source(SOURCE);
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("fn id<T>(value: T) -> T"));
    assert!(canonical.contains("id<i64>(40)"));
    assert!(canonical.contains("first<i64, bool>(2, flag)"));
    let reparsed = parse_source(&canonical);
    assert_eq!(canonical, format::canonical(&reparsed));

    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    assert_eq!(resolved.functions.len(), 1);
    assert_eq!(resolved.function_templates.len(), 3);
    assert_eq!(resolved.function_instances.len(), 3);
    assert!(resolved
        .function_instances
        .iter()
        .all(|instance| instance.template.as_str() != "test.unused"));
    assert_eq!(
        resolved
            .function_instances
            .iter()
            .map(|instance| { (instance.template.as_str(), instance.type_arguments.clone(),) })
            .collect::<Vec<_>>(),
        vec![
            ("test.id", vec![ResolvedType::I64]),
            ("test.id", vec![ResolvedType::Bool]),
            ("test.first", vec![ResolvedType::I64, ResolvedType::Bool]),
        ]
    );
    assert!(resolved
        .function_instances
        .iter()
        .all(|instance| { instance.function.cleanup_plan.schema == "semaprax.cleanup-plan.v2" }));

    let ResolvedExprKind::Block { statements, tail } = &resolved.functions[0].body.kind else {
        panic!("main must resolve as a block");
    };
    assert_eq!(statements.len(), 2);
    let ResolvedExprKind::Binary { right, .. } = &tail.kind else {
        panic!("main tail must be a binary expression");
    };
    let ResolvedExprKind::Call {
        callee,
        instance: Some(instance),
        type_arguments,
        ..
    } = &right.kind
    else {
        panic!("main must call the exact first<i64, bool> instance");
    };
    assert_eq!(callee.as_str(), "test.first");
    assert_eq!(type_arguments, &[ResolvedType::I64, ResolvedType::Bool]);
    assert_eq!(
        instance,
        &hir::FunctionInstanceId::derive(callee, type_arguments)
    );
}

#[test]
fn explicit_generic_function_relays_one_admitted_owned_record_instance() {
    let source = r#"
module test.generic_owned_relay;
@id("generic.relay.pair") record Pair<T, U> {
    @id("generic.relay.pair.payload") payload: T,
    @id("generic.relay.pair.marker") marker: U,
}
@id("generic.relay.function")
fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { value }
@id("generic.relay.consume")
fn consume(value: own Pair<Bytes, u8>) -> i64 {
    match own value { Pair { payload: payload, marker: marker } =>
        if marker == 7u8 { 42 } else { 0 }, }
}
@id("app.main") fn main() -> i64 {
    let input = [1u8, 2u8];
    let value = Pair<Bytes, u8> {
        payload: bytes_copy(array_as_slice(input)), marker: 7u8,
    };
    consume(relay<u8>(value))
}
"#;
    let program = parse_source(source);
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T>"));
    assert_eq!(canonical, format::canonical(&parse_source(&canonical)));

    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let instance = resolved
        .function_instances
        .iter()
        .find(|instance| instance.template.as_str() == "generic.relay.function")
        .expect("reachable relay instance");
    assert_eq!(instance.type_arguments, [ResolvedType::U8]);
    let expected = ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.relay.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::U8],
    };
    assert_eq!(
        instance.function.params[0].ownership,
        hir::OwnershipMode::Own
    );
    assert_eq!(instance.function.params[0].ty, expected);
    assert_eq!(instance.function.return_type, expected);

    let mut hostile = resolved.clone();
    let instance = hostile
        .function_instances
        .iter_mut()
        .find(|instance| instance.template.as_str() == "generic.relay.function")
        .unwrap();
    instance.type_arguments[0] = ResolvedType::F32;
    assert_eq!(hir::validate(&hostile).unwrap_err().code, "SPX-H006");
}

#[test]
fn legacy_flat_owned_generic_multi_parameter_admission_remains_compatible() {
    let source = r#"
module test.generic_flat_multi_owner;
@id("generic.flat.pair") record Pair<T, U> {
    @id("generic.flat.pair.payload") payload: T,
    @id("generic.flat.pair.marker") marker: U,
}
@id("generic.flat.choose")
fn choose<T>(
    left: own Pair<Bytes, T>,
    right: own Pair<Bytes, T>,
    take_left: bool
) -> Pair<Bytes, T> {
    if take_left { left } else { right }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = parse_source(source);
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let template = resolved
        .function_templates
        .iter()
        .find(|template| template.id.as_str() == "generic.flat.choose")
        .unwrap();
    assert_eq!(
        template
            .params
            .iter()
            .filter(|parameter| parameter.ownership == hir::OwnershipMode::Own)
            .count(),
        2
    );
}

fn nested_owned_relay_source(copy_type: &str, copy_value: &str) -> String {
    format!(
        r#"
module test.nested_generic_owned_relay;
@id("nested.relay.box") record Box<T> {{
    @id("nested.relay.box.value") value: T,
}}
@id("nested.relay.pair") record Pair<T, U> {{
    @id("nested.relay.pair.left") left: T,
    @id("nested.relay.pair.right") right: U,
}}
@id("nested.relay.box-function")
fn relay_box<T>(value: own Box<Pair<Bytes, T>>) -> Box<Pair<Bytes, T>> {{ value }}
@id("nested.relay.pair-function")
fn relay_pair<T>(value: own Pair<Box<Bytes>, T>) -> Pair<Box<Bytes>, T> {{ value }}
@id("app.main") fn main() -> i64 {{
    let first = [1u8];
    let boxed = Box<Pair<Bytes, {copy_type}>> {{
        value: Pair<Bytes, {copy_type}> {{
            left: bytes_copy(array_as_slice(first)), right: {copy_value},
        }},
    }};
    let relayed_box = relay_box<{copy_type}>(boxed);
    let second = [2u8];
    let paired = Pair<Box<Bytes>, {copy_type}> {{
        left: Box<Bytes> {{ value: bytes_copy(array_as_slice(second)) }},
        right: {copy_value},
    }};
    let relayed_pair = relay_pair<{copy_type}>(paired);
    0
}}
"#,
    )
}

fn nested_owned_relay_depth_source(box_depth: usize) -> String {
    let mut ty = String::from("Pair<Bytes, T>");
    for _ in 0..box_depth {
        ty = format!("Box<{ty}>");
    }
    format!(
        r#"
module test.nested_generic_relay_depth;
@id("nested.relay.depth.box") record Box<T> {{
    @id("nested.relay.depth.box.value") value: T,
}}
@id("nested.relay.depth.pair") record Pair<T, U> {{
    @id("nested.relay.depth.pair.left") left: T,
    @id("nested.relay.depth.pair.right") right: U,
}}
@id("nested.relay.depth.function")
fn relay<T>(value: own {ty}) -> {ty} {{ value }}
@id("app.main") fn main() -> i64 {{ 0 }}
"#,
    )
}

fn nested_owned_relay_leaf_source(byte_leaves: usize) -> String {
    let fields = (0..byte_leaves)
        .map(|index| format!("    leaf_{index}: Bytes,"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"
module test.nested_generic_relay_leaves;
record Bag {{
{fields}
}}
record Box<T> {{ value: T, }}
record Pair<T, U> {{ left: T, right: U, }}
fn relay<T>(value: own Box<Pair<Bag, T>>) -> Box<Pair<Bag, T>> {{ value }}
fn main() -> i64 {{ 0 }}
"#,
    )
}

fn nominal_type(declaration: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(declaration),
        arguments,
    }
}

#[test]
fn nested_owning_generic_relays_preserve_exact_instances_for_every_copy_scalar() {
    for (copy_type, copy_value, resolved_copy) in [
        ("i64", "7", ResolvedType::I64),
        ("i32", "7i32", ResolvedType::I32),
        ("u8", "7u8", ResolvedType::U8),
        ("usize", "7usize", ResolvedType::Usize),
        ("char", "'x'", ResolvedType::Char),
        ("f32", "1.5f32", ResolvedType::F32),
        ("f64", "1.5f64", ResolvedType::F64),
        ("bool", "true", ResolvedType::Bool),
    ] {
        let source = nested_owned_relay_source(copy_type, copy_value);
        let program = parse_source(&source);
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.severity.is_error()),
            "{copy_type}: {diagnostics:?}"
        );
        let canonical = format::canonical(&program);
        assert_eq!(canonical, format::canonical(&parse_source(&canonical)));

        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        assert_eq!(resolved.function_templates.len(), 2, "{copy_type}");
        assert_eq!(resolved.function_instances.len(), 2, "{copy_type}");

        let pair_bytes_copy = nominal_type(
            "nested.relay.pair",
            vec![ResolvedType::Bytes, resolved_copy.clone()],
        );
        let box_pair = nominal_type("nested.relay.box", vec![pair_bytes_copy]);
        let box_bytes = nominal_type("nested.relay.box", vec![ResolvedType::Bytes]);
        let pair_box = nominal_type("nested.relay.pair", vec![box_bytes, resolved_copy.clone()]);
        for (template_id, expected_type) in [
            ("nested.relay.box-function", box_pair),
            ("nested.relay.pair-function", pair_box),
        ] {
            let template = resolved
                .function_templates
                .iter()
                .find(|template| template.id.as_str() == template_id)
                .unwrap();
            assert_eq!(template.params[0].ownership, hir::OwnershipMode::Own);
            assert_eq!(template.body.ownership, hir::OwnershipMode::Own);
            assert_eq!(template.type_parameters.len(), 1);
            assert_eq!(template.type_parameters[0].index, 0);
            let template_argument = ResolvedType::TypeParameter {
                owner: DeclarationId::new(template_id),
                index: 0,
            };
            let expected_template_type = if template_id == "nested.relay.box-function" {
                nominal_type(
                    "nested.relay.box",
                    vec![nominal_type(
                        "nested.relay.pair",
                        vec![ResolvedType::Bytes, template_argument],
                    )],
                )
            } else {
                nominal_type(
                    "nested.relay.pair",
                    vec![
                        nominal_type("nested.relay.box", vec![ResolvedType::Bytes]),
                        template_argument,
                    ],
                )
            };
            assert_eq!(template.params[0].ty, expected_template_type);
            assert_eq!(template.return_type, expected_template_type);

            let instance = resolved
                .function_instances
                .iter()
                .find(|instance| instance.template.as_str() == template_id)
                .unwrap();
            assert_eq!(
                instance.type_arguments,
                std::slice::from_ref(&resolved_copy)
            );
            assert_eq!(
                instance.id,
                hir::FunctionInstanceId::derive(
                    &DeclarationId::new(template_id),
                    std::slice::from_ref(&resolved_copy)
                )
            );
            assert_eq!(
                instance.function.params[0].ownership,
                hir::OwnershipMode::Own
            );
            assert_eq!(instance.function.params[0].ty, expected_type);
            assert_eq!(instance.function.return_type, expected_type);
            assert_eq!(instance.function.body.ownership, hir::OwnershipMode::Own);
        }
    }
}

#[test]
fn generic_forwarding_closes_over_transitive_instances_in_authored_fifo_order() {
    let source = r#"
module test.generic_forwarding;
@id("forward.box") record Box<T> { @id("forward.box.value") value: T, }
@id("forward.pair") record Pair<T, U> {
  @id("forward.pair.left") left: T,
  @id("forward.pair.right") right: U,
}
@id("forward.inner")
fn inner<T>(value: own Box<Pair<Bytes, T>>, allowed: bool) -> Box<Pair<Bytes, T>>
  requires allowed
{ value }
@id("forward.middle")
fn middle<T>(value: own Box<Pair<Bytes, T>>, allowed: bool) -> Box<Pair<Bytes, T>> {
  inner<T>(value, allowed)
}
@id("forward.outer")
fn outer<T>(value: own Box<Pair<Bytes, T>>, allowed: bool) -> Box<Pair<Bytes, T>> {
  middle<T>(value, allowed)
}
@id("forward.entry")
fn entry(value: own Box<Pair<Bytes, bool>>, allowed: bool) -> Box<Pair<Bytes, bool>> {
  outer<bool>(value, allowed)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = parse_source(source);
    assert!(verify::verify(&parsed).is_empty());
    let resolved = hir::resolve(&parsed).unwrap();
    hir::validate(&resolved).unwrap();
    assert_eq!(
        resolved
            .function_instances
            .iter()
            .map(|instance| instance.template.as_str())
            .collect::<Vec<_>>(),
        ["forward.outer", "forward.middle", "forward.inner"]
    );
    for pair in resolved.function_instances.windows(2) {
        let hir::ResolvedExprKind::Block { tail: call, .. } = &pair[0].function.body.kind else {
            panic!("forwarding instance body must remain a block")
        };
        let hir::ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance: Some(instance),
            ..
        } = &call.kind
        else {
            panic!("forwarding instance body must be one exact generic call")
        };
        assert_eq!(callee, &pair[1].template);
        assert_eq!(type_arguments, &[ResolvedType::Bool]);
        assert_eq!(
            instance,
            &hir::FunctionInstanceId::derive(callee, type_arguments)
        );
    }

    let mut missing = resolved.clone();
    missing.function_instances.pop();
    assert_eq!(hir::validate(&missing).unwrap_err().code, "SPX-H006");

    let mut reordered = resolved.clone();
    reordered.function_instances.swap(1, 2);
    assert_eq!(hir::validate(&reordered).unwrap_err().code, "SPX-H006");

    let mut forged = resolved;
    let replacement = forged.function_instances[0].id.clone();
    let hir::ResolvedExprKind::Block { tail, .. } =
        &mut forged.function_instances[1].function.body.kind
    else {
        panic!("middle body must remain a block")
    };
    let hir::ResolvedExprKind::Call { instance, .. } = &mut tail.kind else {
        panic!("middle body must remain a call")
    };
    *instance = Some(replacement);
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
}

#[test]
fn generic_forwarding_rejects_nonidentity_arguments_and_cycles_stably() {
    let direct_scalar = r#"
module test.direct_scalar_forward;
@id("scalar.inner") fn inner<T>(value: T) -> T { value }
@id("scalar.outer") fn outer<T>(value: T) -> T { inner<T>(value) }
@id("app.main") fn main() -> i64 { if outer<bool>(true) { 0 } else { 1 } }
"#;
    let direct_program = parse_source(direct_scalar);
    assert!(verify::verify(&direct_program).is_empty());
    let direct_hir = hir::resolve(&direct_program).unwrap();
    hir::validate(&direct_hir).unwrap();

    let outer = direct_hir
        .function_templates
        .iter()
        .position(|template| template.id.as_str() == "scalar.outer")
        .unwrap();
    let inner = direct_hir
        .function_templates
        .iter()
        .position(|template| template.id.as_str() == "scalar.inner")
        .unwrap();
    let mut self_cycle = direct_hir.clone();
    let hir::ResolvedExprKind::Block { tail, .. } =
        &mut self_cycle.function_templates[outer].body.kind
    else {
        panic!("outer template body must remain a block")
    };
    let hir::ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        ..
    } = &mut tail.kind
    else {
        panic!("outer template body must remain a call")
    };
    *callee = DeclarationId::new("scalar.outer");
    *instance = Some(hir::FunctionInstanceId::derive(callee, type_arguments));
    assert_eq!(hir::validate(&self_cycle).unwrap_err().code, "SPX-H006");

    let mut two_node_cycle = direct_hir.clone();
    two_node_cycle.function_templates[inner].body =
        two_node_cycle.function_templates[outer].body.clone();
    let hir::ResolvedExprKind::Block { tail, .. } =
        &mut two_node_cycle.function_templates[inner].body.kind
    else {
        panic!("copied template body must remain a block")
    };
    let hir::ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        ..
    } = &mut tail.kind
    else {
        panic!("copied template body must remain a call")
    };
    *callee = DeclarationId::new("scalar.outer");
    type_arguments[0] = ResolvedType::TypeParameter {
        owner: DeclarationId::new("scalar.inner"),
        index: 0,
    };
    *instance = Some(hir::FunctionInstanceId::derive(callee, type_arguments));
    assert_eq!(hir::validate(&two_node_cycle).unwrap_err().code, "SPX-H006");

    let concrete = r#"
module test.bad_concrete_forward;
@id("bad.inner") fn inner<T>(value: T) -> T { value }
@id("bad.outer") fn outer<T>(value: T) -> T { inner<bool>(true) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(error_codes(concrete).contains(&"SPX-T225"));

    let permutation = r#"
module test.bad_permuted_forward;
@id("bad.inner") fn inner<T, U>(left: T, right: U) -> T { left }
@id("bad.outer") fn outer<T, U>(left: T, right: U) -> T { inner<U, T>(right, left) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(error_codes(permutation).contains(&"SPX-T225"));

    let cycle = r#"
module test.bad_forward_cycle;
@id("bad.first") fn first<T>(value: T) -> T { second<T>(value) }
@id("bad.second") fn second<T>(value: T) -> T { first<T>(value) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(error_codes(cycle).contains(&"SPX-T226"));
}

#[test]
fn nested_owning_generic_relay_source_and_hir_boundaries_fail_closed() {
    let admitted = nested_owned_relay_source("u8", "7u8");
    for hostile in [
        admitted.replace("own Box<Pair<Bytes, T>>", "Box<Pair<Bytes, T>>"),
        admitted.replace("own Pair<Box<Bytes>, T>", "borrow Pair<Box<Bytes>, T>"),
        admitted.replace("relay_box<u8>(boxed)", "relay_box<string>(boxed)"),
        admitted.replace("relay_pair<u8>(paired)", "relay_pair(paired)"),
        admitted.replace("Box<Pair<Bytes, T>>", "Box<Pair<String, T>>"),
    ] {
        let diagnostics = error_codes(&hostile);
        assert!(
            diagnostics
                .iter()
                .any(|code| matches!(*code, "SPX-T224" | "SPX-T225" | "SPX-T268")),
            "hostile source unexpectedly admitted: {diagnostics:?}\n{hostile}"
        );
    }

    let mut resolved = hir::resolve(&parse_source(&admitted)).unwrap();
    hir::validate(&resolved).unwrap();

    let box_instance_index = resolved
        .function_instances
        .iter()
        .position(|instance| instance.template.as_str() == "nested.relay.box-function")
        .unwrap();
    let pair_instance_index = resolved
        .function_instances
        .iter()
        .position(|instance| instance.template.as_str() == "nested.relay.pair-function")
        .unwrap();

    let mut wrong_argument = resolved.clone();
    wrong_argument.function_instances[box_instance_index].type_arguments[0] = ResolvedType::String;
    assert_eq!(hir::validate(&wrong_argument).unwrap_err().code, "SPX-H006");

    let box_template_index = resolved
        .function_templates
        .iter()
        .position(|template| template.id.as_str() == "nested.relay.box-function")
        .unwrap();
    let mut wrong_owner = resolved.clone();
    wrong_owner.function_templates[box_template_index].params[0].ownership =
        hir::OwnershipMode::Value;
    assert_eq!(hir::validate(&wrong_owner).unwrap_err().code, "SPX-H006");

    for (owner, index) in [
        (DeclarationId::new("nested.relay.forged"), 0),
        (DeclarationId::new("nested.relay.box-function"), 1),
    ] {
        let mut wrong_parameter = resolved.clone();
        let ResolvedType::Nominal { arguments, .. } =
            &mut wrong_parameter.function_templates[box_template_index].params[0].ty
        else {
            panic!("expected Box template");
        };
        let ResolvedType::Nominal { arguments, .. } = &mut arguments[0] else {
            panic!("expected nested Pair template");
        };
        arguments[1] = ResolvedType::TypeParameter { owner, index };
        assert_eq!(
            hir::validate(&wrong_parameter).unwrap_err().code,
            "SPX-H006"
        );
    }

    let mut wrong_nested_type = resolved.clone();
    wrong_nested_type.function_instances[box_instance_index]
        .function
        .params[0]
        .ty = nominal_type(
        "nested.relay.box",
        vec![nominal_type(
            "nested.relay.pair",
            vec![ResolvedType::Bytes, ResolvedType::Bool],
        )],
    );
    assert_eq!(
        hir::validate(&wrong_nested_type).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_instance = resolved.clone();
    wrong_instance.function_instances[box_instance_index].id = wrong_instance.function_instances
        [pair_instance_index]
        .id
        .clone();
    assert_eq!(hir::validate(&wrong_instance).unwrap_err().code, "SPX-H006");

    resolved.function_instances[pair_instance_index]
        .function
        .return_type = ResolvedType::I64;
    assert_eq!(hir::validate(&resolved).unwrap_err().code, "SPX-H006");
}

#[test]
fn nested_owning_generic_relay_uses_the_existing_global_depth_bound() {
    let exact = parse_source(&nested_owned_relay_depth_source(63));
    assert!(verify::verify(&exact).is_empty());
    hir::validate(&hir::resolve(&exact).unwrap()).unwrap();

    let plus_one = error_codes(&nested_owned_relay_depth_source(64));
    assert!(
        plus_one
            .iter()
            .any(|code| matches!(*code, "SPX-T223" | "SPX-T224")),
        "nested generic relay depth +1 unexpectedly admitted: {plus_one:?}"
    );
}

#[test]
fn nested_owning_generic_relay_uses_the_existing_global_owned_leaf_bound() {
    let exact = parse_source(&nested_owned_relay_leaf_source(256));
    let diagnostics = verify::verify(&exact);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.severity.is_error()),
        "{diagnostics:?}"
    );
    hir::validate(&hir::resolve(&exact).unwrap()).unwrap();

    let plus_one = error_codes(&nested_owned_relay_leaf_source(257));
    assert!(
        plus_one
            .iter()
            .any(|code| matches!(*code, "SPX-T223" | "SPX-T224")),
        "nested generic relay owned-leaf +1 unexpectedly admitted: {plus_one:?}"
    );
}

#[test]
fn direct_scalar_generics_and_owned_relay_template_hostility_keep_stable_diagnostics() {
    let direct = r#"
module test.direct_u8_stays_closed;
@id("generic.direct.id") fn id<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { if id<u8>(1u8) == 1u8 { 0 } else { 1 } }
"#;
    assert_eq!(error_codes(direct), ["SPX-T225"]);

    let noncopy = r#"
module test.generic_owned_relay_noncopy;
@id("generic.closed.pair") record Pair<T, U> {
    @id("generic.closed.pair.payload") payload: T,
    @id("generic.closed.pair.marker") marker: U,
}
@id("generic.closed.relay")
fn relay<T>(value: own Pair<Bytes, T>) -> Pair<Bytes, T> { value }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = parse_source(noncopy);
    assert!(verify::verify(&program).is_empty());
    let mut resolved = hir::resolve(&program).unwrap();
    resolved.function_templates[0].params[0].ownership = hir::OwnershipMode::Value;
    assert_eq!(hir::validate(&resolved).unwrap_err().code, "SPX-H006");

    let result_only_factory = r#"
module test.generic_owned_result_only;
@id("generic.factory.pair") record Pair<T, U> {
    @id("generic.factory.pair.payload") payload: T,
    @id("generic.factory.pair.marker") marker: U,
}
@id("generic.factory.make")
fn make<T>(marker: T) -> Pair<Bytes, T> { marker }
@id("app.main") fn main() -> i64 {
    let value = make<u8>(7u8);
    0
}
"#;
    let program = parse_source(result_only_factory);
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "SPX-T225"
            && diagnostic.message
                == "generic function `make` accepts only direct `i64` or `bool` type arguments, except admitted Copy scalars in an owned-record relay"
    }), "{diagnostics:?}");
}

#[test]
fn invalid_arity_is_bounded_and_specialized_diagnostics_are_stable() {
    let invalid_arity = r#"
module test.invalid_generic_arity;
@id("test.too_many")
fn too_many<A, B, C>(value: A) -> A { value }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(error_codes(invalid_arity), vec!["SPX-T224"]);

    let duplicated = r#"
module test.generic_diagnostic_dedup;
@id("test.bad")
fn bad<T>(value: T) -> T { missing }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let first = error_codes(duplicated);
    let second = error_codes(duplicated);
    assert_eq!(first, second);
    assert_eq!(first.iter().filter(|code| **code == "SPX-T202").count(), 1);
}

#[test]
fn unused_templates_are_checked_without_becoming_executable_instances() {
    let invalid = r#"
module test.invalid_unused_generic;
@id("test.negate")
fn negate<T>(value: T) -> T { -value }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert!(!error_codes(invalid).is_empty());

    let resolved = hir::resolve(&parse_source(SOURCE)).unwrap();
    assert!(resolved
        .function_instances
        .iter()
        .all(|instance| instance.template.as_str() != "test.unused"));
}

#[test]
fn comparison_operators_remain_unambiguous_with_generic_call_lookahead() {
    let source = r#"
module test.legacy_comparisons;
@id("app.main")
fn main() -> i64 { if 1 < 2 && 3 > 2 { 0 } else { 1 } }
"#;
    let program = parse_source(source);
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("1 < 2 && 3 > 2"));

    let identifiers = r#"
module test.legacy_identifier_comparisons;
@id("app.main") fn main() -> i64 {
    let value = 1;
    let limit = 2;
    let high = 3;
    if value < limit && high > (0) { 0 } else { 1 }
}
"#;
    assert!(parse(identifiers, Path::new("identifier-comparisons.spx")).is_ok());
    let before_variant = r#"
module test.legacy_before_variant;
@id("app.main") fn main() -> i64 {
    let value = 1;
    let limit = 2;
    value < limit && false > Option<bool>::None {}
}
"#;
    assert!(parse(before_variant, Path::new("comparison-before-variant.spx")).is_ok());
    let before_block = r#"
module test.legacy_before_block;
@id("app.main") fn main() -> i64 {
    let value = 1;
    let limit = 2;
    if value < limit && 3 > { 0 } { 0 } else { 1 }
}
"#;
    assert!(parse(before_block, Path::new("comparison-before-block.spx")).is_ok());
}

#[test]
fn malformed_generic_call_and_record_postfixes_keep_the_stable_parser_diagnostic() {
    let source = |expression: &str| {
        format!(
            r#"
module test.malformed_generic_postfix;
@id("test.box") record Box<T> {{ @id("test.box.value") value: T, }}
@id("test.id") fn id<T>(value: T) -> T {{ value }}
@id("app.main") fn main() -> i64 {{ {expression} }}
"#,
        )
    };
    for expression in [
        "id<>(0)",
        "id<, i64>(0)",
        "id<i64,, bool>(0)",
        "id<i64,>(0)",
        "id<Box<i64,>>(Box<i64> { value: 0 })",
        "(Box<> { value: 0 }).value",
        "(Box<, i64> { value: 0 }).value",
        "(Box<i64,, bool> { value: 0 }).value",
        "(Box<i64,> { value: 0 }).value",
        "(Box<Option<i64,>> { value: Option<i64>::None {} }).value",
    ] {
        assert_eq!(
            parse(
                &source(expression),
                Path::new("malformed-generic-postfix.spx")
            )
            .unwrap_err()
            .code,
            "SPX-P106",
            "wrong diagnostic for `{expression}`"
        );
    }

    for expression in [
        "id<Option<i64>>(Option<i64>::None {})",
        "(Box<Option<i64>> { value: Option<i64>::None {} }).value",
        "id<foreign.Type>(0)",
        "(Box<foreign.Type> { value: 0 }).value",
    ] {
        parse(
            &source(expression),
            Path::new("valid-nested-generic-postfix.spx"),
        )
        .unwrap_or_else(|diagnostic| {
            panic!("valid nested qualifier `{expression}` failed: {diagnostic}")
        });
    }
}

#[test]
fn graph_v14_is_program_wide_and_context_authenticates_exact_instances() {
    let program = parse_source(SOURCE);
    let json = graph::to_json(&program).unwrap();
    let module_value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(json.starts_with("{\"schema\":\"semaprax.graph.v14\","));
    assert!(json.contains("\"kind\":\"function_template\""));
    assert!(json.contains("\"kind\":\"function_instance\""));
    assert!(json.contains("\"kind\":\"call_instance\""));
    assert!(json.contains("\"template\":\"test.first\""));
    assert!(json.contains(
        "\"type_arguments\":[{\"kind\":\"primitive\",\"name\":\"i64\"},{\"kind\":\"primitive\",\"name\":\"bool\"}]"
    ));
    let first_template = module_value["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "test.first")
        .unwrap();
    assert_eq!(
        first_template["type_parameters"].as_array().unwrap().len(),
        2
    );

    let options = AgentContextOptions::new(
        1,
        64 * 1024,
        32,
        [AgentContextFilter::Contracts, AgentContextFilter::Types],
    )
    .unwrap();
    let caller = graph::agent_context_json(&program, "app.main", &options)
        .unwrap()
        .unwrap();
    serde_json::from_str::<serde_json::Value>(&caller).unwrap();
    assert!(caller.contains("\"source_graph_schema\":\"semaprax.graph.v14\""));
    assert!(caller.contains("\"kind\":\"call_instance\""));
    let call_instances = caller
        .split_once("\"call_instances\":[")
        .unwrap()
        .1
        .split_once("],\"body\"")
        .unwrap()
        .0;
    assert_eq!(call_instances.matches("\"expression\":").count(), 3);
    let resolved = hir::resolve(&program).unwrap();
    for instance in &resolved.function_instances {
        assert!(call_instances.contains(&format!(
            "\"template\":{},\"instance\":{}",
            semaprax::diagnostic::quote_json(instance.template.as_str()),
            semaprax::diagnostic::quote_json(instance.id.as_str())
        )));
    }
    let template = graph::agent_context_json(&program, "test.first", &options)
        .unwrap()
        .unwrap();
    let template_value: serde_json::Value = serde_json::from_str(&template).unwrap();
    assert!(template.contains("\"kind\":\"function_template\""));
    assert!(template.contains("\"instances\":["));
    assert_eq!(
        template_value["facts"][0]["type_parameters"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let unused = r#"
module test.unused_generic_graph;
@id("test.unused") fn unused<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let unused_program = parse_source(unused);
    let unused_json = graph::to_json(&unused_program).unwrap();
    assert!(unused_json.starts_with("{\"schema\":\"semaprax.graph.v14\","));
    assert!(unused_json.contains("\"kind\":\"function_template\""));
    assert!(!unused_json.contains("\"kind\":\"function_instance\""));

    let full_template_context = graph::context_json(&program, "test.first", 0)
        .unwrap()
        .unwrap();
    assert!(full_template_context.contains("\"root\":\"test.first\""));
    serde_json::from_str::<serde_json::Value>(&full_template_context).unwrap();

    let bounded_caller_context = graph::context_json(&program, "app.main", 1)
        .unwrap()
        .unwrap();
    serde_json::from_str::<serde_json::Value>(&bounded_caller_context).unwrap();

    let automatic = parse_source(
        r#"
module test.automatic_generic;
fn automatic<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { automatic<i64>(0) }
"#,
    );
    let automatic_json = graph::to_json(&automatic).unwrap();
    assert!(automatic_json.contains(
        "\"kind\":\"function_template\",\"name\":\"automatic\",\"identity_origin\":\"automatic\",\"persistent\":false"
    ));

    let frontier = parse_source(
        r#"
module test.generic_context_frontier;
@id("test.helper") fn helper() -> bool { true }
@id("test.template") fn template<T>(value: T) -> T {
    if helper() { value } else { value }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let frontier_json = graph::context_json(&frontier, "test.template", 0)
        .unwrap()
        .unwrap();
    assert!(frontier_json.contains(
        "\"view\":{\"kind\":\"context\",\"root\":\"test.template\",\"depth\":0,\"truncated\":true,\"frontier\":[\"test.helper\"]}"
    ));
}

#[test]
fn graph_v14_has_literal_module_and_context_kats_and_wins_the_schema_lattice() {
    let program = parse_source(SOURCE);
    let module = graph::to_json(&program).unwrap();
    let options = AgentContextOptions::new(
        1,
        64 * 1024,
        32,
        [AgentContextFilter::Contracts, AgentContextFilter::Types],
    )
    .unwrap();
    let agent = graph::agent_context_json(&program, "app.main", &options)
        .unwrap()
        .unwrap();
    let context = graph::context_json(&program, "test.first", 1)
        .unwrap()
        .unwrap();
    assert_eq!(
        sha256_text(&module),
        "7a61fa6229f2db7aca6a035fd961720e8a401c138cc66c9cd71c64d45bed5efd"
    );
    assert_eq!(
        sha256_text(&agent),
        "2841401e7ba85fa8e47b3c35a15ae401b4a271d2500d70bbf3627f1453869eb6"
    );
    assert_eq!(
        sha256_text(&context),
        "d7bda2be1fc366195ffb00a9e20b2b03204b4dd6f46e8019842dd84f70b54ab8"
    );

    let mixed = parse_source(
        r#"
module test.generic_function_lattice;
@id("test.box") record Box<T> { @id("test.box.value") value: T, }
@id("test.pair") record Pair { @id("test.pair.value") value: i64, }
@id("test.pattern") fn pattern(input: Pair) -> i64 {
    match input { Pair { value } => value, }
}
@id("test.option") fn option(value: Option<i64>) -> Option<bool> {
    let number = value?;
    Option<bool>::Some { value: number > 0 }
}
@id("test.generic") fn generic<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { 0 }
"#,
    );
    let mixed_json = graph::to_json(&mixed).unwrap();
    assert!(mixed_json.starts_with("{\"schema\":\"semaprax.graph.v14\","));
    assert!(mixed_json.contains("\"kind\":\"record_pattern\""));
    assert!(mixed_json.contains("\"kind\":\"try_option\""));
    assert!(mixed_json.contains("\"kind\":\"function_template\""));
    let legacy_root = graph::agent_context_json(&mixed, "app.main", &options)
        .unwrap()
        .unwrap();
    assert!(legacy_root.contains("\"source_graph_schema\":\"semaprax.graph.v14\""));
}

#[test]
fn hir_rejects_template_instance_and_reachability_confusion() {
    let resolved = hir::resolve(&parse_source(SOURCE)).unwrap();

    let mut wrong_argument = resolved.clone();
    wrong_argument.function_instances[0].type_arguments[0] = ResolvedType::Bool;
    assert_eq!(hir::validate(&wrong_argument).unwrap_err().code, "SPX-H006");

    let mut wrong_body = resolved.clone();
    wrong_body.function_instances[0].function.body =
        wrong_body.function_instances[1].function.body.clone();
    assert_eq!(hir::validate(&wrong_body).unwrap_err().code, "SPX-H006");

    let mut wrong_template = resolved.clone();
    wrong_template.function_templates[0].body = wrong_template.function_templates[1].body.clone();
    assert_eq!(hir::validate(&wrong_template).unwrap_err().code, "SPX-H006");

    let mut missing = resolved.clone();
    missing.function_instances.pop();
    assert_eq!(hir::validate(&missing).unwrap_err().code, "SPX-H006");

    let mut duplicate = resolved.clone();
    duplicate
        .function_instances
        .push(duplicate.function_instances[0].clone());
    assert_eq!(hir::validate(&duplicate).unwrap_err().code, "SPX-H006");

    let mut reordered = resolved.clone();
    reordered.function_instances.swap(0, 1);
    assert_eq!(hir::validate(&reordered).unwrap_err().code, "SPX-H006");

    let mut wrong_parameter_index = resolved.clone();
    wrong_parameter_index.function_templates[0].type_parameters[0].index = 1;
    assert_eq!(
        hir::validate(&wrong_parameter_index).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_parameter_name = resolved.clone();
    wrong_parameter_name.function_templates[0].type_parameters[0].name = "Renamed".to_owned();
    assert_eq!(
        hir::validate(&wrong_parameter_name).unwrap_err().code,
        "SPX-H006"
    );

    let mut duplicate_parameter_name = resolved.clone();
    duplicate_parameter_name.function_templates[1].type_parameters[1].name =
        duplicate_parameter_name.function_templates[1].type_parameters[0]
            .name
            .clone();
    assert_eq!(
        hir::validate(&duplicate_parameter_name).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_parameter_owner = resolved.clone();
    wrong_parameter_owner.function_templates[0].params[0].ty = ResolvedType::TypeParameter {
        owner: DeclarationId::new("test.first"),
        index: 0,
    };
    assert_eq!(
        hir::validate(&wrong_parameter_owner).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_result = resolved.clone();
    wrong_result.function_instances[0].function.return_type = ResolvedType::Bool;
    assert_eq!(hir::validate(&wrong_result).unwrap_err().code, "SPX-H006");

    let mut wrong_contract = resolved.clone();
    wrong_contract.function_instances[0].function.requires = wrong_contract.function_instances[1]
        .function
        .requires
        .clone();
    assert_eq!(hir::validate(&wrong_contract).unwrap_err().code, "SPX-H006");

    let mut wrong_call_instance = resolved.clone();
    let replacement = wrong_call_instance.function_instances[1].id.clone();
    let ResolvedExprKind::Block { statements, .. } =
        &mut wrong_call_instance.functions[0].body.kind
    else {
        panic!("main must be a block");
    };
    let semaprax::hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("first statement must be a let")
    };
    let ResolvedExprKind::Call { instance, .. } = &mut value.kind else {
        panic!("first statement must be a generic call");
    };
    *instance = Some(replacement);
    assert_eq!(
        hir::validate(&wrong_call_instance).unwrap_err().code,
        "SPX-H006"
    );

    let mut cleanup_tamper = resolved;
    cleanup_tamper.function_instances[0]
        .function
        .cleanup_plan
        .schema = "semaprax.cleanup-plan.v3";
    assert_eq!(hir::validate(&cleanup_tamper).unwrap_err().code, "SPX-H006");

    let same_signature = parse_source(
        r#"
module test.same_signature_instances;
@id("test.preserve") fn preserve<T>(marker: bool) -> bool { marker }
@id("app.main") fn main() -> i64 {
    let first = preserve<i64>(true);
    if preserve<bool>(first) { 0 } else { 1 }
}

"#,
    );
    let same_json = graph::to_json(&same_signature).unwrap();
    let mut same_resolved = hir::resolve(&same_signature).unwrap();
    assert_eq!(same_resolved.function_instances.len(), 2);
    for instance in &same_resolved.function_instances {
        assert!(same_json.contains(instance.id.as_str()));
    }
    let replacement = same_resolved.function_instances[1].id.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut same_resolved.functions[0].body.kind
    else {
        panic!("main must be a block");
    };
    let semaprax::hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("first statement must be a let")
    };
    let ResolvedExprKind::Call { instance, .. } = &mut value.kind else {
        panic!("first statement must call preserve<i64>");
    };
    *instance = Some(replacement);
    assert_eq!(hir::validate(&same_resolved).unwrap_err().code, "SPX-H006");
}

#[test]
fn generic_function_hostiles_are_stable_and_fail_closed() {
    let inference = r#"
module test.generic_inference;
@id("test.id") fn id<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { id(1) }
"#;
    assert!(error_codes(inference).contains(&"SPX-T225"));

    let indirect_cycle = r#"
module test.generic_cycle;
@id("test.generic") fn generic<T>(value: T) -> T { helper(value) }
@id("test.helper") fn helper(value: i64) -> i64 { generic<i64>(value) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(error_codes(indirect_cycle).contains(&"SPX-T226"));

    let transitive = r#"
module test.generic_transitive_call;
@id("test.second") fn second<T>(value: T) -> T { value }
@id("test.bridge") fn bridge(value: i64) -> i64 { second<i64>(value) }
@id("test.first") fn first<T>(value: T) -> T {
    let observed = bridge(0);
    if observed == 0 { value } else { value }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    assert!(error_codes(transitive).contains(&"SPX-T226"));

    let reserved = r#"
module test.reserved_execution_identity;
@id("semaprax.function-execution.v1:generic:1:x") fn spoof() -> i64 { 0 }
@id("app.main") fn main() -> i64 { spoof() }
"#;
    assert!(error_codes(reserved).contains(&"SPX-T225"));
}

#[test]
fn generic_function_source_boundaries_are_closed() {
    assert!(parse(
        "module test.zero; @id(\"test.bad\") fn bad<>(value: i64) -> i64 { value } @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("generic-zero.spx")
    )
    .is_err());

    let cases = [
        (
            r#"module test.duplicate;
@id("test.bad") fn bad<T, T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { 0 }"#,
            "SPX-T224",
        ),
        (
            r#"module test.mono_args;
@id("test.mono") fn mono(value: i64) -> i64 { value }
@id("app.main") fn main() -> i64 { mono<i64>(0) }"#,
            "SPX-T225",
        ),
        (
            r#"module test.nominal_args;
@id("test.box") record Box { @id("test.box.value") value: i64, }
@id("test.id") fn id<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { let value = Box { value: 0 }; id<Box>(value).value }"#,
            "SPX-T225",
        ),
        (
            r#"module test.owned_generic;
@id("test.token") resource Token { @id("test.token.drop") drop trivial; }
@id("test.bad") fn bad<T>(value: own T) -> T { value }
@id("app.main") fn main() -> i64 { 0 }"#,
            "SPX-T224",
        ),
        (
            r#"module test.effect_generic;
permit { clock.read }
@id("test.bad") fn bad<T>(value: T) -> T uses { clock.read } { value }
@id("app.main") fn main() -> i64 { 0 }"#,
            "SPX-T226",
        ),
        (
            r#"module test.direct_generic_call;
@id("test.b") fn b<T>(value: T) -> T { value }
@id("test.a") fn a<T>(value: T) -> T { let observed = b<i64>(0); if observed == 0 { value } else { value } }
@id("app.main") fn main() -> i64 { 0 }"#,
            "SPX-T226",
        ),
        (
            r#"module test.direct_recursion;
@id("test.loop") fn loop<T>(value: T) -> T { let observed = loop<i64>(0); if observed == 0 { value } else { value } }
@id("app.main") fn main() -> i64 { 0 }"#,
            "SPX-T226",
        ),
        (
            r#"module test.generic_entry;
@id("app.main") fn main<T>() -> i64 { 0 }"#,
            "SPX-T104",
        ),
    ];
    for (source, code) in cases {
        assert!(error_codes(source).contains(&code), "missing {code}");
    }
}

#[test]
fn generic_functions_do_not_widen_callable_trace_or_resource_boundaries() {
    let program = parse_source(SOURCE);
    let resolved = hir::resolve(&program).unwrap();
    let main = DeclarationId::new("app.main");

    assert!(codegen::preflight_native_callable_bundle(&program, "app.main").is_err());
    assert!(codegen::emit_native_adapter_admission(&resolved, &main, "generic.h").is_err());
    assert!(semantic_trace::build_semantic_event_dictionary(&resolved, &main).is_err());
    #[cfg(feature = "unstable-native-host-internal")]
    {
        assert!(codegen::emit_native_callable_admission(&resolved, &main).is_err());
        assert!(codegen::emit_native_callable_settlement_proof(&resolved, &main).is_err());
        assert!(codegen::emit_native_callable_v3_descriptor(&resolved, &main).is_err());

        let legacy = hir::resolve(&parse_source(
            r#"module test.legacy_trace;
@id("app.main") fn main() -> i64 { 0 }"#,
        ))
        .unwrap();
        let dictionary = semantic_trace::build_semantic_event_dictionary(&legacy, &main).unwrap();
        assert!(
            semaprax::trace_path_certificate::build_trace_path_certificate(
                &resolved,
                &resolved.functions[0],
                &dictionary,
            )
            .is_err()
        );
    }

    let mixed = parse_source(
        r#"
module test.generic_resource_closed;
@id("test.token") resource Token { @id("test.token.drop") drop trivial; }
@id("test.id") fn id<T>(value: T) -> T { value }
@id("app.main") fn main() -> i64 { id<i64>(0) }
"#,
    );
    assert!(codegen::emit_c(&mixed).is_err());
    assert!(wasm::emit_module(&mixed).is_err());
}
