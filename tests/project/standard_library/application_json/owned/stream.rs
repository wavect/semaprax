//! V30 process composition retires the normalized byte owner before encoding.
use super::*;
use std::io::Write as _;
use std::process::Stdio;

const PERMITS: &str =
    "permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}\n";

fn execute(binary: &std::path::Path, input: &[u8]) -> std::process::Output {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut writer = child.stdin.take().unwrap();
    let input = input.to_vec();
    let write = std::thread::spawn(move || writer.write_all(&input).unwrap());
    let output = child.wait_with_output().unwrap();
    write.join().unwrap();
    output
}

#[test]
fn owned_stream_request_drops_input_before_native_decision_and_output() {
    let schema = views::SCHEMA.replace(
        "module consumer.schema;",
        &format!("module consumer.schema;\n{PERMITS}"),
    );
    let root = fixture("owned-json-stream", &schema);
    let manifest = command_manifest("language-command-io.owned-data.v1");
    std::fs::write(root.join("semaprax.toml"), &manifest).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(&("module consumer.app;\n".to_owned() + PERMITS + "@id(\"consumer.command\") fn command()->i64{0}\n@id(\"consumer.main\") fn main()->i64{0}"))).unwrap();
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        project::derive_json_codec_source_with_profile(
            &snapshot.retain_revision(),
            "src/schema.spx",
            "application.request",
            project::JsonCodecProfile::StreamOwnedRequest,
        )
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), generated).unwrap();
    // This fixture consumes error payloads only as status 2: the generated
    // normalizer and decoder retain their distinct raw/normalized offsets.
    let application = imports().to_owned()
        + PERMITS
        + r#"
use type @id("application.request.json.stream-input") from consumer.schema as Input;
use function @id("application.request.json.stream.normalize") from consumer.schema as normalize;
@id("consumer.receive") fn receive()->Outcome uses{process.stdin.read}{
let normalized=normalize();match own normalized{
Input::Error{code,offset,field}=>Outcome::Error{code:code,offset:offset,field:field},
Input::Ready{bytes,length}=>decode(byte_range(bytes_as_slice(bytes),0usize,length)),
}}
@id("consumer.change") fn change(value:own Patient)->Patient{
match own value{Patient{id,arrival,service,priority,deadline}=>Patient{id:id,arrival:arrival+1,service:service,priority:priority,deadline:deadline},}}
@id("consumer.command") fn command()->i64 uses{process.stdin.read,process.stdout.write}{
let outcome=receive();match own outcome{
Outcome::Error{code,offset,field}=>2,
Outcome::Decoded{servers,patients}=>{
let words=vec_sort_owned<string>(servers);let sorted=vec_sort_owned<Patient>(patients);
if vec_len<Patient>(sorted)==0usize{2}else{
let first=vec_clone_at<Patient>(sorted,0usize);let updated=change(first);
let rows=vec_replace<Patient>(sorted,0usize,updated);let encoded=encode(words,rows,131072usize);
match own encoded{Encoded::Refused{required}=>2,Encoded::Encoded{text}=>{
let bytes=str_as_bytes(string_as_str(text));let written=stdout_write(bytes);if written==byte_len(bytes){0}else{1}
},}
}
},}}
@id("consumer.main") fn main()->i64{0}
"#;
    std::fs::write(root.join("src/app.spx"), canonical(&application)).unwrap();
    let binary = root.join(format!("owned-json{}", std::env::consts::EXE_SUFFIX));
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_native(&binary)
    })
    .unwrap();
    for input in [
        VALID.to_vec(),
        [vec![b' '; 70_000], VALID.to_vec(), vec![b'\n'; 5_000]].concat(),
    ] {
        for _ in 0..3 {
            let output = execute(&binary, &input);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, EXPECTED);
            assert!(output.stderr.is_empty());
        }
    }
    for input in [b"{".as_slice(), b"{}", br#"{"servers":["A","\u0041"],"patients":[]}"#,
        br#"{"servers":["A"],"patients":[{"id":"P","arrival":1e0,"service":1,"priority":0,"deadline":0}]}"#] {
        let output = execute(&binary, input);
        assert_eq!(output.status.code(), Some(2));assert!(output.stdout.is_empty());
    }
    // Ordinary generated code cannot make the same source a v29 program.
    std::fs::write(
        root.join("semaprax.toml"),
        manifest.replace(
            "language-command-io.owned-data.v1",
            "language-command-io.stream-data.v2",
        ),
    )
    .unwrap();
    assert!(project::with_authenticated_project(&root.join("semaprax.toml"), |_| Ok(())).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
