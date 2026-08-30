#![cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]

#[path = "linked_publication/support.rs"]
mod support;

use semaprax_offline_wasm_package::{CleanupStatus, PublicationVisibility};
use std::fs;
use support::*;

#[test]
fn real_linked_publication_reopens_and_independently_replays_exact_artifacts() {
    let fixture = Fixture::new(41);
    let root = private_root("success");
    let output = root.join("package");
    let published = fixture
        .publish(&output, fixture.build.clone())
        .expect("public linked publication");
    assert_eq!(published.output, output);
    let reopened = reopen(&output);
    assert_eq!(reopened, fixture.build);
    let replayed = fixture
        .verify(&reopened)
        .expect("independent replay from reopened bytes");
    assert_eq!(replayed, published.verified);
    assert_eq!(
        replayed.packages,
        vec![coordinate(ROOT), coordinate(PROVIDER)]
    );
    assert_inventory(&root, &["package"]);
    cleanup_files(&output, &FILES);
    fs::remove_dir(root).expect("remove empty successful fixture");
}

#[test]
fn hostile_linked_inputs_fail_before_filesystem_authority() {
    let first = Fixture::new(41);
    let second = Fixture::new(42);
    let root = private_root("pre-replay");
    let output = root.join("absent-parent").join("package");
    // A nonexistent parent would produce PP501 if authority were attempted first.
    let mut bad_module = first.build.clone();
    bad_module.module_wasm.push(0);
    let mut bad_evidence = first.build.clone();
    bad_evidence.evidence_json.push('\n');
    let mut crossed_evidence = first.build.clone();
    crossed_evidence.evidence_json = second.build.evidence_json.clone();
    for (candidate, code) in [
        (second.build.clone(), "SPX-PB607"),
        (bad_module, "SPX-PB607"),
        (bad_evidence, "SPX-PB606"),
        (crossed_evidence, "SPX-PB607"),
    ] {
        let error = first.publish(&output, candidate).unwrap_err();
        assert_eq!(error.code, "SPX-PP502");
        assert_eq!(error.compiler_code, Some(code));
        assert_eq!(error.visibility, PublicationVisibility::NotPublished);
        assert_eq!(error.cleanup, CleanupStatus::NotNeeded);
        assert_inventory(&root, &[]);
    }
    let mut stale = Fixture::new(41);
    // Canonical, same-interface provider bytes with a different implementation.
    stale.sources[1].source = second.sources[1].source.clone();
    let error = stale.publish(&output, stale.build.clone()).unwrap_err();
    assert_eq!(error.code, "SPX-PP502");
    assert_eq!(error.compiler_code, Some("SPX-PB607"));
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert_eq!(error.cleanup, CleanupStatus::NotNeeded);
    assert_inventory(&root, &[]);
    fs::remove_dir(root).expect("remove empty successful fixture");
}

#[test]
fn valid_linked_build_never_replaces_an_existing_destination() {
    let fixture = Fixture::new(41);
    let root = private_root("collision");
    let output = root.join("package");
    fs::create_dir(&output).expect("foreign destination");
    fs::write(output.join("sentinel"), b"foreign bytes").expect("foreign sentinel");
    let error = fixture.publish(&output, fixture.build.clone()).unwrap_err();
    assert_eq!(error.code, "SPX-PP503");
    assert_eq!(error.visibility, PublicationVisibility::NotPublished);
    assert_eq!(error.cleanup, CleanupStatus::NotNeeded);
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"foreign bytes");
    assert_inventory(&root, &["package"]);
    cleanup_files(&output, &["sentinel"]);
    fs::remove_dir(root).expect("remove empty successful fixture");
}

/// Explicit offline gate; never downloads Node or substitutes a different host.
#[test]
#[ignore = "requires explicitly provisioned SEMAPRAX_OFFLINE_PACKAGE_NODE; no installation or network acquisition"]
fn provisioned_node_executes_reopened_linked_scalar_package() {
    let node = std::path::PathBuf::from(
        std::env::var_os("SEMAPRAX_OFFLINE_PACKAGE_NODE")
            .expect("set SEMAPRAX_OFFLINE_PACKAGE_NODE to a provisioned absolute Node executable"),
    );
    assert!(
        node.is_absolute() && node.is_file(),
        "provisioned Node executable must exist at an absolute path"
    );
    let fixture = Fixture::new(41);
    let root = private_root("node");
    let output = root.join("package");
    fixture
        .publish(&output, fixture.build.clone())
        .expect("publish real runtime fixture");
    fixture
        .verify(&reopen(&output))
        .expect("replay published bytes before execution");
    let harness = root.join("consumer.mjs");
    fs::write(&harness, include_str!("linked_publication/consumer.mjs"))
        .expect("write local harness");
    let mut command = std::process::Command::new(node);
    command
        .env_clear()
        .current_dir(&root)
        .arg(&harness)
        .arg(&output);
    // Windows process startup may need this platform directory; no tool, package,
    // loader-option, home, proxy, registry, or network configuration is inherited.
    #[cfg(windows)]
    if let Some(system_root) = std::env::var_os("SystemRoot") {
        command.env("SystemRoot", system_root);
    }
    let result = command
        .output()
        .expect("execute explicitly provisioned Node");
    assert!(
        result.status.success(),
        "Node gate failed: {}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, b"linked-package-consumer-ok\n");
    assert!(result.stderr.is_empty());
    cleanup_files(&output, &FILES);
    cleanup_files(&root, &["consumer.mjs"]);
}
