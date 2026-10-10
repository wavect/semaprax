//! Plain UTF-8 owned values, exact policy replay, and independent backend gates.
use super::*;

#[path = "utf8/stream.rs"]
mod stream;
#[path = "utf8/max_shape.rs"]
mod max_shape;

const SCHEMA: &str = r#"module consumer.schema;
@id("unicode.row") record Row {
 @id("unicode.row.number") number:i64,
 @id("unicode.row.text") text:string,
}
@id("unicode.input") record Request {
 @id("unicode.input.labels") labels:Vec<string>,
 @id("unicode.input.rows") rows:Vec<Row>,
}
@id("consumer.schema.anchor") fn anchor()->i64{0}
"#;

fn imports() -> &'static str {
    r#"module consumer.app;
use type @id("unicode.input.json.utf8.owned-result") from consumer.schema as Outcome;
use type @id("unicode.row.json.view-encode") from consumer.schema as Encoded;
use type @id("unicode.row") from consumer.schema as Row;
use function @id("unicode.input.json.utf8.owned.decode") from consumer.schema as decode;
use function @id("unicode.input.json.utf8.owned.encode") from consumer.schema as encode;
use function @id("unicode.input.json.utf8.owned.encoded-len") from consumer.schema as encoded_len;
"#
}

fn install(label: &str, app: &str, bound: usize) -> std::path::PathBuf {
    install_schema(label, SCHEMA, app, bound, "unicode.input")
}

fn install_schema(
    label: &str,
    schema: &str,
    app: &str,
    bound: usize,
    root_id: &str,
) -> std::path::PathBuf {
    let root = fixture(label, schema);
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let profile = project::JsonCodecProfile::Utf8OwnedRequest {
            max_string_bytes: bound,
        };
        let result = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            root_id,
            profile,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            root_id,
            &result,
            profile,
        )?;
        assert_eq!(canonical(&result), result);
        assert!(project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            root_id,
            &result,
            project::JsonCodecProfile::Utf8OwnedRequest {
                max_string_bytes: bound - 1
            }
        )
        .is_err());
        assert!(project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            root_id,
            &result,
            project::JsonCodecProfile::OwnedRequest
        )
        .is_err());
        Ok(result)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), generated).unwrap();
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
        assert!(snapshot
            .retain_revision()
            .semantic_graph()
            .contains("core.num.char_from_i64"));
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        // Large admitted payloads use the existing configurable fuel ceiling;
        // the codec does not change the default fuel or any physical capacity.
        let options = project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
            .map_err(|error| vec![error])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(727)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::super::compile_and_run_c(&c, root, optimization, "727");
        }
        let wasm =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        std::fs::write(root.join("app.wasm"), wasm).unwrap();
        Ok(())
    })
    .unwrap();
    wasm_run(root, 0, 727, "none");
}

fn roundtrip(input: &[u8], expected: &[u8]) -> String {
    format!(
        r#"@id("consumer.detached") fn detached()->Outcome {{let raw={};let owner=bytes_copy(array_as_slice(raw));decode(bytes_as_slice(owner))}}
@id("consumer.main") fn main()->i64 {{let outcome=detached();match own outcome{{
Outcome::Error{{code,offset,field}}=>0,
Outcome::Decoded{{labels,rows}}=>{{let required=encoded_len(labels,rows);
let short=encode(labels,rows,{}usize);let short_ok=match own short{{Encoded::Refused{{required:count}}=>count=={}usize,Encoded::Encoded{{text}}=>false,}};
let full=encode(labels,rows,{}usize);match own full{{Encoded::Refused{{required}}=>0,Encoded::Encoded{{text}}=>{{
let wanted={};let actual=str_as_bytes(string_as_str(text));let mut at=0usize;
let mut equal=byte_len(actual)==byte_len(array_as_slice(wanted));
while equal && at<byte_len(actual){{equal=match byte_get(actual,at){{Option::Some{{value:a}}=>match byte_get(array_as_slice(wanted),at){{Option::Some{{value:b}}=>a==b,Option::None{{}}=>false,}},Option::None{{}}=>false,}};at=at+1usize;equal && at<byte_len(actual)}}
if required=={}usize && short_ok && equal{{727}}else{{0}}
}},}}
}},}}}}
"#,
        array(input),
        expected.len() - 1,
        expected.len(),
        expected.len(),
        array(expected),
        expected.len()
    )
}

#[test]
fn utf8_owned_request_roundtrips_nul_raw_and_escaped_scalars_and_repeated_empty_values() {
    let input = "{\"rows\":[{\"text\":\"é\\u0000\\ud83d\\ude00\\n\\\"\\\\\",\"number\":-1},{\"number\":0,\"text\":\"\"},{\"number\":0,\"text\":\"\"}],\"labels\":[\"\",\"\",\"\\u00e9\",\"é\"]}";
    let expected = "{\"labels\":[\"\",\"\",\"é\",\"é\"],\"rows\":[{\"number\":-1,\"text\":\"é\\u0000😀\\n\\\"\\\\\"},{\"number\":0,\"text\":\"\"},{\"number\":0,\"text\":\"\"}]}";
    let root = install(
        "utf8-json-owned",
        &(imports().to_owned() + &roundtrip(input.as_bytes(), expected.as_bytes())),
        64,
    );
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn utf8_owned_request_preserves_grammar_priority_and_exact_schema_offsets() {
    let mut app = imports().to_owned()
        + r#"@id("consumer.error") fn error(input:borrow Slice<u8>,wanted:i64,at:usize,field:i64)->i64 {
let outcome=decode(input);match own outcome{Outcome::Decoded{labels,rows}=>1,Outcome::Error{code,offset,field:actual}=>if code==wanted && offset==at && actual==field{0}else{1},}}
@id("consumer.main") fn main()->i64{let mut bad=0;
"#;
    for (index, (bytes, code, offset, field)) in [
        (b"{".as_slice(), 1, 1, 0),
        (b"{}", 3, 2, 1),
        (br#"{"labels":[],"labels":[]}"#, 2, 13, 1),
        (br#"{"\u006cabels":[],"labels":[]}"#, 2, 18, 1),
        (br#"{"\u00e9":0}"#, 4, 1, 0),
        (br#"{"labels":null}"#, 5, 10, 1),
        (br#"{"labels":["12345"]}"#, 6, 11, 1),
        // Complete grammar is selected before the earlier semantic overbound.
        (br#"{"labels":["12345"],"rows":[}"#, 1, 28, 0),
        (br#"{"labels":["","","","","","","","",""]}"#, 8, 35, 1),
        ("{\"labels\":[\"ééé\"]}".as_bytes(), 6, 11, 1),
        (b"{\"labels\":[\"\xc0\"]}", 1, 12, 0),
        (br#"{"labels":["\ud800"]}"#, 1, 12, 0),
        (br#"{"labels":["\udc00"]}"#, 1, 12, 0),
        (
            br#"{"rows":[{"text":"x","number":1.5}],"labels":[]}"#,
            6,
            30,
            1,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        app.push_str(&format!("let raw_{index}={};bad=bad+error(array_as_slice(raw_{index}),{code},{offset}usize,{field});\n",array(bytes)));
    }
    // A nonempty record array with an empty first array is plain data here.
    app.push_str(&format!("let valid={};let outcome=decode(array_as_slice(valid));bad=bad+match own outcome{{Outcome::Error{{code,offset,field}}=>1,Outcome::Decoded{{labels,rows}}=>if vec_len<Row>(rows)==1usize && vec_len<string>(labels)==0usize{{0}}else{{1}},}};if bad==0{{727}}else{{0-bad}}}}",array(br#"{"labels":[],"rows":[{"text":"","number":0}]}"#)));
    let root = install("utf8-json-errors", &app, 4);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

fn write_fixed(app: &mut String, bytes: &[u8]) {
    use std::fmt::Write as _;
    for byte in bytes {
        writeln!(
            app,
            "buffer=bytes_set(buffer,cursor,{byte}u8);cursor=cursor+1usize;"
        )
        .unwrap();
    }
}

fn full_bound_app(escaped: bool, canonical_length: usize) -> String {
    let escaped_length = canonical_length + 264 * 64 * 5;
    let mut app = imports().to_owned()+&format!("@id(\"consumer.witness\") fn witness(escaped:bool)->Bytes{{let length=if escaped{{{escaped_length}usize}}else{{{canonical_length}usize}};let mut buffer=bytes_zeroed(length);let mut cursor=0usize;\n");
    write_fixed(&mut app, b"{\"labels\":[");
    app.push_str("let mut label=0usize;while label<8usize{let _ = if label>0usize{buffer=bytes_set(buffer,cursor,44u8);cursor=cursor+1usize;true}else{true};buffer=bytes_set(buffer,cursor,34u8);cursor=cursor+1usize;let mut part=0usize;while part<64usize{let _ = if escaped{\n");
    write_fixed(&mut app, br"\u0041");
    app.push_str("true}else{buffer=bytes_set(buffer,cursor,65u8);cursor=cursor+1usize;true};part=part+1usize;part<64usize}buffer=bytes_set(buffer,cursor,34u8);cursor=cursor+1usize;label=label+1usize;label<8usize}\n");
    write_fixed(&mut app, b"],\"rows\":[");
    app.push_str("let mut row=0usize;while row<256usize{let _ = if row>0usize{buffer=bytes_set(buffer,cursor,44u8);cursor=cursor+1usize;true}else{true};\n");
    write_fixed(&mut app, b"{\"number\":0,\"text\":\"");
    app.push_str("let mut part=0usize;while part<64usize{let _ = if escaped{\n");
    write_fixed(&mut app, br"\u0041");
    app.push_str("true}else{buffer=bytes_set(buffer,cursor,65u8);cursor=cursor+1usize;true};part=part+1usize;part<64usize}\n");
    write_fixed(&mut app, b"\"}");
    app.push_str("row=row+1usize;row<256usize}\n");
    write_fixed(&mut app, b"]}");
    app.push_str(&format!(r#"buffer}}
@id("consumer.main") fn main()->i64{{
let buffer=witness({escaped});let canonical=witness(false);let input=bytes_as_slice(buffer);let expected=bytes_as_slice(canonical);let decoded=decode(input);
match own decoded{{Outcome::Error{{code,offset,field}}=>0,Outcome::Decoded{{labels,rows}}=>{{
let first=vec_clone_at<string>(labels,0usize);let last=vec_clone_at<Row>(rows,255usize);
let count_ok=vec_len<string>(labels)==8usize && vec_len<Row>(rows)==256usize && string_len(first)==64 && string_len(last.text)==64;
let required=encoded_len(labels,rows);let outcome=encode(labels,rows,{canonical_length}usize);
match own outcome{{Encoded::Refused{{required}}=>0,Encoded::Encoded{{text}}=>{{let output=str_as_bytes(string_as_str(text));
let mut same=byte_len(output)==byte_len(expected);let mut index=0usize;
while same && index<byte_len(expected){{same=match byte_get(expected,index){{Option::Some{{value:a}}=>match byte_get(output,index){{Option::Some{{value:b}}=>a==b,Option::None{{}}=>false,}},Option::None{{}}=>false,}};index=index+1usize;same && index<byte_len(expected)}}
if count_ok && required=={canonical_length}usize && same{{727}}else{{0}}
}},}}
}},}}}}
"#));
    app
}

#[test]
fn utf8_owned_request_all_264_values_at_64_decoded_bytes_obey_existing_limits() {
    // One bounded Bytes allocation per witness precedes its construction loops.
    // Both complete requests run through every backend: raw ASCII maximizes
    // scalar work, while escaped ASCII exceeds the external borrowed root cap
    // and must remain an internal, owned Bytes view through the private call.
    let expected = serde_json::json!({
        "labels": vec!["A".repeat(64); 8],
        "rows": (0..256).map(|_| serde_json::json!({"number":0,"text":"A".repeat(64)})).collect::<Vec<_>>()
    });
    let length = serde_json::to_vec(&expected).unwrap().len();
    assert!(length < 65536);
    assert!(length + 264 * 64 * 5 > 65536);
    assert!(length + 264 * 64 * 5 <= 131072);
    for escaped in [false, true] {
        let app = full_bound_app(escaped, length).replace(
            "module consumer.app;",
            "module consumer.app;\npermit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}\n",
        ) + "\n@id(\"consumer.command\") fn command()->i64{main()}\n";
        let root = install(&format!("utf8-json-full-bound-{escaped}"), &app, 64);
        let manifest = MANIFEST
            .replace("owned-data-api.v1", "language-command-io.owned-data.v1")
            .replace("web = []", "web = [\"consumer.command\"]")
            + "\n[command]\nfunction=\"consumer.command\"\ninput=\"argv-utf8+stdin-stream.v1\"\n[capabilities]\nrequired=[\"process.args.read\",\"process.stderr.write\",\"process.stdin.read\",\"process.stdout.write\"]\n";
        std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
        qualify(&root);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn utf8_owned_request_partial_push_and_encoder_clone_refusals_leave_no_owners() {
    let input=br#"{"labels":["\u0000","\ud83d\ude00"],"rows":[{"number":0,"text":"x"},{"number":1,"text":"y"}]}"#;
    let app = imports().to_owned()
        + &format!(
            r#"@id("consumer.main") fn main()->i64{{let raw={};let decoded=decode(array_as_slice(raw));
match own decoded{{Outcome::Error{{code,offset,field}}=>0,Outcome::Decoded{{labels,rows}}=>{{let outcome=encode(labels,rows,131072usize);match own outcome{{Encoded::Refused{{required}}=>0,Encoded::Encoded{{text}}=>727,}}}},}}}}
"#,
            array(input)
        );
    let root = install("utf8-json-partial", &app, 64);
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        Ok(())
    })
    .unwrap();
    for at in 1..=4 {
        wasm_run(&root, 15, 0, &format!("codec-push-{at}"));
    }
    wasm_run(&root, 15, 0, "string-clone-allocation");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn utf8_owned_request_exact_four_byte_scalar_bound_and_unrelated_field_names() {
    let input = br#"{"labels":["\udbff\udfff"],"rows":[]}"#;
    let expected = "{\"labels\":[\"\u{10ffff}\"],\"rows\":[]}";
    let root = install(
        "utf8-json-four-byte",
        &(imports().to_owned() + &roundtrip(input, expected.as_bytes())),
        4,
    );
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
    let schema = SCHEMA
        .replace("Row", "Note")
        .replace("Request", "Catalog")
        .replace("unicode.row", "catalog.note")
        .replace("unicode.input", "catalog.root")
        .replace("labels", "titles")
        .replace("rows", "entries")
        .replace("number", "position")
        .replace("text", "message");
    let input = "{\"entries\":[{\"message\":\"\\u6c49\",\"position\":0}],\"titles\":[\"汉\"]}";
    let expected = "{\"titles\":[\"汉\"],\"entries\":[{\"position\":0,\"message\":\"汉\"}]}";
    let app = (imports().to_owned() + &roundtrip(input.as_bytes(), expected.as_bytes()))
        .replace("unicode.row", "catalog.note")
        .replace("unicode.input", "catalog.root")
        .replace("consumer.schema as Row", "consumer.schema as Note")
        .replace("labels", "titles")
        .replace("rows", "entries");
    let root = install_schema("utf8-json-catalog", &schema, &app, 64, "catalog.root");
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn utf8_owned_encoder_rejects_actual_overbound_values_before_publication() {
    let app = imports().to_owned()
        + r#"@id("consumer.main") fn main()->i64 {
let labels=vec_push<string>(vec_with_capacity<string>(1usize),"ééé");let rows=vec_with_capacity<Row>(0usize);
let length=encoded_len(labels,rows);let encoded=encode(labels,rows,131072usize);
match own encoded{Encoded::Encoded{text}=>0,Encoded::Refused{required}=>if length==18446744073709551615usize && required==length{727}else{0},}}
"#;
    let root = install("utf8-json-encoder-domain", &app, 4);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn utf8_quote_bulk_copy_preserves_escaped_scalars_and_private_input_bounds() {
    let mut app = imports().to_owned()
        + r#"use function @id("unicode.row.json.utf8.quoted-render") from consumer.schema as quote;
use function @id("unicode.row.json.utf8.quoted-len") from consumer.schema as quote_len;
@id("consumer.quote-check") fn check(text:borrow str,wanted:borrow Slice<u8>,required:usize)->bool {
let length=quote_len(text);let outcome=quote(text);let actual=str_as_bytes(string_as_str(outcome));
let mut equal=length==required && byte_len(actual)==byte_len(wanted);let mut at=0usize;
while equal && at<byte_len(actual){equal=match byte_get(actual,at){Option::Some{value:a}=>match byte_get(wanted,at){Option::Some{value:b}=>a==b,Option::None{}=>false,},Option::None{}=>false,};at=at+1usize;equal && at<byte_len(actual)}
equal
}
@id("consumer.main") fn main()->i64{let mut bad=0;
"#;
    let plain_limit = "x".repeat(64);
    let over_limit = "x".repeat(65);
    let cases = [
        ("\"\"".to_owned(), b"\"\"".to_vec(), 2usize),
        (
            "\"ordinary ASCII\"".to_owned(),
            b"\"ordinary ASCII\"".to_vec(),
            16,
        ),
        ("\"é😀\"".to_owned(), "\"é😀\"".as_bytes().to_vec(), 8),
        (
            "string_from_char(char_from_i64(65279))".to_owned(),
            "\"\u{feff}\"".as_bytes().to_vec(),
            5,
        ),
        (
            "string_from_char(char_from_i64(0))".to_owned(),
            br#""\u0000""#.to_vec(),
            8,
        ),
        (
            "string_from_char(char_from_i64(31))".to_owned(),
            br#""\u001f""#.to_vec(),
            8,
        ),
        (
            "string_from_char(char_from_i64(34))".to_owned(),
            br#""\"""#.to_vec(),
            4,
        ),
        (
            "string_from_char(char_from_i64(92))".to_owned(),
            br#""\\""#.to_vec(),
            4,
        ),
        (
            "string_from_char(char_from_i64(10))".to_owned(),
            br#""\n""#.to_vec(),
            4,
        ),
        (
            "string_concat(\"é😀\",string_from_char(char_from_i64(0)))".to_owned(),
            "\"é😀\\u0000\"".as_bytes().to_vec(),
            14,
        ),
        (
            format!("\"{plain_limit}\""),
            format!("\"{plain_limit}\"").into_bytes(),
            66,
        ),
        (format!("\"{over_limit}\""), Vec::new(), usize::MAX),
        (format!("\"{}\"", "é".repeat(33)), Vec::new(), usize::MAX),
    ];
    for (index, (text, expected, length)) in cases.into_iter().enumerate() {
        app.push_str(&format!("let value_{index}={text};bad=bad+(if check(string_as_str(value_{index}),array_as_slice({}),{length}usize){{0}}else{{1}});\n", array(&expected)));
    }
    app.push_str("if bad==0{727}else{0}}\n");
    let root = install("utf8-json-quote-bulk", &app, 64);
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}
