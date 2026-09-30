//! Synthetic transport/orchestration checks, not real model acceptance.

#[test]
fn local_specialization_keeps_execution_and_independent_review_distinct() {
    let output = std::process::Command::new("python3")
        .current_dir(super::root())
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "benchmarks/cross-language-v1/agent/tests",
            "-p",
            "test_*local*.py",
            "-v",
        ])
        .output()
        .expect("python3 must run the local-specialization gate");
    assert!(
        output.status.success(),
        "local-specialization synthetic gate failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
