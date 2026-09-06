use std::path::{Path, PathBuf};

use semaprax::project::{self, ProjectExecutionOptions, ProjectExecutionOutcome, ProjectManifest};

const MANIFEST: &str = "schema = \"semaprax.project.v8\"\nname = \"std-mem\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"std.mem.examples\"\nsources = [\"src/examples.spx\", \"src/mem.spx\", \"src/tests.spx\"]\nweb_exports = []\ntests = [\"std.mem.tests\"]\n";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn temporary(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("semaprax-std-mem-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

#[test]
fn std_mem_manifest_and_three_by_eight_conformance_are_exact() {
    let parsed = ProjectManifest::parse(MANIFEST).unwrap();
    assert!(parsed.web_exports().is_empty());
    assert_eq!(parsed.to_canonical_toml(), MANIFEST);

    let package = root().join("std/mem");
    assert_eq!(
        std::fs::read_to_string(package.join("semaprax.toml")).unwrap(),
        MANIFEST
    );
    let library = std::fs::read_to_string(package.join("src/mem.spx")).unwrap();
    for identity in [
        "std.mem.box.new",
        "std.mem.box.get",
        "std.mem.box.into-inner",
    ] {
        assert!(library.contains(&format!("@id(\"{identity}\")")));
    }
    let conformance = std::fs::read_to_string(package.join("src/tests.spx")).unwrap();
    for scalar in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        for operation in ["new", "get", "into_inner"] {
            assert!(
                conformance.contains(&format!("{operation}<{scalar}>")),
                "std.mem conformance does not instantiate {operation}<{scalar}>"
            );
        }
    }
    let packages: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("std/packages.json")).unwrap())
            .unwrap();
    let entry = packages["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["module"] == "std.mem")
        .expect("std.mem package metadata");
    assert_eq!(entry["directory"], "mem");
    assert_eq!(entry["tier"], "alloc");
    assert_eq!(entry["status"], "partial");
    assert_eq!(
        entry["targets"],
        serde_json::json!(["interpreter", "native-c11", "core-wasm"])
    );
}

#[test]
fn std_mem_project_checks_and_runs_without_a_public_descriptor() {
    let manifest = root().join("std/mem/semaprax.toml");
    project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let options = ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &ProjectExecutionOutcome::Returned(0)
        );
        let errors = snapshot
            .public_api_descriptor()
            .expect_err("std.mem must not have a public descriptor");
        assert!(errors
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-J105"));
        Ok(())
    })
    .unwrap();
}

#[test]
fn std_mem_manifest_and_sources_reject_hostile_lookalikes() {
    for hostile in [
        MANIFEST.replace("std-mem", "std-memory"),
        MANIFEST.replace("owned-data-api.v1", "useful-data.v1"),
        MANIFEST.replace("web_exports = []", "web_exports = [\"std.mem.box.get\"]"),
        MANIFEST.replace("std.mem.examples", "user.mem.examples"),
    ] {
        let errors = ProjectManifest::parse(&hostile).unwrap_err();
        assert!(errors
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-J100"));
    }

    let source_root = root().join("std/mem");
    for (source, needle, replacement) in [
        ("mem.spx", "std.mem.box.get", "std.mem.box.lookalike"),
        (
            "examples.spx",
            "std.mem.examples.main",
            "std.mem.examples.lookalike",
        ),
        ("tests.spx", "std.mem.tests.main", "std.mem.tests.lookalike"),
    ] {
        let scratch = temporary(source);
        std::fs::create_dir(scratch.join("src")).unwrap();
        std::fs::copy(
            source_root.join("semaprax.toml"),
            scratch.join("semaprax.toml"),
        )
        .unwrap();
        for candidate in ["examples.spx", "mem.spx", "tests.spx"] {
            let mut contents =
                std::fs::read_to_string(source_root.join("src").join(candidate)).unwrap();
            if candidate == source {
                contents = contents.replacen(needle, replacement, 1);
            }
            std::fs::write(scratch.join("src").join(candidate), contents).unwrap();
        }
        let errors =
            project::with_authenticated_project(&scratch.join("semaprax.toml"), |_| Ok(()))
                .unwrap_err();
        assert!(
            errors.iter().any(|diagnostic| {
                diagnostic.code == "SPX-J100"
                    && diagnostic
                        .message
                        .contains("authenticated wrapper, example, and conformance")
            }),
            "{source} hostile selected unexpected diagnostics: {errors:?}"
        );
        std::fs::remove_dir_all(scratch).unwrap();
    }
}
