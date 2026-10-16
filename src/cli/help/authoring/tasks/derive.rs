//! Test-only descriptor projection over the ordinary checked scaffold owner.
use semaprax::project::{derive_project_scaffold_v1_with_layout, ProjectManifest, ScaffoldLayout};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn descriptors() -> Value {
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
    serde_json::json!({"schema":"semaprax.authoring-tasks.v6","scaffold_name":"task-demo","layout":"tables","tasks":tasks})
}
