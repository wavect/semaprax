use super::*;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
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

fn test_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn test_decode_lower_hex(value: &str) -> Vec<u8> {
    fn digit(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            _ => panic!("test carrier is not lowercase hexadecimal"),
        }
    }
    assert_eq!(value.len() & 1, 0);
    (0..value.len())
        .step_by(2)
        .map(|index| {
            let bytes = value.as_bytes();
            (digit(bytes[index]) << 4) | digit(bytes[index + 1])
        })
        .collect()
}

fn test_carrier_payload(value: &serde_json::Value) -> String {
    use crate::diagnostic::quote_json;
    let string = |key: &str| value[key].as_str().unwrap();
    let artifacts = value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|artifact| {
            format!(
                "{{\"path\":{},\"bytes\":{},\"sha256\":{},\"content_hex\":{}}}",
                quote_json(artifact["path"].as_str().unwrap()),
                artifact["bytes"].as_u64().unwrap(),
                quote_json(artifact["sha256"].as_str().unwrap()),
                quote_json(artifact["content_hex"].as_str().unwrap()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let nonclaims = value["nonclaims"]
        .as_array()
        .unwrap()
        .iter()
        .map(|claim| quote_json(claim.as_str().unwrap()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":{},\"project_schema\":{},\"project\":{},\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"entry_module\":{},\"encoding\":{},\"limits\":{{\"max_bytes\":{}}},\"artifact_count\":{},\"artifact_bytes\":{},\"artifacts\":[{}],\"nonclaims\":[{}]}}",
        quote_json(string("schema")),
        quote_json(string("project_schema")),
        quote_json(string("project")),
        quote_json(string("project_revision")),
        quote_json(string("workspace_revision")),
        quote_json(string("project_graph_digest")),
        quote_json(string("entry_module")),
        quote_json(string("encoding")),
        value["limits"]["max_bytes"].as_u64().unwrap(),
        value["artifact_count"].as_u64().unwrap(),
        value["artifact_bytes"].as_u64().unwrap(),
        artifacts,
        nonclaims,
    )
}

fn test_resign_carrier(value: &serde_json::Value) -> String {
    let payload = test_carrier_payload(value);
    test_resign_raw_payload(&payload)
}

fn test_resign_raw_payload(payload: &str) -> String {
    use crate::diagnostic::quote_json;
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.project-web-build.payload.v1\0");
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload.as_bytes());
    let digest = format!("sha256:{}", test_lower_hex(&hasher.finalize()));
    let mut envelope = payload.to_owned();
    envelope.pop();
    envelope.push_str(",\"payload_digest\":");
    envelope.push_str(&quote_json(&digest));
    envelope.push('}');
    envelope
}

fn test_unsigned_carrier_payload(envelope: &str) -> String {
    let digest_field = envelope
        .rfind(",\"payload_digest\":")
        .expect("test carrier has payload digest");
    let mut payload = envelope[..digest_field].to_owned();
    payload.push('}');
    payload
}

fn test_set_artifact_content(value: &mut serde_json::Value, index: usize, bytes: &[u8]) {
    let artifact = &mut value["artifacts"][index];
    artifact["bytes"] = serde_json::Value::from(bytes.len() as u64);
    artifact["content_hex"] = serde_json::Value::from(test_lower_hex(bytes));
    artifact["sha256"] =
        serde_json::Value::from(format!("sha256:{}", test_lower_hex(&Sha256::digest(bytes))));
}

fn test_assert_resigned_rejected(value: &serde_json::Value, max_bytes: usize, label: &str) {
    let envelope = test_resign_carrier(value);
    let error = match ProjectWebBuild::verify_envelope(&envelope, max_bytes) {
        Ok(_) => panic!("forged carrier unexpectedly admitted: {label}"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-W117", "wrong diagnostic for {label}");
}

fn test_assert_raw_resigned_rejected(payload: &str, max_bytes: usize, label: &str) {
    let envelope = test_resign_raw_payload(payload);
    let error = match ProjectWebBuild::verify_envelope(&envelope, max_bytes) {
        Ok(_) => panic!("raw forged carrier unexpectedly admitted: {label}"),
        Err(error) => error,
    };
    assert_eq!(error.code, "SPX-W117", "wrong diagnostic for {label}");
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

#[test]
fn inline_web_build_is_exact_pathless_bounded_and_replayable() {
    let root = fixture();
    let before = file_inventory(&root);
    let build = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
    })
    .unwrap();
    assert_eq!(file_inventory(&root), before);
    build.verify().unwrap();
    assert_eq!(
        ProjectWebBuild::verify_envelope(build.envelope(), build.max_bytes()).unwrap(),
        build
    );
    let forged = build.envelope().replacen("app.wasm", "App.wasm", 1);
    assert_eq!(
        ProjectWebBuild::verify_envelope(&forged, build.max_bytes())
            .unwrap_err()
            .code,
        "SPX-W117"
    );
    assert!(build.envelope().len() <= build.max_bytes());
    assert!(build.artifact_bytes() < build.envelope().len());
    assert!(build.payload_digest().starts_with("sha256:"));

    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    assert_eq!(test_resign_carrier(&value), build.envelope());
    assert_eq!(value["schema"], PROJECT_WEB_BUILD_SCHEMA);
    assert_eq!(value["project"], "calculator");
    assert_eq!(value["artifact_count"], 7);
    assert_eq!(
        value["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|artifact| artifact["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "app.wasm",
            "semaprax.js",
            "semaprax.bindings.js",
            "semaprax.bindings.d.ts",
            "semaprax.scalar-exports.json",
            "package.json",
            "index.html",
        ]
    );
    for artifact in value["artifacts"].as_array().unwrap() {
        assert!(artifact["sha256"].as_str().unwrap().starts_with("sha256:"));
        assert!(artifact["content_hex"]
            .as_str()
            .unwrap()
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    }

    let mut fixed_limit = MAX_PROJECT_WEB_BUILD_BYTES;
    let exact = loop {
        let candidate = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
            snapshot.build_web_inline(fixed_limit)
        })
        .unwrap();
        let observed = candidate.envelope().len();
        if observed == fixed_limit {
            break candidate;
        }
        assert!(observed < fixed_limit);
        fixed_limit = observed;
    };
    assert_eq!(exact.envelope().len(), fixed_limit);
    exact.verify().unwrap();
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web_inline(fixed_limit - 1)
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-W117");
    let error = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES + 1)
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-W117");

    let published = root.join("published-web");
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web(&published)
    })
    .unwrap();
    let inline_artifacts = value["artifacts"].as_array().unwrap();
    let published_files = std::fs::read_dir(&published)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(published_files.len(), inline_artifacts.len());
    for artifact in inline_artifacts {
        let path = artifact["path"].as_str().unwrap();
        assert!(published_files.contains(path));
        assert_eq!(
            std::fs::read(published.join(path)).unwrap(),
            test_decode_lower_hex(artifact["content_hex"].as_str().unwrap()),
            "ordinary publication and inline carrier differ for {path}"
        );
    }
    std::fs::remove_dir_all(&published).unwrap();
    assert_eq!(file_inventory(&root), before);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn external_inline_web_verifier_rejects_every_resigned_outer_forgery() {
    let root = fixture();
    let build = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
    })
    .unwrap();
    let original: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    assert_eq!(test_resign_carrier(&original), build.envelope());

    for key in [
        "project",
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
        "entry_module",
    ] {
        let mut forged = original.clone();
        let mut replacement = forged[key].as_str().unwrap().to_owned();
        let last = replacement.pop().unwrap();
        replacement.push(if last == '0' { '1' } else { '0' });
        forged[key] = serde_json::Value::from(replacement);
        test_assert_resigned_rejected(&forged, build.max_bytes(), key);
    }

    for index in 0..7 {
        let mut wrong_length = original.clone();
        let bytes = wrong_length["artifacts"][index]["bytes"].as_u64().unwrap();
        wrong_length["artifacts"][index]["bytes"] = serde_json::Value::from(bytes + 1);
        test_assert_resigned_rejected(
            &wrong_length,
            build.max_bytes(),
            &format!("artifact {index} length"),
        );

        let mut wrong_sha = original.clone();
        let mut sha = wrong_sha["artifacts"][index]["sha256"]
            .as_str()
            .unwrap()
            .to_owned();
        let last = sha.pop().unwrap();
        sha.push(if last == '0' { '1' } else { '0' });
        wrong_sha["artifacts"][index]["sha256"] = serde_json::Value::from(sha);
        test_assert_resigned_rejected(
            &wrong_sha,
            build.max_bytes(),
            &format!("artifact {index} SHA-256"),
        );

        let mut wrong_content = original.clone();
        let mut content = wrong_content["artifacts"][index]["content_hex"]
            .as_str()
            .unwrap()
            .to_owned();
        let first = content.remove(0);
        content.insert(0, if first == '0' { '1' } else { '0' });
        wrong_content["artifacts"][index]["content_hex"] = serde_json::Value::from(content);
        test_assert_resigned_rejected(
            &wrong_content,
            build.max_bytes(),
            &format!("artifact {index} content"),
        );

        let mut self_consistent_content = original.clone();
        let mut decoded = test_decode_lower_hex(
            self_consistent_content["artifacts"][index]["content_hex"]
                .as_str()
                .unwrap(),
        );
        decoded[0] ^= 1;
        test_set_artifact_content(&mut self_consistent_content, index, &decoded);
        test_assert_resigned_rejected(
            &self_consistent_content,
            build.max_bytes(),
            &format!("artifact {index} self-consistent content"),
        );
    }

    for delta in [-1_i64, 1] {
        let mut forged = original.clone();
        let total = forged["artifact_bytes"].as_u64().unwrap();
        forged["artifact_bytes"] = serde_json::Value::from((total as i64 + delta) as u64);
        test_assert_resigned_rejected(
            &forged,
            build.max_bytes(),
            &format!("cumulative artifact bytes {delta}"),
        );
    }
    let mut widened = original.clone();
    widened["limits"]["max_bytes"] = serde_json::Value::from((build.max_bytes() - 1) as u64);
    test_assert_resigned_rejected(&widened, build.max_bytes(), "serialized max_bytes drift");
    assert_eq!(
        ProjectWebBuild::verify_envelope(build.envelope(), build.envelope().len() - 1)
            .unwrap_err()
            .code,
        "SPX-W117"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn external_inline_web_verifier_rejects_resigned_closed_grammar_forgery() {
    let root = fixture();
    let build = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
    })
    .unwrap();
    let original: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();

    for (key, replacement) in [
        ("schema", "semaprax.project-web-build.v0"),
        ("project_schema", "semaprax.project.v0"),
        ("encoding", "hex-upper"),
    ] {
        let mut forged = original.clone();
        forged[key] = serde_json::Value::from(replacement);
        test_assert_resigned_rejected(&forged, build.max_bytes(), key);
    }
    let mut wrong_count = original.clone();
    wrong_count["artifact_count"] = serde_json::Value::from(6_u64);
    test_assert_resigned_rejected(&wrong_count, build.max_bytes(), "artifact_count");

    let mut wrong_nonclaims = original.clone();
    wrong_nonclaims["nonclaims"][0] = serde_json::Value::from("filesystem_authority");
    test_assert_resigned_rejected(&wrong_nonclaims, build.max_bytes(), "nonclaims");

    for index in 0..7 {
        let mut wrong_path = original.clone();
        wrong_path["artifacts"][index]["path"] =
            serde_json::Value::from(format!("foreign-{index}"));
        test_assert_resigned_rejected(
            &wrong_path,
            build.max_bytes(),
            &format!("artifact {index} path"),
        );
    }
    let mut wrong_order = original.clone();
    wrong_order["artifacts"].as_array_mut().unwrap().swap(0, 1);
    test_assert_resigned_rejected(&wrong_order, build.max_bytes(), "artifact order");

    let payload = test_unsigned_carrier_payload(build.envelope());
    let foreign_outer = payload.replacen(
        "\"schema\":\"semaprax.project-web-build.v1\",",
        "\"schema\":\"semaprax.project-web-build.v1\",\"foreign\":null,",
        1,
    );
    assert_ne!(foreign_outer, payload);
    test_assert_raw_resigned_rejected(&foreign_outer, build.max_bytes(), "foreign outer key");
    let missing_outer = payload.replacen("\"encoding\":\"hex-lower\",", "", 1);
    assert_ne!(missing_outer, payload);
    test_assert_raw_resigned_rejected(&missing_outer, build.max_bytes(), "missing outer key");

    let first_path = original["artifacts"][0]["path"].as_str().unwrap();
    let foreign_artifact = payload.replacen(
        &format!("{{\"path\":{}", crate::diagnostic::quote_json(first_path)),
        &format!(
            "{{\"foreign\":null,\"path\":{}",
            crate::diagnostic::quote_json(first_path)
        ),
        1,
    );
    assert_ne!(foreign_artifact, payload);
    test_assert_raw_resigned_rejected(&foreign_artifact, build.max_bytes(), "foreign artifact key");
    let first_bytes = original["artifacts"][0]["bytes"].as_u64().unwrap();
    let missing_artifact = payload.replacen(&format!("\"bytes\":{first_bytes},"), "", 1);
    assert_ne!(missing_artifact, payload);
    test_assert_raw_resigned_rejected(&missing_artifact, build.max_bytes(), "missing artifact key");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn external_inline_web_verifier_rejects_resigned_inner_manifest_inconsistency() {
    let root = fixture();
    let build = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        snapshot.build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
    })
    .unwrap();
    let original: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    let manifest = test_decode_lower_hex(original["artifacts"][4]["content_hex"].as_str().unwrap());
    let manifest = String::from_utf8(manifest).unwrap();
    for key in [
        "project",
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
        "entry_module",
    ] {
        let outer_value = original[key].as_str().unwrap();
        let mut replacement = outer_value.to_owned();
        let last = replacement.pop().unwrap();
        replacement.push(if last == '0' { '1' } else { '0' });
        let forged_manifest = manifest.replacen(
            &format!("\"{key}\":{}", crate::diagnostic::quote_json(outer_value)),
            &format!("\"{key}\":{}", crate::diagnostic::quote_json(&replacement)),
            1,
        );
        assert_ne!(forged_manifest, manifest, "missing manifest field {key}");
        let mut forged = original.clone();
        test_set_artifact_content(&mut forged, 4, forged_manifest.as_bytes());
        test_assert_resigned_rejected(&forged, build.max_bytes(), &format!("inner manifest {key}"));
    }
    let mut foreign_field = original.clone();
    let forged_manifest = manifest.replacen(
        "\"capabilities\":[]",
        "\"capabilities\":[],\"foreign\":null",
        1,
    );
    test_set_artifact_content(&mut foreign_field, 4, forged_manifest.as_bytes());
    test_assert_resigned_rejected(
        &foreign_field,
        build.max_bytes(),
        "inner manifest foreign field",
    );
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
