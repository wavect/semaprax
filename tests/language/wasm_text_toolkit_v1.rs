//! Additive Wasm text/numeric operation gates; frozen standalone profiles stay separate.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::{parse, verify, wasm};
use std::path::Path;
use std::process::Command;

fn wasm_case(body: &str, expected: &str) {
    let source =
        format!("module test.wasm_conversions;\n@id(\"app.main\") fn main() -> i64 {{ {body} }}\n");
    let program = parse(&source, Path::new("wasm-conversions.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let fixture = Fixture::new(&source);
    let root = fixture.root.join("web");
    wasm::build_web(&program, &root).unwrap();
    let probe = format!(
        r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const {{instance}}=await instantiateBytes(bytes,{{maxOwnedByteEntries:2}});
for(let i=0;i<8;i++) {{ {expected} }}
"#
    );
    std::fs::write(root.join("probe.mjs"), probe).unwrap();
    let output = Command::new("node")
        .arg(root.join("probe.mjs"))
        .current_dir(&root)
        .output()
        .expect("Node is required for additive Wasm conversion parity");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let names = [
        "app.wasm",
        "semaprax.js",
        "index.html",
        "package.json",
        "semaprax.manifest.json",
        "probe.mjs",
    ];
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), names.len());
    for name in names {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
    fixture.cleanup();
}

#[test]
fn wasm_numeric_conversions_round_trip_extrema_and_ties_to_even() {
    for (body, value) in [
        (
            "i64_from_f64(f64_from_i64(9007199254740993))",
            "9007199254740992n",
        ),
        (
            "i64_from_f64(-9223372036854775808.0)",
            "-9223372036854775808n",
        ),
        ("i64_from_f64(-1.9)", "-1n"),
        (
            "i64_from_usize(usize_from_i64(9223372036854775807))",
            "9223372036854775807n",
        ),
    ] {
        wasm_case(body, &format!("if(instance.exports.semaprax_main()!=={value}) throw Error('numeric conversion value changed');"));
    }
}

#[test]
fn wasm_numeric_conversion_failures_keep_their_domain_and_settle_owners() {
    // An authored record selects aggregate lowering for an owned String held
    // across the checked conversion. Repeated failed entries require cleanup.
    for (operation, code) in [
        ("i64_from_f64(9223372036854775808.0)", 1),
        ("i64_from_f64(0.0 / 0.0)", 2),
        ("usize_from_i64(-1)", 1),
        ("i64_from_usize(18446744073709551615usize)", 1),
    ] {
        let body =
            format!("let text = \"held\"; let converted = {operation}; string_len(text) + 0");
        let expected = format!("let failed=false;try{{instance.exports.semaprax_main();}}catch(error){{const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.convert.v1'||status.code!=={code})throw error;failed=true;}}if(!failed)throw Error('missing checked conversion failure');");
        // String-only Web packages retain the frozen scalar route. Adding a
        // harmless record is an explicit aggregate selection, not a bypass.
        let source_body = body;
        let source = format!("module test.wasm_conversion_cleanup;\n@id(\"marker\") record Marker {{ @id(\"marker.code\") code: i64, }}\n@id(\"app.main\") fn main() -> i64 {{ {source_body} }}\n");
        let program = parse(&source, Path::new("conversion-cleanup.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let fixture = Fixture::new(&source);
        let root = fixture.root.join("web");
        wasm::build_web(&program, &root).unwrap();
        std::fs::write(root.join("probe.mjs"), format!("import {{readFile}} from 'node:fs/promises'; import {{instantiateBytes,semanticStatus}} from './semaprax.js'; const {{instance}}=await instantiateBytes(await readFile('./app.wasm'),{{maxOwnedByteEntries:2}});for(let i=0;i<8;i++){{{expected}}}")).unwrap();
        let output = Command::new("node")
            .arg(root.join("probe.mjs"))
            .current_dir(&root)
            .output()
            .expect("Node is required for conversion failure settlement");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        let names = [
            "app.wasm",
            "semaprax.js",
            "index.html",
            "package.json",
            "semaprax.manifest.json",
            "probe.mjs",
        ];
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), names.len());
        for name in names {
            std::fs::remove_file(root.join(name)).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
        fixture.cleanup();
    }
}
