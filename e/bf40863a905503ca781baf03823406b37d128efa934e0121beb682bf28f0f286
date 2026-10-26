use serde_json::Value;
const SOURCE: &str = "src/cleanup_plan/replay.rs";
const CURRENT: &str = include_str!("../cleanup_plan/replay.rs");
const PRIOR: &str = include_str!("../../e/751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5");
const SHA: &str = "392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a";
const PREFIX: &str = r####"[[849,"mod fact_preflight;\n",""],[1134,"mod projection_backing;\n",""],[16921,"use fact_preflight::validate_program_context;","fn validate_program_context(\n    program: &dyn crate::cleanup::ReplayContext,\n) -> Result<(), Diagnostic> {\n    let full = program.full_program().ok_or_else(|| {\n        Diagnostic::io(\n            \"SPX-H006\",\n            \"allocation replay requires the held complete program\",\n        )\n    })?;\n    let mut budget = ReplayBudget::new();\n    let helpers = ordinary_renewals::index(full, &mut budget)?;\n    reserve_program_skeleton_work(\n        program,\n        program.functions().iter().chain(\n            program\n                .function_instances()\n                .iter()\n                .map(|instance| &instance.function),\n        ),\n        &mut budget,\n    )?;\n    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;\n    #[cfg(test)]\n    let mut diagnostic_ordinal = 0usize;\n    for function in program.functions() {\n        #[cfg(test)]\n        let _debit_function = {\n            let scope = debit_routes::function(diagnostic_ordinal, false, function.id.as_str());\n            diagnostic_ordinal = diagnostic_ordinal.saturating_add(1);\n            scope\n        };\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);\n        validate_structure_with_budget_storage(\n            program,\n            function,\n            &mut budget,\n            Some(&helpers),\n            Some(&mut continuations),\n        )?;\n    }\n    #[cfg(test)]\n    let mut diagnostic_instance = 0usize;\n    for instance in program.function_instances() {\n        #[cfg(test)]\n        let _debit_function = {\n            let scope =\n                debit_routes::function(diagnostic_instance, true, instance.function.id.as_str());\n            diagnostic_instance = diagnostic_instance.saturating_add(1);\n            scope\n        };\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);\n        validate_structure_with_budget_storage(\n            program,\n            &instance.function,\n            &mut budget,\n            Some(&helpers),\n            Some(&mut continuations),\n        )?;\n    }\n    Ok(())\n}"],[60176,"    #[cfg(test)]\n    if projection_backing::before_selected() {\n        return projection_backing::before_body!(function, budget);\n    }\n",""],[60441,"    // This additional owner/constructor/drop bank precedes formation. Existing\n    // shape receiver, work, growth, clone and leaf invoices remain unchanged.\n    budget.reserve_bytes(\n        function,\n        projection_backing::ENTRY,\n        \"shape projection backing owner\",\n    )?;\n    let mut projections = Vec::new();\n",""],[60903,"projections","Vec::new()"],[61060,"    // Each completed shape proves its starting depth was restored. Drop the\n    // empty backing at the original last-use frontier, before flag validation.\n    drop(projections);\n",""],[323649,"{\n    with_financed_validation_test_oracles, with_financed_workspace_test_oracles,\n}","with_financed_workspace_test_oracles"]]"####;
fn authenticate() {
    assert_eq!((CURRENT.len(), super::digest(CURRENT)), (326143, SHA.to_owned()));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(super::reader::read(&root.join(SOURCE), CURRENT.len(), SHA, 2_097_152).unwrap(), CURRENT);
    assert_eq!(super::reader::read(&root.join("e/751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5"), PRIOR.len(), "751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5", 2_097_152).unwrap(), PRIOR);
}
pub(super) fn before_probe(actual: &str) -> (String, Value) {
    authenticate();
    assert_eq!(actual, CURRENT);
    let edits: Value = serde_json::from_str(PREFIX).unwrap();
    assert!(edits.as_array().unwrap().len() <= 512);
    let mut restored = actual.to_owned();
    let mut next = restored.len();
    for edit in edits.as_array().unwrap().iter().rev() {
        let start = edit[0].as_u64().unwrap() as usize;
        let after = edit[1].as_str().unwrap();
        let end = start.checked_add(after.len()).unwrap();
        assert!(end <= next);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit[2].as_str().unwrap());
        assert!(restored.len() <= 2_097_152);
        next = start;
    }
    assert_eq!(restored, PRIOR, "entire immediate old CI probe source");
    authenticate();
    (restored, edits)
}
pub(super) fn finish_probe(probe: &mut Value, actual: &str, prefix: Value) {
    assert_eq!(actual, CURRENT);
    assert_eq!(probe["length"], PRIOR.len());
    assert_eq!(probe["sha256"], "751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5");
    let entire_prior = probe.clone();
    probe["inverse"].as_array_mut().unwrap().extend(prefix.as_array().unwrap().iter().cloned());
    probe["length"] = actual.len().into();
    probe["sha256"] = SHA.into();
    assert!(probe.get("retained_pre_current1042_probe").is_none());
    probe["retained_pre_current1042_probe"] = entire_prior;
    authenticate();
}
