use std::fs;
use std::path::Path;

const PINNED_NIGHTLY: &str = "nightly-2026-07-16";

#[test]
fn rust_host_asan_lane_is_pinned_instrumented_and_fail_closed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml")).unwrap();
    let script_path = root.join("scripts/verify-rust-host-asan.sh");
    let script = fs::read_to_string(&script_path).unwrap();
    let corpus = fs::read_to_string(
        root.join("crates/semaprax-native-host/tests/runtime_callable_corpus.rs"),
    )
    .unwrap();

    for required in [
        "rust-host-address-sanitizer:",
        "Rust host ASan (nightly-2026-07-16)",
        "runs-on: ubuntu-24.04",
        "components: rust-src",
        "CC: clang-18",
        "SEMAPRAX_REQUIRE_RUST_HOST_ASAN: \"1\"",
        "ASAN_OPTIONS: detect_leaks=1:halt_on_error=1:abort_on_error=1",
        "RUSTFLAGS: -Zsanitizer=address -Zexternal-clangrt -Clinker=clang-18 -Clink-arg=-fsanitize=address -Cforce-frame-pointers=yes",
        "run: scripts/verify-rust-host-asan.sh",
    ] {
        assert!(
            workflow.contains(required),
            "Rust-host ASan workflow lost `{required}`"
        );
    }
    assert!(workflow.matches(PINNED_NIGHTLY).count() >= 2);
    assert!(workflow.contains("toolchain: stable"));
    assert!(workflow.contains("toolchain: \"1.85\""));
    assert!(workflow.contains("callable-host-sanitizers:"));
    assert!(workflow.contains("-fsanitize=address,undefined"));

    for required in [
        "set -euo pipefail",
        "ASAN_OPTIONS must contain halt_on_error=1",
        "ASAN_OPTIONS must contain abort_on_error=1",
        "ASAN_OPTIONS must contain detect_leaks=1",
        "expected_toolchain=\"nightly-2026-07-16\"",
        "expected_rustc_commit=\"d0babd8b6b05ef9bb65d42f928cef4129d64cf65\"",
        "SEMAPRAX_REQUIRE_RUST_HOST_ASAN",
        "rustc_verbose=\"$(rustup run \"$expected_toolchain\" rustc -vV)\"",
        "cargo_verbose=\"$(rustup run \"$expected_toolchain\" cargo -vV)\"",
        "<<<\"$rustc_verbose\"",
        "<<<\"$cargo_verbose\"",
        "clang-18 --version",
        "#![feature(cfg_sanitize)]",
        "#[cfg(not(sanitize = \"address\"))]",
        "-Zsanitizer=address",
        "-Zexternal-clangrt",
        "-Clinker=clang-18",
        "-Clink-arg=-fsanitize=address",
        "heap-use-after-free",
        "-Zbuild-std",
        "--target \"$target\"",
        "--crate-name semaprax_native_host",
        "-perm -111 -print -quit",
        "trap cleanup EXIT",
        "nm \"$probe_binary\"",
        "nm \"$host_test_binary\"",
        "defined or unresolved ASan symbols",
        "--test runtime_callable_host",
        "--test runtime_callable_corpus",
        "authoritative_corpus_executes_through_generated_callable_host_at_o0_and_o2",
    ] {
        assert!(
            script.contains(required),
            "Rust-host ASan verifier lost `{required}`"
        );
    }
    assert!(!script.contains("|| true"));
    assert!(!script.contains("| head"));
    assert!(!script.contains("rustc -vV |"));
    assert!(!script.contains("cargo -vV |"));
    assert!(!script.contains("\nrustc "));
    assert!(!script.contains("\ncargo test"));
    assert!(!script.contains("nm -u \"$probe_binary\""));
    assert!(!script.contains("nm -u \"$host_test_binary\""));

    assert!(corpus.contains("SEMAPRAX_REQUIRE_RUST_HOST_ASAN"));
    assert!(corpus.contains("RequiredSanitizers::Address"));
    assert!(corpus.contains("required_sanitizer_compiler(required_sanitizers)"));
    assert!(corpus.contains("Rust-host ASan evidence requires CC=clang-18"));
    assert!(corpus.contains("\"-fsanitize=address\""));
    assert!(corpus.contains("\"-fsanitize=address,undefined\""));
    assert!(corpus.contains("output.contains(\"__asan_\")"));
    assert!(corpus.contains("output.contains(\"__ubsan_\")"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = fs::metadata(script_path).unwrap().permissions().mode();
        assert_ne!(mode & 0o111, 0, "Rust-host ASan verifier is not executable");
    }
}
