//! Exact observed wires; every previous receiver and raw role check remains.
use super::{graph_digest, HOSTED909, NATIVE909};
use sha2::{Digest, Sha256};

const HOSTED: [&str; 2] = [
    include_str!("../../../docs/evidence/browser-capture-573-20261024/calculator-baseline.json"),
    include_str!("../../../docs/evidence/browser-capture-573-20261024/calculator-renamed.json"),
];
const NATIVE: [&str; 2] = [
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
        "complete previous wire, including all other budgets"
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
    assert_eq!(h["budget"]["used_builder_bytes"], 2_997_308);
    assert_eq!(n["budget"]["used_builder_bytes"], 2_997_524);
    assert_eq!(restore_bill(native, hosted), hosted);
    let previous = wire(HOSTED909[index]);
    assert_eq!(restore_bill(hosted, previous), previous);
    assert_eq!(
        restore_bill(native, wire(NATIVE909[index])),
        wire(NATIVE909[index])
    );
    // Execute the complete previous exact-profile receiver on its whole input.
    assert_eq!(super::pinned_fixture_graph(previous, role), previous);
    hosted
}

pub(super) fn runner_for_graph(graph: &str, runner: &str) -> Option<String> {
    if !HOSTED909
        .into_iter()
        .chain(NATIVE909)
        .any(|text| graph == wire(text))
    {
        return None;
    }
    if runner != include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs") {
        return None;
    }
    Some(before_runner(runner))
}

fn before_runner(current: &str) -> String {
    assert_eq!(sha(current), RUNNER_SHA);
    let mut restored = current.to_owned();
    for (new, old) in RUNNER_INVERSES {
        assert_eq!(restored.matches(new).count(), 1);
        restored = restored.replacen(new, old, 1);
    }
    assert_eq!(
        restored,
        include_str!("../../../docs/evidence/browser-capture-573-20261024/runner.before.mjs")
    );
    restored
}

fn sha(text: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}

fn before_owner(current: &str) -> String {
    assert_eq!(sha(current), OWNER_SHA);
    let mut restored = current.to_owned();
    for (new, old) in OWNER_INVERSES {
        assert_eq!(restored.matches(new).count(), 1);
        restored = restored.replacen(new, old, 1);
    }
    assert_eq!(
        restored,
        include_str!(
            "../../../docs/evidence/browser-capture-573-20261024/historical_builder.before.rs.txt"
        )
    );
    restored
}

#[test]
fn current_complete_profiles_replay_every_previous_bridge_and_raw_role() {
    for (index, role) in ["baseline", "renamed"].into_iter().enumerate() {
        for raw in [HOSTED[index], NATIVE[index]] {
            let graph = wire(raw);
            assert_eq!(
                super::pinned_fixture_graph(graph, role),
                wire(HOSTED[index])
            );
            assert_eq!(
                super::current_browser_digest(wire(HOSTED[index]), role),
                [
                    "sha256:489f02ed298c2d35433d6c9f811ccc26c50ce90117b3976a862b40aff11129b8",
                    "sha256:ccb0ec81a00f3890ec2fe87af8d0f801970a1a397eeb55fe115e83dab02188ba",
                ][index]
            );
        }
    }
}

fn rehash(graph: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
    let field = format!("\"graph_digest\":{}", parsed["graph_digest"]);
    assert_eq!(graph.matches(&field).count(), 1);
    graph.replacen(
        &field,
        &format!("\"graph_digest\":\"{}\"", graph_digest(graph)),
        1,
    )
}

#[test]
fn current_profiles_reject_rehashed_bill_semantics_limit_role_and_truncation() {
    for (index, role) in ["baseline", "renamed"].into_iter().enumerate() {
        for raw in [HOSTED[index], NATIVE[index]] {
            let graph = wire(raw);
            let parsed: serde_json::Value = serde_json::from_str(graph).unwrap();
            for (field, value) in [
                (
                    "used_builder_bytes",
                    parsed["budget"]["used_builder_bytes"].as_u64().unwrap(),
                ),
                ("used_call_sites", 10),
            ] {
                let before = format!("\"{field}\":{value}");
                assert_eq!(graph.matches(&before).count(), 1);
                let changed =
                    rehash(&graph.replacen(&before, &format!("\"{field}\":{}", value + 1), 1));
                assert!(
                    std::panic::catch_unwind(|| super::pinned_fixture_graph(&changed, role))
                        .is_err()
                );
            }
            let changed = rehash(&graph.replacen(
                "\"budget\":{",
                "\"budget\":{\"max_builder_bytes\":67108865,",
                1,
            ));
            assert!(
                std::panic::catch_unwind(|| super::pinned_fixture_graph(&changed, role)).is_err()
            );
            for wrong in ["other", if index == 0 { "renamed" } else { "baseline" }] {
                assert!(
                    std::panic::catch_unwind(|| super::pinned_fixture_graph(graph, wrong)).is_err()
                );
            }
            for foreign in [
                String::new(),
                format!("{graph}\n"),
                graph[..graph.len() - 1].to_owned(),
            ] {
                assert!(
                    std::panic::catch_unwind(|| super::pinned_fixture_graph(&foreign, role))
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn complete_owner_and_runner_inverses_preserve_every_original_body_and_refuse_drift() {
    let owner = include_str!("historical_builder.rs");
    let runner = include_str!("../../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs");
    for (current, restore) in [
        (owner, before_owner as fn(&str) -> String),
        (runner, before_runner),
    ] {
        let old = restore(current);
        for foreign in [
            String::new(),
            format!("{current}\n"),
            current[..current.len() - 1].to_owned(),
            old,
        ] {
            assert!(std::panic::catch_unwind(|| restore(&foreign)).is_err());
        }
    }
}

// Exact complete-byte bindings are populated from the reviewed source delta.
const OWNER_SHA: &str = "d08980293493968e085698d7f3b040966af3ad5c9a133603ed191358ffa12b83";
const RUNNER_SHA: &str = "7abfc4358d9419d8eeb8d2b92594a51392c0e3028b220d3230499b097094fbbb";
const OWNER_INVERSES: [(&str, &str); 3] = [
("\nuse sha2::{Digest, Sha256};\n\n#[path = \"current_profiles573.rs\"]\nmod current_profiles573;\n", "\nuse sha2::{Digest, Sha256};\n"),
("fn current_browser_with_runner(graph: &str, role: &str, runner: &str) -> String {\n    let previous_runner = current_profiles573::runner_for_graph(graph, runner);\n    let runner = previous_runner.as_deref().unwrap_or(runner);\n", "fn current_browser_with_runner(graph: &str, role: &str, runner: &str) -> String {\n"),
("pub(super) fn pinned_fixture_graph(graph: &str, role: &str) -> &'static str {\n    if current_profiles573::recognizes(graph) {\n        return current_profiles573::pinned(graph, role);\n    }\n", "pub(super) fn pinned_fixture_graph(graph: &str, role: &str) -> &'static str {\n"),
];
const RUNNER_INVERSES: [(&str, &str); 5] = [
("e1f47ee56377b3a9b523e8a01c1093b3b8a437a1f564c75be2cf0569f2805292", "4fb7f079823914af14237eaf7e9b2f8eab2bb560116447cce7963033d3aab1f6"),
("3d9aaf7664b21c216c364732409ec65e6b881acff49c1b6735e922775d2a401b", "ef6ae3561ac646386e27326b43b48508ef5c8160e74595fca3b22727dfb0c5db"),
("79b3a2310aadd6f0329f3c97eb9c435118ee779cb71f49fd53bbc47872ea8b46", "0e89531754449a635a8aafbe5864e0006eb0d103b8706081a7df6ff86a2220c1"),
("5b75499eb996e46ef670e01464102656be4efb2f6cfa6bcd0b556cbe6e64d6a3", "e48d2abb60c094120a08dc765719b2df7580234fc473a0515ba54d9d2c3814be"),
("// Current complete graph captures: docs/evidence/browser-capture-573-20261024.\n// Linux 1.88 run 40031634129, job 120681907600, source 573623043c.\n// Prior bill-only captures: docs/evidence/browser-capture-909-20261023.", "// Current bill-only graph captures: docs/evidence/browser-capture-909-20261023.\n// Linux 1.88 run 39894187280, job 120221350676, source 909f0a4763."),
];
