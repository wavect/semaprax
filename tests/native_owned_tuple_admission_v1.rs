//! Raw-C evidence for rejection before the existing provider
//! invocation boundary. This is not the generated safe SDK or a new public ABI.
use semaprax::codegen::NativeOwnedDataProviderArtifact;
use semaprax::project::{with_authenticated_project, PublicApiSubject};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "support/owned_tuple_product.rs"]
mod subject;

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn source_snapshot(root: &Path) -> Vec<Vec<u8>> {
    ["semaprax.toml", "src/app.spx", "src/tests.spx"]
        .into_iter()
        .map(|name| fs::read(root.join(name)).unwrap())
        .collect()
}

fn provider(manifest: &Path, flat: bool) -> NativeOwnedDataProviderArtifact {
    with_authenticated_project(manifest, |snapshot| {
        snapshot.check()?;
        let revision = snapshot.retain_revision();
        let fact = PublicApiSubject {
            project_schema: revision.manifest().schema(),
            project_revision: revision.project_revision(),
            workspace_revision: revision.workspace_revision(),
            project_graph_digest: revision.semantic_graph_digest(),
        };
        let program = revision.entry_program();
        let selected = revision.manifest().web_exports();
        let (bytes, digest, provider) = if flat {
            let descriptor = revision.flat_owned_record_api_descriptor()?;
            let bytes = descriptor.canonical_bytes();
            let digest = descriptor.digest();
            let provider = semaprax::codegen::emit_project_v9_native_flat_owned_record_provider(
                program, selected, fact, &bytes, &digest,
            )
            .map_err(|error| vec![error])?;
            (bytes, digest, provider)
        } else {
            let descriptor = revision.public_api_descriptor()?;
            let bytes = descriptor.canonical_bytes();
            let digest = descriptor.digest();
            let provider = semaprax::codegen::emit_project_v8_native_owned_data_provider(
                program, selected, fact, &bytes, &digest,
            )
            .map_err(|error| vec![error])?;
            (bytes, digest, provider)
        };
        // Each public emitter independently replays these bytes against the
        // retained HIR/selection/revision facts before rendering the provider.
        assert_eq!(provider.descriptor(), bytes);
        assert_eq!(provider.descriptor_digest(), digest);
        let descriptor: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            descriptor["exports"]
                .as_array()
                .unwrap()
                .iter()
                .map(|export| export["stable_id"].as_str().unwrap())
                .collect::<Vec<_>>(),
            if flat {
                vec!["tuple.bytes", "tuple.text"]
            } else {
                vec!["tuple.bytes", "tuple.maybe", "tuple.result", "tuple.text"]
            }
        );
        assert_eq!(descriptor["limits"]["max_borrowed_input_bytes"], 65_536);
        assert_eq!(descriptor["limits"]["max_owned_output_bytes"], 65_536);
        if flat {
            for export in descriptor["exports"].as_array().unwrap() {
                assert_eq!(export["result"]["record_id"], "tuple.Record");
                let fields = export["result"]["fields"].as_array().unwrap();
                assert_eq!(fields.len(), 4);
                for ((field, id), ty) in fields
                    .iter()
                    .zip(["bytes", "text", "left", "right"])
                    .zip(["owned-bytes", "usize", "usize", "usize"])
                {
                    assert_eq!(field["stable_id"], id);
                    assert_eq!(field["type"], ty);
                }
            }
        }
        Ok(provider)
    })
    .unwrap()
}

#[test]
fn real_project_tuple_rejection_precedes_native_entry_and_allocation_at_o0_o2() {
    let compiler = std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from);
    let root = std::env::temp_dir().join(format!(
        "semaprax-native-tuple-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!("retained raw native tuple fixture: {}", root.display());
    for flat in [false, true] {
        let label = if flat { "v9" } else { "v8" };
        let directory = root.join(label);
        fs::create_dir(&directory).unwrap();
        let project = directory.join("project");
        fs::create_dir(&project).unwrap();
        let manifest = subject::write_project(&project, flat);
        let before = source_snapshot(&project);
        let artifact = provider(&manifest, flat);
        fs::write(directory.join("descriptor.json"), artifact.descriptor()).unwrap();
        fs::write(directory.join("provider.c"), artifact.source()).unwrap();
        let source = directory.join("probe.c");
        // Only preprocessing around the unmodified provider observes libc
        // calls. No source rewriting, generated test hooks, fault defines, or
        // substituted semantic functions participate in this witness.
        let instrumented = format!(
            "{}\n{}\n{}\n#define FIXTURE_FLAT {}\n{}",
            include_str!("support/native_fixture_stdio.c"),
            include_str!("native_owned_tuple_admission_v1/allocations.c"),
            artifact.source(),
            u8::from(flat),
            include_str!("native_owned_tuple_admission_v1/probe.c"),
        );
        fs::write(&source, instrumented.as_bytes()).unwrap();
        for optimization in ["-O0", "-O2"] {
            let executable = directory.join(format!(
                "probe{optimization}{}",
                std::env::consts::EXE_SUFFIX,
            ));
            let compile = Command::new(&compiler)
                .current_dir(&directory)
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&source)
                .arg("-o")
                .arg(&executable)
                .output()
                .expect("Clang is required; raw native tuple evidence cannot silently skip");
            assert!(
                compile.status.success(),
                "{} {optimization}: stdout={} stderr={}",
                directory.display(),
                String::from_utf8_lossy(&compile.stdout),
                String::from_utf8_lossy(&compile.stderr),
            );
            let result = Command::new(&executable)
                .current_dir(&directory)
                .output()
                .expect("execute raw native tuple fixture");
            assert!(
                result.status.success(),
                "{} {optimization}: stdout={} stderr={}",
                directory.display(),
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr),
            );
            assert_eq!(result.stdout, b"native-owned-tuple-admission-ok\n");
            assert!(result.stderr.is_empty());
        }
        assert_eq!(source_snapshot(&project), before);
        assert_eq!(
            fs::read(directory.join("provider.c")).unwrap(),
            artifact.source().as_bytes()
        );
        assert_eq!(
            fs::read(directory.join("descriptor.json")).unwrap(),
            artifact.descriptor()
        );
        assert_eq!(fs::read(source).unwrap(), instrumented.as_bytes());
    }
    // Retain the bounded input/project/compiler evidence, including failures.
    // This uses one physical C context per executable; it does not assert that
    // the safe SDK retains contexts, or measure every internal semantic step.
}
