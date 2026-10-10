//! Maximum decoded shape, independent wire bytes, and exact preflight parity.
use super::*;

const PERMITS: &str =
    "permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}\n";

fn command_manifest(root: &std::path::Path) {
    // Match the existing maximum-shape owner: private internal Bytes views,
    // unchanged v30 command admission and unchanged physical/fuel ceilings.
    let manifest = super::super::command_manifest("language-command-io.owned-data.v1");
    assert_eq!(
        project::ProjectManifest::parse(&manifest)
            .unwrap()
            .to_canonical_toml(),
        manifest
    );
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
}

fn qualify_shape(root: &std::path::Path, decoder_id: &str) {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(snapshot
            .retain_revision()
            .semantic_graph()
            .contains(decoder_id));
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        let options = project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
            .map_err(|error| vec![error])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(727)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::super::super::compile_and_run_c(&c, root, optimization, "727");
        }
        let bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        Ok(())
    })
    .unwrap();
    super::wasm_run(root, 0, 727, "none");
}

fn imports_for(namespace: &str, profile: &str) -> String {
    format!(
        r#"module consumer.app;
{PERMITS}
use type @id("{namespace}.input.json.{profile}owned-result") from consumer.schema as Outcome;
use type @id("{namespace}.row.json.view-encode") from consumer.schema as Encoded;
use type @id("{namespace}.row") from consumer.schema as Row;
use function @id("{namespace}.input.json.{profile}owned.decode") from consumer.schema as decode;
use function @id("{namespace}.input.json.{profile}owned.encode") from consumer.schema as encode;
use function @id("{namespace}.input.json.{profile}owned.encoded-len") from consumer.schema as encoded_len;
"#
    )
}

fn install_identifiers(app: &str) -> std::path::PathBuf {
    let schema = SCHEMA.replace("unicode.", "identifier.");
    let root = fixture("json-maximum-identifiers", &schema);
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let profile = project::JsonCodecProfile::OwnedRequest;
        let source = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "identifier.input",
            profile,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "identifier.input",
            &source,
            profile,
        )?;
        assert_eq!(canonical(&source), source);
        assert!(project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "identifier.input",
            &source,
            project::JsonCodecProfile::Utf8OwnedRequest {
                max_string_bytes: 16
            },
        )
        .is_err());
        Ok(source)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), generated).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(app)).unwrap();
    command_manifest(&root);
    root
}

fn append_check(app: &mut String, input_len: usize, expected_len: usize) {
    app.push_str(&format!(
        r#"@id("consumer.main") fn main()->i64 {{
let buffer=witness();let canonical=expected();
let decoded=decode(byte_range(bytes_as_slice(buffer),0usize,{input_len}usize));
// A late grammar fault after the complete maximum shape must remain a typed
// grammar error at its absolute supplied-input offset, never partial success.
let bad=bytes_set(buffer,{input_len}usize,63u8);let malformed=decode(bytes_as_slice(bad));
let malformed_ok=match own malformed{{Outcome::Decoded{{labels,rows}}=>false,
Outcome::Error{{code,offset,field}}=>code==1 && offset=={input_len}usize && field==0,}};
match own decoded{{Outcome::Error{{code,offset,field}}=>0,Outcome::Decoded{{labels,rows}}=>{{
let first=vec_clone_at<string>(labels,0usize);let last=vec_clone_at<Row>(rows,255usize);
let count_ok=vec_len<string>(labels)==8usize && vec_len<Row>(rows)==256usize;
let required=encoded_len(labels,rows);
let short=encode(labels,rows,{}usize);
let short_ok=match own short{{Encoded::Encoded{{text}}=>false,Encoded::Refused{{required:size}}=>size=={expected_len}usize,}};
let full=encode(labels,rows,{expected_len}usize);match own full{{Encoded::Refused{{required}}=>0,Encoded::Encoded{{text}}=>{{
let actual=str_as_bytes(string_as_str(text));let wanted=byte_range(bytes_as_slice(canonical),0usize,{expected_len}usize);
let mut same=byte_len(actual)==byte_len(wanted);let mut at=0usize;
while same && at<byte_len(wanted){{same=match byte_get(actual,at){{Option::None{{}}=>false,Option::Some{{value:a}}=>match byte_get(wanted,at){{Option::None{{}}=>false,Option::Some{{value:b}}=>a==b,}},}};at=at+1usize;same && at<byte_len(wanted)}}
if count_ok && string_len(first)==__VALUE_BYTES__ && string_len(last.text)==__VALUE_BYTES__ && required=={expected_len}usize && short_ok && malformed_ok && same{{727}}else{{0}}
}},}}
}},}}}}
@id("consumer.command") fn command()->i64{{main()}}
"#,
        expected_len - 1,
    ));
}

fn append_identifier(app: &mut String, prefix: u8, index: &str, escaped: bool) {
    super::write_fixed(app, b"\"");
    app.push_str(&format!(
        "let mut part=0usize;while part<16usize{{let number=i64_from_usize({index});let byte=if part==0usize{{{prefix}}}else{{if part<13usize{{65}}else{{if part==13usize{{48+number/100}}else{{if part==14usize{{48+(number/10)%10}}else{{48+number%10}}}}}}}};\n"
    ));
    if escaped {
        super::write_fixed(app, br"\u00");
        app.push_str("let high=byte/16;let low=byte%16;buffer=bytes_set(buffer,cursor,u8_from_i64(if high<10{48+high}else{87+high}));cursor=cursor+1usize;buffer=bytes_set(buffer,cursor,u8_from_i64(if low<10{48+low}else{87+low}));cursor=cursor+1usize;\n");
    } else {
        app.push_str("buffer=bytes_set(buffer,cursor,u8_from_i64(byte));cursor=cursor+1usize;\n");
    }
    app.push_str("part=part+1usize;part<16usize}\n");
    super::write_fixed(app, b"\"");
}

fn append_expected_identifier(app: &mut String, prefix: u8, index: &str) {
    // Independent canonical oracle: zero-padded decimal formatting, rather
    // than the input writer's positional division/modulo digit extraction.
    super::write_fixed(app, b"\"");
    super::write_fixed(app, &[prefix]);
    super::write_fixed(app, b"AAAAAAAAAAAA");
    app.push_str(&format!("let digits=string_from_usize({index});let bytes=str_as_bytes(string_as_str(digits));let mut padding=byte_len(bytes);while padding<3usize{{buffer=bytes_set(buffer,cursor,48u8);cursor=cursor+1usize;padding=padding+1usize;padding<3usize}}let mut digit=0usize;while digit<byte_len(bytes){{let value=match byte_get(bytes,digit){{Option::Some{{value}}=>value,Option::None{{}}=>0u8,}};buffer=bytes_set(buffer,cursor,value);cursor=cursor+1usize;digit=digit+1usize;digit<byte_len(bytes)}}\n"));
    super::write_fixed(app, b"\"");
}

fn identifier_wire(app: &mut String, function: &str, length: usize, escaped: bool) {
    app.push_str(&format!("@id(\"consumer.{function}\") fn {function}()->Bytes{{let mut buffer=bytes_zeroed({}usize);let mut cursor=0usize;\n", length + 1));
    super::write_fixed(app, b"{\"labels\":[");
    app.push_str("let mut label=0usize;while label<8usize{let _ = if label>0usize{buffer=bytes_set(buffer,cursor,44u8);cursor=cursor+1usize;true}else{true};\n");
    if function == "expected" {
        append_expected_identifier(app, b'S', "label");
    } else {
        append_identifier(app, b'S', "label", escaped);
    }
    app.push_str("label=label+1usize;label<8usize}\n");
    super::write_fixed(app, b"],\"rows\":[");
    app.push_str("let mut row=0usize;while row<256usize{let _ = if row>0usize{buffer=bytes_set(buffer,cursor,44u8);cursor=cursor+1usize;true}else{true};\n");
    super::write_fixed(app, b"{\"number\":0,\"text\":");
    if function == "expected" {
        append_expected_identifier(app, b'P', "row");
    } else {
        append_identifier(app, b'P', "row", escaped);
    }
    super::write_fixed(app, b"}");
    app.push_str("row=row+1usize;row<256usize}\n");
    super::write_fixed(app, b"]}");
    app.push_str("buffer}\n");
}

#[test]
fn owned_identifier_maximum_shape_preserves_exact_16_byte_ids_and_short_output() {
    let value = serde_json::json!({
        "labels": (0..8).map(|n| format!("S{}{:03}", "A".repeat(12), n)).collect::<Vec<_>>(),
        "rows": (0..256).map(|n| serde_json::json!({"number":0,"text":format!("P{}{:03}","A".repeat(12),n)})).collect::<Vec<_>>()
    });
    let canonical = serde_json::to_vec(&value).unwrap();
    let distinct = value["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["text"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(distinct.len(), 256);
    assert!(distinct.iter().all(|text| text.len() == 16));
    for escaped in [false, true] {
        let length = canonical.len() + if escaped { 264 * 16 * 5 } else { 0 };
        assert!(length + 1 <= 131072);
        let mut app = imports_for("identifier", "");
        identifier_wire(&mut app, "witness", length, escaped);
        identifier_wire(&mut app, "expected", canonical.len(), false);
        append_check(&mut app, length, canonical.len());
        let app = app.replace("__VALUE_BYTES__", "16");
        let root = install_identifiers(&app);
        qualify_shape(&root, "identifier.input.json.owned.decode");
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn escaped_token(value: &str, mixed: bool) -> Vec<u8> {
    let mut text = String::from("\"");
    for (index, scalar) in value.chars().enumerate() {
        if mixed && index % 2 == 0 && scalar >= ' ' && scalar != '"' && scalar != '\\' {
            text.push(scalar);
        } else {
            let mut units = [0; 2];
            for unit in scalar.encode_utf16(&mut units) {
                use std::fmt::Write as _;
                write!(text, "\\u{unit:04x}").unwrap();
            }
        }
    }
    text.push('"');
    text.into_bytes()
}

fn append_token(app: &mut String) {
    app.push_str("let mut part=0usize;while part<byte_len(array_as_slice(token)){let byte=match byte_get(array_as_slice(token),part){Option::Some{value}=>value,Option::None{}=>0u8,};buffer=bytes_set(buffer,cursor,byte);cursor=cursor+1usize;part=part+1usize;part<byte_len(array_as_slice(token))}\n");
}

fn unicode_wire(app: &mut String, function: &str, token: &[u8], length: usize) {
    app.push_str(&format!("@id(\"consumer.{function}\") fn {function}()->Bytes{{let token={};let mut buffer=bytes_zeroed({}usize);let mut cursor=0usize;\n", array(token), length + 1));
    super::write_fixed(app, b"{\"labels\":[");
    app.push_str("let mut label=0usize;while label<8usize{let _ = if label>0usize{buffer=bytes_set(buffer,cursor,44u8);cursor=cursor+1usize;true}else{true};\n");
    append_token(app);
    app.push_str("label=label+1usize;label<8usize}\n");
    super::write_fixed(app, b"],\"rows\":[");
    app.push_str("let mut row=0usize;while row<256usize{let _ = if row>0usize{buffer=bytes_set(buffer,cursor,44u8);cursor=cursor+1usize;true}else{true};\n");
    super::write_fixed(app, b"{\"number\":0,\"text\":");
    append_token(app);
    super::write_fixed(app, b"}");
    app.push_str("row=row+1usize;row<256usize}\n");
    super::write_fixed(app, b"]}");
    app.push_str("buffer}\n");
}

#[test]
fn utf8_maximum_shape_preserves_raw_escaped_and_mixed_64_byte_values() {
    // Existing parent tests already cover full raw/escaped ASCII. These three
    // additional witnesses cover the bulk non-ASCII path and mixed scalar,
    // surrogate-pair, NUL/control/quote/backslash paths at the same full shape.
    let unicode = "é😀".repeat(10) + "雪A";
    let mixed = "é😀\0\"\\\n".repeat(6) + "雪A";
    for (label, value, spelling) in [
        ("raw", &unicode, 0),
        ("escaped", &unicode, 1),
        ("mixed", &mixed, 2),
    ] {
        assert_eq!(value.len(), 64);
        let canonical_token = serde_json::to_vec(value).unwrap();
        let input_token = match spelling {
            0 => canonical_token.clone(),
            1 => escaped_token(value, false),
            _ => escaped_token(value, true),
        };
        assert_eq!(
            serde_json::from_slice::<String>(&input_token).unwrap(),
            *value
        );
        let expected = serde_json::json!({
            "labels": vec![value; 8],
            "rows": (0..256).map(|_| serde_json::json!({"number":0,"text":value})).collect::<Vec<_>>()
        });
        let canonical = serde_json::to_vec(&expected).unwrap();
        let punctuation = canonical.len() - 264 * canonical_token.len();
        let length = punctuation + 264 * input_token.len();
        assert!(length + 1 <= 131072);
        assert!(canonical.len() <= 131072);
        let mut app = imports_for("unicode", "utf8.");
        unicode_wire(&mut app, "witness", &input_token, length);
        unicode_wire(&mut app, "expected", &canonical_token, canonical.len());
        append_check(&mut app, length, canonical.len());
        let app = app.replace("__VALUE_BYTES__", "64");
        let root = install(&format!("utf8-json-maximum-{label}"), &app, 64);
        command_manifest(&root);
        qualify_shape(&root, "unicode.input.json.utf8.owned.decode");
        std::fs::remove_dir_all(root).unwrap();
    }
}
