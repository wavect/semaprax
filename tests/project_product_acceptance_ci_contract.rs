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
        "SEMAPRAX_REQUIRE_DARWIN_REAL_ARCHIVE: \"1\"",
        "tests::darwin_real_d_archive_is_exact_and_reproducible_across_tool_versions",
        "VCToolsInstallDir",
        "SEMAPRAX_LINKER=",
        "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER=%SEMAPRAX_LINKER%",
        "echo LINK=",
        "echo _LINK_=",
        "cargo fetch --locked\n",
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
    let root_fetch = acceptance
        .find("cargo fetch --locked\n")
        .expect("root workspace dependency fetch");
    let standalone_fetch = acceptance
        .find("cargo fetch --locked --manifest-path examples/calculator-rust/Cargo.toml")
        .expect("standalone consumer dependency fetch");
    let first_offline_test = acceptance
        .find("cargo test --locked --offline")
        .expect("offline acceptance test");
    assert!(
        root_fetch < standalone_fetch && standalone_fetch < first_offline_test,
        "both dependency closures must be fetched before the first offline test"
    );
    assert_eq!(
        acceptance.matches("cargo fetch --locked\n").count(),
        1,
        "the focused job must fetch the root workspace exactly once"
    );
}

#[test]
fn generated_project_consumers_bind_the_bounded_windows_linker_path_at_both_revisions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let binder = fs::read_to_string(root.join("tests/support/native_rust_cargo.rs"))
        .expect("read the nested Cargo linker binder");
    for required in [
        "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
        "std::env::var_os(\"SEMAPRAX_LINKER\")",
        "std::env::var_os(\"SEMAPRAX_VCTOOLS\")",
        r#"Path::new(r"bin\Hostx64\x64\link.exe")"#,
        "command.env_remove(\"LINK\")",
        "command.env_remove(\"_LINK_\")",
        "does not hold the linker image or its ancestors",
        "close a same-path substitution race",
    ] {
        assert!(binder.contains(required), "nested Cargo lost `{required}`");
    }
    for forbidden in ["command.env(\"PATH\"", "RUSTFLAGS", "-Clinker="] {
        assert!(
            !binder.contains(forbidden),
            "nested Cargo admitted forbidden linker configuration `{forbidden}`"
        );
    }

    let support = fs::read_to_string(root.join("tests/support/project_product.rs"))
        .expect("read Project product support");
    assert_eq!(
        support
            .matches("native_rust_cargo::cargo_command()")
            .count(),
        3,
        "Project SDK setup, lock, and consumer must share the bounded Cargo linker binding"
    );
    for required in [
        "let cargo_target = root.join(\"target\")",
        "assert_windows_cargo_target_budget(&cargo_target)",
        "GENERATED_SDK_BUILD_SCRIPT_OBJECT_SUFFIX",
        "object_units < MAX_PATH_UTF16_UNITS",
        "\"run\",\n            \"--verbose\"",
    ] {
        assert!(
            support.contains(required),
            "Project nested Cargo path evidence lost `{required}`",
        );
    }
    assert_eq!(
        support
            .matches(".env(\"CARGO_TARGET_DIR\", &cargo_target)")
            .count(),
        2,
        "lock and run must share one short, invocation-owned Cargo target",
    );
    let verbose_run = support
        .split("let run = native_rust_cargo::cargo_command()")
        .nth(1)
        .and_then(|tail| tail.split(".output()\n        .unwrap();").next())
        .expect("nested Project Cargo run block");
    assert!(verbose_run.contains("\"--verbose\""));
    assert!(!verbose_run.contains("\"--quiet\""));
    let acceptance = fs::read_to_string(root.join("tests/project_product_acceptance_v1.rs"))
        .expect("read Project product acceptance");
    for required in [
        "run_project_rust_sdk(&fixture, \"baseline\")",
        "run_project_rust_sdk(&fixture, \"renamed\")",
    ] {
        assert!(
            acceptance.contains(required),
            "Project acceptance lost bounded-linker consumer path `{required}`"
        );
    }
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
        "examples/calculator.spx --target web",
        "examples/calculator-project/semaprax.toml --target web",
        "SEMAPRAX_DIRECT_CALCULATOR_ROOT=",
        "SEMAPRAX_PROJECT_CALCULATOR_ROOT=",
        "npm run test:fixtures --",
    ] {
        assert!(
            browser.contains(required),
            "direct Chromium acceptance lost `{required}`"
        );
    }
    assert!(!browser.contains("continue-on-error"));
    assert_eq!(
        browser.matches("--moduleResolution NodeNext").count(),
        2,
        "both direct-source and Project declaration consumers must type-check"
    );
}
