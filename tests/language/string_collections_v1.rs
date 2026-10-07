//! String Collections v1 (`docs/STRING-COLLECTIONS-V1.md`): bytewise
//! `string_compare` and the legacy no-type-arguments map API, whose default
//! shape is `Map<string, i64>` kept in ascending bytewise key order. The same
//! corpus runs on the reference interpreter and on generated C11 under an
//! allocation-counting allocator that rejects duplicate and foreign frees and
//! requires zero live allocations after every case, including checked map
//! failures inside loops. Invalid collection shapes and ownership modes keep
//! stable source diagnostics.

use std::path::Path;
use std::process::Command;

use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::wasm::internal_strings::{emit_module, InternalStringOptions};
use semaprax::{format, graph, hir, parse, verify};
use serde_json::Value;

// Loaded once by the owned-string-loops module; a second `mod` would duplicate it.
use super::owned_string_loops_v1::support;
use support::Fixture;

const SOURCE: &str = r#"
module test.string_collections;

@id("coll.basic")
fn basic() -> i64
{
    let mut counts = map_new(8usize);
    counts = map_add(counts, "b", 2);
    counts = map_add(counts, "a", 1);
    counts = map_add(counts, "b", 5);
    let key = "c";
    counts = map_set(counts, key, 40);
    counts = map_set(counts, key, 30);
    let present = if map_has(counts, "a") && !map_has(counts, "z") { 1 } else { 0 };
    let size = map_len(counts);
    map_get_or(counts, "b", 0) * 1000 + map_get_or(counts, key, 0) * 10 + map_get_or(counts, "zz", 9) + present * 100000 + string_len(key) * 1000000 + if size == 3usize { 0 } else { 99999999 }
}

@id("coll.order")
fn order() -> i64
{
    let mut words = map_new(16usize);
    words = map_add(words, "pear", 1);
    words = map_add(words, "", 2);
    words = map_add(words, "Zebra", 3);
    words = map_add(words, "apple", 4);
    words = map_add(words, "app", 5);
    words = map_add(words, "é", 6);
    let mut joined = "";
    let mut weights = 0;
    let count = map_len(words);
    let mut index = 0usize;
    while index < count {
        joined = string_concat(joined, map_key_at(words, index));
        joined = string_concat(joined, "|");
        weights = weights * 10 + map_value_at(words, index);
        index = index + 1usize;
        0
    }
    if string_compare(joined, "|Zebra|app|apple|pear|é|") == 0 { weights } else { -1 }
}

@id("coll.compare")
fn compare() -> i64
{
    let a = string_compare("abc", "abd");
    let b = string_compare("abd", "abc");
    let c = string_compare("same", "same");
    let d = string_compare("ab", "abc");
    let e = string_compare("B", "a");
    let f = string_compare("é", "z");
    let g = string_compare("", "");
    (a + 1) * 1000000 + (b + 1) * 100000 + (c + 1) * 10000 + (d + 1) * 1000 + (e + 1) * 100 + (f + 1) * 10 + g + 1
}

@id("coll.words")
fn words() -> i64
{
    let text = "to be or not to be that is";
    let size = string_len(text);
    let mut counts = map_new(16usize);
    let mut start = 0;
    while start < size {
        let found = string_find(text, " ", start);
        let end = if found < 0 { size } else { found };
        counts = map_add(counts, string_slice(text, start, end), 1);
        start = end + 1;
        0
    }
    let mut shown = 0;
    let mut previous_count = 0;
    let mut previous_index = 0usize;
    let mut ranked = 0;
    while shown < 3 {
        let mut best = 0usize;
        let mut best_count = -1;
        let mut i = 0usize;
        while i < map_len(counts) {
            let c = map_value_at(counts, i);
            let after = shown == 0 || c < previous_count || c == previous_count && i > previous_index;
            let better = after && c > best_count;
            best = if better { i } else { best };
            best_count = if better { c } else { best_count };
            i = i + 1usize;
            0
        }
        ranked = ranked * 100 + string_len(map_key_at(counts, best)) * 10 + best_count;
        previous_count = best_count;
        previous_index = best;
        shown = shown + 1;
        0
    }
    let distinct = map_len(counts);
    if distinct == 6usize { ranked } else { -1 }
}

@id("coll.loop_local")
fn loop_local() -> i64
{
    let mut totals = map_new(4usize);
    let mut i = 0;
    while i < 3 {
        let mut scratch = map_new(2usize);
        scratch = map_add(scratch, "x", i);
        totals = map_add(totals, "sum", map_get_or(scratch, "x", 0));
        i = i + 1;
        0
    }
    map_get_or(totals, "sum", -1)
}

@id("coll.reopen_branch")
fn reopen_branch() -> i64
{
    let mut evens = map_new(8usize);
    let mut i = 0;
    while i < 6 {
        let counted = if i % 2 == 0 {
            evens = map_add(evens, string_from_i64(i), i);
            0
        } else {
            0
        };
        i = i + 1;
        0
    }
    map_get_or(evens, "4", 0) * 10 + map_get_or(evens, "3", 7)
}

@id("coll.empty")
fn empty() -> i64
{
    let none = map_new(0usize);
    let size = map_len(none);
    if size == 0usize && !map_has(none, "") { map_get_or(none, "", 5) } else { -1 }
}

@id("coll.full")
fn full() -> i64
{
    let mut small = map_new(2usize);
    small = map_add(small, "a", 1);
    small = map_add(small, "b", 1);
    small = map_add(small, "a", 1);
    small = map_add(small, "c", 1);
    map_get_or(small, "a", 0)
}

@id("coll.full_in_loop")
fn full_in_loop() -> i64
{
    let mut seen = map_new(3usize);
    let mut log = "";
    let mut i = 0;
    while i < 10 {
        let key = string_from_i64(i);
        log = string_concat(log, key);
        seen = map_add(seen, string_from_i64(i), 1);
        i = i + 1;
        0
    }
    string_len(log)
}

@id("coll.zero_capacity")
fn zero_capacity() -> i64
{
    let mut none = map_new(0usize);
    none = map_set(none, "a", 1);
    0
}

@id("coll.key_index")
fn key_index() -> i64
{
    let mut one = map_new(1usize);
    one = map_add(one, "a", 1);
    string_len(map_key_at(one, 1usize))
}

@id("coll.value_index")
fn value_index() -> i64
{
    let none = map_new(1usize);
    map_value_at(none, 0usize)
}

@id("coll.capacity")
fn capacity() -> i64
{
    let huge = map_new(65537usize);
    0
}

@id("coll.max_capacity")
fn max_capacity() -> i64
{
    let mut largest = map_new(65536usize);
    largest = map_add(largest, "k", 2);
    map_get_or(largest, "k", 0)
}

@id("coll.overflow")
fn overflow() -> i64
{
    let mut big = map_new(1usize);
    big = map_add(big, "k", 9223372036854775807);
    big = map_set(big, "k", 9223372036854775806);
    big = map_add(big, "k", 1);
    big = map_add(big, "k", 1);
    0
}

@id("coll.underflow")
fn underflow() -> i64
{
    let mut low = map_new(1usize);
    low = map_add(low, "k", -9223372036854775807);
    low = map_add(low, "k", -1);
    low = map_add(low, "k", -1);
    0
}

@id("coll.condition")
fn condition() -> i64
{
    let mut grow = map_new(8usize);
    let mut i = 0;
    while map_len(grow) < 5usize {
        grow = map_add(grow, string_from_i64(i % 7), 1);
        i = i + 1;
        0
    }
    i
}

@id("app.main")
fn main() -> i64
{
    basic()
}
"#;

/// Every case and its observation on both lanes.
const CASES: &[(&str, &str)] = &[
    ("coll.basic", "ok|1107309"),
    ("coll.order", "ok|235416"),
    ("coll.compare", "ok|210021"),
    ("coll.words", "ok|222221"),
    ("coll.loop_local", "ok|3"),
    ("coll.reopen_branch", "ok|47"),
    ("coll.empty", "ok|5"),
    ("coll.full", "semaprax.map.v1|1"),
    ("coll.full_in_loop", "semaprax.map.v1|1"),
    ("coll.zero_capacity", "semaprax.map.v1|1"),
    ("coll.key_index", "semaprax.map.v1|2"),
    ("coll.value_index", "semaprax.map.v1|2"),
    ("coll.capacity", "semaprax.map.v1|3"),
    ("coll.max_capacity", "ok|2"),
    ("coll.overflow", "semaprax.map.v1|4"),
    ("coll.underflow", "semaprax.map.v1|4"),
    ("coll.condition", "ok|5"),
];

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn string_collections_round_trip_and_project_compiler_identities() {
    let program = parse(SOURCE, Path::new("string-collections.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, Path::new("string-collections-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&reparsed));
    let graph = graph::to_json(&program).unwrap();
    assert_eq!(graph, graph::to_json(&reparsed).unwrap());
    for callee in [
        "core.string.compare",
        "core.map.new",
        "core.map.add",
        "core.map.set",
        "core.map.get_or",
        "core.map.has",
        "core.map.len",
        "core.map.key_at",
        "core.map.value_at",
    ] {
        assert!(
            graph.contains(&format!("\"callee\":\"{callee}\"")),
            "{callee}"
        );
    }
    assert!(graph.contains("\"kind\":\"string_map\""));
    assert!(graph.contains("\"core.map.drop\""));
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
    // An annotated binding spells the one admitted instantiation canonically.
    let annotated = "module test.annotated;\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    let counts: Map<string, i64> = map_new(4usize);\n    let size = map_len(counts);\n    if size == 0usize { 0 } else { 1 }\n}\n";
    let program = parse(annotated, Path::new("annotated.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(format::canonical(&program), annotated);
}

#[test]
fn reference_interpreter_runs_string_collections() {
    let canonical = format::canonical(&parse(SOURCE, Path::new("string-collections.spx")).unwrap());
    let fixture = Fixture::new(&canonical);
    for (id, expected) in CASES {
        let result =
            interpreter::interpret(&fixture.source, id, &[], &InterpreterOptions::default())
                .unwrap();
        let envelope: Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        let observed = if outcome["kind"] == "returned" {
            format!("ok|{}", outcome["value"].as_str().unwrap())
        } else {
            assert_eq!(outcome["kind"], "failed", "{}", result.envelope);
            format!(
                "{}|{}",
                outcome["status"]["domain_id"].as_str().unwrap(),
                outcome["status"]["code"].as_u64().unwrap()
            )
        };
        assert_eq!(&observed, expected, "{id}");
    }
    fixture.cleanup();
}

#[test]
fn native_string_collections_settle_every_allocation_on_every_exit() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("string-collections-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    let mut probe = format!(
        "{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c")
    );
    let mut expected = String::new();
    for (id, observation) in CASES {
        let symbol = id
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        probe.push_str(&format!(
            r#"{{
    int64_t value=INT64_MIN;
    spx_status_token token=spx_decl_{symbol}(&context,&value);
    if(token==0) {{ (void)printf("{id}|ok|%lld\n",(long long)value); }}
    else {{
        REQUIRE(value==INT64_MIN);
        const struct spx_normalized_status *status=spx_status_resolve(&context,token);
        REQUIRE(status!=NULL);
        (void)printf("{id}|%s|%u\n",status->domain_id,(unsigned)status->code);
    }}
    REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
}}
"#
        ));
        expected.push_str(&format!("{id}|{observation}\n"));
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn core_wasm_emits_string_collections() {
    let program = parse(SOURCE, Path::new("string-collections-wasm.spx")).unwrap();
    let frozen = emit_module(
        &program,
        &["coll.compare".to_owned()],
        InternalStringOptions::default(),
    )
    .expect_err("the original standalone selector retains its refusal");
    assert_eq!(frozen.code, "SPX-W116");
    semaprax::wasm::internal_strings::emit_text_toolkit_module(
        &program,
        &["coll.compare".to_owned()],
        InternalStringOptions::default(),
    )
    .expect("the additive toolkit selector lowers String comparison");
    for lane in [
        semaprax::wasm::emit_module(&program),
        semaprax::wasm::emit_module_with_scalar_exports(&program, &["coll.compare".to_owned()]),
    ] {
        lane.expect("every Core Wasm lane emits String Collections v1");
    }
}

fn diagnostics(prefix: &str, body: &str) -> Vec<semaprax::diagnostic::Diagnostic> {
    let source = format!(
        "module test.refused;\n\n{prefix}@id(\"app.main\")\nfn main() -> i64\n{{\n{body}\n}}\n"
    );
    match parse(&source, Path::new("refused.spx")) {
        Ok(program) => {
            let found = verify::verify(&program);
            if found
                .iter()
                .any(|diagnostic| diagnostic.severity.is_error())
            {
                found
            } else {
                hir::resolve(&program).err().unwrap_or_default()
            }
        }
        Err(diagnostic) => vec![diagnostic],
    }
}

fn first_code(prefix: &str, body: &str) -> (String, String) {
    let found = diagnostics(prefix, body);
    let first = found
        .first()
        .unwrap_or_else(|| panic!("no diagnostic for {body}"));
    (first.code.to_owned(), first.message.clone())
}

#[test]
fn misplaced_maps_have_stable_diagnostics() {
    // Unsupported collection shapes and missing type arguments retain stable
    // type diagnostics; admitted maps can use the supported scalar types.
    assert_eq!(
        first_code("", "    let m = map_new<char, i64>(1usize);\n    0").0,
        "SPX-T274"
    );
    assert_eq!(
        first_code("", "    let m = map_new<i64, Bytes>(1usize);\n    0").0,
        "SPX-T274"
    );
    assert_eq!(
        first_code("", "    let m: Map = map_new(1usize);\n    0").0,
        "SPX-T274"
    );
    assert_eq!(
        first_code(
            "@id(\"t.map\")\nrecord Map {\n    @id(\"t.map.x\")\n    x: i64,\n}\n\n",
            "    0"
        )
        .0,
        "SPX-S113"
    );
    // Collection parameters require an explicit ownership mode.
    assert_eq!(
        first_code(
            "@id(\"t.f\")\nfn f(m: Map<i64, i64>) -> i64\n{\n    0\n}\n\n",
            "    0",
        )
        .0,
        "SPX-O001"
    );
    // Whole replacement and reuse after the reopen keep their ownership codes.
    assert_eq!(
        first_code(
            "",
            "    let mut m = map_new(3usize);\n    m = map_new(4usize);\n    0"
        )
        .0,
        "SPX-U105"
    );
    assert_eq!(
        first_code(
            "",
            "    let mut m = map_new(3usize);\n    m = map_add(m, \"b\", map_get_or(m, \"a\", 0));\n    0"
        )
        .0,
        "SPX-O101"
    );
    // Explicit collection operations require the exact type argument count.
    assert_eq!(
        first_code(
            "",
            "    let m = map_new(3usize);\n    let n = map_len<string, i64, bool>(m);\n    0"
        )
        .0,
        "SPX-T274"
    );
    assert_eq!(
        first_code("", "    let m = map_new(3usize);\n    map_get_or(m, \"a\")").0,
        "SPX-T204"
    );
    assert_eq!(
        first_code("", "    let m = map_new(3);\n    0").0,
        "SPX-T205"
    );
    // A while condition may allocate its key, but cannot consume an outer map.
    assert!(diagnostics("", "    let mut m = map_new(3usize);\n    while !map_has(m, \"a\") {\n        m = map_add(m, \"a\", 1);\n        0\n    }\n    0").is_empty());
    assert_eq!(
        first_code(
            "",
            "    let mut m = map_new(3usize);\n    while map_len(map_remove(m, \"a\")) > 0usize {\n        0\n    }\n    0"
        )
        .0,
        "SPX-T252"
    );
    // The names are reserved compiler functions.
    assert_eq!(
        first_code(
            "@id(\"t.map_len\")\nfn map_len(x: i64) -> i64\n{\n    x\n}\n\n",
            "    0"
        )
        .0,
        "SPX-S113"
    );
}

const COMMAND: &str = r#"module app.tally;

permit { fs.read, process.args.read, process.stderr.write, process.stdout.write }

@id("app.report")
fn report() -> i64
    uses { fs.read, process.args.read, process.stdout.write }
{
    let path = arg_utf8(0usize);
    let text = file_read_text(path);
    let size = string_len(text);
    let mut counts = map_new(1024usize);
    let mut start = 0;
    while start < size {
        let found = string_find(text, "\n", start);
        let end = if found < 0 { size } else { found };
        let word = string_trim(string_slice(text, start, end));
        let counted = if string_len(word) > 0 { counts = map_add(counts, word, 1); 0 } else { 0 };
        start = end + 1;
        0
    }
    let mut out = "";
    let mut index = 0usize;
    while index < map_len(counts) {
        out = string_concat(out, map_key_at(counts, index));
        out = string_concat(out, " ");
        out = string_concat(out, string_from_i64(map_value_at(counts, index)));
        out = string_concat(out, "\n");
        index = index + 1usize;
        0
    }
    let view = string_as_str(out);
    let written = stdout_write(str_as_bytes(view));
    0
}

@id("app.main")
fn main() -> i64
    uses { fs.read, process.args.read, process.stderr.write, process.stdout.write }
{
    if args_len() == 1usize { report() } else { 2 }
}
"#;

fn run(fixture: &Fixture, native: bool, arguments: &[&str]) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_semaprax"));
    command
        .current_dir(&fixture.root)
        .arg("run")
        .arg("source.spx");
    if native {
        command.arg("--native");
    }
    let output = command.args(arguments).output().unwrap();
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn command_line_program_counts_words_in_key_order() {
    let program = parse(COMMAND, Path::new("tally.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(format::canonical(&program), COMMAND);
    let mut fixture = Fixture::new(COMMAND);
    fixture.write("words.txt", "pear\napple\n\n pear \nZoo\napple\npear\n");
    if cfg!(windows) {
        // The scoped host filesystem provider is Unix-only (the CLI passes
        // no provider off Unix), so file reading fails closed on Windows.
        let (code, stdout, stderr) = run(&fixture, false, &["--", "words.txt"]);
        assert_eq!((code, stdout.as_str()), (1, ""), "{stderr}");
        assert!(stderr.contains("file access denied"), "{stderr}");
        fixture.cleanup();
        return;
    }
    let lanes: &[bool] = if command_available("clang") {
        &[false, true]
    } else {
        &[false]
    };
    for &native in lanes {
        assert_eq!(
            run(&fixture, native, &["--", "words.txt"]),
            (0, "Zoo 1\napple 2\npear 3\n".to_owned(), String::new()),
            "native={native}"
        );
    }
    fixture.cleanup();
}

/// A map update inside `for` traversal. The bounded Vec allocates through
/// `calloc`, outside the allocation-counting harness, so this case runs
/// through `semaprax run` on both lanes instead.
const FOR_SOURCE: &str = r#"module test.for_map;

@id("app.main")
fn main() -> i64
{
    let mut building = vec_with_capacity<i64>(4usize);
    building = vec_push<i64>(building, 3);
    building = vec_push<i64>(building, 1);
    building = vec_push<i64>(building, 3);
    let values = building;
    let mut counts = map_new(8usize);
    for item in values {
        counts = map_add(counts, string_from_i64(item), 1);
        0
    }
    let size = map_len(counts);
    if size == 2usize { map_get_or(counts, "3", 0) } else { -1 }
}
"#;

#[test]
fn map_updates_inside_for_traversal() {
    let program = parse(FOR_SOURCE, Path::new("for-map.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    assert_eq!(format::canonical(&program), FOR_SOURCE);
    let fixture = Fixture::new(FOR_SOURCE);
    let lanes: &[bool] = if command_available("clang") {
        &[false, true]
    } else {
        &[false]
    };
    for &native in lanes {
        assert_eq!(
            run(&fixture, native, &[]),
            (0, "2\n".to_owned(), String::new()),
            "native={native}"
        );
    }
    fixture.cleanup();
}
