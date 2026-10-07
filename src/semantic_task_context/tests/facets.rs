use serde_json::Value;

use super::*;

#[test]
fn declaration_facets_are_attached_to_exactly_the_closure_ids_they_name() {
    let program = program(FIXTURE);
    let options = per_seed_options(1);
    let budget = generous_budget("byte-v1");
    let goal = CompilationGoal::new(vec![CompilationSeed::new("app.goal_a_root", 1, "")]).unwrap();
    let facets = DeclarationFacets::new()
        .with_requirement("app.helper_a", "REQ-1")
        .with_test("app.helper_a", "test_helper_a_behaves")
        .with_candidate_diff_change("app.goal_a_root")
        .with_requirement("app.helper_b", "REQ-2");
    let raw = compile_with_declaration_facets(&program, &goal, &options, budget, &facets).unwrap();
    let document: Value = serde_json::from_str(&raw).unwrap();
    let facet_entries = seed_entry(&document, "app.goal_a_root")["declaration_facets"]
        .as_array()
        .unwrap();
    let by_id = |id: &str| -> &Value {
        facet_entries
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap_or_else(|| panic!("facet entry for `{id}` is present"))
    };
    assert_eq!(by_id("app.goal_a_root")["candidate_diff"], true);
    assert_eq!(
        by_id("app.helper_a")["requirements"].as_array().unwrap(),
        &vec![Value::String("REQ-1".to_owned())]
    );
    assert_eq!(
        by_id("app.helper_a")["tests"].as_array().unwrap(),
        &vec![Value::String("test_helper_a_behaves".to_owned())]
    );
    assert!(!facet_entries
        .iter()
        .any(|entry| entry["id"] == "app.helper_b"));
}

#[test]
fn declaration_facets_are_absent_when_no_facet_data_is_supplied() {
    let program = program(FIXTURE);
    let options = per_seed_options(1);
    let budget = generous_budget("byte-v1");
    let goal = CompilationGoal::new(vec![CompilationSeed::new("app.goal_a_root", 1, "")]).unwrap();
    let raw = compile_with_declaration_facets(
        &program,
        &goal,
        &options,
        budget,
        &DeclarationFacets::new(),
    )
    .unwrap();
    let document: Value = serde_json::from_str(&raw).unwrap();
    assert!(
        seed_entry(&document, "app.goal_a_root")["declaration_facets"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn compile_never_emits_a_declaration_facets_field() {
    let program = program(FIXTURE);
    let options = per_seed_options(1);
    let budget = generous_budget("byte-v1");
    let goal = CompilationGoal::new(vec![CompilationSeed::new("app.goal_a_root", 1, "")]).unwrap();
    let document: Value =
        serde_json::from_str(&compile(&program, &goal, &options, budget).unwrap()).unwrap();
    assert!(seed_entry(&document, "app.goal_a_root")
        .get("declaration_facets")
        .is_none());
}

#[test]
fn returned_declaration_facets_are_bound_to_the_goal_digest() {
    let program = program(FIXTURE);
    let options = per_seed_options(1);
    let budget = generous_budget("byte-v1");
    let goal = CompilationGoal::new(vec![CompilationSeed::new("app.goal_a_root", 1, "")]).unwrap();
    let compile = |facets: &DeclarationFacets| {
        serde_json::from_str::<Value>(
            &compile_with_declaration_facets(&program, &goal, &options, budget, facets).unwrap(),
        )
        .unwrap()
    };
    let documents = [
        compile(&DeclarationFacets::new().with_requirement("app.helper_a", "REQ-A")),
        compile(&DeclarationFacets::new().with_requirement("app.helper_a", "REQ-B")),
        compile(&DeclarationFacets::new().with_test("app.helper_a", "TEST-A")),
        compile(&DeclarationFacets::new().with_candidate_diff_change("app.helper_a")),
    ];
    let digests = documents
        .iter()
        .map(|document| &document["goal_digest"])
        .collect::<Vec<_>>();
    for (index, digest) in digests.iter().enumerate() {
        assert!(digests
            .iter()
            .enumerate()
            .all(|(other_index, other)| index == other_index || digest != other));
    }
}

#[test]
fn declaration_facet_order_and_unreachable_facets_do_not_change_output() {
    let program = program(FIXTURE);
    let options = per_seed_options(1);
    let budget = generous_budget("byte-v1");
    let goal = CompilationGoal::new(vec![CompilationSeed::new("app.goal_a_root", 1, "")]).unwrap();
    let first = DeclarationFacets::new()
        .with_requirement("app.helper_a", "REQ-B")
        .with_requirement("app.helper_a", "REQ-A")
        .with_test("app.helper_a", "TEST-A");
    let second = DeclarationFacets::new()
        .with_test("app.helper_a", "TEST-A")
        .with_requirement("app.helper_a", "REQ-A")
        .with_requirement("app.helper_a", "REQ-B");
    assert_eq!(
        compile_with_declaration_facets(&program, &goal, &options, budget, &first).unwrap(),
        compile_with_declaration_facets(&program, &goal, &options, budget, &second).unwrap()
    );
    let unreachable = DeclarationFacets::new()
        .with_requirement("app.helper_b", "UNREACHABLE")
        .with_candidate_diff_change("app.helper_b");
    assert_eq!(
        compile_with_declaration_facets(
            &program,
            &goal,
            &options,
            budget,
            &DeclarationFacets::new()
        )
        .unwrap(),
        compile_with_declaration_facets(&program, &goal, &options, budget, &unreachable).unwrap()
    );
}

#[test]
fn declaration_facet_payload_is_budgeted_as_one_complete_unit() {
    let program = program(FIXTURE);
    let options = per_seed_options(1);
    let goal = CompilationGoal::new(vec![CompilationSeed::new("app.goal_a_root", 1, "")]).unwrap();
    let facets = DeclarationFacets::new()
        .with_requirement("app.helper_a", "REQ-A")
        .with_test("app.helper_a", "TEST-A")
        .with_candidate_diff_change("app.goal_a_root");
    let compile_at = |budget| {
        serde_json::from_str::<Value>(
            &compile_with_declaration_facets(&program, &goal, &options, budget, &facets).unwrap(),
        )
        .unwrap()
    };
    let generous = compile_at(generous_budget("byte-v1"));
    let tokens = seed_entry(&generous, "app.goal_a_root")["tokens"]
        .as_u64()
        .unwrap() as usize;
    let exact = compile_at(CompilationBudget::new(tokens, "byte-v1").unwrap());
    assert_eq!(seed_entry(&exact, "app.goal_a_root")["status"], "included");
    assert_eq!(exact["budget"]["used_tokens"], tokens);
    let short = compile_at(CompilationBudget::new(tokens - 1, "byte-v1").unwrap());
    assert_eq!(
        seed_entry(&short, "app.goal_a_root")["status"],
        "omitted_budget_exhausted"
    );
    assert_eq!(short["budget"]["used_tokens"], 0);
}

#[test]
fn oversized_declaration_facet_identifiers_are_refused_before_rendering() {
    let program = program(FIXTURE);
    let options = per_seed_options(1);
    let goal = CompilationGoal::new(vec![CompilationSeed::new("app.goal_a_root", 1, "")]).unwrap();
    let oversized = "X".repeat(MAX_FACET_IDENTIFIER_BYTES + 1);
    for facets in [
        DeclarationFacets::new().with_requirement("app.helper_a", oversized.clone()),
        DeclarationFacets::new().with_requirement(oversized.clone(), "REQ-A"),
        DeclarationFacets::new().with_test("app.helper_a", oversized.clone()),
        DeclarationFacets::new().with_test(oversized.clone(), "TEST-A"),
        DeclarationFacets::new().with_candidate_diff_change(oversized.clone()),
    ] {
        let error = compile_with_declaration_facets(
            &program,
            &goal,
            &options,
            generous_budget("byte-v1"),
            &facets,
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-Z801");
    }
}
