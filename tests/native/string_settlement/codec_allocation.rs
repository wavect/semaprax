//! OPT-724 composed fault sites; ordinary native failure policy is unchanged.
//! Vec refusal settles through status/out. String OOM is fatal outside that
//! channel: native_runtime.rs/native_scalar_runtime.rs. Process retirement is
//! neither zero-owner cleanup nor recovery; that part of OPT-724 remains unmet.
use super::{codegen, compile_and_run, fs, symbol, Ordering, OBSERVER, SERIAL, STDIO};
use semaprax::{format, hir, parse, project};

const SCHEMA: &str = r#"module consumer.schema;
@id("fault.config") record Config {@id("fault.config.label") label:string,@id("fault.config.retry") retry:usize,}
@id("fault.row") record Row {@id("fault.row.sku") sku:string,@id("fault.row.count") count:u8,}
@id("fault.report") record Report {@id("fault.report.config") config:Config,@id("fault.report.rows") rows:Vec<Row>,@id("fault.report.ok") ok:bool,}
@id("fault.anchor") fn anchor()->i64{0}
"#;
// Independent exact oracle. Input differs in order, whitespace and escapes.
const INPUT: &str = r#" {"ok":true,"rows":[{"count":255,"sku":"a"},{"sku":"\u03a9","count":7}],"config":{"retry":2,"label":"\u00e9\u0000"}} "#;
const OUTPUT: &str = "{\"config\":{\"label\":\"é\\u0000\",\"retry\":2},\"rows\":[{\"sku\":\"a\",\"count\":255},{\"sku\":\"Ω\",\"count\":7}],\"ok\":true}";
fn canonical(source: &str) -> String {
    format::canonical(&parse(source, "codec-allocation.spx").unwrap())
}
fn app() -> String {
    let bytes = OUTPUT
        .bytes()
        .map(|b| format!("{b}u8"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"module consumer.app;
use type @id("fault.report.json.nested.decode-result") from consumer.schema as Decoded;
use type @id("fault.report.json.nested-response.encode-result") from consumer.schema as Encoded;
use function @id("fault.report.json.nested.decode") from consumer.schema as decode;
use function @id("fault.report.json.nested-response.encode") from consumer.schema as encode;
use function @id("fault.report.json.nested-response.encoded-len") from consumer.schema as encoded_len;
@id("fault.decode") fn decode_only(input:borrow Slice<u8>)->i64{{
let outcome=decode(input,byte_len(input));match own outcome{{
Decoded::Error{{code,offset,field}}=>0,
Decoded::Ready{{value}}=>if value.config.retry==2usize && value.ok{{724}}else{{0}},
}}
}}
@id("fault.compose") fn compose(input:borrow Slice<u8>)->i64{{
let outcome=decode(input,byte_len(input));match own outcome{{
Decoded::Error{{code,offset,field}}=>0,
Decoded::Ready{{value}}=>{{
let required=encoded_len(value);let encoded=encode(value,required);
match own encoded{{Encoded::Refused{{required:n}}=>0,Encoded::Encoded{{text}}=>{{
let expected=[{bytes}];let expected_view=array_as_slice(expected);
let actual=str_as_bytes(string_as_str(text));let mut at=0usize;
let mut same=byte_len(actual)==byte_len(expected_view);
while same && at<byte_len(actual){{
same=match byte_get(actual,at){{Option::None{{}}=>false,Option::Some{{value:a}}=>match byte_get(expected_view,at){{Option::None{{}}=>false,Option::Some{{value:b}}=>a==b,}},}};
at=at+1usize;same && at<byte_len(actual)
}}
if same && required=={length}usize{{724}}else{{0}}
}},}}
}},}}
}}
@id("consumer.main") fn main()->i64{{
let bytes=[0u8];let input=array_as_slice(bytes);
let first=decode_only(input);let second=compose(input);first+second
}}
"#,
        length = OUTPUT.len()
    )
}
fn generated() -> String {
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
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("semaprax.toml"),
        include_str!("bulk_utf8/semaprax.toml"),
    )
    .unwrap();
    fs::write(root.join("src/schema.spx"), canonical(SCHEMA)).unwrap();
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
            max_string_bytes: 16,
            max_array_items: 8,
        },
        project::JsonCodecProfile::NestedResponse {
            max_string_bytes: 16,
            max_array_items: 8,
        },
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
    fs::write(root.join("src/app.spx"), canonical(&app())).unwrap();
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
#[test]
fn composed_nested_codec_allocation_failures_preserve_return_and_fatal_boundaries() {
    let generated = generated();
    let raw = INPUT
        .bytes()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let allocator = include_str!("codec_allocation/allocator.c");
    let probe = include_str!("codec_allocation/probe.c");
    let decode = symbol("fault.decode");
    let compose = symbol("fault.compose");
    let source = format!(
        r#"#define _GNU_SOURCE
{STDIO}
#define FIXTURE_TRACK_CALLOC
{OBSERVER}
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
#define FAULT_DECODE {decode}
#define FAULT_COMPOSE {compose}
static uint8_t fault_input[] = {{{raw}}};
{probe}
"#
    );
    compile_and_run("composed-codec-allocation", &source, false);
}
