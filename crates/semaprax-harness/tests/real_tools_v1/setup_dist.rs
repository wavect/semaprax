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
