//! OPT808: actual slot borrows, short-output preflight and fatal boundaries.
use super::*;
const INPUT_V2: &str = r#" {"rows":["same","same","\u00e9\u0000\ud83d\ude00","\"\\"],"ok":true,"config":{"retry":2,"label":"\u00e9\u0000"}} "#;
const OUTPUT_V2: &str = "{\"config\":{\"label\":\"é\\u0000\",\"retry\":2},\"rows\":[\"same\",\"same\",\"é\\u0000😀\",\"\\\"\\\\\"],\"ok\":true}";
fn app_v2(invalid: bool) -> String {
    let bytes = OUTPUT_V2
        .bytes()
        .map(|b| format!("{b}u8"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"module consumer.app;
use type @id("fault.report") from consumer.schema as Report;
use type @id("fault.report.json.nested.decode-result") from consumer.schema as Decoded;
use type @id("fault.report.json.nested-response-v2.encode-result") from consumer.schema as Encoded;
use function @id("fault.report.json.nested.decode") from consumer.schema as decode;
use function @id("fault.report.json.nested-response-v2.encode") from consumer.schema as encode;
use function @id("fault.report.json.nested-response-v2.encoded-len") from consumer.schema as encoded_len;
@id("fault.decision") fn decision(values:borrow Vec<string>)->bool{{
let count=vec_len<string>(values);if count==4usize{{let first=vec_str_at(values,0usize);let last=vec_str_at(values,count-1usize);str_len_bytes(first)==4usize && str_len_bytes(last)==2usize}}else{{false}}
}}
@id("fault.decode") fn decode_only(input:borrow Slice<u8>)->i64{{let outcome=decode(input,byte_len(input));match own outcome{{Decoded::Error{{code,offset,field}}=>0,Decoded::Ready{{value}}=>if value.config.retry==2usize && value.ok{{724}}else{{0}},}}}}
@id("fault.preflight") fn preflight(input:borrow Slice<u8>)->i64{{let outcome=decode(input,byte_len(input));match own outcome{{Decoded::Error{{code,offset,field}}=>0,Decoded::Ready{{value}}=>{{
let accepted=decision(value.rows);let required=encoded_len(value);let short=encode(value,required-1usize);
let refused=match own short{{Encoded::Refused{{required:n}}=>n==required,Encoded::Encoded{{text}}=>false,}};
if accepted && refused && required=={length}usize{{724}}else{{0}}
}},}}}}
@id("fault.compose") fn compose(input:borrow Slice<u8>)->i64{{let outcome=decode(input,byte_len(input));match own outcome{{Decoded::Error{{code,offset,field}}=>0,Decoded::Ready{{value}}=>{{
let accepted=decision(value.rows);let required=encoded_len(value);let encoded=encode(value,required);
match own encoded{{Encoded::Refused{{required:n}}=>if {invalid} && n==18446744073709551615usize {{724}}else{{0}},Encoded::Encoded{{text}}=>{{
let expected=[{bytes}];let expected_view=array_as_slice(expected);let actual=str_as_bytes(string_as_str(text));let mut at=0usize;let mut same=byte_len(actual)==byte_len(expected_view);
while same && at<byte_len(actual){{same=match byte_get(actual,at){{Option::None{{}}=>false,Option::Some{{value:a}}=>match byte_get(expected_view,at){{Option::None{{}}=>false,Option::Some{{value:b}}=>a==b,}},}};at=at+1usize;same && at<byte_len(actual)}}
if accepted && same && required=={length}usize{{724}}else{{0}}
}},}}
}},}}}}
@id("consumer.main") fn main()->i64{{let bytes=[0u8];let input=array_as_slice(bytes);let first=decode_only(input);let second=preflight(input);let third=compose(input);first+second+third}}
"#,
        length = if invalid { usize::MAX } else { OUTPUT_V2.len() },
        invalid = invalid
    )
}
fn generated_v2(response: project::JsonCodecProfile, invalid: bool) -> String {
    let root = std::env::temp_dir().join(format!(
        "native-codec-allocation-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("semaprax.toml"),
        include_str!("../bulk_utf8/semaprax.toml"),
    )
    .unwrap();
    fs::write(
        root.join("src/schema.spx"),
        canonical(&SCHEMA.replace("Vec<Row>", "Vec<string>")),
    )
    .unwrap();
    for (path, source) in [
        (
            "src/app.spx",
            "module consumer.app;@id(\"consumer.main\") fn main()->i64{0}",
        ),
        (
            "src/tests.spx",
            "module consumer.tests;@id(\"consumer.tests\") fn main()->i64{0}",
        ),
    ] {
        fs::write(root.join(path), canonical(source)).unwrap();
    }
    for policy in [
        project::JsonCodecProfile::NestedRequest {
            max_string_bytes: 64,
            max_array_items: 256,
        },
        response,
    ] {
        let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            let source = project::derive_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "fault.report",
                policy,
            )?;
            project::verify_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "fault.report",
                &source,
                policy,
            )?;
            assert_eq!(source, canonical(&source));
            Ok(source)
        })
        .unwrap();
        fs::write(root.join("src/schema.spx"), source).unwrap();
    }
    fs::write(root.join("src/app.spx"), canonical(&app_v2(invalid))).unwrap();
    let result = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        let first = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        let second = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert_eq!(first, second);
        // main reaches both roots, so entry linking preserves both symbols.
        for id in ["fault.decode", "fault.compose"] {
            assert!(first.contains(&symbol(id)));
        }
        Ok(first)
    })
    .unwrap();
    fs::remove_dir_all(root).unwrap();
    result
}
// Probe-only forwarding: count the exact emitted descriptor operation while the
// original total-copy observer still sees every production memcpy.
fn observe_slot_copies(generated: String) -> String {
    let backend =
        include_str!("../../../../src/codegen/native_emit/expression/vec_ops/string_slot.rs");
    for exact in [
        "layout.kinds.as_slice() != [9] || layout.offsets.as_slice() != [0]",
        "spx_leaf_read_field(spx_ctx, &({}), &{}, {}, UINT32_C(0), &{field})",
        "memcpy(&{text}, {field}, sizeof({text}));",
    ] {
        assert!(
            backend.contains(exact),
            "String-slot metadata ownership premise changed: {exact}"
        );
    }
    let needle = "memcpy(&spx_string_slot_text_";
    let count = generated.matches(needle).count();
    assert!(
        count >= 4,
        "decision first/last plus length/render slot reads"
    );
    for line in generated.lines().filter(|line| line.contains(needle)) {
        let suffix = line
            .split(needle)
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap();
        assert!(suffix.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(line.trim(), format!("memcpy(&spx_string_slot_text_{suffix}, spx_string_slot_{suffix}, sizeof(spx_string_slot_text_{suffix}));"));
        assert!(generated.contains(&format!("&spx_string_slot_{suffix});")));
    }
    let watched = generated.replace(needle, "fixture_slot_memcpy(&spx_string_slot_text_");
    assert_eq!(
        watched.replace("fixture_slot_memcpy(&spx_string_slot_text_", needle),
        generated
    );
    watched
}
fn expected_wire_bytes() -> String {
    OUTPUT_V2
        .bytes()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",")
}
#[test]
fn primitive_string_codec_reads_and_short_refusal_allocate_nothing_then_copy_final_string_once() {
    let generated = observe_slot_copies(generated_v2(
        project::JsonCodecProfile::NestedResponseV2 {
            max_string_bytes: 64,
            max_array_items: 256,
        },
        false,
    ));
    let raw = INPUT_V2
        .bytes()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let allocator = include_str!("allocator.c");
    let copies = include_str!("../bulk_utf8/copies.c");
    let classified_copies = include_str!("string_slot_copies.c");
    let expected_bytes = expected_wire_bytes();
    let physical = format!(
        r#"
static void physical_checks(void) {{
fixture_copy_classification_control();
fixture_payload_watch(fixture_expected_wire,sizeof fixture_expected_wire);
fixture_copy_reset();size_t decoded=baseline(false),decode_copies=fixture_copy_calls,decode_bytes=fixture_copy_bytes;
REQUIRE(fixture_slot_copy_calls==0 && fixture_other_copy_calls==decode_copies && fixture_other_copy_bytes==decode_bytes && fixture_payload_copy_calls==0);
struct spx_status_entry entries[16];struct spx_context context={{0}};context_init(&context,entries);
fault_attempts=0;fault_selected=0;fixture_copy_reset();int64_t result=INT64_MIN;
REQUIRE(FAULT_PREFLIGHT(&context,input_view(),&result)==SPX_STATUS_SUCCESS && result==724);
REQUIRE(fault_attempts==decoded && fixture_other_copy_calls==decode_copies && fixture_other_copy_bytes==decode_bytes);
REQUIRE(fixture_slot_copy_calls>0 && fixture_slot_copy_bytes==fixture_slot_copy_calls*sizeof(const char *));
REQUIRE(fixture_copy_calls==decode_copies+fixture_slot_copy_calls && fixture_copy_bytes==decode_bytes+fixture_slot_copy_bytes);
REQUIRE(fixture_payload_copy_calls==0);size_t preflight_slots=fixture_slot_copy_calls;
REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees);
REQUIRE(context.call_depth==0 && context.borrowed_str_depth==0);
fixture_copy_reset();size_t composed=baseline(true);
REQUIRE(composed==decoded+2); /* one actual Bytes reservation, one final String */
REQUIRE(fixture_other_copy_calls==decode_copies+1 && fixture_other_copy_bytes==decode_bytes+{});
REQUIRE(fixture_slot_copy_calls>preflight_slots && fixture_slot_copy_bytes==fixture_slot_copy_calls*sizeof(const char *));
REQUIRE(fixture_copy_calls==fixture_other_copy_calls+fixture_slot_copy_calls && fixture_copy_bytes==fixture_other_copy_bytes+fixture_slot_copy_bytes);
REQUIRE(fixture_payload_copy_calls==1 && fixture_payload_copy_bytes==sizeof fixture_expected_wire);
}}
"#,
        OUTPUT_V2.len()
    );
    let probe = include_str!("probe.c")
        .replace(
            "static void child(",
            &format!("{physical}\nstatic void child("),
        )
        .replace(
            "size_t decoded=baseline(false),composed=baseline(true);",
            "physical_checks();size_t decoded=baseline(false),composed=baseline(true);",
        );
    let source = format!(
        r#"#define _GNU_SOURCE
{STDIO}
#define FIXTURE_TRACK_CALLOC
{OBSERVER}
{copies}
static const uint8_t fixture_expected_wire[]={{{expected_bytes}}};
{classified_copies}
#undef malloc
#undef calloc
{allocator}
#define malloc fault_malloc
#define calloc fault_calloc
#define main fixture_generated_main
{generated}
#undef main
#undef malloc
#undef calloc
#undef free
#undef memcpy
#define FAULT_DECODE {}
#define FAULT_COMPOSE {}
#define FAULT_PREFLIGHT {}
static uint8_t fault_input[]={{{raw}}};
{probe}
"#,
        symbol("fault.decode"),
        symbol("fault.compose"),
        symbol("fault.preflight")
    );
    // Same complete fault injection receiver, including fatal/nonrecoverable policy.
    compile_and_run("primitive-string-codec-allocation", &source, false);
}

#[test]
fn primitive_string_codec_actual_byte_and_item_plus_one_preflight_never_reserve_output() {
    for (label, policy) in [
        (
            "bytes",
            project::JsonCodecProfile::NestedResponseV2 {
                max_string_bytes: 6,
                max_array_items: 256,
            },
        ),
        (
            "items",
            project::JsonCodecProfile::NestedResponseV2 {
                max_string_bytes: 64,
                max_array_items: 3,
            },
        ),
    ] {
        let generated = observe_slot_copies(generated_v2(policy, true));
        let raw = INPUT_V2
            .bytes()
            .map(|b| b.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let allocator = include_str!("allocator.c");
        let copies = include_str!("../bulk_utf8/copies.c");
        let classified_copies = include_str!("string_slot_copies.c");
        let expected_bytes = expected_wire_bytes();
        let prefix = include_str!("probe.c")
            .split_once("static void child(")
            .unwrap()
            .0;
        let source = format!(
            r#"#define _GNU_SOURCE
{STDIO}
#define FIXTURE_TRACK_CALLOC
{OBSERVER}
{copies}
static const uint8_t fixture_expected_wire[]={{{expected_bytes}}};
{classified_copies}
#undef malloc
#undef calloc
{allocator}
#define malloc fault_malloc
#define calloc fault_calloc
#define main fixture_generated_main
{generated}
#undef main
#undef malloc
#undef calloc
#undef free
#undef memcpy
#define FAULT_DECODE {}
#define FAULT_COMPOSE {}
#define FAULT_PREFLIGHT {}
static uint8_t fault_input[]={{{raw}}};
{prefix}
int main(void){{
REQUIRE(fixture_binary_stdout());
fault_borrowed_input=mmap(NULL,sizeof fault_input,PROT_READ|PROT_WRITE,MAP_SHARED|MAP_ANONYMOUS,-1,0);REQUIRE(fault_borrowed_input!=MAP_FAILED);
memcpy(fault_borrowed_input,fault_input,sizeof fault_input);REQUIRE(mprotect(fault_borrowed_input,sizeof fault_input,PROT_READ)==0);
fixture_copy_classification_control();fixture_payload_watch(fixture_expected_wire,sizeof fixture_expected_wire);
fixture_copy_reset();size_t decoded=baseline(false),calls=fixture_copy_calls,bytes=fixture_copy_bytes;
REQUIRE(fixture_slot_copy_calls==0 && fixture_other_copy_calls==calls && fixture_other_copy_bytes==bytes && fixture_payload_copy_calls==0);
struct spx_status_entry entries[16];struct spx_context context={{0}};context_init(&context,entries);
fault_attempts=0;fault_selected=0;fixture_copy_reset();int64_t result=INT64_MIN;
REQUIRE(FAULT_PREFLIGHT(&context,input_view(),&result)==SPX_STATUS_SUCCESS && result==724);
REQUIRE(fault_attempts==decoded && fixture_other_copy_calls==calls && fixture_other_copy_bytes==bytes);
REQUIRE(fixture_slot_copy_calls>0 && fixture_slot_copy_bytes==fixture_slot_copy_calls*sizeof(const char *));
REQUIRE(fixture_copy_calls==calls+fixture_slot_copy_calls && fixture_copy_bytes==bytes+fixture_slot_copy_bytes && fixture_payload_copy_calls==0);
REQUIRE(fixture_live==0 && fixture_allocations==fixture_frees && context.call_depth==0 && context.borrowed_str_depth==0);
fixture_copy_reset();REQUIRE(baseline(true)==decoded && fixture_other_copy_calls==calls && fixture_other_copy_bytes==bytes);
REQUIRE(fixture_slot_copy_calls>0 && fixture_slot_copy_bytes==fixture_slot_copy_calls*sizeof(const char *));
REQUIRE(fixture_copy_calls==calls+fixture_slot_copy_calls && fixture_copy_bytes==bytes+fixture_slot_copy_bytes && fixture_payload_copy_calls==0);
REQUIRE(!memcmp(fault_borrowed_input,fault_input,sizeof fault_input));REQUIRE(munmap(fault_borrowed_input,sizeof fault_input)==0);
(void)puts("native-ordinary-strings-settled");return 0;
}}
"#,
            symbol("fault.decode"),
            symbol("fault.compose"),
            symbol("fault.preflight")
        );
        compile_and_run(
            &format!("primitive-string-codec-{label}-refusal"),
            &source,
            false,
        );
    }
}
