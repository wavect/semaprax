//! Exact physical copy witnesses. Existing bulk_utf8 tests own value/status parity.
use super::{checked, codegen, compile_and_run, fs, symbol, Ordering, OBSERVER, SERIAL, STDIO};
use semaprax::{format, hir, parse, project};

const SOURCE: &str = r#"module native.bulk_utf8;
@id("s.copy") fn copy(input:borrow Slice<u8>)->string {string_from_utf8(input)}
@id("s.detached") fn detached()->string {
    let raw=[65u8,0u8,195u8,169u8];
    let owner=bytes_copy(array_as_slice(raw));
    string_from_utf8(bytes_as_slice(owner))
}
@id("s.main") fn main()->i64 {
    let raw=[65u8];let text=copy(array_as_slice(raw));string_len(text)
}
"#;
const COPY_OBSERVER: &str = include_str!("bulk_utf8/copies.c");

fn instrument(generated: &str) -> String {
    format!(
        "{STDIO}\n#define FIXTURE_TRACK_CALLOC\n{OBSERVER}\n{COPY_OBSERVER}\n#define main fixture_generated_main\n{generated}\n#undef main\n#undef memcpy\n#undef malloc\n#undef calloc\n#undef free\n"
    )
}

fn directory(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "native-bulk-{label}-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    root
}

#[test]
fn bulk_constructor_copies_one_final_payload_and_preserves_borrowed_input() {
    let generated = codegen::emit_c(&checked(SOURCE)).unwrap();
    let probe = format!(
        "{}\n#define FIXTURE_COPY {}\n#define FIXTURE_DETACHED {}\n{}",
        instrument(&generated),
        symbol("s.copy"),
        symbol("s.detached"),
        include_str!("bulk_utf8/constructor.c")
    );
    compile_and_run("bulk-utf8-copy", &probe, false);
}

const SCHEMA: &str = r#"module consumer.schema;
@id("probe.row") record Row {
 @id("probe.row.text") text:string,
 @id("probe.row.number") number:i64,
}
@id("probe.request") record Request {
 @id("probe.request.labels") labels:Vec<string>,
 @id("probe.request.rows") rows:Vec<Row>,
}
@id("probe.anchor") fn anchor()->i64{0}
"#;

const APP: &str = r#"module consumer.app;
use function @id("probe.row.json.utf8.materialize-text") from consumer.schema as materialize;
use function @id("probe.request.json.utf8.owned.decode") from consumer.schema as decode;
use type @id("probe.request.json.utf8.owned-result") from consumer.schema as Outcome;
use type @id("probe.row") from consumer.schema as Row;
@id("probe.copy") fn copy(input:borrow Slice<u8>)->string {
    materialize(input,0usize,byte_len(input))
}
@id("probe.decode") fn decode_size(input:borrow Slice<u8>)->i64 {
    let outcome=decode(input);match own outcome {
        Outcome::Error{code,offset,field}=>0,
        Outcome::Decoded{labels,rows}=>{
            if vec_len<string>(labels)==1usize && vec_len<Row>(rows)==1usize{42}else{0}
        },
    }
}
@id("consumer.main") fn main()->i64 {
    let raw=[34u8,65u8,34u8];let text=copy(array_as_slice(raw));
    if string_len(text)==1{decode_size(array_as_slice(raw))}else{0}
}
"#;

fn derived_decoder() -> String {
    let root = directory("derived-source");
    fs::create_dir(root.join("src")).unwrap();
    fs::write(
        root.join("semaprax.toml"),
        include_str!("bulk_utf8/semaprax.toml"),
    )
    .unwrap();
    let canonical = |text: &str| format::canonical(&parse(text, "probe.spx").unwrap());
    fs::write(root.join("src/schema.spx"), canonical(SCHEMA)).unwrap();
    fs::write(
        root.join("src/app.spx"),
        canonical("module consumer.app;@id(\"consumer.main\") fn main()->i64{0}"),
    )
    .unwrap();
    fs::write(
        root.join("src/tests.spx"),
        canonical("module consumer.tests;@id(\"consumer.tests\") fn main()->i64{0}"),
    )
    .unwrap();
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let policy = project::JsonCodecProfile::Utf8OwnedRequest {
            max_string_bytes: 64,
        };
        let source = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "probe.request",
            policy,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "probe.request",
            &source,
            policy,
        )?;
        assert_eq!(source, canonical(&source));
        assert!(source.contains("string_from_utf8"));
        Ok(source)
    })
    .unwrap();
    fs::write(root.join("src/schema.spx"), generated).unwrap();
    fs::write(root.join("src/app.spx"), canonical(APP)).unwrap();
    // Compile the actual checked derivation and linked scanner; no copied or
    // guessed generated body is accepted as the fast-path witness.
    let emitted = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])
    })
    .unwrap();
    // This directory was created here for the authenticated source fixture;
    // the physical-probe cleanup helper owns a different fixed file inventory.
    fs::remove_dir_all(&root).unwrap();
    emitted
}

#[test]
fn generated_unescaped_json_decode_uses_one_copy_per_materialized_string() {
    let generated = derived_decoder();
    let probe = format!(
        "{}\n#define FIXTURE_COPY {}\n#define FIXTURE_DECODE {}\n{}",
        instrument(&generated),
        symbol("probe.copy"),
        symbol("probe.decode"),
        include_str!("bulk_utf8/decoder.c")
    );
    compile_and_run("bulk-utf8-generated-decode", &probe, false);
}

#[test]
fn stream_bulk_copy_rechecks_epoch_and_preserves_internal_and_foreign_bounds() {
    let stream_source = SOURCE.replacen(
        "module native.bulk_utf8;",
        "module native.bulk_utf8;\npermit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }",
        1,
    );
    let stream_source = stream_source.replace(
        "fn main()->i64 {",
        "fn main()->i64 uses {process.stdin.read} {\nlet mut reader=stdin_stream_open();let chunk_size={let chunk=stdin_stream_chunk(reader);byte_len(chunk)};reader=stdin_stream_next(reader);",
    );
    let program = hir::resolve(&checked(&stream_source)).unwrap();
    let generated = codegen::emit_hir_c_with_stdin_stream_text(&program, "s.main").unwrap();
    let source = format!(
        "{}\n#define FIXTURE_COPY {}\n{}",
        instrument(&generated),
        symbol("s.copy"),
        include_str!("bulk_utf8/epoch.c")
    );
    epoch::compile_and_run(&source);
}

#[path = "bulk_utf8/epoch.rs"]
mod epoch;

#[path = "bulk_utf8/paired_physical.rs"]
mod paired_physical;
