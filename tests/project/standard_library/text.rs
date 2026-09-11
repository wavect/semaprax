//! Probe pinning the exact `SPX-W119` gap (GitHub issue #217) that blocks
//! every byte-inspection `std.text` operation requested by GitHub issue #122
//! (SPX-AI-023).
//!
//! `useful-text-consumer.v1` admission (`src/project/admission/legacy.rs::
//! useful_text`) delegates to `crate::wasm::emit_resolved_module_with_text_exports`,
//! whose `validate_function` (`src/wasm/text_exports.rs`) walks **every**
//! monomorphic function the shared core emitter materializes for the whole
//! linked workspace program, not only the functions actually named in
//! `[exports]` — see the closure-vs-full-inventory comment directly above
//! its own `prepare()` loop. It rejects, module-wide:
//! - any `Match` / `ConstructVariant` / other aggregate-or-variant expression
//!   (`SPX-W119: ... reaches an aggregate or variant expression`), which is
//!   exactly the shape `match byte_get(view, index) { Option::Some { .. } =>
//!   .., Option::None {} => .. }` needs to consume one inspected byte, and
//! - any `while` loop at all (`SPX-W119: ... reaches a loop`), independent of
//!   whether its body touches bytes, `Option`, or any aggregate.
//!
//! Both probes below build a throwaway `useful-text-consumer.v1` package the
//! same way `package_manifest_links_borrowed_text_from_std_text` (in the
//! parent module) builds its own scratch consumer, and put the offending
//! shape in a *private, unexported* helper to demonstrate that the whole
//! linked module is in scope, not merely its declared exports.
//!
//! Because every non-trivial `std.text` scan (ASCII-whitespace trim, blank
//! detection, delimited-field walking, or any other byte-scanning operation
//! GitHub issue #122 asks for) needs at least one of these two shapes, and
//! this validation runs over the *whole linked program* (so even an
//! unreachable helper inside `std/text/src/text.spx` itself would trip it
//! for every consumer that merely depends on the package), GitHub issue
//! #122's implementation intentionally leaves `std/text/src/text.spx`
//! unchanged: there is no scanning operation admitted by the current profile
//! beyond the four already-wrapped compiler-owned `str_*` calls
//! (`str_len_bytes`, `str_is_empty`, `str_starts_with`, `str_contains`; see
//! `src/str_ops.rs::by_id`) that `std.text` already exposes as `byte_len`,
//! `is_empty`, `starts_with`, and `contains`. When GitHub issue #217 closes
//! (either by lowering `Option`/`match` in the text-export emitter, or by
//! narrowing its validation to genuinely exported functions), these two
//! assertions are expected to start failing along with a `check()` call on
//! the same fixture succeeding; that transition is exactly the signal to
//! resume GitHub issue #122's scanning work and land a `std.text`
//! trim/split shape instead of duplicating one from scratch.

use super::{project, temporary};

fn scratch_text_probe(directory: &str, helper: &str) -> std::path::PathBuf {
    let scratch = temporary(directory);
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"text-w119-probe\"\nversion = \"0.1.0\"\nprofile = \"useful-text-consumer.v1\"\n\n[modules]\nentry = \"consumer.text\"\nsources = [\"src/tests.spx\", \"src/text.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.empty\"]\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/text.spx"),
        format!(
            "module consumer.text;\n\n{helper}\n\n@id(\"consumer.empty\")\nfn empty(value: borrow str) -> bool\n{{\n    str_is_empty(value)\n}}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
        ),
    )
    .unwrap();
    scratch
}

/// A private, unexported helper that inspects one byte with the canonical
/// `match byte_get(...)` shape is rejected module-wide, even though it is
/// not named in `[exports]`.
#[test]
fn text_export_profile_rejects_byte_inspection_match_pending_spx_w119() {
    let scratch = scratch_text_probe(
        "text-w119-match",
        "@id(\"consumer.first_byte_is_space\")\nfn first_byte_is_space(value: borrow str) -> bool\n{\n    let bytes = str_as_bytes(value);\n    match byte_get(bytes, 0usize) { Option::Some { value: byte } => byte == 32u8, Option::None {} => false, }\n}",
    );
    let diagnostics =
        project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
            snapshot.check()
        })
        .unwrap_err();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "SPX-W119", "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message,
        "Public Borrowed Text Export Profile v1 function `consumer.first_byte_is_space` reaches an aggregate or variant expression"
    );
    let _ = std::fs::remove_dir_all(scratch);
}

/// A private, unexported helper that loops at all — with no byte inspection,
/// `Option`, or aggregate anywhere in it — is rejected module-wide too: the
/// profile's loop ban is unconditional, not specific to byte scanning.
#[test]
fn text_export_profile_rejects_any_while_loop_pending_spx_w119() {
    let scratch = scratch_text_probe(
        "text-w119-loop",
        "@id(\"consumer.count_up\")\nfn count_up(bound: i64) -> i64\n{\n    let mut index = 0;\n    let mut looping = index < bound;\n    while looping {\n        index = index + 1;\n        looping = index < bound;\n        looping\n    }\n    index\n}",
    );
    let diagnostics =
        project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
            snapshot.check()
        })
        .unwrap_err();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "SPX-W119", "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message,
        "Public Borrowed Text Export Profile v1 function `consumer.count_up` reaches a loop"
    );
    let _ = std::fs::remove_dir_all(scratch);
}
