//! Real streamed request input; all raw chunk splits preserve the same result.

use std::io::Write as _;
use std::process::{Command, Stdio};

use super::*;

#[path = "stream_native/boundary.rs"]
mod boundary;

const PERMITS: &str =
    "permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}\n";

fn app() -> String {
    r#"module consumer.app;
use type @id("application.request.json.stream-input") from consumer.schema as StreamInput;
use type @id("application.request.json.request-result") from consumer.schema as RequestResult;
use type @id("application.patient.json.view-encode") from consumer.schema as Encoded;
use type @id("application.patient.json.view") from consumer.schema as PatientView;
use function @id("application.request.json.stream.normalize") from consumer.schema as normalize;
use function @id("application.request.json.request.decode") from consumer.schema as decode;
use function @id("application.request.json.request.encode") from consumer.schema as encode;
"#
    .to_owned()
        + PERMITS
        + r#"@id("consumer.diagnostic") fn diagnostic(kind:borrow str,code:i64,offset:usize)->i64 uses{process.stderr.write}{
let prefix=string_concat(string_from_str(kind),":");
let number=string_concat(prefix,string_from_i64(code));
let colon=string_concat(number,":");
let position=string_concat(colon,string_from_usize(offset));
let line=string_concat(position,"\n");
let view=string_as_str(line);let written=stderr_write(str_as_bytes(view));2
}
@id("consumer.command") fn command()->i64 uses{process.stdin.read,process.stdout.write,process.stderr.write}{
let normalized=normalize();
match own normalized{
StreamInput::Error{code,offset,field}=>{let label="stream";diagnostic(string_as_str(label),code,offset)},
StreamInput::Ready{bytes,length}=>{
let full=bytes_as_slice(bytes);let source=byte_range(full,0usize,length);
let decoded=decode(source);
match own decoded{
RequestResult::Error{code,offset,field}=>{let label="schema";diagnostic(string_as_str(label),code,offset)},
RequestResult::Decoded{servers,patients}=>{
let mut valid=true;let mut index=0usize;
while valid && index<vec_len<PatientView>(patients){
let value=vec_get<PatientView>(patients,index);
valid=value.arrival>=0 && value.arrival<=1000000 && value.service>=1 && value.service<=100000 && value.priority>=0 && value.priority<=9 && value.deadline>=0 && value.deadline<=1000000;
index=index+1usize;valid && index<vec_len<PatientView>(patients)
}
if !valid{let label="domain";diagnostic(string_as_str(label),6,0usize)}else{
let rendered=encode(source,servers,patients,131072usize);
match own rendered{
Encoded::Refused{required}=>{let label="schema";diagnostic(string_as_str(label),9,required)},
Encoded::Encoded{text}=>{
let line=string_concat(text,"\n");let view=string_as_str(line);let output=str_as_bytes(view);let written=stdout_write(output);if written==byte_len(output){0}else{1}
},
}
}
},
}
},
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
    // Concurrent drain prevents a maximal encoded request from deadlocking a
    // pipe while the parent is still writing the streamed input.
    let mut writer = child.stdin.take().unwrap();
    let input = input.to_vec();
    let thread = std::thread::spawn(move || writer.write_all(&input).unwrap());
    let output = child.wait_with_output().unwrap();
    thread.join().unwrap();
    output
}

fn escaped(value: &str) -> String {
    value.bytes().map(|byte| format!("\\u{byte:04x}")).collect()
}

fn request(count: usize, servers: usize, escaped_wire: bool) -> Vec<u8> {
    let text = |value: &str| {
        if escaped_wire {
            escaped(value)
        } else {
            value.to_owned()
        }
    };
    let mut patients = Vec::new();
    for index in 0..count {
        let id = format!("P{index:015}");
        patients.push(format!(
            "{{\"{}\":\"{}\",\"{}\":0,\"{}\":100000,\"{}\":9,\"{}\":1000000}}",
            text("id"),
            text(&id),
            text("arrival"),
            text("service"),
            text("priority"),
            text("deadline")
        ));
    }
    let servers = (0..servers)
        .map(|index| format!("\"{}\"", text(&format!("S{index}"))))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"{}\":[{}],\"{}\":[{}]}}",
        text("servers"),
        servers,
        text("patients"),
        patients.join(",")
    )
    .into_bytes()
}

#[test]
fn streamed_request_codecs_accept_full_raw_domain_and_preserve_late_grammar_priority() {
    let root = fixture("json-full-stream", views::SCHEMA);
    let manifest=MANIFEST.replace("owned-data-api.v1","language-command-io.stream-data.v2").replace("web = []","web = [\"consumer.command\"]")+"\n[command]\nfunction = \"consumer.command\"\ninput = \"argv-utf8+stdin-stream.v1\"\n\n[capabilities]\nrequired = [\"process.args.read\",\"process.stderr.write\",\"process.stdin.read\",\"process.stdout.write\"]\n";
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    let schema = views::SCHEMA.replace(
        "module consumer.schema;",
        "module consumer.schema; permit {process.stdin.read}",
    );
    std::fs::write(root.join("src/schema.spx"), canonical(&schema)).unwrap();
    std::fs::write(root.join("src/app.spx"),canonical(&("module consumer.app;".to_owned()+PERMITS+"@id(\"consumer.command\") fn command()->i64{0} @id(\"consumer.main\") fn main()->i64{0}"))).unwrap();
    let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::derive_json_codec_source_with_profile(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "application.request",
            project::JsonCodecProfile::StreamRequestViews,
        )
    })
    .unwrap();
    assert_eq!(canonical(&source), source);
    std::fs::write(root.join("src/schema.spx"), source).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(&app())).unwrap();
    let binary = root.join("stream-request");
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        snapshot.build_native(&binary)
    })
    .unwrap();
    let traced = boundary::build_observed_provider(&root);
    let valid = request(1, 1, true);
    let mut expected = request(1, 1, false);
    expected.push(b'\n');
    for split in 0..=valid.len() {
        let mut input = vec![b' '; 4096 - split];
        input.extend_from_slice(&valid);
        let output = boundary::execute_observed(&traced, &input);
        assert_eq!(
            output.status.code(),
            Some(0),
            "split {split}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected, "split {split}");
        assert!(output.stderr.is_empty());
    }
    boundary::lexical_splits(&traced);
    let domain = execute(&binary, b"  {\"unknown\":0}");
    assert_eq!(domain.status.code(), Some(2));
    assert!(domain.stdout.is_empty());
    assert_eq!(domain.stderr, b"schema:4:1\n");
    let maximal = request(256, 8, true);
    assert!(maximal.len() > 65536);
    let mut expected = request(256, 8, false);
    expected.push(b'\n');
    let output = execute(&binary, &maximal);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
    let mut whitespace = vec![b' '; 70000];
    whitespace.extend_from_slice(&request(256, 8, false));
    whitespace.extend(std::iter::repeat_n(b'\n', 70000));
    let output = execute(&binary, &whitespace);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected);
    let too_many = request(257, 1, false);
    let item = too_many
        .windows(b"{\"id\":\"P000000000000256\"".len())
        .position(|window| window == b"{\"id\":\"P000000000000256\"")
        .unwrap();
    let nine_servers = request(0, 9, false);
    let ninth = nine_servers
        .windows(b"\"S8\"".len())
        .position(|window| window == b"\"S8\"")
        .unwrap();
    let no_servers = br#"{"servers":[],"patients":[{"id":"P1","arrival":0,"service":1,"priority":0,"deadline":1}]}"#.to_vec();
    let bad_domain = br#"{"servers":["S1"],"patients":[{"id":"P1","arrival":0,"service":0,"priority":0,"deadline":1}]}"#.to_vec();
    for (invalid, expected) in [
        (too_many, format!("schema:8:{item}\n")),
        (nine_servers, format!("schema:8:{ninth}\n")),
        (
            no_servers.clone(),
            format!("schema:11:{}\n", no_servers.len()),
        ),
        (bad_domain, "domain:6:0\n".to_owned()),
    ] {
        let output = execute(&binary, &invalid);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, expected.as_bytes());
    }
    // A huge syntactically valid number exhausts only semantic storage. A later
    // malformed token must still select grammar, not the pending refusal.
    let mut late = b"{\"oversized\":".to_vec();
    late.extend(std::iter::repeat_n(b'1', 140000));
    late.extend_from_slice(b",?");
    let at = late.len() - 1;
    let output = execute(&binary, &late);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, format!("stream:1:{at}\n").as_bytes());
    let mut valid_overflow = b"{\"oversized\":".to_vec();
    valid_overflow.extend(std::iter::repeat_n(b'1', 140000));
    valid_overflow.push(b'}');
    let output = execute(&binary, &valid_overflow);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"stream:9:131072\n");
    // Structural EOF retains precedence over an earlier malformed UTF-8 token.
    let invalid_utf8 = b"{\"servers\":[\"\xc0\xaf\"],\"patients\":[]";
    let output = execute(&binary, invalid_utf8);
    assert_eq!(
        output.stderr,
        format!("stream:1:{}\n", invalid_utf8.len()).as_bytes()
    );
    let observed = boundary::execute_observed(&traced, &maximal);
    assert_eq!(observed.status.code(), Some(0));
    assert_eq!(observed.stdout, expected);
    assert!(observed.stderr.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
