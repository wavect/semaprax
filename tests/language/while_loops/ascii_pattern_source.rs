//! Private source witnesses; no package registration or completion claim.
use semaprax::interpreter::InterpreterOptions;
use semaprax::{format, graph, hir, interpreter, parse};
use sha2::{Digest as _, Sha256};

const ENGINE: &str = include_str!("../../../experiments/ascii-pattern-source/ascii.spx");
const COMPILED: &str =
    include_str!("../../../experiments/ascii-pattern-source/compiled-witnesses.json");
const CASES: &str = include_str!("../../../experiments/ascii-pattern-source/fixtures/cases.json");

fn bytes(name: &str, value: &[u8]) -> String {
    format!(
        "let {name}: [u8; {}] = [{}];\nlet {name}_view = array_as_slice({name});\n",
        value.len(),
        value
            .iter()
            .map(|v| format!("{v}u8"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn input_source(value: &[u8]) -> String {
    if value.len() >= 1024 && value.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
        let mut escaped = String::with_capacity(value.len());
        for byte in value {
            match byte {
                b'\\' => escaped.push_str("\\\\"),
                b'"' => escaped.push_str("\\\""),
                _ => escaped.push(char::from(*byte)),
            }
        }
        return format!(
            "let input_text = \"{escaped}\";\nlet input_str = string_as_str(input_text);\nlet input_view = str_as_bytes(input_str);\n"
        );
    }
    bytes("input", value)
}

fn expect_packet(
    status: u64,
    spans: &[[u64; 2]],
    work: Option<u64>,
    reason: u64,
    detail: u64,
) -> String {
    let mut checks = vec![
        format!("status(matcher) == {status}usize"),
        format!("capture_count(matcher) == {}usize", spans.len()),
        format!("read(matcher, 2703usize) == {reason}usize"),
        format!("load8(matcher, 2720usize) == {detail}usize"),
    ];
    if let Some(work) = work {
        checks.push(format!("work_used(matcher) == {work}usize"));
    }
    for (index, [start, end]) in spans.iter().enumerate() {
        checks.push(format!(
            "capture_start(matcher, {index}usize) == {start}usize"
        ));
        checks.push(format!("capture_end(matcher, {index}usize) == {end}usize"));
    }
    format!("if {} {{ 1 }} else {{ -1 }}", checks.join(" && "))
}

fn compile_source(pattern: &[u8], input: &[u8], expected: &str) -> String {
    compile_source_with_limit(pattern, input, expected, 8192)
}

fn compile_source_with_limit(
    pattern: &[u8],
    input: &[u8],
    expected: &str,
    work_limit: u64,
) -> String {
    format!("{ENGINE}\n@id(\"experiment.pattern.witness.main\")\nfn main() -> i64\n{{\n{}{}\nlet storage = bytes_zeroed(3072usize);\nlet initial = matcher_from_bytes(storage);\nlet compiled = compile(initial, pattern_view, {work_limit}usize);\nlet ready = status(compiled) == 0usize;\nlet mut matcher = compiled;\nlet mut iteration = 0usize;\nwhile iteration < 1usize {{\nmatcher = full_match(matcher, input_view, {work_limit}usize);\niteration = iteration + 1usize;\n0\n}}\nif ready {{ {expected} }} else {{ -2 }}\n}}\n", bytes("pattern", pattern), input_source(input))
}

fn fixture_pattern(value: &serde_json::Value) -> Vec<u8> {
    if let Some(text) = value.as_str() {
        return text.as_bytes().to_vec();
    }
    let repeat = value["repeat"].as_str().expect("pattern repeat text");
    let count = value["count"].as_u64().expect("pattern repeat count") as usize;
    let suffix = value["suffix"].as_str().unwrap_or_default();
    format!("{}{suffix}", repeat.repeat(count)).into_bytes()
}

fn fixture_input(value: &serde_json::Value) -> Vec<u8> {
    if let Some(text) = value["ascii"].as_str() {
        return text.as_bytes().to_vec();
    }
    if let Some(text) = value["hex"].as_str() {
        return (0..text.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
            .collect();
    }
    if let Some(repeat) = value.get("repeat_ascii") {
        return repeat["text"]
            .as_str()
            .unwrap()
            .repeat(repeat["count"].as_u64().unwrap() as usize)
            .into_bytes();
    }
    if let Some(repeat) = value.get("repeat_ascii_suffix") {
        return format!(
            "{}{}",
            repeat["text"]
                .as_str()
                .unwrap()
                .repeat(repeat["count"].as_u64().unwrap() as usize),
            repeat["suffix"].as_str().unwrap()
        )
        .into_bytes();
    }
    let repeat = &value["repeat_hex"];
    let byte = u8::from_str_radix(repeat["byte"].as_str().unwrap(), 16).unwrap();
    vec![byte; repeat["count"].as_u64().unwrap() as usize]
}

fn interpret(source: &str, stem: &str) {
    let program =
        semaprax::check(source, "ascii-private-witness.spx").expect("private source verifies");
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
    let canonical = format::canonical(&program);
    assert_eq!(
        format::canonical(&parse(&canonical, "canonical.spx").unwrap()),
        canonical
    );
    let json = graph::to_json(&program).unwrap();
    graph::verify_json(&program, &json).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "semaprax-private-ascii-{}-{stem}-{}",
        std::process::id(),
        super::NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("witness.spx");
    std::fs::write(&path, source).unwrap();
    let result = interpreter::internal_strings::interpret(
        &path,
        "experiment.pattern.witness.main",
        &[],
        &InterpreterOptions::default(),
    )
    .unwrap();
    let document: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert!(
        result.returned,
        "ordinary fuel must return: {}",
        result.envelope
    );
    assert_eq!(
        document["payload"]["outcome"]["value"], "1",
        "{}",
        result.envelope
    );
    std::fs::remove_file(&path).unwrap();
    std::fs::remove_dir(path.parent().unwrap()).unwrap();
}

#[test]
fn private_ascii_pattern_compile_and_greedy_capture_witnesses() {
    for (index, (pattern, input, spans)) in [
        ("(a*)(a*)", "aa", vec![[0, 2], [2, 2]]),
        ("([0-9]+)[0-9]", "123", vec![[0, 2]]),
        (
            "([A-Za-z-]+):[ \\x09]*([^\\x0D\\x0A]*)",
            "X-Key: value",
            vec![[0, 5], [7, 12]],
        ),
        (
            "([A-Za-z_][A-Za-z0-9_]*)=([^\\x00\\x0A]*)",
            "event_id=abc123",
            vec![[0, 8], [9, 15]],
        ),
        (
            "([0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2})\\x2B([0-9]{2}):([0-9]{2})",
            "2026-10-09T07:00:00+02:00",
            vec![[0, 19], [20, 22], [23, 25]],
        ),
        ("()(a*)()", "a", vec![[0, 0], [0, 1], [1, 1]]),
        ("(\\xC3).", "\u{00e9}", vec![[0, 1]]),
    ]
    .into_iter()
    .enumerate()
    {
        let expected = expect_packet(1, &spans, None, 0, 0);
        interpret(
            &compile_source(pattern.as_bytes(), input.as_bytes(), &expected),
            &format!("ascii-capture-{index}"),
        );
    }
    let expected = expect_packet(2, &[], None, 0, 0);
    for (index, (pattern, input)) in [
        (b"a\\+[0-9]+".as_slice(), b"a-12".as_slice()),
        (
            b"([0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2})\\x2B([0-9]{2}):([0-9]{2})"
                .as_slice(),
            b"2026-10-09T07:00:00-02:00".as_slice(),
        ),
        (
            b"([A-Za-z-]+):[ \\x09]*([^\\x0D\\x0A]*)".as_slice(),
            b"X-Key: value\r\n".as_slice(),
        ),
        (
            b"([A-Za-z_][A-Za-z0-9_]*)=([^\\x00\\x0A]*)".as_slice(),
            b"event_id=bad\0".as_slice(),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        interpret(
            &compile_source(pattern, input, &expected),
            &format!("ascii-nonmatch-{index}"),
        );
    }
    let expected = "if read(matcher, 7usize) == 1usize { 1 } else { -1 }";
    interpret(
        &compile_source(b"[a-c][abc]", b"ab", expected),
        "ascii-class-dedup",
    );
}

#[test]
fn private_ascii_pattern_fixture_cases_match_independent_exhaustive_oracle() {
    let fixtures: serde_json::Value = serde_json::from_str(CASES).unwrap();
    let mut count = 0usize;
    for case in fixtures["cases"].as_array().unwrap() {
        if case["source_differential"] != true {
            continue;
        }
        let expected_match = &case["expected"]["match"];
        let status = expected_match["status_code"].as_u64().unwrap();
        assert!(
            matches!(status, 1 | 2),
            "{} is semantic, not a refusal",
            case["id"]
        );
        let spans: Vec<[u64; 2]> = expected_match["spans"]
            .as_array()
            .unwrap()
            .iter()
            .map(|span| [span[0].as_u64().unwrap(), span[1].as_u64().unwrap()])
            .collect();
        let expected = expect_packet(status, &spans, None, 0, 0);
        let source = compile_source_with_limit(
            &fixture_pattern(&case["pattern"]),
            &fixture_input(&case["input"]),
            &expected,
            262_144,
        );
        interpret(&source, case["id"].as_str().unwrap());
        count += 1;
    }
    assert!(
        count >= 9,
        "fixture-driven source comparison stays populated"
    );
}

#[test]
fn private_ascii_pattern_malformed_offsets_and_compile_invalidation() {
    for (index, (pattern, offset)) in [
        (b"\\xG0".as_slice(), 2),
        (b"a{01}".as_slice(), 3),
        (b"[z-a]".as_slice(), 3),
        (b"[]".as_slice(), 1),
        (b"a|b".as_slice(), 1),
        (b"(a".as_slice(), 2),
        (b"(a)*".as_slice(), 3),
        (b"a**".as_slice(), 2),
    ]
    .into_iter()
    .enumerate()
    {
        let source = format!("{ENGINE}\n@id(\"experiment.pattern.witness.main\")\nfn main() -> i64\n{{\n{}\nlet storage = bytes_zeroed(3072usize);\nlet initial = matcher_from_bytes(storage);\nlet matcher = compile(initial, pattern_view, 8192usize);\n{}\n}}", bytes("pattern", pattern), expect_packet(3, &[], None, 1, offset));
        interpret(&source, &format!("ascii-invalid-{index}"));
    }
    let source = format!("{ENGINE}\n@id(\"experiment.pattern.witness.main\")\nfn main() -> i64\n{{\n{}{}{}\nlet storage = bytes_zeroed(3072usize);\nlet initial = matcher_from_bytes(storage);\nlet good_matcher = compile(initial, good_view, 8192usize);\nlet bad_matcher = compile(good_matcher, bad_view, 33usize);\nlet refused = status(bad_matcher) == 4usize && work_used(bad_matcher) == 33usize && read(bad_matcher, 5usize) == 0usize;\nlet matcher = full_match(bad_matcher, input_view, 8192usize);\nif refused {{ {} }} else {{ -2 }}\n}}", bytes("good", b"a"), bytes("bad", b"b"), bytes("input", b"a"), expect_packet(3, &[], Some(44), 2, 5));
    interpret(&source, "ascii-compile-invalidates");
}

#[test]
fn private_ascii_pattern_precompiled_table_and_exact_work_witnesses() {
    let fixtures: serde_json::Value = serde_json::from_str(COMPILED).unwrap();
    for case in fixtures["cases"].as_array().unwrap() {
        let mut storage = vec![0u8; 3072];
        for segment in case["segments"].as_array().unwrap() {
            let offset = segment["offset"].as_u64().unwrap() as usize;
            let hex = segment["hex"].as_str().unwrap();
            for (index, pair) in hex.as_bytes().chunks_exact(2).enumerate() {
                storage[offset + index] =
                    u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
            }
        }
        assert_eq!(
            format!(
                "{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(&storage))
            ),
            case["compiled_sha256"].as_str().unwrap()
        );
        let mut seed = String::new();
        for (index, value) in storage.iter().enumerate().filter(|(_, value)| **value != 0) {
            // Chained source bindings transfer one whole owner; zero gaps need
            // no stores. The binding ordinal, rather than byte index, links them.
            let ordinal = seed.lines().count();
            let previous = if ordinal == 0 {
                "initial".to_owned()
            } else {
                format!("seed{}", ordinal - 1)
            };
            seed.push_str(&format!(
                "let seed{ordinal} = store1({previous}, {index}usize, {value}usize);\n"
            ));
        }
        let last = if seed.is_empty() {
            "initial".to_owned()
        } else {
            format!("seed{}", seed.lines().count() - 1)
        };
        let expected = &case["expected"];
        let spans: Vec<[u64; 2]> = expected["spans"]
            .as_array()
            .unwrap()
            .iter()
            .map(|span| [span[0].as_u64().unwrap(), span[1].as_u64().unwrap()])
            .collect();
        let limit = case["work_limit"].as_u64().unwrap();
        let packet = expect_packet(
            expected["status"].as_u64().unwrap(),
            &spans,
            expected["work_used"].as_u64(),
            expected["reason"].as_u64().unwrap(),
            expected["detail"].as_u64().unwrap(),
        );
        let bounded = format!("if work_used(matcher) <= {limit}usize {{ {packet} }} else {{ -3 }}");
        let source = format!("{ENGINE}\n@id(\"experiment.pattern.witness.main\")\nfn main() -> i64\n{{\n{}\nlet storage = bytes_zeroed(3072usize);\nlet initial = matcher_from_bytes(storage);\n{seed}let matcher = full_match({last}, input_view, {limit}usize);\n{bounded}\n}}", bytes("input", case["input_ascii"].as_str().unwrap().as_bytes()));
        interpret(&source, case["id"].as_str().unwrap());
    }
}

#[test]
fn private_ascii_pattern_header_source_backend_parity_and_settlement() {
    use super::super::owned_string_loops_v1::support::Fixture;
    let expected = expect_packet(1, &[[0, 5], [7, 12]], None, 0, 0);
    let source = compile_source(
        b"([A-Za-z-]+):[ \\x09]*([^\\x0D\\x0A]*)",
        b"X-Key: value",
        &expected,
    );
    interpret(&source, "ascii-header-interpreter");
    let program = semaprax::check(&source, "ascii-header.spx").unwrap();
    let mut fixture = Fixture::new(&source);
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let probe = format!("{}\n#define FIXTURE_TRACK_CALLOC\n{}\n{generated}\n#undef malloc\n#undef calloc\n#undef free\n#undef FIXTURE_TRACK_CALLOC\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\nfor(unsigned i=0;i<4;++i) {{ int64_t value=INT64_MIN; REQUIRE(spx_decl_{}(&context,&value)==0); REQUIRE(value==1); REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees); }}\nreturn 0; }}\n",
        include_str!("../../support/native_fixture_stdio.c"),
        include_str!("../../native_owned_utf8_settlement_v1/allocations.c"), super::hex_identity("experiment.pattern.witness.main"));
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), "");
    }
    let web = fixture.root.join("web");
    semaprax::wasm::build_web(&program, &web).unwrap();
    std::fs::write(web.join("probe.mjs"), r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:2});
for(let i=0;i<4;i++) { const value=instance.exports.semaprax_main(); if(value!==1n) throw Error(`pattern:${value}`); }
"#).unwrap();
    let output = std::process::Command::new("node")
        .arg(web.join("probe.mjs"))
        .current_dir(&web)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    for name in [
        "app.wasm",
        "semaprax.js",
        "index.html",
        "package.json",
        "semaprax.manifest.json",
        "probe.mjs",
    ] {
        std::fs::remove_file(web.join(name)).unwrap();
    }
    std::fs::remove_dir(web).unwrap();
    fixture.cleanup();
}
