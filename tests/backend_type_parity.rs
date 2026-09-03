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
//! A `no` is not a defect. Several are deliberate profile boundaries: the
//! public Wasm export profile carries a Copy-scalar ABI and leaves aggregates
//! and owned data to the owned-data programme.

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
];

/// The recorded answer per type: does the native backend lower it, does the
/// Wasm backend lower it inside a module, and does the public scalar export
/// profile admit it at the boundary?
const RECORDED: &[(&str, bool, bool, bool)] = &[
    // type        native  wasm module  wasm export
    ("i64", true, true, true),
    ("i32", true, true, true),
    ("u8", true, true, true),
    ("char", true, true, true),
    ("f32", true, true, true),
    ("f64", true, true, true),
    ("bool", true, true, true),
    // `usize` is a checked semantic integer with no host width; the scalar
    // widenings deliberately left it outside the exported Copy-scalar surface.
    ("usize", true, true, false),
    // Owned and borrowed data lower on both backends but carry no export ABI
    // until the owned-data programme provides one.
    ("string", true, false, false),
    ("bytes", true, true, false),
    ("str", true, true, false),
    ("slice_u8", true, true, false),
];

fn program(source: &str) -> String {
    format!("module probe.case;\n\n@id(\"probe.fn\")\n{source}\n\n@id(\"probe.main\")\nfn main() -> i64 {{ 0 }}\n")
}

#[test]
fn every_backend_admits_exactly_the_recorded_types() {
    let recorded: BTreeMap<&str, (bool, bool, bool)> = RECORDED
        .iter()
        .map(|(name, native, module, export)| (*name, (*native, *module, *export)))
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

        let native = codegen::emit_c(&parsed).is_ok();
        let module = wasm::emit_module(&parsed).is_ok();
        let export =
            wasm::emit_module_with_scalar_exports(&parsed, &["probe.fn".to_owned()]).is_ok();

        let (want_native, want_module, want_export) = recorded[name];
        for (path, got, want) in [
            ("native", native, want_native),
            ("wasm module", module, want_module),
            ("wasm export", export, want_export),
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
