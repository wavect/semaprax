//! Actual owned request values, detached from input, on the same three backends.
use super::*;

#[path = "owned/stream.rs"]
mod stream;
#[path = "owned/public_example.rs"]
mod public_example;
#[path = "owned/stream_utf8_public_example.rs"]
mod stream_utf8_public_example;

const VALID: &[u8] = br#"{"patients":[{"id":"P2","arrival":2,"service":5,"priority":1,"deadline":8},{"id":"\u00501","arrival":1,"service":4,"priority":0,"deadline":9}],"servers":["S2","\u00531"]}"#;
const EXPECTED: &[u8] = br#"{"servers":["S1","S2"],"patients":[{"id":"P1","arrival":2,"service":4,"priority":0,"deadline":9},{"id":"P2","arrival":2,"service":5,"priority":1,"deadline":8}]}"#;

fn imports() -> &'static str {
    r#"module consumer.app;
use type @id("application.patient") from consumer.schema as Patient;
use type @id("application.request.json.owned-result") from consumer.schema as Outcome;
use type @id("application.patient.json.view-encode") from consumer.schema as Encoded;
use function @id("application.request.json.owned.decode") from consumer.schema as decode;
use function @id("application.request.json.owned.encode") from consumer.schema as encode;
use function @id("application.request.json.owned.encoded-len") from consumer.schema as encoded_len;
"#
}

fn app(errors: bool) -> String {
    let mut source = imports().to_owned();
    source.push_str(&format!(r#"@id("consumer.detached") fn detached()->Outcome {{
let raw={};let input=bytes_copy(array_as_slice(raw));decode(bytes_as_slice(input))
}}
@id("consumer.change") fn change(value:own Patient)->Patient {{
match own value {{Patient{{id,arrival,service,priority,deadline}}=>Patient{{id:id,arrival:arrival+1,service:service,priority:priority,deadline:deadline}},}}
}}
@id("consumer.error") fn error(input:borrow Slice<u8>,wanted:i64,at:usize,field:i64)->i64 {{
let result=decode(input);match own result {{Outcome::Decoded{{servers,patients}}=>1,Outcome::Error{{code,offset,field:actual}}=>if code==wanted && offset==at && actual==field{{0}}else{{1}},}}
}}
@id("consumer.main") fn main()->i64 {{
let mut failures=0;
"#, array(VALID)));
    if errors {
        for (i, (bytes, code, at, field)) in [
            (b"{".as_slice(), 1, 1, 0),
            (b"{}", 3, 2, 1),
            (br#"{"servers":[],"servers":[]}"#, 2, 14, 1),
            (br#"{"extra":0}"#, 4, 1, 0),
            (br#"{"servers":true}"#, 5, 11, 1),
            (br#"{"servers":[""]}"#, 6, 12, 1),
            (br#"{"servers":["12345678901234567"]}"#, 6, 12, 1),
            (br#"{"servers":["A","\u0041"]}"#, 10, 16, 1),
            (br#"{"servers":["\u0000"]}"#, 6, 12, 1),
        ]
        .into_iter()
        .enumerate()
        {
            source.push_str(&format!("let bad_{i}={};failures=failures+error(array_as_slice(bad_{i}),{code},{at}usize,{field});\n", array(bytes)));
        }
    }
    let invalid_controls = if errors {
        r#"
let duplicate_words=vec_push<string>(vec_push<string>(vec_with_capacity<string>(2usize),"S"),"S");
let duplicate_text=encode(duplicate_words,rows,131072usize);
let duplicate_ok=match own duplicate_text{Encoded::Refused{required}=>required==18446744073709551615usize,Encoded::Encoded{text}=>false,};
let bad_row=Patient{id:"wrong value",arrival:0,service:1,priority:0,deadline:0};
let bad_rows=vec_push<Patient>(vec_with_capacity<Patient>(1usize),bad_row);
let bad_text=encode(words,bad_rows,131072usize);
let bad_ok=match own bad_text{Encoded::Refused{required}=>required==18446744073709551615usize,Encoded::Encoded{text}=>false,};
let repeated=vec_push<Patient>(vec_push<Patient>(vec_with_capacity<Patient>(2usize),vec_clone_at<Patient>(rows,0usize)),vec_clone_at<Patient>(rows,0usize));
let repeated_text=encode(words,repeated,131072usize);
let repeated_ok=match own repeated_text{Encoded::Refused{required}=>required==18446744073709551615usize,Encoded::Encoded{text}=>false,};
let invalid_ok=duplicate_ok && bad_ok && repeated_ok;
"#
    } else {
        "let invalid_ok=true;\n"
    };
    source.push_str(&format!(r#"let result=detached();
failures=failures+match own result {{Outcome::Error{{code,offset,field}}=>1,
Outcome::Decoded{{servers,patients}}=>{{
let sorted=vec_sort_owned<Patient>(patients);let words=vec_sort_owned<string>(servers);
let first=vec_clone_at<Patient>(sorted,0usize);let changed=change(first);
let rows=vec_replace<Patient>(sorted,0usize,changed);
let needed=encoded_len(words,rows);
{invalid_controls}
let short=encode(words,rows,{}usize);
let short_ok=match own short{{Encoded::Refused{{required}}=>required=={}usize,Encoded::Encoded{{text}}=>false,}};
let full=encode(words,rows,{}usize);
let same=match own full{{Encoded::Refused{{required}}=>false,Encoded::Encoded{{text}}=>{{
let wanted={};let bytes=str_as_bytes(string_as_str(text));let mut index=0usize;
let mut equal=byte_len(bytes)==byte_len(array_as_slice(wanted));
while equal && index<byte_len(bytes){{equal=match byte_get(bytes,index){{Option::None{{}}=>false,Option::Some{{value:a}}=>match byte_get(array_as_slice(wanted),index){{Option::None{{}}=>false,Option::Some{{value:b}}=>a==b,}},}};index=index+1usize;equal && index<byte_len(bytes)}}equal
}},}};
if needed=={}usize && short_ok && same && invalid_ok{{0}}else{{1}}
}},}};
if failures==0{{726}}else{{0-failures}}
}}
"#, EXPECTED.len()-1, EXPECTED.len(), EXPECTED.len(), array(EXPECTED), EXPECTED.len()));
    source
}

fn install(label: &str, schema: &str, application: &str, root_id: &str) -> std::path::PathBuf {
    let root = fixture(label, schema);
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let generated = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            root_id,
            project::JsonCodecProfile::OwnedRequest,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            root_id,
            &generated,
            project::JsonCodecProfile::OwnedRequest,
        )?;
        assert_eq!(canonical(&generated), generated);
        let forged = generated.replace("length <= 16", "length <= 17");
        assert_ne!(generated, forged);
        assert!(
            project::verify_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                root_id,
                &forged,
                project::JsonCodecProfile::OwnedRequest
            )
            .is_err()
        );
        assert!(
            project::verify_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                root_id,
                &generated,
                project::JsonCodecProfile::RequestViews
            )
            .is_err()
        );
        Ok(generated)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), generated).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(application)).unwrap();
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
        let revision = snapshot.retain_revision();
        assert!(revision.semantic_graph().contains("json.owned.decode"));
        assert!(revision.semantic_graph().contains("core.vec.clone-at"));
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert_eq!(
            snapshot
                .execute_entry(&project::ProjectExecutionOptions::default())?
                .outcome(),
            &project::ProjectExecutionOutcome::Returned(726)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::super::compile_and_run_c(&c, root, optimization, "726");
        }
        let wasm =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        std::fs::write(root.join("app.wasm"), wasm).unwrap();
        Ok(())
    })
    .unwrap();
    wasm_run(root, 0, 726, "none");
}

#[test]
fn owned_request_detaches_input_then_sorts_updates_and_encodes_on_three_backends() {
    let root = install(
        "owned-json-detached",
        views::SCHEMA,
        &app(true),
        "application.request",
    );
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn unrelated_owned_catalog_uses_different_field_names_and_identity_position() {
    let schema = views::SCHEMA
        .replace("Patient", "Item")
        .replace("Request", "Catalog")
        .replace("application.patient", "catalog.item")
        .replace("application.request", "catalog.input")
        .replace("servers", "labels")
        .replace("patients", "items")
        .replace("arrival", "quantity")
        .replace("service", "price")
        .replace("priority", "rank")
        .replace("deadline", "restock")
        .replace(" id:string,", " sku:string,");
    // Move the owned field between scalar fields; admission is independent of order.
    let owned = " @id(\"catalog.item.id\") sku:string,\n";
    assert!(schema.contains(owned));
    let schema = schema
        .replace(owned, "")
        .replace("price:i64,\n", &format!("price:i64,\n{owned}"));
    let input =
        br#"{"items":[{"sku":"X","quantity":1,"price":2,"rank":3,"restock":4}],"labels":["L"]}"#;
    let expected =
        br#"{"labels":["L"],"items":[{"quantity":1,"price":2,"sku":"X","rank":3,"restock":4}]}"#;
    let expected_source = format!(
        "\"{}\"",
        std::str::from_utf8(expected).unwrap().replace('"', "\\\"")
    );
    let application = format!(
        r#"module consumer.app;
use type @id("catalog.input.json.owned-result") from consumer.schema as Outcome;
use type @id("catalog.item.json.view-encode") from consumer.schema as Encoded;
use function @id("catalog.input.json.owned.decode") from consumer.schema as decode;
use function @id("catalog.input.json.owned.encode") from consumer.schema as encode;
@id("consumer.main") fn main()->i64{{let raw={};let result=decode(array_as_slice(raw));
match own result{{Outcome::Error{{code,offset,field}}=>0,Outcome::Decoded{{labels,items}}=>{{
let rendered=encode(labels,items,{}usize);match own rendered{{Encoded::Refused{{required}}=>0,Encoded::Encoded{{text}}=>if text=={}{{726}}else{{0}},}}
}},}}
}}
"#,
        array(input),
        expected.len(),
        expected_source
    );
    let root = install("owned-json-catalog", &schema, &application, "catalog.input");
    qualify(&root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn owned_request_partial_materialization_and_encoder_clone_failures_settle_every_owner() {
    let root = install(
        "owned-json-partial",
        views::SCHEMA,
        &app(false),
        "application.request",
    );
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        Ok(())
    })
    .unwrap();
    // Two identifiers, then two actual authored records. Refuse every group
    // commit position and retain all already-constructed owners for cleanup.
    for at in 1..=4 {
        wasm_run(&root, 15, 0, &format!("codec-push-{at}"));
    }
    let encode_only = imports().to_owned()
        + &format!(
            r#"
@id("consumer.main") fn main()->i64 {{let raw={};let decoded=decode(array_as_slice(raw));
match own decoded{{Outcome::Error{{code,offset,field}}=>0,Outcome::Decoded{{servers,patients}}=>{{
let result=encode(servers,patients,131072usize);match own result{{Encoded::Refused{{required}}=>0,Encoded::Encoded{{text}}=>726,}}
}},}}
}}
"#,
            array(VALID)
        );
    std::fs::write(root.join("src/app.spx"), canonical(&encode_only)).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        Ok(())
    })
    .unwrap();
    wasm_run(&root, 15, 0, "string-clone-allocation");
    std::fs::remove_dir_all(root).unwrap();
}
