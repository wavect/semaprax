//! Which types each backend admits, pinned.
//!
//! `AGENTS.md` requires equivalent checked behavior on every backend that
//! claims to implement an admitted feature, so what each backend claims has to
//! be legible. It was not: the answer lived in scattered admission predicates,
//! and the only way to learn that a type stopped short of a backend was to try
//! it. This pins the whole surface — every admitted type against every lowering
//! path — so a narrowing fails here, and a widening has to be recorded
//! deliberately rather than passing unnoticed.
//!
//! A `no` in one export column is not "unavailable". SEMAPRAX has several
//! public Wasm export profiles, each with its own ABI, and a type reaches the
//! boundary through the profile built for it: Copy scalars through the scalar
//! profile's direct adapters, `borrow str` through the borrowed-text profile,
//! `borrow Slice<u8>` and `usize` results through the useful-data profile's
//! scratch/status ABI, and owned `Bytes` and `string` through the descriptor-
//! driven owned-data and owned-UTF-8 project profiles, which need a Project
//! subject and so are exercised by the project harnesses rather than here.
//! Position matters as much as the type: the same type can be admitted as a
//! borrowed parameter and refused as a result. The rows below therefore pin
//! one exact probe shape each, not a claim about the type in every position.

use std::collections::BTreeMap;
use std::path::Path;

use semaprax::{codegen, parse, wasm};

/// One probe per admitted type shape: a function that takes the type and
/// returns something, so a backend must lower the parameter, the body, and the
/// result rather than merely parse a declaration.
const PROBES: &[(&str, &str)] = &[
    ("i64", "fn probe(v: i64) -> i64 { v + 1 }"),
    ("i32", "fn probe(v: i32) -> i32 { v + 1i32 }"),
    ("u8", "fn probe(v: u8) -> bool { v == 1u8 }"),
    ("char", "fn probe(v: char) -> bool { v == 'S' }"),
    ("f32", "fn probe(v: f32) -> f32 { v }"),
    ("f64", "fn probe(v: f64) -> f64 { v }"),
    (
        "bool",
        "fn probe(v: bool) -> bool { if v { false } else { true } }",
    ),
    ("usize", "fn probe(v: usize) -> bool { v == 1usize }"),
    (
        "string",
        "fn probe(l: string, r: string) -> string { string_concat(l, r) }",
    ),
    (
        "bytes",
        "fn probe(v: borrow Bytes) -> usize { byte_len(bytes_as_slice(v)) }",
    ),
    (
        "str",
        "fn probe(v: borrow str) -> usize { byte_len(str_as_bytes(v)) }",
    ),
    (
        "slice_u8",
        "fn probe(v: borrow Slice<u8>) -> usize { byte_len(v) }",
    ),
    // The same `str` in the position the borrowed-text profile is built for:
    // a borrowed view in, a scalar out. The `str` row above returns `usize`,
    // which that profile does not carry, so it isolates position from type.
    ("str borrowed", "fn probe(v: borrow str) -> i64 { 0 }"),
];

/// The recorded answer per probe: does the native backend lower it, does the
/// Wasm backend lower it inside a module, and which public Wasm export profile
/// admits it at the boundary — the Copy-scalar profile, the borrowed-text
/// profile, or the useful-data profile.
const RECORDED: &[(&str, bool, bool, bool, bool, bool)] = &[
    // type          native module scalar text data
    ("i64", true, true, true, false, false),
    ("i32", true, true, true, false, false),
    ("u8", true, true, true, false, false),
    ("char", true, true, true, false, false),
    ("f32", true, true, true, false, false),
    ("f64", true, true, true, false, false),
    ("bool", true, true, true, false, false),
    // `usize` is a checked semantic integer with no host width, so the scalar
    // widenings deliberately left it out of the exported Copy-scalar ABI. It
    // still reaches the boundary as a useful-data *result* — see `slice_u8`,
    // whose probe returns one — but not as a by-value scalar parameter.
    ("usize", true, true, false, false, false),
    // Owned and borrowed data lower on both backends. `string` lowers into a
    // core module here; it is the legacy Web *package* wrapper that rejects
    // it, for want of the String runtime imports, not the backend. Owned
    // `string` and `Bytes` results reach the boundary through the descriptor-
    // driven owned-UTF-8 and owned-data project profiles, which need a Project
    // subject and are gated by the project harnesses.
    ("string", true, true, false, false, false),
    ("bytes", true, true, false, false, false),
    // `str` in this shape returns `usize`, which the borrowed-text profile
    // does not carry; `str borrowed` below is the same type in the shape that
    // profile does admit.
    ("str", true, true, false, false, false),
    // A borrowed byte view in and a `usize` out: exactly the useful-data ABI.
    ("slice_u8", true, true, false, false, true),
    ("str borrowed", true, true, false, true, false),
];

fn program(source: &str) -> String {
    format!("module probe.case;\n\n@id(\"probe.fn\")\n{source}\n\n@id(\"probe.main\")\nfn main() -> i64 {{ 0 }}\n")
}

#[test]
fn every_backend_admits_exactly_the_recorded_types() {
    let recorded: BTreeMap<&str, (bool, bool, bool, bool, bool)> = RECORDED
        .iter()
        .map(|(name, native, module, scalar, text, data)| {
            (*name, (*native, *module, *scalar, *text, *data))
        })
        .collect();
    assert_eq!(
        recorded.len(),
        PROBES.len(),
        "every probe needs exactly one recorded row"
    );

    let mut drift = Vec::new();
    for (name, source) in PROBES {
        let text = program(source);
        let parsed = parse(&text, Path::new("probe.spx"))
            .unwrap_or_else(|error| panic!("probe `{name}` must parse and verify: {error:?}"));

        let selected = ["probe.fn".to_owned()];
        let native = codegen::emit_c(&parsed).is_ok();
        let module = wasm::emit_module(&parsed).is_ok();
        let scalar = wasm::emit_module_with_scalar_exports(&parsed, &selected).is_ok();
        let text = wasm::emit_module_with_text_exports(&parsed, &selected).is_ok();
        let data = wasm::emit_module_with_byte_exports(&parsed, &selected).is_ok();

        let (want_native, want_module, want_scalar, want_text, want_data) = recorded[name];
        for (path, got, want) in [
            ("native", native, want_native),
            ("wasm module", module, want_module),
            ("scalar export", scalar, want_scalar),
            ("borrowed-text export", text, want_text),
            ("useful-data export", data, want_data),
        ] {
            if got != want {
                drift.push(format!(
                    "  {name} on {path}: recorded {}, now {}",
                    if want { "admitted" } else { "rejected" },
                    if got { "admitted" } else { "rejected" }
                ));
            }
        }
    }

    assert!(
        drift.is_empty(),
        "backend type admission changed. A newly rejected type is a regression in \
         cross-backend equivalence. A newly admitted one is progress, but update RECORDED \
         in the same change so the surface each backend claims stays exact:\n{}",
        drift.join("\n")
    );
}
