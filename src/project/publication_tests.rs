//! Pure host handoff tests; callbacks do not perform physical publication.
use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn fixture(version: u8) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-npm-handoff-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let root = std::fs::canonicalize(root).unwrap();
    std::fs::create_dir(root.join("src")).unwrap();
    let (profile, declaration) = match version {
        8 => ("owned-data-api.v1", "@id(\"api.value\") fn value(input: borrow Slice<u8>) -> Bytes { bytes_copy(input) }"),
        9 => ("flat-owned-record-api.v1", "@id(\"api.record\") record Payload { @id(\"api.bytes\") bytes: Bytes, }\n@id(\"api.value\") fn value(input: borrow Slice<u8>) -> Payload { Payload { bytes: bytes_copy(input), } }"),
        10 => ("owned-utf8-api.v1", "@id(\"api.value\") fn value() -> string { \"hello\" }"),
        _ => unreachable!(),
    };
    let app =
        format!("module handoff.app;\n{declaration}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n");
    for (name, source) in [
        ("app.spx", app.as_str()),
        (
            "tests.spx",
            "module handoff.tests;\n@id(\"tests.main\") fn main() -> i64 { 0 }\n",
        ),
    ] {
        let parsed = crate::parse(source, root.join("src").join(name)).unwrap();
        std::fs::write(
            root.join("src").join(name),
            crate::format::canonical(&parsed),
        )
        .unwrap();
    }
    std::fs::write(root.join("semaprax.toml"), format!("schema = \"semaprax.project.v{version}\"\nname = \"handoff\"\nversion = \"1.0.0\"\nprofile = \"{profile}\"\nentry = \"handoff.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"api.value\"]\ntests = [\"handoff.tests\"]\n")).unwrap();
    // Retain fixtures, including drifted inputs, for failure diagnosis.
    root
}

fn expected(build: &ProjectNpmBuild) -> Vec<(String, Vec<u8>)> {
    build.verify().unwrap();
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["hex"].as_str().unwrap();
            (
                row["path"].as_str().unwrap().to_owned(),
                (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn owned_profiles_handoff_exact_six_artifacts_and_preserve_callback_errors() {
    for version in [8, 9, 10] {
        let root = fixture(version);
        with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let artifacts = expected(&snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?);
            assert_eq!(artifacts.len(), 6);
            let output = root.join("package");
            let mut calls = 0;
            snapshot.build_owned_npm_with(&output, |plan, target| {
                calls += 1;
                assert_eq!(target, output);
                assert_eq!(
                    plan.project_schema(),
                    format!("semaprax.project.v{version}")
                );
                assert_eq!(
                    plan.artifacts()
                        .map(|(name, bytes)| (name.to_owned(), bytes.to_vec()))
                        .collect::<Vec<_>>(),
                    artifacts
                );
                Ok(())
            })?;
            assert_eq!(calls, 1);
            assert_eq!(snapshot.published_subject, Some(NPM_PUBLICATION_SUBJECT));
            assert!(!output.exists()); // Trusted callback success, not physical evidence.
            Ok(())
        })
        .unwrap();
        with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let errors = snapshot
                .build_owned_npm_with(&root.join("package"), |_, _| {
                    Err(vec![Diagnostic::io("SPX-I999", "publisher sentinel")])
                })
                .unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "SPX-I999");
            assert_eq!(errors[0].message, "publisher sentinel");
            assert_eq!(snapshot.published_subject, None);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn drift_before_handoff_skips_host_and_drift_after_success_is_uncertain() {
    for version in [8, 9, 10] {
        for after in [false, true] {
            let root = fixture(version);
            let result = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                if !after {
                    std::fs::write(root.join("src/tests.spx"), "changed").unwrap();
                }
                let mut calls = 0;
                let errors = snapshot
                    .build_owned_npm_with(&root.join("package"), |_, _| {
                        calls += 1;
                        std::fs::write(root.join("src/tests.spx"), "changed").unwrap();
                        Ok(())
                    })
                    .unwrap_err();
                assert_eq!(calls, usize::from(after));
                assert_eq!(
                    snapshot.published_subject,
                    after.then_some(NPM_PUBLICATION_SUBJECT)
                );
                if after {
                    assert_eq!(errors[0].code, "SPX-J103");
                    assert_eq!(errors[0].message, format!("Project v1 inputs drifted after one complete {NPM_PUBLICATION_SUBJECT} was published"));
                    assert!(errors.len() > 1);
                }
                Err::<(), _>(errors)
            });
            assert!(result.is_err());
            assert!(!root.join("package").exists());
        }
    }
}

#[test]
fn scalar_project_is_rejected_before_host_handoff() {
    let root = fixture(8);
    std::fs::write(root.join("semaprax.toml"), "schema = \"semaprax.project.v1\"\nname = \"handoff\"\nentry = \"handoff.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"app.main\"]\ntests = [\"handoff.tests\"]\n").unwrap();
    let source = "module handoff.app;\n@id(\"app.main\") fn main() -> i64 { 0 }\n";
    let program = crate::parse(source, root.join("src/app.spx")).unwrap();
    std::fs::write(root.join("src/app.spx"), crate::format::canonical(&program)).unwrap();
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let errors = snapshot
            .build_owned_npm_with(&root.join("package"), |_, _| {
                panic!("unsupported profile reached publisher")
            })
            .unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SPX-J114");
        assert_eq!(
            errors[0].message,
            "owned npm publication requires the exact Project v8, v9, v10, or v11 profile"
        );
        assert_eq!(snapshot.published_subject, None);
        assert!(!root.join("package").exists());
        Ok(())
    })
    .unwrap();
}

#[test]
fn publisher_error_stays_primary_when_callback_also_changes_source() {
    for version in [8, 9, 10] {
        let root = fixture(version);
        let errors = with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let result = snapshot.build_owned_npm_with(&root.join("package"), |_, _| {
                std::fs::write(root.join("src/tests.spx"), "changed").unwrap();
                Err(vec![Diagnostic::io("SPX-I999", "publisher sentinel")])
            });
            assert_eq!(snapshot.published_subject, None);
            let errors = result.as_ref().unwrap_err();
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "SPX-I999");
            result
        })
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-I999");
        assert_eq!(errors[0].message, "publisher sentinel");
        assert!(errors.len() > 1);
        assert!(errors[1..].iter().any(|error| error.code == "SPX-J102"));
        assert!(!errors.iter().any(|error| error.code == "SPX-J103"));
        assert!(!root.join("package").exists());
    }
}

#[cfg(windows)]
#[test]
fn direct_owned_carriers_reject_before_parent_effects_or_foreign_output_access() {
    for version in [8, 9, 10] {
        let root = fixture(version);
        with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
            build.verify().unwrap();
            let absent_parent = root.join("absent-parent");
            let foreign = root.join("foreign");
            let bytes = b"foreign output must remain unchanged";
            std::fs::write(&foreign, bytes).unwrap();
            let identity = same_file::Handle::from_path(&foreign).unwrap();
            for output in [absent_parent.join("package"), foreign.clone()] {
                let error = build.publish(&output).unwrap_err();
                assert_eq!(error.code, "SPX-W120");
                assert_eq!(error.message, "Project v8-v11 npm publication requires semaprax-full with safe handle-relative Windows authority");
            }
            assert!(!absent_parent.exists());
            assert_eq!(std::fs::read(&foreign).unwrap(), bytes);
            assert_eq!(same_file::Handle::from_path(&foreign).unwrap(), identity);
            assert_eq!(snapshot.published_subject, None);
            Ok(())
        }).unwrap();
    }
}
