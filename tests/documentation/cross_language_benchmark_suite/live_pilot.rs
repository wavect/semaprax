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

#[test]
fn linux_pilot_requires_explicit_provision_and_restricts_authority() {
    let output = std::process::Command::new("python3")
        .current_dir(super::root())
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env_remove("SEMAPRAX_LINUX_LAUNCHER")
        .env_remove("SEMAPRAX_LINUX_PROVISION")
        .env_remove("SEMAPRAX_LINUX_PROVISION_SHA256")
        .env_remove("SEMAPRAX_LINUX_FIXTURE_EVIDENCE")
        .args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "benchmarks/cross-language-v1/agent/tests",
            "-p",
            "test_pilot_linux_host.py",
            "-v",
        ])
        .output()
        .expect("python3 must run the Linux pilot admission gate");
    assert!(
        output.status.success(),
        "Linux pilot admission gate failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
