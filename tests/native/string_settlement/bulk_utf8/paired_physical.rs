//! Opt-in measurements, never executed by ordinary gates. No guessed counters.
//! Both algorithms pass the same current checked Project/HIR and C emitter.
use super::{directory, instrument, symbol, SCHEMA};
use semaprax::{codegen, format, hir, parse, project};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fmt::Write as _, fs, path::Path, process::Command};

const BASELINE: &str = "fc9e5090daffaada9cdb738a410cd3ae92577785";
const HISTORICAL_SHA256: &str = "c3ece1ef543c28c39f2615cb77c9fa325f15c16b46e1ed1074b79b8a627fdc67";
const OLD: &str = "1e9c886b2f34280ea8d37bae60478d59510853ff";
const LEGACY: &str = include_str!("historical_text.spx");
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn canonical(text: &str) -> String {
    format::canonical(&parse(text, "paired.spx").unwrap())
}

// Only the balanced definitions identified by persistent identity are replaced.
fn definition(source: &str, identity: &str) -> std::ops::Range<usize> {
    let marker = format!("@id(\"{identity}\")");
    assert_eq!(source.matches(&marker).count(), 1);
    let start = source.find(&marker).unwrap();
    let opening = start + source[start..].find('{').unwrap();
    let (mut depth, mut quoted, mut escaped) = (0, false, false);
    for (offset, byte) in source.as_bytes()[opening..].iter().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                quoted = false;
            }
        } else if *byte == b'"' {
            quoted = true;
        } else if *byte == b'{' {
            depth += 1;
        } else if *byte == b'}' {
            depth -= 1;
            if depth == 0 {
                return start..opening + offset + 1;
            }
        }
    }
    panic!("unclosed historical definition")
}
fn legacy(current: &str) -> String {
    let expanded = LEGACY
        .replace("__ROW_ID__", "probe.row")
        .replace("__ROW__", "Row")
        .replace("__BOUND__", "64");
    let expanded = canonical(&format!("module consumer.schema;{expanded}"));
    let mut result = current.to_owned();
    for kind in ["materialize-text", "quoted-render"] {
        let id = format!("probe.row.json.utf8.{kind}");
        let old = definition(&expanded, &id);
        let now = definition(&result, &id);
        assert_ne!(&expanded[old.clone()], &result[now.clone()]);
        result.replace_range(now, &expanded[old]);
    }
    result
}
fn fixed(app: &mut String, bytes: &[u8]) {
    for byte in bytes {
        writeln!(
            app,
            "buffer=bytes_set(buffer,cursor,{byte}u8);cursor=cursor+1usize;"
        )
        .unwrap();
    }
}
fn quoted(value: &str, escaped: bool) -> String {
    if !escaped {
        return serde_json::to_string(value).unwrap();
    }
    let mut result = String::from("\"");
    for unit in value.encode_utf16() {
        write!(result, "\\u{unit:04x}").unwrap();
    }
    result.push('"');
    result
}
fn input(value: &str, escaped: bool) -> String {
    let token = quoted(value, escaped);
    format!(
        "{{\"labels\":[{}],\"rows\":[{}]}}",
        vec![token.clone(); 8].join(","),
        vec![format!("{{\"text\":{token},\"number\":7}}"); 256].join(",")
    )
}
fn app(value: &str, escaped: bool) -> String {
    let raw = input(value, escaped);
    assert!(raw.len() <= 131072);
    let token = quoted(value, escaped);
    let mut app = r#"module consumer.app;
use function @id("probe.row.json.utf8.materialize-text") from consumer.schema as materialize;
use function @id("probe.row.json.utf8.quoted-render") from consumer.schema as quote;
use function @id("probe.request.json.utf8.owned.decode") from consumer.schema as decode;
use type @id("probe.request.json.utf8.owned-result") from consumer.schema as Outcome;
use type @id("probe.row") from consumer.schema as Row;
@id("paired.copy") fn copy(input:borrow Slice<u8>)->string{materialize(input,0usize,byte_len(input))}
@id("paired.quote") fn render(input:borrow Slice<u8>)->string{let text=materialize(input,0usize,byte_len(input));quote(string_as_str(text))}
"#.to_owned();
    // One internal Bytes owner; no large foreign slice, no cap override.
    writeln!(app,"@id(\"paired.input\") fn input()->Bytes{{let mut buffer=bytes_zeroed({}usize);let mut cursor=0usize;",raw.len()).unwrap();
    let token_bytes = token
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte}u8"))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(
        app,
        "let token=[{token_bytes}];let token_view=array_as_slice(token);"
    )
    .unwrap();
    let append_token = "let mut part=0usize;while part<byte_len(token_view){let byte=match byte_get(token_view,part){Option::Some{value}=>value,Option::None{}=>0u8,};buffer=bytes_set(buffer,cursor,byte);cursor=cursor+1usize;part=part+1usize;part<byte_len(token_view)}\n";
    fixed(&mut app, b"{\"labels\":[");
    app.push_str("let mut label=0usize;while label<8usize{let _ = if label>0usize{buffer=bytes_set(buffer,cursor,44u8);cursor=cursor+1usize;true}else{true};\n");
    app.push_str(append_token);
    app.push_str("label=label+1usize;label<8usize}\n");
    fixed(&mut app, b"],\"rows\":[");
    app.push_str("let mut row=0usize;while row<256usize{let _ = if row>0usize{buffer=bytes_set(buffer,cursor,44u8);cursor=cursor+1usize;true}else{true};\n");
    fixed(&mut app, b"{\"text\":");
    app.push_str(append_token);
    fixed(&mut app, b",\"number\":7}");
    app.push_str("row=row+1usize;row<256usize}\n");
    fixed(&mut app, b"]}");
    app.push_str("buffer}\n");
    let wanted = value
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte}u8"))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(app,"@id(\"paired.equal\") fn equal(text:borrow str)->bool{{let wanted=[{wanted}];let actual=str_as_bytes(text);let wanted_view=array_as_slice(wanted);let mut same=byte_len(actual)=={}usize;let mut index=0usize;while same && index<{}usize{{same=match byte_get(actual,index){{Option::Some{{value:a}}=>match byte_get(wanted_view,index){{Option::Some{{value:b}}=>a==b,Option::None{{}}=>false,}},Option::None{{}}=>false,}};index=index+1usize;same && index<{}usize}}same}}",value.len(),value.len(),value.len()).unwrap();
    app.push_str(r#"@id("paired.full") fn full()->i64{let owner=input();let decoded=decode(bytes_as_slice(owner));match own decoded{
Outcome::Error{code,offset,field}=>0,
Outcome::Decoded{labels,rows}=>{let mut same=vec_len<string>(labels)==8usize && vec_len<Row>(rows)==256usize;let mut at=0usize;
while same && at<8usize{let value=vec_clone_at<string>(labels,at);same=equal(string_as_str(value));at=at+1usize;same && at<8usize}
let mut index=0usize;while same && index<256usize{let value=vec_clone_at<Row>(rows,index);same=value.number==7 && equal(string_as_str(value.text));index=index+1usize;same && index<256usize}
if same{264}else{0}},}}
@id("consumer.main") fn main()->i64{let raw=[34u8,65u8,34u8];let copied=copy(array_as_slice(raw));let rendered=render(array_as_slice(raw));if string_len(copied)==1 && string_len(rendered)==3{full()}else{0}}
"#);
    canonical(&app)
}
fn require_baseline_equivalence() {
    let delta = Command::new("git")
        .args([
            "diff",
            "--name-only",
            BASELINE,
            "HEAD",
            "--",
            ".",
            ":(exclude)tests/native/string_settlement/bulk_utf8.rs",
            ":(exclude)tests/native/string_settlement/bulk_utf8/paired_physical.rs",
            ":(exclude)tests/native/string_settlement/bulk_utf8/paired_probe.c",
            ":(exclude)tests/native/string_settlement/bulk_utf8/historical_text.spx",
            ":(exclude)tests/native/string_settlement/bulk_utf8/historical_text.sha256",
        ])
        .output()
        .unwrap();
    assert!(
        delta.status.success() && delta.stdout.is_empty(),
        "baseline source drift: {}",
        String::from_utf8_lossy(&delta.stdout)
    );
}
fn derive(root: &Path) -> String {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
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
        Ok(source)
    })
    .unwrap()
}
fn bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",")
}
fn probe(generated: &str, value: &str, escaped: bool) -> String {
    let token = quoted(value, escaped);
    let rendered = quoted(value, false);
    format!("{}\n#define FIXTURE_COPY {}\n#define FIXTURE_RENDER {}\n#define FIXTURE_FULL {}\nstatic const uint8_t token[]={{{}}};\nstatic const uint8_t payload[]={{{}}};\nstatic const uint8_t rendered[]={{{}}};\n{}",instrument(generated),symbol("paired.copy"),symbol("paired.quote"),symbol("paired.full"),bytes(token.as_bytes()),bytes(value.as_bytes()),bytes(rendered.as_bytes()),include_str!("paired_probe.c"))
}

#[test]
#[ignore = "explicit physical measurement: requires fresh external SEMAPRAX_PAIRED_PHYSICAL_OUTPUT"]
fn matched_historical_current_codec_physical_work() {
    assert_eq!(
        sha(LEGACY.as_bytes()),
        include_str!("historical_text.sha256").trim()
    );
    assert_eq!(sha(LEGACY.as_bytes()), HISTORICAL_SHA256);
    require_baseline_equivalence();
    let output = std::path::PathBuf::from(
        std::env::var_os("SEMAPRAX_PAIRED_PHYSICAL_OUTPUT")
            .expect("explicit external output required"),
    );
    assert!(output.is_absolute() && !output.exists());
    assert!(!output.starts_with(std::env::current_dir().unwrap()));
    fs::create_dir(&output).unwrap();
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head.status.success());
    let clean = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .unwrap();
    assert!(clean.status.success() && clean.stdout.is_empty());
    let mut receipt = json!({"schema":"semaprax.paired-physical-codec.v1","proof_kind":"native-owning-private","source_checkout_head":String::from_utf8(head.stdout).unwrap().trim(),"historical_algorithm_source":OLD,"historical_template_sha256":sha(LEGACY.as_bytes()),"historical_compiler":null,"test_executable_sha256":sha(&fs::read(std::env::current_exe().unwrap()).unwrap()),"baseline_source": BASELINE,"baseline_equivalence":"all tracked bytes outside five named owning witness files unchanged","logical_meter":"ordinary native: no Fixed 4096 counter","executed":true,"status":"started","rows":[]});
    fs::write(
        output.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    let cases = [
        ("ascii16", "A".repeat(16), false),
        ("ascii64", "A".repeat(64), false),
        ("escaped64", "A".repeat(64), true),
        ("unicode64", "é😀".repeat(10) + "éé", false),
        ("mixed64", "A\0\"\\é".repeat(10) + "ABCD", true),
    ];
    for (name, value, escaped) in cases {
        assert!(value.len() <= 64);
        let case = output.join(name);
        fs::create_dir(&case).unwrap();
        let root = directory("paired-project");
        fs::create_dir(root.join("src")).unwrap();
        fs::write(root.join("semaprax.toml"), include_str!("semaprax.toml")).unwrap();
        fs::write(root.join("src/schema.spx"), canonical(SCHEMA)).unwrap();
        fs::write(
            root.join("src/app.spx"),
            canonical("module consumer.app;@id(\"consumer.main\") fn main()->i64{0}"),
        )
        .unwrap();
        fs::write(
            root.join("src/tests.spx"),
            canonical("module consumer.tests;@id(\"consumer.tests\") fn tests()->i64{0}"),
        )
        .unwrap();
        let current = derive(&root);
        let old = legacy(&current);
        for (arm, source) in [("old", old), ("current", current)] {
            let arm_path = case.join(arm);
            fs::create_dir(&arm_path).unwrap();
            fs::write(root.join("src/schema.spx"), &source).unwrap();
            let application = app(&value, escaped);
            fs::write(root.join("src/app.spx"), &application).unwrap();
            let generated =
                project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                    hir::validate(snapshot.entry_program()).map_err(|e| vec![e])?;
                    codegen::emit_hir_c(snapshot.entry_program()).map_err(|e| vec![e])
                })
                .unwrap();
            fs::write(arm_path.join("schema.spx"), &source).unwrap();
            fs::write(arm_path.join("generated.c"), &generated).unwrap();
            fs::write(arm_path.join("app.spx"), application).unwrap();
            fs::write(arm_path.join("input.json"), input(&value, escaped)).unwrap();
            let c = probe(&generated, &value, escaped);
            let path = arm_path.join("probe.c");
            fs::write(&path, &c).unwrap();
            for optimization in ["-O0", "-O2"] {
                let binary = arm_path.join(format!(
                    "probe{optimization}{}",
                    std::env::consts::EXE_SUFFIX
                ));
                let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
                let build = Command::new(&compiler)
                    .args([
                        "-std=c11",
                        optimization,
                        "-Wall",
                        "-Wextra",
                        "-Werror",
                        "-DSPX_NO_ENTRY_WRAPPER",
                    ])
                    .arg(&path)
                    .arg("-o")
                    .arg(&binary)
                    .output()
                    .unwrap();
                fs::write(
                    arm_path.join(format!("compile{optimization}.stdout")),
                    &build.stdout,
                )
                .unwrap();
                fs::write(
                    arm_path.join(format!("compile{optimization}.stderr")),
                    &build.stderr,
                )
                .unwrap();
                assert!(
                    build.status.success(),
                    "C compiler refused: {}",
                    String::from_utf8_lossy(&build.stderr)
                );
                let run = Command::new(&binary).output().unwrap();
                fs::write(
                    arm_path.join(format!("physical{optimization}.jsonl")),
                    &run.stdout,
                )
                .unwrap();
                fs::write(
                    arm_path.join(format!("physical{optimization}.stderr")),
                    &run.stderr,
                )
                .unwrap();
                assert!(
                    run.status.success() && run.stderr.is_empty(),
                    "physical witness refused: {}",
                    String::from_utf8_lossy(&run.stderr)
                );
                let records = String::from_utf8(run.stdout)
                    .unwrap()
                    .lines()
                    .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(records.len(), 3);
                let quote_increment = json!({
                    "method":"observed paired phase subtraction, including String temporaries",
                    "native_allocations":records[1]["native_allocations"].as_u64().unwrap().checked_sub(records[0]["native_allocations"].as_u64().unwrap()).expect("quote allocation increment"),
                    "explicit_memcpy_bytes":records[1]["explicit_memcpy_bytes"].as_u64().unwrap().checked_sub(records[0]["explicit_memcpy_bytes"].as_u64().unwrap()).expect("quote copy increment")
                });
                receipt["rows"].as_array_mut().unwrap().push(json!({"case":name,"arm":arm,"optimization":optimization,"c_sha256":sha(c.as_bytes()),"binary_sha256":sha(&fs::read(binary).unwrap()),"records":records,"quote_increment_over_materialization":quote_increment}));
                fs::write(
                    output.join("receipt.json"),
                    serde_json::to_vec_pretty(&receipt).unwrap(),
                )
                .unwrap();
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
    let after = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    let clean = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=no"])
        .output()
        .unwrap();
    assert!(after.status.success() && clean.status.success() && clean.stdout.is_empty());
    assert_eq!(
        String::from_utf8(after.stdout).unwrap().trim(),
        receipt["source_checkout_head"].as_str().unwrap()
    );
    require_baseline_equivalence();
    receipt["status"] = json!("success");
    fs::write(
        output.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
}
