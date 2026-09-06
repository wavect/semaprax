use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use semaprax::hir::{DeclarationId, ResolvedType};
use semaprax::project::{
    with_authenticated_project, ProjectCandidate, ProjectNpmBuild, PublicApiResultType,
    SemanticChange, MAX_PROJECT_NPM_BUILD_BYTES,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-concrete-generic-project-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        crate::concrete_generic_record_product::write_project(&root);
        Self(root)
    }

    fn revision(&self) -> Arc<semaprax::project::ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    fn replace_generic_import(&self, stable_id: &str, declaration: &str) {
        let types = format!(
            "module generic.product.types;\n\n{declaration}\n\n@id(\"generic.product.types.marker\")\nfn type_marker() -> i64 {{ 0 }}\n"
        );
        let app = format!(
            "module generic.product.app;\nuse type @id(\"{stable_id}\") from generic.product.types as Imported;\n\n@id(\"generic.product.evaluate\")\nfn evaluate(input: borrow Slice<u8>) -> i64 {{ if byte_len(input) > 0usize {{ 1 }} else {{ 0 }} }}\n\n@id(\"generic.product.app.main\")\nfn main() -> i64 {{ 0 }}\n"
        );
        for (path, source) in [
            (self.0.join("src/types.spx"), types),
            (self.0.join("src/app.spx"), app),
        ] {
            let parsed = semaprax::parse(&source, &path).unwrap();
            std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct RuntimeDirectory(PathBuf);

impl RuntimeDirectory {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "spx-concrete-generic-project-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}

impl Drop for RuntimeDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn package_artifacts(build: &ProjectNpmBuild) -> BTreeMap<String, Vec<u8>> {
    let envelope: Value = serde_json::from_str(build.envelope()).unwrap();
    envelope["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect()
}

fn compile_and_run_c(source: &str, directory: &Path, label: &str, optimization: &str) {
    let c = directory.join(format!("{label}.c"));
    let executable = directory.join(format!("{label}-{}", &optimization[1..]));
    std::fs::write(&c, source).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
        .arg(&c)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new(executable).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "1");
    assert!(output.stderr.is_empty());
}

fn concrete_pair() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new("generic.product.pair"),
        arguments: vec![ResolvedType::Bytes, ResolvedType::Bool],
    }
}

fn assert_concrete_identity(revision: &semaprax::project::ProjectRevision) {
    let program = revision.entry_program();
    let make = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "generic.product.make")
        .expect("cross-file make closure");
    assert_eq!(make.return_type, concrete_pair());
    assert!(program.types.iter().any(|declaration| {
        declaration.id.as_str() == "generic.product.pair" && declaration.type_parameters.len() == 2
    }));
    assert!(make
        .cleanup
        .slots
        .iter()
        .any(|slot| slot.ty == concrete_pair()));
}

#[test]
fn cross_file_generic_owned_identity_replays_without_widening_the_public_abi() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    assert_eq!(revision.sources().len(), 3);
    assert_concrete_identity(&revision);

    let descriptor = revision.public_api_descriptor().unwrap();
    assert_eq!(descriptor.exports().len(), 1);
    assert_eq!(
        descriptor.exports()[0].stable_id().as_str(),
        "generic.product.evaluate"
    );
    assert_eq!(descriptor.exports()[0].result(), PublicApiResultType::I64);
    assert!(!String::from_utf8(descriptor.canonical_bytes())
        .unwrap()
        .contains("generic.product.pair"));

    let first = revision
        .build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)
        .unwrap();
    let second = revision
        .build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)
        .unwrap();
    assert_eq!(first.envelope(), second.envelope());
    assert_eq!(first.payload_digest(), second.payload_digest());
    first.verify().unwrap();
    let envelope: Value = serde_json::from_str(first.envelope()).unwrap();
    let recipe = envelope["semantic_recipe"].as_str().unwrap();
    assert!(recipe.contains("record Pair<P0, P1>"));
    assert!(recipe.contains("Pair<Bytes, bool>"));
    ProjectNpmBuild::inspect_envelope(first.envelope(), MAX_PROJECT_NPM_BUILD_BYTES).unwrap();

    let base = ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let change = SemanticChange::new(
        revision.project_revision(),
        &json!({
            "kind": "rename_declaration",
            "target": "generic.product.evaluate",
            "name": "evaluate_product"
        }),
    )
    .unwrap();
    let candidate = base.apply(base.candidate_digest(), &change).unwrap();
    assert_concrete_identity(candidate.revision());

    let delta = candidate.abi_delta(candidate.candidate_digest()).unwrap();
    let delta_value: Value = serde_json::from_str(&delta).unwrap();
    assert!(delta_value["facts"]["public_nominals"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        delta_value["facts"]["functions"][0]["id"],
        "generic.product.evaluate"
    );
    candidate
        .verify_abi_delta(candidate.candidate_digest(), delta.as_bytes())
        .unwrap();

    let capsule = candidate.recovery_capsule().unwrap();
    let restored = ProjectCandidate::restore(
        Arc::clone(&revision),
        revision.project_revision(),
        capsule.as_bytes(),
    )
    .unwrap();
    assert_eq!(restored.candidate_digest(), candidate.candidate_digest());
    assert_concrete_identity(restored.revision());
}

#[test]
fn cross_file_generic_owned_record_executes_through_project_products() {
    let fixture = Fixture::new();
    let native = RuntimeDirectory::new("native");
    let npm = RuntimeDirectory::new("npm");
    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        for _ in 0..3 {
            let entry = snapshot.execute_entry(&Default::default())?;
            assert_eq!(
                entry.outcome(),
                &semaprax::project::ProjectExecutionOutcome::Returned(1)
            );
            let tests = snapshot.execute_test(&Default::default())?;
            assert_eq!(
                tests.outcome(),
                &semaprax::project::ProjectExecutionOutcome::Returned(0)
            );
        }

        let generated =
            semaprax::codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            compile_and_run_c(&generated, &native.0, "entry", optimization);
        }

        let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        build.verify().map_err(|error| vec![error])?;
        for (path, bytes) in package_artifacts(&build) {
            std::fs::write(npm.0.join(path), bytes).unwrap();
        }
        std::fs::write(
            npm.0.join("project-generic-owned.mjs"),
            r#"import fs from 'node:fs';
import instantiate from './semaprax.bindings.js';
const wasm = new Uint8Array(fs.readFileSync(new URL('./app.wasm', import.meta.url)));
const api = await instantiate(wasm);
const evaluate = api.functions['generic.product.evaluate'];
for (let i = 0; i < 4; i += 1) {
  if (evaluate(new Uint8Array()) !== 0n) throw Error('empty generic record result');
  if (evaluate(new Uint8Array([1, 2, 3])) !== 1n) throw Error('owned generic record result');
}
console.log('project-generic-owned-ok');
"#,
        )
        .unwrap();
        let output = Command::new("node")
            .arg("project-generic-owned.mjs")
            .current_dir(&npm.0)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "project-generic-owned-ok"
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn cross_file_generic_owned_product_is_selected_by_the_required_linux_gate() {
    let workflow = include_str!("../../.github/workflows/ci.yml");
    assert!(workflow.contains("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS: \"1\""));
    assert!(workflow.contains(
        "cargo test --locked -p semaprax --test project concrete_generic_project_prerequisites::cross_file_generic_owned_record_executes_through_project_products -- --exact --nocapture"
    ));
}

#[test]
fn cross_file_generic_imports_reject_nonrecord_and_nonflat_templates_with_stable_g172() {
    const MESSAGE: &str = "type target must be an admitted nongeneric value type or flat generic record template without borrowed or nested storage";
    for (stable_id, declaration) in [
        (
            "generic.hostile.class",
            "@id(\"generic.hostile.class\") class Imported<T> { @id(\"generic.hostile.class.value\") value: T, }",
        ),
        (
            "generic.hostile.variant",
            "@id(\"generic.hostile.variant\") variant Imported<T> { @id(\"generic.hostile.variant.some\") Some { @id(\"generic.hostile.variant.some.value\") value: T, }, @id(\"generic.hostile.variant.none\") None, }",
        ),
        (
            "generic.hostile.string",
            "@id(\"generic.hostile.string\") record Imported<T> { @id(\"generic.hostile.string.value\") value: T, @id(\"generic.hostile.string.text\") text: String, }",
        ),
    ] {
        let fixture = Fixture::new();
        fixture.replace_generic_import(stable_id, declaration);
        let errors = with_authenticated_project(&fixture.0.join("semaprax.toml"), |_| Ok(()))
            .unwrap_err();
        assert!(errors.iter().any(|error| {
            error.code == "SPX-G172" && error.message == MESSAGE
        }), "{stable_id}: {errors:?}");
    }
}
