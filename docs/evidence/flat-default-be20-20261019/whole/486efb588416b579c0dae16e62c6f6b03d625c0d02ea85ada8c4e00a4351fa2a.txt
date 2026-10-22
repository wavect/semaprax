//! One fixed metadata overlay. Current source admission follows composition.
use super::{digest, held_4a6eb, SOURCE_CAP};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const PAGE_CAP: usize = 262144;
// Bound from the final original-parser source union; this draft grants no paths.
const REPLACEMENTS: &[&str] = &[
    "CHANGELOG.md",
    "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
    "src/cleanup_plan/replay.rs",
    "src/cleanup_plan/replay/branch_clone/direct.rs",
    "src/cleanup_plan/replay/branch_clone/direct/before.rs",
    "src/cleanup_plan/replay/branch_clone/direct/tests.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs",
    "src/cleanup_plan/replay/skeleton_work.rs",
    "src/source_reflows.rs",
    "src/source_reflows_merge/main67107_inputs.rs",
    "src/source_reflows_merge/main99f_inputs.rs",
    "tests/hir_module_boundaries.rs",
    "tests/workspace/semantic_graph.rs",
    "tests/workspace/semantic_graph/source_inventory.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/before_selected_route.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs",
    "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs",
    "src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs",
    "src/cleanup_plan/replay/expression_facts/arena/retained.rs",
    "src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs",
    "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity.rs",
    "src/cleanup/schema_summary/tests.rs",
    "src/cleanup_plan/replay/live_oracle/source_custody/a.rs",
    "src/cleanup_plan/replay/live_oracle/source_custody/b.rs",
    "src/cleanup_plan/replay/physical_budget.rs",
    "src/hir/ids/fixed_operation_tests.rs",
    "src/hir/lexical_exit.rs",
    "src/hir/resolve_program.rs",
    "src/source_reflows/incoming090_semantic_receivers.rs",
    "src/source_reflows_merge/current344.rs",
    "src/source_reflows_merge/final_ready_inputs.rs",
    "src/source_reflows_merge/main_eb2_inputs.rs",
    "src/source_reflows_merge/ordinary_plan_inputs.rs",
    "src/workspace_graph/source_schema/static_text/tests.rs",
    "tests/doctor_production_provisioner_v1.rs",
    "tests/quality_routing.rs",
    "tests/support/production_source_inventory.rs",
    "tests/wasm/scalar_browser_ci_contract.rs",
    "tests/wasm/scalar_browser_ci_contract/historical_builder.rs",
    "docs/evidence/main-e177-final-source-binding-20261017/current-projection-inputs.tsv",
    "src/bounded_output/stage_debits/route_debits.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/matched_receiving_layers/successor_targets.rs",
    "src/hir/ids/declaration_tag/before.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before.rs",
    "src/cleanup_plan/replay/inventory_coverage/paid/queries.rs",
    "src/cleanup_plan/replay/inventory_coverage/paid_before.rs",
    "src/cleanup_plan/replay/inventory_coverage/typed_tests/selected_family.rs",
    "src/cleanup_plan/replay/path_join/selected.rs",
    "src/cleanup_plan/replay/path_join/selected/joins.rs",
    "src/cleanup_plan/replay/path_join/selected/lexical_flow.rs",
    "src/cleanup_plan/replay/path_join/selected/queries.rs",
    "src/cleanup_plan/replay/path_join/selected/renewal.rs",
    "src/cleanup_plan/replay/path_join/selected/strings.rs",
    "src/cleanup_plan/replay/path_join/selected/tests/history.rs",
    "src/cleanup_plan/replay/path_join/selected/tests/lookup.rs",
    "src/cleanup_plan/replay/path_join/selected/walk.rs",
    "src/cleanup_plan/replay/schema/selected_paid.rs",
    "src/cleanup_plan/replay/selected_entry/tests/paid/selected_family.rs",
    "src/cleanup_plan/replay/structural_control/selected.rs",
    "src/cleanup_plan/replay/structural_control/selected/blocks.rs",
    "src/cleanup_plan/replay/structural_control/selected/calls.rs",
    "src/cleanup_plan/replay/structural_control/selected/helpers.rs",
    "src/cleanup_plan/replay/structural_control/selected/place_before_formals.rs",
    "src/cleanup_plan/replay/structural_control/selected/tests/boundary.rs",
    "src/cleanup_plan/replay/inventory_coverage.rs",
    "src/cleanup_plan/replay/inventory_coverage/paid.rs",
];
const ADDITIONS: &[&str] = &[
    "src/cleanup_plan/renewal_profile.rs",
    "src/cleanup_plan/renewal_profile/funded.rs",
    "src/cleanup_plan/renewal_profile/funded_receiving.rs",
    "src/cleanup_plan/replay/expression_facts/arena/retained/into_tests.rs",
    "src/cleanup_plan/replay/expression_facts/arena/retained_into.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup_before_receiving.rs",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before_receiving.rs",
    "src/cleanup_plan/replay/ordinary_receiving.rs",
    "src/cleanup_plan/replay/ordinary_receiving/caller_before.rs",
    "src/cleanup_plan/replay/ordinary_receiving/tests.rs",
    "src/cleanup_plan/replay/skeleton_work/dag_storage.rs",
    "src/cleanup_plan/replay/skeleton_work/receiving.rs",
    "src/hir/lexical_exit/owner_funded.rs",
    "src/hir/lexical_exit/owner_funded_receiving.rs",
    "src/literal_format.rs",
    "src/literal_format/funded.rs",
    "src/literal_format/funded_receiving.rs",
    "src/string_ops/conditions.rs",
    "src/string_ops/conditions/funded.rs",
    "src/string_ops/conditions/funded_receiving.rs",
    "src/cleanup_plan/replay/expression_facts/errors.rs",
    "docs/ORDINARY-REPLAY-RECEIVING-V1.md",
    "src/hir/ids/declaration_backing_tests.rs",
    "src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs",
];
const POLICY_PATH: &str = "docs/evidence/finite-source-overlay-v2-20261018/policy.json";
const PAGE_PATH: &str = "docs/evidence/finite-source-overlay-v2-20261018/page.json";
const POLICY_LENGTH: usize = 16328; // Filled from the complete literal policy below.
const POLICY_SHA256: &str = "4a2afe363442ed139c6c2ed42e561d265f5116ba0f9b9f5a92b80f98a41f78b6";
const EXTENSION_PATH: &str = "docs/evidence/fmt-local-441-20261018/extension.json";
const EXTENSION_LENGTH: usize = 262076;
const EXTENSION_SHA256: &str = "2af7299253e5cfb027186ba4fc151011a937214ff381c9267bfc11ef7d0003b6";

macro_rules! require {
    ($condition:expr, $message:literal) => {
        if !$condition {
            return Err($message);
        }
    };
}
fn keys(value: &Value, expected: &[&str]) -> bool {
    value.as_object().is_some_and(|v| {
        v.len() == expected.len() && expected.iter().all(|key| v.contains_key(*key))
    })
}
fn normal(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':', '\0'])
        && path
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}
fn control(path: &str) -> bool {
    [
        POLICY_PATH,
        PAGE_PATH,
        EXTENSION_PATH,
        "src/source_reflows_merge/main_e177_final_inputs.rs",
        "src/source_reflows_merge/main_e177_overlay_v2.rs",
        "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py",
        "docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py",
        "docs/evidence/merge-2e92-browser-20261018/guard-config.json",
    ]
    .contains(&path)
}
// Canonical object keys only; arrays (including every inventory) keep their order.
fn canonical(value: &Value) -> Vec<u8> {
    fn write(v: &Value, out: &mut String) {
        match v {
            Value::Object(map) => {
                out.push('{');
                for (i, (key, value)) in map.iter().collect::<BTreeMap<_, _>>().iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(key).unwrap());
                    out.push(':');
                    write(value, out);
                }
                out.push('}');
            }
            Value::Array(array) => {
                out.push('[');
                for (i, value) in array.iter().enumerate() {
                    if i != 0 {
                        out.push(',');
                    }
                    write(value, out);
                }
                out.push(']');
            }
            _ => out.push_str(&serde_json::to_string(v).unwrap()),
        }
    }
    let mut result = String::new();
    write(value, &mut result);
    result.push('\n');
    result.into_bytes()
}
fn json(raw: &str) -> Result<Value, &'static str> {
    let value: Value = serde_json::from_str(raw).map_err(|_| "JSON syntax")?;
    // This also rejects duplicate keys, alternate escaping, trailing bytes and
    // alternate whitespace: none can reserialize to these exact complete bytes.
    require!(
        canonical(&value) == raw.as_bytes(),
        "noncanonical or duplicate JSON"
    );
    Ok(value)
}
fn pin(value: &Value) -> Result<(usize, &str, &str), &'static str> {
    let length =
        usize::try_from(value["length"].as_u64().ok_or("length")?).map_err(|_| "length")?;
    let sha = value["sha256"].as_str().ok_or("digest")?;
    let path = value["archive"].as_str().ok_or("archive")?;
    require!(
        length <= SOURCE_CAP && normal(path),
        "source bound or archive path"
    );
    require!(
        sha.len() == 64
            && sha
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "digest"
    );
    Ok((length, sha, path))
}
fn image(value: &Value) -> Result<String, &'static str> {
    let (length, sha, path) = pin(value)?;
    held_4a6eb(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
        length,
        sha,
    )
}
fn page_bytes(
    compiled: &str,
    physical: &str,
    length: usize,
    sha: &str,
) -> Result<(), &'static str> {
    require!(
        length <= PAGE_CAP && physical.len() == length,
        "page bound or length"
    );
    require!(
        compiled == physical && digest(physical.as_bytes()) == sha,
        "page compiled/physical/digest"
    );
    Ok(())
}
fn load() -> Result<(Value, Value), &'static str> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiled =
        include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/policy.json");
    require!(POLICY_LENGTH <= PAGE_CAP, "policy bound");
    let raw = held_4a6eb(&root.join(POLICY_PATH), POLICY_LENGTH, POLICY_SHA256)?;
    require!(raw == compiled, "policy compiled/physical");
    let policy = json(&raw)?;
    require!(
        keys(
            &policy,
            &[
                "schema",
                "page",
                "predecessor",
                "replacements",
                "additions",
                "new_fragment_edges"
            ]
        ),
        "policy keys"
    );
    require!(
        policy["schema"] == "semaprax.finite-source-overlay-policy.v2",
        "policy schema"
    );
    require!(
        keys(&policy["page"], &["path", "length", "sha256"]) && policy["page"]["path"] == PAGE_PATH,
        "one literal page"
    );
    require!(
        keys(
            &policy["predecessor"],
            &["extension", "rows", "logical_sha256"]
        ),
        "predecessor keys"
    );
    require!(policy["predecessor"]["rows"] == 519, "predecessor rows");
    require!(
        policy["predecessor"]["extension"]
            == serde_json::json!({"path":EXTENSION_PATH,"length":EXTENSION_LENGTH,"sha256":EXTENSION_SHA256}),
        "predecessor extension"
    );
    require!(
        policy["replacements"] == serde_json::json!(REPLACEMENTS),
        "literal replacement scope"
    );
    require!(
        policy["additions"]
            .as_array()
            .ok_or("additions")?
            .iter()
            .map(|r| r["source"].as_str())
            .collect::<Vec<_>>()
            == ADDITIONS.iter().map(|p| Some(*p)).collect::<Vec<_>>(),
        "literal addition scope"
    );
    // Original metadata is read as metadata. It is never substituted for a
    // changed physical Rust source, and it retains its original individual cap.
    let extension = include_str!("../../docs/evidence/fmt-local-441-20261018/extension.json");
    require!(
        held_4a6eb(
            &root.join(EXTENSION_PATH),
            EXTENSION_LENGTH,
            EXTENSION_SHA256
        )? == extension,
        "predecessor physical/compiled"
    );
    let length = usize::try_from(policy["page"]["length"].as_u64().ok_or("page length")?)
        .map_err(|_| "page length")?;
    let sha = policy["page"]["sha256"].as_str().ok_or("page digest")?;
    require!(length <= PAGE_CAP, "page bound");
    let physical = held_4a6eb(&root.join(PAGE_PATH), length, sha)?;
    let compiled = include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/page.json");
    page_bytes(compiled, &physical, length, sha)?;
    Ok((policy, json(&physical)?))
}
fn edges(packet: &Value) -> Result<Vec<(String, String)>, &'static str> {
    let rows = packet["rows"].as_array().ok_or("rows")?;
    let by: BTreeMap<_, _> = rows
        .iter()
        .map(|r| (r["source"].as_str().unwrap_or(""), r))
        .collect();
    require!(
        by.len() == rows.len() && !by.contains_key(""),
        "duplicate source"
    );
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for row in rows {
        if let Some(children) = row.get("fragments") {
            for child in children.as_array().ok_or("fragments")? {
                let source = child["source"].as_str().ok_or("fragment source")?;
                require!(
                    child.get("fragments").is_none() && child["compiled"] == true,
                    "noncompiled or nested fragment"
                );
                require!(
                    by.get(source) == Some(&child) && row["source"] != source,
                    "fragment canonical row or self edge"
                );
                let edge = (
                    row["source"].as_str().ok_or("parent")?.to_owned(),
                    source.to_owned(),
                );
                require!(seen.insert(edge.clone()), "duplicate fragment edge");
                result.push(edge);
            }
        }
    }
    Ok(result)
}
fn compose_values(before: Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
    require!(
        keys(
            page,
            &[
                "schema",
                "index",
                "total_pages",
                "predecessor",
                "replacements",
                "additions"
            ]
        ),
        "page keys"
    );
    require!(
        page["schema"] == "semaprax.finite-source-overlay-page.v2"
            && page["index"] == 1
            && page["total_pages"] == 1,
        "one page schema/index"
    );
    require!(
        page["predecessor"] == policy["predecessor"],
        "wrong predecessor"
    );
    require!(
        digest(&canonical(&before)) == policy["predecessor"]["logical_sha256"],
        "whole predecessor logical digest"
    );
    let old = before["rows"].as_array().ok_or("old rows")?;
    require!(
        old.len() as u64 == policy["predecessor"]["rows"].as_u64().ok_or("old count")?,
        "old count"
    );
    require!(
        before["source_cap"].as_u64() == Some(SOURCE_CAP as u64),
        "unchanged source cap domain"
    );
    let mut result = before.clone();
    let rows = result["rows"].as_array_mut().ok_or("rows")?;
    let positions: BTreeMap<_, _> = old
        .iter()
        .enumerate()
        .map(|(i, r)| (r["source"].as_str().unwrap_or(""), i))
        .collect();
    require!(
        positions.len() == old.len() && !positions.contains_key(""),
        "old uniqueness"
    );
    let replacements = page["replacements"].as_array().ok_or("replacement rows")?;
    let paths = policy["replacements"]
        .as_array()
        .ok_or("replacement paths")?;
    require!(paths.len() == replacements.len(), "missing replacement");
    let mut previous = None;
    let mut seen = BTreeSet::new();
    for (row, path) in replacements.iter().zip(paths) {
        let source = path.as_str().ok_or("replacement path")?;
        require!(
            normal(source) && !control(source) && row["source"] == source && seen.insert(source),
            "foreign/duplicate/control source"
        );
        let index = *positions.get(source).ok_or("foreign replacement")?;
        require!(previous.is_none_or(|p| p < index), "reordered replacements");
        previous = Some(index);
        let prior = &old[index];
        require!(
            row["compiled"] == prior["compiled"],
            "classification changed"
        );
        let views = row["views"].as_array().ok_or("views")?;
        let old_views = prior["views"].as_array().ok_or("old views")?;
        require!(
            views.len() >= old_views.len() && &views[..old_views.len()] == old_views.as_slice(),
            "old views lost/reordered/changed"
        );
        pin(&row["current"])?;
        for view in &views[old_views.len()..] {
            pin(view)?;
        }
        if row["current"] != prior["current"] {
            let mut retained = prior["current"].clone();
            retained["name"] = "whole-pre-overlay-v2-current".into();
            require!(
                views[old_views.len()..].contains(&retained),
                "old current descriptor lost"
            );
        }
        let mut restored = row.clone();
        restored["current"] = prior["current"].clone();
        restored["views"] = prior["views"].clone();
        if let Some(children) = prior.get("fragments") {
            restored["fragments"] = children.clone();
        } else {
            restored.as_object_mut().ok_or("row")?.remove("fragments");
        }
        require!(
            &restored == prior,
            "whole replacement row inverse/unknown fields"
        );
        rows[index] = row.clone();
    }
    let additions = page["additions"].as_array().ok_or("addition rows")?;
    let bindings = policy["additions"].as_array().ok_or("addition bindings")?;
    require!(additions.len() == bindings.len(), "missing addition");
    for (row, binding) in additions.iter().zip(bindings) {
        require!(keys(binding, &["source", "compiled"]), "addition binding");
        let source = binding["source"].as_str().ok_or("addition source")?;
        require!(
            normal(source)
                && !control(source)
                && row["source"] == source
                && !positions.contains_key(source)
                && seen.insert(source),
            "foreign/duplicate addition"
        );
        require!(
            binding["compiled"].is_boolean() && row["compiled"] == binding["compiled"],
            "addition classification"
        );
        pin(&row["current"])?;
        for view in row["views"].as_array().ok_or("addition views")? {
            pin(view)?;
        }
        rows.push(row.clone());
    }
    require!(
        rows.len()
            == old
                .len()
                .checked_add(additions.len())
                .ok_or("count overflow")?,
        "count"
    );
    let prior_edges = edges(&before)?;
    let current_edges = edges(&result)?;
    require!(
        current_edges
            .iter()
            .filter(|e| prior_edges.contains(e))
            .cloned()
            .collect::<Vec<_>>()
            == prior_edges,
        "old fragment edges lost/reordered"
    );
    let additions = current_edges
        .iter()
        .filter(|e| !prior_edges.contains(e))
        .map(|(parent, child)| serde_json::json!({"parent":parent,"child":child}))
        .collect::<Vec<_>>();
    require!(
        policy["new_fragment_edges"] == Value::Array(additions),
        "undeclared fragment edges"
    );
    let mut restored = result.clone();
    restored["rows"] = before["rows"].clone();
    require!(restored == before, "ENTIRE predecessor object inverse");
    Ok(result)
}
pub(super) fn compose(before: Value) -> Value {
    let (policy, page) = load().unwrap();
    compose_values(before, &policy, &page).unwrap()
}
fn affected(policy: &Value, source: &str) -> bool {
    policy["replacements"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == source)
        || policy["additions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["source"] == source)
}
fn current_identity(
    row: &Value,
    physical: &str,
    compiled: Option<&str>,
    archive: &str,
) -> Result<(), &'static str> {
    if row["compiled"] == true {
        require!(
            compiled == Some(physical),
            "complete current compiled source"
        );
    }
    require!(archive == physical, "complete current archive");
    Ok(())
}
fn current_tree(
    row: &Value,
    read_source: &impl Fn(&Value) -> Result<String, &'static str>,
    compiled: &impl Fn(&str) -> Option<&'static str>,
    read_archive: &impl Fn(&Value) -> Result<String, &'static str>,
) -> Result<String, &'static str> {
    let raw = read_source(row)?;
    let source = row["source"].as_str().ok_or("source")?;
    current_identity(row, &raw, compiled(source), &read_archive(&row["current"])?)?;
    for child in row
        .get("fragments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        current_tree(child, read_source, compiled, read_archive)?;
    }
    Ok(raw)
}
fn current(row: &Value) -> Result<String, &'static str> {
    current_tree(
        row,
        &|row| {
            let (length, sha, _) = pin(&row["current"])?;
            held_4a6eb(
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(row["source"].as_str().ok_or("source")?),
                length,
                sha,
            )
        },
        &super::compiled,
        &image,
    )
}
fn affected_row(source: &str) -> Option<Value> {
    if !REPLACEMENTS.contains(&source) && !ADDITIONS.contains(&source) {
        return None;
    }
    let (policy, page) = load().unwrap();
    if !affected(&policy, source) {
        return None;
    }
    // Metadata replay precedes current validation. No old physical source is read
    // to establish the current source; only the composed row is then admitted.
    let packet = compose_values(super::packet_before_overlay(), &policy, &page).unwrap();
    Some(
        packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == source)
            .unwrap()
            .clone(),
    )
}
pub(super) fn held_source(
    path: &Path,
    length: usize,
    expected: &str,
) -> Option<Result<String, &'static str>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).ok()?.to_str()?;
    if !normal(relative) {
        return None;
    }
    let row = affected_row(relative)?;
    Some((|| {
        let actual = current(&row)?;
        if row["current"]["length"].as_u64() == Some(length as u64)
            && row["current"]["sha256"] == expected
        {
            return Ok(actual);
        }
        let prior = row["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["length"].as_u64() == Some(length as u64) && p["sha256"] == expected)
            .ok_or("foreign overlay tuple")?;
        image(prior)
    })())
}
fn stage(row: &Value) -> Option<Value> {
    let source = row["source"].as_str().unwrap();
    let current_row = affected_row(source)?;
    let canonical = super::clippy_stage::rows();
    assert_eq!(
        canonical["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == source),
        Some(row),
        "closed original stage row"
    );
    current(&current_row).unwrap();
    Some(current_row)
}
pub(super) fn stage_current(row: &Value) -> Option<String> {
    stage(row)?;
    Some(image(&row["current"]).unwrap())
}
pub(super) fn stage_normalize(row: &Value, supplied: &str) -> Option<String> {
    let current_row = stage(row)?;
    let raw = current(&current_row).unwrap();
    if supplied == raw {
        Some(image(&row["before"]).unwrap())
    } else {
        None
    }
}
pub(super) fn stage_requested(
    row: &Value,
    expected: &str,
    supplied: &str,
) -> Option<Option<String>> {
    let current_row = stage(row)?;
    let raw = current(&current_row).unwrap();
    let selected = [&row["before"], &row["current"]]
        .into_iter()
        .chain(
            row.get("views")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        )
        .find(|p| p["sha256"] == expected);
    Some(selected.and_then(|p| {
        let prior = image(p).unwrap();
        if supplied == raw || supplied == prior {
            Some(prior)
        } else {
            None
        }
    }))
}
pub(super) fn compiled(source: &str) -> Option<&'static str> {
    match source {
        "src/source_reflows_merge/main_e177_overlay_v2.rs" => {
            Some(include_str!("main_e177_overlay_v2.rs"))
        }
        "src/cleanup_plan/replay/expression_facts/errors.rs" => Some(include_str!(
            "../../src/cleanup_plan/replay/expression_facts/errors.rs"
        )),
        "src/cleanup_plan/renewal_profile.rs" => Some(include_str!("../../src/cleanup_plan/renewal_profile.rs")),
        "src/cleanup_plan/renewal_profile/funded.rs" => Some(include_str!("../../src/cleanup_plan/renewal_profile/funded.rs")),
        "src/cleanup_plan/renewal_profile/funded_receiving.rs" => Some(include_str!("../../src/cleanup_plan/renewal_profile/funded_receiving.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/retained/into_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/retained/into_tests.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/retained_into.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/retained_into.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests/receiving.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup_before_receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup_before_receiving.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before_receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_before_receiving.rs")),
        "src/cleanup_plan/replay/ordinary_receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_receiving.rs")),
        "src/cleanup_plan/replay/ordinary_receiving/caller_before.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_receiving/caller_before.rs")),
        "src/cleanup_plan/replay/ordinary_receiving/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_receiving/tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/dag_storage.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/dag_storage.rs")),
        "src/cleanup_plan/replay/skeleton_work/receiving.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/receiving.rs")),
        "src/hir/lexical_exit/owner_funded.rs" => Some(include_str!("../../src/hir/lexical_exit/owner_funded.rs")),
        "src/hir/lexical_exit/owner_funded_receiving.rs" => Some(include_str!("../../src/hir/lexical_exit/owner_funded_receiving.rs")),
        "src/literal_format.rs" => Some(include_str!("../../src/literal_format.rs")),
        "src/literal_format/funded.rs" => Some(include_str!("../../src/literal_format/funded.rs")),
        "src/literal_format/funded_receiving.rs" => Some(include_str!("../../src/literal_format/funded_receiving.rs")),
        "src/string_ops/conditions.rs" => Some(include_str!("../../src/string_ops/conditions.rs")),
        "src/string_ops/conditions/funded.rs" => Some(include_str!("../../src/string_ops/conditions/funded.rs")),
        "src/string_ops/conditions/funded_receiving.rs" => Some(include_str!("../../src/string_ops/conditions/funded_receiving.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs" => Some(include_str!(
            "../../src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs"
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn record(source: &str, value: &str) -> Value {
        serde_json::json!({"source":source,"compiled":false,
            "current":{"length":value.len(),"sha256":digest(value.as_bytes()),"archive":format!("e/{}",digest(value.as_bytes()))},
            "views":[],"unknown_metadata":{"retained":[3,1,2]}})
    }
    fn fixture() -> (Value, Value, Value) {
        let before = serde_json::json!({"schema":"fixture","source_cap":SOURCE_CAP,
            "unknown_root":{"order":[4,2,1]},"rows":[record("src/one.rs","old"),record("src/two.rs","two")]});
        let predecessor = serde_json::json!({"extension":{"path":EXTENSION_PATH,"length":EXTENSION_LENGTH,"sha256":EXTENSION_SHA256},"rows":2,"logical_sha256":digest(&canonical(&before))});
        let mut row = before["rows"][0].clone();
        row["current"] = record("src/one.rs", "new")["current"].clone();
        let mut old = before["rows"][0]["current"].clone();
        old["name"] = "whole-pre-overlay-v2-current".into();
        row["views"].as_array_mut().unwrap().push(old);
        let page = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2","index":1,"total_pages":1,"predecessor":predecessor,"replacements":[row],"additions":[record("src/three.rs","three")]});
        let policy = serde_json::json!({"schema":"semaprax.finite-source-overlay-policy.v2","predecessor":predecessor,"replacements":["src/one.rs"],"additions":[{"source":"src/three.rs","compiled":false}],"new_fragment_edges":[]});
        (before, policy, page)
    }
    #[test]
    fn finite_page_retains_the_entire_predecessor_object_and_unknown_fields() {
        let (before, policy, page) = fixture();
        let result = compose_values(before.clone(), &policy, &page).unwrap();
        assert_eq!(result["rows"][1], before["rows"][1]);
        assert_eq!(
            result["rows"][0]["unknown_metadata"],
            before["rows"][0]["unknown_metadata"]
        );
        assert_eq!(result["unknown_root"], before["unknown_root"]);
        assert_eq!(
            result["rows"][0]["views"][0]["archive"],
            before["rows"][0]["current"]["archive"]
        );
        let mut inverse = result;
        inverse["rows"] = before["rows"].clone();
        assert_eq!(inverse, before);
    }
    #[test]
    fn finite_page_refuses_wrong_schema_predecessor_scope_order_and_unknown_keys() {
        let (before, policy, page) = fixture();
        let mut bad = Vec::new();
        for (key, value) in [
            ("schema", serde_json::json!("foreign")),
            ("index", serde_json::json!(2)),
            ("total_pages", serde_json::json!(2)),
            ("unexpected", serde_json::json!(true)),
        ] {
            let mut p = page.clone();
            p[key] = value;
            bad.push(p);
        }
        let mut p = page.clone();
        p["predecessor"]["logical_sha256"] = "0".repeat(64).into();
        bad.push(p);
        let mut p = page.clone();
        p["replacements"] = serde_json::json!([]);
        bad.push(p);
        let mut p = page.clone();
        p["replacements"]
            .as_array_mut()
            .unwrap()
            .push(page["replacements"][0].clone());
        bad.push(p);
        for source in [
            "src/foreign.rs",
            "../src/one.rs",
            "/src/one.rs",
            "src//one.rs",
            PAGE_PATH,
        ] {
            let mut p = page.clone();
            p["replacements"][0]["source"] = source.into();
            bad.push(p);
        }
        for p in bad {
            assert!(compose_values(before.clone(), &policy, &p).is_err());
        }
        let mut reordered_policy = policy.clone();
        reordered_policy["replacements"] = serde_json::json!(["src/two.rs", "src/one.rs"]);
        let mut reordered = page;
        reordered["replacements"] = serde_json::json!([
            before["rows"][1].clone(),
            reordered["replacements"][0].clone()
        ]);
        assert!(compose_values(before, &reordered_policy, &reordered).is_err());
    }
    #[test]
    fn finite_page_refuses_lost_views_old_current_classification_and_unknown_fields() {
        let (before, policy, page) = fixture();
        let mut bad = Vec::new();
        let mut p = page.clone();
        p["replacements"][0]["views"] = serde_json::json!([]);
        bad.push(p);
        let mut p = page.clone();
        p["replacements"][0]["views"][0]["archive"] = "e/same-payload-different-channel".into();
        bad.push(p);
        let mut p = page.clone();
        p["replacements"][0]["compiled"] = true.into();
        bad.push(p);
        let mut p = page.clone();
        p["replacements"][0]
            .as_object_mut()
            .unwrap()
            .remove("unknown_metadata");
        bad.push(p);
        let mut p = page.clone();
        p["replacements"][0]["unknown_metadata"]["retained"] = serde_json::json!([1, 2, 3]);
        bad.push(p);
        let mut p = page.clone();
        p["additions"][0]["compiled"] = true.into();
        bad.push(p);
        for p in bad {
            assert!(compose_values(before.clone(), &policy, &p).is_err());
        }
        let mut old = before;
        old["rows"][0]["views"] = serde_json::json!([{"name":"first"},{"name":"second"}]);
        let mut p = page;
        p["replacements"][0]["views"]
            .as_array_mut()
            .unwrap()
            .insert(0, serde_json::json!({"name":"second"}));
        p["replacements"][0]["views"]
            .as_array_mut()
            .unwrap()
            .insert(1, serde_json::json!({"name":"first"}));
        let mut policy = policy;
        policy["predecessor"]["logical_sha256"] = digest(&canonical(&old)).into();
        p["predecessor"] = policy["predecessor"].clone();
        assert!(compose_values(old, &policy, &p).is_err());
    }
    #[test]
    fn finite_page_refuses_nonleaf_duplicate_forged_and_undeclared_fragments() {
        let (before, policy, page) = fixture();
        for child in [
            serde_json::json!({"source":"src/one.rs","compiled":true}),
            serde_json::json!({"source":"src/missing.rs","compiled":true}),
            page["additions"][0].clone(),
        ] {
            let mut p = page.clone();
            p["replacements"][0]["fragments"] = serde_json::json!([child]);
            assert!(compose_values(before.clone(), &policy, &p).is_err());
        }
        // A genuine declared leaf edge has to use the complete same-scope row.
        let mut p = page;
        p["additions"][0]["compiled"] = true.into();
        let child = p["additions"][0].clone();
        p["replacements"][0]["fragments"] = serde_json::json!([child]);
        let mut policy = policy;
        policy["additions"][0]["compiled"] = true.into();
        policy["new_fragment_edges"] =
            serde_json::json!([{"parent":"src/one.rs","child":"src/three.rs"}]);
        assert!(compose_values(before.clone(), &policy, &p).is_ok());
        p["replacements"][0]["fragments"][0]["fragments"] = serde_json::json!([]);
        assert!(compose_values(before, &policy, &p).is_err());
    }
    #[test]
    fn finite_page_exact_bound_compiled_physical_and_json_hostiles_are_independent() {
        let raw = "x".repeat(PAGE_CAP);
        let sha = digest(raw.as_bytes());
        assert!(page_bytes(&raw, &raw, PAGE_CAP, &sha).is_ok());
        assert!(page_bytes(&raw, &raw, PAGE_CAP + 1, &sha).is_err());
        assert!(page_bytes(&raw, &raw[..raw.len() - 1], PAGE_CAP, &sha).is_err());
        assert!(page_bytes("different", &raw, PAGE_CAP, &sha).is_err());
        assert!(page_bytes(&raw, &raw, PAGE_CAP, &"0".repeat(64)).is_err());
        assert!(json("{\"a\":1,\"a\":1}\n").is_err());
        assert!(json("{ \"a\":1}\n").is_err());
        assert!(json("{\"a\":1}\n ").is_err());
        assert!(json("{\"a\":1}\n").is_ok());
    }
    #[test]
    fn finite_page_source_cap_is_per_full_source_and_not_an_aggregate_budget() {
        let (before, policy, mut page) = fixture();
        page["replacements"][0]["current"]["length"] = (SOURCE_CAP + 1).into();
        assert!(compose_values(before.clone(), &policy, &page).is_err());
        page["replacements"][0]["current"]["length"] = SOURCE_CAP.into();
        page["additions"][0]["current"]["length"] = SOURCE_CAP.into();
        assert!(
            compose_values(before, &policy, &page).is_ok(),
            "two separately bounded sources do not share a new aggregate cap"
        );
    }
    #[test]
    fn finite_page_literal_policy_replays_the_complete_519_metadata_inverse() {
        let before = super::super::packet_before_overlay();
        let mut after = compose(before.clone());
        after["rows"] = before["rows"].clone();
        assert_eq!(after, before);
        let (policy, page) = load().unwrap();
        assert_eq!(page["predecessor"], policy["predecessor"]);
        assert_eq!(page["total_pages"], 1);
    }
    #[test]
    fn finite_page_current_compiled_and_archive_channels_are_required_before_history() {
        let mut row = record("src/one.rs", "new");
        row["compiled"] = true.into();
        assert!(current_identity(&row, "new", Some("new"), "new").is_ok());
        for (physical, compiled, archive) in [
            ("old", Some("new"), "new"),
            ("new", Some("old"), "new"),
            ("new", None, "new"),
            ("new", Some("new"), "old"),
        ] {
            assert!(current_identity(&row, physical, compiled, archive).is_err());
        }
        let mut root = record("src/root.rs", "root");
        root["compiled"] = true.into();
        let mut child = record("src/leaf.rs", "new");
        child["compiled"] = true.into();
        root["fragments"] = serde_json::json!([child]);
        for broken in ["physical", "compiled", "archive"] {
            let history = std::cell::Cell::new(false);
            let source = |r: &Value| {
                Ok(if r["source"] == "src/root.rs" {
                    "root".to_owned()
                } else if broken == "physical" {
                    "old".to_owned()
                } else {
                    "new".to_owned()
                })
            };
            let compiled = |path: &str| {
                Some(if path == "src/root.rs" {
                    "root"
                } else if broken == "compiled" {
                    "old"
                } else {
                    "new"
                })
            };
            let archive = |p: &Value| {
                Ok(if p["sha256"] == digest(b"root") {
                    "root".to_owned()
                } else if broken == "archive" {
                    "old".to_owned()
                } else {
                    "new".to_owned()
                })
            };
            let result = current_tree(&root, &source, &compiled, &archive).map(|_| {
                history.set(true);
                "historical"
            });
            assert!(result.is_err());
            assert!(
                !history.get(),
                "a changed leaf must refuse before historical selection"
            );
        }
    }
    #[test]
    fn finite_page_original_held_reader_keeps_nonregular_and_symlink_refusals() {
        let path = std::env::temp_dir().join(format!("semaprax-overlay-{}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        assert!(held_4a6eb(&path, 0, &digest(b"")).is_err());
        #[cfg(unix)]
        {
            let target = path.join("target");
            std::fs::write(&target, "actual").unwrap();
            let link = path.join("link");
            std::os::unix::fs::symlink(&target, &link).unwrap();
            assert!(held_4a6eb(&link, 6, &digest(b"actual")).is_err());
        }
        std::fs::remove_dir_all(path).unwrap();
    }
}
