//! A small external host using only SEMAPRAX's public embedding facade.
//!
//! This is a checkout consumer: the path dependency makes it runnable before
//! the embedding API is published. It intentionally does not access compiler
//! internals, the CLI, or the filesystem.

use semaprax::embedding_api::{check_source, format_source, graph_source, EMBEDDING_API_VERSION};

const SOURCE: &str =
    "module host.demo;\n\n@id(\"host.demo.main\")\nfn main() -> i64\n{\n    42\n}\n";

fn main() {
    assert!(EMBEDDING_API_VERSION.is_compatible_with(1));
    assert!(!EMBEDDING_API_VERSION.is_compatible_with(0));
    assert!(!EMBEDDING_API_VERSION.is_compatible_with(2));

    let checked = check_source("host-demo.spx", SOURCE);
    assert!(
        checked.ok,
        "valid source was rejected: {:?}",
        checked.diagnostics
    );
    assert!(checked.diagnostics.is_empty());
    assert!(checked.revision.is_some());
    assert_eq!(checked.unit_name, "host-demo.spx");

    let formatted = format_source("host-demo.spx", SOURCE);
    assert!(
        formatted.ok,
        "valid source did not format: {:?}",
        formatted.diagnostics
    );
    let canonical = formatted
        .canonical_source
        .expect("successful formatting must return canonical source");
    assert_eq!(
        canonical, SOURCE,
        "formatting returns the exact canonical source"
    );
    assert_eq!(
        format_source("host-demo.spx", &canonical).canonical_source,
        Some(canonical)
    );

    let graphed = graph_source("host-demo.spx", SOURCE);
    assert!(
        graphed.ok,
        "valid source did not graph: {:?}",
        graphed.diagnostics
    );
    let graph = graphed
        .graph_json
        .expect("successful graph rendering must return graph JSON");
    assert!(graph.contains("host.demo.main"));

    let malformed = check_source("host-demo.spx", "module host.demo;\n");
    assert!(!malformed.ok);
    assert_eq!(malformed.diagnostics[0].code, "SPX-P101");

    let warned = check_source(
        "host-demo.spx",
        "module host.demo;\n\nfn main() -> i64\n{\n    42\n}\n",
    );
    assert!(warned.ok);
    assert!(warned
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-S103"));
}
