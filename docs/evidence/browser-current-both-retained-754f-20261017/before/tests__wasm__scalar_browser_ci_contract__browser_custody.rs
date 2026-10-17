//! Strict independent whole-source inputs for both retained browser branches.
use sha2::{Digest, Sha256};
fn hash(source: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}
fn view(
    current: &str,
    current_sha: &str,
    previous: &str,
    previous_sha: &str,
    edits: &[(&str, &str)],
) -> String {
    assert_eq!(hash(current), current_sha, "physical browser source domain");
    assert_eq!(
        hash(previous),
        previous_sha,
        "complete retained browser input"
    );
    let mut restored = current.to_owned();
    for (after, before) in edits {
        assert_eq!(
            restored.matches(after).count(),
            1,
            "unique whole-source edit"
        );
        restored = restored.replacen(after, before, 1);
    }
    assert_eq!(restored, previous, "every original branch input byte");
    restored
}

pub(super) fn parent_for_ours(current: &str) -> String {
    let original = pin_input("tests/wasm/scalar_browser_ci_contract.rs", current);
    let current = original.as_str();
    view(
        current,
        "93e328799e7200273eadd54438c27d3e1872fe831aa299e27b6f160f4e8f98f3",
        include_str!(
            "../../../docs/evidence/main-eb13-browser-join-20261016/parent_for_ours.before.txt"
        ),
        "edc7963894937fc560afd392a71c0039cd8c687eec489b2e44bea82f46b17a03",
        &[(
            r######"                        .expect("capture complete actual calculator graph");
                }
                previous_digests.push(historical_builder::main_digest(revision.semantic_graph()));
                let actual = [
                    revision.project_revision(),
"######,
            r######"                        .expect("capture complete actual calculator graph");
                }
                previous_digests.push(historical_builder::current_digest(revision.semantic_graph()));
                let actual = [
                    revision.project_revision(),
"######,
        )],
    )
}

pub(super) fn parent_for_main(current: &str) -> String {
    let original = pin_input("tests/wasm/scalar_browser_ci_contract.rs", current);
    let current = original.as_str();
    view(
        current,
        "93e328799e7200273eadd54438c27d3e1872fe831aa299e27b6f160f4e8f98f3",
        include_str!(
            "../../../docs/evidence/main-eb13-browser-join-20261016/parent_for_main.before.txt"
        ),
        "2e5e392cb33e2eb998093b21c29c2cab4decb8e85e99bbbaf27ca0a6fb50f9d5",
        &[(
            r######"                        .expect("capture complete actual calculator graph");
                }
                previous_digests.push(historical_builder::main_digest(revision.semantic_graph()));
                let actual = [
                    revision.project_revision(),
"######,
            r######"                        .expect("capture complete actual calculator graph");
                }
                previous_digests.push(historical_builder::digest(revision.semantic_graph()));
                let actual = [
                    revision.project_revision(),
"######,
        )],
    )
}

pub(super) fn runner_for_ours(current: &str) -> String {
    let original = pin_input(
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs",
        current,
    );
    let current = original.as_str();
    view(
        current,
        "b84b2940d4f909f7240c3e698b3f920546b4f99e8ed141560c782e848708b935",
        include_str!(
            "../../../docs/evidence/main-eb13-browser-join-20261016/runner_for_ours.before.txt"
        ),
        "873c7e40a692eebcc5e4bd468f66cc866b09eff146000aea4d30706b7b5aa4fe",
        &[
            (
                r######"  result,
}));
// Exact hosted compiler records from 268cb0523, including the complete current
// builder bill. The Rust browser contract retains full historical wire and
// cursor controls; docs/evidence/browser-graph-pins-268cb0523 binds this capture.
const fixtureSpecs = [
  { name: "direct-source", schema: "semaprax.web.v4" },
"######,
                r######"  result,
}));
// Actual local 35cc compiler records include the complete current builder bill.
// The Rust browser contract retains every complete historical wire and cursor
// control; docs/evidence/browser-graph-pins-35cc-20261016 binds this capture.
const fixtureSpecs = [
  { name: "direct-source", schema: "semaprax.web.v4" },
"######,
            ),
            (
                r######"    project_revision: "sha256:8576caa566cb7f0d265354927c5bc7b481146f05e616f76917f340b4af26f053",
    workspace_revision: "sha256:f0454397a2b339677bc49c9ccd8e8491917426202c6aba2475221879e02ae3f6",
    project_graph_digest: "sha256:041e8b6ec3039aecf3c176f271b21239426c6e3a933e6bc116c9e9433463cc5d",
  },
  {
"######,
                r######"    project_revision: "sha256:8576caa566cb7f0d265354927c5bc7b481146f05e616f76917f340b4af26f053",
    workspace_revision: "sha256:f0454397a2b339677bc49c9ccd8e8491917426202c6aba2475221879e02ae3f6",
    project_graph_digest: "sha256:82b22b0f2c49b594045bef876d491bb182df03a734b34b9a7430464e0c38f347",
  },
  {
"######,
            ),
            (
                r######"    project_revision: "sha256:afa7b35b6b057eaa1cbf89c68ccd1e19a8d988f4168049f70717f80c28218fb7",
    workspace_revision: "sha256:8fcf973950f10bf9393ff5597484333b178e5f93c5d2a1847f6ccc18d6185f71",
    project_graph_digest: "sha256:952423e076aa2792f1b9e46d031eaac56357fce2dfeaa5670fe401afe30c9870",
  },
];
"######,
                r######"    project_revision: "sha256:afa7b35b6b057eaa1cbf89c68ccd1e19a8d988f4168049f70717f80c28218fb7",
    workspace_revision: "sha256:8fcf973950f10bf9393ff5597484333b178e5f93c5d2a1847f6ccc18d6185f71",
    project_graph_digest: "sha256:d47fc1d22cb152e19f9f99e64ff2d675babb111a75c9d4aaac639d032729abcf",
  },
];
"######,
            ),
        ],
    )
}

pub(super) fn before_2600(current: &str) -> String {
    view(
        current,
        "c8f7162f8910572826e18000efd3415464a0f3653581c05b4b0a431c2761e32c",
        include_str!(
            "../../../docs/evidence/main-eb13-browser-join-20261016/before_2600.before.txt"
        ),
        "e9a110ae30efac1012deed4c9ac4750dbc4f82e4cb698f1b33508872d968ca26",
        &[(
            r######"    );
    let current = include_str!("../scalar_browser_ci_contract.rs");
    let current = super::super::custody::parent_for_main(current);
    let changes = [
        ("#[path = \"scalar_browser_ci_contract/historical_builder.rs\"]\nmod historical_builder;\n\n", ""),
"######,
            r######"    );
    let current = include_str!("../scalar_browser_ci_contract.rs");
    let changes = [
        ("#[path = \"scalar_browser_ci_contract/historical_builder.rs\"]\nmod historical_builder;\n\n", ""),
"######,
        )],
    )
}

pub(super) fn pin_input(source: &str, current: &str) -> String {
    let previous = browser_b80_previous(source, current);
    let current = previous.as_str();
    pin_project(
        include_str!(
            "../../../docs/evidence/actual47f-pin-fixture-successor-20261016/current-to-47f.json"
        ),
        source,
        current,
    )
}
fn pin_project(rows: &str, source: &str, current: &str) -> String {
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
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
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
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(result.as_bytes()))
        ),
        row["original_sha256"]
    );
    result
}

fn browser_b80_previous(source: &str, current: &str) -> String {
    let pinned = actual83ca_input(source, current);
    let current = pinned.as_str();
    pin_project(
        include_str!(
            "../../../docs/evidence/main-b80-browser-custody-20261016/current-to-df2.json"
        ),
        source,
        current,
    )
}
fn browser_c451_input(source: &str, current: &str) -> String {
    let pinned = actual83ca_input(source, current);
    let current = pinned.as_str();
    pin_project(
        include_str!(
            "../../../docs/evidence/main-b80-browser-custody-20261016/current-to-main.json"
        ),
        source,
        current,
    )
}
pub(super) fn main_c451_parent(current: &str) -> String {
    browser_c451_input("tests/wasm/scalar_browser_ci_contract.rs", current)
}
pub(super) fn main_c451_runner(current: &str) -> String {
    browser_c451_input(
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs",
        current,
    )
}
pub(super) fn main_c451_before(current: &str) -> String {
    browser_c451_input(
        "tests/wasm/scalar_browser_ci_contract/before_c451.rs",
        current,
    )
}
pub(super) fn main_c451_before_2600(current: &str) -> String {
    browser_c451_input(
        "tests/wasm/scalar_browser_ci_contract/main_c451_before_2600.rs",
        current,
    )
}

fn actual83ca_input(source: &str, current: &str) -> String {
    let merged = main8a_previous(source, current);
    let current = merged.as_str();
    let main65 = pin_project(
        include_str!(
            "../../../docs/evidence/main65-private-merge-20261016/browser-current-to-75d8.json"
        ),
        source,
        current,
    );
    let current = main65.as_str();
    pin_project(
        include_str!(
            "../../../docs/evidence/actual83ca-pin-successor-20261016/browser-current-to-83ca.json"
        ),
        source,
        current,
    )
}

pub(super) fn main8a_previous(source: &str, current: &str) -> String {
    let restored = before_e47_input(source, current);
    let current = restored.as_str();
    pin_project(
        include_str!(
            "../../../docs/evidence/main8a-independent-custody-20261016/browser-current-to55e.json"
        ),
        source,
        current,
    )
}
pub(super) fn main8a_input(source: &str, current: &str) -> String {
    let restored = before_e47_input(source, current);
    let current = restored.as_str();
    pin_project(include_str!("../../../docs/evidence/main8a-independent-custody-20261016/browser-current-toMain8a.json"), source, current)
}

// Independent outer seam; every old immutable browser table sees exact4251.
pub(super) fn before_e47_input(source: &str, current: &str) -> String {
    let restored = current67b_input(source, current);
    let current = restored.as_str();
    pin_project(
        include_str!(
            "../../../docs/evidence/browser-main75b-independent-20261016/current-to-prior.json"
        ),
        source,
        current,
    )
}

// Exact outer inverse. Every older source predicate receives its full preimage.
pub(super) fn current67b_input(source: &str, current: &str) -> String {
    let restored = next_actual_input(source, current);
    let current = restored.as_str();
    pin_project(
        include_str!("../../../docs/evidence/browser-67b1b-successor-20261016/current-to-prior.json"),
        source,
        current,
    )
}
pub(super) fn current67b_runner75b(current: &str) -> String {
    let restored = next_actual_input("platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs", current);
    let current = restored.as_str();
    pin_project(
        include_str!("../../../docs/evidence/browser-67b1b-successor-20261016/current-to-retained75b-runner.json"),
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs",
        current,
    )
}

pub(super) fn next_actual_input(source: &str, current: &str) -> String {
    let current = main671_input(source, current);
    let current = current.as_str();
    pin_project(include_str!("../../../docs/evidence/browser-next-actual-218a9-20261016/current-to-prior.json"), source, current)
}

// CLOSED MAIN671 outer acquisition; every older browser algorithm remains above.
pub(super) fn main671_input(source: &str, current: &str) -> String {
    let raw = match source {
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs" =>
            include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs"),
        "tests/wasm/scalar_browser_ci_contract/historical_builder.rs" => include_str!("historical_builder.rs"),
        "tests/wasm/scalar_browser_ci_contract/browser_custody.rs" => include_str!("browser_custody.rs"),
        "tests/wasm/scalar_browser_ci_contract/next_actual.rs" => include_str!("next_actual.rs"),
        _ => return current.to_owned(),
    };
    // Literal compile-time whole bytes bind each actual acquisition, including
    // this owner itself, without a circular source/table self-hash claim.
    assert_eq!(current, raw, "exact compiled MAIN671 browser acquisition");
    if source == "tests/wasm/scalar_browser_ci_contract/next_actual.rs" {
        let edit = "    let runner = super::custody::main671_input(\"platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs\", runner);\n";
        assert_eq!(current.matches(edit).count(), 1);
        let restored = current.replacen(edit, "", 1);
        assert_eq!(hash(&restored), "0d33eea6239ab16b61f0db4344cba9e4b854b95c0ab95275fecde8fe44c469c9");
        return restored;
    }
    // This immutable old table owns the exact218a9 input stage, which differs
    // from492's already-composed MAIN524 runner and added MAIN7f8 namespace.
    // Acquire its complete current-stage preimage BEFORE the unchanged inverse.
    let table = include_str!("../../../docs/evidence/browser-next-actual-218a9-20261016/current-to-prior.json");
    assert_eq!(hash(table), "e472406e433dd61bd26a852637996981a53b64cf4db9feb054185fb6e752c74a");
    let rows: serde_json::Value = serde_json::from_str(table).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 5);
    assert_eq!(rows.as_array().unwrap().iter().filter(|row| row["source"] == source).count(), 1);
    let row = rows.as_array().unwrap().iter().find(|row| row["source"] == source).unwrap();
    assert_eq!(row["edits"].as_array().unwrap().len(), 1);
    let restored = row["edits"][0]["new"].as_str().unwrap().to_owned();
    assert_eq!(hash(&restored), row["current_sha256"]);
    restored
}
