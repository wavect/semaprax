//! Authored cross-target evidence; no synthetic arithmetic or allocator ABI.
use semaprax::interpreter::{
    evaluate_resolved_owned_data, OwnedDataCleanupEvent, OwnedDataEvaluationOutcome,
    OwnedDataValue, DEFAULT_MAX_STEPS,
};
use semaprax::project::{
    with_authenticated_project, PublicApiSubject, MAX_PROJECT_NPM_BUILD_BYTES,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::owned_npm_publication;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = include_str!("../usize_mul_owned_v1/subject.spx");
// A failure is followed by a valid zero case on the same native/JS instance.
const CASES: [(&str, usize, u32); 12] = [
    ("forward", 0, 0),
    ("forward", 1, 0),
    ("forward", 2, 3),
    ("forward", 0, 0),
    ("forward", 3, 0),
    ("reverse", 0, 0),
    ("reverse", 1, 0),
    ("reverse", 2, 3),
    ("reverse", 0, 0),
    ("reverse", 3, 0),
    ("precedence", 1, 1),
    ("precedence", 0, 0),
];
const FILES: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.api.json",
    "package.json",
];

fn native(root: &Path, provider: &str) {
    let calls = CASES
        .iter()
        .map(|(name, length, status)| {
            format!(
        "run_case(spx_owned_data_call_spx_mul_dot_{name}_v1,UINT64_C({length}),UINT32_C({status}));"
    )
        })
        .collect::<String>();
    let source = format!(
        "{}\n{}\n{}\n#define FIXTURE_CASES() do {{ {} }} while (0)\n{}",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_tuple_admission_v1/allocations.c"),
        provider,
        calls,
        include_str!("../usize_mul_owned_v1/native.c")
    );
    let path = root.join("native.c");
    fs::write(&path, source.as_bytes()).unwrap();
    let compiler = std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from);
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "native{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let output = Command::new(&compiler)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&path)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang required for usize ownership evidence");
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(&executable).output().unwrap();
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    assert_eq!(fs::read(path).unwrap(), source.as_bytes());
}

#[test]
fn usize_multiplication_zero_overflow_and_cleanup_match_across_targets() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-usize-owned-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!("retained usize ownership fixture: {}", root.display());
    fs::create_dir(root.join("src")).unwrap();
    let checked = semaprax::check(SOURCE, "app.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    assert_eq!(
        semaprax::format::canonical(&semaprax::parse(&canonical, "app.spx").unwrap()),
        canonical
    );
    fs::write(root.join("src/app.spx"), &canonical).unwrap();
    let tests = "module mul.tests; @id(\"mul.tests.main\") fn main() -> i64 { 0 }\n";
    let tests = semaprax::format::canonical(&semaprax::check(tests, "tests.spx").unwrap());
    fs::write(root.join("src/tests.spx"), &tests).unwrap();
    let manifest_text = "schema = \"semaprax.project.v8\"\nname = \"usize-owned\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"mul.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"mul.forward\", \"mul.precedence\", \"mul.reverse\"]\ntests = [\"mul.tests\"]\n";
    let manifest = root.join("semaprax.toml");
    fs::write(&manifest, manifest_text).unwrap();
    let provider = with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let revision = snapshot.retain_revision();
        let program = revision.entry_program();
        for (name, length, status) in CASES {
            let result = evaluate_resolved_owned_data(
                program,
                &format!("mul.{name}"),
                &[19, 23, 29][..length],
                DEFAULT_MAX_STEPS,
            )?;
            if status == 0 {
                assert_eq!(
                    result.outcome,
                    OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(vec![7, 0, 255]))
                );
                assert_eq!(
                    result.cleanup_events,
                    [OwnedDataCleanupEvent::CopyOutAndSettleBytes]
                );
            } else {
                let OwnedDataEvaluationOutcome::LanguageFailure(failure) = result.outcome else {
                    panic!("{name}/{length}: expected checked failure")
                };
                assert_eq!(failure.domain_id(), "semaprax.arithmetic.v1");
                assert_eq!(failure.code(), status);
                assert!(result.cleanup_events.is_empty());
            }
        }
        let descriptor = revision.public_api_descriptor()?;
        let provider = semaprax::codegen::emit_project_v8_native_owned_data_provider(
            program,
            revision.manifest().web_exports(),
            PublicApiSubject {
                project_schema: revision.manifest().schema(),
                project_revision: revision.project_revision(),
                workspace_revision: revision.workspace_revision(),
                project_graph_digest: revision.semantic_graph_digest(),
            },
            &descriptor.canonical_bytes(),
            &descriptor.digest(),
        )
        .map_err(|error| vec![error])?;
        assert_eq!(provider.descriptor(), descriptor.canonical_bytes());
        assert_eq!(provider.descriptor_digest(), descriptor.digest());
        let inline = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        inline.verify().unwrap();
        owned_npm_publication::publish(snapshot, &manifest, &root.join("package"), false)?;
        let envelope: serde_json::Value = serde_json::from_str(inline.envelope()).unwrap();
        let rows = envelope["artifacts"].as_array().unwrap();
        assert_eq!(rows.len(), FILES.len());
        for (row, name) in rows.iter().zip(FILES) {
            assert_eq!(row["path"], name);
            let hex = row["hex"].as_str().unwrap();
            let expected = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(fs::read(root.join("package").join(name)).unwrap(), expected);
        }
        Ok(provider)
    })
    .unwrap();
    native(&root, provider.source());
    fs::write(root.join("cases.json"), serde_json::to_vec(&CASES).unwrap()).unwrap();
    fs::write(
        root.join("consumer.mjs"),
        include_bytes!("../usize_mul_owned_v1/consumer.mjs"),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("consumer.mjs")
        .current_dir(&root)
        .output()
        .expect("Node required for real npm usize ownership evidence");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"usize-owned-mul-ok\n");
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(root.join("src/app.spx")).unwrap(),
        canonical
    );
    assert_eq!(
        fs::read_to_string(root.join("src/tests.spx")).unwrap(),
        tests
    );
    assert_eq!(fs::read_to_string(manifest).unwrap(), manifest_text);
    // Keep the bounded real Project, package and compiler evidence. No cleanup
    // authority over unrecognized compiler output or failed fixtures is claimed.
}
