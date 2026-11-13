use serde_json::Value;
const SOURCE: &str = "src/cleanup_plan/replay.rs";
const CURRENT: &str =
    include_str!("../../e/392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a");
const PRIOR: &str =
    include_str!("../../e/751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5");
const SHA: &str = "392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a";
const PREFIX: &str = r####"[[849,"mod fact_preflight;\n",""],[1134,"mod projection_backing;\n",""],[16921,"use fact_preflight::validate_program_context;","fn validate_program_context(\n    program: &dyn crate::cleanup::ReplayContext,\n) -> Result<(), Diagnostic> {\n    let full = program.full_program().ok_or_else(|| {\n        Diagnostic::io(\n            \"SPX-H006\",\n            \"allocation replay requires the held complete program\",\n        )\n    })?;\n    let mut budget = ReplayBudget::new();\n    let helpers = ordinary_renewals::index(full, &mut budget)?;\n    reserve_program_skeleton_work(\n        program,\n        program.functions().iter().chain(\n            program\n                .function_instances()\n                .iter()\n                .map(|instance| &instance.function),\n        ),\n        &mut budget,\n    )?;\n    let mut continuations = continuation_storage::Storage::for_program(full, &mut budget)?;\n    #[cfg(test)]\n    let mut diagnostic_ordinal = 0usize;\n    for function in program.functions() {\n        #[cfg(test)]\n        let _debit_function = {\n            let scope = debit_routes::function(diagnostic_ordinal, false, function.id.as_str());\n            diagnostic_ordinal = diagnostic_ordinal.saturating_add(1);\n            scope\n        };\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);\n        validate_structure_with_budget_storage(\n            program,\n            function,\n            &mut budget,\n            Some(&helpers),\n            Some(&mut continuations),\n        )?;\n    }\n    #[cfg(test)]\n    let mut diagnostic_instance = 0usize;\n    for instance in program.function_instances() {\n        #[cfg(test)]\n        let _debit_function = {\n            let scope =\n                debit_routes::function(diagnostic_instance, true, instance.function.id.as_str());\n            diagnostic_instance = diagnostic_instance.saturating_add(1);\n            scope\n        };\n        #[cfg(feature = \"unstable-workflow-profiling\")]\n        let _paid_structure = budget.paid_phase(PaidPhase::ReplayStructure);\n        validate_structure_with_budget_storage(\n            program,\n            &instance.function,\n            &mut budget,\n            Some(&helpers),\n            Some(&mut continuations),\n        )?;\n    }\n    Ok(())\n}"],[60176,"    #[cfg(test)]\n    if projection_backing::before_selected() {\n        return projection_backing::before_body!(function, budget);\n    }\n",""],[60441,"    // This additional owner/constructor/drop bank precedes formation. Existing\n    // shape receiver, work, growth, clone and leaf invoices remain unchanged.\n    budget.reserve_bytes(\n        function,\n        projection_backing::ENTRY,\n        \"shape projection backing owner\",\n    )?;\n    let mut projections = Vec::new();\n",""],[60903,"projections","Vec::new()"],[61060,"    // Each completed shape proves its starting depth was restored. Drop the\n    // empty backing at the original last-use frontier, before flag validation.\n    drop(projections);\n",""],[323649,"{\n    with_financed_validation_test_oracles, with_financed_workspace_test_oracles,\n}","with_financed_workspace_test_oracles"]]"####;
fn authenticate() {
    assert_eq!(
        (CURRENT.len(), super::digest(CURRENT)),
        (326143, SHA.to_owned())
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        super::reader::read(
            &root.join("e/392da88cb3c30b1adcff8d845c8af6a797586dfcaab9af2f0c898243f7c8350a"),
            CURRENT.len(),
            SHA,
            2_097_152
        )
        .unwrap(),
        CURRENT
    );
    assert_eq!(
        super::reader::read(
            &root.join("e/751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5"),
            PRIOR.len(),
            "751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5",
            2_097_152
        )
        .unwrap(),
        PRIOR
    );
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
    assert_eq!(
        probe["sha256"],
        "751c95561915a9b99ebec0bdf702dc9c7448d411f0f80ac58d244bc7df20dde5"
    );
    let entire_prior = probe.clone();
    probe["inverse"]
        .as_array_mut()
        .unwrap()
        .extend(prefix.as_array().unwrap().iter().cloned());
    probe["length"] = actual.len().into();
    probe["sha256"] = SHA.into();
    assert!(probe.get("retained_pre_current1042_probe").is_none());
    probe["retained_pre_current1042_probe"] = entire_prior;
    authenticate();
}

// The frame-root caller has already authenticated the lexical-join predecessor.
// Rejoin only that exact full stage to its physical source before the unchanged
// batch/closure chains; no other supplied string is admitted as physical input.
pub(super) fn physical_batch_input<'a>(
    source: &str,
    supplied: &'a str,
    physical: &'a str,
) -> std::borrow::Cow<'a, str> {
    if source != "src/hir/validation/lexical_exit.rs" || supplied == physical {
        return std::borrow::Cow::Borrowed(supplied);
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let metadata_source = "src/hir/validation/lexical_exit/lexical_join_inverse.json";
    let metadata = include_str!("../hir/validation/lexical_exit/lexical_join_inverse.json");
    let child_source = "src/hir/validation/lexical_exit/lexical_join.rs";
    let child = include_str!("../hir/validation/lexical_exit/lexical_join.rs");
    let data: Value = serde_json::from_str(metadata).unwrap();
    let row = data["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
        .unwrap();
    let length = row["current"]["length"].as_u64().unwrap() as usize;
    let sha = row["current"]["sha256"].as_str().unwrap();
    let before = row["before_text"].as_str().unwrap();
    assert_eq!(physical.len(), length);
    assert_eq!(super::digest(physical), sha);
    assert_eq!(
        before.len() as u64,
        row["before"]["length"].as_u64().unwrap()
    );
    assert_eq!(
        super::digest(before),
        row["before"]["sha256"].as_str().unwrap()
    );
    assert_eq!(
        supplied, before,
        "exact authenticated lexical-join predecessor at batch ingress"
    );
    assert_eq!(
        child.len() as u64,
        data["new_child"]["length"].as_u64().unwrap()
    );
    assert_eq!(
        super::digest(child),
        data["new_child"]["sha256"].as_str().unwrap()
    );
    for _ in 0..2 {
        assert_eq!(
            super::reader::read(
                &root.join(metadata_source),
                metadata.len(),
                &super::digest(metadata),
                2_097_152
            )
            .unwrap(),
            metadata
        );
        assert_eq!(
            super::reader::read(
                &root.join(child_source),
                child.len(),
                &super::digest(child),
                2_097_152
            )
            .unwrap(),
            child
        );
        assert_eq!(
            super::reader::read(&root.join(source), length, sha, 2_097_152).unwrap(),
            physical
        );
    }
    std::borrow::Cow::Borrowed(physical)
}

// Complete new943 physical root -> entire392da immediate predecessor -> the
// original751c CI probe leg. Old before/finish bodies and metadata stay whole.
const NEXT_CURRENT: &str =
    include_str!("../../e/2ae4da23cf6002ee01f95842c072dc8f80533814090607e442d2da05a5324a16");
const NEXT_SHA: &str = "2ae4da23cf6002ee01f95842c072dc8f80533814090607e442d2da05a5324a16";
const NEXT_PREFIX: &str = r####"[[1455,"#[cfg(test)]\nmod trailing_active;\n",""],[121014,"    #[cfg(test)]\n    if trailing_active::before_selected() {\n        return trailing_active::before_sequence_skeleton_paths(prefixes, suffixes, work);\n    }\n",""],[122004,"    }\n    #[cfg(test)]\n    if !single && first {\n        trailing_active::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged, first, single, combined);\n",""],[124572,"if !precharged { final_output::record_final_frontier(0, work); }\n        if precharged { precharged = false; }\n        else { work.charge(1, \"skeleton prefix clone\")?; }","final_output::record_final_frontier(0, work);\n        work.charge(1, \"skeleton prefix clone\")?;"]]"####;
fn authenticate_next() {
    assert_eq!(
        (NEXT_CURRENT.len(), super::digest(NEXT_CURRENT)),
        (326576, NEXT_SHA.to_owned())
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        super::reader::read(
            &root.join(format!("e/{NEXT_SHA}")),
            NEXT_CURRENT.len(),
            NEXT_SHA,
            2_097_152
        )
        .unwrap(),
        NEXT_CURRENT
    );
    authenticate();
}
fn next_before_943(actual: &str) -> (String, Value) {
    authenticate_next();
    assert_eq!(actual, NEXT_CURRENT);
    let edits: Value = serde_json::from_str(NEXT_PREFIX).unwrap();
    assert!(edits.as_array().unwrap().len() <= 512);
    let mut before = actual.to_owned();
    let mut next = before.len();
    for e in edits.as_array().unwrap().iter().rev() {
        let at = e[0].as_u64().unwrap() as usize;
        let after = e[1].as_str().unwrap();
        let end = at.checked_add(after.len()).unwrap();
        assert!(end <= next);
        assert_eq!(before.get(at..end), Some(after));
        before.replace_range(at..end, e[2].as_str().unwrap());
        assert!(before.len() <= 2_097_152);
        next = at;
    }
    assert_eq!(
        before, CURRENT,
        "ENTIRE326143-byte immediate source, not a new qualified epoch"
    );
    authenticate_next();
    (before, edits)
}
// Authenticate the live replay root before replaying each retained CI inverse.
const PHYSICAL_CURRENT: &str = include_str!("../cleanup_plan/replay.rs");
const PHYSICAL_SHA: &str = "ba0e6b1df76928071487692d329017d6567a43ea4644b96b01f110475a113a5f";
const PHYSICAL_PREFIX: &str = r####"[[1455,"","#[cfg(test)]\nmod trailing_active;\n"],[1543,"#[cfg(test)]\nmod trailing_active;\n",""],[2767,"","use expression_children::replay_expression_child;\nuse expression_facts::expression_facts;\nuse std::collections::{BTreeMap, BTreeSet, VecDeque};\n#[cfg(test)]\nuse crate::hir::ResolvedTypeDeclarationKind;\n#[cfg(test)]\nuse std::cell::Cell;\n"],[3701,"#[cfg(test)]\nuse crate::hir::ResolvedTypeDeclarationKind;\n",""],[4103,"use expression_children::replay_expression_child;\nuse expression_facts::expression_facts;\n#[cfg(test)]\nuse std::cell::Cell;\nuse std::collections::{BTreeMap, BTreeSet, VecDeque};\n",""],[17727,"        // A selected factored comparison borrows ordinary work, not an unused\n        // enumerating allowance. Its bounded fallback reserves before entry.\n        if function_upper != 0 && factored::defer_enumerating_reservation(function, budget)? {\n            continue;\n        }\n",""],[107785,"        && factored::validate_with_deferred_index_storage(\n","        && factored::validate_with_index_storage(\n"],[110954,"        ReplayBudget { payer: AllocationPayer::LegacyUnobserved, .. } if function.cleanup_plan.lexical_control.is_none() && { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } => !plan_prefix::equal_paths(function, budget, &mut expected, &mut actual)?,\n","        ReplayBudget { payer: AllocationPayer::LegacyUnobserved, .. } if { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } => !plan_prefix::equal_paths(function, budget, &mut expected, &mut actual)?,\n"],[114070,"        // Bootstrap refusal text before receiving backing. A recursive clone\n        // still acquires and uses its complete real cursor at that clone.\n","        // Exactly the old ordinary bootstrap/cursor frontier. No new receiving\n        // owner exists before these earlier errors have been selected.\n"],[114266,"","        budget.prepare_clone_cursor(function)?;\n"],[115299,"        budget.reserve_bytes(\n            function,\n            skeleton_work::query_proof::HEADER,\n            \"skeleton query neutral slot receivers\",\n        )?;\n        SkeletonWork {\n            queries: skeleton_work::query_proof::Slot::new(function, budget),\n","        budget.reserve_bytes(function, skeleton_work::query_proof::HEADER, \"skeleton query neutral slot receivers\")?;\n        SkeletonWork { queries: skeleton_work::query_proof::Slot::new(function, budget),\n"],[121527,"    sequence_vector::inplace_dispatch!(prefixes, suffixes, work);\n    let prefixes =\n        ordinary_plan_history::completed_fork::emission::dispatch!(prefixes, suffixes, work);\n","    let prefixes = ordinary_plan_history::completed_fork::emission::dispatch!(\n        prefixes, suffixes, work\n    );\n"],[122469,"        trailing_active::reuse!(\n            prefixes,\n            suffixes,\n            work,\n            share_prefix,\n            minimum,\n            precharged,\n            first,\n            single,\n            combined\n        );\n","        trailing_active::reuse!(prefixes, suffixes, work, share_prefix, minimum, precharged, first, single, combined);\n"],[125098,"        if !precharged {\n            final_output::record_final_frontier(0, work);\n        }\n        if precharged {\n            precharged = false;\n        } else {\n            work.charge(1, \"skeleton prefix clone\")?;\n        }\n","        if !precharged { final_output::record_final_frontier(0, work); }\n        if precharged { precharged = false; }\n        else { work.charge(1, \"skeleton prefix clone\")?; }\n"],[130336,"use continuation_storage::expression_skeleton;\n","fn expression_skeleton<'hir>(\n    program: &'hir dyn crate::cleanup::ReplayContext,\n    function: &'hir ResolvedFunction,\n    expression: &'hir ResolvedExpr,\n    work: &mut SkeletonWork<'hir, '_>,\n) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {\n    let (lease, mut frames) = if let Some(storage) = work.continuations.as_mut() {\n        if !storage.matches(program) {\n            return Err(replay_error(\n                function,\n                \"typed-HIR continuation storage belongs to another program\",\n            ));\n        }\n        storage.acquire(function, work.budget)?\n    } else {\n        (\n            continuation_storage::Lease::Local,\n            match work.budget {\n                ReplayBudget {\n                    payer: AllocationPayer::ActiveBuilder,\n                    lent_skeleton: false,\n                    ..\n                } => continuation_storage::fresh_receiving(function, work.budget)?,\n                // A factored constructor has lent its ordinary allowance to the\n                // original skeleton events. Keep its standalone fixed backing;\n                // incremental relocation needs an independent ordinary account.\n                _ => continuation_storage::fresh(function, work.budget)?,\n            },\n        )\n    };\n    // No directory borrow spans the runner: guards re-enter this same Storage.\n    // A panic still unwinds to invocation Drop, as in the original lease owner.\n    let result = expression_skeleton_run(program, function, expression, work, &mut frames);\n    frames.clear(); // Drop every owned continuation payload before reuse.\n    if let Some(storage) = work.continuations.as_mut() {\n        storage.release(lease, frames);\n    }\n    result\n}\n"],[130584,"    frames: &mut impl continuation_storage::FrameStack<'hir>,\n","    frames: &mut Vec<continuation_storage::Frame<'hir>>,\n"],[130871,"                continuation_storage::FrameStack::reserve_push($frames, function, work.budget)?;\n","                continuation_storage::reserve_push($frames, function, work.budget)?;\n"],[131096,"            $frames.push_owned($frame, function, work.budget)?;\n","            $frames.push($frame);\n"],[132158,"    fn advance_block_value<'a, 'e, S: continuation_storage::FrameStack<'e>>(\n","    fn advance_block_value<'a, 'e>(\n"],[132275,"        frames: &mut S,\n","        frames: &mut Vec<Frame<'e>>,\n"],[132576,"        let mut push = |frames: &mut S, frame: Frame<'e>| -> Result<(), Diagnostic> {\n","        let mut push = |frames: &mut Vec<Frame<'e>>, frame: Frame<'e>| -> Result<(), Diagnostic> {\n"],[132713,"                continuation_storage::FrameStack::reserve_push(frames, function, work.budget)?;\n","                continuation_storage::reserve_push(frames, function, work.budget)?;\n"],[132937,"            frames.push_owned(frame, function, work.budget)?;\n","            frames.push(frame);\n"],[135880,"    while let Some(frame) = frames.pop_owned(function, work.budget)? {\n","    while let Some(frame) = frames.pop() {\n"],[267912,"                observations,\n                suffix,\n                work\n","                observations, suffix, work\n"],[271625,"        let mut failure = skeleton_work::status_clone!(work, path, \"status failure path clone\")?;\n","        let mut failure = work.clone_expr_path(&path, \"status failure path clone\")?;\n"],[277490,"    // The shared PLAN prefix route is admitted only for ordinary control.\n    if matches!(budget.payer, AllocationPayer::LegacyUnobserved) && function.cleanup_plan.lexical_control.is_none() && { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } { return plan_prefix::build(function, budget); }\n","    if matches!(budget.payer, AllocationPayer::LegacyUnobserved) && { #[cfg(test)] { !ordinary_plan_history::completed_fork::emission::before_selected() } #[cfg(not(test))] { true } } { return plan_prefix::build(function, budget); }\n"],[288582,"                        branch = branch_clone::production_capacity::unpaid_clone(\n","                        branch = skeleton_clone(\n"],[297402,"                        branch = branch_clone::production_capacity::unpaid_clone(\n","                        branch = skeleton_clone(\n"]]"####;

const LIVE_SHA: &str = "3e89300daede8b284157cb5c24765fec66a76abd37487a89188cf64af66a1702";
const LIVE_PREFIX: &str = r####"[[17686,"skeleton_work_upper_with_selection","skeleton_work_upper"],[17738,", Some(budget)",""],[17755,"","\n        // A selected factored comparison borrows ordinary work, not an unused\n        // enumerating allowance. Its bounded fallback reserves before entry.\n        if function_upper != 0 && factored::defer_enumerating_reservation(function, budget)? {\n            continue;\n        }"],[18833,"    all_blocks_reachable: bool,\n",""],[39971,"    skeleton_work_upper_with_selection(program, function, None)\n}\n\nfn skeleton_work_upper_with_selection(\n    program: &dyn crate::cleanup::ReplayContext,\n    function: &ResolvedFunction,\n    selection: Option<&ReplayBudget>,\n) -> Result<usize, Diagnostic> {\n",""],[42259,"let upper = ",""],[42453,"?;\n    if let Some(selection) = selection {\n        if upper != 0\n            && factored::strategy::select_from_enumerating_census(\n                function,\n                &cfg,\n                semantic_paths,\n                selection,\n            )\n        {\n            return Ok(0);\n        }\n    }\n    Ok(upper)",""],[158761,"                            } else if let Some(op) = (work.budget.selected()\n                                && program.retained_static_byte_iterators()\n                                && instance.is_none())\n                            .then(|| crate::iterator_ops::by_id(callee.as_str()))\n                            .flatten()\n                            {\n                                typed_storage::iterator_params(op, expression, work)?.into()\n",""],[234074,"if","let _ ="],[234081,".budget.selected()\n        && program.retained_static_byte_iterators()\n        && selected_profile::paid_children::byte_iterator::step(&scrutinee.ty)\n    {\n        return typed_storage::iterator_match(program, expression, scrutinee, arms, work)",""],[234326,"\n    }",""]]"####;

fn replay_inverse(actual: &str, edits: &Value) -> String {
    let rows = edits.as_array().unwrap();
    assert!(rows.len() <= 512);
    let mut before = actual.to_owned();
    let mut next = before.len();
    for row in rows.iter().rev() {
        let start = row[0].as_u64().unwrap() as usize;
        let after = row[1].as_str().unwrap();
        let end = start.checked_add(after.len()).unwrap();
        assert!(end <= next);
        assert_eq!(before.get(start..end), Some(after));
        before.replace_range(start..end, row[2].as_str().unwrap());
        assert!(before.len() <= 2_097_152);
        next = start;
    }
    before
}

fn physical_before(actual: &str) -> (String, Value) {
    assert_eq!(actual, PHYSICAL_CURRENT);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        super::reader::read(&root.join(SOURCE), actual.len(), LIVE_SHA, 2_097_152).unwrap(),
        actual
    );
    assert_eq!(
        super::reader::read(
            &root.join("e").join(LIVE_SHA),
            actual.len(),
            LIVE_SHA,
            2_097_152
        )
        .unwrap(),
        actual
    );
    let previous = super::reader::read(
        &root.join("e").join(PHYSICAL_SHA),
        325862,
        PHYSICAL_SHA,
        2_097_152,
    )
    .unwrap();
    let live_edits: Value = serde_json::from_str(LIVE_PREFIX).unwrap();
    assert_eq!(replay_inverse(actual, &live_edits), previous);

    let edits: Value = serde_json::from_str(PHYSICAL_PREFIX).unwrap();
    let before = replay_inverse(&previous, &edits);
    assert_eq!(before, NEXT_CURRENT);
    authenticate_next();
    (before, edits)
}

pub(crate) fn next_before(actual: &str) -> (String, Value) {
    let (physical, edits) = physical_before(actual);
    let (immediate, mut prefix) = next_before_943(&physical);
    prefix
        .as_array_mut()
        .unwrap()
        .extend(edits.as_array().unwrap().iter().cloned());
    let live: Value = serde_json::from_str(LIVE_PREFIX).unwrap();
    prefix
        .as_array_mut()
        .unwrap()
        .extend(live.as_array().unwrap().iter().cloned());
    (immediate, prefix)
}

pub(super) fn before_probe(actual: &str) -> (String, Value) {
    let (immediate, extra) = next_before(actual);
    let (prior, mut prefix) = before_probe_before_943(&immediate);
    prefix
        .as_array_mut()
        .unwrap()
        .extend(extra.as_array().unwrap().iter().cloned());
    authenticate_next();
    (prior, prefix)
}
pub(super) fn finish_probe(probe: &mut Value, actual: &str, prefix: Value) {
    let (physical_before, physical) = physical_before(actual);
    let (immediate, extra) = next_before_943(&physical_before);
    let original: Value = serde_json::from_str(PREFIX).unwrap();
    let mut combined = original.as_array().unwrap().clone();
    combined.extend(extra.as_array().unwrap().iter().cloned());
    combined.extend(physical.as_array().unwrap().iter().cloned());
    let live: Value = serde_json::from_str(LIVE_PREFIX).unwrap();
    combined.extend(live.as_array().unwrap().iter().cloned());
    assert_eq!(
        prefix,
        Value::Array(combined),
        "exact four complete source transports only"
    );
    finish_probe_before_943(probe, &immediate, original);
    let entire_immediate_probe = probe.clone();
    probe["inverse"]
        .as_array_mut()
        .unwrap()
        .extend(extra.as_array().unwrap().iter().cloned());
    probe["length"] = physical_before.len().into();
    probe["sha256"] = NEXT_SHA.into();
    assert!(probe.get("retained_pre_943_probe").is_none());
    probe["retained_pre_943_probe"] = entire_immediate_probe;
    let entire_physical_predecessor = probe.clone();
    probe["inverse"]
        .as_array_mut()
        .unwrap()
        .extend(physical.as_array().unwrap().iter().cloned());
    probe["length"] = 325862.into();
    probe["sha256"] = PHYSICAL_SHA.into();
    assert!(probe.get("retained_pre_physical_probe").is_none());
    probe["retained_pre_physical_probe"] = entire_physical_predecessor;
    let complete_previous_probe = probe.clone();
    probe["inverse"]
        .as_array_mut()
        .unwrap()
        .extend(live.as_array().unwrap().iter().cloned());
    probe["length"] = actual.len().into();
    probe["sha256"] = LIVE_SHA.into();
    assert!(probe.get("retained_pre_live_probe").is_none());
    probe["retained_pre_live_probe"] = complete_previous_probe;
    authenticate_next();
}
// New numeric banks append to allocation.rs; every old CI source byte and its
// existing inverse stay unchanged, after fresh physical and archive acquisition.
pub(super) fn allocation_input<'a>(source: &str, supplied: &'a str) -> std::borrow::Cow<'a, str> {
    if source != "src/cleanup_plan/replay/allocation.rs" {
        return std::borrow::Cow::Borrowed(supplied);
    }
    const ACTUAL: &str = include_str!("../cleanup_plan/replay/allocation.rs");
    const OLD: &str =
        include_str!("../../e/d3f844655708972aa9e089c39923ddced6891b6b776f483d4f0be07e43cceb59");
    const ACTUAL_SHA: &str = "0d9c44752ba86a56eb75d5a77cbe97f60a7152dce52530daf9b5c06066faeda5";
    assert_eq!(supplied, ACTUAL);
    let acquired = super::current_source_predecessor(source, ACTUAL);
    let preceding = acquired.as_ref();
    assert_eq!(
        (preceding.len(), super::digest(preceding)),
        (12094, ACTUAL_SHA.to_owned())
    );
    assert_eq!(&preceding[..OLD.len()], OLD);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..2 {
        assert_eq!(
            super::reader::read(
                &root.join(format!("e/{ACTUAL_SHA}")),
                preceding.len(),
                ACTUAL_SHA,
                2_097_152
            )
            .unwrap(),
            preceding
        );
        assert_eq!(
            super::reader::read(
                &root.join("e/d3f844655708972aa9e089c39923ddced6891b6b776f483d4f0be07e43cceb59"),
                OLD.len(),
                "d3f844655708972aa9e089c39923ddced6891b6b776f483d4f0be07e43cceb59",
                2_097_152
            )
            .unwrap(),
            OLD
        );
    }
    std::borrow::Cow::Borrowed(OLD)
}

// A later complete marks owner enters the unchanged #913 inverse only through
// its exact whole immediate input, with both live and archive channels checked.
pub(super) fn batch_compiled_input<'a>(
    source: &str,
    supplied: &'a str,
) -> std::borrow::Cow<'a, str> {
    if source != "src/hir/validation/scope_checkpoint/marks.rs" {
        return std::borrow::Cow::Borrowed(supplied);
    }
    const ACTUAL: &str = include_str!("../hir/validation/scope_checkpoint/marks.rs");
    const OLD: &str =
        include_str!("../../e/2965bb2d4a43a3ca060b21db1a7ad11ca0af3a7e5061e48832366a833bca3c0e");
    assert_eq!(
        supplied, ACTUAL,
        "complete compiled marks input before fixed #913 source"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let authenticate = || {
        assert_eq!(
            super::reader::read(
                &root.join(source),
                ACTUAL.len(),
                "8d1919c8eb29198b0111a3759a8058ef7f2995365abea78b3a9d071f1a9a5f3d",
                2_097_152
            )
            .unwrap(),
            ACTUAL
        );
        assert_eq!(
            super::reader::read(
                &root.join("e/8d1919c8eb29198b0111a3759a8058ef7f2995365abea78b3a9d071f1a9a5f3d"),
                ACTUAL.len(),
                "8d1919c8eb29198b0111a3759a8058ef7f2995365abea78b3a9d071f1a9a5f3d",
                2_097_152
            )
            .unwrap(),
            ACTUAL
        );
        assert_eq!(
            super::reader::read(
                &root.join("e/2965bb2d4a43a3ca060b21db1a7ad11ca0af3a7e5061e48832366a833bca3c0e"),
                14549,
                "2965bb2d4a43a3ca060b21db1a7ad11ca0af3a7e5061e48832366a833bca3c0e",
                2_097_152
            )
            .unwrap(),
            OLD
        );
    };
    authenticate();
    let edits: Value = serde_json::from_str(r########"[[305,464,"        #[cfg(test)]\n        if direct_tests::before_selected() {\n            return self.mark_value_sources_moved_before_direct(expression, scope);\n        }\n",""],[2349,4430,"            // This is the original terminal arm, not a second validation or\n            // scratch borrower. The existing prepaid kind/Place loans, local\n            // Owned root and every semantic call retain their exact roles.\n            // Only the two first backing constructors, their push/index/pop\n            // transports and their eventual backing drops are absent.\n            ResolvedExprKind::Place(place) => {\n                let mut root = scope.take_owned();\n                let Some(binding) = root.get(&place.root) else {\n                    scope.replace_owned(root);\n                    return Ok(());\n                };\n                let (place_ty, place_ownership) = self.resolve_place(place, binding)?;\n                let should_move = self.is_owned_resource(&place_ty, place_ownership)?\n                    && Self::place_availability(binding, &place.projections)\n                        == Availability::Available;\n                if should_move {\n                    if self.binding_has_live_overlapping_loans(\n                        binding,\n                        place,\n                        &expression.id,\n                        LoanPointPhase::Before,\n                    )? {\n                        return Err(hir_error(\n                            \"SPX-T265: move or transfer would invalidate a lexical byte view\",\n                        ));\n                    }\n                    let binding = root.get_mut(&place.root)?.ok_or_else(|| {\n                        hir_error(format!(\n                            \"resolved value `{}` disappeared during ownership validation\",\n                            place.root\n                        ))\n                    })?;\n                    if place.projections.is_empty() {\n                        binding.availability = Availability::Moved;\n                    } else {\n                        scope_checkpoint::move_projection(binding, &place.projections)?;\n                    }\n                }\n                scope.replace_owned(root);\n                return Ok(());\n            }\n",""],[5960,5960,"","            | ResolvedExprKind::Place(_)\n"],[16748,16812,"\n#[cfg(test)]\nmod direct_before;\n#[cfg(test)]\nmod direct_tests;\n",""]]"########).unwrap();
    let edits = edits.as_array().unwrap();
    assert!(edits.len() <= 512);
    let mut restored = supplied.to_owned();
    let mut next = restored.len();
    for edit in edits.iter().rev() {
        assert_eq!(edit.as_array().unwrap().len(), 4);
        let lo = usize::try_from(edit[0].as_u64().unwrap()).unwrap();
        let hi = usize::try_from(edit[1].as_u64().unwrap()).unwrap();
        assert!(lo <= hi && hi <= next);
        assert_eq!(restored.get(lo..hi), Some(edit[2].as_str().unwrap()));
        restored.replace_range(lo..hi, edit[3].as_str().unwrap());
        assert!(restored.len() <= 2_097_152);
        next = lo;
    }
    assert_eq!(
        restored, OLD,
        "ENTIRE fixed marks source before unchanged old edits"
    );
    authenticate();
    std::borrow::Cow::Owned(restored)
}
