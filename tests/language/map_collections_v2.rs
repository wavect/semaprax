//! Generic Map/Set v2: source projections, private helper/record boundaries,
//! deterministic ordering, cloned String reads, and checked settlement.
//! The Wasm and independent hostile-HIR corpus belongs to the owning backend lane.
use std::path::Path;
use std::process::Command;

use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, graph, hir, parse, verify};
use serde_json::Value;

const SOURCE: &str = r#"module test.map_collections_v2;

@id("map.box")
record Collections {
    @id("map.box.values")
    values: Map<i64, string>,
    @id("map.box.keys")
    keys: Set<i64>,
}

@id("map.make")
fn make_map() -> Map<i64, string> {
    let mut values = map_new<i64, string>(3usize);
    values = map_set<i64, string>(values, 2, "two");
    values
}
@id("map.forward")
fn forward(values: own Map<i64, string>) -> Map<i64, string> { values }
@id("map.read")
fn read(values: borrow Map<i64, string>, key: i64) -> string {
    map_get_or<i64, string>(values, key, "missing")
}
@id("map.consume-record")
fn consume_record(collections: own Collections) -> i64 {
    match own collections {
        Collections { values: values, keys: keys } => {
            let word = read(values, 2);
            if set_has<i64>(keys, 2) { string_len(word) } else { -1 }
        },
    }
}

@id("map.strings")
fn strings() -> i64 {
    let key = "é\u{0}";
    let input = "alpha";
    let mut values = map_new<string, string>(2usize);
    values = map_set<string, string>(values, key, input);
    let copied = map_get_or<string, string>(values, key, "absent");
    let changed = string_concat(copied, "!");
    let fallback = "fallback";
    let missing = map_get_or<string, string>(values, "missing", fallback);
    let changed_fallback = string_concat(missing, "!");
    let output_key = map_key_at<string, string>(values, 0usize);
    let changed_key = string_concat(output_key, "!");
    let output_value = map_value_at<string, string>(values, 0usize);
    if string_len(key) == 3 && string_len(input) == 5
        && string_len(changed) == 6 && string_len(fallback) == 8
        && string_len(changed_fallback) == 9 && string_len(changed_key) == 4
        && string_compare(output_value, input) == 0
        && map_has<string, string>(values, key) { 101 } else { -1 }
}

@id("map.order")
fn order() -> i64 {
    let mut values = map_new<i64, i64>(3usize);
    values = map_set<i64, i64>(values, 9, 90);
    values = map_set<i64, i64>(values, -5, 50);
    values = map_set<i64, i64>(values, 2, 20);
    values = map_set<i64, i64>(values, 2, 21);
    values = map_remove<i64, i64>(values, 99);
    values = map_remove<i64, i64>(values, 2);
    values = map_set<i64, i64>(values, 1, 10);
    if map_len<i64, i64>(values) == 3usize
        && map_key_at<i64, i64>(values, 0usize) == -5
        && map_key_at<i64, i64>(values, 1usize) == 1
        && map_key_at<i64, i64>(values, 2usize) == 9
        && map_value_at<i64, i64>(values, 1usize) == 10
        && map_get_or<i64, i64>(values, 2, 7) == 7 { 102 } else { -1 }
}

@id("map.sets")
fn sets() -> i64 {
    let mut words = set_new<string>(3usize);
    let word = "é";
    words = set_insert<string>(words, word);
    words = set_insert<string>(words, "a");
    words = set_insert<string>(words, "a");
    words = set_remove<string>(words, "absent");
    let first = set_key_at<string>(words, 0usize);
    let changed = string_concat(first, "!");
    let mut integers = set_new<i64>(2usize);
    integers = set_insert<i64>(integers, 9);
    integers = set_insert<i64>(integers, -2);
    integers = set_remove<i64>(integers, 9);
    let mut flags = set_new<bool>(2usize);
    flags = set_insert<bool>(flags, true);
    flags = set_insert<bool>(flags, false);
    flags = set_insert<bool>(flags, true);
    if set_len<string>(words) == 2usize && set_has<string>(words, word)
        && string_compare(changed, "a!") == 0 && string_len(word) == 2
        && set_key_at<i64>(integers, 0usize) == -2
        && !set_key_at<bool>(flags, 0usize) && set_key_at<bool>(flags, 1usize)
        && set_len<bool>(flags) == 2usize { 103 } else { -1 }
}

@id("map.helpers")
fn helpers() -> i64 {
    let values = forward(make_map());
    let mut keys = set_new<i64>(2usize);
    keys = set_insert<i64>(keys, 2);
    let collections = Collections { values: values, keys: keys };
    consume_record(collections)
}

@id("map.loop")
fn loop_case() -> i64 {
    let mut values = map_new<i64, string>(3usize);
    let mut keys = set_new<i64>(3usize);
    let mut i = 0;
    while i < 3 {
        values = map_set<i64, string>(values, i, string_from_i64(i));
        keys = set_insert<i64>(keys, i);
        i = i + 1;
        0
    }
    let mut total = 0;
    let mut index = 0usize;
    while index < map_len<i64, string>(values) {
        let text = map_value_at<i64, string>(values, index);
        total = total + string_len(text);
        index = index + 1usize;
        0
    }
    if set_len<i64>(keys) == 3usize { total } else { -1 }
}

@id("map.scalars")
fn scalars() -> i64 {
    let mut a = map_new<bool, i64>(1usize);
    a = map_set<bool, i64>(a, true, -7);
    let mut b = map_new<bool, i32>(1usize);
    b = map_set<bool, i32>(b, true, -7i32);
    let mut c = map_new<bool, u8>(1usize);
    c = map_set<bool, u8>(c, true, 255u8);
    let mut d = map_new<bool, usize>(1usize);
    d = map_set<bool, usize>(d, true, 18446744073709551615usize);
    let mut e = map_new<bool, char>(1usize);
    e = map_set<bool, char>(e, true, 'é');
    let mut f = map_new<bool, f32>(1usize);
    f = map_set<bool, f32>(f, true, 1.5f32);
    let mut g = map_new<bool, f64>(1usize);
    g = map_set<bool, f64>(g, true, 2.5f64);
    let mut h = map_new<bool, bool>(1usize);
    h = map_set<bool, bool>(h, true, true);
    if map_get_or<bool, i64>(a, true, 0) == -7
        && map_get_or<bool, i32>(b, true, 0i32) == -7i32
        && map_get_or<bool, u8>(c, true, 0u8) == 255u8
        && map_get_or<bool, usize>(d, true, 0usize) == 18446744073709551615usize
        && map_value_at<bool, char>(e, 0usize) == 'é'
        && map_value_at<bool, f32>(f, 0usize) == 1.5f32
        && map_value_at<bool, f64>(g, 0usize) == 2.5f64
        && map_value_at<bool, bool>(h, 0usize) { 8 } else { -1 }
}

@id("map.bool")
fn bool_map() -> i64 {
    let mut values = map_new<bool, string>(2usize);
    values = map_set<bool, string>(values, true, "yes");
    values = map_set<bool, string>(values, false, "no");
    if !map_key_at<bool, string>(values, 0usize)
        && map_key_at<bool, string>(values, 1usize) {
        string_len(map_value_at<bool, string>(values, 0usize))
    } else { -1 }
}

@id("map.empty")
fn empty() -> i64 {
    let values = map_new<i64, string>(0usize);
    let keys = set_new<bool>(0usize);
    if map_len<i64, string>(values) == 0usize && set_len<bool>(keys) == 0usize {
        string_len(map_get_or<i64, string>(values, 0, "none"))
    } else { -1 }
}
@id("map.full")
fn full() -> i64 {
    let mut values = map_new<i64, string>(1usize);
    values = map_set<i64, string>(values, 1, "first");
    values = map_set<i64, string>(values, 1, "replaced");
    values = map_set<i64, string>(values, 2, "overflow");
    0
}
@id("map.set-full")
fn set_full() -> i64 {
    let mut keys = set_new<string>(1usize);
    keys = set_insert<string>(keys, "a");
    keys = set_insert<string>(keys, "a");
    keys = set_insert<string>(keys, "b");
    0
}
@id("map.index")
fn index() -> i64 {
    let values = map_new<bool, string>(1usize);
    string_len(map_value_at<bool, string>(values, 0usize))
}
@id("map.set-index")
fn set_index() -> i64 {
    let keys = set_new<string>(0usize);
    string_len(set_key_at<string>(keys, 18446744073709551615usize))
}
@id("map.capacity")
fn capacity() -> i64 { let values = map_new<i64, bool>(65537usize); 0 }
@id("map.staging")
fn staging() -> i64 {
    let mut values = map_new<i64, i64>(1usize);
    values = map_set<i64, i64>(values, 0, 1 / 0);
    0
}
@id("map.legacy")
fn legacy() -> i64 {
    let mut values = map_new(2usize);
    values = map_add(values, "a", 2);
    values = map_add(values, "b", 3);
    values = map_remove(values, "a");
    map_get_or(values, "b", 0)
}
@id("map.take-values")
fn take_values(collections: own Collections) -> Map<i64, string> {
    match own collections { Collections { values: values, keys: keys } => values, }
}
@id("map.take-keys")
fn take_keys(collections: own Collections) -> Set<i64> {
    match own collections { Collections { values: values, keys: keys } => keys, }
}
@id("map.unpack-map")
fn unpack_map() -> i64 {
    let mut keys = set_new<i64>(1usize); keys = set_insert<i64>(keys, 2);
    let values = take_values(Collections { values: make_map(), keys: keys });
    string_len(map_get_or<i64, string>(values, 2, ""))
}
@id("map.unpack-set")
fn unpack_set() -> i64 {
    let mut keys = set_new<i64>(1usize); keys = set_insert<i64>(keys, 2);
    let kept = take_keys(Collections { values: make_map(), keys: keys });
    set_key_at<i64>(kept, 0usize)
}
@id("map.legacy-box")
record LegacyBox { @id("map.legacy-box.values") values: Map<string, i64>, @id("map.legacy-box.label") label: string, }
@id("map.take-legacy")
fn take_legacy(boxed: own LegacyBox) -> Map<string, i64> {
    match own boxed { LegacyBox { values: values, label: label } => values, }
}
@id("map.unpack-legacy")
fn unpack_legacy() -> i64 {
    let mut values = map_new(1usize); values = map_set(values, "kept", 17);
    let kept = take_legacy(LegacyBox { values: values, label: "discarded" });
    map_get_or(kept, "kept", 0)
}
@id("app.main")
fn main() -> i64 { strings() }
"#;

const CASES: &[(&str, &str)] = &[
    ("map.strings", "ok|101"),
    ("map.order", "ok|102"),
    ("map.sets", "ok|103"),
    ("map.helpers", "ok|3"),
    ("map.loop", "ok|3"),
    ("map.scalars", "ok|8"),
    ("map.bool", "ok|2"),
    ("map.empty", "ok|4"),
    ("map.full", "semaprax.map.v2|1"),
    ("map.set-full", "semaprax.map.v2|1"),
    ("map.index", "semaprax.map.v2|2"),
    ("map.set-index", "semaprax.map.v2|2"),
    ("map.capacity", "semaprax.map.v2|3"),
    ("map.staging", "semaprax.arithmetic.v1|4"),
    ("map.legacy", "ok|3"),
    ("map.unpack-map", "ok|3"),
    ("map.unpack-set", "ok|2"),
    ("map.unpack-legacy", "ok|17"),
];

#[test]
fn map_set_v2_source_roundtrip_and_checked_graph() {
    let program = parse(SOURCE, Path::new("map-collections-v2.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let canonical = format::canonical(&program);
    let roundtrip = parse(&canonical, Path::new("map-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&roundtrip), canonical);
    assert_eq!(graph::revision(&program), graph::revision(&roundtrip));
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&roundtrip).unwrap()
    );
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
}

#[test]
fn map_set_v2_interpreter_values_order_failure_and_private_boundaries() {
    let fixture = Fixture::new(SOURCE);
    for (id, expected) in CASES {
        let options = InterpreterOptions::default();
        let result = if *id == "map.helpers" {
            // Direct String-signature helpers require the explicit internal
            // profile; the frozen ordinary interpreter must still refuse them.
            let errors = interpreter::interpret(&fixture.source, id, &[], &options).unwrap_err();
            assert_eq!(errors.len(), 1, "{id}: {errors:?}");
            assert_eq!(errors[0].code, "SPX-F102", "{id}: {errors:?}");
            assert!(errors[0].message.contains("unsupported_callee"));
            let result =
                interpreter::internal_strings::interpret(&fixture.source, id, &[], &options)
                    .unwrap_or_else(|error| panic!("{id}: {error:?}"));
            interpreter::internal_strings::verify_envelope(&result.envelope).unwrap();
            interpreter::internal_strings::verify_envelope_against_source(
                &result.envelope,
                &fixture.source,
            )
            .unwrap();
            result
        } else {
            interpreter::interpret(&fixture.source, id, &[], &options)
                .unwrap_or_else(|error| panic!("{id}: {error:?}"))
        };
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

// Typed collection carriers use calloc. Observe that allocation through the
// same exact pointer ledger as malloc; retain all foreign/interior/duplicate
// free and balanced-owner assertions without increasing the ledger capacity.
const CALLOC_EVIDENCE_C: &str = r#"
static __attribute__((unused)) void *fixture_calloc(size_t count, size_t size) {
    REQUIRE(count != 0 && size != 0 && count <= SIZE_MAX / size);
    void *pointer = fixture_malloc(count * size);
    memset(pointer, 0, count * size);
    return pointer;
}
#define calloc fixture_calloc
"#;

#[test]
fn map_set_v2_native_balances_all_owners_at_o0_and_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(SOURCE, Path::new("map-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    let mut probe = format!("{}\n{}\n{}\n{generated}\n#undef malloc\n#undef calloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
        include_str!("../support/native_fixture_stdio.c"),
        include_str!("../native_owned_utf8_settlement_v1/allocations.c"), CALLOC_EVIDENCE_C);
    let mut expected = String::new();
    for _ in 0..4 {
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
    }
    probe.push_str("return 0; }\n");
    let mut fixture = Fixture::new(SOURCE);
    for optimization in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, optimization), expected);
    }
    fixture.cleanup();
}

#[test]
fn map_set_v2_unsupported_shapes_and_wrong_operands_have_stable_diagnostics() {
    for (body, expected) in [
        ("let values = map_new<char, i64>(1usize); 0", "SPX-T274"),
        ("let keys = set_new<u8>(1usize); 0", "SPX-T274"),
        ("let values = map_new<i64, Bytes>(1usize); 0", "SPX-T274"),
        ("let values = map_new<i64, i64>(1); 0", "SPX-T205"),
        (
            "let values = map_new<i64, i64>(1usize); map_get_or<i64, i64>(values, 0)",
            "SPX-T204",
        ),
    ] {
        let source =
            format!("module test.map_refused; @id(\"app.main\") fn main() -> i64 {{ {body} }}");
        let program = parse(&source, Path::new("map-refused.spx")).unwrap();
        let diagnostics = verify::verify(&program);
        assert_eq!(
            diagnostics.first().map(|d| d.code),
            Some(expected),
            "{body}: {diagnostics:?}"
        );
    }
}
