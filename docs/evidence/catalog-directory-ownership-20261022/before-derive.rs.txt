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
    serde_json::json!({"schema":"semaprax.authoring-tasks.v6","scaffold_name":"task-demo","layout":"tables","tasks":tasks,"json_task":json_descriptor(catalog),"selected_file_task":selected_file_descriptor()})
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
pub(super) fn retained_json_descriptor(catalog: &Value) -> Value {
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

fn json_descriptor(catalog: &Value) -> Value {
    let descriptors: Value = serde_json::from_str(include_str!("catalog.json")).unwrap();
    let route = descriptors["json_task"].clone();
    let retained = retained_json_descriptor(catalog);
    for key in [
        "project_profile",
        "target",
        "input",
        "capabilities",
        "dependencies",
        "dependency_closure",
        "dependency_versions",
        "declarations",
    ] {
        assert_eq!(
            route[key], retained[key],
            "exact generic prerequisite: {key}"
        );
    }
    let manifest = route["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "semaprax.toml")
        .unwrap();
    let manifest = ProjectManifest::parse(manifest["content"].as_str().unwrap()).unwrap();
    assert_eq!(route["sources"], serde_json::json!(manifest.sources()));
    for file in route["files"].as_array().unwrap() {
        let path = file["path"].as_str().unwrap();
        if path.ends_with(".spx") {
            let source = file["content"].as_str().unwrap();
            let (_, canonical) = semaprax::parse_canonical(source, path).unwrap();
            assert_eq!(
                source, canonical,
                "canonical neutral JSON task source: {path}"
            );
        }
    }
    check_descriptor_project(route["files"].as_array().unwrap());
    route
}

// The production descriptor contains only neutral examples. This test-only
// owner checks their ordinary grammar/types and the exact existing profile.
fn selected_file_descriptor() -> Value {
    let descriptors: Value = serde_json::from_str(include_str!("catalog.json")).unwrap();
    let route = descriptors["selected_file_task"].clone();
    let original = std::fs::read_to_string(
        repository().join("experiments/selected-file-input/loglens/semaprax.toml"),
    )
    .unwrap();
    let original = ProjectManifest::parse(&original).unwrap();
    let neutral = ProjectManifest::parse(route["manifest"].as_str().unwrap()).unwrap();
    assert_eq!(neutral.profile(), original.profile());
    assert_eq!(neutral.command_input(), original.command_input());
    assert_eq!(neutral.capabilities(), original.capabilities());
    assert_eq!(route["project_profile"], neutral.profile().unwrap());
    assert_eq!(route["input"], neutral.command_input().unwrap());
    assert_eq!(
        route["capabilities"],
        serde_json::json!(neutral.capabilities())
    );
    assert_eq!(route["sources"], serde_json::json!(neutral.sources()));
    assert_eq!(route["command"], neutral.command().unwrap());
    assert!(neutral.dependencies().is_empty());
    for (key, path) in [("source", "main.spx"), ("test_source", "tests.spx")] {
        let source = route[key].as_str().unwrap();
        let (program, canonical) = semaprax::parse_canonical(source, path).unwrap();
        assert_eq!(
            source, canonical,
            "canonical neutral selected-file task source: {path}"
        );
        assert!(program.module_uses.is_empty());
    }
    check_descriptor_project(&[
        serde_json::json!({"path":"semaprax.toml","content":route["manifest"]}),
        serde_json::json!({"path":"main.spx","content":route["source"]}),
        serde_json::json!({"path":"tests.spx","content":route["test_source"]}),
    ]);
    route
}

// Library modules have no executable main. Validate every declaration and
// import through the ordinary authenticated project owner, rather than asking
// the standalone executable resolver to accept each library independently.
fn check_descriptor_project(files: &[Value]) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Directory(std::path::PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let directory = Directory(std::env::temp_dir().join(format!(
        "semaprax-task-descriptor-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir(&directory.0).unwrap();
    for file in files {
        let path = std::path::Path::new(file["path"].as_str().unwrap());
        assert!(path
            .components()
            .all(|part| matches!(part, std::path::Component::Normal(_))));
        let path = directory.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, file["content"].as_str().unwrap()).unwrap();
    }
    semaprax::project::with_authenticated_project(&directory.0.join("semaprax.toml"), |project| {
        project.check()
    })
    .unwrap();
}
