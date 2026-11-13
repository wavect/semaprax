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
        assert!(
            collected.contains(text(file, "path")) && collected.contains(text(file, "content"))
        );
    }
    for source in route["sources"].as_array().unwrap() {
        assert!(collected.contains(source.as_str().unwrap()));
    }
    for forbidden in [
        "ShiftSim",
        "shiftsim",
        "examples/shiftsim",
        "schedule decision",
    ] {
        assert!(
            !collected.contains(forbidden),
            "benchmark assistance leaked: {forbidden}"
        );
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
    let catalog: Value = serde_json::from_str(super::super::super::LIBRARY_INDEX).unwrap();
    let retained = derive::retained_json_descriptor(&catalog);
    let route = &retained;
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

#[test]
fn selected_file_task_pages_keep_full_prerequisites_inputs_and_exact_byte_bounds() {
    let descriptors: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    let route = &descriptors["selected_file_task"];
    assert_eq!(
        route["project_profile"],
        "source-command.selected-file.resource-output.v1"
    );
    assert_eq!(route["input"], "argv-utf8-256+selected-file.follow.v1");
    assert!(route["dependencies"].as_object().unwrap().is_empty());
    assert!(route["declarations"].as_array().unwrap().is_empty());
    let catalog: Value = serde_json::from_str(super::super::super::LIBRARY_INDEX).unwrap();
    let items = selected_file_items(route, &catalog).unwrap();
    let capsule = selected_file_capsule(route);
    let mut collected = String::new();
    let mut start = 0usize;
    loop {
        let query = format!("author:task:selected-file:source-command.selected-file.resource-output.v1:native64:{start}");
        let output = lookup(&query, 2048).unwrap();
        assert_eq!(lookup(&query, 2048).unwrap(), output);
        assert!(output.len() <= 2048 && output.starts_with(&capsule));
        for cap in route["capabilities"].as_array().unwrap() {
            assert!(output.contains(cap.as_str().unwrap()));
        }
        for fact in [
            "256 argv",
            "65536 aggregate argv bytes",
            "4096 path bytes",
            "65536 UTF-8 file bytes",
            "64 read attempts/1MiB",
            "1048576-byte",
            "INDEX=0..255",
            "no file grant",
            "one regular-file descriptor",
            "not an atomic snapshot",
            "Interpreter/Web/Wasm/npm/non-Unix refuse",
            "qualification pending",
            "including G171",
        ] {
            assert!(
                output.contains(fact),
                "missing complete prerequisite: {fact}"
            );
        }
        collected.push_str(output.strip_prefix(&capsule).unwrap());
        if let Some(next) = output.lines().find_map(|line| {
            line.split_once("next: help language ")
                .map(|(_, next)| next)
        }) {
            start = next.rsplit(':').next().unwrap().parse().unwrap();
        } else {
            assert!(output.ends_with("executable qualification pending.\n"));
            break;
        }
    }
    assert!(collected.contains(route["manifest"].as_str().unwrap()));
    assert!(collected.contains("NOT a new template"));
    assert!(collected.contains("semaprax test \"$p\" --target native"));
    assert!(collected.contains("./app-bin --spx-selected-file-v1 0 -- <file> <your-options>"));
    for forbidden in [
        "LogLens",
        "loglens",
        "experiments/selected-file-input",
        "sha256",
        "--top",
    ] {
        assert!(
            !collected.contains(forbidden),
            "benchmark assistance leaked: {forbidden}"
        );
        assert!(!capsule.contains(forbidden));
    }
    assert!(collected.contains(route["source"].as_str().unwrap()));
    assert!(collected.contains(route["test_source"].as_str().unwrap()));
    for start in 0..items.len() {
        let exact = capsule.len()
            + items[start].len()
            + selected_file_continuation(start + 1, items.len()).len();
        let page = selected_file_page(route, &catalog, start, exact).unwrap();
        assert_eq!(page.len(), exact);
        assert!(selected_file_page(route, &catalog, start, exact - 1).is_err());
        assert_eq!(
            selected_file_page(route, &catalog, start, exact + 1).unwrap(),
            page
        );
    }
    assert!(selected_file_page(route, &catalog, 0, 0).is_err());
    assert!(selected_file_page(route, &catalog, items.len(), 2048).is_err());
    let existing = super::super::lookup("author:selected-file-resource").unwrap();
    for bound in ["256", "65536", "4096", "64", "1048576"] {
        assert!(existing.contains(bound) && capsule.contains(bound));
    }
}

#[test]
fn selected_file_task_refuses_foreign_profiles_stale_ids_and_malformed_continuations() {
    for query in [
        "author:task:selected-file:source-command.v1:native64:0",
        "author:task:selected-file:source-command.selected-file.v1:native64:0",
        "author:task:selected-file:source-command.selected-file.resource-output.v1:wasm32:0",
        "author:task:selected-file:source-command.selected-file.resource-output.v1:web:0",
        "author:task:selected-file:source-command.selected-file.resource-output.v1:native64:00",
        "author:task:selected-file:source-command.selected-file.resource-output.v1:native64:-1",
        "author:task:selected-file:source-command.selected-file.resource-output.v1:native64:18446744073709551616",
        "author:task:selected-file:source-command.selected-file.resource-output.v1:native64:0:extra",
    ] {
        assert!(lookup(query, 2048).is_err(), "{query}");
    }
    let descriptors: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    let catalog: Value = serde_json::from_str(super::super::super::LIBRARY_INDEX).unwrap();
    let mut stale = descriptors["selected_file_task"].clone();
    stale["declarations"] = serde_json::json!(["zz.foreign.late.identity"]);
    assert!(selected_file_page(&stale, &catalog, 0, 2048)
        .unwrap_err()
        .contains("stale/ambiguous"));
    stale["declarations"] = serde_json::json!(["std.text.equals", "std.text.equals"]);
    assert!(selected_file_page(&stale, &catalog, 0, 2048)
        .unwrap_err()
        .contains("ordered and unique"));
    assert!(
        lookup("author:task:file-text:source-command.v1:native64:0", 2048)
            .unwrap()
            .contains("source-command-file-text")
    );
}

#[test]
fn neutral_task_sources_are_canonical_before_ordinary_admission() {
    let descriptors: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    let selected = &descriptors["selected_file_task"];
    let mut sources = vec![
        ("main.spx", text(selected, "source")),
        ("tests.spx", text(selected, "test_source")),
    ];
    for file in descriptors["json_task"]["files"].as_array().unwrap() {
        let path = text(file, "path");
        if path.ends_with(".spx") {
            sources.push((path, text(file, "content")));
        }
    }
    assert_eq!(sources.len(), 6, "every emitted neutral SPX source");
    for (path, source) in sources {
        let (_, canonical) = semaprax::parse_canonical(source, path).unwrap();
        assert_eq!(source, canonical, "canonical neutral task source: {path}");
        assert_eq!(
            semaprax::parse_canonical(&canonical, path).unwrap().1,
            canonical,
            "canonical neutral task source is stable: {path}"
        );
    }
}

#[test]
fn neutral_task_examples_pass_ordinary_project_checks_and_independent_codec_plan_replay() {
    use semaprax::project;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let descriptors: Value = serde_json::from_str(DESCRIPTORS).unwrap();
    for category in ["selected_file_task", "json_task"] {
        let route = &descriptors[category];
        let serialized = serde_json::to_string(route).unwrap();
        for forbidden in [
            "LogLens",
            "loglens",
            "ShiftSim",
            "shiftsim",
            "experiments/",
            "examples/",
            "sha256",
        ] {
            assert!(
                !serialized.contains(forbidden),
                "non-neutral production descriptor: {forbidden}"
            );
        }

        // Project admission refuses symlink ancestors, including macOS /var.
        let temporary_parent = std::env::temp_dir().canonicalize().unwrap();
        let directory = temporary_parent.join(format!(
            "semaprax-neutral-task-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        struct Remove(std::path::PathBuf);
        impl Drop for Remove {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }
        let cleanup = Remove(directory);
        if category == "selected_file_task" {
            for (path, key) in [
                ("semaprax.toml", "manifest"),
                ("main.spx", "source"),
                ("tests.spx", "test_source"),
            ] {
                std::fs::write(cleanup.0.join(path), text(route, key)).unwrap();
            }
        } else {
            for file in route["files"].as_array().unwrap() {
                let path = cleanup.0.join(text(file, "path"));
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, text(file, "content")).unwrap();
            }
        }
        let admission =
            project::with_authenticated_project(&cleanup.0.join("semaprax.toml"), |snapshot| {
                // Ordinary project checking owns both entry and declared-test HIR;
                // these no-op examples claim no execution or useful application.
                if category == "json_task" {
                    let plan = serde_json::to_vec(&route["plan"]).unwrap();
                    let (expected_revision, entries) =
                        project::decode_json_codec_plan(&plan).map_err(|error| vec![error])?;
                    assert!(expected_revision.is_none());
                    let revision = snapshot.retain_revision();
                    let bundle = project::derive_json_codec_plan(&revision, &entries)?;
                    project::verify_json_codec_plan(&revision, &entries, &bundle)?;
                }
                Ok(())
            });
        if category == "selected_file_task" && !cfg!(all(unix, target_pointer_width = "64")) {
            let errors =
                admission.expect_err("the selected-file profile requires a native64 Unix host");
            assert_eq!(errors.len(), 1);
            assert_eq!(errors[0].code, "SPX-B104");
            assert_eq!(
                errors[0].message,
                "source-command.selected-file.resource-output.v1 requires a native64 Unix host"
            );
        } else {
            admission
                .unwrap_or_else(|error| panic!("neutral {category} ordinary admission: {error:?}"));
        }
    }
}
