//! MR-07 dependency and mounting gate (reads the tree; links nothing).
//!
//! - The decision core depends on `serde_json` and `sha2` only.
//! - The standalone `semaprax` package's resolved closure (normal, dev and
//!   build dependencies, as `Cargo.lock` records them) carries no private
//!   harness/toolchain crate, no decision-core crate and no model runtime.
//! - The core has exactly one implementation: this crate's library is the
//!   standalone package's `src/model_routing/engine/`, which must not name
//!   `crate::` so both mountings resolve alike.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// `[[package]]` name -> dependency names (version suffixes stripped).
fn lock_graph() -> BTreeMap<String, Vec<String>> {
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for block in read("Cargo.lock").split("[[package]]").skip(1) {
        let name = block
            .lines()
            .find_map(|l| l.strip_prefix("name = "))
            .map(|n| n.trim_matches('"').to_string())
            .expect("package name");
        let mut deps = Vec::new();
        if let Some(start) = block.find("dependencies = [") {
            let list = &block[start + "dependencies = [".len()..];
            let list = &list[..list.find(']').expect("closed dependency list")];
            for d in list.split(',') {
                let d = d.trim().trim_matches('"');
                if let Some(n) = d.split_whitespace().next() {
                    deps.push(n.to_string());
                }
            }
        }
        graph.entry(name).or_default().extend(deps);
    }
    graph
}

fn closure(graph: &BTreeMap<String, Vec<String>>, root: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![root.to_string()];
    while let Some(n) = stack.pop() {
        for d in graph.get(&n).into_iter().flatten() {
            if seen.insert(d.clone()) {
                stack.push(d.clone());
            }
        }
    }
    seen
}

/// Dependency-table entries of a manifest: (table header, line).
fn dependency_lines(manifest: &str) -> Vec<(String, String)> {
    let mut table = String::new();
    let mut out = Vec::new();
    for line in manifest.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            table = t.to_string();
        } else if table.ends_with("dependencies]") && !t.is_empty() && !t.starts_with('#') {
            out.push((table.clone(), t.to_string()));
        }
    }
    out
}

#[test]
fn decision_core_depends_on_serde_json_and_sha2_only() {
    let manifest = read("crates/semaprax-decision-core/Cargo.toml");
    let deps: BTreeSet<String> = dependency_lines(&manifest)
        .into_iter()
        .map(|(table, line)| {
            assert_eq!(table, "[dependencies]", "unexpected table {table}");
            line.split('=').next().unwrap().trim().to_string()
        })
        .collect();
    assert_eq!(
        deps,
        BTreeSet::from(["serde_json".to_string(), "sha2".to_string()])
    );
    let graph = lock_graph();
    let direct: BTreeSet<String> = graph["semaprax-decision-core"].iter().cloned().collect();
    assert_eq!(direct, deps, "Cargo.lock disagrees with the manifest");
    let mut allowed = closure(&graph, "serde_json");
    allowed.extend(closure(&graph, "sha2"));
    allowed.extend(deps);
    assert!(closure(&graph, "semaprax-decision-core").is_subset(&allowed));
}

#[test]
fn standalone_package_closure_has_no_private_or_model_runtime_crates() {
    let graph = lock_graph();
    let root = closure(&graph, "semaprax");
    for private in [
        "semaprax-harness",
        "semaprax-toolchain",
        "semaprax-decision-core",
    ] {
        assert!(
            !root.contains(private),
            "`semaprax` closure carries {private}"
        );
    }
    for runtime in [
        "pyo3",
        "candle-core",
        "ort",
        "tch",
        "tokenizers",
        "llama-cpp-2",
        "llama_cpp",
    ] {
        assert!(
            !root.contains(runtime),
            "`semaprax` closure carries {runtime}"
        );
    }
    // No path dependency (and so no workspace crate) in any root dependency table.
    for (table, line) in dependency_lines(&read("Cargo.toml")) {
        assert!(
            !line.contains("path =") && !line.starts_with("semaprax"),
            "root {table} names a workspace crate: {line}"
        );
    }
}

#[test]
fn the_core_has_one_implementation_mounted_twice() {
    let manifest = read("crates/semaprax-decision-core/Cargo.toml");
    assert!(manifest.contains("path = \"../../src/model_routing/engine/mod.rs\""));
    assert!(read("src/model_routing/mod.rs").contains("\npub mod engine;\n"));
    assert!(!repo().join("crates/semaprax-decision-core/src").exists());
    for entry in fs::read_dir(repo().join("src/model_routing/engine")).unwrap() {
        let path = entry.unwrap().path();
        let text = fs::read_to_string(&path).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap();
            assert!(
                !code.contains("crate::"),
                "{}:{}: `crate::` resolves differently in the two mountings",
                path.display(),
                n + 1
            );
            for ambient in [
                "std::fs",
                "std::env",
                "std::net",
                "std::process",
                "std::time",
            ] {
                assert!(
                    !code.contains(ambient),
                    "{}:{}: the core performs no ambient I/O ({ambient})",
                    path.display(),
                    n + 1
                );
            }
        }
    }
    // Harness code no longer carries the engine modules it re-exports.
    for moved in [
        "route",
        "route_v2",
        "policy",
        "rules",
        "plan",
        "render",
        "call",
        "cache",
        "replay",
        "consult",
        "model_profile",
        "registry",
    ] {
        let p = repo().join(format!("crates/semaprax-harness/src/decision/{moved}.rs"));
        assert!(!p.exists(), "duplicate engine module {}", p.display());
    }
}
