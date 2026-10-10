//! The real derived pure library is distinct from an exported or command ABI.
use super::*;

fn source_bytes(root: &std::path::Path) -> Vec<Vec<u8>> {
    [
        "semaprax.toml",
        "src/app.spx",
        "src/schema.spx",
        "src/schema.generated.spx",
        "src/tests.spx",
    ]
    .map(|path| std::fs::read(root.join(path)).unwrap())
    .into()
}

#[test]
fn actual_pure_nested_decoder_admission_does_not_authorize_exports_or_old_commands() {
    // Derive through the real CLI and install its exact canonical output. The
    // existing sibling corpus executes these detached owners on all backends.
    let root = install("nested-order-profile-admission");
    let path = root.join("semaprax.toml");
    let pure_manifest = std::fs::read_to_string(&path).unwrap();
    let before = source_bytes(&root);
    project::with_authenticated_project(&path, |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert!(snapshot
            .entry_program()
            .functions
            .iter()
            .any(|function| function.id.as_str() == "orders.read-fixture"));
        let errors = snapshot.public_api_descriptor().unwrap_err();
        assert_eq!(errors[0].code, "SPX-J105");
        assert_eq!(
            errors[0].message,
            "retained Project v8 admission has no owned-data descriptor"
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(source_bytes(&root), before);

    // Neither selected root reaches the generated schema helpers. Refusal must
    // still inspect that module before reachability cropping can hide them.
    let app = canonical(
        r#"module orders.app;
@id("orders.main") fn main()->i64 {0}
@id("orders.export") fn exported()->i64 {7}
@id("orders.command") fn command()->i64 {0}
"#,
    );
    std::fs::write(root.join("src/app.spx"), app).unwrap();
    let exported = pure_manifest.replace("web = []", "web = [\"orders.export\"]");
    let command = pure_manifest
        .replace("owned-data-api.v1", "language-command-io.collection-record.v1")
        .replace("web = []", "web = [\"orders.command\"]")
        .replace(
            "[dependencies]",
            "[command]\nfunction = \"orders.command\"\ninput = \"argv-utf8+stdin-stream.v1\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n\n[dependencies]",
        );
    for manifest in [exported, command] {
        std::fs::write(&path, manifest).unwrap();
        let before = source_bytes(&root);
        let errors = project::with_authenticated_project(&path, |_| Ok(())).unwrap_err();
        assert!(errors.iter().any(|error| {
            error.code == "SPX-G172"
                && error.message == "nested outcome runtime requires the explicitly selected language-command-io.nested-outcome.v1 profile"
        }), "{errors:?}");
        assert_eq!(source_bytes(&root), before);
    }
    std::fs::remove_dir_all(root).unwrap();
}
