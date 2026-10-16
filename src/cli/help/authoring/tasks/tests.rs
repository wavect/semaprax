use super::*;
use std::collections::BTreeSet;
#[path = "derive.rs"]
mod derive;

#[test]
fn task_descriptors_equal_actual_checked_scaffolds_and_exact_import_inventory() {
    let actual: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    assert_eq!(actual, derive::descriptors());
    assert_eq!(
        DESCRIPTORS,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&derive::descriptors()).unwrap()
        )
    );
    let catalog: Value = serde_json::from_str(super::super::super::LIBRARY_INDEX).unwrap();
    for route in actual["tasks"].as_array().unwrap() {
        for id in route["declarations"].as_array().unwrap() {
            let row = declaration(&catalog, id.as_str().unwrap()).unwrap();
            assert!(row.contains("library required-profile"));
            assert!(row.contains("status partial"));
        }
    }
    assert!(actual["tasks"][1]["declarations"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(actual["tasks"][1]["dependencies"]
        .as_object()
        .unwrap()
        .is_empty());
}

#[test]
fn task_pages_repeat_whole_prerequisites_and_cover_every_import_deterministically() {
    let descriptors: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    let catalog: Value = serde_json::from_str(super::super::super::LIBRARY_INDEX).unwrap();
    assert!(lookup("author:tasks", 2_048).unwrap().len() <= 2_048);
    for route in descriptors["tasks"].as_array().unwrap() {
        let mut start = 0;
        let mut seen = Vec::new();
        loop {
            let query = format!(
                "author:task:{}:{}:{}:{start}",
                text(route, "task"),
                text(route, "project_profile"),
                text(route, "target")
            );
            let output = lookup(&query, 2_048).unwrap();
            assert_eq!(lookup(&query, 2_048).unwrap(), output);
            assert!(output.len() <= 2_048 && output.starts_with(&capsule(route)));
            for id in route["declarations"].as_array().unwrap() {
                let id = id.as_str().unwrap();
                if output.contains(&format!("\n{id}\n")) {
                    assert!(output.contains(&declaration(&catalog, id).unwrap()));
                    seen.push(id.to_owned());
                }
            }
            if let Some(next) = output.lines().find_map(|line| {
                line.split_once("next: help language ")
                    .map(|(_, next)| next)
            }) {
                let next_start = next.rsplit(':').next().unwrap().parse::<usize>().unwrap();
                assert!(next_start > start);
                start = next_start;
            } else {
                assert!(output.ends_with(
                    "Coverage: complete checked-starter imported declaration subset.\n"
                ));
                break;
            }
        }
        assert_eq!(
            seen,
            route["declarations"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        );
        assert_eq!(seen.iter().collect::<BTreeSet<_>>().len(), seen.len());
        let exact = capsule(route).len()
            + if seen.is_empty() {
                0
            } else {
                declaration(&catalog, seen.last().unwrap()).unwrap().len()
            }
            + continuation(route, seen.len(), seen.len()).len();
        let start = seen.len().saturating_sub(1);
        assert_eq!(page(route, &catalog, start, exact).unwrap().len(), exact);
        assert!(page(route, &catalog, start, exact - 1).is_err());
        assert_eq!(
            page(route, &catalog, start, exact + 1).unwrap().len(),
            exact
        );
    }
    for query in [
        "author:task:file-text:source-command.v1:native64:00",
        "author:task:file-text:source-command.v1:native64:3",
        "author:task:file-text:source-command.v1:wasm32:0",
        "author:task:file-text:owned-data-api.v1:native64:0",
        "author:task:unknown:source-command.v1:native64:0",
        "author:task:file-text:source-command.v1:native64:0:extra",
        "author:task:stdin-json:language-command-io.stream-data.v1:native64:0",
    ] {
        assert!(lookup(query, 2_048).is_err(), "{query}");
    }
    let mut stale = descriptors["tasks"][0].clone();
    stale["declarations"][2] = Value::String("std.missing.identity".to_owned());
    assert!(page(&stale, &catalog, 0, 2_048)
        .unwrap_err()
        .contains("stale/ambiguous"));
}

#[test]
fn json_task_whole_entries_are_complete_deterministic_and_exactly_bounded() {
    let descriptors: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    let route = &descriptors["json_task"];
    let catalog: Value = serde_json::from_str(super::super::super::LIBRARY_INDEX).unwrap();
    let items = json_items(route, &catalog).unwrap();
    let first = lookup(
        "author:task:stdin-json:language-command-io.collection-record.v1:native64:0",
        2_048,
    )
    .unwrap();
    for required in [
        "semaprax-full new fresh-app --from",
        "--codec-plan",
        "--command-source",
        "semaprax check fresh-app",
        "semaprax test fresh-app --target native-collection-record-pure.v1",
        "semaprax build fresh-app --target native",
        "./app-bin <",
        "no Web/Wasm",
        "executable qualification pending",
    ] {
        assert!(first.contains(required), "first page lacks {required}");
    }
    let mut start = 0;
    let mut collected = String::new();
    loop {
        let query = format!(
            "author:task:stdin-json:language-command-io.collection-record.v1:native64:{start}"
        );
        let output = lookup(&query, 2_048).unwrap();
        assert_eq!(lookup(&query, 2_048).unwrap(), output);
        assert!(output.len() <= 2_048 && output.starts_with(&json_capsule(route)));
        let next = output
            .lines()
            .find_map(|line| line.split_once("next: help language ").map(|(_, q)| q));
        let end = next.map_or(items.len(), |q| {
            q.rsplit(':').next().unwrap().parse::<usize>().unwrap()
        });
        assert!(end > start);
        assert_eq!(
            output,
            format!(
                "{}{}{}",
                json_capsule(route),
                items[start..end].concat(),
                json_continuation(end, items.len())
            )
        );
        collected.push_str(&items[start..end].concat());
        if next.is_none() {
            break;
        }
        start = end;
    }
    assert_eq!(collected, items.concat());
    for (index, item) in items.iter().enumerate() {
        let exact = json_capsule(route).len()
            + item.len()
            + json_continuation(index + 1, items.len()).len();
        assert_eq!(
            json_page(route, &catalog, index, exact).unwrap().len(),
            exact
        );
        assert!(json_page(route, &catalog, index, exact - 1).is_err());
        assert_eq!(
            json_page(route, &catalog, index, exact + 1).unwrap().len(),
            exact
        );
    }
    for file in route["files"].as_array().unwrap() {
        assert!(collected.contains(text(file, "path")) && collected.contains(text(file, "sha256")));
    }
    for source in route["sources"].as_array().unwrap() {
        assert!(collected.contains(source.as_str().unwrap()));
    }
    for stage in route["pipeline"].as_array().unwrap() {
        assert!(collected.contains(text(stage, "id")));
        for line in stage["head"].as_array().unwrap() {
            assert!(collected.contains(line.as_str().unwrap()));
        }
    }
    assert!(collected.contains("semaprax-full new fresh-app --from"));
    assert!(collected.contains("native-collection-record-pure.v1"));
    assert!(collected.contains("No source overwrite or runtime execution"));
    assert!(collected.contains("not a claim every declaration is callable"));
    assert!(json_page(route, &catalog, items.len(), 2_048).is_err());
}

#[test]
fn json_task_refuses_foreign_profiles_stale_tail_and_duplicate_identities() {
    for query in [
        "author:task:stdin-json:language-command-io.stream-data.v1:native64:0",
        "author:task:stdin-json:language-command-io.collection-record.v1:wasm32:0",
        "author:task:stdin-json:language-command-io.collection-record.v1:native64:00",
        "author:task:stdin-json:language-command-io.collection-record.v1:native64:999999",
        "author:task:stdin-json:language-command-io.collection-record.v1:native64:0:extra",
    ] {
        assert!(lookup(query, 2_048).is_err(), "{query}");
    }
    let descriptor: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    let catalog: Value = serde_json::from_str(super::super::super::LIBRARY_INDEX).unwrap();
    let mut route = descriptor["json_task"].clone();
    let count = route["declarations"].as_array().unwrap().len();
    route["declarations"][count - 1] = Value::String("zzzz.stale.later.identity".to_owned());
    assert!(json_page(&route, &catalog, 0, 2_048)
        .unwrap_err()
        .contains("stale/ambiguous"));
    let mut route = descriptor["json_task"].clone();
    route["declarations"][1] = route["declarations"][0].clone();
    assert!(json_page(&route, &catalog, 0, 2_048)
        .unwrap_err()
        .contains("strictly ordered"));
}

#[test]
fn json_task_exact_specimen_has_ordinary_paired_starter_compiler_admission() {
    // This is an owning future compiler check, not a metadata-only claim of
    // admission. Ordinary G171808 and every other refusal must remain visible.
    use semaprax::project;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let descriptor: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    let route = &descriptor["json_task"];
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .find(|root| root.join(text(route, "specimen")).is_dir())
        .unwrap();
    let path = std::env::temp_dir().join(format!(
        "semaprax-json-task-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    let cleanup = Remove(path.canonicalize().unwrap());
    let original = repository.join(text(route, "specimen"));
    for file in route["files"].as_array().unwrap() {
        let target = cleanup.0.join(text(file, "path"));
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(original.join(text(file, "path")), target).unwrap();
    }
    let plan = std::fs::read(cleanup.0.join("codec-plan.json")).unwrap();
    let command = project::ApplicationCommandSource {
        path: "src/app.spx".to_owned(),
        source: std::fs::read_to_string(cleanup.0.join("src/app.command.spx")).unwrap(),
    };
    project::with_authenticated_project(&cleanup.0.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let actual: BTreeSet<_> = revision
            .sources()
            .iter()
            .filter_map(|source| {
                source
                    .path()
                    .strip_prefix("dependencies/")
                    .and_then(|s| s.split('/').next())
                    .map(str::to_owned)
            })
            .collect();
        let expected: BTreeSet<_> = route["dependency_closure"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_owned())
            .collect();
        assert_eq!(
            actual, expected,
            "closure must match the compiler's authenticated dependencies"
        );
        let versions: serde_json::Map<String, Value> = revision
            .sources()
            .iter()
            .filter_map(|source| {
                let path = source.path().strip_prefix("dependencies/")?;
                let mut parts = path.split('/');
                Some((
                    parts.next().unwrap().to_owned(),
                    Value::String(parts.next().unwrap().to_owned()),
                ))
            })
            .collect();
        assert_eq!(
            Value::Object(versions),
            route["dependency_versions"],
            "exact resolved versions"
        );
        let starter = project::derive_application_starter(
            &revision,
            &plan,
            text(route, "project_profile"),
            Some(&command),
        )?;
        assert!(starter
            .files()
            .iter()
            .any(|(path, _)| path == "src/app.spx"));
        Ok(())
    })
    .expect(
        "fixed JSON task must pass the same ordinary paired-plan/activated-command compiler owner",
    );
}
