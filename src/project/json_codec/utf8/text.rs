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
