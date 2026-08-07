use std::path::{Path, PathBuf};

#[test]
fn local_markdown_links_resolve() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut documents = [
        "AGENTS.md",
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
