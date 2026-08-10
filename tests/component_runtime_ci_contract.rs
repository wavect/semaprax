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

    let checked_in_v6 = read("platform-tests/component-runtime/wit/semaprax-private-v6.wit");
    let expected_v6 = "package semaprax:private@0.4.0;\n\ninterface nested-records {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  record inner { value: s64, flag: bool }\n  record outer { inner: inner, other: s64 }\n  transform: func(input: outer, delta: s64) -> result<outer, status>;\n}\n\nworld semaprax-private-v6 {\n  export nested-records;\n}\n";
    assert_eq!(checked_in_v6, expected_v6);

    let checked_in_source_v6 = read("platform-tests/component-runtime/v6.spx");
    let expected_source_v6 = r#"module test.component_nested_record_v6;

@id("component.inner")
record Inner {
    @id("component.inner.value") value: i64,
    @id("component.inner.flag") flag: bool,
}

@id("component.outer")
record Outer {
    @id("component.outer.inner") inner: Inner,
    @id("component.outer.other") other: i64,
}

@id("component.transform")
fn transform(input: Outer, delta: i64) -> Outer
    requires delta != -99
    ensures delta != 13
{
    input with {
        inner: input.inner with { value: input.inner.value + delta },
        other: input.other / (delta - 1),
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(checked_in_source_v6, expected_source_v6);

    let checked_in_v7 = read("platform-tests/component-runtime/wit/semaprax-private-v7.wit");
    let expected_v7 = "package semaprax:private@0.5.0;\n\ninterface generic-records {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  record duo-i64-bool { left: s64, right: bool }\n  record duo-bool-i64 { left: bool, right: s64 }\n  record phantom-i64 { marker: bool }\n  record phantom-bool { marker: bool }\n  transform-i64-bool: func(input: duo-i64-bool, delta: s64, divisor: s64) -> result<duo-i64-bool, status>;\n  transform-bool-i64: func(input: duo-bool-i64, delta: s64, divisor: s64) -> result<duo-bool-i64, status>;\n  preserve-phantom-i64: func(input: phantom-i64) -> result<phantom-i64, status>;\n  invert-phantom-bool: func(input: phantom-bool) -> result<phantom-bool, status>;\n}\n\nworld semaprax-private-v7 {\n  export generic-records;\n}\n";
    assert_eq!(checked_in_v7, expected_v7);

    let checked_in_source_v7 = read("platform-tests/component-runtime/v7.spx");
    let expected_source_v7 = r#"module test.component_generic_record_v7;

@id("component.duo")
record Duo<T, U> {
    @id("component.duo.left") left: T,
    @id("component.duo.right") right: U,
}

@id("component.phantom")
record Phantom<T> {
    @id("component.phantom.marker") marker: bool,
}

@id("component.transform-i64-bool")
fn transform_i64_bool(input: Duo<i64, bool>, delta: i64, divisor: i64) -> Duo<i64, bool>
    requires delta != -99
    ensures divisor != 13
{
    input with { left: (input.left + delta) / divisor }
}

@id("component.transform-bool-i64")
fn transform_bool_i64(input: Duo<bool, i64>, delta: i64, divisor: i64) -> Duo<bool, i64>
    requires delta != -99
    ensures divisor != 13
{
    input with { right: (input.right + delta) / divisor }
}

@id("component.preserve-phantom-i64")
fn preserve_phantom_i64(input: Phantom<i64>) -> Phantom<i64> {
    input with { marker: input.marker }
}

@id("component.invert-phantom-bool")
fn invert_phantom_bool(input: Phantom<bool>) -> Phantom<bool> {
    input with { marker: !input.marker }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(checked_in_source_v7, expected_source_v7);

    let checked_in_v8 = read("platform-tests/component-runtime/wit/semaprax-private-v8.wit");
    let expected_v8 = "package semaprax:private@0.6.0;\n\ninterface record-pattern-projections {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  record phantom-i64 { marker: bool }\n  record phantom-bool { marker: bool }\n  preserve-phantom-i64: func(input: phantom-i64, control: s64) -> result<bool, status>;\n  invert-phantom-i64: func(input: phantom-i64, control: s64) -> result<bool, status>;\n  preserve-phantom-bool: func(input: phantom-bool, control: s64) -> result<bool, status>;\n  invert-phantom-bool: func(input: phantom-bool, control: s64) -> result<bool, status>;\n}\n\nworld semaprax-private-v8 {\n  export record-pattern-projections;\n}\n";
    assert_eq!(checked_in_v8, expected_v8);

    let checked_in_source_v8 = read("platform-tests/component-runtime/v8.spx");
    let expected_source_v8 = r#"module test.component_record_pattern_v8;

@id("component.pattern.phantom")
record Phantom<T> {
    @id("component.pattern.phantom.marker") marker: bool,
}

@id("component.pattern.preserve-phantom-i64")
fn preserve_phantom_i64(input: Phantom<i64>, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    match input { Phantom { marker } => marker, }
}

@id("component.pattern.invert-phantom-i64")
fn invert_phantom_i64(input: Phantom<i64>, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    match input { Phantom { marker } => !marker, }
}

@id("component.pattern.preserve-phantom-bool")
fn preserve_phantom_bool(input: Phantom<bool>, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    match input { Phantom { marker } => marker, }
}

@id("component.pattern.invert-phantom-bool")
fn invert_phantom_bool(input: Phantom<bool>, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    match input { Phantom { marker } => !marker, }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    assert_eq!(checked_in_source_v8, expected_source_v8);

    let checked_in_v9 = read("platform-tests/component-runtime/wit/semaprax-private-v9.wit");
    let expected_v9 = "package semaprax:private@0.7.0;\n\ninterface generic-function-instances {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  preserve-i64: func(marker: bool, control: s64) -> result<bool, status>;\n  invert-i64: func(marker: bool, control: s64) -> result<bool, status>;\n  preserve-bool: func(marker: bool, control: s64) -> result<bool, status>;\n  invert-bool: func(marker: bool, control: s64) -> result<bool, status>;\n  ordered-i64-bool: func(marker: bool, control: s64) -> result<bool, status>;\n  ordered-bool-i64: func(marker: bool, control: s64) -> result<bool, status>;\n}\n\nworld semaprax-private-v9 {\n  export generic-function-instances;\n}\n";
    assert_eq!(checked_in_v9, expected_v9);

    let checked_in_source_v9 = read("platform-tests/component-runtime/v9.spx");
    let expected_source_v9 = r#"module test.component_generic_function_v9;

@id("component.generic-function.preserve")
fn preserve<T>(marker: bool, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    marker
}

@id("component.generic-function.invert")
fn invert<T>(marker: bool, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    !marker
}

@id("component.generic-function.ordered")
fn ordered<T, U>(marker: bool, control: i64) -> bool
    requires control != -99
    ensures control != 13
{
    marker
}

@id("component.generic-function.materialize")
fn materialize() -> bool {
    let preserve_i64 = preserve<i64>(true, 0);
    let invert_i64 = invert<i64>(false, 0);
    let preserve_bool = preserve<bool>(true, 0);
    let invert_bool = invert<bool>(false, 0);
    let ordered_i64_bool = ordered<i64, bool>(true, 0);
    let ordered_bool_i64 = ordered<bool, i64>(true, 0);
    preserve_i64 && invert_i64 && preserve_bool && invert_bool && ordered_i64_bool && ordered_bool_i64
}

@id("app.main")
fn main() -> i64 { if materialize() { 0 } else { 1 } }
"#;
    assert_eq!(checked_in_source_v9, expected_source_v9);
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
        "--features unstable-wit-component-harness --lib wasm::nested_record_component_v6::tests::",
        "--features unstable-wit-component-harness --lib wit_component::nested_record_v6::tests::",
        "--features unstable-wit-component-harness --lib wasm::generic_record_component_v7::tests::",
        "--features unstable-wit-component-harness --lib wit_component::generic_record_v7::tests::",
        "--features unstable-wit-component-harness --lib wasm::record_pattern_component_v8::tests::",
        "--features unstable-wit-component-harness --lib wit_component::record_pattern_v8::tests::",
        "--features unstable-wit-component-harness --lib wasm::generic_function_component_v9::tests::",
        "--features unstable-wit-component-harness --lib wit_component::generic_function_v9::tests::",
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
        "execute every v3/v4/v5/v6/v7/v8/v9 Component gate offline",
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
        "SemapraxPrivateV6::instantiate",
        "SemapraxPrivateV7::instantiate",
        "SemapraxPrivateV8::instantiate",
        "SemapraxPrivateV9::instantiate",
        "#[allow(clippy::too_many_lines)]\nfn run_v5_instance",
        "#[allow(clippy::too_many_lines)]\nfn run_v6_instance",
        "#[allow(clippy::too_many_lines)]\nfn run_v7_instance",
        "#[allow(clippy::too_many_lines)]\nfn run_v8",
        "#[allow(clippy::too_many_lines)]\nfn run_v9",
        "Keep the exact source-instance-to-WIT mapping reviewable in one function.",
        "Keep the exact nested-record field and status matrix visible in one reviewable",
        "This is deliberately a flat, reviewable six-export protocol matrix.",
        "semaprax-private-v4.wit",
        "semaprax-private-v5.wit",
        "semaprax-private-v6.wit",
        "semaprax-private-v7.wit",
        "semaprax-private-v8.wit",
        "semaprax-private-v9.wit",
        "EXPECTED_COMPONENT_V4_SHA256",
        "EXPECTED_GENERATED_CORE_V4_SHA256",
        "EXPECTED_SOURCE_REVISION_V4",
        "EXPECTED_COMPONENT_V5_SHA256",
        "EXPECTED_GENERATED_CORE_V5_SHA256",
        "EXPECTED_SOURCE_REVISION_V5",
        "EXPECTED_COMPONENT_V6_SHA256",
        "EXPECTED_GENERATED_CORE_V6_SHA256",
        "EXPECTED_SOURCE_REVISION_V6",
        "EXPECTED_COMPONENT_V7_SHA256",
        "EXPECTED_GENERATED_CORE_V7_SHA256",
        "EXPECTED_SOURCE_REVISION_V7",
        "EXPECTED_COMPONENT_V8_SHA256",
        "EXPECTED_GENERATED_CORE_V8_SHA256",
        "EXPECTED_SOURCE_REVISION_V8",
        "EXPECTED_COMPONENT_V9_SHA256",
        "EXPECTED_GENERATED_CORE_V9_SHA256",
        "EXPECTED_SOURCE_REVISION_V9",
        "sha256:86411224efe3adace5ffdd410c243306859edc280dbe3342adcf830588b62259",
        "0x6c, 0xeb, 0x9e, 0x30, 0x96, 0x94, 0xa5, 0xb9",
        "0x08, 0x25, 0xf2, 0x70, 0xcf, 0x2c, 0x94, 0xbd",
        "sha256:4391bc27b5db547f2b162c2b5467c2b75797e8a5ef64e4ffe4abef15678c6254",
        "validate_private_source_result_component_v4",
        "validate_private_scalar_algebra_component_v5",
        "validate_private_nested_record_component_v6",
        "validate_private_generic_record_component_v7",
        "validate_private_record_pattern_component_v8",
        "validate_private_generic_function_component_v9",
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
        "semaprax:private/nested-records@0.4.0",
        "semaprax:private/generic-records@0.5.0",
        "semaprax:private/record-pattern-projections@0.6.0",
        "semaprax:private/generic-function-instances@0.7.0",
        "cabi_transform_nested_record_v6",
        "call_transform",
        "call_transform_i64_bool",
        "call_transform_bool_i64",
        "call_preserve_phantom_i64",
        "call_invert_phantom_bool",
        "prove_raw_core_v6_poison_status_and_invalid_bool",
        "v6 raw result pointer was negative",
        "v6 raw status pointer changed",
        "status[24..32] != [0xa5; 8]",
        "poison != [0xa5; 32]",
        "cabi_transform_i64_bool_v7",
        "cabi_transform_bool_i64_v7",
        "cabi_preserve_phantom_i64_v7",
        "cabi_invert_phantom_bool_v7",
        "prove_raw_core_v7_mapping_poison_and_invalid_bools",
        "prove_same_signature_phantom_swap_is_observable_v7",
        "v7 exact validator admitted Phantom core-index swap",
        "v7 same-signature Phantom swap was not observably crossed",
        "v7 raw invalid bool retained stale output",
        "bytes != [0xa5; 24]",
        "cabi_preserve_pattern_phantom_i64_v8",
        "cabi_invert_pattern_phantom_i64_v8",
        "cabi_preserve_pattern_phantom_bool_v8",
        "cabi_invert_pattern_phantom_bool_v8",
        "prove_raw_core_v8_mapping_poison_and_invalid_bools",
        "prove_all_pair_swaps_reject_and_polarity_swaps_are_observable_v8",
        "v8 exact validator admitted pair swap",
        "v8 polarity-changing pair swap was not observable",
        "v8 raw invalid bool retained stale output",
        "cabi_preserve_i64_v9",
        "cabi_invert_i64_v9",
        "cabi_preserve_bool_v9",
        "cabi_invert_bool_v9",
        "cabi_ordered_i64_bool_v9",
        "cabi_ordered_bool_i64_v9",
        "prove_raw_core_v9_mapping_poison_and_invalid_bools",
        "prove_all_pair_swaps_reject_and_polarity_swaps_are_observable_v9",
        "v9 exact validator admitted pair swap",
        "v9 polarity-changing pair swap was not observable",
        "v9 pair-swap 15/8/7 partition changed",
        "v9 raw invalid bool retained stale output",
        "bytes != [0xa5; 20]",
        "sha256:d1fcbc45b3d86fa1d7910378578828df3c557dba92f90ed9459f928c5bf2fe8a",
        "0xad, 0x40, 0x8a, 0x7a, 0x6a, 0x35, 0x96, 0xa0",
        "0x42, 0x83, 0x5d, 0xcb, 0xf9, 0x80, 0x78, 0xac",
        "sha256:2c2c38ae4a6400730bc6c91de659675074020651b9b58bb6a39d047630ef7303",
        "0x78, 0x0a, 0x0c, 0xcf, 0xc3, 0x5c, 0x7f, 0xf6",
        "0xd2, 0x18, 0xff, 0x1e, 0xaf, 0xf5, 0xf3, 0xf6",
        "sha256:2baac0c0920dbb153789767bf506a4a81713081586a81444d8e5f5a8f5a8516d",
        "0xd8, 0x85, 0x90, 0x75, 0x2e, 0xd7, 0xb0, 0x8b",
        "0xb6, 0xe1, 0xdb, 0xf9, 0x52, 0x2d, 0xbb, 0x98",
        "sha256:218085fb5ea1bcc090c04ac0acb3395912d0dad09027b9118d8817978b2fde0c",
        "0x3c, 0xf6, 0xc7, 0xd7, 0xd0, 0x2e, 0x83, 0x8f",
        "0x9f, 0x17, 0x82, 0x07, 0xa0, 0x40, 0x6f, 0x74",
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
        "artifact.digest()",
        "pointer as usize",
    ] {
        assert!(
            !runner.contains(forbidden),
            "runner contains ambient or untyped surface: {forbidden}"
        );
    }
    assert_eq!(
        runner
            .matches("get_typed_func::<(i64, i32, i64, i64), i32>")
            .count(),
        2,
        "only the exact v6/v7 i64-first raw signatures may bypass typed bindings"
    );
    assert_eq!(
        runner.matches("get_typed_func::<(i32, i64), i32>").count(),
        5,
        "only the authenticated v8/v9 raw signatures may bypass typed bindings"
    );
    assert_eq!(
        runner.matches("Module::new").count(),
        4,
        "only authenticated v6/v7/v8/v9 embedded cores may be instantiated directly"
    );
    assert_eq!(
        runner.matches("usize::try_from(").count(),
        14,
        "every v6/v7/v8/v9 raw result pointer must be converted without signed truncation"
    );
}
