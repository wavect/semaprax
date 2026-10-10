//! Nested response derivation and unchanged three-backend owning command closure.
use super::*;

const SCHEMA: &str = r#"module consumer.schema;
@id("r.config") record Config {@id("r.config.label") label:string,@id("r.config.retry") retry:usize,}
@id("r.row") record Row {@id("r.row.sku") sku:string,@id("r.row.count") count:u8,}
@id("r.root") record Report {@id("r.root.config") config:Config,@id("r.root.rows") rows:Vec<Row>,@id("r.root.ok") ok:bool,}
@id("consumer.schema.anchor") fn anchor()->i64{0}
"#;
const PROFILE: project::JsonCodecProfile = project::JsonCodecProfile::NestedResponse {
    max_string_bytes: 16,
    max_array_items: 8,
};
fn imports() -> String {
    r#"module consumer.app;
use type @id("r.config") from consumer.schema as Config;
use type @id("r.row") from consumer.schema as Row;
use type @id("r.root") from consumer.schema as Report;
use type @id("r.root.json.nested-response.encode-result") from consumer.schema as Encoded;
use function @id("r.root.json.nested-response.encoded-len") from consumer.schema as encoded_len;
use function @id("r.root.json.nested-response.encode") from consumer.schema as encode;
@id("consumer.equal") fn equal(actual:borrow str,expected:borrow str)->bool{
let left=str_as_bytes(actual);let right=str_as_bytes(expected);let mut at=0usize;
let mut same=byte_len(left)==byte_len(right);
while same && at<byte_len(left){same=match byte_get(left,at){Option::Some{value:a}=>match byte_get(right,at){Option::Some{value:b}=>a==b,Option::None{}=>false,},Option::None{}=>false,};at=at+1usize;same && at<byte_len(left)}same
}
"#.to_owned()
}
fn quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}
fn install(label: &str, app: &str) -> std::path::PathBuf {
    install_schema(label, SCHEMA, app)
}
fn install_schema(label: &str, schema: &str, app: &str) -> std::path::PathBuf {
    install_policy(label, schema, app, PROFILE)
}
fn install_policy(
    label: &str,
    schema: &str,
    app: &str,
    profile: project::JsonCodecProfile,
) -> std::path::PathBuf {
    let root = fixture(label, schema);
    let project::JsonCodecProfile::NestedResponse {
        max_array_items, ..
    } = profile
    else {
        panic!("response policy")
    };
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let generated = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "r.root",
            profile,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "r.root",
            &generated,
            profile,
        )?;
        for changed in [
            generated.replace(
                &format!("count <= {max_array_items}usize"),
                &format!("count <= {}usize", max_array_items + 1),
            ),
            generated.replace("value.config.label", "value.config.missing"),
        ] {
            assert_ne!(changed, generated);
            assert_eq!(
                project::verify_json_codec_source_with_profile(
                    &revision,
                    "src/schema.spx",
                    "r.root",
                    &changed,
                    profile
                )
                .unwrap_err()[0]
                    .code,
                "SPX-J180"
            );
        }
        let project::JsonCodecProfile::NestedResponse {
            max_string_bytes,
            max_array_items,
        } = profile
        else {
            unreachable!()
        };
        for changed in [
            project::JsonCodecProfile::NestedResponse {
                max_string_bytes: if max_string_bytes == 64 {
                    63
                } else {
                    max_string_bytes + 1
                },
                max_array_items,
            },
            project::JsonCodecProfile::NestedResponse {
                max_string_bytes,
                max_array_items: if max_array_items == 256 {
                    255
                } else {
                    max_array_items + 1
                },
            },
        ] {
            assert_eq!(
                project::verify_json_codec_source_with_profile(
                    &revision,
                    "src/schema.spx",
                    "r.root",
                    &generated,
                    changed
                )
                .unwrap_err()[0]
                    .code,
                "SPX-J180"
            );
        }
        assert_eq!(
            std::fs::read_to_string(root.join("src/schema.spx")).unwrap(),
            canonical(schema)
        );
        Ok(generated)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), generated).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(app)).unwrap();
    root
}
fn qualify(root: &std::path::Path) {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert!(snapshot
            .retain_revision()
            .semantic_graph()
            .contains("r.root.json.nested-response.encode"));
        let options = project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
            .map_err(|error| vec![error])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(724)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::super::compile_and_run_c(&c, root, optimization, "724");
        }
        let bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        Ok(())
    })
    .unwrap();
    let host = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/owned_data/owned_leaf_vec/host.js");
    let output = Command::new("node")
        .arg(host)
        .arg(root.join("app.wasm"))
        .args(["0", "724", "none"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn nested_response_borrowed_unicode_rows_exact_and_one_short_output_agree_on_backends() {
    let expected = "{\"config\":{\"label\":\"é\\u0000\",\"retry\":18446744073709551615},\"rows\":[{\"sku\":\"\\\"\\\\\",\"count\":255}],\"ok\":false}";
    let app = imports()
        + &format!(
            r#"
@id("consumer.main") fn main()->i64{{
let label=string_concat("é",string_from_char(char_from_i64(0)));
let row=Row{{sku:"\"\\",count:255u8}};
let report=Report{{config:Config{{label:label,retry:18446744073709551615usize}},rows:vec_push<Row>(vec_with_capacity<Row>(1usize),row),ok:false}};
let required=encoded_len(report);let short=encode(report,{}usize);
let refused=match own short{{Encoded::Refused{{required:n}}=>n=={}usize,Encoded::Encoded{{text:unexpected}}=>false,}};
let full=encode(report,required);let expected={};
let same=match own full{{Encoded::Refused{{required:n}}=>false,Encoded::Encoded{{text:actual}}=>equal(string_as_str(actual),string_as_str(expected)),}};
let alive=vec_len<Row>(report.rows)==1usize && byte_len(str_as_bytes(string_as_str(report.config.label)))==3usize;
if required=={}usize && refused && same && alive{{724}}else{{0}}
}}
"#,
            expected.len() - 1,
            expected.len(),
            quote(expected),
            expected.len()
        );
    let root = install("nested-response-exact", &app);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_response_empty_arrays_and_actual_string_array_plus_one_refuse_before_publication() {
    let app = imports()
        + r#"
@id("consumer.row") fn row()->Row{Row{sku:"",count:0u8}}
@id("consumer.main") fn main()->i64{
let empty=Report{config:Config{label:"",retry:0usize},rows:vec_with_capacity<Row>(0usize),ok:true};
let n=encoded_len(empty);let encoded=encode(empty,n);let expected="{\"config\":{\"label\":\"\",\"retry\":0},\"rows\":[],\"ok\":true}";
let empty_ok=match own encoded{Encoded::Refused{required:a}=>false,Encoded::Encoded{text:b}=>equal(string_as_str(b),string_as_str(expected)),};
let invalid=Report{config:Config{label:"12345678901234567",retry:0usize},rows:vec_with_capacity<Row>(0usize),ok:true};
let bad=encode(invalid,131072usize);let string_ok=match own bad{Encoded::Refused{required:c}=>c==18446744073709551615usize,Encoded::Encoded{text:d}=>false,};
let mut rows=vec_with_capacity<Row>(9usize);let mut at=0usize;
while at<9usize{rows=vec_push<Row>(rows,row());at=at+1usize;at<9usize}
let oversized=Report{config:Config{label:"",retry:0usize},rows:rows,ok:true};
let bad_count=encode(oversized,131072usize);let count_ok=match own bad_count{Encoded::Refused{required:e}=>e==18446744073709551615usize,Encoded::Encoded{text:f}=>false,};
if empty_ok && string_ok && count_ok{724}else{0}
}
"#;
    let root = install("nested-response-refusals", &app);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_response_nested_sibling_scalar_and_copy_record_vectors_preserve_order() {
    let schema = SCHEMA
        .replace("sku:string", "sku:i64")
        .replace("retry:usize", "retry:Vec<usize>");
    let expected = "{\"config\":{\"label\":\"x\",\"retry\":[18446744073709551615,0]},\"rows\":[{\"sku\":-9223372036854775808,\"count\":255}],\"ok\":true}";
    let app = imports()
        + &format!(
            r#"
@id("consumer.main") fn main()->i64{{
let first=vec_push<usize>(vec_with_capacity<usize>(2usize),18446744073709551615usize);let retries=vec_push<usize>(first,0usize);
let row=Row{{sku:-9223372036854775808,count:255u8}};let rows=vec_push<Row>(vec_with_capacity<Row>(1usize),row);
let report=Report{{config:Config{{label:"x",retry:retries}},rows:rows,ok:true}};
let required=encoded_len(report);let full=encode(report,required);let expected={};
let same=match own full{{Encoded::Refused{{required:n}}=>false,Encoded::Encoded{{text:actual}}=>equal(string_as_str(actual),string_as_str(expected)),}};
if required=={}usize && same && vec_get<usize>(report.config.retry,0usize)==18446744073709551615usize{{724}}else{{0}}
}}
"#,
            quote(expected),
            expected.len()
        );
    let root = install_schema("nested-response-siblings", &schema, &app);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_request_to_response_uses_same_contract_and_detached_owners() {
    let root = fixture("nested-codec-roundtrip", SCHEMA);
    let decoder = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::derive_json_codec_source_with_profile(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "r.root",
            project::JsonCodecProfile::NestedRequest {
                max_string_bytes: 16,
                max_array_items: 8,
            },
        )
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), decoder).unwrap();
    let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::derive_json_codec_source_with_profile(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "r.root",
            PROFILE,
        )
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), source).unwrap();
    let wire = "{\"config\":{\"label\":\"é\\u0000\",\"retry\":0},\"rows\":[{\"sku\":\"a\",\"count\":255}],\"ok\":true}";
    let app = imports().replace(
        "@id(\"consumer.equal\")",
        r#"use type @id("r.root.json.nested.decode-result") from consumer.schema as Decoded;
use function @id("r.root.json.nested.decode") from consumer.schema as decode;
@id("consumer.equal")"#,
    ) + &format!(
        r#"
@id("consumer.main") fn main()->i64{{
let input={};let outcome=decode(array_as_slice(input),{}usize);
match own outcome{{Decoded::Error{{code,offset,field}}=>0,Decoded::Ready{{value}}=>{{
let required=encoded_len(value);let full=encode(value,required);let expected={};
match own full{{Encoded::Refused{{required:n}}=>0,Encoded::Encoded{{text:actual}}=>if required=={}usize && equal(string_as_str(actual),string_as_str(expected)){{724}}else{{0}},}}
}},}}
}}
"#,
        array(wire.as_bytes()),
        wire.len(),
        quote(wire),
        wire.len()
    );
    std::fs::write(root.join("src/app.spx"), canonical(&app)).unwrap();
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_response_256_two_string_rows_and_direct_string_use_existing_owner_limits() {
    let schema = SCHEMA.replace("count:u8", "count:string");
    let row_wire = "{\"sku\":\"1234567890123456\",\"count\":\"abcdefghijklmnop\"}";
    let expected = format!(
        "{{\"config\":{{\"label\":\"x\",\"retry\":0}},\"rows\":[{}],\"ok\":true}}",
        vec![row_wire; 256].join(",")
    );
    assert!(expected.len() < 131072);
    let app = imports()
        + &format!(
            r#"
@id("consumer.row") fn row()->Row{{Row{{sku:"1234567890123456",count:"abcdefghijklmnop"}}}}
@id("consumer.main") fn main()->i64{{
let mut rows=vec_with_capacity<Row>(256usize);let mut at=0usize;
while at<256usize{{rows=vec_push<Row>(rows,row());at=at+1usize;at<256usize}}
let report=Report{{config:Config{{label:"x",retry:0usize}},rows:rows,ok:true}};
let required=encoded_len(report);let result=encode(report,required);let expected={};
let same=match own result{{Encoded::Refused{{required:n}}=>false,Encoded::Encoded{{text:actual}}=>equal(string_as_str(actual),string_as_str(expected)),}};
if same && required=={}usize && vec_len<Row>(report.rows)==256usize{{724}}else{{0}}
}}
"#,
            quote(&expected),
            expected.len()
        );
    let root = install_policy(
        "nested-response-512-leaves",
        &schema,
        &app,
        project::JsonCodecProfile::NestedResponse {
            max_string_bytes: 16,
            max_array_items: 256,
        },
    );
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_response_escaped_output_cap_refuses_without_rendering_partial_text() {
    let schema = SCHEMA.replace("count:u8", "count:string");
    let app = imports()
        + r#"
@id("consumer.text") fn text()->string{
let mut text="";let mut at=0usize;
while at<64usize{text=string_concat(text,string_from_char(char_from_i64(0)));at=at+1usize;at<64usize}text
}
@id("consumer.main") fn main()->i64{
let seed=Row{sku:text(),count:text()};let seeds=vec_push<Row>(vec_with_capacity<Row>(1usize),seed);
let mut rows=vec_with_capacity<Row>(256usize);let mut at=0usize;
while at<256usize{let row=vec_clone_at<Row>(seeds,0usize);rows=vec_push<Row>(rows,row);at=at+1usize;at<256usize}
let report=Report{config:Config{label:"",retry:0usize},rows:rows,ok:true};
let required=encoded_len(report);let result=encode(report,18446744073709551615usize);
let refused=match own result{Encoded::Refused{required:n}=>n==18446744073709551615usize,Encoded::Encoded{text:actual}=>false,};
if refused && required==18446744073709551615usize && vec_len<Row>(report.rows)==256usize{724}else{0}
}
"#;
    let root = install_policy(
        "nested-response-output-cap",
        &schema,
        &app,
        project::JsonCodecProfile::NestedResponse {
            max_string_bytes: 64,
            max_array_items: 256,
        },
    );
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_response_selected_exact_eight_and_late_dynamic_string_plus_one_settle_owners() {
    let schema = SCHEMA.replace("retry:usize", "retry:Vec<usize>");
    let row = "{\"sku\":\"1234567890123456\",\"count\":1}";
    let expected = format!(
        "{{\"config\":{{\"label\":\"\",\"retry\":[]}},\"rows\":[{}],\"ok\":true}}",
        vec![row; 8].join(",")
    );
    let app = imports()
        + &format!(
            r#"
@id("consumer.row") fn row()->Row{{Row{{sku:string_concat("12345678","90123456"),count:1u8}}}}
@id("consumer.main") fn main()->i64{{
let mut rows=vec_with_capacity<Row>(8usize);let mut at=0usize;
while at<8usize{{rows=vec_push<Row>(rows,row());at=at+1usize;at<8usize}}
let report=Report{{config:Config{{label:"",retry:vec_with_capacity<usize>(0usize)}},rows:rows,ok:true}};
let required=encoded_len(report);let result=encode(report,required);let expected={};
let exact=match own result{{Encoded::Refused{{required:n}}=>false,Encoded::Encoded{{text:actual}}=>equal(string_as_str(actual),string_as_str(expected)),}};
let mut later=vec_with_capacity<Row>(8usize);let mut next=0usize;
while next<7usize{{later=vec_push<Row>(later,row());next=next+1usize;next<7usize}}
let bad_text=string_concat(string_concat("12345678","90123456"),"x");
let final_rows=vec_push<Row>(later,Row{{sku:bad_text,count:1u8}});
let invalid=Report{{config:Config{{label:"",retry:vec_with_capacity<usize>(0usize)}},rows:final_rows,ok:true}};
let invalid_size=encoded_len(invalid);let refused=encode(invalid,131072usize);
let rejected=match own refused{{Encoded::Refused{{required:n}}=>n==18446744073709551615usize,Encoded::Encoded{{text:bad}}=>false,}};
let alive=byte_len(str_as_bytes(vec_field<Row>(invalid.rows,7usize,"sku")))==17usize && vec_field<Row>(invalid.rows,0usize,"count")==1u8;
if exact && required=={}usize && invalid_size==18446744073709551615usize && rejected && alive{{724}}else{{0}}
}}
"#,
            quote(&expected),
            expected.len()
        );
    let root = install_schema("nested-response-selected-bound", &schema, &app);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_response_new_authenticated_schema_refuses_prior_artifact_claim() {
    let root = fixture("nested-response-source-drift", SCHEMA);
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::derive_json_codec_source_with_profile(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "r.root",
            PROFILE,
        )
    })
    .unwrap();
    let changed = canonical(&SCHEMA.replace("retry:usize", "attempts:usize"));
    std::fs::write(root.join("src/schema.spx"), &changed).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert_eq!(
            project::verify_json_codec_source_with_profile(
                &snapshot.retain_revision(),
                "src/schema.spx",
                "r.root",
                &generated,
                PROFILE
            )
            .unwrap_err()[0]
                .code,
            "SPX-J180"
        );
        assert_eq!(
            std::fs::read_to_string(root.join("src/schema.spx")).unwrap(),
            changed
        );
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
