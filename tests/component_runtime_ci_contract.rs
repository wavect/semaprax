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
fn runtime_wits_are_the_exact_private_result_contracts() {
    let checked_in_v3 = read("platform-tests/component-runtime/wit/semaprax-private-v1.wit");
    let expected_v3 = "package semaprax:private@0.1.0;\n\ninterface evaluation {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  evaluate: func(left: s64, right: s64) -> result<s64, status>;\n}\n\nworld semaprax-private-v1 {\n  export evaluation;\n}\n";
    assert_eq!(checked_in_v3, expected_v3);

    let checked_in_v4 = read("platform-tests/component-runtime/wit/semaprax-private-v4.wit");
    let expected_v4 = "package semaprax:private@0.2.0;\n\ninterface evaluation {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  type language-result = result<bool, bool>;\n  evaluate: func(value: s64, reject: bool, divisor: s64) -> result<language-result, status>;\n}\n\nworld semaprax-private-v4 {\n  export evaluation;\n}\n";
    assert_eq!(checked_in_v4, expected_v4);

    let checked_in_v5 = read("platform-tests/component-runtime/wit/semaprax-private-v5.wit");
    let expected_v5 = "package semaprax:private@0.3.0;\n\ninterface scalar-algebra {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  type maybe-i64 = option<s64>;\n  type maybe-bool = option<bool>;\n  type language-result-i64-i64 = result<s64, s64>;\n  type language-result-i64-bool = result<s64, bool>;\n  type language-result-bool-i64 = result<bool, s64>;\n  type language-result-bool-bool = result<bool, bool>;\n  option-i64: func(value: s64, select: bool, divisor: s64) -> result<maybe-i64, status>;\n  option-bool: func(value: s64, select: bool, divisor: s64) -> result<maybe-bool, status>;\n  result-i64-i64: func(value: s64, select: bool, divisor: s64) -> result<language-result-i64-i64, status>;\n  result-i64-bool: func(value: s64, select: bool, divisor: s64) -> result<language-result-i64-bool, status>;\n  result-bool-i64: func(value: s64, select: bool, divisor: s64) -> result<language-result-bool-i64, status>;\n  result-bool-bool: func(value: s64, select: bool, divisor: s64) -> result<language-result-bool-bool, status>;\n}\n\nworld semaprax-private-v5 {\n  export scalar-algebra;\n}\n";
    assert_eq!(checked_in_v5, expected_v5);
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
        "--features unstable-wit-component-harness --lib wit_component::source_result_v4::tests::",
        "--features unstable-wit-component-harness --lib wasm::scalar_algebra_component_v5::tests::",
        "--features unstable-wit-component-harness --lib wit_component::scalar_algebra_v5::tests::",
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
        let mut found = false;
        for line in script.lines().filter(|line| line.starts_with(command)) {
            found = true;
            assert!(line.contains("--locked"), "unlocked command: {line}");
            assert!(line.contains("--offline"), "online command: {line}");
            assert!(line.contains("--manifest-path"), "unscoped command: {line}");
        }
        assert!(found, "missing {command}");
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
        "execute every v3/v4/v5 Component gate offline",
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
        "SemapraxPrivateV4::instantiate",
        "SemapraxPrivateV5::instantiate",
        "#[allow(clippy::too_many_lines)]\nfn run_v5_instance",
        "This is deliberately a flat, reviewable six-export protocol matrix.",
        "semaprax-private-v4.wit",
        "semaprax-private-v5.wit",
        "EXPECTED_COMPONENT_V4_SHA256",
        "EXPECTED_GENERATED_CORE_V4_SHA256",
        "EXPECTED_SOURCE_REVISION_V4",
        "EXPECTED_COMPONENT_V5_SHA256",
        "EXPECTED_GENERATED_CORE_V5_SHA256",
        "EXPECTED_SOURCE_REVISION_V5",
        "sha256:86411224efe3adace5ffdd410c243306859edc280dbe3342adcf830588b62259",
        "0x6c, 0xeb, 0x9e, 0x30, 0x96, 0x94, 0xa5, 0xb9",
        "0x08, 0x25, 0xf2, 0x70, 0xcf, 0x2c, 0x94, 0xbd",
        "sha256:4391bc27b5db547f2b162c2b5467c2b75797e8a5ef64e4ffe4abef15678c6254",
        "validate_private_source_result_component_v4",
        "validate_private_scalar_algebra_component_v5",
        "config.consume_fuel(true)",
        "store.set_fuel(0)",
        "engine_failure.is_ok()",
        "CASES.into_iter().chain(CASES)",
        "CASES_V4.into_iter().chain(CASES_V4)",
        "for case in CASES",
        "for case in CASES_V4",
        "call_option_i64",
        "call_option_bool",
        "call_result_i64_i64",
        "call_result_i64_bool",
        "call_result_bool_i64",
        "call_result_bool_bool",
        "semaprax:private/scalar-algebra@0.3.0",
        "(left + 1) / right",
        "name: \"addition-overflow\"",
        "left: i64::MAX",
        "name: \"division-by-zero\"",
        "name: \"sticky-add-overflow-before-division-by-zero\"",
        "name: \"false-precondition\"",
        "name: \"false-postcondition\"",
        "name: \"inner-ok-true\"",
        "name: \"inner-ok-false\"",
        "name: \"inner-err-true\"",
        "name: \"inner-err-false\"",
        "let checked = source(value, reject)?;",
        "(checked + 1) / divisor > 0",
        "name: \"false-postcondition-after-ok\"",
        "name: \"false-postcondition-after-err\"",
        "0x7d, 0x86, 0x44, 0x38, 0x49, 0x48, 0xf5, 0x91",
        "0xd5, 0x5f, 0x76, 0xa0, 0xe6, 0x97, 0x47, 0x77",
        "0x3e, 0x7b, 0x9c, 0x2d, 0xdc, 0x8c, 0xa6, 0xfd",
        "0x54, 0xfa, 0x28, 0x22, 0xc5, 0x1a, 0x71, 0xce",
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
        "artifact.digest()",
    ] {
        assert!(
            !runner.contains(forbidden),
            "runner contains ambient or untyped surface: {forbidden}"
        );
    }
}
