use super::*;

use std::path::Path;

/// One module whose source order disagrees with alphabetical order in every
/// dimension the projection could accidentally sort on: a function is written
/// before every type, the records descend alphabetically, the class methods
/// descend alphabetically, and every field descends alphabetically.
const DISORDERED: &str = "module test.docorder;\n\
\n\
@id(\"d.zulu_fn\")\n\
fn zulu_fn(value: i64) -> i64\n\
{\n\
    value\n\
}\n\
\n\
@id(\"d.zebra\")\n\
record Zebra {\n\
    @id(\"d.zebra.tail\")\n\
    tail: i64,\n\
    @id(\"d.zebra.head\")\n\
    head: i64,\n\
}\n\
\n\
@id(\"d.alpha\")\n\
class Alpha {\n\
    @id(\"d.alpha.value\")\n\
    value: i64,\n\
\n\
    @id(\"d.alpha.zoom\")\n\
    fn zoom(self: Alpha) -> i64\n\
    {\n\
        self.value\n\
    }\n\
\n\
    @id(\"d.alpha.aim\")\n\
    fn aim(self: Alpha) -> i64\n\
    {\n\
        self.value\n\
    }\n\
}\n\
\n\
@id(\"d.aardvark\")\n\
record Aardvark {\n\
    @id(\"d.aardvark.zed\")\n\
    zed: i64,\n\
    @id(\"d.aardvark.abe\")\n\
    abe: i64,\n\
}\n\
\n\
@id(\"app.main\")\n\
fn main() -> i64\n\
{\n\
    0\n\
}\n";

fn parsed(source: &str) -> (Program, Comments) {
    crate::parse_with_comments(source, Path::new("test.spx")).expect("fixture parses")
}

fn ids(document: &Document) -> Vec<(&str, &str)> {
    document
        .entries
        .iter()
        .map(|entry| (entry.kind, entry.id.as_str()))
        .collect()
}

/// The order of `declarations` is declaration-list order — types (with each
/// class's methods immediately after it), then interfaces, protocols,
/// implementations, and finally free functions — and within each list it is
/// source order. Neither alphabetical order nor a hash order can produce this
/// sequence, so a regression to either fails here.
#[test]
fn declaration_order_is_list_order_then_source_order() {
    let (program, comments) = parsed(DISORDERED);
    let document = document(&program, &comments);
    assert_eq!(
        ids(&document),
        vec![
            ("record", "d.zebra"),
            ("class", "d.alpha"),
            ("method", "d.alpha.zoom"),
            ("method", "d.alpha.aim"),
            ("record", "d.aardvark"),
            ("function", "d.zulu_fn"),
            ("function", "app.main"),
        ],
        "types precede functions even when a function is written first, and \
each list stays in source order"
    );

    let zebra = &document.entries[0];
    assert_eq!(
        zebra
            .members
            .iter()
            .map(|member| member.id.as_str())
            .collect::<Vec<_>>(),
        vec!["d.zebra.tail", "d.zebra.head"],
        "fields keep source order, not alphabetical order"
    );

    // Every method carries the owning class as its first fact, so a reader of
    // the flattened list can still attribute it.
    for index in [2, 3] {
        let method = &document.entries[index];
        assert_eq!(method.facts[0].label, "Owner");
        assert_eq!(method.facts[0].values, vec!["d.alpha".to_owned()]);
    }
}

/// Markdown regroups the same entries under the fixed [`SECTIONS`] headings
/// while keeping source order inside each section. The JSON keeps the
/// flattened list order instead, so the two projections are ordered by
/// different rules and neither may drift into the other's.
#[test]
fn markdown_groups_by_kind_while_json_keeps_list_order() {
    let (program, comments) = parsed(DISORDERED);
    let markdown = markdown(&program, &comments);
    let position = |needle: &str| {
        markdown
            .find(needle)
            .unwrap_or_else(|| panic!("missing `{needle}` in\n{markdown}"))
    };

    let records = position("## Records");
    let classes = position("## Classes");
    let methods = position("## Methods");
    let functions = position("## Functions");
    assert!(
        records < classes && classes < methods && methods < functions,
        "sections follow the canonical kind order"
    );

    assert!(
        position("### `Zebra`") < position("### `Aardvark`"),
        "records stay in source order inside their section"
    );
    assert!(
        position("### `zoom`") < position("### `aim`"),
        "methods stay in source order inside their section"
    );
    assert!(
        position("### `zulu_fn`") < position("### `main`"),
        "functions stay in source order inside their section"
    );
    // The method section is a regrouping: in the flattened list the methods
    // sit between the two records, in Markdown they sit after every class.
    assert!(
        classes < methods,
        "methods are collected after the classes rather than interleaved"
    );

    let json = json(&program, &comments);
    let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    let declarations: Vec<&str> = value["declarations"]
        .as_array()
        .expect("declarations array")
        .iter()
        .map(|declaration| declaration["id"].as_str().expect("id string"))
        .collect();
    assert_eq!(
        declarations,
        vec![
            "d.zebra",
            "d.alpha",
            "d.alpha.zoom",
            "d.alpha.aim",
            "d.aardvark",
            "d.zulu_fn",
            "app.main",
        ],
        "JSON keeps the flattened list order, not the Markdown grouping"
    );
}

/// "Source formatting, graph JSON, ... and contracted generated artifacts are
/// deterministic." A module with seven declarations across four kinds gives a
/// hash-ordered container enough room to flip, so repeated rendering that is
/// not byte-identical fails here.
#[test]
fn both_renderings_are_byte_identical_across_repeated_generation() {
    let (program, comments) = parsed(DISORDERED);
    let first_markdown = markdown(&program, &comments);
    let first_json = json(&program, &comments);
    for _ in 0..8 {
        assert_eq!(markdown(&program, &comments), first_markdown);
        assert_eq!(json(&program, &comments), first_json);
    }

    // A second independent parse of the same bytes must land on the same
    // document, so nothing address- or allocation-dependent leaks in.
    let (reparsed, recomments) = parsed(DISORDERED);
    assert_eq!(
        document(&reparsed, &recomments),
        document(&program, &comments)
    );
    assert_eq!(markdown(&reparsed, &recomments), first_markdown);
    assert_eq!(json(&reparsed, &recomments), first_json);
}

/// Descriptions are arbitrary comment text and identities are arbitrary
/// strings, so the hand-written JSON writer has to escape both. The rendered
/// line must parse and give the text back unchanged, including the non-ASCII
/// characters that must stay raw UTF-8 rather than becoming `\u` escapes.
#[test]
fn json_escapes_comment_and_identity_text_without_altering_it() {
    let source = "module test.docescape;\n\
\n\
// Quotes a \"phrase\", a back\\slash, and na\u{ef}ve \u{65e5}\u{672c}\u{8a9e}.\n\
// Second\tline with a tab.\n\
@id(\"d.esc\\\"aped\")\n\
fn probe(value: i64) -> i64\n\
{\n\
    value\n\
}\n";
    let (program, comments) = parsed(source);
    let document = document(&program, &comments);
    let entry = document
        .entries
        .iter()
        .find(|entry| entry.name == "probe")
        .expect("probe is documented");
    assert_eq!(
        entry.description,
        vec![
            "Quotes a \"phrase\", a back\\slash, and na\u{ef}ve \u{65e5}\u{672c}\u{8a9e}."
                .to_owned(),
            "Second\tline with a tab.".to_owned(),
        ]
    );
    assert_eq!(entry.id, "d.esc\"aped");

    let rendered = json(&program, &comments);
    assert!(
        rendered.contains("na\u{ef}ve \u{65e5}\u{672c}\u{8a9e}"),
        "non-ASCII text stays raw UTF-8 rather than being \\u-escaped"
    );
    let value: serde_json::Value =
        serde_json::from_str(&rendered).expect("escaped output is still valid JSON");
    let declaration = value["declarations"]
        .as_array()
        .expect("declarations array")
        .iter()
        .find(|declaration| declaration["name"] == "probe")
        .expect("probe declaration");
    assert_eq!(declaration["id"], "d.esc\"aped");
    assert_eq!(
        declaration["description"][0],
        "Quotes a \"phrase\", a back\\slash, and na\u{ef}ve \u{65e5}\u{672c}\u{8a9e}."
    );
    assert_eq!(declaration["description"][1], "Second\tline with a tab.");
    // The signature re-emits the identity in source syntax, so it carries the
    // escape twice over: once for `.spx`, once for JSON.
    assert_eq!(
        declaration["signature"]
            .as_str()
            .expect("signature string")
            .lines()
            .next(),
        Some("@id(\"d.esc\\\"aped\")")
    );
}

/// The smallest module the grammar admits — one nullary function, no types,
/// no permits, no uses, no comments — still renders a complete document on
/// both projections. Every list the renderers walk is empty here, so a
/// regression that indexes or unwraps one of them fails, and a regression
/// that emits an unconditional section heading or a stray separator shows up
/// as extra bytes.
#[test]
fn the_smallest_admissible_module_still_renders_a_complete_document() {
    assert_eq!(
        crate::parse_with_comments("module test.docempty;\n", Path::new("test.spx"))
            .expect_err("a declaration-free module is rejected by the grammar")
            .code,
        "SPX-P101",
        "the minimum documented module is one function, not zero declarations"
    );

    let source = "module test.docempty;\n\nfn main() -> i64\n{\n    0\n}\n";
    let (program, comments) = parsed(source);
    let document = document(&program, &comments);
    assert!(document.permits.is_empty());
    assert!(document.uses.is_empty());
    assert_eq!(document.revision, graph::revision(&program));
    assert_eq!(document.entries.len(), 1);
    let entry = &document.entries[0];
    assert_eq!(entry.kind, "function");
    assert!(entry.description.is_empty());
    assert!(entry.members.is_empty());
    assert!(
        !entry.persistent,
        "an automatic identity is revision-scoped, not persistent"
    );
    assert_eq!(
        entry
            .facts
            .iter()
            .map(|fact| fact.label)
            .collect::<Vec<_>>(),
        vec!["Returns"],
        "empty fact lists are dropped rather than emitted as empty labels"
    );

    let markdown = markdown(&program, &comments);
    assert!(markdown.starts_with("# Module `test.docempty`\n"));
    assert!(markdown.contains(&format!("- Graph revision: `{}`", document.revision)));
    assert!(
        !markdown.contains("- Permits:"),
        "no permit line when the module declares none"
    );
    for (kind, heading) in SECTIONS {
        assert_eq!(
            markdown.contains(&format!("## {heading}")),
            *kind == "function",
            "only the sections with entries get a heading"
        );
    }
    assert!(
        markdown.contains(&format!(
            "- Identity: `{}` (automatic, revision-scoped)",
            entry.id
        )),
        "{markdown}"
    );

    let rendered = json(&program, &comments);
    assert!(rendered.ends_with('\n'));
    assert_eq!(rendered.matches('\n').count(), 1);
    let value: serde_json::Value =
        serde_json::from_str(&rendered).expect("minimal document is still valid JSON");
    assert_eq!(value["schema"], SCHEMA_V1);
    assert_eq!(value["module"], "test.docempty");
    assert_eq!(value["revision"], document.revision);
    assert!(value["permits"].as_array().expect("permits").is_empty());
    assert!(value["uses"].as_array().expect("uses").is_empty());
    let declarations = value["declarations"].as_array().expect("declarations");
    assert_eq!(declarations.len(), 1);
    assert_eq!(declarations[0]["persistent"], false);
    assert!(declarations[0]["description"]
        .as_array()
        .expect("description")
        .is_empty());
    assert!(declarations[0]["members"]
        .as_array()
        .expect("members")
        .is_empty());
}
