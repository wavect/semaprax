//! Complete physical-input successors for the immutable CI and Source36 views.
//! This test-only projection owns no compiler authority and grants no admission.
#[path = "source_reflows_merge/constructor_stage.rs"]
mod constructor_stage;
#[path = "source_reflows_merge/joined_custody.rs"]
mod joined_custody;
#[path = "source_reflows_merge/prefix_custody.rs"]
mod prefix_custody;
#[path = "source_reflows_merge/release_custody.rs"]
mod release_custody;
#[path = "source_reflows_merge/successor_custody.rs"]
mod successor_custody;
use sha2::{Digest, Sha256};
#[path = "source_reflows_merge/final_custody.rs"]
mod final_custody;
#[path = "source_reflows_merge/metadata_json.rs"]
mod metadata_json;
#[path = "source_reflows_merge/runtime_json.rs"]
mod runtime_json;
// Closed authenticated repository DATA only; no live-source or compiler authority.
pub(crate) use runtime_json::{receiver_fixture, receiver_fixture_source_before, ReceiverFixture};
// The final compositor authenticates its raw view at this independent owner front.
pub(crate) fn boxed849_input(source: &str, current: &str) -> String {
    let compile_fixed = compile_fixture_input(source, current);
    let current = compile_fixed.as_str();
    let qualified = metadata_json::project(metadata_json::Fixture::M00, source, current);
    let current = qualified.as_str();
    let next = metadata_json::project(metadata_json::Fixture::M01, source, current);
    let current = next.as_str();
    let joined = metadata_json::project(metadata_json::Fixture::M02, source, current);
    let current = joined.as_str();
    metadata_json::project(metadata_json::Fixture::M03, source, current)
}
pub(crate) fn boxed849_previous(source: &str, current: &str) -> String {
    metadata_json::project(
        metadata_json::Fixture::M04,
        source,
        &boxed849_input(source, current),
    )
}
// Direct current831 owner; inherited guards first receive the exact2d4b input.
pub(crate) fn unit831_input(source: &str, current: &str) -> String {
    let latest = main696_previous(source, current);
    let current = latest.as_str();
    let joined = metadata_json::project(metadata_json::Fixture::M05, source, current);
    let current = joined.as_str();
    let preflight = metadata_json::project(metadata_json::Fixture::M06, source, current);
    let current = preflight.as_str();
    metadata_json::project(metadata_json::Fixture::M07, source, current)
}
pub(crate) fn compile_fixture_input(source: &str, current: &str) -> String {
    let previous = main531_previous(source, current);
    let current = previous.as_str();
    metadata_json::project(metadata_json::Fixture::M08, source, current)
}
pub(crate) fn main531_previous(source: &str, current: &str) -> String {
    let fixed = main531_child_input(source, current);
    let current = fixed.as_str();
    metadata_json::project(metadata_json::Fixture::M09, source, current)
}
pub(crate) fn main531_input(source: &str, current: &str) -> String {
    let fixed = main531_child_input(
        "src/cleanup_plan/replay/path_join/main_single_groups/mod.rs",
        current,
    );
    let current = fixed.as_str();
    metadata_json::project(metadata_json::Fixture::M10, source, current)
}
fn main531_child_input(source: &str, current: &str) -> String {
    let fixed = actual50_fixture_input(source, current);
    let current = fixed.as_str();
    metadata_json::project(metadata_json::Fixture::M11, source, current)
}
fn b3a_compile_fixture_input(source: &str, current: &str) -> String {
    let joined = fitting759_input(source, current);
    let current = joined.as_str();
    metadata_json::project(metadata_json::Fixture::M12, source, current)
}
fn post_main0ab1_input(source: &str, current: &str) -> String {
    let compiled = b3a_compile_fixture_input(source, current);
    let current = compiled.as_str();
    metadata_json::project(metadata_json::Fixture::M13, source, current)
}
fn main696_corrected_input(source: &str, current: &str) -> String {
    let next = post_main0ab1_input(source, current);
    let current = next.as_str();
    metadata_json::project(metadata_json::Fixture::M14, source, current)
}
pub(crate) fn main696_input(source: &str, current: &str) -> String {
    let corrected = main696_corrected_input(source, current);
    let current = corrected.as_str();
    metadata_json::project(metadata_json::Fixture::M15, source, current)
}
fn main696_previous(source: &str, current: &str) -> String {
    let corrected = main696_corrected_input(source, current);
    let current = corrected.as_str();
    metadata_json::project(metadata_json::Fixture::M16, source, current)
}
fn actual50_fixture_input(source: &str, current: &str) -> String {
    let latest = main696_previous(source, current);
    let current = latest.as_str();
    let joined = metadata_json::project(metadata_json::Fixture::M17, source, current);
    let current = joined.as_str();
    metadata_json::project(metadata_json::Fixture::M18, source, current)
}
fn fitting759_input(source: &str, current: &str) -> String {
    let pinned = pin47_input(source, current);
    let current = pinned.as_str();
    metadata_json::project(metadata_json::Fixture::M19, source, current)
}
fn pin47_input(source: &str, current: &str) -> String {
    let joined = future_bcd_input(source, current);
    let current = joined.as_str();
    metadata_json::project(metadata_json::Fixture::M20, source, current)
}
// The new Main receiver has one additional, independently authenticated input; its older
// four owners still use their exact Main696 domains.
pub(crate) fn main8fb_input(source: &str, current: &str) -> String {
    if source == "src/source_verify/declaration.rs" {
        let joined = future_main8fb_input(source, current);
        let current = joined.as_str();
        return metadata_json::project(metadata_json::Fixture::M21, source, current);
    }
    main696_input(source, current)
}
pub(crate) fn future_bcd_input(source: &str, current: &str) -> String {
    let joined = post21_input(source, current);
    let current = joined.as_str();
    metadata_json::project(metadata_json::Fixture::M22, source, current)
}
pub(crate) fn future_main8fb_input(source: &str, current: &str) -> String {
    let joined = post21_input(source, current);
    let current = joined.as_str();
    metadata_json::project(metadata_json::Fixture::M23, source, current)
}
fn post21_input(source: &str, current: &str) -> String {
    let joined = main_b80_prior_input(source, current);
    let current = joined.as_str();
    metadata_json::project(metadata_json::Fixture::M24, source, current)
}
// Only literal historical ENTRY stages are acquired here. The complete existing compile35f
// front and every intermediate inverse below still execute in order.
fn historical_project_entry(source: &str, expected: &str, current: &str) -> String {
    // The preceding authentic a3 projector already restored this complete stage.
    let restored_receiving_stage = source
        == "src/workspace_graph/constructor_failure/receiving/before.rs"
        && expected == "6135d75cf3f57029b9dd754030aba155a75ca902772aa036345481c1d3abc8b1"
        && current.len() == 14489
        && source_sha(current) == expected;
    if !restored_receiving_stage {
        if let Some(entry) = constructor825_compiled_entry(source, expected, current) {
            return entry;
        }
    }
    // This source is compiled DATA, not a canonical-row admission. Restore its exact
    // formatter-only predecessor before the unchanged old projector.
    if source == "src/diagnostic.rs"
        && expected == "922f3150549194a6016b86d86fe29dc56a9fb6a23384032b1afc144b248b782e"
    {
        assert_eq!(current, include_str!("diagnostic.rs"));
        assert_eq!(current.len(), 11599);
        assert_eq!(
            source_sha(current),
            "028ee6f9753c7c70c88cf146f35f7b7c61c692d6b4035d3abc03260456eefaa1"
        );
        let formatted = "#[rustfmt::skip]\nmod before_writer;";
        assert_eq!(current.matches(formatted).count(), 1);
        let entry = current.replacen(formatted, "mod before_writer;", 1);
        assert_eq!(entry.len(), 11582);
        assert_eq!(source_sha(&entry), expected);
        return entry;
    }
    let admitted = matches!(
        (source, expected),
        (
            "src/workspace_graph/expected_projection/borrowed_imports.rs",
            "326d155ffc918d43dd00ccbb2f1aa562e45fbfd9d0fe5b4809ac4401408c7d1c"
        ) | (
            "CHANGELOG.md",
            "dabe64d8fc192968a47f494ebf1ace0de89092c213f615e077d7fe4bc90f2148"
        ) | (
            "docs/ARCHITECTURE.md",
            "ebc8d9becb91cb72845e18825d3f49bec992e15c63459b6368305a532cb235c4"
        ) | (
            "src/workspace_graph/expected_projection/borrowed_imports/typed.rs",
            "2532586630034d6c7f2953a1f487c34c25953a5b13dc86a2170bb7561a8fac36"
        ) | (
            "src/cleanup_plan/replay/live_oracle/source_custody/b.rs",
            "ca0e42fbd3f14f29ff59b7dc4c6bff26a364a1e305967bcf548970a53774a9ca"
        ) | (
            "src/cleanup_plan/replay/lexical_replay.rs",
            "52a1b94530881bd2f59d873a71663fcd1656a9a2401440bc4f8edb69839fbd7f"
        ) | (
            "src/cleanup_plan/replay/lexical_replay/operand_tests/source.rs",
            "575f7461fb6624f8dfdf1ed511cbade1227a47358abd0858755396d949e2931b"
        ) | (
            "src/workspace_graph.rs",
            "b8d71709bbe9961b338552bba4f6098e59c8d78c5da7ca5f8ff4157b36ea3f31"
        ) | (
            "tests/workspace/semantic_graph/source_inventory.rs",
            "aa4e1844d2e867c519bd24fdb560535c9f07b7daa790e343c1b4b04e978d0989"
        ) | (
            "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs",
            "0bf3a868125c5d381ead3b67127f622085d8fba09b23862db68bdad13c271ec6"
        ) | (
            "src/workspace_graph/cold_canonical/tests.rs",
            "ecc2a07ac366adeaddc986c5bf1c676000e95ec98f5dd526c37c78f65406d62c"
        )
    );
    let admitted = admitted || constructor_stage::entry(source, expected);
    if !admitted {
        return current.to_owned();
    }
    let entry = main_e177_final_inputs::value_direct(source, expected, current);
    assert_eq!(
        source_sha(&entry),
        expected,
        "complete fixed historical projector entry: {source}"
    );
    entry
}
fn project(rows: &str, source: &str, current: &str) -> String {
    let rows: serde_json::Value = serde_json::from_str(rows).expect("fixed merge projections");
    let matching: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["source"] == source)
        .collect();
    assert!(matching.len() <= 1, "unique merge source view");
    let Some(row) = matching.first() else {
        return current.to_owned();
    };
    let entry = historical_project_entry(source, row["current_sha256"].as_str().unwrap(), current);
    let acquired = main67107_inputs::compile35f_expected_input(
        source,
        row["current_sha256"].as_str().unwrap(),
        &entry,
    );
    let current = acquired.as_str();
    assert_eq!(source_sha(current), row["current_sha256"], "{source}");
    let mut result = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(
            result.matches(new).count(),
            1,
            "complete unique merge inverse"
        );
        result = result.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(source_sha(&result), row["original_sha256"]);
    result
}
// Exact post-build name/visibility repair, before every protected older domain. Unlisted
// sources remain unchanged; listed inputs must match their full new SHA.
fn before_build_fix(source: &str, current: &str) -> String {
    runtime_json::project(runtime_json::Fixture::F00, source, current)
}
pub(crate) fn ours(source: &str, current: &str) -> String {
    let staged = runtime_json::project(runtime_json::Fixture::F01, source, current);
    let current = staged.as_str();
    runtime_json::project(runtime_json::Fixture::F02, source, current)
}
fn prior_main(source: &str, current: &str) -> String {
    let predecessor = assembled_previous(source, current);
    let current = predecessor.as_str();
    runtime_json::project(runtime_json::Fixture::F03, source, current)
}
pub(crate) fn held(source: &str, current: &str) -> String {
    let predecessor = assembled_previous(source, current);
    let current = predecessor.as_str();
    runtime_json::project(runtime_json::Fixture::F04, source, current)
}
pub(crate) fn weighted(path: &str, current: &str) -> String {
    let staged = if path == "../epsilon_alias/tests.rs" {
        final_ready_input(
            "d8",
            "src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs",
            current,
        )
    } else {
        current.to_owned()
    };
    let current = staged.as_str();
    if path == "../../../../../../docs/ARCHITECTURE.md" {
        return assembled_previous("docs/ARCHITECTURE.md", current);
    }
    if path != "../epsilon_alias/tests.rs" {
        return current.to_owned();
    }
    runtime_json::project(
        runtime_json::Fixture::F05,
        "src/cleanup_plan/replay/factored/canonical_shared/epsilon_alias/tests.rs",
        current,
    )
}
// NEXT842 source transport precedes every immutable runtime domain.
fn inline_weighted_previous(source: &str, current: &str) -> String {
    runtime_json::project(runtime_json::Fixture::F06, source, current)
}
// Separate authenticated #843 predecessor; older runtime manifests stay immutable.
pub(crate) fn closure_previous(source: &str, current: &str) -> String {
    runtime_json::project(runtime_json::Fixture::F07, source, current)
}
// Each isolated branch projector remains strict. Authenticate the actual final input
// separately into its named branch before comparing complete base bytes.
pub(crate) fn assembly_view(stage: &str, source: &str, current: &str) -> String {
    let previous = latest_previous(source, current);
    let current = previous.as_str();
    let data = match stage {
        "9d" => runtime_json::Fixture::F08,
        "842" => runtime_json::Fixture::F09,
        "843" => runtime_json::Fixture::F10,
        _ => panic!("unknown immutable assembly source domain"),
    };
    runtime_json::project(data, source, current)
}
fn assembled_previous(source: &str, current: &str) -> String {
    let repaired = assembly_view("9d", source, current);
    let repaired = before_build_fix(source, &repaired);
    let inline = assembly_view("842", source, current);
    let inline = inline_weighted_previous(source, &inline);
    let closure = assembly_view("843", source, current);
    let closure = closure_previous(source, &closure);
    assert_eq!(
        repaired, inline,
        "independent branch sources must reach the same complete7800 input"
    );
    assert_eq!(
        repaired, closure,
        "held branch source must reach the same complete7800 input"
    );
    repaired
}
// Strict newest input, before the immutable9365 branch domains. No old raw input is accepted
// as an alternative to its complete current source.
fn latest_previous(source: &str, current: &str) -> String {
    let corrected = corrected_previous(source, current);
    let current = corrected.as_str();
    runtime_json::project(runtime_json::Fixture::F11, source, current)
}
pub(crate) fn main(source: &str, current: &str) -> String {
    if source == "src/cleanup_plan/replay/skeleton_work/ordinary_dag/tests.rs" {
        return main_f045_input(source, current);
    }
    let branch = final_main_input(source, current);
    let physical = before_f045(source, &branch);
    let _prior = prior_main(source, current);
    let corrected = corrected_previous(source, current);
    let _historical_main7f = runtime_json::project(runtime_json::Fixture::F12, source, &corrected);
    runtime_json::project(runtime_json::Fixture::F13, source, &physical)
}
// Actual corrective owners authenticate before the immutable d1bb source view.
fn corrected_previous(source: &str, current: &str) -> String {
    let baseline = consumer_previous(source, current);
    let current = baseline.as_str();
    runtime_json::project(runtime_json::Fixture::F14, source, current)
}
pub(crate) fn scalar_policy_input(source: &str, current: &str) -> String {
    let previous = before_main927(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F15, source, current)
}
// The consumer correction has one strict current→frozen3d4b predecessor.
fn consumer_previous(source: &str, current: &str) -> String {
    let predecessor = corrected_consumers_previous(source, current);
    let current = predecessor.as_str();
    runtime_json::project(runtime_json::Fixture::F16, source, current)
}
// Closed packet/source pairs only. This acquires the original consumer INPUT, never its
// final inverse output; all caller hooks and full-byte checks remain.
pub(crate) fn receiver_fixture_entry(
    fixture: ReceiverFixture,
    source: &str,
    current: &str,
) -> String {
    let expected = match (fixture, source) {
        (ReceiverFixture::PaidLiveReuse, "src/loan_plan/paid_live.rs") => "bcb520a64f14a76f2bac1e6053ce40f424d15e11e8b1e70f8c67d4406c450374",
        (ReceiverFixture::PaidLiveReuse, "src/loan_plan/paid_plan.rs") => "0eb3df7863a61410f5a4547831aa399ffd03d460dad86f1f40caf626de0f2be7",
        (ReceiverFixture::PaidLiveReuse, "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs") => "d764e8d6fbd373a2dbb40953baabf688fb4459562760fd5e1545cb9b73b9e6b0",
        (ReceiverFixture::BorrowedGeneric, "src/workspace_graph/generic_type_import.rs") => "e4aaa275d0b6d79539a30b496844388ba9fc3b4b793499b907b803f759ee8dc9",
        (ReceiverFixture::BorrowedGeneric, "src/workspace_graph/expected_projection/cost.rs") => "ddf8dc2e74e4c0cc193e4e592671ffcfd9118b8bfe91cef2d0cf19eca3a635ce",
        (ReceiverFixture::BorrowedGeneric, "tests/workspace/semantic_graph/source_inventory.rs") => "4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14",
        (ReceiverFixture::LexicalOperands, "src/cleanup_plan/replay/expression_facts/receiving.rs") => "db822f51812769eadf1570bfee4b315ceadd31b689fc6efa643e5da03d7a9bb7",
        (ReceiverFixture::LexicalOperands, "src/cleanup_plan/replay/expression_facts/receiving/directory.rs") => "d161622003e500b3b9197c0f463b88eee388d9447c5a87ba75f464d3aa2a59cc",
        _ => return current.to_owned(),
    };
    let entry = main_e177_final_inputs::value_direct(source, expected, current);
    assert_eq!(
        source_sha(&entry),
        expected,
        "complete fixed receiver input: {source}"
    );
    entry
}
pub(crate) fn consumer_input(domain: &str, source: &str, current: &str) -> String {
    let entry = if domain == "lexical832" {
        receiver_fixture_entry(ReceiverFixture::LexicalOperands, source, current)
    } else {
        current.to_owned()
    };
    let current = entry.as_str();
    let predecessor = corrected_consumers_previous(source, current);
    let current = predecessor.as_str();
    let data = match domain {
        "decl825" => runtime_json::Fixture::F17,
        "cost825" => runtime_json::Fixture::F18,
        "signature825" => runtime_json::Fixture::F19,
        "capacity834" => runtime_json::Fixture::F20,
        "registry833" => runtime_json::Fixture::F21,
        "receiving831" => runtime_json::Fixture::F22,
        "lexical832" => runtime_json::Fixture::F23,
        "cold835" => runtime_json::Fixture::F24,
        _ => panic!("unknown complete source consumer domain"),
    };
    runtime_json::project(data, source, current)
}
fn corrected_consumers_previous(source: &str, current: &str) -> String {
    let fixes = final_fix_input(source, current);
    let fixes = runtime_json::project(runtime_json::Fixture::F25, source, &fixes);
    let fixes = runtime_json::project(runtime_json::Fixture::F26, source, &fixes);
    let main = final_main_input(source, current);
    let main = ff_previous(source, &main);
    assert_eq!(
        fixes, main,
        "independent final branches must reach the same complete ae91 input"
    );
    fixes
}
// These three formatter inverses bind the ENTIRE actual compiled child body, not merely an
// archived before image. No token or string-content normalization.
pub(crate) fn receiver_child_entry(source: &str, current: &str) -> String {
    let (expected, length, compiled_sha, after, before) = match source {
        "src/workspace_graph/expected_projection/private_layout/before_registry.rs" => (
            "0d6e2e30c78d040eafcd7c11e66a4696d17e949aa09eac7d79d6d1b6634721cd", 8142,
            "986c2f1eb41715186ee5a1856e58abbcde03d53e86e53b1e903e1b0845c002b8",
            "}\n\n#[cfg(test)]\nthread_local! {static DERIVATIONS:",
            "}\n\n\n#[cfg(test)]\nthread_local! {static DERIVATIONS:",
        ),
        "src/workspace_graph/cold_canonical/before.rs" => (
            "78fbcd95f4430f0750b4ae1e8b2cfdc5277c3bf9ef02fe4776cea96bc5a494e2", 19289,
            "cd153b0db9d19b6b8983fc66f3970494d1247c2180b595d839db96eeaf4f8487",
            "                    staged!(\n                        CorePrebound,\n                        constructor_failure::attempt::reserve(resolve_builder_bytes)\n                    )?;",
            "                    staged!(CorePrebound, constructor_failure::attempt::reserve(resolve_builder_bytes))?;",
        ),
        "src/format/comments/compare/tests.rs" => (
            "7adedbc8fc515199d21e71a93fbe7a1624c5500e46278f4ddf834050ef0a7b38", 3421,
            "4b422dd4781d2123eddd2eabb1459c5e3bedb222fc6957729d4c444e3ca2cf2c",
            "    assert_eq!(\n        refusal.message,\n        \"a module must declare at least one function\"\n    );",
            "    assert_eq!(refusal.message, \"a module must declare at least one function\");",
        ),
        _ => return current.to_owned(),
    };
    // Fresh physical/compiled/archive authentication precedes historical lookup.
    let entry = main_e177_final_inputs::value_direct(source, expected, current);
    assert_eq!(
        current.len(),
        length,
        "complete compiled child extent: {source}"
    );
    assert_eq!(
        source_sha(current),
        compiled_sha,
        "complete compiled child bytes: {source}"
    );
    assert_eq!(
        current.matches(after).count(),
        1,
        "one literal child formatter inverse: {source}"
    );
    let inverse = current.replacen(after, before, 1);
    assert_eq!(
        source_sha(&inverse),
        expected,
        "complete original compiled child: {source}"
    );
    assert_eq!(
        inverse, entry,
        "whole compiled child equals independently acquired history: {source}"
    );
    inverse
}
pub(crate) fn consumer_child(domain: &str, source: &str, current: &str) -> String {
    let entry = receiver_child_entry(source, current);
    let current = entry.as_str();
    let branch = final_fix_input(source, current);
    let current = branch.as_str();
    let rows = match domain {
        "registry833" => runtime_json::Fixture::F27,
        "cold835" => runtime_json::Fixture::F28,
        "ordered838" => runtime_json::Fixture::F29,
        _ => panic!("unknown source child domain"),
    };
    runtime_json::project(rows, source, current)
}
fn final_fix_input(source: &str, current: &str) -> String {
    let previous = before_main927(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F30, source, current)
}
fn final_main_input(source: &str, current: &str) -> String {
    let previous = before_main927(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F31, source, current)
}
fn ff_previous(source: &str, current: &str) -> String {
    let predecessor = before_f045(source, current);
    let previous = runtime_json::project(runtime_json::Fixture::F32, source, &predecessor);
    runtime_json::project(runtime_json::Fixture::F33, source, &previous)
}
// MAIN's exact guards retain their complete immutable f045 physical domain.
pub(crate) fn main_f045_input(source: &str, current: &str) -> String {
    let branch = final_main_input(source, current);
    runtime_json::project(runtime_json::Fixture::F34, source, &branch)
}
fn before_f045(source: &str, current: &str) -> String {
    runtime_json::project(runtime_json::Fixture::F35, source, current)
}
// Preserve the complete frozen9b domains independently of the newer MAIN proof.
fn before_main927(source: &str, current: &str) -> String {
    let previous = before_setup825(source, current);
    let current = previous.as_str();
    let main_view = assembly_main927_input(source, current);
    let hex_view = runtime_json::project(runtime_json::Fixture::F36, source, current);
    let hex_previous = facade_hex_previous(source, &hex_view);
    let previous = main927_previous(source, &main_view);
    assert_eq!(
        previous, hex_previous,
        "complete independent predecessor: {source}"
    );
    previous
}
fn main927_previous(source: &str, current: &str) -> String {
    runtime_json::project(runtime_json::Fixture::F37, source, current)
}
// Only these named inputs may enter MAIN's unchanged semantic source proofs.
pub(crate) fn main927_input(source: &str, current: &str) -> String {
    let previous = before_setup825(source, current);
    let current = previous.as_str();
    let view = assembly_main927_input(source, current);
    let current = view.as_str();
    runtime_json::project(runtime_json::Fixture::F38, source, current)
}
// Compiler-confirmed facade digest correction precedes immutable9b source views.
fn facade_hex_previous(source: &str, current: &str) -> String {
    runtime_json::project(runtime_json::Fixture::F39, source, current)
}
// Independent complete branch inputs; neither branch is fed the other's view.
fn assembly_main927_input(source: &str, current: &str) -> String {
    runtime_json::project(runtime_json::Fixture::F40, source, current)
}
// Authenticated physical successor for the bounded preowned-setup/starter batch.
fn before_setup825(source: &str, current: &str) -> String {
    let previous = before_setup_reference_controls(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F41, source, current)
}
// Preserve the complete immutable typed825 runtime manifest's own exact domain.
pub(crate) fn setup_typed_input(source: &str, current: &str) -> String {
    let typed_entry = match source {
        "src/workspace_graph/expected_projection/defaults/typed.rs" => {
            constructor825_compiled_entry(
                source,
                "9761dcd2ec5fd2351807f7526479680a56c51211eb584385ca85dbc893f38fb7",
                current,
            )
        }
        "src/workspace_graph/expected_projection/import_stub/typed.rs" => {
            constructor825_compiled_entry(
                source,
                "c220a90bfa131f6b75131e461ead26aea1dcee6c3ad09c42e5f481f5c12c5b11",
                current,
            )
        }
        _ => None,
    };
    let current = typed_entry.as_deref().unwrap_or(current);
    let acquired;
    let current = if source == "src/workspace_graph/constructor_failure/attempt.rs" {
        acquired = constructor825_attempt_before(current);
        acquired.as_str()
    } else {
        current
    };
    let previous = before_setup_reference_controls(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F42, source, current)
}
// Corrected physical receiving fees precede the immutable initial setup views.
fn before_setup_reference_controls(source: &str, current: &str) -> String {
    let previous = before_region_status_view(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F43, source, current)
}
// Full physical successor; the prior receiving correction domain stays exact.
fn before_region_status_view(source: &str, current: &str) -> String {
    let previous = before_constructor_complete_views(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F44, source, current)
}
fn before_constructor_complete_views(source: &str, current: &str) -> String {
    let previous = before_current_main(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F45, source, current)
}
// These two complete manifests describe physical e4b inputs, not CI reflows.
pub(crate) fn constructor_complete_input(source: &str, current: &str) -> String {
    let previous = before_current_main(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F46, source, current)
}
// New MAIN/fixture bytes authenticate into the complete previous private batch.
fn before_current_main(source: &str, current: &str) -> String {
    let previous = before_fixture841(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F47, source, current)
}
// MAIN's refusal-site source proofs retain their actual5ba physical domain.
pub(crate) fn main_current_input(source: &str, current: &str) -> String {
    let previous = before_fixture841(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F48, source, current)
}
// Independent branches: inherited guards see13d; the pilot sees its exact5f input.
fn before_fixture841(source: &str, current: &str) -> String {
    let previous = before_retained833(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F49, source, current)
}
pub(crate) fn compressed_fixture_input(source: &str, current: &str) -> String {
    // Independently authenticate the raw-default checkpoint and its complete immediate
    // shim view; the inherited chain below retains every old guard.
    let raw_default = final_batch_input("841", source, current);
    let _whole_raw_default_before =
        runtime_json::project(runtime_json::Fixture::F50, source, &raw_default);
    let previous = before_retained833(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F51, source, current)
}
// The retained registry owner and inherited guards have distinct whole inputs.
fn before_retained833(source: &str, current: &str) -> String {
    let previous = before_main66517(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F52, source, current)
}
pub(crate) fn retained833_input(source: &str, current: &str) -> String {
    let previous = before_main66517(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F53, source, current)
}
// Latest MAIN source enters the immutable combined833/841 checkpoint first.
fn before_main66517(source: &str, current: &str) -> String {
    let previous = before_retry_guard(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F54, source, current)
}
// Preserve the complete old runtime input before every inherited source domain.
fn before_retry_guard(source: &str, current: &str) -> String {
    let previous = before_direct_carriers831(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F55, source, current)
}
pub(crate) fn retry_fixture_input(source: &str, current: &str) -> String {
    let previous = before_direct_carriers831(source, current);
    let current = previous.as_str();
    runtime_json::project(runtime_json::Fixture::F56, source, current)
}
// Every older runtime source guard retains the exact297 physical domain.
fn before_direct_carriers831(source: &str, current: &str) -> String {
    let observed = before_cached_acquisition_observer(source, current);
    let current = observed.as_str();
    runtime_json::project(runtime_json::Fixture::F57, source, current)
}
// Test-only observation successor; exact prior runtime tables remain immutable.
fn before_cached_acquisition_observer(source: &str, current: &str) -> String {
    let staged = before_sourceguard_inputs(source, current);
    let current = staged.as_str();
    runtime_json::project(runtime_json::Fixture::F58, source, current)
}
// Preserve complete physical7a87 inputs before the cached observer's older view.
fn before_sourceguard_inputs(source: &str, current: &str) -> String {
    let staged = before_actual_pin_consumers(source, current);
    let current = staged.as_str();
    runtime_json::project(runtime_json::Fixture::F59, source, current)
}
pub(crate) fn source36_classifier_input(source: &str, current: &str) -> String {
    let staged = before_storage_oracle_join(source, current);
    runtime_json::project(runtime_json::Fixture::F60, source, &staged)
}
// Actual producer literals are transported only for historical full-source guards.
fn before_actual_pin_consumers(source: &str, current: &str) -> String {
    let current = final_ready_input("d8", source, current);
    let current = current.as_str();
    runtime_json::project(runtime_json::Fixture::F61, source, current)
}
// Independent physical-input domains: no checkpoint table accepts another stage.
fn final_ready_input(stage: &str, source: &str, current: &str) -> String {
    let current = if stage == "844" {
        let previous = main27424_stage("523", source, current);
        runtime_json::project(runtime_json::Fixture::F62, source, &previous)
    } else {
        let previous = main27424_stage("7f", source, current);
        runtime_json::project(runtime_json::Fixture::F63, source, &previous)
    };
    let current = current.as_str();
    let data = match stage {
        "d8" => runtime_json::Fixture::F64,
        "844" => runtime_json::Fixture::F65,
        "846" => runtime_json::Fixture::F66,
        _ => panic!("unknown exact ready checkpoint"),
    };
    runtime_json::project(data, source, current)
}
pub(crate) fn compact844_current_input(source: &str, current: &str) -> String {
    final_ready_input("844", source, current)
}
pub(crate) fn compact844_input(source: &str, current: &str) -> String {
    let current = final_ready_input("844", source, current);
    runtime_json::project(runtime_json::Fixture::F67, source, &current)
}
pub(crate) fn diagnostic846_input(source: &str, current: &str) -> String {
    let entry;
    let current = if source == "src/diagnostic.rs" {
        entry = historical_project_entry(
            source,
            "922f3150549194a6016b86d86fe29dc56a9fb6a23384032b1afc144b248b782e",
            current,
        );
        entry.as_str()
    } else {
        current
    };
    final_ready_input("846", source, current)
}
// MAIN and the two677 compile corrections have independent physical inputs.
fn main27424_stage(stage: &str, source: &str, current: &str) -> String {
    let current = combined_ready_input("main", source, current);
    let current = current.as_str();
    let data = match stage {
        "7f" => runtime_json::Fixture::F68,
        "523" => runtime_json::Fixture::F69,
        "main" => runtime_json::Fixture::F70,
        _ => panic!("unknown exact MAIN27424 source domain"),
    };
    runtime_json::project(data, source, current)
}
pub(crate) fn main27424_input(source: &str, current: &str) -> String {
    main27424_stage("main", source, current)
}
// Independent physical847 source for its complete owning guards.
fn retained_function847_previous(source: &str, current: &str) -> String {
    joined_feature_input("453", source, current)
}
pub(crate) fn retained_function847_input(source: &str, current: &str) -> String {
    retained_function847_previous(source, current)
}
pub(crate) fn directory825_input(source: &str, current: &str) -> String {
    let previous = joined_feature_input("453", source, current);
    runtime_json::project(runtime_json::Fixture::F71, source, &previous)
}
fn joined_feature_input(stage: &str, source: &str, current: &str) -> String {
    let current = combined_ready_input("alias", source, current);
    let current = current.as_str();
    let rows = match stage {
        "453" => runtime_json::Fixture::F72,
        "523" => runtime_json::Fixture::F73,
        "7f" => runtime_json::Fixture::F74,
        _ => panic!("unknown independent feature input"),
    };
    runtime_json::project(rows, source, current)
}
// Independent final raw fronts for MAIN integration versus retained alias owners.
fn combined_ready_input(stage: &str, source: &str, current: &str) -> String {
    let final_view = final_batch_input("796", source, current);
    let current = final_view.as_str();
    let previous = runtime_json::project(runtime_json::Fixture::F75, source, current);
    let current = previous.as_str();
    let prior = runtime_json::project(runtime_json::Fixture::F76, source, current);
    let current = prior.as_str();
    let previous = runtime_json::project(runtime_json::Fixture::F77, source, current);
    let current = previous.as_str();
    let rows = match stage {
        "main" => runtime_json::Fixture::F78,
        "alias" => runtime_json::Fixture::F79,
        _ => panic!("unknown exact final source front"),
    };
    runtime_json::project(rows, source, current)
}
// The direct831 source owner has a separate immediate domain from old guards.
pub(crate) fn branch_closed831_input(source: &str, current: &str) -> String {
    final_batch_input("831", source, current)
}
fn final_batch_input(branch: &str, source: &str, current: &str) -> String {
    let before_transport = before_runtime_json_transport(source, current);
    let current = before_transport.as_str();
    let before_runtime848 = runtime_json::project(runtime_json::Fixture::F80, source, current);
    let current = before_runtime848.as_str();
    let before_main = runtime_json::project(runtime_json::Fixture::F81, source, current);
    let current = before_main.as_str();
    let prior_docs = runtime_json::project(runtime_json::Fixture::F82, source, current);
    let current = prior_docs.as_str();
    let rows = match branch {
        "796" => runtime_json::Fixture::F83,
        "831" => runtime_json::Fixture::F84,
        "841" => runtime_json::Fixture::F85,
        "845" => runtime_json::Fixture::F86,
        _ => panic!("unknown independent final batch source checkpoint"),
    };
    runtime_json::project(rows, source, current)
}
// Direct845 owners authenticate independently from inherited source guards.
pub(crate) fn initial845_input(source: &str, current: &str) -> String {
    let acquired = initial845_current_owner_before(source, current);
    let current = acquired.as_deref().unwrap_or(current);
    let ledger_entry = initial845_ledger_entry(source, current);
    let current = ledger_entry.as_deref().unwrap_or(current);
    let owner_entry = initial845_compiled_owner_entry(source, current);
    let current = owner_entry.as_deref().unwrap_or(current);
    let checkpoint = final_batch_input("845", source, current);
    let _complete_original845_before =
        runtime_json::project(runtime_json::Fixture::F87, source, &checkpoint);
    if source == "src/workspace_graph/diagnostics.rs" {
        return constructor825_compiled_entry(
            source,
            "1c1552d56400ec781eb96ff6fd825bbbeec854f5042fab960e4d3d2941e60127",
            &checkpoint,
        )
        .expect("complete diagnostics845 entry");
    }
    checkpoint
}
// Direct runtime-fixture custody stays independent of inherited source views.
pub(crate) fn runtime848_input(source: &str, current: &str) -> String {
    let before_transport = before_runtime_json_transport(source, current);
    let current = before_transport.as_str();
    runtime_json::project(runtime_json::Fixture::F88, source, current)
}
// Complete pre-transport physical inputs precede every older named view.
fn before_runtime_json_transport(source: &str, current: &str) -> String {
    let before_main83 = main83_physical_input(source, current);
    let current = before_main83.as_str();
    runtime_json::project(runtime_json::Fixture::Custody, source, current)
}
// MAIN83 reads its formatted fixture independently of historical preformat F56.
pub(crate) fn main83_retry_fixture_input(source: &str, current: &str) -> String {
    let formatted = main83_physical_input(source, current);
    let _historical_preformat = retry_fixture_input(source, current);
    formatted
}
fn main83_physical_input(source: &str, current: &str) -> String {
    let joined = before_storage_oracle_join(source, current);
    let current = joined.as_str();
    metadata_json::project(metadata_json::Fixture::M25, source, current)
}
// Full post-compiler-fix inputs precede the immutable MAIN83 and older domains.
fn before_storage_oracle_join(source: &str, current: &str) -> String {
    let compile_fixed = compile_fixture_input(source, current);
    let current = compile_fixed.as_str();
    let qualified = metadata_json::project(metadata_json::Fixture::M00, source, current);
    let current = qualified.as_str();
    let next = metadata_json::project(metadata_json::Fixture::M01, source, current);
    let current = next.as_str();
    let integrated = metadata_json::project(metadata_json::Fixture::M26, source, current);
    let current = integrated.as_str();
    let previous = metadata_json::project(metadata_json::Fixture::M27, source, current);
    let current = previous.as_str();
    metadata_json::project(metadata_json::Fixture::M28, source, current)
}
// Independent complete Main and pre-Main source domains.
pub(crate) fn main_b80_input(source: &str, current: &str) -> String {
    let merged = main_dd0_previous(source, current);
    let current = merged.as_str();
    if matches!(
        source,
        "src/cleanup_plan/replay/compact_stage/before.rs"
            | "src/cleanup_plan/replay/compact_stage/lexical_before.rs"
    ) {
        return metadata_json::project(metadata_json::Fixture::M29, source, current);
    }
    let latest = session_reference_input(source, current);
    let current = latest.as_str();
    metadata_json::project(metadata_json::Fixture::M30, source, current)
}
pub(crate) fn main_b80_prior_input(source: &str, current: &str) -> String {
    let merged = main_dd0_previous(source, current);
    let current = merged.as_str();
    let latest = session_reference_input(source, current);
    let current = latest.as_str();
    metadata_json::project(metadata_json::Fixture::M31, source, current)
}
// Independent whole-driver input for the immutable first781 receiving owners.
pub(crate) fn reference781_input(source: &str, current: &str) -> String {
    let merged = main_dd0_previous(source, current);
    let current = merged.as_str();
    let latest = reference_family_input(source, current);
    let current = latest.as_str();
    metadata_json::project(metadata_json::Fixture::M32, source, current)
}
// Independent physical predecessor domains, before all immutable older stages.
fn session_reference_input(source: &str, current: &str) -> String {
    metadata_json::project(metadata_json::Fixture::M33, source, current)
}
fn reference_family_input(source: &str, current: &str) -> String {
    metadata_json::project(metadata_json::Fixture::M34, source, current)
}
// A later compositor uses this normalization-only front BEFORE its raw table.
#[allow(
    dead_code,
    reason = "Retained compatibility entry for independently compiled historical and semantic controls."
)]
pub(crate) fn bulk848_arm_input(source: &str, current: &str) -> String {
    let final_input = main8a_previous(source, current);
    let current = final_input.as_str();
    release_custody::normalize(source, current)
}
// Main's compiled test corpus keeps every original source-domain check.
fn main_dd0_previous(source: &str, current: &str) -> String {
    let main65 = main65_previous(source, current);
    let current = main65.as_str();
    let cursor = cursor_clippy_input(source, current);
    let current = cursor.as_str();
    let release = release_custody::input(release_custody::Fixture::Previous2c, source, current);
    let current = release.as_str();
    let joined = joined_custody::input(source, current);
    let current = joined.as_str();
    let prefix_view = prefix_custody::input(prefix_custody::Fixture::SealedB5, source, current);
    let current = prefix_view.as_str();
    let successor =
        successor_custody::input(successor_custody::Fixture::Sealed724, source, current);
    let current = successor.as_str();
    let final_view = final_custody::input(final_custody::Fixture::Common, source, current);
    let current = final_view.as_str();
    let metadata = metadata_json::previous(source, current);
    let current = metadata.as_str();
    let pair = runtime_json::pair_previous(source, current);
    let current = pair.as_str();
    metadata_json::project(metadata_json::Fixture::M35, source, current)
}
// New owner and inherited domains remain independent; all old tables stay exact.
pub(crate) fn arena854_input(source: &str, current: &str) -> String {
    // The incoming Main join supersedes this one complete pre-merge root. Authenticate
    // and restore that exact raw input before the unchanged zip inverse; main65 below
    // independently checks the original current source.
    let query_input = if source == "src/cleanup_plan/replay/factored/canonical_shared.rs" {
        current344::project(source, current)
    } else {
        current.to_owned()
    };
    let next = project(
        include_str!("../docs/evidence/arena854-borrowed-query-zip-20261016/current-to-2d87.json"),
        source,
        &query_input,
    );
    let main65 = main65_previous(source, current);
    // The complete16c overlay may already remove this same source delta. Authenticate
    // raw bytes above, then accept only raw or that exact old view.
    let main65 = if next != current {
        assert!(
            main65 == current || main65 == next,
            "arena854 normalized input drift"
        );
        next
    } else {
        main65
    };
    let current = main65.as_str();
    let cursor = cursor_clippy_input(source, current);
    let current = cursor.as_str();
    let release = release_custody::input(release_custody::Fixture::Previous2c, source, current);
    let current = release.as_str();
    let joined = joined_custody::input(source, current);
    let current = joined.as_str();
    let prefix_view = prefix_custody::input(prefix_custody::Fixture::SealedB5, source, current);
    let current = prefix_view.as_str();
    let successor =
        successor_custody::input(successor_custody::Fixture::Sealed724, source, current);
    let current = successor.as_str();
    let final_view = final_custody::input(final_custody::Fixture::Arena, source, current);
    let current = final_view.as_str();
    let receiving = final_custody::project(final_custody::Fixture::ArenaReceiving, source, current);
    let current = receiving.as_str();
    final_custody::project(final_custody::Fixture::ArenaJoined, source, current)
}
pub(crate) fn arena854_previous(source: &str, current: &str) -> String {
    let component = arena854_input(source, current);
    final_custody::project(final_custody::Fixture::ArenaPrevious, source, &component)
}
// The complete twelve-row Main receiver has an independent scoped input.
pub(crate) fn main7c6_input(source: &str, current: &str) -> String {
    let main65 = main65_previous(source, current);
    let current = main65.as_str();
    let cursor = cursor_clippy_input(source, current);
    let current = cursor.as_str();
    let release = release_custody::input(release_custody::Fixture::Previous2c, source, current);
    let current = release.as_str();
    let joined = joined_custody::input(source, current);
    let current = joined.as_str();
    let prefix_view = prefix_custody::input(prefix_custody::Fixture::SealedB5, source, current);
    let current = prefix_view.as_str();
    let successor =
        successor_custody::input(successor_custody::Fixture::Sealed724, source, current);
    let current = successor.as_str();
    final_custody::input(final_custody::Fixture::Main7c6, source, current)
}
// Independent complete Main-afba receiver scope.
pub(crate) fn main_afba_input(source: &str, current: &str) -> String {
    let main65 = main65_previous(source, current);
    let current = main65.as_str();
    let cursor = cursor_clippy_input(source, current);
    let current = cursor.as_str();
    let release = release_custody::input(release_custody::Fixture::Previous2c, source, current);
    let current = release.as_str();
    let joined = joined_custody::input(source, current);
    let current = joined.as_str();
    let prefix_view = prefix_custody::input(prefix_custody::Fixture::SealedB5, source, current);
    let current = prefix_view.as_str();
    successor_custody::input(successor_custody::Fixture::MainAfba, source, current)
}
pub(crate) fn main_f1_input(source: &str, current: &str) -> String {
    let main65 = main65_previous(source, current);
    let current = main65.as_str();
    let cursor = cursor_clippy_input(source, current);
    let current = cursor.as_str();
    let release = release_custody::input(release_custody::Fixture::Previous2c, source, current);
    let current = release.as_str();
    let joined = joined_custody::input(source, current);
    let current = joined.as_str();
    prefix_custody::input(prefix_custody::Fixture::MainF1, source, current)
}
// Updated Main24 inputs are independent of every inherited predecessor domain.
pub(crate) fn main24_input(source: &str, current: &str) -> String {
    let main65 = main65_previous(source, current);
    let current = main65.as_str();
    let cursor = cursor_clippy_input(source, current);
    let current = cursor.as_str();
    release_custody::input(release_custody::Fixture::Main24, source, current)
}
pub(crate) fn main24_proofs(compact: bool) -> String {
    runtime_json::release_custody(if compact {
        release_custody::Fixture::Compact24
    } else {
        release_custody::Fixture::Receivers24
    })
}
// Complete raw source difference for the statement-local prepaid-cursor lint expectation.
fn cursor_clippy_input(source: &str, current: &str) -> String {
    const KEYS: &[&str] = &[
        "src/cleanup_plan/replay/factored/canonical_shared.rs",
        "src/source_reflows_merge.rs",
        "tests/hir_module_boundaries.rs",
        "tests/workspace/semantic_graph.rs",
    ];
    if KEYS.binary_search(&source).is_err() {
        return current.to_owned();
    }
    project(
        include_str!(
            "../docs/evidence/prepaid-canonical-cursor-clippy-20261016/current-to-f766.json"
        ),
        source,
        current,
    )
}
// Exact whole-source Main65 merge front; independent historical stages receive75d8.
fn main65_previous(source: &str, current: &str) -> String {
    let corrected = compile9ce_input(source, current);
    let current = corrected.as_str();
    const KEYS: &[&str] = &[
        "CHANGELOG.md",
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
        "docs/SUMMARY.md",
        "src/cleanup_plan/replay.rs",
        "src/source_reflows_merge.rs",
        "tests/hir_module_boundaries.rs",
        "tests/wasm/scalar_browser_ci_contract.rs",
        "tests/wasm/scalar_browser_ci_contract/browser_custody.rs",
        "tests/workspace/semantic_graph.rs",
        "tests/workspace/semantic_graph/source_inventory.rs",
    ];
    if KEYS.binary_search(&source).is_err() {
        return current.to_owned();
    }
    project(
        include_str!("../docs/evidence/main65-private-merge-20261016/current-to-75d8.json"),
        source,
        current,
    )
}
// Restore the complete pre-correction test inputs before immutable Main65 custody.
fn compile9ce_input(source: &str, current: &str) -> String {
    let previous = main8a_previous(source, current);
    let current = previous.as_str();
    const KEYS: &[&str] = &[
        "src/cleanup_plan/replay/factored/canonical_shared/before_arena.rs",
        "src/cleanup_plan/replay/semantic_refusal/tests.rs",
        "src/cleanup_plan/replay/sequence_prefix.rs",
        "src/source_reflows_merge.rs",
        "tests/hir_module_boundaries.rs",
        "tests/workspace/semantic_graph.rs",
    ];
    if KEYS.binary_search(&source).is_err() {
        return current.to_owned();
    }
    project(
        include_str!(
            "../docs/evidence/test-compile-corrections-9ce9-20261016/current-to-9ce9.json"
        ),
        source,
        current,
    )
}
#[path = "source_reflows_merge/main8a_custody.rs"]
mod main8a_custody;
pub(crate) use main8a_custody::Fixture as Main8aFixture;
pub(crate) fn main8a_metadata(fixture: Main8aFixture) -> String {
    runtime_json::main8a_metadata(fixture)
}

// Independent current raw domains: never serialize Main predecessors through old raw guards.
pub(crate) fn main8a_previous(source: &str, current: &str) -> String {
    let previous = maina3_previous(source, current);
    let current = previous.as_str();
    let normalized = main8a_custody::normalize(source, current);
    main8a_custody::input(main8a_custody::Input::Previous55e, source, &normalized)
}
pub(crate) fn main8a_input(source: &str, current: &str) -> String {
    let previous = maina3_previous(source, current);
    let current = previous.as_str();
    let normalized = main8a_custody::normalize(source, current);
    main8a_custody::input(main8a_custody::Input::Main8a, source, &normalized)
}
#[path = "source_reflows_merge/maina3_custody.rs"]
mod maina3_custody;
pub(crate) use maina3_custody::Fixture as Maina3Fixture;
pub(crate) fn maina3_metadata(fixture: Maina3Fixture) -> String {
    runtime_json::maina3_metadata(fixture)
}
pub(crate) fn maina3_previous(source: &str, current: &str) -> String {
    let normalized = maina3_custody::normalize(source, current);
    maina3_custody::input(maina3_custody::Input::Previous16c, source, &normalized)
}
pub(crate) fn maina3_input(source: &str, current: &str) -> String {
    let normalized = maina3_custody::normalize(source, current);
    maina3_custody::input(maina3_custody::Input::Maina3, source, &normalized)
}
pub(crate) fn maina3_847_input(source: &str, current: &str) -> String {
    let held = maina3_custody::alias_directory_input(source, current);
    let current = held.as_str();
    let normalized = maina3_custody::normalize(source, current);
    maina3_custody::input(maina3_custody::Input::Alias1ae, source, &normalized)
}

// The incoming test-only e47 owning inputs form their own raw domain.
pub(crate) fn main_e47_input(source: &str, current: &str) -> String {
    let normalized = maina3_custody::normalize(source, current);
    maina3_custody::input(maina3_custody::Input::MainE47, source, &normalized)
}
#[path = "source_reflows_merge/main44_custody.rs"]
mod main44_custody;
pub(crate) use main44_custody::Fixture as Main44Fixture;
pub(crate) fn main44_metadata(fixture: Main44Fixture) -> String {
    runtime_json::main44_metadata(fixture)
}
pub(crate) fn main44_input(source: &str, current: &str) -> String {
    main44_custody::input(source, current)
}
#[path = "source_reflows_merge/main043_custody.rs"]
mod main043_custody;
pub(crate) use main043_custody::Fixture as Main043Fixture;
pub(crate) fn main043_raw_metadata(fixture: Main043Fixture) -> String {
    runtime_json::main043_raw_metadata(fixture)
}
pub(crate) fn main043_metadata(fixture: Main043Fixture) -> String {
    runtime_json::main043_metadata(fixture)
}
pub(crate) fn main043_input(source: &str, current: &str) -> String {
    main043_custody::input(source, current)
}
pub(crate) fn main043_historical_ci_extra(source: &str, current: &str) -> String {
    main043_custody::historical_ci_extra(source, current)
}
// Current344 direct readers keep their complete historical predicates below.
#[path = "source_reflows_merge/current344.rs"]
pub(crate) mod current344;
pub(crate) fn release_current_input(source: &str, current: &str) -> String {
    let previous = main65_previous(source, current);
    cursor_clippy_input(source, &previous)
}

// Exact after-inputs for the two original Main8a historical metadata owners.
#[path = "source_reflows_merge/main8a_historical.rs"]
pub(crate) mod main8a_historical;
pub(crate) mod main_eb2_inputs;
mod opt858_sequence_input;

// Distinct closed Main7f8 historical input domains; no compiler authority.
#[path = "source_reflows_merge/final_ready_inputs.rs"]
mod final_ready_inputs;
#[path = "source_reflows_merge/main67107_inputs.rs"]
pub(crate) mod main67107_inputs;
#[path = "source_reflows_merge/main7f8_presence.rs"]
pub(crate) mod main7f8_presence;
#[path = "source_reflows_merge/main7f8_source_inputs.rs"]
pub(crate) mod main7f8_source_inputs;
#[path = "source_reflows_merge/ordinary_plan_inputs.rs"]
mod ordinary_plan_inputs;

// Only the historical PLAN source test receives this exact root checkpoint.
pub(crate) fn ordinary_plan_history_root_checkpoint(
    current: &str,
    compiled_before: &str,
) -> String {
    ordinary_plan_inputs::original_root_checkpoint(current, compiled_before)
}
#[path = "source_reflows_merge/main_c883_inputs.rs"]
pub(crate) mod main_c883_inputs;

#[path = "source_reflows_merge/main99f_inputs.rs"]
pub(crate) mod main99f_inputs;

#[path = "source_reflows_merge/main_e177_inputs.rs"]
pub(crate) mod main_e177_inputs;

// Distinct complete final-batch views precede both frozen parent readers.
#[path = "source_reflows_merge/next_big_batch_inputs.rs"]
pub(crate) mod next_big_batch_inputs;

#[path = "source_reflows_merge/main_e177_final_inputs.rs"]
pub(crate) mod main_e177_final_inputs;

#[test]
fn merge_projection_selects_only_the_requested_complete_current_view() {
    let source = "src/cleanup_plan/replay/allocation/oracle.rs";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let current = std::fs::read_to_string(root.join(source)).unwrap();
    let expected = std::fs::read_to_string(
        root.join("docs/evidence/fmt-a110/-sNiaR-dl5pY-SdFlvBwJ-5c5dFFUGlebbR8SBoVMu4.txt"),
    )
    .unwrap();
    let sha = source_sha(&expected);
    let rows = serde_json::json!([{
        "source": source, "current_sha256": sha, "original_sha256": sha, "edits": []
    }])
    .to_string();
    assert_eq!(project(&rows, source, &current), expected);
    assert_eq!(project(&rows, source, &expected), expected);
    for bad in [
        String::new(),
        "foreign".to_owned(),
        format!("{current} "),
        current[..current.len() - 1].to_owned(),
        format!("{expected} "),
    ] {
        assert!(std::panic::catch_unwind(|| project(&rows, source, &bad)).is_err());
    }
    assert_eq!(project(&rows, "unlisted", "foreign"), "foreign");
    let mut foreign = serde_json::from_str::<serde_json::Value>(&rows).unwrap();
    foreign[0]["current_sha256"] = "0".repeat(64).into();
    assert!(std::panic::catch_unwind(|| project(&foreign.to_string(), source, &current)).is_err());
}

pub(crate) fn complete_receiver_followup(source: &str) -> String {
    runtime_json::complete_receiver_followup(source)
}
// Closed complete compiled ENTRY acquisition precedes every original #825 inverse.
fn constructor825_compiled_entry(source: &str, expected: &str, current: &str) -> Option<String> {
    let restored_typed = constructor_stage::typed(source, current);
    let current = restored_typed.as_deref().unwrap_or(current);
    let typed_before = if source
        == "src/workspace_graph/constructor_failure/receiving/typed/tests.rs"
        && expected == "2cd6c2ce3b35abb30253549dc931c6c025653419d2bd16b9fb6c35224a34d6e4"
    {
        assert_eq!(
            current,
            constructor_stage::typed(
                source,
                include_str!("workspace_graph/constructor_failure/receiving/typed/tests.rs")
            )
            .as_deref()
            .unwrap()
        );
        assert_eq!(current.len(), 27662);
        assert_eq!(
            source_sha(current),
            "ddaa2f3c3d54bab731acb976de3a9482d71fa65a4d409a83c2036ed8f9ad1251"
        );
        let mut before = current.to_owned();
        for &(after, prior) in &[
            ("    // Retain the exact complete typed825 caller already authenticated above.\n    let caller = historical_caller.expect(\"complete historical workspace caller\");\n    assert_eq!(caller.len(), 231867);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(caller.as_bytes()))), \"f517b9c93b4de7db618db0abc0139d4d88121f298ae9f3b16246dc20e833c0a9\");\n    let live_caller = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/workspace_graph.rs\"));\n    let validate_live = |actual: &str| {\n        assert_eq!(actual, live_caller, \"complete current workspace caller\");\n        assert_eq!(actual.len(), 217358);\n        assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(actual.as_bytes()))), \"763fd745d6b8afb41a7738ad14b8c0a0cffc782c70f698f4a272d398f1651699\");\n        assert!(actual.contains(\"constructor_failure::typed::construct_selected_core(\"));\n        assert!(actual.contains(\".receive_typed(failure)\"));\n    };\n    validate_live(live_caller);\n    let dispatcher = include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/workspace_graph/constructor_failure/typed.rs\"));\n    assert_eq!(dispatcher.len(), 11670);\n    assert_eq!(format!(\"{:x}\", crate::digest_hex::LowerHex(Sha256::digest(dispatcher.as_bytes()))), \"b07ecf1da889cf25e1b9296bf320070d49c58af316efe6036dce534fccd73fb7\");\n    assert!(dispatcher.contains(\"    if allow_transient && possible {\\n        construct_transient_alias_core(program, authored, programs, registry, core, owner)\\n    } else {\\n        construct_transient_core(program, authored, programs, registry, core, owner)\\n    }\"));\n    for hostile in [format!(\"{live_caller}\\n\"), live_caller[..live_caller.len()-1].to_owned(), caller.clone()] {\n        assert!(std::panic::catch_unwind(|| validate_live(&hostile)).is_err());\n    }\n", "    let caller = include_str!(concat!(\n        env!(\"CARGO_MANIFEST_DIR\"),\n        \"/src/workspace_graph.rs\"\n    ));\n"),
            ("        assert_eq!(restored, original, \"{path}\");\n        if path == \"src/workspace_graph.rs\" { historical_caller = Some(current.to_owned()); }\n", "        assert_eq!(restored, original, \"{path}\");\n"),
            ("    let mut historical_caller = None;\n    let records = packet.as_array().unwrap();\n", "    let records = packet.as_array().unwrap();\n"),
        ] {
            assert_eq!(before.matches(after).count(), 1, "unique current typed owner inverse");
            before = before.replacen(after, prior, 1);
        }
        assert_eq!(before.len(), 25856);
        assert_eq!(
            source_sha(&before),
            "6199f2912e6064de90f1ecccfcbc516ba0e275faabbaac96126abb470d3755b7"
        );
        Some(before)
    } else {
        None
    };
    let current = typed_before.as_deref().unwrap_or(current);
    let (compiled, length, current_sha, edits): (&str, usize, &str, &[(&str, &str)]) = match (source, expected) {
        ("src/workspace_graph/constructor_failure/report.rs", "fc2eb1ecc98cc13b6df906a1e2f59f09d198cf958a4c7ff3472e0aae1a866a84") => (include_str!("workspace_graph/constructor_failure/report.rs"), 10609, "a389503a2f7e6b241bbbaf906f71e980e564fa689d2389543d581dbc12fe5662", &[
            ("        // Reserve the full width of the captured usize maximum. Otherwise the\n        // same successful construction acquires different report backing at\n        // each decimal limit boundary, so its exact minimum changes its bill.\n        // Rendering below still uses the actual maximum and original help.\n        let message_len = counted(|out| message(out, usize::MAX))?;\n        let base_len = counted(|out| write!(out, \"{}\", BorrowedBuilderHelp(usize::MAX)))?;\n", "        let message_len = counted(|out| message(out, maximum))?;\n        let base_len = counted(|out| write!(out, \"{}\", BorrowedBuilderHelp(maximum)))?;\n"),
        ]),
        ("src/workspace_graph/constructor_failure/receiving/before.rs", "6135d75cf3f57029b9dd754030aba155a75ca902772aa036345481c1d3abc8b1") => (include_str!("workspace_graph/constructor_failure/receiving/before.rs"), 14616, "a43797702bc674b8764a7b5533059e49c0eb1272006d38aaf22538bf8334254b", &[
            ("    charge_builder_prebound, checked_retention, collect_expected_edges, constructor_failure,\n    expected_projection, graph_error, identity_resolution, loan_retention, owned_generics,\n", "    AuthoredDeclaration, ResolvedCore, ResolvedModuleInput, WorkspaceSource,\n    WorkspaceValidationIndex, charge_builder_prebound, checked_retention, collect_expected_edges,\n    constructor_failure, expected_projection, graph_error, identity_resolution, loan_retention,\n    owned_generics, reconstruct_workspace_declaration_facts_with_authority,\n"),
            ("    validate_retained_declaration_shapes, verify_resolved_call_edges, AuthoredDeclaration,\n    ResolvedCore, ResolvedModuleInput, WorkspaceSource,\n", "    validate_retained_declaration_shapes, validate_stub_signatures_authored,\n    verify_resolved_call_edges,\n"),
            ("                    &mut core_proofs,\n                    frontend.as_deref(),\n", "                    &mut core_proofs,frontend.as_deref(),\n"),
            ("\nuse crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;\nuse crate::workspace_graph::validation::before_roots::{\n    validate_stub_signatures_authored, WorkspaceValidationIndex,\n};\n", ""),
        ]),
        ("src/workspace_graph/constructor_failure/receiving/typed/tests.rs", "2cd6c2ce3b35abb30253549dc931c6c025653419d2bd16b9fb6c35224a34d6e4") => (typed_before.as_deref().expect("authenticated complete prior typed owner"), 25856, "6199f2912e6064de90f1ecccfcbc516ba0e275faabbaac96126abb470d3755b7", &[
            ("        let old =\n            constructor_test_inline_defaults::default_expr(&ty, &declarations, false).unwrap();\n", "        let old = constructor_test_inline_defaults::default_expr(&ty, &declarations, false).unwrap();\n"),
            ("                let actual = constructor_test_typed_defaults::default_expr(\n                    &ty,\n                    &declarations,\n                    false,\n                    &mut owner,\n                )\n                .unwrap();\n", "                let actual =\n                    constructor_test_typed_defaults::default_expr(&ty, &declarations, false, &mut owner).unwrap();\n"),
            ("                        let actual = constructor_test_typed_defaults::default_expr(\n                            &ty,\n                            &declarations,\n                            false,\n                            &mut owner,\n                        );\n", "                        let actual =\n                            constructor_test_typed_defaults::default_expr(&ty, &declarations, false, &mut owner);\n"),
            ("                let actual = constructor_test_typed_import_stub::default_expr(\n                    function,\n                    &[],\n                    false,\n                    false,\n                    &mut owner,\n                );\n", "                let actual =\n                    constructor_test_typed_import_stub::default_expr(function, &[], false, false, &mut owner);\n"),
            ("    use crate::workspace_graph::expected_projection::{private_layout, CoreProviderProofs};\n", "    use crate::workspace_graph::expected_projection::{CoreProviderProofs, private_layout};\n"),
            ("                        assert!(actual\n                            .source()\n                            .types()\n                            .iter()\n                            .eq(old.source().types().iter()));\n                        assert!(actual\n                            .source()\n                            .functions()\n                            .iter()\n                            .eq(old.source().functions().iter()));\n", "                        assert!(actual.source().types().iter().eq(old.source().types().iter()));\n                        assert!(actual.source().functions().iter().eq(old.source().functions().iter()));\n"),
            ("            match constructor_test_typed_defaults::default_expr(&Type::I64, &[], false, &mut owner)\n            {\n", "            match constructor_test_typed_defaults::default_expr(&Type::I64, &[], false, &mut owner) {\n"),
        ]),
        ("src/workspace_graph/expected_projection/defaults/inline_tests.rs", "1fa04bba6279f410439f819afeb9cbe0b9e33c43d6af56550e1cd39ade0e7ead") => (include_str!("workspace_graph/expected_projection/defaults/inline_tests.rs"), 15754, "21751179a0051ebb89691962eb1cd4f3161a96cdc5ad5158c60faceb8ec5df4c", &[
            ("            record[\"source\"].as_str().unwrap(),\n            current,\n", "            record[\"source\"].as_str().unwrap(), current,\n"),
            ("            record[\"source\"].as_str().unwrap(),\n            source,\n", "            record[\"source\"].as_str().unwrap(), source,\n"),
        ]),
        ("src/workspace_graph/constructor_failure/receiving/before.rs", "f07589cda30b524062f7ad4dcb87cfc1244b2746729b2b1e5cd0e84320786aed") => (include_str!("workspace_graph/constructor_failure/receiving/before.rs"), 14616, "a43797702bc674b8764a7b5533059e49c0eb1272006d38aaf22538bf8334254b", &[
            ("//! Complete original core constructor, independently compiled; body unchanged.\nuse crate::bounded_output::staged;\nuse crate::workspace_graph::{\n    charge_builder_prebound, checked_retention, collect_expected_edges, constructor_failure,\n    expected_projection, graph_error, identity_resolution, loan_retention, owned_generics,\n    reserve_builder_structure, reserve_workspace_module_carrier, resolved_loan_bytes,\n    retain_workspace_module, retained_validation, static_text,\n    validate_retained_declaration_shapes, verify_resolved_call_edges, AuthoredDeclaration,\n    ResolvedCore, ResolvedModuleInput, WorkspaceSource,\n};\nuse crate::{ast::Program, diagnostic::Diagnostic, hir};\nuse std::cell::Cell;\n", "//! Complete original core constructor, independently compiled; body unchanged.\nuse crate::bounded_output::staged;\nuse crate::workspace_graph::{\n    AuthoredDeclaration, ResolvedCore, ResolvedModuleInput, WorkspaceSource,\n    charge_builder_prebound, checked_retention, collect_expected_edges,\n    constructor_failure, expected_projection, graph_error, identity_resolution, loan_retention,\n    owned_generics,\n    reserve_builder_structure, reserve_workspace_module_carrier, resolved_loan_bytes,\n    retain_workspace_module, retained_validation, static_text,\n    validate_retained_declaration_shapes,\n    verify_resolved_call_edges,\n};\nuse crate::{ast::Program, diagnostic::Diagnostic, hir};\nuse std::cell::Cell;\n"),
            ("            // until this legacy outer adapter.\n            let (resolved, function_costs, reused_functions, static_text) =\n                identity_resolution::resolve_module_with_core(\n                    &mut core_proofs,\n                    frontend.as_deref(),\n                    &program.path,\n                    &synthetic,\n                    private,\n", "            // until this legacy outer adapter.\n            let (resolved, function_costs, reused_functions, static_text) =\n                identity_resolution::resolve_module_with_core(\n                    &mut core_proofs,frontend.as_deref(),\n                    &program.path,\n                    &synthetic,\n                    private,\n"),
            ("    ))\n}\n\nuse crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;\nuse crate::workspace_graph::validation::before_roots::{\n    validate_stub_signatures_authored, WorkspaceValidationIndex,\n};\n", "    ))\n}\n\nuse crate::workspace_graph::validation::before_roots::{WorkspaceValidationIndex, validate_stub_signatures_authored};\nuse crate::workspace_graph::before_private_roots::reconstruct_workspace_declaration_facts_with_authority;\n"),
        ]),
        ("src/workspace_graph/diagnostics.rs", "1c1552d56400ec781eb96ff6fd825bbbeec854f5042fab960e4d3d2941e60127") => (include_str!("workspace_graph/diagnostics.rs"), 15942, "b0785724bdbde5253b6a1aee53779d668f4fdbd13fe079badb586082b4045612", &[
            ("use crate::diagnostic::Diagnostic;\n\nuse super::{\n    WorkspaceResolvedModule, MAX_BUILDER_BYTES, MAX_CALLABLES, MAX_CALLS, MAX_CROSS_FILE_EDGES,\n    MAX_DECLARATIONS, MAX_DEPENDENCY_DEPTH, MAX_ENTRY_MODULE_BYTES, MAX_FILES, MAX_OUTPUT_BYTES,\n    MAX_TOTAL_SOURCE_BYTES, MAX_USES,\n};\n\npub(super) const PROVIDER_MAIN_HELP: &str = \"`entry` in semaprax.toml must name the module that declares `main`; every other listed source is a provider module and declares no `main`\";\n", "use crate::diagnostic::Diagnostic;\n\nuse super::{\n    MAX_BUILDER_BYTES, MAX_CALLABLES, MAX_CALLS, MAX_CROSS_FILE_EDGES, MAX_DECLARATIONS,\n    MAX_DEPENDENCY_DEPTH, MAX_ENTRY_MODULE_BYTES, MAX_FILES, MAX_OUTPUT_BYTES,\n    MAX_TOTAL_SOURCE_BYTES, MAX_USES, WorkspaceResolvedModule,\n};\n\npub(super) const PROVIDER_MAIN_HELP: &str = \"`entry` in semaprax.toml must name the module that declares `main`; every other listed source is a provider module and declares no `main`\";\n"),
        ]),
        ("src/workspace_graph/constructor_failure/typed.rs", "1482ad177d1d605a183cccc3ef185aaed36e7214b1bf4db8065a466303fc99f6") => (restored_typed.as_deref().unwrap_or(include_str!("workspace_graph/constructor_failure/typed.rs")), 11670, "b07ecf1da889cf25e1b9296bf320070d49c58af316efe6036dce534fccd73fb7", &[
            ("/// Reserved wrappers and ordinary function imports can trigger a full probe;\n", "/// Only these reserved identity families can reach FunctionEntry::Borrowed;\n"),
            ("    if SCALAR_SIGNATURE_BEFORE.with(std::cell::Cell::get) {\n        return before_scalar_dispatch::construct_selected_core(\n            program,\n            authored,\n            programs,\n            registry,\n            core,\n            allow_transient,\n            owner,\n        );\n    }\n    #[cfg(test)]\n", ""),
            ("            before_scalar_dispatch::construct_transient_alias_core(\n                program, authored, programs, registry, core, owner,\n            )\n", "            construct_transient_alias_core(program, authored, programs, registry, core, owner)\n"),
            ("    owner.reserve(program.module_uses.len().saturating_mul(\n        PER_IMPORT_BYTES + 2 * size_of::<&crate::ast::ModuleUseKind>() + 2 * size_of::<bool>(),\n    ))?;\n    let possible = program.module_uses.iter().any(|import| {\n        may_borrow_wrapper(import.persistent_id.as_str())\n            || import.kind == crate::ast::ModuleUseKind::Function\n    });\n", "    owner.reserve(program.module_uses.len().saturating_mul(PER_IMPORT_BYTES))?;\n    let possible = program\n        .module_uses\n        .iter()\n        .any(|import| may_borrow_wrapper(import.persistent_id.as_str()));\n"),
            ("\n#[cfg(test)]\n#[path = \"before_scalar_dispatch.rs\"]\npub(in crate::workspace_graph) mod before_scalar_dispatch;\n\n#[cfg(test)]\nthread_local! {\n    static SCALAR_SIGNATURE_BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };\n}\n/// The entire immediate-old actual selector and its original producer choices.\n#[cfg(test)]\npub(in crate::workspace_graph) fn with_original_scalar_dispatch<T>(run: impl FnOnce() -> T) -> T {\n    struct Restore(bool);\n    impl Drop for Restore {\n        fn drop(&mut self) {\n            SCALAR_SIGNATURE_BEFORE.with(|value| value.set(self.0));\n        }\n    }\n    let restore = Restore(SCALAR_SIGNATURE_BEFORE.with(|value| value.replace(true)));\n    let result = run();\n    drop(restore);\n    result\n}\n", ""),
        ]),
        ("src/workspace_graph/expected_projection/defaults/typed.rs", "9761dcd2ec5fd2351807f7526479680a56c51211eb584385ca85dbc893f38fb7") => (include_str!("workspace_graph/expected_projection/defaults/typed.rs"), 6321, "3316c665e3b4cc8cdcf5c8a71c90ba721f2e67df36c9ed743c58e6008ca55233", &[
            ("    checked_builder_sum, Span, StructuralCost, Type, TypeDeclaration, TypeDeclarationKind,\n", "    Span, StructuralCost, Type, TypeDeclaration, TypeDeclarationKind, checked_builder_sum,\n"),
        ]),
        ("src/workspace_graph/expected_projection/import_stub/typed.rs", "c220a90bfa131f6b75131e461ead26aea1dcee6c3ad09c42e5f481f5c12c5b11") => (include_str!("workspace_graph/expected_projection/import_stub/typed.rs"), 1466, "cbb50f93fbfdb170e83b2955461b50a018f7541179d79ab613ef502cef8b8d31", &[
            ("use super::{forward_parameter, record_shape, Expr, ExprKind, Function, ParamMode, Type};\n", "use super::{Expr, ExprKind, Function, ParamMode, Type, forward_parameter, record_shape};\n"),
        ]),
        _ => return None,
    };
    assert_eq!(
        current, compiled,
        "actual complete compiled constructor ENTRY"
    );
    assert_eq!(current.len(), length);
    assert_eq!(source_sha(current), current_sha);
    let mut entry = current.to_owned();
    for &(new, old) in edits {
        assert_eq!(
            entry.matches(new).count(),
            1,
            "unique complete constructor ENTRY hunk"
        );
        entry = entry.replacen(new, old, 1);
    }
    assert_eq!(source_sha(&entry), expected);
    Some(entry)
}

fn constructor825_attempt_before(current: &str) -> String {
    assert_eq!(
        current,
        include_str!("workspace_graph/constructor_failure/attempt.rs")
    );
    assert_eq!(current.len(), 4136);
    assert_eq!(
        source_sha(current),
        "a6ee9ade44a91d067b516603e52453d965b0e4bd8060add721ebe057f5fcb082"
    );
    let added = "    #[cfg(feature = \"unstable-workflow-profiling\")]\n    pub(in crate::workspace_graph) fn observation_succeeded(&self) -> bool {\n        self.result.is_ok()\n    }\n";
    assert_eq!(current.matches(added).count(), 1);
    let before = current.replacen(added, "", 1);
    assert_eq!(before.len(), 3973);
    assert_eq!(
        source_sha(&before),
        "6a4ab15e6a334f636565dc83a21411c4ba40c6de8e61445c015309c94bc892f3"
    );
    before
}

fn initial845_current_owner_before(source: &str, current: &str) -> Option<String> {
    let normalized = constructor_stage::owner(source, current);
    let current = normalized.as_deref().unwrap_or(current);
    let (compiled, length, current_sha, old_length, old_sha, edits): (&str, usize, &str, usize, &str, &[(&str, &str)]) = match source {
        "src/bounded_output.rs" => (normalized.as_deref().unwrap_or(include_str!("bounded_output.rs")), 52412, "fa91c8eca4825931211d066d6ebde8c0ff157de1ec78b5950abe867c295e481f", 51963, "f9379a489970b6cb369265386226ecaa82e2d657355d50eebedb9e33c8b6589a", &[
            ("#[cfg(feature = \"unstable-workflow-profiling\")]\npub(crate) mod exclusive_debits;\n", ""),
            ("                    #[cfg(feature = \"unstable-workflow-profiling\")]\n                    exclusive_debits::exceptional_settlement(&parent, remaining.min(consumed));\n", ""),
            ("        #[cfg(feature = \"unstable-workflow-profiling\")]\n        exclusive_debits::refusal(budget, length);\n", ""),
            ("    #[cfg(feature = \"unstable-workflow-profiling\")]\n    exclusive_debits::debit(budget, length);\n", ""),
        ]),
        "src/workspace_graph/initial_report/build.rs" => (normalized.as_deref().unwrap_or(include_str!("workspace_graph/initial_report/build.rs")), 17509, "48b56f916c25d00004bf6d8bc13e56a0972d07f531fceda2c7c94b57d2af20e1", 17054, "0fd9df9abe4ceadd45105394a4efcda2edc3a3ab780d0326d924288a60ea5bda", &[
            ("                #[cfg(feature = \"unstable-workflow-profiling\")]\n                let mut exclusive = crate::bounded_output::exclusive_debits::attempt(\n                    resolve_builder_bytes,\n                    layouts.receipt(),\n                    fallback_mode,\n                );\n                let result = constructor_failure::attempt::run(active_builder_limit(), || {\n", "                constructor_failure::attempt::run(active_builder_limit(), || {\n"),
            ("                });\n                #[cfg(feature = \"unstable-workflow-profiling\")]\n                exclusive.complete(result.observation_succeeded());\n                result\n", "                })\n"),
        ]),
        "src/workspace_graph/initial_report/tests.rs" => (normalized.as_deref().unwrap_or(include_str!("workspace_graph/initial_report/tests.rs")), 15256, "0f530933b3964478e93a13bd7065cda6a97ab121b7c77508db45e72aff401ad7", 13095, "52afb2ca386cf27e4bd8bf2e70c46565539427a51b11a1cdded57b3762570971", &[
            ("    let complete_before = crate::source_reflows_merge::initial845_input(\n        \"src/workspace_graph/initial_report/before.rs\", include_str!(\"before.rs\"),\n    );\n    assert!(complete_before.contains(&body));\n", "    assert!(include_str!(\"before.rs\").contains(&body));\n"),
            ("\n#[cfg(feature = \"unstable-workflow-profiling\")]\n#[test]\nfn exclusive_core_capture_keeps_complete_real_builder_outputs_and_every_original_limit() {\n    use crate::workflow_profile::plan_comparison::{Binding, Capture, CoreDebitAlgorithm};\n    for limit in [0, 1, 127, 1023, 8191, MAX_BUILDER_BYTES] {\n        let run = || {\n            build_owned_retaining_sources_with_frontend_limit::<Vec<Diagnostic>>(\n                sources(),\n                limit,\n                None,\n                false,\n                None,\n            )\n            .map(|(build, _)| build.source_graph_schemas().unwrap())\n        };\n        let plain = run();\n        let capture = Capture::begin(Binding::caller_labelled(\n            \"real initial builder parity\",\n            [0; 32],\n        ))\n        .unwrap();\n        let watched = run();\n        let report = capture.finish().unwrap();\n        assert_eq!(\n            plain, watched,\n            \"actual full compiler product at unchanged limit {limit}\"\n        );\n        assert_eq!(\n            report.resolved_core.algorithm,\n            CoreDebitAlgorithm::InstrumentedLibraryTest\n        );\n        assert!(\n            report.resolved_core.complete\n                && !report.resolved_core.invalid\n                && !report.resolved_core.lost\n                && !report.resolved_core.overflow,\n            \"{report:?}\"\n        );\n        for attempt in &report.resolved_core.attempts[..report.resolved_core.stored_attempts] {\n            assert!(attempt.complete && !attempt.invalid);\n            assert_eq!(attempt.stage_sum(), Some(attempt.consumed));\n            assert_eq!(attempt.operation_sum(), Some(attempt.consumed));\n        }\n        if limit == MAX_BUILDER_BYTES {\n            assert!(watched.is_ok());\n            assert!(report.resolved_core.entered_attempts > 0);\n            assert_eq!(\n                report.resolved_core.final_attempt_ordinal,\n                Some(report.resolved_core.entered_attempts - 1)\n            );\n        }\n    }\n}\n", ""),
        ]),
        _ => return None,
    };
    assert_eq!(
        current, compiled,
        "complete actual current #845 source owner"
    );
    assert_eq!(current.len(), length);
    assert_eq!(source_sha(current), current_sha);
    let mut before = current.to_owned();
    for &(new, old) in edits {
        assert_eq!(
            before.matches(new).count(),
            1,
            "unique entire #845 owner inverse"
        );
        before = before.replacen(new, old, 1);
    }
    assert_eq!(before.len(), old_length);
    assert_eq!(source_sha(&before), old_sha);
    Some(before)
}

// Closed complete legacy ledger ENTRY after the authentic current #908 inverse.
fn initial845_ledger_entry(source: &str, current: &str) -> Option<String> {
    if source != "src/bounded_output.rs" {
        return None;
    }
    assert_eq!(current.len(), 51963);
    assert_eq!(
        source_sha(current),
        "f9379a489970b6cb369265386226ecaa82e2d657355d50eebedb9e33c8b6589a"
    );
    let mut entry = current.to_owned();
    for &(new, old) in &[
        ("pub(crate) mod cfg_reservation;\npub(crate) mod format_acceptance;\npub(crate) mod identity_reservation;\npub(crate) mod key_directory;\npub(crate) mod prepaid;\n", "pub(crate) mod format_acceptance;\npub(crate) mod cfg_reservation;\npub(crate) mod identity_reservation;\npub(crate) mod key_directory;\npub(crate) mod prepaid;\n"),
        ("            ACTIVE.with(|active| {\n                active.replace(previous.clone());\n            });\n            #[cfg(feature = \"unstable-workflow-profiling\")]\n            crate::workflow_profile::plan_comparison::observe_ledger_transition();\n            if let Some(parent) = previous {\n                if !reserve(Some(&parent), consumed) {\n                    record_refusal(&parent, consumed);\n", "            ACTIVE.with(|active| {\n                active.replace(previous.clone());\n            });\n            if let Some(parent) = previous {\n                if !reserve(Some(&parent), consumed) {\n                    record_refusal(&parent, consumed);\n"),
        ("        stage: Cell::new(None),\n    });\n    let previous = ACTIVE.with(|active| active.replace(Some(Rc::clone(&budget))));\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    crate::workflow_profile::plan_comparison::observe_ledger_transition();\n    let restore = Restore {\n        previous,\n        current: Rc::clone(&budget),\n", "        stage: Cell::new(None),\n    });\n    let previous = ACTIVE.with(|active| active.replace(Some(Rc::clone(&budget))));\n    let restore = Restore {\n        previous,\n        current: Rc::clone(&budget),\n"),
        ("    stage: ReservationStage,\n    operation: impl FnOnce() -> T,\n) -> T {\n    #[cfg(test)]\n    let _allocation_phase = crate::test_allocation_observer::phase_guard(stage);\n    let restore = reservation_stage_guard(stage);\n    let value = operation();\n    drop(restore);\n", "    stage: ReservationStage,\n    operation: impl FnOnce() -> T,\n) -> T {\n    let restore = reservation_stage_guard(stage);\n    let value = operation();\n    drop(restore);\n"),
        ("    if let Some(budget) = active() {\n        budget.floor.set(0);\n    }\n}\n\n// Read-only feature evidence; no ledger reference or reservation authority escapes.\n#[cfg(feature = \"unstable-workflow-profiling\")]\npub(crate) fn active_ledger_snapshot()\n-> Option<crate::workflow_profile::plan_comparison::LedgerSnapshot> {\n    ACTIVE.with(|active| {\n        let active = active.try_borrow().ok()?;\n        let budget = active.as_ref()?;\n        crate::workflow_profile::plan_comparison::LedgerSnapshot::observe(\n            Rc::as_ptr(budget) as usize,\n            budget.remaining.get(),\n        )\n    })\n}\n\npub(crate) fn active_remaining() -> Option<usize> {\n", "    if let Some(budget) = active() {\n        budget.floor.set(0);\n    }\n}\n\npub(crate) fn active_remaining() -> Option<usize> {\n"),
        ("        );\n    }\n}\n\n/// Keep the original clone's text debit and refusal/floor selection while\n/// borrowing its immutable input instead of allocating a temporary String.\n/// The caller must hold that input through the returned view. This grants no\n/// heap backing or authority and retains the original clone tariff unchanged.\npub(crate) fn borrowed_clone_view(value: &str) -> &str {\n    let budget = active();\n    if !reserve(budget.as_deref(), value.len()) {\n        return \"\";\n    }\n    value\n}\n\n// Same input, budget owner, reserve receiver and bool as budgeted_clone. Its\n// old String result slot covers this replacement borrowed-text result; no\n// unused bytes are credited and no heap-carrier allowance is borrowed.\nconst _: () = {\n    assert!(std::mem::size_of::<&str>() <= std::mem::size_of::<String>());\n    assert!(std::mem::align_of::<&str>() <= std::mem::align_of::<String>());\n};\n", "        );\n    }\n}\n"),
] {
        assert_eq!(entry.matches(new).count(), 1, "unique complete legacy ledger ENTRY hunk");
        entry = entry.replacen(new, old, 1);
    }
    assert_eq!(entry.len(), 50190);
    assert_eq!(
        source_sha(&entry),
        "d01a82d602c286044cc1d5df8d05d02dfe5168d1139a82380aa01b06e47c7fd5"
    );
    Some(entry)
}

// Fixed complete original #845 owner inputs; all historical pipelines remain intact.
fn initial845_compiled_owner_entry(source: &str, current: &str) -> Option<String> {
    let (length, sha, prior_length, prior_sha, edits): (usize, &str, usize, &str, &[(&str, &str)]) = match source {
        "src/workspace_graph/initial_report/before.rs" => (17548, "ae04e8fc229cd104dc096c44f968119aa76395676ca90b821aec2f4e14131f27", 17445, "5c7f71670fb7bfb0ffd77071fcfa60c24230692e310644ce47263a59d43ab87f", &[
            ("            if let Some(frontend) = frontend.as_deref_mut() {\n                frontend.canonicalized();\n            }\n            canonical\n                .bytes\n                .expect(\"canonical byte count checked before source equality\")\n        };\n        canonical_bytes = checked_usage(\n            canonical_bytes,\n", "            if let Some(frontend) = frontend.as_deref_mut() {\n                frontend.canonicalized();\n            }\n            canonical.bytes.expect(\"canonical byte count checked before source equality\")\n        };\n        canonical_bytes = checked_usage(\n            canonical_bytes,\n"),
            ("                );\n                constructor_failure::attempt::run(active_builder_limit(), || {\n                    constructor_failure::attempt::reserve(layouts.receipt())?;\n                    staged!(\n                        CorePrebound,\n                        constructor_failure::attempt::reserve(resolve_builder_bytes)\n                    )?;\n                    let custody = custody::SourceCustody::plan(\n                        &sources,\n                        &programs,\n", "                );\n                constructor_failure::attempt::run(active_builder_limit(), || {\n                    constructor_failure::attempt::reserve(layouts.receipt())?;\n                    staged!(CorePrebound, constructor_failure::attempt::reserve(resolve_builder_bytes))?;\n                    let custody = custody::SourceCustody::plan(\n                        &sources,\n                        &programs,\n"),
            ("    Ok((build, sources))\n}\n\n// The exact original terminal Vec producer, independently callable after a\n// real failed attempt. The whole original Err site remains in the body above.\npub(super) fn terminal(\n", "    Ok((build, sources))\n}\n\n\n// The exact original terminal Vec producer, independently callable after a\n// real failed attempt. The whole original Err site remains in the body above.\npub(super) fn terminal(\n"),
        ]),
        "src/workspace_graph/initial_report/before_mappers.rs" => (4561, "69b2e94928db4f1f68c5172258be1c1332c802581d84dba238b8455664bcceea", 4109, "d736b5a92caa2a355e5fa1a2112469297272afd673b704c4195224e782eb3e1d", &[
            ("//! Complete original owning mapper bodies; only lexical visibility adapts.\n\npub(super) mod change {\n    use crate::diagnostic::Diagnostic;\n\n    use crate::semantic_workspace;\n\n    pub(in crate::workspace_graph::initial_report) fn map_change_builder_limit(\n        diagnostics: Vec<Diagnostic>,\n        internal_maximum: usize,\n    ) -> Vec<Diagnostic> {\n        let expected = crate::bounded_output::budgeted_format(format_args!(\n            \"Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}\"\n        ));\n        if diagnostics.len() == 1\n            && diagnostics[0].code == \"SPX-G171\"\n            && diagnostics[0].message == expected\n        {\n            limit(\n                \"total_analysis_builder_bytes\",\n                semantic_workspace::MAX_CHANGE_BUILDER_BYTES,\n            )\n        } else {\n            diagnostics\n        }\n    }\n\n    fn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {\n        vec![Diagnostic::io(\n            \"SPX-G183\",\n            crate::bounded_output::budgeted_format(format_args!(\n                \"Semantic Workspace Change `{field}` exceeds {maximum}\"\n            )),\n        )]\n    }\n}\n\npub(super) mod structural {\n    use crate::diagnostic::Diagnostic;\n\n    pub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize =\n        16 * 1024 * 1024;\n\n    pub(in crate::workspace_graph::initial_report) const MAX_ANALYSIS_BUILDER_BYTES: usize =\n        32 * 1024 * 1024;\n\n    pub(in crate::workspace_graph::initial_report) fn map_candidate_builder_limit(\n        diagnostics: Vec<Diagnostic>,\n        internal_maximum: usize,\n    ) -> Vec<Diagnostic> {\n        let expected = crate::bounded_output::budgeted_format(format_args!(\n            \"Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}\"\n        ));\n        if diagnostics.len() == 1\n            && diagnostics[0].code == \"SPX-G171\"\n            && diagnostics[0].message == expected\n        {\n            limit(\n                if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\n                    \"candidate_graph_builder_bytes\"\n                } else {\n                    \"analysis_builder_bytes\"\n                },\n                if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\n                    MAX_CANDIDATE_GRAPH_BUILDER_BYTES\n                } else {\n                    MAX_ANALYSIS_BUILDER_BYTES\n                },\n            )\n        } else {\n            diagnostics\n        }\n    }\n\n    fn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {\n        vec![Diagnostic::io(\n            \"SPX-G191\",\n            crate::bounded_output::budgeted_format(format_args!(\n                \"Semantic Workspace Structural Change limit exceeded: {field} maximum {maximum}\"\n            )),\n        )]\n    }\n}\n\npub(super) mod operations {\n    use crate::diagnostic::Diagnostic;\n\n    pub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize =\n        16_777_216;\n\n    pub(in crate::workspace_graph::initial_report) const MAX_OPERATIONS_BUILDER_BYTES: usize =\n        67_108_864;\n\n    pub(in crate::workspace_graph::initial_report) fn map_candidate_diagnostics(\n        diagnostics: Vec<Diagnostic>,\n        candidate_graph_limit: usize,\n        operations_builder_limit: usize,\n    ) -> Vec<Diagnostic> {\n        if diagnostics.len() != 1 || diagnostics[0].code != \"SPX-G171\" {\n            return diagnostics;\n        }\n        let builder_message =\n            format!(\"Workspace Semantic Graph `builder_bytes` exceeds {candidate_graph_limit}\");\n        if diagnostics[0].message == builder_message {\n            return if candidate_graph_limit == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\n                limit(\n                    \"candidate_graph_builder_bytes\",\n                    MAX_CANDIDATE_GRAPH_BUILDER_BYTES,\n                )\n            } else {\n                limit(\"operations_builder_bytes\", MAX_OPERATIONS_BUILDER_BYTES)\n            };\n        }\n        let change_message = format!(\n            \"Workspace Semantic Graph `change_builder_bytes` exceeds {}\",\n            operations_builder_limit\n        );\n        if diagnostics[0].message == change_message {\n            return limit(\"operations_builder_bytes\", MAX_OPERATIONS_BUILDER_BYTES);\n        }\n        diagnostics\n    }\n\n    fn limit(f: &str, m: usize) -> Vec<Diagnostic> {\n        vec![Diagnostic::io(\n            \"SPX-G199\",\n            format!(\"Semantic Workspace Operations exceeds {f} maximum {m}\"),\n        )]\n    }\n}\n", "//! Complete original owning mapper bodies; only lexical visibility adapts.\n\npub(super) mod change {\nuse crate::diagnostic::Diagnostic;\n\nuse crate::semantic_workspace;\n\npub(in crate::workspace_graph::initial_report) fn map_change_builder_limit(\n    diagnostics: Vec<Diagnostic>,\n    internal_maximum: usize,\n) -> Vec<Diagnostic> {\n    let expected = crate::bounded_output::budgeted_format(format_args!(\n        \"Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}\"\n    ));\n    if diagnostics.len() == 1\n        && diagnostics[0].code == \"SPX-G171\"\n        && diagnostics[0].message == expected\n    {\n        limit(\n            \"total_analysis_builder_bytes\",\n            semantic_workspace::MAX_CHANGE_BUILDER_BYTES,\n        )\n    } else {\n        diagnostics\n    }\n}\n\nfn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {\n    vec![Diagnostic::io(\n        \"SPX-G183\",\n        crate::bounded_output::budgeted_format(format_args!(\n            \"Semantic Workspace Change `{field}` exceeds {maximum}\"\n        )),\n    )]\n}\n}\n\npub(super) mod structural {\nuse crate::diagnostic::Diagnostic;\n\npub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16 * 1024 * 1024;\n\npub(in crate::workspace_graph::initial_report) const MAX_ANALYSIS_BUILDER_BYTES: usize = 32 * 1024 * 1024;\n\npub(in crate::workspace_graph::initial_report) fn map_candidate_builder_limit(\n    diagnostics: Vec<Diagnostic>,\n    internal_maximum: usize,\n) -> Vec<Diagnostic> {\n    let expected = crate::bounded_output::budgeted_format(format_args!(\n        \"Workspace Semantic Graph `change_builder_bytes` exceeds {internal_maximum}\"\n    ));\n    if diagnostics.len() == 1\n        && diagnostics[0].code == \"SPX-G171\"\n        && diagnostics[0].message == expected\n    {\n        limit(\n            if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\n                \"candidate_graph_builder_bytes\"\n            } else {\n                \"analysis_builder_bytes\"\n            },\n            if internal_maximum == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\n                MAX_CANDIDATE_GRAPH_BUILDER_BYTES\n            } else {\n                MAX_ANALYSIS_BUILDER_BYTES\n            },\n        )\n    } else {\n        diagnostics\n    }\n}\n\nfn limit(field: &'static str, maximum: usize) -> Vec<Diagnostic> {\n    vec![Diagnostic::io(\n        \"SPX-G191\",\n        crate::bounded_output::budgeted_format(format_args!(\n            \"Semantic Workspace Structural Change limit exceeded: {field} maximum {maximum}\"\n        )),\n    )]\n}\n}\n\npub(super) mod operations {\nuse crate::diagnostic::Diagnostic;\n\npub(in crate::workspace_graph::initial_report) const MAX_CANDIDATE_GRAPH_BUILDER_BYTES: usize = 16_777_216;\n\npub(in crate::workspace_graph::initial_report) const MAX_OPERATIONS_BUILDER_BYTES: usize = 67_108_864;\n\npub(in crate::workspace_graph::initial_report) fn map_candidate_diagnostics(\n    diagnostics: Vec<Diagnostic>,\n    candidate_graph_limit: usize,\n    operations_builder_limit: usize,\n) -> Vec<Diagnostic> {\n    if diagnostics.len() != 1 || diagnostics[0].code != \"SPX-G171\" {\n        return diagnostics;\n    }\n    let builder_message =\n        format!(\"Workspace Semantic Graph `builder_bytes` exceeds {candidate_graph_limit}\");\n    if diagnostics[0].message == builder_message {\n        return if candidate_graph_limit == MAX_CANDIDATE_GRAPH_BUILDER_BYTES {\n            limit(\n                \"candidate_graph_builder_bytes\",\n                MAX_CANDIDATE_GRAPH_BUILDER_BYTES,\n            )\n        } else {\n            limit(\"operations_builder_bytes\", MAX_OPERATIONS_BUILDER_BYTES)\n        };\n    }\n    let change_message = format!(\n        \"Workspace Semantic Graph `change_builder_bytes` exceeds {}\",\n        operations_builder_limit\n    );\n    if diagnostics[0].message == change_message {\n        return limit(\"operations_builder_bytes\", MAX_OPERATIONS_BUILDER_BYTES);\n    }\n    diagnostics\n}\n\nfn limit(f: &str, m: usize) -> Vec<Diagnostic> {\n    vec![Diagnostic::io(\n        \"SPX-G199\",\n        format!(\"Semantic Workspace Operations exceeds {f} maximum {m}\"),\n    )]\n}\n}\n"),
        ]),
        "src/workspace_graph/initial_report/build.rs" => (17054, "0fd9df9abe4ceadd45105394a4efcda2edc3a3ab780d0326d924288a60ea5bda", 16803, "607af6cab243e1970aed326c8dc7ce7c211ffe902f4e425467599538aef013d1", &[
            ("                })\n            });\n        let (core, paid_refusal) = core.into_parts();\n        #[cfg(test)]\n        tests::record_core_attempt(\n            fallback_mode,\n            resolve_builder_bytes,\n            consumed,\n            overflowed,\n            &refusal,\n            &site,\n            &core.as_ref().err(),\n        );\n        core_builder_bytes = core_builder_bytes.max(consumed);\n        if !overflowed {\n            break core;\n", "                })\n            });\n        let (core, paid_refusal) = core.into_parts();\n        core_builder_bytes = core_builder_bytes.max(consumed);\n        if !overflowed {\n            break core;\n"),
        ]),
        _ => return None,
    };
    assert_eq!(current.len(), length);
    assert_eq!(source_sha(current), sha);
    let mut prior = current.to_owned();
    for &(new, old) in edits {
        assert_eq!(
            prior.matches(new).count(),
            1,
            "unique complete original845 owner hunk"
        );
        prior = prior.replacen(new, old, 1);
    }
    assert_eq!(prior.len(), prior_length);
    assert_eq!(source_sha(&prior), prior_sha);
    Some(prior)
}

fn source_sha(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
