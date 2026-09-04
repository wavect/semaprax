use std::fs;
use std::path::Path;

fn read(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn contains_adjacent_lines(text: &str, first: &str, second: &str) -> bool {
    text.lines()
        .zip(text.lines().skip(1))
        .any(|(left, right)| left == first && right == second)
}

#[test]
fn external_consumer_is_isolated_default_feature_and_exactly_locked() {
    let manifest = read("platform-tests/public-scalar-wit-interface/Cargo.toml");
    assert!(manifest.contains("[workspace]\nresolver = \"2\""));
    assert!(manifest.contains(
        "semaprax = { version = \"=0.3.0\", path = \"../..\", default-features = false }"
    ));
    assert!(!manifest.contains("unstable-wit-component-harness"));
    assert!(manifest.contains(
        "wit-parser = { version = \"=0.252.0\", default-features = false, features = [\"std\"] }"
    ));

    let root_manifest = read("Cargo.toml");
    assert!(!root_manifest.contains("wit-parser"));
    let root_lock = read("Cargo.lock");
    assert!(!root_lock.contains("name = \"wit-parser\""));

    let lock = read("platform-tests/public-scalar-wit-interface/Cargo.lock");
    assert!(contains_adjacent_lines(
        &lock,
        "name = \"wit-parser\"",
        "version = \"0.252.0\""
    ));
    assert!(lock.contains("name = \"semaprax-public-scalar-wit-interface-consumer\""));
}

#[test]
fn exact_lock_rows_accept_unix_and_windows_checkout_newlines() {
    for lock in [
        "name = \"wit-parser\"\nversion = \"0.252.0\"\n",
        "name = \"wit-parser\"\r\nversion = \"0.252.0\"\r\n",
    ] {
        assert!(contains_adjacent_lines(
            lock,
            "name = \"wit-parser\"",
            "version = \"0.252.0\""
        ));
    }
}

#[test]
fn external_consumer_script_acquires_then_runs_locked_and_offline() {
    let script = read("scripts/public-scalar-wit-interface.sh");
    let offline = script
        .find("export CARGO_NET_OFFLINE=true")
        .expect("script must establish the offline boundary");
    for acquisition in [
        "cargo fetch --locked --manifest-path \"$readonly_root/Cargo.toml\"",
        "cargo fetch --locked --manifest-path \"$readonly_manifest\"",
    ] {
        let position = script
            .find(acquisition)
            .unwrap_or_else(|| panic!("missing acquisition command: {acquisition}"));
        assert!(
            position < offline,
            "dependency acquisition crossed offline boundary"
        );
    }
    for command in ["cargo clippy", "cargo test", "cargo run"] {
        let line = script
            .lines()
            .find(|line| line.starts_with(command))
            .unwrap_or_else(|| panic!("missing {command} command"));
        assert!(line.contains("--locked"), "unlocked command: {line}");
        assert!(line.contains("--offline"), "online command: {line}");
        assert!(line.contains("--manifest-path \"$readonly_manifest\""));
        assert!(
            script.find(line).expect("command must be present") > offline,
            "command precedes offline boundary: {line}"
        );
    }
    assert!(script.contains(
        "cargo fmt --manifest-path \"$readonly_manifest\" --package semaprax-public-scalar-wit-interface-consumer -- --check"
    ));
}

#[test]
fn consumer_uses_only_the_public_retained_interface_and_maintained_parser() {
    let source = read("platform-tests/public-scalar-wit-interface/src/main.rs");
    for required in [
        "with_authenticated_project",
        "scalar_wit_interface_v1",
        "replay_scalar_wit_interface_v1",
        "Resolve::default()",
        ".push_str(\"project-scalar-v1.wit\", artifact.wit())",
        "TypeDefKind::Result",
        "TypeDefKind::Record",
        "TypeDefKind::Option(Type::Bool)",
        "project-scalar-v1",
    ] {
        assert!(
            source.contains(required),
            "missing external-consumer check: {required}"
        );
    }
    assert!(!source.contains("wit_component"));
    assert!(!source.contains("emit_resolved_module"));
}
