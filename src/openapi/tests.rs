use super::*;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn write_temp(source: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-openapi-unit-{}-{}.spx",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&path, source).unwrap();
    path
}

fn cleanup(path: &Path) {
    let _ = std::fs::remove_file(path);
}

fn emit(path: &Path, selections: &[&str]) -> String {
    let owned: Vec<String> = selections.iter().map(|name| (*name).to_owned()).collect();
    generate(path, &owned, &OpenApiOptions::default()).expect("fixture generates")
}

fn errors(path: &Path, selections: &[&str]) -> Vec<Diagnostic> {
    let owned: Vec<String> = selections.iter().map(|name| (*name).to_owned()).collect();
    generate(path, &owned, &OpenApiOptions::default()).expect_err("selection must fail closed")
}

/// Four admitted functions whose source order is the exact reverse of their
/// alphabetical order, so any container that emits in insertion order rather
/// than canonical key order gives a different byte string.
const REVERSED: &str = r#"module test.openapi.order;

@id("api.zulu")
fn zulu(second: i64, first: bool) -> i64 { second }

@id("api.yankee")
fn yankee(value: i32) -> i32 { value }

@id("api.bravo")
fn bravo(value: f64) -> f64 { value }

@id("api.alpha")
fn alpha(value: char) -> bool { true }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn offset(envelope: &str, needle: &str) -> usize {
    envelope
        .find(needle)
        .unwrap_or_else(|| panic!("missing `{needle}` in\n{envelope}"))
}

/// "Source formatting, graph JSON, Wasm bytes, diagnostics, semantic patches,
/// and contracted generated artifacts are deterministic."
///
/// The envelope must be a function of the *set* of admitted functions, never
/// of the order the caller listed them in, of how each one was named, or of
/// how many times generation runs. Four selections give a hash-ordered map
/// enough room for an ordering flip to surface.
#[test]
fn the_envelope_depends_on_the_selected_set_and_not_on_selection_order() {
    let path = write_temp(REVERSED);
    let selections = ["api.alpha", "api.bravo", "api.yankee", "api.zulu"];
    let canonical = emit(&path, &selections);

    for _ in 0..4 {
        assert_eq!(
            emit(&path, &selections),
            canonical,
            "repeated generation must be byte-identical"
        );
    }
    assert_eq!(
        emit(&path, &["api.zulu", "api.yankee", "api.bravo", "api.alpha"]),
        canonical,
        "reversing the selection list must not move a byte"
    );
    assert_eq!(
        emit(&path, &["api.yankee", "api.alpha", "api.zulu", "api.bravo"]),
        canonical,
        "an arbitrary selection permutation must not move a byte"
    );
    assert_eq!(
        emit(&path, &["zulu", "api.bravo", "yankee", "api.alpha"]),
        canonical,
        "mixing plain names with stable ids must not move a byte"
    );
    cleanup(&path);
}

/// The document orders its *keys* canonically (sorted, because the JSON object
/// model is ordered) while it keeps its *arrays* in declaration order. Both
/// rules matter and they disagree with each other here, so a change that
/// sorted the arrays or that preserved insertion order for the keys fails.
#[test]
fn object_keys_are_sorted_while_parameter_arrays_stay_in_declared_order() {
    let path = write_temp(REVERSED);
    let envelope = emit(&path, &["api.zulu", "api.yankee", "api.bravo", "api.alpha"]);

    // Path keys and component-schema keys ascend even though the functions
    // were written, and selected, in descending order.
    for keys in [
        [
            "\"/api.alpha\"",
            "\"/api.bravo\"",
            "\"/api.yankee\"",
            "\"/api.zulu\"",
        ],
        [
            "\"api_alpha.Request\"",
            "\"api_bravo.Request\"",
            "\"api_yankee.Request\"",
            "\"api_zulu.Request\"",
        ],
    ] {
        let offsets: Vec<usize> = keys.iter().map(|key| offset(&envelope, key)).collect();
        assert!(
            offsets.windows(2).all(|pair| pair[0] < pair[1]),
            "{keys:?} must appear in ascending key order, got {offsets:?}"
        );
    }

    let value: Value = serde_json::from_str(&envelope).expect("valid JSON");
    let request = &value["document"]["components"]["schemas"]["api_zulu.Request"];
    assert_eq!(
        request["required"],
        serde_json::json!(["second", "first"]),
        "the required array follows the declared parameter order, not the \
alphabetical order of the property keys"
    );
    // ... while the property object those names index into is key-sorted.
    let properties = request["properties"].as_object().expect("properties");
    assert!(
        offset(&envelope, "\"first\":{\"description\"") < offset(&envelope, "\"second\":{"),
        "property keys are sorted even though `second` is declared first"
    );
    assert_eq!(properties.len(), 2);

    cleanup(&path);
}

/// Component names are derived by mapping every character outside
/// `[A-Za-z0-9_]` to `_`, so two distinct identities can land on one name. The
/// schema map would silently keep only the last one, so generation must fail
/// closed instead of publishing a document that has lost an operation.
#[test]
fn identities_that_derive_one_component_name_fail_closed() {
    assert_eq!(derived_name("api.echo"), derived_name("api_echo"));
    let path = write_temp(
        r#"module test.openapi.collide;

@id("api.echo")
fn dotted(value: i64) -> i64 { value }

@id("api_echo")
fn scored(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );

    let colliding = errors(&path, &["api.echo", "api_echo"]);
    assert_eq!(colliding.len(), 1);
    assert_eq!(colliding[0].code, "SPX-OA103");
    assert!(
        colliding[0].message.contains("collide") && colliding[0].message.contains("api_echo"),
        "{}",
        colliding[0].message
    );

    // Either identity alone is perfectly admissible; only the pair collides.
    for selection in ["api.echo", "api_echo"] {
        let envelope = emit(&path, &[selection]);
        let value: Value = serde_json::from_str(&envelope).expect("valid JSON");
        assert_eq!(value["operations"], 1);
        assert_eq!(
            value["document"]["paths"].as_object().expect("paths").len(),
            1
        );
    }
    cleanup(&path);
}

/// The document is an *export* surface: a declaration that was not selected
/// must not appear anywhere in it, not as a path, not as a component, and not
/// as a stray mention inside a description. `main` is the standing example of
/// a declaration that every module has and no caller means to publish.
#[test]
fn unselected_declarations_never_reach_the_document() {
    let path = write_temp(
        r#"module test.openapi.exports;

@id("api.published")
fn published(value: i64) -> i64 { value }

@id("internal.helper")
fn helper(value: i64) -> i64 { value }

@id("app.main")
fn main() -> i64 { helper(published(1)) }
"#,
    );
    let envelope = emit(&path, &["api.published"]);

    for hidden in ["internal.helper", "internal_helper", "app.main", "app_main"] {
        assert!(
            !envelope.contains(hidden),
            "`{hidden}` leaked into the published document:\n{envelope}"
        );
    }
    // `helper` and `main` are still plain function names; make sure the
    // absence above is not an artefact of only checking stable ids.
    assert!(!envelope.contains("SEMAPRAX function helper"));
    assert!(!envelope.contains("SEMAPRAX function main"));

    let value: Value = serde_json::from_str(&envelope).expect("valid JSON");
    assert_eq!(value["operations"], 1);
    assert_eq!(
        value["document"]["paths"]
            .as_object()
            .expect("paths")
            .keys()
            .collect::<Vec<_>>(),
        vec!["/api.published"]
    );
    assert_eq!(
        value["document"]["components"]["schemas"]
            .as_object()
            .expect("schemas")
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            STATUS_COMPONENT_NAME,
            "api_published.Request",
            "api_published.Result",
        ],
        "only the selected operation's components, plus the shared status \
component it references, are published"
    );
    cleanup(&path);
}

/// An identity is an arbitrary string, so it can carry characters that JSON
/// must escape and characters that are not ASCII at all. The envelope must
/// stay parseable and hand the identity back byte-for-byte, while the derived
/// component name stays inside `[A-Za-z0-9_]` — one `_` per character, not per
/// UTF-8 byte, so a multi-byte character does not widen the name.
#[test]
fn identities_needing_escaping_round_trip_and_derive_ascii_component_names() {
    let identity = "api.qu\"o\\te.na\u{ef}ve.\u{65e5}";
    assert_eq!(derived_name(identity), "api_qu_o_te_na_ve__");
    let path = write_temp(
        "module test.openapi.escape;\n\
\n\
@id(\"api.qu\\\"o\\\\te.na\u{ef}ve.\u{65e5}\")\n\
fn probe(value: i64) -> i64 { value }\n\
\n\
@id(\"app.main\")\n\
fn main() -> i64 { 0 }\n",
    );

    let envelope = emit(&path, &[identity]);
    let value: Value = serde_json::from_str(&envelope).expect("escaped envelope is valid JSON");
    let paths = value["document"]["paths"].as_object().expect("paths");
    assert_eq!(
        paths.keys().collect::<Vec<_>>(),
        vec![&format!("/{identity}")],
        "the path key carries the raw identity, escaped only by the serializer"
    );
    assert_eq!(
        paths[&format!("/{identity}")]["post"]["x-stable-id"],
        identity,
        "the identity round-trips through the envelope unchanged"
    );
    assert_eq!(
        paths[&format!("/{identity}")]["post"]["operationId"],
        "api_qu_o_te_na_ve__"
    );
    assert!(value["document"]["components"]["schemas"]["api_qu_o_te_na_ve__.Request"].is_object());

    // The digest is taken over the exact document bytes, so an escaping change
    // that still parsed the same would be caught by the envelope's own binding.
    assert_eq!(
        value["sha256"],
        document_digest(&value["document"]),
        "the payload digest authenticates the escaped document bytes"
    );
    cleanup(&path);
}
