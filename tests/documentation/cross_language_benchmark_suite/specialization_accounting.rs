//! Frozen-cell accounting is not model execution or an independent review.

#[test]
fn specialization_accounting_preserves_frozen_cells_and_missing_authority() {
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
            "test_specialization_accounting.py",
            "-v",
        ])
        .output()
        .expect("python3 must run the specialization-accounting gate");
    assert!(
        output.status.success(),
        "specialization-accounting gate failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
