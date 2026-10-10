//! Authenticated v30 stream composition; raw grammar and normalized schema stay separate.
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
fn utf8_stream_owned_request_preserves_empty_first_array_unicode_and_unbounded_whitespace() {
    let schema = SCHEMA.replace(
        "module consumer.schema;",
        &format!("module consumer.schema;\n{PERMITS}"),
    );
    let root = fixture("utf8-json-stream", &schema);
    let manifest = command_manifest("language-command-io.owned-data.v1");
    std::fs::write(root.join("semaprax.toml"), &manifest).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(&("module consumer.app;\n".to_owned() + PERMITS + "@id(\"consumer.command\") fn command()->i64{0}\n@id(\"consumer.main\") fn main()->i64{0}"))).unwrap();
    let generated = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let profile = project::JsonCodecProfile::StreamUtf8OwnedRequest {
            max_string_bytes: 16,
        };
        let source = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "unicode.input",
            profile,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "unicode.input",
            &source,
            profile,
        )?;
        assert!(project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "unicode.input",
            &source,
            project::JsonCodecProfile::StreamOwnedRequest,
        )
        .is_err());
        Ok(source)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), generated).unwrap();
    let application = imports().to_owned()
        + PERMITS
        + r#"
use type @id("unicode.input.json.stream-input") from consumer.schema as Input;
use function @id("unicode.input.json.stream.normalize") from consumer.schema as normalize;
@id("consumer.receive") fn receive()->Outcome uses{process.stdin.read}{
let normalized=normalize();match own normalized{
Input::Error{code,offset,field}=>Outcome::Error{code:code,offset:offset,field:field},
Input::Ready{bytes,length}=>decode(byte_range(bytes_as_slice(bytes),0usize,length)),
}}
@id("consumer.command") fn command()->i64 uses{process.stdin.read,process.stdout.write}{
let outcome=receive();match own outcome{
Outcome::Error{code,offset,field}=>2,
Outcome::Decoded{labels,rows}=>{
let encoded=encode(labels,rows,131072usize);match own encoded{
Encoded::Refused{required}=>2,Encoded::Encoded{text}=>{
let bytes=str_as_bytes(string_as_str(text));let written=stdout_write(bytes);
if written==byte_len(bytes){0}else{1}
},}
},}}
@id("consumer.main") fn main()->i64{0}
"#;
    std::fs::write(root.join("src/app.spx"), canonical(&application)).unwrap();
    let binary = root.join(format!("utf8-json{}", std::env::consts::EXE_SUFFIX));
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_native(&binary)
    })
    .unwrap();
    for (input, expected) in [
        (
            br#"{"labels":[],"rows":[{"text":"","number":0}]}"#.as_slice(),
            br#"{"labels":[],"rows":[{"number":0,"text":""}]}"#.as_slice(),
        ),
        (
            br#"{"rows":[{"text":"\u0000\ud83d\ude00","number":-1}],"labels":["","","\u00e9"]}"#,
            "{\"labels\":[\"\",\"\",\"é\"],\"rows\":[{\"number\":-1,\"text\":\"\\u0000😀\"}]}"
                .as_bytes(),
        ),
    ] {
        for raw in [
            input.to_vec(),
            [vec![b' '; 70_000], input.to_vec(), vec![b'\n'; 5_000]].concat(),
        ] {
            let output = execute(&binary, &raw);
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, expected);
            assert!(output.stderr.is_empty());
        }
    }
    for input in [
        b"{".as_slice(),
        b"{}",
        br#"{"labels":[],"labels":[],"rows":[]}"#,
        br#"{"labels":["12345678901234567"],"rows":[]}"#,
        br#"{"labels":["12345678901234567"],"rows":[}"#,
        br#"{"labels":["\ud800"],"rows":[]}"#,
        b"{\"labels\":[\"\xc0\"],\"rows\":[]}",
    ] {
        let output = execute(&binary, input);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
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
