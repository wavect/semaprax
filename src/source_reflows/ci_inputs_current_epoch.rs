use serde_json::Value;
const SOURCE: &str = "src/cleanup_plan/replay.rs";
const CURRENT: &str = include_str!("../../e/392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a");
const PRIOR: &str = include_str!("../../e/751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5");
const SHA: &str = "392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a";
const PREFIX: &str = r####"[[849,"mod fact_preflight;\n",""],[1134,"mod projection_backing;\n",""],[16921,"use fact_preflight::validate_program_context;","fn validate_program_context(\n    program: &dyn crate::cleanup::ReplayContext,\n) -> Result<(), Diagnostic> {\n    let full = program.full_program().ok_or_else(|| {\n        Diagnostic::io(\n            \"SPX-H006\",\n            \"allocation replay requires the held complete program\",\n        )\n    })?;\n    let mut budget = ReplayBudget::new();\n    let helpers = ordinary_renewals::index(full, &mut budget)?;\n    reserve_program_skeleton_work(\n        program,\n        program.functions().iter().chain(\n            program\n                .function_instances()\n                .iter()\n                .map(|instance| &instance.function),\n        ),\n        &mut budget,\n    )?;\n    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;\n    #[cfg(test)]\n    let mut diagnostic_ordinal = 0usize;\n    for function in program.functions() {\n        #[cfg(test)]\n        let _debit_function = {\n            let scope = debit_routes::function(diagnostic_ordinal, false, function.id.as_str());\n            diagnostic_ordinal = diagnostic_ordinal.saturating_add(1);\n            scope\n        };\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);\n        validate_structure_with_budget_storage(\n            program,\n            function,\n            &mut budget,\n            Some(&helpers),\n            Some(&mut continuations),\n        )?;\n    }\n    #[cfg(test)]\n    let mut diagnostic_instance = 0usize;\n    for instance in program.function_instances() {\n        #[cfg(test)]\n        let _debit_function = {\n            let scope =\n                debit_routes::function(diagnostic_instance, true, instance.function.id.as_str());\n            diagnostic_instance = diagnostic_instance.saturating_add(1);\n            scope\n        };\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);\n        validate_structure_with_budget_storage(\n            program,\n            &instance.function,\n            &mut budget,\n            Some(&helpers),\n            Some(&mut continuations),\n        )?;\n    }\n    Ok(())\n}"],[60176,"    #[cfg(test)]\n    if projection_backing::before_selected() {\n        return projection_backing::before_body!(function, budget);\n    }\n",""],[60441,"    // This additional owner/constructor/drop bank precedes formation. Existing\n    // shape receiver, work, growth, clone and leaf invoices remain unchanged.\n    budget.reserve_bytes(\n        function,\n        projection_backing::ENTRY,\n        \"shape projection backing owner\",\n    )?;\n    let mut projections = Vec::new();\n",""],[60903,"projections","Vec::new()"],[61060,"    // Each completed shape proves its starting depth was restored. Drop the\n    // empty backing at the original last-use frontier, before flag validation.\n    drop(projections);\n",""],[323649,"{\n    with_financed_validation_test_oracles, with_financed_workspace_test_oracles,\n}","with_financed_workspace_test_oracles"]]"####;
fn authenticate() {
    assert_eq!((CURRENT.len(), super::digest(CURRENT)), (326143, SHA.to_owned()));
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(super::reader::read(&root.join("e/392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a"), CURRENT.len(), SHA, 2_097_152).unwrap(), CURRENT);
    assert_eq!(super::reader::read(&root.join("e/751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5"), PRIOR.len(), "751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5", 2_097_152).unwrap(), PRIOR);
}
fn before_probe_before_943(actual: &str) -> (String, Value) {
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
fn finish_probe_before_943(probe: &mut Value, actual: &str, prefix: Value) {
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

// The frame-root caller has already authenticated the lexical-join predecessor.
// Rejoin only that exact full stage to its physical source before the unchanged
// batch/closure chains; no other supplied string is admitted as physical input.
pub(super) fn physical_batch_input<'a>(source: &str, supplied: &'a str, physical: &'a str) -> std::borrow::Cow<'a, str> {
    if source != "src/hir/validation/lexical_exit.rs" || supplied == physical {
        return std::borrow::Cow::Borrowed(supplied);
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let metadata_source = "src/hir/validation/lexical_exit/lexical_join_inverse.json";
    let metadata = include_str!("../hir/validation/lexical_exit/lexical_join_inverse.json");
    let child_source = "src/hir/validation/lexical_exit/lexical_join.rs";
    let child = include_str!("../hir/validation/lexical_exit/lexical_join.rs");
    let data: Value = serde_json::from_str(metadata).unwrap();
    let row = data["rows"].as_array().unwrap().iter().find(|r| r["source"] == source).unwrap();
    let length = row["current"]["length"].as_u64().unwrap() as usize;
    let sha = row["current"]["sha256"].as_str().unwrap();
    let before = row["before_text"].as_str().unwrap();
    assert_eq!(physical.len(), length); assert_eq!(super::digest(physical), sha);
    assert_eq!(before.len() as u64, row["before"]["length"].as_u64().unwrap());
    assert_eq!(super::digest(before), row["before"]["sha256"].as_str().unwrap());
    assert_eq!(supplied, before, "exact authenticated lexical-join predecessor at batch ingress");
    assert_eq!(child.len() as u64, data["new_child"]["length"].as_u64().unwrap());
    assert_eq!(super::digest(child), data["new_child"]["sha256"].as_str().unwrap());
    for _ in 0..2 {
        assert_eq!(super::reader::read(&root.join(metadata_source), metadata.len(), &super::digest(metadata), 2_097_152).unwrap(), metadata);
        assert_eq!(super::reader::read(&root.join(child_source), child.len(), &super::digest(child), 2_097_152).unwrap(), child);
        assert_eq!(super::reader::read(&root.join(source), length, sha, 2_097_152).unwrap(), physical);
    }
    std::borrow::Cow::Borrowed(physical)
}

// Complete new943 physical root -> entire392da immediate predecessor -> the
// original751c CI probe leg. Old before/finish bodies and metadata stay whole.
const NEXT_CURRENT: &str = include_str!("../cleanup_plan/replay.rs");
const NEXT_SHA: &str = "4830476b83035da6f6748315a02222a67dba06ac79bcb6f68ed27b09dbca7b4a";
const NEXT_PREFIX: &str = r####"[[1455,"#[cfg(test)]\nmod trailing_active;\n",""],[121014,"    #[cfg(test)]\n    if trailing_active::before_selected() {\n        return trailing_active::before_sequence_skeleton_paths(prefixes, suffixes, work);\n    }\n",""],[122004,"    }\n    #[cfg(test)]\n    if !single && first {\n        trailing_active::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged, first, single);\n",""],[124554,"        if !precharged { final_output::record_final_frontier(0, work); }\n        if precharged { precharged = false; }\n        else { work.charge(1, \"skeleton prefix clone\")?; }\n","        final_output::record_final_frontier(0, work);\n        work.charge(1, \"skeleton prefix clone\")?;\n"]]"####;
fn authenticate_next() {
    assert_eq!((NEXT_CURRENT.len(), super::digest(NEXT_CURRENT)), (326566, NEXT_SHA.to_owned()));
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(super::reader::read(&root.join(SOURCE), NEXT_CURRENT.len(), NEXT_SHA, 2_097_152).unwrap(), NEXT_CURRENT);
    authenticate();
}
fn next_before(actual: &str) -> (String, Value) {
    authenticate_next(); assert_eq!(actual, NEXT_CURRENT);
    let edits:Value=serde_json::from_str(NEXT_PREFIX).unwrap();assert!(edits.as_array().unwrap().len()<=512);
    let mut before=actual.to_owned();let mut next=before.len();
    for e in edits.as_array().unwrap().iter().rev() {
        let at=e[0].as_u64().unwrap() as usize;let after=e[1].as_str().unwrap();let end=at.checked_add(after.len()).unwrap();
        assert!(end<=next);assert_eq!(before.get(at..end),Some(after));
        before.replace_range(at..end,e[2].as_str().unwrap());assert!(before.len()<=2_097_152);next=at;
    }
    assert_eq!(before,CURRENT,"ENTIRE326143-byte immediate source, not a new qualified epoch");authenticate_next();(before,edits)
}
pub(super) fn before_probe(actual: &str) -> (String, Value) {
    let (immediate,extra)=next_before(actual);let (prior,mut prefix)=before_probe_before_943(&immediate);
    prefix.as_array_mut().unwrap().extend(extra.as_array().unwrap().iter().cloned());authenticate_next();(prior,prefix)
}
pub(super) fn finish_probe(probe: &mut Value, actual: &str, prefix: Value) {
    let (immediate,extra)=next_before(actual);
    let original:Value=serde_json::from_str(PREFIX).unwrap();
    let mut combined=original.as_array().unwrap().clone();combined.extend(extra.as_array().unwrap().iter().cloned());
    assert_eq!(prefix,Value::Array(combined),"exact two complete source transports only");
    finish_probe_before_943(probe,&immediate,original);
    let entire_immediate_probe=probe.clone();
    probe["inverse"].as_array_mut().unwrap().extend(extra.as_array().unwrap().iter().cloned());
    probe["length"]=actual.len().into();probe["sha256"]=NEXT_SHA.into();
    assert!(probe.get("retained_pre_943_probe").is_none());probe["retained_pre_943_probe"]=entire_immediate_probe;
    authenticate_next();
}
// New numeric banks append to allocation.rs; every old CI source byte and its
// existing inverse stay unchanged, after fresh physical and archive acquisition.
pub(super) fn allocation_input<'a>(source:&str, supplied:&'a str) -> std::borrow::Cow<'a,str> {
    if source!="src/cleanup_plan/replay/allocation.rs" {return std::borrow::Cow::Borrowed(supplied);}
    const ACTUAL:&str=include_str!("../cleanup_plan/replay/allocation.rs");
    const OLD:&str=include_str!("../../e/d3f844655708972aa9e089c39923ddced6891b6b776f483d4f0be07e43cceb59");
    const ACTUAL_SHA:&str="0d9c44752ba86a56eb75d5a77cbe97f60a7152dce52530daf9b5c06066faeda5";
    assert_eq!((ACTUAL.len(),super::digest(ACTUAL)),(12094,ACTUAL_SHA.to_owned()));
    assert_eq!(supplied,ACTUAL);assert_eq!(&ACTUAL[..OLD.len()],OLD);
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..2 {
        assert_eq!(super::reader::read(&root.join(source),ACTUAL.len(),ACTUAL_SHA,2_097_152).unwrap(),ACTUAL);
        assert_eq!(super::reader::read(&root.join("e/d3f844655708972aa9e089c39923ddced6891b6b776f483d4f0be07e43cceb59"),OLD.len(),"d3f844655708972aa9e089c39923ddced6891b6b776f483d4f0be07e43cceb59",2_097_152).unwrap(),OLD);
    }
    std::borrow::Cow::Borrowed(OLD)
}
