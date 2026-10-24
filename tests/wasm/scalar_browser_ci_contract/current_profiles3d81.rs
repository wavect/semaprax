//! Complete observed profiles and exact source bridges to the retained controls.
use super::{graph_digest, HOSTED909, NATIVE909};
use sha2::{Digest, Sha256};
use std::borrow::Cow;

const HOSTED: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-capture-3d81-20261024/calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-capture-3d81-20261024/calculator-renamed.json"),
];
const NATIVE: [&str; 2] = [
    include_str!(
        "../../../docs/evidence/browser-capture-3d81-20261024/native-calculator-baseline.json"
    ),
    include_str!(
        "../../../docs/evidence/browser-capture-3d81-20261024/native-calculator-renamed.json"
    ),
];
const PREVIOUS_HOSTED: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-capture-573-20261024/calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-capture-573-20261024/calculator-renamed.json"),
];
const PREVIOUS_NATIVE: [&str; 2] = [
    include_str!(
        "../../../docs/evidence/browser-capture-573-20261024/native-calculator-baseline.json"
    ),
    include_str!(
        "../../../docs/evidence/browser-capture-573-20261024/native-calculator-renamed.json"
    ),
];

fn wire(text: &'static str) -> &'static str {
    text.strip_suffix('\n').unwrap()
}

fn sha(text: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}

pub(super) fn recognizes(graph: &str) -> bool {
    HOSTED
        .into_iter()
        .chain(NATIVE)
        .any(|text| graph == wire(text))
}

fn restore_bill(graph: &str, previous: &str) -> String {
    let actual: serde_json::Value = serde_json::from_str(graph).unwrap();
    let prior: serde_json::Value = serde_json::from_str(previous).unwrap();
    assert_eq!(graph_digest(graph), actual["graph_digest"]);
    assert_eq!(graph_digest(previous), prior["graph_digest"]);
    let bill = actual["budget"]["used_builder_bytes"].as_u64().unwrap();
    assert!(bill > 0 && bill <= 67_108_864);
    let field = format!("\"used_builder_bytes\":{bill}");
    assert_eq!(graph.matches(&field).count(), 1);
    let restored = graph.replacen(
        &field,
        &format!(
            "\"used_builder_bytes\":{}",
            prior["budget"]["used_builder_bytes"]
        ),
        1,
    );
    let field = format!("\"graph_digest\":{}", actual["graph_digest"]);
    assert_eq!(restored.matches(&field).count(), 1);
    let restored = restored.replacen(
        &field,
        &format!("\"graph_digest\":{}", prior["graph_digest"]),
        1,
    );
    assert_eq!(
        restored, previous,
        "complete previous wire, including every other budget and limit"
    );
    restored
}

pub(super) fn pinned(graph: &str, role: &str) -> &'static str {
    let index = match role {
        "baseline" => 0,
        "renamed" => 1,
        _ => panic!("unknown exact current fixture role"),
    };
    let hosted = wire(HOSTED[index]);
    let native = wire(NATIVE[index]);
    assert!(
        graph == hosted || graph == native,
        "complete current role wire required"
    );
    let h: serde_json::Value = serde_json::from_str(hosted).unwrap();
    let n: serde_json::Value = serde_json::from_str(native).unwrap();
    assert_eq!(h["budget"]["used_builder_bytes"], 2_999_812);
    assert_eq!(n["budget"]["used_builder_bytes"], 3_000_028);
    assert_eq!(restore_bill(native, hosted), hosted);
    let prior_hosted = wire(PREVIOUS_HOSTED[index]);
    let prior_native = wire(PREVIOUS_NATIVE[index]);
    assert_eq!(restore_bill(hosted, prior_hosted), prior_hosted);
    assert_eq!(restore_bill(native, prior_native), prior_native);
    assert_eq!(
        super::current_profiles573::pinned(prior_hosted, role),
        prior_hosted
    );
    assert_eq!(
        super::current_profiles573::pinned(prior_native, role),
        prior_hosted
    );
    hosted
}

fn compiled(source: &str) -> &'static str {
    match source {
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs" => {
            include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs")
        }
        "tests/wasm/scalar_browser_ci_contract/historical_builder.rs" => {
            include_str!("historical_builder.rs")
        }
        "tests/wasm/scalar_browser_ci_contract/current_profiles573.rs" => {
            include_str!("current_profiles573.rs")
        }
        _ => panic!("foreign source bridge"),
    }
}

fn before(source: &str) -> &'static str {
    match source {
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs" => include_str!("../../../docs/evidence/browser-capture-3d81-20261024/runner.before.mjs"),
        "tests/wasm/scalar_browser_ci_contract/historical_builder.rs" => include_str!("../../../docs/evidence/browser-capture-3d81-20261024/historical_builder.before.rs.txt"),
        "tests/wasm/scalar_browser_ci_contract/current_profiles573.rs" => include_str!("../../../docs/evidence/browser-capture-3d81-20261024/current_profiles573.before.rs.txt"),
        _ => panic!("foreign source bridge"),
    }
}

fn prior_source<'a>(source: &str, current: &'a str) -> Cow<'a, str> {
    let data =
        include_str!("../../../docs/evidence/browser-capture-3d81-20261024/source-inverses.json");
    assert!(data.len() <= 262_144);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        std::fs::read_to_string(
            root.join("docs/evidence/browser-capture-3d81-20261024/source-inverses.json")
        )
        .unwrap(),
        data
    );
    let records: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(
        records["schema"],
        "semaprax.browser-complete-source-bridge.v1"
    );
    assert_eq!(records["rows"].as_array().unwrap().len(), 3);
    let rows: Vec<_> = records["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["source"] == source)
        .collect();
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    let previous = before(source);
    assert_eq!(sha(previous), row["previous_sha256"]);
    // This exact historical input was already accepted by the retained receiver.
    if current == previous {
        return Cow::Borrowed(current);
    }
    assert_eq!(sha(current), row["current_sha256"]);
    assert_eq!(current, compiled(source));
    assert_eq!(current, std::fs::read_to_string(root.join(source)).unwrap());
    let mut restored = current.to_owned();
    for edit in row["inverse"].as_array().unwrap().iter().rev() {
        let new = edit["current"].as_str().unwrap();
        assert!(!new.is_empty());
        assert_eq!(
            restored.matches(new).count(),
            1,
            "unique whole-source inverse"
        );
        restored = restored.replacen(new, edit["previous"].as_str().unwrap(), 1);
    }
    assert_eq!(
        restored, previous,
        "entire original receiver or runner retained"
    );
    Cow::Owned(restored)
}

pub(super) fn prior_runner(current: &str) -> Cow<'_, str> {
    prior_source(
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs",
        current,
    )
}

pub(super) fn prior_owner(current: &str) -> Cow<'_, str> {
    prior_source(
        "tests/wasm/scalar_browser_ci_contract/historical_builder.rs",
        current,
    )
}

pub(super) fn runner_for_graph(graph: &str, runner: &str) -> Option<String> {
    if runner != compiled("platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs") {
        return None;
    }
    if !PREVIOUS_HOSTED
        .into_iter()
        .chain(PREVIOUS_NATIVE)
        .chain(HOSTED909)
        .chain(NATIVE909)
        .any(|text| graph == wire(text))
    {
        return None;
    }
    Some(prior_runner(runner).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_current_profiles_replay_all_previous_complete_controls() {
        for (index, role) in ["baseline", "renamed"].into_iter().enumerate() {
            for raw in [HOSTED[index], NATIVE[index]] {
                assert_eq!(
                    super::super::pinned_fixture_graph(wire(raw), role),
                    wire(HOSTED[index])
                );
            }
            assert_eq!(
                super::super::current_browser_digest(wire(HOSTED[index]), role),
                [
                    "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
                    "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
                ][index]
            );
        }
    }

    #[test]
    fn changed_bill_semantics_limits_roles_and_incomplete_wires_remain_refused() {
        for (index, role) in ["baseline", "renamed"].into_iter().enumerate() {
            for raw in [HOSTED[index], NATIVE[index]] {
                let graph = wire(raw);
                let value: serde_json::Value = serde_json::from_str(graph).unwrap();
                for (key, old) in [
                    (
                        "used_builder_bytes",
                        value["budget"]["used_builder_bytes"].as_u64().unwrap(),
                    ),
                    ("used_call_sites", 10),
                ] {
                    let field = format!("\"{key}\":{old}");
                    assert_eq!(graph.matches(&field).count(), 1);
                    let changed = graph.replacen(&field, &format!("\"{key}\":{}", old + 1), 1);
                    let digest_field = format!("\"graph_digest\":{}", value["graph_digest"]);
                    let changed = changed.replacen(
                        &digest_field,
                        &format!("\"graph_digest\":\"{}\"", graph_digest(&changed)),
                        1,
                    );
                    assert!(
                        std::panic::catch_unwind(|| super::super::pinned_fixture_graph(
                            &changed, role
                        ))
                        .is_err()
                    );
                }
                let changed = graph.replacen(
                    "\"budget\":{",
                    "\"budget\":{\"max_builder_bytes\":67108865,",
                    1,
                );
                assert!(
                    std::panic::catch_unwind(|| super::super::pinned_fixture_graph(&changed, role))
                        .is_err()
                );
                for wrong in ["foreign", if index == 0 { "renamed" } else { "baseline" }] {
                    assert!(
                        std::panic::catch_unwind(|| super::super::pinned_fixture_graph(
                            graph, wrong
                        ))
                        .is_err()
                    );
                }
                for changed in [
                    String::new(),
                    format!("{graph}\n"),
                    graph[..graph.len() - 1].to_owned(),
                ] {
                    assert!(
                        std::panic::catch_unwind(|| super::super::pinned_fixture_graph(
                            &changed, role
                        ))
                        .is_err()
                    );
                }
            }
        }
    }

    #[test]
    fn complete_previous_sources_are_preserved_and_foreign_sources_fail_closed() {
        for source in [
            "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs",
            "tests/wasm/scalar_browser_ci_contract/historical_builder.rs",
            "tests/wasm/scalar_browser_ci_contract/current_profiles573.rs",
        ] {
            let current = compiled(source);
            assert_eq!(prior_source(source, current), before(source));
            for foreign in [
                String::new(),
                format!("{current}\n"),
                current[..current.len() - 1].to_owned(),
            ] {
                assert!(std::panic::catch_unwind(|| prior_source(source, &foreign)).is_err());
            }
        }
    }
}
