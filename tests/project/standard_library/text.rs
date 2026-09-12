//! Probes for the `SPX-W119` admission gap tracked as GitHub issue #217,
//! which blocks every byte-inspection `std.text` operation requested by
//! GitHub issue #122 (SPX-AI-023).
//!
//! `useful-text-consumer.v1` admission (`src/project/admission/legacy.rs::
//! useful_text`) delegates to `crate::wasm::emit_resolved_module_with_text_exports`,
//! which first calls `text_exports::prepare` (`src/wasm/text_exports.rs`).
//! Before this fix, `prepare`'s `validate_function` walked **every**
//! monomorphic function the shared core emitter materializes for the whole
//! linked workspace program, not only the closure reachable from the
//! functions actually named in `[exports]`. That meant a private,
//! unexported helper using a shape the profile forbids — a `match` on the
//! `Option` that `byte_get` returns, or an unconditional `while` loop —
//! broke admission for every consumer that merely depended on a package
//! containing it, even though such a helper is never compiled into any call
//! this profile's raw wrappers can reach.
//!
//! `prepare` now scopes both validation and recursion-cycle detection to the
//! closure actually reachable from `[exports]`, discovered transitively
//! through `validate_function`'s own callee walk (see the closure loop
//! directly above `reject_call_cycles`'s call site in `prepare`). The tests
//! below confirm the two different outcomes that follow from that closure
//! being sound:
//!
//! - [`text_export_profile_admits_an_unreachable_scalar_while_loop`] -- a
//!   private, unexported `while` loop over plain `i64`/`bool` locals is now
//!   admitted, because the shared scalar-core Wasm emitter (`emit_expr` in
//!   `src/wasm.rs`, "Bounded While-Loops v1") already lowers such a loop
//!   correctly and nothing else in the pipeline forbids it once it is no
//!   longer reachable-irrelevant code the closure walk wrongly charged to
//!   every consumer.
//! - [`text_export_profile_still_rejects_an_unreachable_byte_inspection_match`]:
//!   a private, unexported `match byte_get(...) { Option::Some { .. } => ..,
//!   Option::None {} => .. }` helper no longer trips `text_exports.rs`'s own
//!   `SPX-W119` check (that check is scoped to the closure now, and this
//!   helper is not in it), but admission still fails, with a **different**
//!   code, `SPX-W115`. That comes from a second, independent whole-program
//!   scan in `emit_resolved_module_internal` (`src/wasm.rs`): it builds a
//!   `VariantLayoutCache` over the *entire* linked `ResolvedProgram` — not
//!   the profile's reachable closure — and unconditionally rejects any
//!   public-profile module (scalar or text) that contains a concrete variant
//!   instantiation anywhere in that program. `Option<u8>` from the
//!   unreachable helper is exactly such an instantiation. `src/wasm.rs` is
//!   outside `text_exports.rs`'s ownership (owned by parallel Wasm
//!   backend-parity work on owned-record collections at the time of this
//!   fix), so narrowing *that* scan to the reachable closure — the change
//!   that would let an unreachable `std.text` helper coexist with an
//!   unrelated consumer's `Option`-free export — is left as follow-up work;
//!   this test pins the residual gap's exact, now-correct diagnostic so a
//!   regression is caught either way.
//! - [`text_export_profile_rejects_a_reachable_byte_inspection_match`] -- a
//!   `match` on `Option` reached directly from a declared export is, and
//!   must remain, rejected: no Wasm lowering exists anywhere in this profile
//!   for a variant scrutinee (`src/wasm.rs::emit_expr` only lowers
//!   `Refutable Match v1` for `Copy`-scalar scrutinees and explicitly
//!   returns `SPX-W110` for anything else), so admitting it here would be a
//!   silent-miscompilation risk, not merely an over-strict diagnostic. This
//!   keeps `text_exports.rs`'s own `SPX-W119` check — and its exact message
//!   — stable for the one case it must still cover.
//!
//! Because `std.text` byte-scanning still has no admissible `match`/`Option`
//! path for a genuinely exported function, GitHub issue #122's
//! implementation continues to leave `std/text/src/text.spx` unchanged:
//! there is no scanning operation admitted by the current profile beyond the
//! four already-wrapped compiler-owned `str_*` calls (`str_len_bytes`,
//! `str_is_empty`, `str_starts_with`, `str_contains`; see
//! `src/str_ops.rs::by_id`) that `std.text` already exposes as `byte_len`,
//! `is_empty`, `starts_with`, and `contains`.

use super::{project, temporary};

fn scratch_text_probe(directory: &str, helper: &str) -> std::path::PathBuf {
    scratch_text_probe_with_export_body(directory, helper, "str_is_empty(value)")
}

fn scratch_text_probe_with_export_body(
    directory: &str,
    helper: &str,
    export_body: &str,
) -> std::path::PathBuf {
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
    let helper_block = if helper.is_empty() {
        String::new()
    } else {
        format!("{helper}\n\n")
    };
    std::fs::write(
        scratch.join("src/text.spx"),
        format!(
            "module consumer.text;\n\n{helper_block}@id(\"consumer.empty\")\nfn empty(value: borrow str) -> bool\n{{\n    {export_body}\n}}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
        ),
    )
    .unwrap();
    scratch
}

const BYTE_INSPECTION_MATCH_HELPER: &str = "@id(\"consumer.first_byte_is_space\")\nfn first_byte_is_space(value: borrow str) -> bool\n{\n    let bytes = str_as_bytes(value);\n    match byte_get(bytes, 0usize) { Option::Some { value: byte } => byte == 32u8, Option::None {} => false, }\n}";

/// A private, unexported helper that loops at all — with no byte inspection,
/// `Option`, or aggregate anywhere in it — no longer breaks admission for a
/// consumer that merely depends on the package containing it: the closure
/// this profile actually validates is scoped to what `[exports]` reaches,
/// and the shared scalar-core emitter already lowers a plain scalar `while`
/// loop correctly.
#[test]
fn text_export_profile_admits_an_unreachable_scalar_while_loop() {
    let scratch = scratch_text_probe(
        "text-w119-loop",
        "@id(\"consumer.count_up\")\nfn count_up(bound: i64) -> i64\n{\n    let mut index = 0;\n    let mut looping = index < bound;\n    while looping {\n        index = index + 1;\n        looping = index < bound;\n        looping\n    }\n    index\n}",
    );
    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

/// A private, unexported `match byte_get(...)` helper no longer trips
/// `text_exports.rs`'s own closure-scoped `SPX-W119` check, but admission
/// still fails: a second, whole-program `VariantLayoutCache` scan in
/// `src/wasm.rs::emit_resolved_module_internal` (outside this file's
/// ownership) rejects any public-profile module containing a concrete
/// variant instantiation anywhere in the linked program, reachable or not.
/// This pins that residual gap's current, correct diagnostic.
#[test]
fn text_export_profile_still_rejects_an_unreachable_byte_inspection_match() {
    let scratch = scratch_text_probe("text-w119-match-unreachable", BYTE_INSPECTION_MATCH_HELPER);
    let diagnostics =
        project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
            snapshot.check()
        })
        .unwrap_err();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "SPX-W115", "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message,
        "Public Scalar Export Profile v1 does not admit aggregate or variant lowering"
    );
    let _ = std::fs::remove_dir_all(scratch);
}

/// A `match` on the `Option` `byte_get` returns, reached directly from a
/// declared export, must stay rejected: `src/wasm.rs::emit_expr` has no
/// lowering for a variant-scrutinee match (it lowers `Refutable Match v1`
/// for `Copy`-scalar scrutinees only and returns `SPX-W110` for anything
/// else), so admitting this shape would risk a silent miscompilation, not
/// merely an over-strict diagnostic. `text_exports.rs`'s own check catches
/// it first and keeps the original, stable `SPX-W119` diagnostic.
#[test]
fn text_export_profile_rejects_a_reachable_byte_inspection_match() {
    let scratch = scratch_text_probe_with_export_body(
        "text-w119-match-reachable",
        "",
        "let bytes = str_as_bytes(value);\n    match byte_get(bytes, 0usize) { Option::Some { value: byte } => byte == 32u8, Option::None {} => false, }",
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
        "Public Borrowed Text Export Profile v1 function `consumer.empty` reaches an aggregate or variant expression"
    );
    let _ = std::fs::remove_dir_all(scratch);
}
