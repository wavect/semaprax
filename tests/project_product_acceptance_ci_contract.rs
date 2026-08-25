use std::fs;
use std::path::Path;

fn workflow() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml"))
        .expect("read the pinned CI workflow")
}

fn job<'a>(workflow: &'a str, name: &str) -> &'a str {
    let marker = format!("  {name}:\n");
    let (_, after_job) = workflow
        .split_once(&marker)
        .unwrap_or_else(|| panic!("missing {name} job"));
    let end = after_job
        .match_indices('\n')
        .map(|(index, _)| index + 1)
        .find(|start| {
            let line = &after_job[*start..];
            line.starts_with("  ")
                && line.as_bytes().get(2).is_some_and(|byte| *byte != b' ')
                && line
                    .split_once('\n')
                    .map_or(line, |(line, _)| line)
                    .ends_with(':')
        })
        .unwrap_or(after_job.len());
    &after_job[..end]
}

#[test]
fn focused_product_acceptance_is_locked_offline_and_tool_authenticated() {
    let workflow = workflow();
    let acceptance = job(&workflow, "project-product-acceptance-v1");
    for required in [
        "toolchain: \"1.88\"",
        "node-version: 22",
        "npm ci --ignore-scripts",
        "Version 5.8.3",
        "TSC=",
        "RUSTC=",
        "CLANG=",
        "SEMAPRAX_ARCHIVER=",
        "VCToolsInstallDir",
        "SEMAPRAX_LINKER=",
        "cargo fetch --locked --manifest-path examples/calculator-rust/Cargo.toml",
        "cargo test --locked --offline -p semaprax --test project_product_acceptance_ci_contract",
        "SEMAPRAX_REQUIRE_PROJECT_TYPESCRIPT: \"1\"",
        "SEMAPRAX_REQUIRE_PROJECT_NATIVE_RUST_SDK: \"1\"",
        "cargo test --locked --offline -p semaprax --test project_product_acceptance_v1 -- --test-threads=1 --nocapture",
    ] {
        assert!(
            acceptance.contains(required),
            "focused product acceptance lost `{required}`"
        );
    }
    assert!(
        !acceptance.contains("continue-on-error: true"),
        "a literal non-blocking step must not masquerade as product promotion"
    );
}

#[test]
fn unix_is_blocking_while_windows_remains_explicitly_diagnostic() {
    let workflow = workflow();
    let acceptance = job(&workflow, "project-product-acceptance-v1");
    for required in [
        "name: Project Product Acceptance v1 (${{ matrix.os }}, ${{ matrix.evidence }})",
        "continue-on-error: ${{ matrix.diagnostic }}",
        "- os: ubuntu-24.04\n            evidence: blocking\n            diagnostic: false",
        "- os: macos-15\n            evidence: blocking\n            diagnostic: false",
        "- os: windows-2025\n            evidence: diagnostic-only\n            diagnostic: true",
        "Resolve exact held native tools and MSVC environment (Windows diagnostic only)",
    ] {
        assert!(
            acceptance.contains(required),
            "product acceptance evidence boundary lost `{required}`"
        );
    }
    assert_eq!(
        acceptance.matches("continue-on-error:").count(),
        1,
        "only the matrix-level diagnostic switch may be non-blocking"
    );
}

#[test]
fn dedicated_chromium_acceptance_remains_a_separate_blocking_job() {
    let workflow = workflow();
    let browser = job(&workflow, "wasm-scalar-exports-browser-v1");
    for required in [
        "name: Public Wasm Scalar Exports v1 Chromium",
        "npm test -- --workers=1 --retries=0",
    ] {
        assert!(
            browser.contains(required),
            "direct Chromium acceptance lost `{required}`"
        );
    }
    assert!(!browser.contains("continue-on-error"));
}
