//! HN-07 provisioned evidence (ignored by default): build the distribution
//! tarball from the built host binary, extract it outside the checkout with an
//! empty HOME and a scrubbed environment, run `setup` against preinstalled
//! RTK and Graft, and run a native-only task and a Graft-backed task.

use crate::support::*;
use std::process::Command;

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_RTK HARNESS_GRAFT HARNESS_NODE HARNESS_PYTHON"]
fn hn07_dist_tarball_installs_sets_up_and_runs_outside_the_checkout() {
    let script = repo_root().join("scripts/harness_dist.sh");
    let out = fixture_dir("hp-hn07r");
    let built = Command::new("sh")
        .arg(&script)
        .args(["build", "--binary"])
        .arg(harness_bin())
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let tarball = String::from_utf8(built.stdout).unwrap().trim().to_string();
    let arg = |v: &str| required_tool(v).to_string_lossy().into_owned();
    let smoke = Command::new("sh")
        .arg(&script)
        .args(["smoke", "--tarball", &tarball])
        .args(["--compiler", &arg("SEMAPRAX_COMPILER")])
        .args(["--rtk", &arg("HARNESS_RTK")])
        .args(["--graft", &arg("HARNESS_GRAFT")])
        .args(["--node", &arg("HARNESS_NODE")])
        .args(["--python", &arg("HARNESS_PYTHON")])
        .current_dir(&out)
        .output()
        .expect("smoke");
    let text = String::from_utf8_lossy(&smoke.stdout).into_owned();
    assert!(
        smoke.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&smoke.stderr)
    );
    assert!(text.contains("SMOKE PASSED"), "{text}");
}

/// Build a tarball from the built host binary into a fresh directory.
fn build_tarball(prefix: &str) -> (std::path::PathBuf, String) {
    let script = repo_root().join("scripts/harness_dist.sh");
    let out = fixture_dir(prefix);
    let built = Command::new("sh")
        .arg(&script)
        .args(["build", "--binary"])
        .arg(harness_bin())
        .arg("--out")
        .arg(&out)
        .output()
        .expect("build");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    (
        script,
        String::from_utf8(built.stdout).unwrap().trim().to_string(),
    )
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER python3 git"]
fn hn18_core_smoke_passes_every_cell_with_a_compiler() {
    let (script, tarball) = build_tarball("hp-hn18d");
    let compiler = required_tool("SEMAPRAX_COMPILER");
    let o = Command::new("sh")
        .arg(&script)
        .args(["smoke-core", "--tarball", &tarball, "--compiler"])
        .arg(&compiler)
        .output()
        .expect("smoke-core");
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(
        o.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    for cell in [
        "fresh-install",
        "default-skills-offline",
        "offline-reuse",
        "relocated-asset-paths",
        "native-task",
    ] {
        assert!(
            text.contains(&format!("CELL {cell} pass")),
            "{cell}: {text}"
        );
    }
    assert!(text.contains("SMOKE PASSED"), "{text}");
}

#[test]
#[ignore = "provisioned: needs python3 (tarball catalog)"]
fn hn18_core_smoke_without_a_compiler_is_incomplete_never_passed() {
    let (script, tarball) = build_tarball("hp-hn18e");
    let o = Command::new("sh")
        .arg(&script)
        .args(["smoke-core", "--tarball", &tarball])
        .output()
        .expect("smoke-core");
    let text = String::from_utf8_lossy(&o.stdout).into_owned();
    assert_eq!(o.status.code(), Some(3), "{text}");
    assert!(text.contains("CELL native-task untested"), "{text}");
    assert!(
        text.contains("SMOKE INCOMPLETE") && !text.contains("SMOKE PASSED"),
        "{text}"
    );
}
