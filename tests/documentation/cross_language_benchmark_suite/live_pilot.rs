//! Local fixtures only; no provider or second-host execution claim.

#[test]
fn live_pilot_retains_inventory_and_refuses_unsafe_candidate_transport() {
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
            "test_live_pilot.py",
            "-v",
        ])
        .output()
        .expect("python3 must run the live-pilot fixture gate");
    assert!(
        output.status.success(),
        "live-pilot fixture gate failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
