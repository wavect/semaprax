use super::*;

const MULTI_OWNER_SOURCE: &str = r#"
module fixture.multi_owner;

@id("fixture.multi_owner.Input")
record Input {
    @id("fixture.multi_owner.Input.left")
    left: Bytes,
    @id("fixture.multi_owner.Input.right")
    right: Bytes,
}

@id("fixture.multi_owner.Output")
record Output {
    @id("fixture.multi_owner.Output.first")
    first: Bytes,
    @id("fixture.multi_owner.Output.second")
    second: Bytes,
}

@id("fixture.multi_owner.transform")
fn transform(input: borrow Input, owned: own Bytes) -> Output
{
    let owned_view = bytes_as_slice(owned);
    let copied = bytes_copy(owned_view);
    Output {
        first: copied,
        second: owned,
    }
}

@id("fixture.multi_owner.main")
fn main() -> i64
{
    0
}
"#;

// This fixture deliberately keeps the two physical owners in the active
// `Choice::Data` arm. The native wrapper cannot marshal variant *arguments*,
// but the generated function constructs and returns this admitted active case
// itself, so the probe observes its selected-case result cleanup rather than
// a hand-written C approximation.
const VARIANT_ARM_MULTI_OWNER_SOURCE: &str = r#"
module fixture.variant_arm_multi_owner;

@id("fixture.variant_arm_multi_owner.Choice")
variant Choice {
    @id("fixture.variant_arm_multi_owner.Choice.empty")
    Empty,
    @id("fixture.variant_arm_multi_owner.Choice.data")
    Data {
        @id("fixture.variant_arm_multi_owner.Choice.data.first")
        first: Bytes,
        @id("fixture.variant_arm_multi_owner.Choice.data.second")
        second: Bytes,
    },
}

@id("fixture.variant_arm_multi_owner.transform")
fn transform(first: own Bytes, second: own Bytes) -> Choice
{
    Choice::Data { first: first, second: second }
}

@id("fixture.variant_arm_multi_owner.empty")
fn empty() -> Choice
{
    Choice::Empty {}
}

@id("fixture.variant_arm_multi_owner.main")
fn main() -> i64
{
    0
}
"#;

fn settlement(owner: &str, counter: &str) -> String {
    format!(
        "    spx_bytes_drop(&({owner}));\n    if (({owner}).ptr != NULL || ({owner}).len != 0) return 91;\n    ++{counter};\n"
    )
}

fn record(record: &str, fields: &[(&str, RetainedValue)]) -> RetainedValue {
    RetainedValue::Record(RetainedRecord {
        record: DeclarationId::new(record),
        fields: fields
            .iter()
            .map(|(name, value)| RetainedField {
                field: DeclarationId::new(format!("{record}.{name}")),
                value: value.clone(),
            })
            .collect(),
    })
}

fn expected_result() -> RetainedValue {
    record(
        "fixture.multi_owner.Output",
        &[
            ("first", RetainedValue::Bytes(b"owned".to_vec())),
            ("second", RetainedValue::Bytes(b"owned".to_vec())),
        ],
    )
}

#[test]
fn native_multi_owner_cleanup_order_is_physical_and_hostile_mutants_reject() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("native cleanup evidence requires held clang");
    let program = hir::resolve(
        &crate::check(MULTI_OWNER_SOURCE, Path::new("multi-owner.spx"))
            .expect("multi-owner fixture checks"),
    )
    .expect("multi-owner fixture resolves");
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "fixture.multi_owner.transform")
        .expect("transform entry exists");
    let arguments = [
        record(
            "fixture.multi_owner.Input",
            &[
                ("left", RetainedValue::Bytes(b"left".to_vec())),
                ("right", RetainedValue::Bytes(b"right!".to_vec())),
            ],
        ),
        RetainedValue::Bytes(b"owned".to_vec()),
    ];
    let (base_body, borrowed_count) = render_driver(&program, entry, &arguments).unwrap();
    assert_eq!(borrowed_count, 2);

    let left_arg = format!(
        "spx_native_exec_arg_1.{}",
        field_symbol(&DeclarationId::new("fixture.multi_owner.Input.left"))
    );
    let right_arg = format!(
        "spx_native_exec_arg_1.{}",
        field_symbol(&DeclarationId::new("fixture.multi_owner.Input.right"))
    );
    let output_first = format!(
        "(spx_native_exec_result).{}",
        field_symbol(&DeclarationId::new("fixture.multi_owner.Output.first"))
    );
    let output_second = format!(
        "(spx_native_exec_result).{}",
        field_symbol(&DeclarationId::new("fixture.multi_owner.Output.second"))
    );

    let mut argument_emitter = Emitter::new();
    argument_emitter.bytes_expr(b"left");
    argument_emitter.bytes_expr(b"right!");
    let original_owned_expression = argument_emitter.bytes_expr(b"owned");
    assert_eq!(base_body.matches(&original_owned_expression).count(), 1);
    let captured_owned_expression =
        format!("(test_original_owned = {original_owned_expression}, test_original_owned)");
    let mut body = base_body.replacen(&original_owned_expression, &captured_owned_expression, 1);
    body.insert_str(0, "    spx_bytes_v1 test_original_owned = {0};\n");

    let call_prefix = format!(
        "    spx_status_token spx_native_exec_token = {}(",
        function_symbol(&entry.id)
    );
    let call_start = body.find(&call_prefix).expect("entry call rendered");
    let call_end = body[call_start..]
        .find('\n')
        .map(|offset| call_start + offset + 1)
        .expect("entry call line ends");
    let identity_probe = format!(
        "    if ((void *)({output_second}).ptr != (void *)test_original_owned.ptr || (void *)({output_first}).ptr == (void *)test_original_owned.ptr) return 94;\n    test_expected[0] = (void *)({left_arg}).ptr;\n    test_expected[1] = (void *)({right_arg}).ptr;\n    test_expected[2] = (void *)test_original_owned.ptr;\n    test_expected[3] = (void *)({output_first}).ptr;\n"
    );
    body.insert_str(call_end, &identity_probe);

    let left_drop = settlement(&left_arg, "spx_borrowed_settled");
    let right_drop = settlement(&right_arg, "spx_borrowed_settled");
    assert_eq!(body.matches(&left_drop).count(), 1);
    assert_eq!(body.matches(&right_drop).count(), 1);
    let right_then_left = body.find(&right_drop).expect("borrowed cleanup is emitted")
        < body.find(&left_drop).expect("borrowed cleanup is emitted");
    assert!(
        right_then_left,
        "borrowed fields settle in reverse field order"
    );

    let output_second_drop = settlement(&output_second, "spx_result_settled");
    assert_eq!(body.matches(&output_second_drop).count(), 1);
    let terminal_assertions = "    if (test_allocs != 4 || test_frees != 4) return 92;\n    if (test_free_order[0] != 1 || test_free_order[1] != 0 || test_free_order[2] != 2 || test_free_order[3] != 3) return 93;\n";
    assert_eq!(body.matches("    return 0;\n").count(), 1);
    body = body.replace(
        "    return 0;\n",
        &format!("{terminal_assertions}    return 0;\n"),
    );

    let generated_body = crate::codegen::emit_hir_c(&program).unwrap();
    let generated = format!(
        r#"#include <stdlib.h>
static void *test_live[32];
static void *test_expected[4];
static unsigned test_allocs, test_frees, test_free_order[32];
static void *test_malloc(size_t n) {{
    void *p = malloc(n); if (!p || test_allocs == 32) abort();
    test_live[test_allocs++] = p; return p;
}}
static void test_free(void *p) {{
    if (!p) return;
    for (unsigned i=0; i<4; ++i) if (test_expected[i] == p) {{
        for (unsigned j=0; j<test_allocs; ++j) if (test_live[j] == p) {{
            test_live[j] = NULL; test_free_order[test_frees++] = i; free(p); return;
        }}
        abort();
    }}
    abort();
}}
#define malloc test_malloc
#define free test_free
{generated_body}"#
    );

    for optimization in ["-O0", "-O2"] {
        for mutation in ["none", "wrong-order", "extra-own-drop", "omit-result-drop"] {
            let mut selected_body = body.clone();
            match mutation {
                "wrong-order" => {
                    selected_body = selected_body
                        .replace(&right_drop, "__RIGHT_BORROW_DROP__")
                        .replace(&left_drop, &right_drop)
                        .replace("__RIGHT_BORROW_DROP__", &left_drop);
                }
                "extra-own-drop" => {
                    selected_body = selected_body.replace(
                        &output_second_drop,
                        &format!("    test_free(test_expected[2]);\n{output_second_drop}"),
                    );
                }
                "omit-result-drop" => {
                    selected_body = selected_body.replace(
                        &output_second_drop,
                        &format!(
                            "    ({output_second}).ptr = NULL; ({output_second}).len = 0;\n    ++spx_result_settled;\n"
                        ),
                    );
                }
                _ => {}
            }
            let root = ProbeDirectory::create().unwrap();
            let result =
                compile_and_run(&generated, &selected_body, &root, &host, optimization, None);
            root.cleanup();
            if mutation == "none" {
                let stdout = result.unwrap();
                let declaration = nominal_declaration(&entry.return_type).unwrap();
                let evaluation =
                    decode(entry.id.clone(), declaration, &stdout, 1000, borrowed_count).unwrap();
                assert_eq!(
                    evaluation.outcome,
                    RetainedCallOutcome::Returned(expected_result())
                );
                assert_eq!(
                    evaluation.cleanup_events,
                    [OwnedDataCleanupEvent::CopyOutAndSettleBytes; 2]
                );
            } else {
                let error = result.expect_err("physical cleanup-order mutant must reject");
                assert!(
                    error.message.contains("native_executor.run"),
                    "{mutation} at {optimization} must execute and trip the physical probe: {error:?}"
                );
            }
        }
    }
}

#[test]
fn native_active_variant_arm_multi_owner_cleanup_is_physical_and_hostile_mutants_reject() {
    if !crate::agent_lifecycle::tests::stage_process_host_supported() {
        return;
    }
    let host = crate::agent_lifecycle::tests::native_stage_host()
        .expect("native cleanup evidence requires held clang");
    let program = hir::resolve(
        &crate::check(
            VARIANT_ARM_MULTI_OWNER_SOURCE,
            Path::new("variant-arm-multi-owner.spx"),
        )
        .expect("active variant-arm fixture checks"),
    )
    .expect("active variant-arm fixture resolves");
    let entry = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "fixture.variant_arm_multi_owner.transform")
        .expect("variant-arm transform entry exists");
    let arguments = [
        RetainedValue::Bytes(b"first-owner".to_vec()),
        RetainedValue::Bytes(b"second-owner".to_vec()),
    ];
    let (base_body, borrowed_count) = render_driver(&program, entry, &arguments).unwrap();
    assert_eq!(borrowed_count, 0);

    let active_case = case_symbol(&DeclarationId::new(
        "fixture.variant_arm_multi_owner.Choice.data",
    ));
    let output_first = format!(
        "(spx_native_exec_result).spx_payload.{active_case}.{}",
        field_symbol(&DeclarationId::new(
            "fixture.variant_arm_multi_owner.Choice.data.first"
        ))
    );
    let output_second = format!(
        "(spx_native_exec_result).spx_payload.{active_case}.{}",
        field_symbol(&DeclarationId::new(
            "fixture.variant_arm_multi_owner.Choice.data.second"
        ))
    );
    let mut argument_emitter = Emitter::new();
    let first_expression = argument_emitter.bytes_expr(b"first-owner");
    let second_expression = argument_emitter.bytes_expr(b"second-owner");
    assert_eq!(base_body.matches(&first_expression).count(), 1);
    assert_eq!(base_body.matches(&second_expression).count(), 1);
    let mut body = base_body.replacen(
        &first_expression,
        &format!("(test_original_first = {first_expression}, test_original_first)"),
        1,
    );
    body = body.replacen(
        &second_expression,
        &format!("(test_original_second = {second_expression}, test_original_second)"),
        1,
    );
    body.insert_str(
        0,
        "    spx_bytes_v1 test_original_first = {0};\n    spx_bytes_v1 test_original_second = {0};\n",
    );

    let call_prefix = format!(
        "    spx_status_token spx_native_exec_token = {}(",
        function_symbol(&entry.id)
    );
    let call_start = body.find(&call_prefix).expect("entry call rendered");
    let call_end = body[call_start..]
        .find('\n')
        .map(|offset| call_start + offset + 1)
        .expect("entry call line ends");
    let identity_probe = format!(
        "    if ((void *)({output_first}).ptr != (void *)test_original_first.ptr || (void *)({output_second}).ptr != (void *)test_original_second.ptr) return 94;\n    test_expected[0] = (void *)test_original_first.ptr;\n    test_expected[1] = (void *)test_original_second.ptr;\n"
    );
    body.insert_str(call_end, &identity_probe);

    let output_first_drop = settlement(&output_first, "spx_result_settled");
    let output_second_drop = settlement(&output_second, "spx_result_settled");
    assert_eq!(body.matches(&output_first_drop).count(), 1);
    assert_eq!(body.matches(&output_second_drop).count(), 1);
    assert!(
        body.find(&output_second_drop)
            .expect("second result cleanup is emitted")
            < body
                .find(&output_first_drop)
                .expect("first result cleanup is emitted"),
        "the active variant arm's two transferred owners settle in reverse result-field order"
    );
    let terminal_assertions = "    if (test_allocs != 2 || test_frees != 2) return 92;\n    if (test_free_order[0] != 1 || test_free_order[1] != 0) return 93;\n";
    assert_eq!(body.matches("    return 0;\n").count(), 1);
    body = body.replace(
        "    return 0;\n",
        &format!("{terminal_assertions}    return 0;\n"),
    );

    let generated_body = crate::codegen::emit_hir_c(&program).unwrap();
    let generated = format!(
        r#"#include <stdlib.h>
static void *test_live[32];
static void *test_expected[2];
static unsigned test_allocs, test_frees, test_free_order[32];
static void *test_malloc(size_t n) {{
    void *p = malloc(n); if (!p || test_allocs == 32) abort();
    test_live[test_allocs++] = p; return p;
}}
static void test_free(void *p) {{
    if (!p) return;
    for (unsigned i=0; i<2; ++i) if (test_expected[i] == p) {{
        for (unsigned j=0; j<test_allocs; ++j) if (test_live[j] == p) {{
            test_live[j] = NULL; test_free_order[test_frees++] = i; free(p); return;
        }}
        abort();
    }}
    abort();
}}
#define malloc test_malloc
#define free test_free
{generated_body}"#
    );

    for optimization in ["-O0", "-O2"] {
        for mutation in ["none", "wrong-order", "extra-free", "omitted-free"] {
            let mut selected_body = body.clone();
            match mutation {
                "wrong-order" => {
                    selected_body = selected_body
                        .replace(&output_second_drop, "__SECOND_RESULT_DROP__")
                        .replace(&output_first_drop, &output_second_drop)
                        .replace("__SECOND_RESULT_DROP__", &output_first_drop);
                }
                "extra-free" => {
                    selected_body = selected_body.replace(
                        &output_second_drop,
                        &format!("    test_free(test_expected[1]);\n{output_second_drop}"),
                    );
                }
                "omitted-free" => {
                    selected_body = selected_body.replace(
                        &output_second_drop,
                        &format!(
                            "    ({output_second}).ptr = NULL; ({output_second}).len = 0;\n    ++spx_result_settled;\n"
                        ),
                    );
                }
                _ => {}
            }
            let root = ProbeDirectory::create().unwrap();
            let result =
                compile_and_run(&generated, &selected_body, &root, &host, optimization, None);
            root.cleanup();
            if mutation == "none" {
                let stdout = result.unwrap();
                let declaration = nominal_declaration(&entry.return_type).unwrap();
                let evaluation =
                    decode(entry.id.clone(), declaration, &stdout, 1000, borrowed_count).unwrap();
                assert_eq!(
                    evaluation.outcome,
                    RetainedCallOutcome::Returned(RetainedValue::Variant(RetainedVariant {
                        variant: DeclarationId::new("fixture.variant_arm_multi_owner.Choice"),
                        case: DeclarationId::new("fixture.variant_arm_multi_owner.Choice.data"),
                        fields: vec![
                            RetainedField {
                                field: DeclarationId::new(
                                    "fixture.variant_arm_multi_owner.Choice.data.first"
                                ),
                                value: RetainedValue::Bytes(b"first-owner".to_vec()),
                            },
                            RetainedField {
                                field: DeclarationId::new(
                                    "fixture.variant_arm_multi_owner.Choice.data.second"
                                ),
                                value: RetainedValue::Bytes(b"second-owner".to_vec()),
                            },
                        ],
                    }))
                );
                assert_eq!(
                    evaluation.cleanup_events,
                    [OwnedDataCleanupEvent::CopyOutAndSettleBytes; 2]
                );
            } else {
                let error = result.expect_err("physical active-variant cleanup mutant must reject");
                assert!(
                    error.message.contains("native_executor.run"),
                    "{mutation} at {optimization} must execute and trip the physical probe: {error:?}"
                );
            }
        }

        let empty_entry = program
            .functions
            .iter()
            .find(|function| function.id.as_str() == "fixture.variant_arm_multi_owner.empty")
            .expect("empty variant entry exists");
        let (mut empty_body, empty_borrowed_count) =
            render_driver(&program, empty_entry, &[]).expect("empty variant result renders");
        assert_eq!(empty_borrowed_count, 0);
        assert_eq!(empty_body.matches("    return 0;\n").count(), 1);
        empty_body = empty_body.replace(
            "    return 0;\n",
            "    if (test_allocs != 0 || test_frees != 0) return 95;\n    return 0;\n",
        );
        let root = ProbeDirectory::create().unwrap();
        let stdout = compile_and_run(&generated, &empty_body, &root, &host, optimization, None)
            .expect("payload-free variant result must not allocate or settle Bytes");
        root.cleanup();
        let declaration = nominal_declaration(&empty_entry.return_type).unwrap();
        let evaluation = decode(
            empty_entry.id.clone(),
            declaration,
            &stdout,
            1000,
            empty_borrowed_count,
        )
        .expect("payload-free variant result receipt decodes");
        assert_eq!(
            evaluation.outcome,
            RetainedCallOutcome::Returned(RetainedValue::Variant(RetainedVariant {
                variant: DeclarationId::new("fixture.variant_arm_multi_owner.Choice"),
                case: DeclarationId::new("fixture.variant_arm_multi_owner.Choice.empty"),
                fields: Vec::new(),
            }))
        );
        assert!(evaluation.cleanup_events.is_empty());
    }
}
