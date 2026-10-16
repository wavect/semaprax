//! Test-only descriptor projection over the ordinary checked scaffold owner.
use semaprax::project::{derive_project_scaffold_v1_with_layout, ProjectManifest, ScaffoldLayout};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn descriptors() -> Value {
    let catalog: Value = serde_json::from_str(
        &std::fs::read_to_string(repository().join("std/catalog.json")).unwrap(),
    )
    .unwrap();
    descriptors_from_catalog(&catalog)
}

// Regeneration consumes the newly derived catalog in the same invocation;
// changing a library never requires two passes through an old embedded catalog.
pub(super) fn descriptors_from_catalog(catalog: &Value) -> Value {
    let mut tasks = Vec::new();
    for (task, template, card) in [
        ("file-text", "source-command-file-text", "author:file-text"),
        ("stdin-stream", "stdin-stream-data", "author:stdin-json"),
    ] {
        let artifact =
            derive_project_scaffold_v1_with_layout("task-demo", template, ScaffoldLayout::Tables)
                .unwrap();
        let manifest_file = artifact
            .files()
            .iter()
            .find(|f| f.path() == "semaprax.toml")
            .unwrap();
        let manifest = ProjectManifest::parse(manifest_file.utf8()).unwrap();
        let mut ids = BTreeSet::new();
        let mut files = vec![
            serde_json::json!({"path":manifest_file.path(),"sha256":manifest_file.sha256().strip_prefix("sha256:").unwrap()}),
        ];
        for source in manifest.sources() {
            let file = artifact
                .files()
                .iter()
                .find(|f| f.path() == source)
                .unwrap();
            files.push(serde_json::json!({"path":file.path(),"sha256":file.sha256().strip_prefix("sha256:").unwrap()}));
            let program = semaprax::parse(file.utf8(), file.path()).unwrap();
            for import in &program.module_uses {
                if import.target_module.starts_with("std.") {
                    ids.insert(import.persistent_id.clone());
                }
            }
        }
        let dependencies: serde_json::Map<String, Value> = manifest
            .dependencies()
            .iter()
            .map(|d| (d.name().to_owned(), Value::String(d.range().to_owned())))
            .collect();
        tasks.push(serde_json::json!({"task":task,"template":template,"card":card,"project_profile":manifest.profile().unwrap(),"target":"native64","input":manifest.command_input().unwrap(),"capabilities":manifest.capabilities(),"dependencies":dependencies,"sources":manifest.sources(),"files":files,"declarations":ids.into_iter().collect::<Vec<_>>()}));
    }
    serde_json::json!({"schema":"semaprax.authoring-tasks.v6","scaffold_name":"task-demo","layout":"tables","tasks":tasks,"json_task":json_descriptor(catalog)})
}

// The same fixed paired-plan specimen already has complete compiler/backend
// owners. This metadata producer parses actual inputs and documentation; it
// neither installs the candidate nor claims those pending owners executed.
fn repository() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|root| {
            root.join("examples/shiftsim-typed-record-successor/codec-plan.json")
                .is_file()
        })
        .expect("repository owning the fixed task specimen")
        .to_owned()
}
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
fn json_descriptor(catalog: &Value) -> Value {
    let root = repository();
    let specimen = "examples/shiftsim-typed-record-successor";
    let directory = root.join(specimen);
    let manifest_text = std::fs::read_to_string(directory.join("semaprax.toml")).unwrap();
    let manifest = ProjectManifest::parse(&manifest_text).unwrap();
    let plan_text = std::fs::read_to_string(directory.join("codec-plan.json")).unwrap();
    semaprax::project::decode_json_codec_plan(plan_text.as_bytes()).unwrap();
    let plan: Value = serde_json::from_str(&plan_text).unwrap();
    let dependencies: serde_json::Map<String, Value> = manifest
        .dependencies()
        .iter()
        .map(|d| (d.name().to_owned(), Value::String(d.range().to_owned())))
        .collect();
    let mut closure = BTreeSet::new();
    let mut versions = serde_json::Map::new();
    let mut pending: Vec<_> = dependencies.keys().cloned().collect();
    while let Some(name) = pending.pop() {
        if !closure.insert(name.clone()) {
            continue;
        }
        let module = catalog["modules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["module"] == name)
            .expect("exact dependency module");
        let library = ProjectManifest::parse(
            &std::fs::read_to_string(
                root.join(module["package"].as_str().unwrap())
                    .join("semaprax.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        versions.insert(
            name.clone(),
            Value::String(library.package_version().unwrap().to_owned()),
        );
        for dependency in library.dependencies() {
            pending.push(dependency.name().to_owned());
        }
    }
    let mut declarations = BTreeSet::new();
    for module in catalog["modules"].as_array().unwrap() {
        if closure.contains(module["module"].as_str().unwrap()) {
            for declaration in module["declarations"].as_array().unwrap() {
                declarations.insert(declaration["id"].as_str().unwrap().to_owned());
            }
        }
    }
    let mut paths: BTreeSet<_> = manifest.sources().iter().cloned().collect();
    paths.extend(
        [
            "semaprax.toml",
            "codec-plan.json",
            "src/app.command.spx",
            "fixtures/request.json",
        ]
        .map(str::to_owned),
    );
    let files: Vec<_> = paths
        .into_iter()
        .map(|path| {
            let bytes = std::fs::read(directory.join(&path)).unwrap();
            serde_json::json!({"path":path,"sha256":digest(&bytes)})
        })
        .collect();
    let pipeline: Vec<_> = [
        ("src/app.command.spx", "shiftsim.receive"),
        ("src/schedule.spx", "shiftsim.schedule.run"),
        ("src/app.command.spx", "shiftsim.command"),
    ]
    .into_iter()
    .map(|(path, id)| {
        let source = std::fs::read_to_string(directory.join(path)).unwrap();
        let (program, comments) = semaprax::parse_with_comments(&source, path).unwrap();
        let document = semaprax::doc::document(&program, &comments);
        let entry = document
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .unwrap();
        let head: Vec<_> = entry
            .signature
            .lines()
            .filter(|line| !line.trim_start().starts_with("@id("))
            .collect();
        serde_json::json!({"source":path,"id":id,"head":head})
    })
    .collect();
    serde_json::json!({"task":"stdin-json","specimen":specimen,
        "project_profile":manifest.profile().unwrap(),"target":"native64",
        "input":manifest.command_input().unwrap(),"capabilities":manifest.capabilities(),
        "sources":manifest.sources(),"dependencies":dependencies,"dependency_closure":closure,"dependency_versions":versions,
        "declarations":declarations,"files":files,"plan":plan,"pipeline":pipeline})
}
