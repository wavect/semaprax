use super::*;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn manifest() -> String {
    "schema = \"semaprax.project.v1\"\nname = \"calculator\"\nentry = \"calculator.app\"\nsources = [\"a/core.spx\", \"t/tests.spx\", \"z/app.spx\"]\nweb_exports = [\"calculator.add\", \"calculator.divide\"]\ntests = [\"calculator.tests\"]\n".to_owned()
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-v1-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("a")).unwrap();
    std::fs::create_dir_all(root.join("t")).unwrap();
    std::fs::create_dir_all(root.join("z")).unwrap();
    std::fs::write(root.join(MANIFEST_FILE), manifest()).unwrap();
    std::fs::write(
            root.join("a/core.spx"),
            "module calculator.core;\n\n@id(\"calculator.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n\n@id(\"calculator.divide\")\nfn divide(left: i64, right: i64) -> i64\n    requires right != 0\n{\n    left / right\n}\n",
        )
        .unwrap();
    std::fs::write(
            root.join("t/tests.spx"),
            "module calculator.tests;\nuse function @id(\"calculator.add\") from calculator.core as add;\n\n@id(\"calculator.tests.main\")\nfn main() -> i64\n{\n    if add(19, 23) == 42 { 0 } else { 1 }\n}\n",
        )
        .unwrap();
    std::fs::write(
            root.join("z/app.spx"),
            "module calculator.app;\nuse function @id(\"calculator.add\") from calculator.core as add;\n\n@id(\"calculator.app.main\")\nfn main() -> i64\n{\n    add(19, 23)\n}\n",
        )
        .unwrap();
    root.canonicalize().unwrap()
}

fn file_inventory(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, inventory: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_string_lossy();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                inventory.insert(format!("directory:{relative}"), Vec::new());
                visit(root, &path, inventory);
            } else {
                assert!(kind.is_file(), "unexpected inventory object {relative}");
                inventory.insert(format!("file:{relative}"), std::fs::read(path).unwrap());
            }
        }
    }

    let mut inventory = BTreeMap::new();
    visit(root, root, &mut inventory);
    inventory
}

#[test]
fn canonical_manifest_round_trips_and_rejects_confusion() {
    let parsed = ProjectManifest::parse(&manifest()).unwrap();
    assert_eq!(parsed.name(), "calculator");
    assert_eq!(parsed.entry(), "calculator.app");
    assert_eq!(parsed.test_module(), "calculator.tests");
    assert_eq!(parsed.to_canonical_toml(), manifest());

    let crlf = manifest().replace('\n', "\r\n");
    assert_eq!(
        ProjectManifest::parse(&crlf).unwrap_err()[0].code,
        "SPX-J100"
    );

    for malformed in [
        manifest().replace("schema =", "unknown ="),
        manifest().replace(
            "name = \"calculator\"\nentry",
            "entry = \"calculator.app\"\nname",
        ),
        manifest().replace("a/core.spx\", \"t/tests.spx", "t/tests.spx\", \"a/core.spx"),
        manifest().replace(
            "calculator.add\", \"calculator.divide",
            "calculator.add\", \"calculator.add",
        ),
        manifest().replace("entry = \"calculator.app\"", "entry = \"calculator.tests\""),
        manifest().trim_end().to_owned(),
    ] {
        assert_eq!(
            ProjectManifest::parse(&malformed).unwrap_err()[0].code,
            "SPX-J100"
        );
    }
}

#[test]
fn relative_manifest_is_resolved_but_aliased_components_are_rejected() {
    assert!(DeclaredPathSelection::open(Path::new("Cargo.toml"), "test").is_ok());

    let root = fixture();
    #[cfg(windows)]
    let dotted = {
        let mut spelling = root.as_os_str().to_os_string();
        spelling.push(r"\z\..\semaprax.toml");
        PathBuf::from(spelling)
    };
    #[cfg(not(windows))]
    let dotted = root.join("z").join("..").join(MANIFEST_FILE);
    let error = with_authenticated_project(&dotted, |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J100");
    assert!(error[0].message.contains("must not contain `.` or `..`"));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn windows_declared_alias_scan_preserves_raw_dot_components_only() {
    for hostile in [
        r"z\..\semaprax.toml",
        "z/../semaprax.toml",
        r".\semaprax.toml",
        "./semaprax.toml",
        r"C:.\semaprax.toml",
        r"C:..\semaprax.toml",
        r"c:.\semaprax.toml",
        r"c:..\semaprax.toml",
        "C:./semaprax.toml",
        "C:../semaprax.toml",
    ] {
        assert!(
            has_declared_alias_component(Path::new(hostile)),
            "missed raw alias component in {hostile}"
        );
        let error = declared_absolute_path(Path::new(hostile), "test").unwrap_err();
        assert_eq!(error[0].code, "SPX-J100");
        assert!(error[0].message.contains("must not contain `.` or `..`"));
    }
    for ordinary in [
        ".semaprax.toml",
        "name..toml",
        "...",
        r"dir.with.dots\semaprax.toml",
        r"C:.semaprax.toml",
        r"C:name..toml",
        r"C:\dir.with.dots\semaprax.toml",
    ] {
        assert!(
            !has_declared_alias_component(Path::new(ordinary)),
            "rejected ordinary dotted name {ordinary}"
        );
    }
}

#[test]
fn bounded_recheck_rejects_growth_without_unbounded_reading() {
    let root = fixture();
    let path = root.join("bounded-input");
    std::fs::write(&path, b"ok").unwrap();
    let mut held = HeldFile::open(path.clone(), 8).unwrap();
    std::fs::write(&path, b"123456789").unwrap();
    let error = held.recheck().unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scalar_profile_is_admitted_before_the_operation_observes_a_snapshot() {
    let root = fixture();
    let changed = manifest().replace("calculator.divide", "calculator.missing");
    std::fs::write(root.join(MANIFEST_FILE), changed).unwrap();
    let called = std::cell::Cell::new(false);
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| {
        called.set(true);
        Ok(())
    })
    .unwrap_err();
    assert!(!called.get());
    assert!(error[0].code.starts_with("SPX-W"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn authenticated_entry_and_test_execution_are_exact_deterministic_and_read_only() {
    let root = fixture();
    let before = file_inventory(&root);
    let options = ProjectExecutionOptions::new(16 * 1024, 10_000).unwrap();
    let (entry, test) = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        let first = snapshot.execute_entry(&options)?;
        let second = snapshot.execute_entry(&options)?;
        assert_eq!(first, second);
        Ok((first, snapshot.execute_test(&options)?))
    })
    .unwrap();

    assert_eq!(entry.role(), ProjectExecutionRole::Entry);
    assert_eq!(entry.module(), "calculator.app");
    assert_eq!(entry.stable_id(), "calculator.app.main");
    assert_eq!(entry.outcome(), &ProjectExecutionOutcome::Returned(42));
    assert!(entry.command_succeeded());
    assert_eq!(entry.max_steps(), 10_000);
    assert!(entry.steps_used() > 0);

    assert_eq!(test.role(), ProjectExecutionRole::Test);
    assert_eq!(test.module(), "calculator.tests");
    assert_eq!(test.stable_id(), "calculator.tests.main");
    assert_eq!(test.outcome(), &ProjectExecutionOutcome::Returned(0));
    assert!(test.command_succeeded());

    let entry_json: serde_json::Value = serde_json::from_str(entry.envelope()).unwrap();
    assert_eq!(entry_json["schema"], PROJECT_EXECUTION_SCHEMA);
    assert_eq!(entry_json["project_schema"], PROJECT_SCHEMA);
    assert_eq!(entry_json["project"], "calculator");
    assert_eq!(entry_json["role"], "entry");
    assert_eq!(entry_json["module"], "calculator.app");
    assert_eq!(entry_json["stable_id"], "calculator.app.main");
    assert_eq!(entry_json["limits"]["max_bytes"], 16 * 1024);
    assert_eq!(entry_json["limits"]["max_steps"], 10_000);
    assert_eq!(entry_json["fuel"]["steps_used"], entry.steps_used());
    assert_eq!(entry_json["fuel"]["max_steps"], 10_000);
    assert_eq!(entry_json["outcome"]["kind"], "returned");
    assert_eq!(entry_json["outcome"]["value"], "42");
    assert_eq!(
        entry_json["nonclaims"],
        serde_json::json!([
            "in_process_reference_interpreter_only",
            "no_target_execution",
            "no_filesystem_process_or_backend_authority",
            "no_test_discovery",
            "no_cache_or_persistence",
        ])
    );
    assert!(entry_json["payload_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert!(entry_json["project_revision"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert!(entry_json["workspace_revision"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    assert_eq!(file_inventory(&root), before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_execution_distinguishes_language_fuel_depth_and_test_failure() {
    let failure_root = fixture();
    std::fs::write(
        failure_root.join("z/app.spx"),
        "module calculator.app;\nuse function @id(\"calculator.add\") from calculator.core as add;\n\n@id(\"calculator.app.main\")\nfn main() -> i64\n{\n    1 / 0\n}\n",
    )
    .unwrap();
    let failure = with_authenticated_project(&failure_root.join(MANIFEST_FILE), |snapshot| {
        snapshot.execute_entry(&ProjectExecutionOptions::default())
    })
    .unwrap();
    assert!(matches!(
        failure.outcome(),
        ProjectExecutionOutcome::LanguageFailure(status)
            if status.code() == crate::cleanup_plan::StatusCase::DivisionByZero.code()
    ));
    assert!(!failure.command_succeeded());

    let fuel_root = fixture();
    let fuel = with_authenticated_project(&fuel_root.join(MANIFEST_FILE), |snapshot| {
        snapshot.execute_entry(&ProjectExecutionOptions::new(4096, 1).unwrap())
    })
    .unwrap();
    assert_eq!(fuel.outcome(), &ProjectExecutionOutcome::FuelExhausted);
    assert_eq!(fuel.steps_used(), 1);

    let depth_root = fixture();
    std::fs::write(
        depth_root.join("z/app.spx"),
        "module calculator.app;\nuse function @id(\"calculator.add\") from calculator.core as add;\n\n@id(\"calculator.app.main\")\nfn main() -> i64\n{\n    main()\n}\n",
    )
    .unwrap();
    let depth = with_authenticated_project(&depth_root.join(MANIFEST_FILE), |snapshot| {
        snapshot.execute_entry(&ProjectExecutionOptions::default())
    })
    .unwrap();
    assert_eq!(depth.outcome(), &ProjectExecutionOutcome::CallDepthExceeded);

    let failed_test_root = fixture();
    std::fs::write(
        failed_test_root.join("t/tests.spx"),
        "module calculator.tests;\nuse function @id(\"calculator.add\") from calculator.core as add;\n\n@id(\"calculator.tests.main\")\nfn main() -> i64\n{\n    1\n}\n",
    )
    .unwrap();
    let failed_test =
        with_authenticated_project(&failed_test_root.join(MANIFEST_FILE), |snapshot| {
            snapshot.execute_test(&ProjectExecutionOptions::default())
        })
        .unwrap();
    assert_eq!(failed_test.outcome(), &ProjectExecutionOutcome::Returned(1));
    assert!(!failed_test.command_succeeded());

    for root in [failure_root, fuel_root, depth_root, failed_test_root] {
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn project_execution_guard_state_is_diagnostic_not_an_outcome() {
    let root = fixture();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        let entrypoint = snapshot.entry_program.entrypoint.clone();
        let entry = snapshot
            .entry_program
            .functions
            .iter_mut()
            .find(|function| function.id == entrypoint)
            .unwrap();
        entry.body.kind = crate::hir::ResolvedExprKind::Bool(true);
        snapshot.execute_entry(&ProjectExecutionOptions::default())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-F105");
    assert!(error[0]
        .message
        .contains("impossible post-validation state"));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn web_build_rechecks_inputs_before_publication() {
    let root = fixture();
    let output = root.with_extension("web-output");
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        std::fs::write(root.join("z/app.spx"), "changed").unwrap();
        snapshot.build_web(&output)
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    assert!(!output.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn native_destination_checks_reject_existing_outputs_before_emission() {
    let root = fixture();
    let output = root.with_extension("native-output");
    std::fs::write(&output, b"sentinel").unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_native(&output)
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I307");
    assert!(error[0].message.contains("already exists"));
    assert_eq!(std::fs::read(&output).unwrap(), b"sentinel");

    let drift_output = root.with_extension("native-drift-output");
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.check()?;
        std::fs::write(root.join("z/app.spx"), "changed").unwrap();
        snapshot.build_native(&drift_output)
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    assert!(!drift_output.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn native_entry_c_projections_are_deterministic() {
    let first = with_authenticated_project(&fixture().join(MANIFEST_FILE), |snapshot| {
        Ok(crate::codegen::emit_hir_c(snapshot.entry_program()).unwrap())
    })
    .unwrap();
    let second = with_authenticated_project(&fixture().join(MANIFEST_FILE), |snapshot| {
        Ok(crate::codegen::emit_hir_c(snapshot.entry_program()).unwrap())
    })
    .unwrap();
    assert_eq!(first, second);
    assert!(first.contains("int main("));
}

#[test]
fn post_publication_drift_is_uncertain_but_preserves_the_complete_old_package() {
    let baseline_root = fixture();
    let baseline_output = baseline_root.with_extension("baseline-web");
    with_authenticated_project(&baseline_root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web(&baseline_output)
    })
    .unwrap();
    let expected = file_inventory(&baseline_output);

    let root = fixture();
    let output = root.with_extension("uncertain-web");
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web(&output)?;
        std::fs::write(root.join("z/app.spx"), "changed").unwrap();
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-J103");
    assert_eq!(file_inventory(&output), expected);
    assert_eq!(file_inventory(&output).len(), 7);

    let _ = std::fs::remove_dir_all(baseline_output);
    let _ = std::fs::remove_dir_all(baseline_root);
    let _ = std::fs::remove_dir_all(output);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn check_has_exactly_zero_workspace_or_source_side_effects() {
    let root = fixture();
    let before = file_inventory(&root);
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| snapshot.check()).unwrap();
    let after = file_inventory(&root);
    assert_eq!(after, before);
    assert!(!root.join(".semaprax-workspace").exists());
    for forbidden in [
        ".semaprax-workspace",
        "LOCK",
        "ACTIVE",
        "generations",
        "cache",
    ] {
        assert!(after.keys().all(|path| !path.contains(forbidden)));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn snapshot_reuses_workspace_phase_a_and_rechecks_bytes() {
    let root = fixture();
    let manifest_path = root.join(MANIFEST_FILE);
    let revision = with_authenticated_project(&manifest_path, |snapshot| {
        assert_eq!(snapshot.sources().len(), 3);
        assert!(snapshot.workspace_manifest().ends_with('\n'));
        snapshot.check()?;
        assert_eq!(snapshot.entry_program().module, "calculator.app");
        assert_eq!(snapshot.test_program().module, "calculator.tests");
        assert!(snapshot.project_revision().starts_with("sha256:"));
        Ok(snapshot.workspace_revision().to_owned())
    })
    .unwrap();
    assert!(revision.starts_with("sha256:"));

    let error = with_authenticated_project(&manifest_path, |_| {
        std::fs::write(root.join("z/app.spx"), "changed").unwrap();
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| {
        std::fs::write(root.join("z/app.spx"), "changed").unwrap();
        Err::<(), _>(vec![Diagnostic::io("SPX-TEST", "primary")])
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-TEST");
    assert_eq!(error[1].code, "SPX-J102");
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn source_alias_and_duplicate_physical_identity_are_rejected() {
    use std::os::unix::fs::symlink;

    let root = fixture();
    let target = root.join("a/core.spx");
    std::fs::remove_file(root.join("z/app.spx")).unwrap();
    symlink(&target, root.join("z/app.spx")).unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    let _ = std::fs::remove_file(root.join("z/app.spx"));
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    std::fs::rename(root.join("a"), root.join("real-a")).unwrap();
    symlink(root.join("real-a"), root.join("a")).unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    let _ = std::fs::remove_file(root.join("a"));
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    std::fs::rename(root.join(MANIFEST_FILE), root.join("real-manifest")).unwrap();
    symlink(root.join("real-manifest"), root.join(MANIFEST_FILE)).unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    let _ = std::fs::remove_file(root.join(MANIFEST_FILE));
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| {
        std::fs::rename(root.join("z/app.spx"), root.join("z/selected-app.spx")).unwrap();
        symlink(root.join("z/selected-app.spx"), root.join("z/app.spx")).unwrap();
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    let _ = std::fs::remove_file(root.join("z/app.spx"));
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    let alias_root = root.with_extension("symlink-alias");
    symlink(&root, &alias_root).unwrap();
    let error =
        with_authenticated_project(&alias_root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    std::fs::remove_file(alias_root).unwrap();
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    std::fs::remove_file(root.join("a/core.spx")).unwrap();
    std::fs::hard_link(root.join(MANIFEST_FILE), root.join("a/core.spx")).unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    std::fs::remove_file(root.join("z/app.spx")).unwrap();
    std::fs::hard_link(root.join("a/core.spx"), root.join("z/app.spx")).unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    let external = root.with_extension("external-hardlink");
    std::fs::hard_link(root.join("a/core.spx"), &external).unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    std::fs::remove_file(external).unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn windows_manifest_and_source_link_counts_are_one() {
    let root = fixture();
    let source_link = root.with_extension("windows-source-link");
    std::fs::hard_link(root.join("a/core.spx"), &source_link).unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    std::fs::remove_file(source_link).unwrap();
    let _ = std::fs::remove_dir_all(root);

    let root = fixture();
    let manifest_link = root.with_extension("windows-manifest-link");
    std::fs::hard_link(root.join(MANIFEST_FILE), &manifest_link).unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert_eq!(error[0].code, "SPX-J102");
    std::fs::remove_file(manifest_link).unwrap();
    let _ = std::fs::remove_dir_all(root);
}
