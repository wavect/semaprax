use std::fs;
use std::path::Path;

fn workflow() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml"))
        .unwrap()
}

fn attributes() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".gitattributes")).unwrap()
}

#[test]
fn embedded_javascript_preserves_canonical_bytes_on_windows_checkouts() {
    assert!(
        attributes().lines().any(|line| line == "*.js text eol=lf"),
        "embedded JavaScript must remain LF even with core.autocrlf=true"
    );
    let prelude = fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/project/npm/owned_data_input_v8.js"),
    )
    .unwrap();
    assert!(prelude.contains(&b'\n'));
    assert!(
        !prelude.contains(&b'\r'),
        "checkout line endings must not alter the frozen generated runtime"
    );
}

fn project_job(workflow: &str) -> &str {
    let (_, after_job) = workflow
        .split_once("  project-v1:\n")
        .expect("project-v1 job");
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
fn project_v1_cross_platform_gate_is_web_only_and_source_locked() {
    assert!(
        attributes()
            .lines()
            .any(|line| line == "*.toml text eol=lf"),
        "Project v1 canonical TOML must remain LF on every checkout"
    );
    let workflow = workflow();
    let job = project_job(&workflow);
    for required in [
        "name: Project Manifest v1 (${{ matrix.os }})",
        "ubuntu-24.04",
        "macos-15",
        "windows-2025",
        "toolchain: \"1.88\"",
        "cargo test --locked -p semaprax --all-features --test project_cli_v1 -- --test-threads=1",
        "cargo test --locked -p semaprax --all-features --test project_manifest_v1 -- --test-threads=1",
        "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1",
        "dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772",
    ] {
        assert!(job.contains(required), "project-v1 job is missing `{required}`");
    }
    for forbidden in [
        "project_backend_equivalence_v1",
        "clang",
        "CLANG:",
        "semaprax run",
        "semaprax test",
    ] {
        assert!(
            !job.contains(forbidden),
            "project-v1 job must not add held native/test-runner work: `{forbidden}`"
        );
    }
}
