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
