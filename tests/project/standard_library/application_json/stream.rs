//! Genuine v29 process composition; the codec still admits one bounded JSON value.

use std::io::Write as _;
use std::process::{Command, Stdio};

use super::*;

const PERMITS: &str = "permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }\n";

fn manifest() -> String {
    command_manifest("language-command-io.stream-data.v2")
}

fn command_source() -> String {
    imports().to_owned()
        + PERMITS
        + r#"@id("consumer.command") fn command()->i64 uses { process.stdin.read, process.stdout.write } {
    let mut reader=stdin_stream_open();
    let decoded={
        let chunk=stdin_stream_chunk(reader);
        decode(chunk,4096usize)
    };
    reader=stdin_stream_next(reader);
    if !stdin_stream_eof(reader) {2} else {
        match decoded {
            PatientJsonDecode::Error{code,offset,field}=>2,
            PatientJsonDecode::Decoded{value}=>{
                let rows=vec_push<Patient>(vec_with_capacity<Patient>(1usize),value);
                let first=vec_get<Patient>(rows,0usize);
                let updated=Patient{n:first.n,count:first.count+1usize,byte:first.byte,ok:first.ok};
                let rendered=encode(updated,256usize);
                match own rendered {
                    PatientJsonEncode::Refused{required}=>2,
                    PatientJsonEncode::Encoded{text}=>{
                        let view=string_as_str(text);
                        let bytes=str_as_bytes(view);
                        let written=stdout_write(bytes);
                        if written==byte_len(bytes) {0} else {1}
                    },
                }
            },
        }
    }
}
@id("consumer.main") fn main()->i64{0}
"#
}

fn execute(binary: &std::path::Path, input: &[u8]) -> std::process::Output {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn generated_application_json_composes_with_record_vectors_in_v29_stream_command() {
    let root = fixture("json-codec-stream-command", SCHEMA);
    std::fs::write(root.join("semaprax.toml"), manifest()).unwrap();
    // Derivation is checked through the selected command profile before helpers
    // are installed. It cannot make an otherwise inadmissible project valid.
    let initial = "module consumer.app;\n".to_owned()
        + PERMITS
        + "@id(\"consumer.command\") fn command()->i64 {0}\n@id(\"consumer.main\") fn main()->i64 {0}\n";
    std::fs::write(root.join("src/app.spx"), canonical(&initial)).unwrap();
    let derived = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::derive_json_codec_source(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "application.patient",
        )
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), derived).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(&command_source())).unwrap();
    let binary = root.join("json-command");
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        let graph = snapshot.retain_revision();
        assert!(graph
            .semantic_graph()
            .contains("application.patient.json.decode"));
        assert!(graph.semantic_graph().contains("core.vec.push"));
        snapshot.build_native(&binary)
    })
    .unwrap();
    let valid = br#" {"ok":true,"byte":255,"count":41,"\u006e":-7} "#;
    for _ in 0..4 {
        let output = execute(&binary, valid);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            br#"{"n":-7,"count":42,"byte":255,"ok":true}"#
        );
        assert!(output.stderr.is_empty());
    }
    // Typed grammar/schema errors produce no partial output or fabricated row.
    for malformed in [
        b"{\"n\":".as_slice(),
        b"{}",
        br#"{"n":0,"n":1}"#,
        br#"{"n":0,"count":0,"byte":256,"ok":true}"#,
    ] {
        let output = execute(&binary, malformed);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
    std::fs::remove_dir_all(root).unwrap();
}
