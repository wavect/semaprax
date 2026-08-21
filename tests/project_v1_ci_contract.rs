use std::fs;
use std::path::Path;

fn workflow() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/ci.yml"))
        .unwrap()
}

fn project_job(workflow: &str) -> &str {
    let (_, after_job) = workflow
        .split_once("  project-v1:\n")
        .expect("project-v1 job");
    after_job
        .split_once("\n  verify:")
        .map_or(after_job, |(job, _)| job)
}

#[test]
fn project_v1_cross_platform_gate_is_web_only_and_source_locked() {
    let workflow = workflow();
    let job = project_job(&workflow);
    for required in [
        "name: Project Manifest v1 (${{ matrix.os }})",
        "ubuntu-24.04",
        "macos-15",
        "windows-2025",
        "toolchain: \"1.85\"",
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
