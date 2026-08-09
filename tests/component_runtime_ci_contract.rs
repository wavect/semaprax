use std::{fs, path::Path};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read(relative: &str) -> String {
    fs::read_to_string(root().join(relative))
        .unwrap_or_else(|error| panic!("failed to read {relative}: {error}"))
}

#[test]
fn standalone_runner_is_pinned_private_and_outside_the_root_workspace() {
    let attributes = read(".gitattributes");
    assert!(
        attributes
            .lines()
            .any(|line| line == "platform-tests/component-runtime/** text eol=lf"),
        "the byte-locked runtime inputs must retain canonical LF checkouts on Windows"
    );

    let manifest = read("platform-tests/component-runtime/Cargo.toml");
    for required in [
        "publish = false",
        "license = \"Apache-2.0\"",
        "[workspace]",
        "resolver = \"2\"",
        "semaprax = { version = \"=0.2.0\", path = \"../..\", default-features = false, features = [\"unstable-wit-component-harness\"] }",
        "sha2 = { version = \"=0.10.9\", default-features = false }",
        "wasmtime = { version = \"=47.0.3\", default-features = false, features = [\"component-model\", \"cranelift\", \"runtime\", \"std\"] }",
        "unsafe_code = \"forbid\"",
    ] {
        assert!(manifest.contains(required), "missing runner lock: {required}");
    }
    for forbidden in ["wasmtime-wasi", "wasi-common", "version = \"*\""] {
        assert!(
            !manifest.contains(forbidden),
            "runner manifest grants forbidden dependency surface: {forbidden}"
        );
    }

    let root_manifest = read("Cargo.toml");
    assert!(root_manifest.contains("license = \"Apache-2.0\""));
    assert_eq!(
        root_manifest
            .lines()
            .find(|line| line.starts_with("members = ")),
        Some("members = [\"crates/semaprax-native-host\", \"crates/semaprax-native-loader\"]")
    );

    let toolchain = read("platform-tests/component-runtime/rust-toolchain.toml");
    assert!(toolchain.contains("channel = \"1.97.1\""));
    assert!(toolchain.contains("profile = \"minimal\""));
    assert_eq!(
        read("platform-tests/component-runtime/toolchain.lock"),
        "rustc.release=1.97.1\nrustc.commit=8bab26f4f68e0e26f0bb7960be334d5b520ea452\nrustc.commit-date=2026-07-14\nrustc.llvm=22.1.6\ncargo.release=1.97.1\ncargo.commit=c980f4866141969fab6254a680546a277789d6f0\ncargo.commit-date=2026-06-30\nci.host=x86_64-unknown-linux-gnu\nwasmtime.version=47.0.3\n"
    );

    let lock = read("platform-tests/component-runtime/Cargo.lock");
    assert!(lock.contains("name = \"wasmtime\"\nversion = \"47.0.3\""));
    assert!(lock.contains("name = \"semaprax-private-component-runtime-v3\""));
    for forbidden in [
        "name = \"wasmtime-wasi\"",
        "name = \"wasi-common\"",
        "name = \"wasip2\"",
        "name = \"wasip3\"",
    ] {
        assert!(
            !lock.contains(forbidden),
            "standalone lock contains ambient runtime crate: {forbidden}"
        );
    }
}

#[test]
fn runtime_wit_is_the_exact_existing_private_result_contract() {
    let checked_in = read("platform-tests/component-runtime/wit/semaprax-private-v1.wit");
    let expected = "package semaprax:private@0.1.0;\n\ninterface evaluation {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  evaluate: func(left: s64, right: s64) -> result<s64, status>;\n}\n\nworld semaprax-private-v1 {\n  export evaluation;\n}\n";
    assert_eq!(checked_in, expected);
}

#[test]
fn component_job_fetches_root_and_runner_then_runs_every_gate_locked_and_offline() {
    let script = read("scripts/component-runtime-v3.sh");
    assert_eq!(script.matches("cargo fetch").count(), 2);
    assert!(script.contains("export CARGO_NET_OFFLINE=true"));
    for mandatory_root_gate in [
        "cargo fetch --locked --manifest-path \"$readonly_root/Cargo.toml\"",
        "cargo fetch --locked --manifest-path \"$readonly_manifest\"",
        "--features unstable-wit-component-harness --lib wit_component::result_v3::tests::",
        "--test component_runtime_ci_contract",
    ] {
        assert!(
            script.contains(mandatory_root_gate),
            "missing mandatory root Component gate: {mandatory_root_gate}"
        );
    }
    for exact_toolchain_field in [
        "release: 1.97.1",
        "commit-hash: 8bab26f4f68e0e26f0bb7960be334d5b520ea452",
        "commit-date: 2026-07-14",
        "host: x86_64-unknown-linux-gnu",
        "LLVM version: 22.1.6",
        "commit-hash: c980f4866141969fab6254a680546a277789d6f0",
        "commit-date: 2026-06-30",
    ] {
        assert!(script.contains(exact_toolchain_field));
    }
    for command in ["cargo clippy", "cargo test", "cargo run"] {
        let line = script
            .lines()
            .find(|line| line.starts_with(command))
            .unwrap_or_else(|| panic!("missing {command}"));
        assert!(line.contains("--locked"), "unlocked command: {line}");
        assert!(line.contains("--offline"), "online command: {line}");
        assert!(line.contains("--manifest-path"), "unscoped command: {line}");
    }

    let workflow = read(".github/workflows/ci.yml");
    for required in [
        "component-runtime-v3:",
        "name: Private Wasmtime Component result runtime",
        "runs-on: ubuntu-24.04",
        "toolchain: 1.97.1",
        "manifest-path: platform-tests/component-runtime/Cargo.toml",
        "--config platform-tests/component-runtime/deny.toml",
        "scripts/component-runtime-v3.sh",
    ] {
        assert!(
            workflow.contains(required),
            "missing CI source lock: {required}"
        );
    }
    assert!(
        !workflow.contains("arguments: --manifest-path"),
        "cargo-deny arguments duplicated the action's dedicated manifest-path input"
    );
}

#[test]
fn capability_and_dependency_policy_are_fail_closed() {
    let deny = read("platform-tests/component-runtime/deny.toml");
    for required in [
        "wildcards = \"deny\"",
        "unknown-registry = \"deny\"",
        "unknown-git = \"deny\"",
        "name = \"wasmtime-wasi\"",
        "name = \"wasi-common\"",
    ] {
        assert!(deny.contains(required), "missing deny rule: {required}");
    }

    let runner = read("platform-tests/component-runtime/src/main.rs");
    for required in [
        "wasm_component_model(true)",
        "Component::new",
        "component.component_type().imports(&engine)",
        "Linker::<()>::new(engine)",
        "call_evaluate",
        "config.consume_fuel(true)",
        "store.set_fuel(0)",
        "engine_failure.is_ok()",
        "CASES.into_iter().chain(CASES)",
        "for case in CASES",
        "(left + 1) / right",
        "name: \"addition-overflow\"",
        "left: i64::MAX",
        "name: \"division-by-zero\"",
        "name: \"sticky-add-overflow-before-division-by-zero\"",
        "name: \"false-precondition\"",
        "name: \"false-postcondition\"",
    ] {
        assert!(
            runner.contains(required),
            "missing runtime gate: {required}"
        );
    }
    for forbidden in [
        "wasmtime_wasi",
        "std::fs",
        "std::net",
        "std::env",
        "std::process",
        "func_wrap",
        "add_to_linker",
        "get_func",
        "get_typed_func",
        "Module::new",
        ".generated_core()",
        "artifact.digest()",
    ] {
        assert!(
            !runner.contains(forbidden),
            "runner contains ambient or untyped surface: {forbidden}"
        );
    }
}
