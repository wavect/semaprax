use std::path::{Path, PathBuf};

#[test]
fn local_markdown_links_resolve() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut documents = [
        "AGENTS.md",
        "CLAUDE.md",
        "CHANGELOG.md",
        "CONTRIBUTING.md",
        "README.md",
        "SECURITY.md",
    ]
    .into_iter()
    .map(|path| root.join(path))
    .collect::<Vec<_>>();
    collect_markdown(&root.join("docs"), &mut documents);
    documents.sort();

    for document in documents {
        let source = std::fs::read_to_string(&document).unwrap();
        for target in markdown_targets(&source) {
            if target.starts_with("https://")
                || target.starts_with("http://")
                || target.starts_with('#')
                || target.starts_with("mailto:")
            {
                continue;
            }
            let target = target.split('#').next().unwrap();
            if target.is_empty() {
                continue;
            }
            let resolved = document.parent().unwrap().join(target);
            assert!(
                resolved.exists(),
                "{} links to missing local target {target}",
                document.display()
            );
        }
    }
}

#[test]
fn documentation_catalog_and_metadata_are_complete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let docs = root.join("docs");
    let summary = std::fs::read_to_string(docs.join("SUMMARY.md")).unwrap();
    let mut documents = Vec::new();
    collect_markdown(&docs, &mut documents);
    documents.sort();

    for document in documents {
        let source = std::fs::read_to_string(&document).unwrap();
        let mut lines = source.lines();
        let title = lines.next().unwrap_or_default();
        assert!(
            title.starts_with("# "),
            "{} must start with one H1 title",
            document.display()
        );

        let metadata = source.lines().take(12).collect::<Vec<_>>();
        assert!(
            metadata.iter().any(|line| line
                .strip_prefix("- ")
                .unwrap_or(line)
                .starts_with("Status:")),
            "{} must state its status within the first 12 lines",
            document.display()
        );
        assert!(
            metadata.iter().any(|line| line
                .strip_prefix("- ")
                .unwrap_or(line)
                .starts_with("Audience:")),
            "{} must state its audience within the first 12 lines",
            document.display()
        );

        if document
            .file_name()
            .is_some_and(|name| name == "SUMMARY.md")
        {
            continue;
        }
        let relative = document.strip_prefix(&docs).unwrap();
        let catalog_entry = format!("]({})", relative.to_string_lossy().replace('\\', "/"));
        assert!(
            summary.contains(&catalog_entry),
            "{} is missing from docs/SUMMARY.md",
            document.display()
        );
    }
}

fn collect_markdown(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_markdown(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            output.push(path);
        }
    }
}

fn markdown_targets(source: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    let mut remaining = source;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let Some(end) = remaining.find(')') else {
            break;
        };
        targets.push(remaining[..end].trim_matches(['<', '>']));
        remaining = &remaining[end + 1..];
    }
    targets
}
