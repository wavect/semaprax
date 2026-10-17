//! Representation accounting differs; complete semantic authority does not.
use super::*;
use crate::cleanup_plan::replay::expression_facts::Lookup;
use crate::digest_hex::LowerHex;
use sha2::{Digest, Sha256};

#[test]
fn complete_arena_ordinary_receiver_keeps_exact_same_representation_work_and_paid_fact_payloads() {
    let program = program();
    let function = &program.functions[0];
    assert!(!lexical_cert::requires_lexical(function));
    assert_eq!(
        ReplayBudget::new().remaining,
        ReplayBudget::unobserved().remaining
    );
    let expected = expression_facts(function).unwrap();
    let ((), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let facts = expression_facts::with_budget(function, &mut budget).unwrap();
        assert!(matches!(&facts, expression_facts::Facts::Arena(_)));
        let entries = Lookup::entries_paid(&facts, function, &mut budget).unwrap();
        assert_eq!(expected.len(), facts.len());
        for ((old_id, old), (id, current)) in expected.iter().zip(entries) {
            assert_eq!(old_id, id);
            assert_eq!(old_id.as_str().as_ptr(), id.as_str().as_ptr());
            match (old, current) {
                (Some(old), Some(current)) => {
                    assert_eq!(old.callee, current.callee);
                    assert_eq!(old.instance, current.instance);
                    assert_eq!(old.arguments, current.arguments);
                    assert_eq!(old.type_arguments, current.type_arguments);
                }
                (None, None) => {}
                _ => panic!("complete fact payload parity"),
            }
        }
        assert!(!budget.selected() && budget.physical.is_none());
    });
    assert!(!refused);
    let run = |limit| {
        with_limit_usage_refusal(limit, || {
            let mut budget = ReplayBudget::new();
            let upper =
                reserve_program_skeleton_work(&program, std::iter::once(function), &mut budget)
                    .unwrap();
            let result = validate_structure_with_budget(&program, function, &mut budget, None);
            assert!(!budget.selected() && budget.physical.is_none());
            let usage = (budget.remaining, budget.skeleton_spent, upper);
            (result, usage)
        })
    };
    let ((full, usage), refused, charged, first) = run(usize::MAX);
    full.unwrap();
    assert!(!refused && first.is_none());
    let ((exact, repeated), refused, used, first) = run(charged);
    exact.unwrap();
    assert!(!refused && first.is_none());
    assert_eq!(used, charged);
    assert_eq!(repeated, usage);
}

#[test]
fn complete_ordinary_tree_receiver_original_owner_and_current_accounting_scope_stay_source_bound() {
    let original = ci_str!(
        "../../../../../docs/evidence/funded-planner-fact-owner/original-allocation-tests.txt"
    );
    let original_owner = ci_str!(
        "../../../../../docs/evidence/funded-planner-fact-owner/original-ordinary-owner.txt"
    );
    assert!(original.contains(original_owner));
    let preamble = r#"//! Full old owning body binds the complete independent Tree replay receiver.
use super::*;
use crate::cleanup_plan::replay::expression_facts::ordinary_before::validate_for_test as validate_structure_with_budget;
mod expression_facts {
    pub(super) use crate::cleanup_plan::replay::expression_facts::ordinary_before::tree_facts_for_test as with_budget;
}
"#;
    assert_eq!(
        ci_str!("ordinary_before.rs"),
        format!("{preamble}{original_owner}\n")
    );
    let receiver = ci_str!(
        "../../../../../docs/evidence/funded-planner-fact-owner/original-ordinary-receiver.txt"
    );
    let preamble = r#"//! Complete original ordinary receiver with a lexical old-producer adapter.
use super::*;
mod expression_facts {
    use super::*;
    pub(super) use super::super::receiving;
    pub(super) fn with_budget<'hir>(
        function: &'hir ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Facts<'hir>, Diagnostic> {
        saved::with_budget(function, budget).map(Facts::Tree)
    }
}
"#;
    let suffix = r#"
pub(in crate::cleanup_plan::replay) fn validate_for_test(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
) -> Result<(), Diagnostic> {
    validate_structure_with_budget(program, function, budget, helpers)
}
pub(in crate::cleanup_plan::replay) fn tree_facts_for_test<'hir>(
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeMap<ExpressionId, Option<CallFact<'hir>>>, Diagnostic> {
    saved::with_budget(function, budget)
}
"#;
    assert_eq!(
        ci_str!("../../expression_facts/ordinary_before.rs"),
        format!("{preamble}{receiver}{suffix}")
    );
    // Invert exactly the paid-summary hook; compare the complete old receiver.
    let current_replay =
        crate::cleanup_plan::replay::expression_facts::receiving::before_status_retention(
            "src/cleanup_plan/replay.rs",
            ci_str!("../../../replay.rs"),
        );
    let mut current_replay = current_replay;
    assert_eq!(current_replay.matches("    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;").count(), 1);
    current_replay = current_replay.replacen(
        "    expression_facts::receiving::validate_entry(function, &storage, &leaves, budget)?;",
        "    validate_entry(function, &storage, &leaves)?;",
        1,
    );
    assert_eq!(
        current_replay
            .matches(
                "    expression_facts::receiving::validate_regions(function, &storage, budget)?;"
            )
            .count(),
        1
    );
    current_replay = current_replay.replacen(
        "    expression_facts::receiving::validate_regions(function, &storage, budget)?;",
        "    validate_regions(function, &storage)?;",
        1,
    );
    assert_eq!(current_replay.matches("    expression_facts::receiving::validate_exits(\n        program,\n        function,\n        &storage,\n        &leaves,\n        &BTreeSet::new(),\n        budget,\n    )?;").count(), 1);
    current_replay = current_replay.replacen("    expression_facts::receiving::validate_exits(\n        program,\n        function,\n        &storage,\n        &leaves,\n        &BTreeSet::new(),\n        budget,\n    )?;", "    validate_exits(program, function, &storage, &leaves, &BTreeSet::new())?;", 1);
    let paid_schema_hook = "    let ordinary_schema =\n        schema::paid_summary::selected_with_budget(program, function, helpers, budget)?;";
    let original_schema_hook =
        "    let ordinary_schema = schema::selected_schema_with_index(program, function, helpers)?;";
    assert_eq!(current_replay.matches(paid_schema_hook).count(), 1);
    let original_receiver_projection =
        current_replay.replacen(paid_schema_hook, original_schema_hook, 1);
    assert!(original_receiver_projection.contains(receiver));
    let old = "        assert_eq!(active.remaining, legacy.remaining);";
    let new = r#"        // The current Arena prepays search/rotation/receiving visits that the
        // independent Tree route does not charge. Exact original Tree parity
        // remains compiled in ordinary_before; skeleton parity stays exact.
        assert!(active.remaining < legacy.remaining);
        assert!(active.remaining > 0);"#;
    assert_eq!(original.matches(old).count(), 1);
    assert_eq!(
        ci_str!("../tests.rs"),
        format!(
            "{}\nmod accounting_scope;\nmod ordinary_before;\n",
            original.replacen(old, new, 1)
        )
    );
    let facts = ci_str!("../../../../../docs/evidence/funded-planner-fact-owner/original-expression-facts-module.txt");
    assert_eq!(ci_str!("../../expression_facts.rs"), format!("{facts}\n#[cfg(test)]\nuse crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20;\n#[cfg(test)]\npub(super) mod ordinary_before;\n"));
    assert_eq!(
        format!(
            "{:x}",
            LowerHex(Sha256::digest(
                ci_str!("../../expression_facts/saved.rs").as_bytes()
            ))
        ),
        "0aef16929be73f5fe43438565c98a17f00cf52cd4169a4bc33cc99e6f184650a"
    );
}
