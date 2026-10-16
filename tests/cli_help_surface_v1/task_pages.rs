use super::invoke;

#[test]
fn profile_task_pages_are_bounded_exact_complete_and_inert() {
    let descriptors: serde_json::Value = serde_json::from_str(include_str!(
        "../../src/cli/help/authoring/tasks/catalog.json"
    ))
    .unwrap();
    for route in descriptors["tasks"].as_array().unwrap() {
        let mut query = format!(
            "author:task:{}:{}:{}:0",
            route["task"].as_str().unwrap(),
            route["project_profile"].as_str().unwrap(),
            route["target"].as_str().unwrap()
        );
        let mut seen = Vec::new();
        loop {
            let (page, directory) = invoke(&["help", "language", &query]);
            assert!(page.status.success(), "{query}: {page:?}");
            assert!(page.stderr.is_empty() && page.stdout.len() <= 2_048);
            let output = std::str::from_utf8(&page.stdout).unwrap();
            assert!(output.contains("Checked starter subset; not whole-profile/library admission."));
            assert!(output.contains(route["input"].as_str().unwrap()));
            for cap in route["capabilities"].as_array().unwrap() {
                assert!(output.contains(cap.as_str().unwrap()));
            }
            for file in route["files"].as_array().unwrap() {
                assert!(output.contains(file["path"].as_str().unwrap()));
                assert!(output.contains(file["sha256"].as_str().unwrap()));
            }
            for id in route["declarations"].as_array().unwrap() {
                if output.contains(&format!("\n{}\n", id.as_str().unwrap())) {
                    seen.push(id.as_str().unwrap().to_owned());
                }
            }
            let (again, second_directory) = invoke(&["help", "language", &query]);
            assert!(again.status.success());
            assert_eq!(again.stdout, page.stdout);
            assert!(again.stderr.is_empty());
            for directory in [&directory, &second_directory] {
                assert_eq!(std::fs::read_dir(directory).unwrap().count(), 0);
                std::fs::remove_dir(directory).unwrap();
            }
            if let Some(next) = output.lines().find_map(|line| {
                line.split_once("next: help language ")
                    .map(|(_, next)| next)
            }) {
                assert_ne!(next, query);
                query = next.to_owned();
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
                .map(|id| id.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        );
    }
    let (index, directory) = invoke(&["help", "language", "author:tasks"]);
    assert!(index.status.success() && index.stderr.is_empty() && index.stdout.len() <= 2048);
    assert!(std::str::from_utf8(&index.stdout)
        .unwrap()
        .contains("no checked JSON-composition starter"));
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
    std::fs::remove_dir(directory).unwrap();
    for selector in [
        "author:task:file-text:source-command.v1:native64:00",
        "author:task:file-text:source-command.v1:native64:3",
        "author:task:file-text:source-command.v1:wasm32:0",
        "author:task:file-text:owned-data-api.v1:native64:0",
        "author:task:unknown:source-command.v1:native64:0",
        "author:task:file-text:source-command.v1:native64:0:extra",
        "author:task:stdin-json:language-command-io.stream-data.v1:native64:0",
    ] {
        let (page, directory) = invoke(&["help", "language", selector]);
        assert_eq!(page.status.code(), Some(2), "{selector}: {page:?}");
        assert!(page.stdout.is_empty());
        assert!(!page.stderr.is_empty());
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 0);
        std::fs::remove_dir(directory).unwrap();
    }
}
