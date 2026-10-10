//! JSON escapes and raw UTF-8 are separate scalar pull paths.
use super::*;

pub(super) fn imports() -> String {
    let mut source = String::new();
    for (module, name, alias) in [
        ("query", "scalar_at", "ju_escape_scalar"),
        ("utf8", "scalar_at", "ju_raw_scalar"),
        ("utf8", "sequence_end", "ju_raw_end"),
        ("utf8", "utf8_end", "ju_raw_utf8_end"),
    ] {
        writeln!(source, "use function @id(\"std.data.json.{module}.{name}\") from std.data.json.{module} as {alias};").unwrap();
    }
    source
}

pub(super) fn source(record: &TypeDeclaration, max_string_bytes: usize) -> String {
    include_str!("text.spx")
        .replace("__ROW__", &record.name)
        .replace("__ROW_ID__", &record.stable_id)
        .replace("__BOUND__", &max_string_bytes.to_string())
}

pub(super) fn response_source(record: &TypeDeclaration, bound: usize) -> String {
    // Rewrite template markers before substituting authored names/identities.
    // Decoder helpers are not needed by the response-only source profile.
    let template = include_str!("text.spx");
    let suffix = template
        .split_once("@id(\"__ROW_ID__.json.utf8.owned-valid\")")
        .expect("owned UTF-8 encoder template")
        .1;
    format!("@id(\"__ROW_ID__.json.utf8.owned-valid\"){suffix}")
        .replace("json___ROW___", "json___ROW___response_")
        .replace(".json.utf8.", ".json.collection-response.")
        .replace("__ROW__", &record.name)
        .replace("__ROW_ID__", &record.stable_id)
        .replace("__BOUND__", &bound.to_string())
}

pub(super) fn nested_decode_source(root: &TypeDeclaration, bound: usize) -> String {
    let template = include_str!("text.spx")
        .split_once("@id(\"__ROW_ID__.json.utf8.owned-valid\")")
        .expect("decoder prefix")
        .0;
    let body = template
        .replace(
            "json___ROW___identifier_valid",
            "json___ROW___nested_text_valid",
        )
        .replace("json___ROW___utf8_text", "json___ROW___nested_text")
        .replace(".json.utf8.valid", ".json.nested.text-valid")
        .replace(".json.utf8.materialize-text", ".json.nested.text")
        .replace("__ROW__", &root.name)
        .replace("__ROW_ID__", &root.stable_id)
        .replace("__BOUND__", &bound.to_string());
    format!("{}{body}", imports())
}
