//! The language shapes catalog that `semaprax help shapes` prints: every
//! declaration of every committed example, rendered through the same
//! documentation model as `semaprax doc`, so the bundled reference of admitted
//! shapes cannot drift from what the compiler verifies.

use std::path::{Path, PathBuf};

use semaprax::{doc, verify};

const CATALOG: &str = "docs/LANGUAGE-SHAPES-CATALOG.md";

/// Section heading per entry kind, in the documentation model's order.
const SECTIONS: &[(&str, &str)] = &[
    ("record", "Records"),
    ("variant", "Variants"),
    ("class", "Classes"),
    ("method", "Methods"),
    ("resource", "Resources"),
    ("interface", "Interfaces"),
    ("protocol", "Protocols"),
    ("implementation", "Implementations"),
    ("function", "Functions"),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn examples() -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(root().join("examples"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "spx"))
        .collect();
    paths.sort();
    assert!(paths.len() >= 20, "{}", paths.len());
    paths
}

fn relative(path: &Path) -> String {
    path.strip_prefix(root())
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

/// Render the catalog and return it with the number of declarations it lists.
fn render_catalog() -> (String, usize) {
    let mut documents = Vec::new();
    for path in examples() {
        let source = std::fs::read_to_string(&path).unwrap();
        let (program, comments) = semaprax::parse_with_comments(&source, &path).unwrap();
        assert!(
            !verify::verify(&program)
                .iter()
                .any(|item| item.severity.is_error()),
            "{}",
            path.display()
        );
        documents.push((relative(&path), doc::document(&program, &comments)));
    }
    let mut output = String::from("# Language shapes catalog\n\n");
    output.push_str("Status: generated from `examples/*.spx` through the `semaprax doc` documentation model by `tests/projections.rs::shapes_catalog`; edit the examples, then regenerate with `cargo test --locked -p semaprax --test projections -- --ignored shapes_catalog::regenerate_shapes_catalog`.\n\n");
    output.push_str(
        "Audience: agents and humans writing SEMAPRAX declarations from an installed compiler.\n\n",
    );
    output.push_str("Every shape below is the canonical header of a declaration in a committed, verified example, rendered by the same documentation model as `semaprax doc`, so the catalog cannot show a shape the compiler rejects. `semaprax help shapes` prints this document. The [agent quick reference](AGENT-QUICK-REFERENCE.md) explains the rules behind the shapes, and [Documentation Projection v1](DOC-PROJECTION-V1.md) owns the model. Identities are the examples' own `@id` attributes; bodies are omitted.\n");
    let mut count = 0;
    for (kind, heading) in SECTIONS {
        let mut section = String::new();
        for (path, document) in &documents {
            for entry in document.entries.iter().filter(|entry| entry.kind == *kind) {
                count += 1;
                section.push_str(&format!("\n### `{}` (`{path}`)\n\n", entry.id));
                for line in &entry.description {
                    section.push_str(line);
                    section.push('\n');
                }
                if !entry.description.is_empty() {
                    section.push('\n');
                }
                section.push_str(&format!("```semaprax\n{}```\n", entry.signature));
            }
        }
        if !section.is_empty() {
            output.push_str(&format!("\n## {heading}\n"));
            output.push_str(&section);
        }
    }
    (output, count)
}

#[test]
fn committed_shapes_catalog_matches_the_examples() {
    let (rendered, count) = render_catalog();
    assert!(count >= 100, "{count}");
    let committed = std::fs::read_to_string(root().join(CATALOG)).unwrap();
    assert!(
        committed == rendered,
        "{CATALOG} is stale; regenerate with `cargo test --locked -p semaprax --test projections -- --ignored shapes_catalog::regenerate_shapes_catalog`"
    );
    for heading in [
        "## Records",
        "## Variants",
        "## Classes",
        "## Resources",
        "## Interfaces",
        "## Functions",
    ] {
        assert!(rendered.contains(&format!("\n{heading}\n")), "{heading}");
    }
}

#[test]
#[ignore = "writes the generated catalog; run explicitly after changing examples/"]
fn regenerate_shapes_catalog() {
    let (rendered, _) = render_catalog();
    std::fs::write(root().join(CATALOG), rendered).unwrap();
}
