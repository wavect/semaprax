//! Finite nested response source, independent wire oracle and physical owner gates.
#[path = "collection_response/multi_string.rs"]
mod multi_string;

use super::*;

const SCHEMA: &str = r#"module consumer.schema;
@id("response.item") record Item {
 @id("response.item.number") number:i64,
 @id("response.item.label") label:string,
 @id("response.item.byte") byte:u8,
 @id("response.item.count") count:usize,
 @id("response.item.active") active:bool,
}
@id("response.metrics") record Metrics {
 @id("response.metrics.selected") selected:usize,
 @id("response.metrics.total") total:i64,
 @id("response.metrics.live") live:bool,
}
@id("response.report") record Report {
 @id("response.report.metrics") stats:Metrics,
 @id("response.report.items") entries:Vec<Item>,
}
@id("consumer.schema.anchor") fn anchor()->i64{0}
"#;

fn imports() -> &'static str {
    r#"module consumer.app;
use type @id("response.item") from consumer.schema as Item;
use type @id("response.metrics") from consumer.schema as Metrics;
use type @id("response.report") from consumer.schema as Report;
use type @id("response.report.json.collection-response.encode-result") from consumer.schema as Encoded;
use function @id("response.report.json.collection-response.encoded-len") from consumer.schema as encoded_len;
use function @id("response.report.json.collection-response.encode") from consumer.schema as encode;
@id("consumer.equal") fn equal(actual:borrow str,expected:borrow str)->bool{
let left=str_as_bytes(actual);let right=str_as_bytes(expected);let mut at=0usize;
let mut same=byte_len(left)==byte_len(right);
while same && at<byte_len(left){same=match byte_get(left,at){Option::Some{value:a}=>match byte_get(right,at){Option::Some{value:b}=>a==b,Option::None{}=>false,},Option::None{}=>false,};at=at+1usize;same && at<byte_len(left)}same
}
"#
}

fn install(label: &str, app: &str, bound: usize) -> std::path::PathBuf {
    install_schema(label, SCHEMA, app, bound)
}

fn install_schema(label: &str, schema: &str, app: &str, bound: usize) -> std::path::PathBuf {
    let root = fixture(label, schema);
    let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let policy = project::JsonCodecProfile::CollectionResponse {
            max_string_bytes: bound,
        };
        let source = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "response.report",
            policy,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "response.report",
            &source,
            policy,
        )?;
        assert_eq!(
            project::verify_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "response.report",
                &source,
                project::JsonCodecProfile::CollectionResponse {
                    max_string_bytes: if bound == 64 { 63 } else { bound + 1 }
                }
            )
            .unwrap_err()[0]
                .code,
            "SPX-J180"
        );
        let forged = source.replace("count <= 256usize", "count <= 257usize");
        assert_ne!(forged, source);
        assert_eq!(
            project::verify_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "response.report",
                &forged,
                policy
            )
            .unwrap_err()[0]
                .code,
            "SPX-J180"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("src/schema.spx")).unwrap(),
            canonical(schema)
        );
        Ok(source)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), source).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(app)).unwrap();
    root
}

fn wasm_run(root: &std::path::Path, status: u32, value: i64, refusal: &str) {
    let host = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/owned_data/owned_leaf_vec/host.js");
    let output = Command::new("node")
        .arg(host)
        .arg(root.join("app.wasm"))
        .arg(status.to_string())
        .arg(value.to_string())
        .arg(refusal)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{refusal}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn qualify(root: &std::path::Path) {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|e| vec![e])?;
        let graph = snapshot.retain_revision();
        assert!(graph
            .semantic_graph()
            .contains("response.report.json.collection-response.encode"));
        assert!(graph.semantic_graph().contains("core.vec.clone-at"));
        let options = project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
            .map_err(|e| vec![e])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(728)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|e| vec![e])?;
        for optimization in ["-O0", "-O2"] {
            super::super::compile_and_run_c(&c, root, optimization, "728");
        }
        let bytes = wasm::emit_resolved_module(snapshot.entry_program()).map_err(|e| vec![e])?;
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        Ok(())
    })
    .unwrap();
    wasm_run(root, 0, 728, "none");
}

#[test]
fn collection_response_unicode_nul_scalar_boundaries_and_exact_output_agree_on_three_backends() {
    let expected="{\"stats\":{\"selected\":3,\"total\":-9223372036854775808,\"live\":false},\"entries\":[{\"number\":-9223372036854775808,\"label\":\"é\\u0000😀\\n\\\"\\\\\",\"byte\":255,\"count\":18446744073709551615,\"active\":false},{\"number\":0,\"label\":\"\",\"byte\":0,\"count\":0,\"active\":true},{\"number\":0,\"label\":\"\",\"byte\":0,\"count\":0,\"active\":true}]}";
    let app = imports().to_owned()
        + &format!(
            r#"
@id("consumer.empty") fn empty()->Item{{Item{{number:0,label:"",byte:0u8,count:0usize,active:true}}}}
@id("consumer.main") fn main()->i64{{
let nul=string_from_char(char_from_i64(0));let prefix=string_concat("é",nul);let text=string_concat(prefix,"😀\n\"\\");
let item=Item{{number:-9223372036854775808,label:text,byte:255u8,count:18446744073709551615usize,active:false}};
let first=vec_push<Item>(vec_with_capacity<Item>(3usize),item);let second=vec_push<Item>(first,empty());let rows=vec_push<Item>(second,empty());
let report=Report{{stats:Metrics{{selected:3usize,total:-9223372036854775808,live:false}},entries:rows}};
let required=encoded_len(report);let short=encode(report,{}usize);
let short_ok=match own short{{Encoded::Refused{{required:count}}=>count=={}usize,Encoded::Encoded{{text}}=>false,}};
let full=encode(report,{}usize);let expected={};
let same=match own full{{Encoded::Refused{{required}}=>false,Encoded::Encoded{{text}}=>equal(string_as_str(text),string_as_str(expected)),}};
if required=={}usize && short_ok && same{{728}}else{{0}}
}}
"#,
            expected.len() - 1,
            expected.len(),
            expected.len(),
            spx_string(expected),
            expected.len()
        );
    let root = install("response-unicode", &app, 64);
    qualify(&root);
    // The first vector deep clone is in preflight, before response publication.
    // The strict host refuses that String leaf allocation and asserts all
    // payload/vector authority is retired, preserving the selected status.
    wasm_run(&root, 15, 0, "string-clone-allocation");
    std::fs::remove_dir_all(root).unwrap();
}

fn spx_string(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    )
}

#[test]
fn collection_response_checks_actual_cardinality_and_string_bound_before_output() {
    let app = imports().to_owned()
        + r#"
@id("consumer.row") fn row()->Item{Item{number:0,label:"xx",byte:0u8,count:0usize,active:true}}
@id("consumer.main") fn main()->i64{
let invalid=Report{stats:Metrics{selected:1usize,total:0,live:true},entries:vec_push<Item>(vec_with_capacity<Item>(1usize),row())};
let bad=encode(invalid,131072usize);let bad_ok=match own bad{Encoded::Refused{required}=>required==18446744073709551615usize,Encoded::Encoded{text}=>false,};
let mut rows=vec_with_capacity<Item>(257usize);let mut at=0usize;
while at<257usize{rows=vec_push<Item>(rows,row());at=at+1usize;at<257usize}
let too_many=Report{stats:Metrics{selected:257usize,total:0,live:true},entries:rows};
let refused=encode(too_many,131072usize);let count_ok=match own refused{Encoded::Refused{required}=>required==18446744073709551615usize,Encoded::Encoded{text}=>false,};
let empty=Report{stats:Metrics{selected:0usize,total:0,live:true},entries:vec_with_capacity<Item>(0usize)};
let needed=encoded_len(empty);let outcome=encode(empty,needed);
let expected="{\"stats\":{\"selected\":0,\"total\":0,\"live\":true},\"entries\":[]}";
let empty_ok=match own outcome{Encoded::Refused{required}=>false,Encoded::Encoded{text}=>equal(string_as_str(text),string_as_str(expected)),};
if bad_ok && count_ok && empty_ok{728}else{0}
}
"#;
    let root = install("response-refusal", &app, 1);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn collection_response_full_256_rows_at_64_bytes_preserves_independent_wire_oracle() {
    let row_json = format!(
        "{{\"number\":0,\"label\":\"{}\",\"byte\":0,\"count\":0,\"active\":true}}",
        "\\u0000".repeat(64)
    );
    let head = "{\"stats\":{\"selected\":256,\"total\":0,\"live\":true},\"entries\":[";
    let required = head.len() + 256 * row_json.len() + 255 + 2;
    assert!(required > 100_000);
    assert!(required <= 131_072);
    let app = imports().to_owned()
        + &format!(
            r#"
@id("consumer.main") fn main()->i64{{
let mut text="";let mut scalar=0usize;
while scalar<64usize{{text=string_concat(text,string_from_char(char_from_i64(0)));scalar=scalar+1usize;scalar<64usize}}
let seed=Item{{number:0,label:text,byte:0u8,count:0usize,active:true}};
let seeds=vec_push<Item>(vec_with_capacity<Item>(1usize),seed);
let mut rows=vec_with_capacity<Item>(256usize);let mut expected={};let expected_row={};let mut at=0usize;
while at<256usize{{
let row=vec_clone_at<Item>(seeds,0usize);rows=vec_push<Item>(rows,row);
let _ = if at>0usize{{expected=string_concat(expected,",");true}}else{{true}};
expected=string_concat(expected,expected_row);at=at+1usize;at<256usize
}}
let wanted=string_concat(expected,"]}}");
let report=Report{{stats:Metrics{{selected:256usize,total:0,live:true}},entries:rows}};
let required=encoded_len(report);let short=encode(report,{}usize);
let short_ok=match own short{{Encoded::Refused{{required:count}}=>count=={}usize,Encoded::Encoded{{text}}=>false,}};
let full=encode(report,{}usize);let same=match own full{{Encoded::Refused{{required}}=>false,Encoded::Encoded{{text}}=>equal(string_as_str(text),string_as_str(wanted)),}};
if required=={}usize && short_ok && same{{728}}else{{0}}
}}
"#,
            spx_string(head),
            spx_string(&row_json),
            required - 1,
            required,
            required,
            required
        );
    let root = install("response-full-cardinality", &app, 64);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn collection_response_long_wire_keys_select_fixed_output_capacity_refusal() {
    let app = imports().to_owned()
        + r#"
@id("consumer.main") fn main()->i64{
let mut text="";let mut scalar=0usize;
while scalar<64usize{text=string_concat(text,string_from_char(char_from_i64(0)));scalar=scalar+1usize;scalar<64usize}
let seed=Item{number:0,label:text,byte:0u8,count:0usize,active:true};
let seeds=vec_push<Item>(vec_with_capacity<Item>(1usize),seed);
let mut rows=vec_with_capacity<Item>(256usize);let mut at=0usize;
while at<256usize{let row=vec_clone_at<Item>(seeds,0usize);rows=vec_push<Item>(rows,row);at=at+1usize;at<256usize}
let report=Report{stats:Metrics{selected:256usize,total:0,live:true},entries:rows};
let required=encoded_len(report);let outcome=encode(report,18446744073709551615usize);
let refused=match own outcome{Encoded::Refused{required}=>required==18446744073709551615usize,Encoded::Encoded{text}=>false,};
if required==18446744073709551615usize && refused{728}else{0}
}
"#;
    let mut schema = SCHEMA.to_owned();
    let mut app = app;
    for (field, ty, letter) in [
        ("number", "i64", "n"),
        ("label", "string", "l"),
        ("byte", "u8", "b"),
        ("count", "usize", "c"),
        ("active", "bool", "a"),
    ] {
        let long = letter.repeat(64);
        schema = schema.replace(&format!("{field}:{ty}"), &format!("{long}:{ty}"));
        app = app.replace(&format!("{field}:"), &format!("{long}:"));
    }
    // Independently, key text + the escaped String alone exceeds the cap.
    assert!(256 * (5 * 64 + 64 * 6) > 131_072);
    let root = install_schema("response-output-capacity", &schema, &app, 64);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}
