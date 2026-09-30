//! Offline supported-set checks, not official toolchain execution evidence.

#[test]
fn supported_comparison_scope_preserves_denominators_and_refusal_boundaries() {
    let output = std::process::Command::new("python3")
        .current_dir(super::root())
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            super::SUITE,
            "-p",
            "test_supported_scope.py",
            "-v",
        ])
        .output()
        .expect("python3 must run the supported comparison-scope gate");
    assert!(
        output.status.success(),
        "supported-scope gate failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}
