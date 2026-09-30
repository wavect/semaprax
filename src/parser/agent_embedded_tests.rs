use crate::ast::AgentOperationKind;

pub(crate) fn source() -> String {
    let mut source =
        String::from("module agent.embedded;\n@id(\"agent\") agent Example { types {\n");
    for role in [
        "task",
        "state",
        "observation",
        "proposal",
        "outcome",
        "result",
    ] {
        source.push_str(&format!("@id(\"type.{role}\") type {role};\n"));
    }
    source.push_str("} operations {\n");
    for role in [
        "initialize",
        "observe",
        "propose",
        "authorize",
        "execute",
        "reduce",
    ] {
        source.push_str(&format!("@id(\"op.{role}\") "));
        match role {
            "propose" => source.push_str("model fn propose;\n"),
            "execute" => source.push_str("effect fn execute;\n"),
            _ => source.push_str(&format!("fn {role}(value:i64)->i64 {{ value + 1 }}\n")),
        }
    }
    source.push_str(
        "} runtime_v1 { canonical_json \"{}\"; } }\n@id(\"main\") fn main()->i64 {initialize(1)}\n",
    );
    source
}

#[test]
fn embedded_operations_have_one_ordinary_body_and_canonical_origin() {
    let parsed = crate::parse(&source(), "embedded.spx").unwrap();
    assert_eq!(parsed.functions.len(), 5);
    assert_eq!(
        parsed.agents[0]
            .operations
            .iter()
            .filter(|op| op.embedded_function_index.is_some())
            .count(),
        4
    );
    parsed.agents[0]
        .validate_execution_metadata(&parsed)
        .unwrap();
    let canonical = crate::format::canonical(&parsed);
    let repeated = crate::parse(&canonical, "embedded.spx").unwrap();
    assert_eq!(crate::format::canonical(&repeated), canonical);
    repeated.agents[0]
        .validate_execution_metadata(&repeated)
        .unwrap();
    for operation in &repeated.agents[0].operations {
        if operation.kind == AgentOperationKind::Deterministic {
            assert_eq!(
                repeated.agents[0]
                    .embedded_function(operation, &repeated.functions)
                    .unwrap()
                    .stable_id,
                operation.stable_id
            );
        }
    }
}

#[test]
fn raw_duplicate_top_level_identity_survives_formatting_before_s102() {
    let text = format!(
        "{}\n@id(\"op.initialize\") fn separate()->i64 {{ 0 }}",
        source()
    );
    let parsed = crate::parse(&text, "duplicate.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(canonical.matches("@id(\"op.initialize\")").count(), 2);
    let repeated = crate::parse(&canonical, "duplicate.spx").unwrap();
    assert_eq!(
        repeated
            .functions
            .iter()
            .filter(|f| f.stable_id == "op.initialize")
            .count(),
        2
    );
    assert!(crate::hir::resolve(&repeated)
        .unwrap_err()
        .iter()
        .any(|error| error.code == "SPX-S102"));
}

#[test]
fn model_and_effect_bodies_and_unknown_wait_versions_stay_p124() {
    for text in [
        source().replace(
            "model fn propose;",
            "model fn propose(value:i64)->i64 {value}",
        ),
        source().replace(
            "effect fn execute;",
            "effect fn execute(value:i64)->i64 {value}",
        ),
        source().replace(
            "runtime_v1",
            "model_wait_v2 { propose = \"helper\"; } runtime_v1",
        ),
    ] {
        assert_eq!(
            crate::parse(&text, "refused.spx").unwrap_err().code,
            "SPX-P124"
        );
    }
}

#[test]
fn explicit_wait_helper_identity_has_an_exact_byte_boundary() {
    for (length, admitted) in [(240, true), (241, false)] {
        let text = source().replace(
            "runtime_v1",
            &format!(
                "model_wait_v1 {{ propose = \"{}\"; }} runtime_v1",
                "a".repeat(length)
            ),
        );
        match crate::parse(&text, "bound.spx") {
            Ok(program) => {
                assert!(admitted);
                assert_eq!(
                    program.agents[0]
                        .model_wait
                        .as_ref()
                        .unwrap()
                        .helper_id
                        .len(),
                    240
                );
            }
            Err(error) => {
                assert!(!admitted);
                assert_eq!(error.code, "SPX-P124");
            }
        }
    }
}

#[test]
fn stale_or_forged_origin_cannot_hide_an_unrelated_function() {
    let mut program = crate::parse(&source(), "origin.spx").unwrap();
    program.agents[0].operations[0].embedded_function_index = Some(program.functions.len() - 1);
    assert!(program.agents[0]
        .embedded_function(&program.agents[0].operations[0], &program.functions)
        .is_none());
    let canonical = crate::format::canonical(&program);
    assert!(canonical.contains("fn main("));
    assert_eq!(
        crate::hir::resolve(&program).unwrap_err()[0].code,
        "SPX-G559"
    );
}

#[test]
fn embedded_functions_keep_the_ordinary_module_namespace() {
    let text = format!(
        "{}\n@id(\"another.observe\") fn observe()->i64 {{0}}",
        source()
    );
    let parsed = crate::parse(&text, "namespace.spx").unwrap();
    assert!(crate::hir::resolve(&parsed)
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-S101"));
}

#[test]
fn wait_binding_is_closed_and_checked_but_carries_no_execution_authority() {
    for binding in [
        "model_wait_v1 { execute = \"helper\"; }",
        "model_wait_v1 { propose = \"helper\"; propose = \"helper\"; }",
        "model_wait_v1 { propose = \"helper\"; } model_wait_v1 { propose = \"helper\"; }",
        "model_wait_v1 { propose = \"helper\"; } model_wait_v2 { propose = \"helper\"; }",
    ] {
        let text = source().replace("runtime_v1", &format!("{binding} runtime_v1"));
        assert_eq!(
            crate::parse(&text, "closed.spx").unwrap_err().code,
            "SPX-P124"
        );
    }
    for helper in [
        "missing",
        "op.propose",
        "op.initialize",
        "type.state",
        "agent",
    ] {
        let text = source().replace(
            "runtime_v1",
            &format!("model_wait_v1 {{ propose = \"{helper}\"; }} runtime_v1"),
        );
        let parsed = crate::parse(&text, "association.spx").unwrap();
        assert_eq!(
            crate::hir::resolve(&parsed).unwrap_err()[0].code,
            "SPX-G559"
        );
    }
}

#[test]
fn generic_or_yielding_embedded_roles_are_refused_by_source_profile() {
    for signature in [
        "fn initialize<T>(value:i64)->i64 { value + 1 }",
        "fn initialize(value:i64)->i64 yields i64 -> i64 { yield value }",
    ] {
        let text = source()
            .replacen("fn initialize(value:i64)->i64 { value + 1 }", signature, 1)
            .replace("initialize(1)", "0");
        let parsed = crate::parse(&text, "profile.spx").unwrap();
        assert!(crate::hir::resolve(&parsed)
            .unwrap_err()
            .iter()
            .any(|d| d.code == "SPX-G559"));
    }
}

#[test]
fn model_wait_helper_must_be_concrete_and_outside_every_agent_body() {
    let text = source().replace(
        "runtime_v1",
        "model_wait_v1 { propose = \"helper\"; } runtime_v1",
    );
    let generic = format!("{text}\n@id(\"helper\") fn wait<T>(value:T)->T {{value}}");
    let parsed = crate::parse(&generic, "generic-helper.spx").unwrap();
    assert_eq!(
        crate::hir::resolve(&parsed).unwrap_err()[0].code,
        "SPX-G559"
    );
    let mut parsed = crate::parse(&text, "other-agent.spx").unwrap();
    let mut other = parsed.agents[0].clone();
    other.stable_id = "other.agent".to_owned();
    other.name = "Other".to_owned();
    other.model_wait = None;
    parsed.agents[0].model_wait.as_mut().unwrap().helper_id = "op.initialize".to_owned();
    // The selected Agent may not borrow another Agent's actual embedded entry as top-level metadata.
    parsed.agents[0].operations[0].stable_id = "reference.initialize".to_owned();
    parsed.agents[0].operations[0].embedded_function_index = None;
    let mut reference = parsed.functions[0].clone();
    reference.stable_id = "reference.initialize".to_owned();
    reference.span = crate::ast::Span::default();
    parsed.functions.push(reference);
    parsed.agents.push(other);
    assert!(parsed.agents[0]
        .validate_execution_metadata(&parsed)
        .unwrap_err()
        .contains("top-level"));
}

#[test]
fn embedded_comments_and_ordinary_contracts_keep_one_canonical_projection() {
    let text = source().replace("@id(\"op.observe\")", "// observe body\n@id(\"op.observe\")").replace("fn observe(value:i64)->i64 { value + 1 }", "fn observe(value:i64)->i64 requires value >= 0 ensures result > value { value + 1 } // observed");
    crate::check(&text, "comments.spx").unwrap();
    let (_, canonical) = crate::parse_canonical(&text, "comments.spx").unwrap();
    assert_eq!(canonical.matches("// observe body").count(), 1);
    assert_eq!(canonical.matches("// observed").count(), 1);
    assert!(canonical.contains("            requires value >= 0"));
    assert!(canonical.contains("            ensures result > value"));
    crate::check(&canonical, "comments.spx").unwrap();
    let (_, repeated) = crate::parse_canonical(&canonical, "comments.spx").unwrap();
    assert_eq!(repeated, canonical);
}
