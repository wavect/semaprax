use std::path::{Path, PathBuf};

use semaprax::{format, graph, hir, parse, verify};

#[test]
fn every_committed_example_is_canonical_and_verified() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut paths = std::fs::read_dir(&root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "spx"))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty());

    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let program = parse(&source, &path).unwrap_or_else(|error| panic!("{error}"));
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics.is_empty(),
            "{} produced diagnostics: {diagnostics:#?}",
            path.display()
        );
        hir::resolve(&program).unwrap_or_else(|diagnostics| {
            panic!("{} did not resolve: {diagnostics:#?}", path.display())
        });
        assert_eq!(
            format::canonical(&program),
            source,
            "{} is not canonical",
            path.display()
        );
    }
}

#[test]
fn contract_graph_matches_exact_snapshot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("examples/meaning.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    let program = parse(&source, &path).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(
        format!("{}\n", graph::to_json(&program).unwrap()),
        include_str!("snapshots/meaning.graph.json")
    );
}

#[test]
fn meaning_revision_matches_the_domain_separated_sha256_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = root.join("examples/meaning.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    let program = parse(&source, &path).unwrap();

    assert_eq!(
        graph::revision(&program),
        "sha256:42aeae2650d15b1e44b8fd6d8a7ce6018d61f43e0e7988a58da2426b2f0c1657"
    );
}

/// Every example in a project subdirectory is canonical too.
///
/// `every_committed_example_is_canonical_and_verified` reads only the top of
/// `examples/`, so a project module could sit in the tree unformatted: this
/// gate found `owned-data-rust/owned_data.spx` written with brace-on-the-same-
/// line bodies. A module inside a manifest closure resolves only against its
/// siblings, so this walk asserts what a single file can own on its own -- it
/// parses, and its text already is its canonical formatting.
#[test]
fn every_example_below_the_top_level_is_canonical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut paths = Vec::new();
    collect_sources(&root, &mut paths);
    paths.retain(|path| path.parent() != Some(root.as_path()));
    paths.sort();
    assert!(
        !paths.is_empty(),
        "the project fixtures under examples/ carry .spx modules"
    );

    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let program = parse(&source, &path).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            format::canonical(&program),
            source,
            "{} is not canonical; run `semaprax fmt` on it",
            path.display()
        );
    }
}

/// Every standard-library source is canonical. Library modules verify only
/// inside their package, which `tests/project.rs::standard_library` proves;
/// this gate keeps the text in the quick `test-advisory` profile so an
/// unformatted `std/` module fails before the project harness is built.
#[test]
fn every_standard_library_source_is_canonical() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("std");
    let mut paths = Vec::new();
    collect_sources(&root, &mut paths);
    paths.sort();
    assert!(!paths.is_empty(), "std/ carries .spx modules");

    for path in paths {
        let source = std::fs::read_to_string(&path).unwrap();
        let program = parse(&source, &path).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            format::canonical(&program),
            source,
            "{} is not canonical; run `semaprax fmt` on it",
            path.display()
        );
    }
}

fn collect_sources(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_sources(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "spx") {
            output.push(path);
        }
    }
}

mod readme_index {
    use std::path::{Path, PathBuf};

    const PREFIX: &str = "examples/";

    fn examples_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples")
    }

    fn readme() -> String {
        std::fs::read_to_string(examples_root().join("README.md")).unwrap()
    }

    fn committed_entry_names() -> Vec<String> {
        let mut names = std::fs::read_dir(examples_root())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| !name.starts_with('.') && name != "README.md")
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    fn is_path_character(character: char) -> bool {
        character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.' | '/')
    }

    /// Every `examples/...` path the README names, with the prefix removed.
    fn mentioned_paths(readme: &str) -> Vec<&str> {
        let mut mentioned = Vec::new();
        let mut rest = readme;
        while let Some(start) = rest.find(PREFIX) {
            rest = &rest[start + PREFIX.len()..];
            if !rest.starts_with(|character: char| character.is_ascii_alphanumeric()) {
                continue;
            }
            let end = rest
                .find(|character: char| !is_path_character(character))
                .unwrap_or(rest.len());
            let mention = rest[..end].trim_end_matches(['.', '/']);
            if !mention.is_empty() {
                mentioned.push(mention);
            }
        }
        mentioned
    }

    /// Every local Markdown link target the README names, anchors removed.
    fn local_link_targets(readme: &str) -> Vec<&str> {
        let mut targets = Vec::new();
        let mut rest = readme;
        while let Some(start) = rest.find("](") {
            rest = &rest[start + 2..];
            let Some(end) = rest.find(')') else { break };
            let target = &rest[..end];
            rest = &rest[end + 1..];
            let target = target.split('#').next().unwrap_or_default();
            if target.is_empty() || target.starts_with("http") || target.starts_with("mailto:") {
                continue;
            }
            targets.push(target);
        }
        targets
    }

    #[test]
    fn the_readme_index_mentions_every_committed_example() {
        let readme = readme();
        let names = committed_entry_names();
        assert!(
            names.len() >= 30,
            "examples/ unexpectedly holds only {} entries: {names:#?}",
            names.len()
        );

        for name in names {
            let mention = format!("{PREFIX}{name}");
            assert!(
                readme.contains(&mention),
                "examples/README.md does not mention `{mention}`; add a row for it"
            );
        }
    }

    #[test]
    fn every_path_and_link_in_the_readme_index_resolves() {
        let root = examples_root();
        let readme = readme();

        let mentioned = mentioned_paths(&readme);
        assert!(
            mentioned.len() >= 30,
            "examples/README.md names only {} paths under {PREFIX}",
            mentioned.len()
        );
        for mention in mentioned {
            assert!(
                root.join(mention).exists(),
                "examples/README.md names `{PREFIX}{mention}`, which is not on disk"
            );
        }

        for target in local_link_targets(&readme) {
            assert!(
                root.join(target).exists(),
                "examples/README.md links to `{target}`, which does not resolve"
            );
        }
    }
}
