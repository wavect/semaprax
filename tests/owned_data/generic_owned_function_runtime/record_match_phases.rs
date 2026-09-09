//! Nested matches share an expression ID between entry and result publication.
use super::*;

const SOURCE: &str = r#"module test.record_match_phases;
@id("phase.packet") record Packet {
 @id("phase.packet.data") data: Bytes,
 @id("phase.packet.marker") marker: i64,
}
@id("phase.guard") fn guard(value: own Bytes, allowed: bool)->Bytes
 ensures allowed { value }
@id("phase.combine") fn combine(first: own Packet, second: own Packet, allowed: bool)->Packet {
 match own first { Packet {data: first_data, marker: first_marker} =>
  match own second { Packet {data: second_data, marker: second_marker} =>
   Packet {data: guard(second_data, allowed), marker: first_marker + second_marker},
  },
 }
}
@id("phase.consume") fn consume(value: own Packet)->i64 {
 match own value { Packet {data, marker} =>
  if byte_len(bytes_as_slice(data)) == 2usize {marker} else {0},
 }
}
@id("app.main") fn main()->i64 {
 let one=[1u8];
 let two=[2u8,3u8];
 let first=Packet {data: bytes_copy(array_as_slice(one)), marker: 19};
 let second=Packet {data: bytes_copy(array_as_slice(two)), marker: 23};
 consume(combine(first, second, true))
}
"#;

#[test]
fn nested_record_match_entry_and_result_phases_settle_across_engines() {
    let clang = Command::new("clang").arg("--version").output().is_ok();
    let node = Command::new("node").arg("--version").output().is_ok();
    if std::env::var_os("SEMAPRAX_REQUIRE_GENERIC_OWNED_BACKENDS").is_some() {
        assert!(clang && node);
    }
    if std::env::var_os("SPX_REQUIRE_CLANG").is_some() {
        assert!(clang);
    }
    if std::env::var_os("SPX_REQUIRE_NODE").is_some() {
        assert!(node);
    }
    for allowed in [true, false] {
        let source = SOURCE.replace(
            "combine(first, second, true)",
            &format!("combine(first, second, {allowed})"),
        );
        let parsed = semaprax::check(&source, "record-match-phases.spx").unwrap();
        let canonical = semaprax::format::canonical(&parsed);
        let reparsed = semaprax::check(&canonical, "record-match-phases-canonical.spx").unwrap();
        assert_eq!(canonical, semaprax::format::canonical(&reparsed));
        let resolved = hir::resolve(&parsed).unwrap();
        hir::validate(&resolved).unwrap();
        let graph = semaprax::graph::to_json(&parsed).unwrap();
        semaprax::graph::verify_json(&parsed, &graph).unwrap();
        let expected = if allowed {
            Expected::Value(42)
        } else {
            Expected::Failure("semaprax.contract.v1", 2, "SEMAPRAX contract failure")
        };
        run_interpreter_source("record-match-phases", &source, expected);
        if clang {
            run_native(&parsed, expected);
        }
        if node {
            // Bypass the nested matches in the comparison program: neither
            // match entry nor publication may introduce an aggregate copy.
            let baseline =
                SOURCE.replace("consume(combine(first, second, true))", "consume(second)");
            run_wasm_source(&parsed, &baseline, expected);
        }
    }
}
