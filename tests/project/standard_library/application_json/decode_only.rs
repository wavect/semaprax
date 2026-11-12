//! Decode-only selectors retain the real old decoder on the same backend inputs.
use super::*;
use std::io::Write as _;
use std::path::Path;
use std::process::Stdio;

type DiagnosticFields<'a> = (
    &'static str,
    semaprax::diagnostic::Severity,
    &'a str,
    Option<&'a str>,
    Option<semaprax::ast::Span>,
    Option<&'a str>,
);

fn diagnostic_fields(rows: &[semaprax::diagnostic::Diagnostic]) -> Vec<DiagnosticFields<'_>> {
    rows.iter()
        .map(|row| {
            (
                row.code,
                row.severity,
                row.message.as_ref(),
                row.path.as_deref(),
                row.span,
                row.help.as_deref(),
            )
        })
        .collect()
}

const UTF8_SCHEMA: &str = r#"module consumer.schema;
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
const PERMITS: &str =
    "permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}\n";

fn profiles(utf8: bool, stream: bool) -> (project::JsonCodecProfile, project::JsonCodecProfile) {
    use project::JsonCodecProfile::*;
    match (utf8, stream) {
        (false, false) => (OwnedRequest, OwnedRequestDecodeOnlyV1),
        (false, true) => (StreamOwnedRequest, StreamOwnedRequestDecodeOnlyV1),
        (true, false) => (
            Utf8OwnedRequest {
                max_string_bytes: 4,
            },
            Utf8OwnedRequestDecodeOnlyV1 {
                max_string_bytes: 4,
            },
        ),
        (true, true) => (
            StreamUtf8OwnedRequest {
                max_string_bytes: 4,
            },
            StreamUtf8OwnedRequestDecodeOnlyV1 {
                max_string_bytes: 4,
            },
        ),
    }
}
fn root_id(utf8: bool) -> &'static str {
    if utf8 {
        "unicode.input"
    } else {
        "application.request"
    }
}
fn setup(label: &str, utf8: bool, stream: bool) -> std::path::PathBuf {
    let schema = if utf8 { UTF8_SCHEMA } else { views::SCHEMA };
    let schema = if stream {
        schema.replace(
            "module consumer.schema;",
            &format!("module consumer.schema;\n{PERMITS}"),
        )
    } else {
        schema.to_owned()
    };
    let root = fixture(label, &schema);
    if stream {
        std::fs::write(
            root.join("semaprax.toml"),
            command_manifest("language-command-io.owned-data.v1"),
        )
        .unwrap();
        std::fs::write(
            root.join("src/app.spx"),
            canonical(&format!(
                "module consumer.app;\n{PERMITS}{}@id(\"consumer.main\") fn main()->i64{{0}}",
                stream_bootstrap_command()
            )),
        )
        .unwrap();
    }
    root
}
fn derive(root: &Path, utf8: bool, profile: project::JsonCodecProfile) -> String {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let source = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            root_id(utf8),
            profile,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            root_id(utf8),
            &source,
            profile,
        )?;
        Ok(source)
    })
    .unwrap()
}
fn one_function(source: &str, id: &str) -> String {
    let mut program = parse(source, "src/schema.spx").unwrap();
    program.functions.retain(|f| f.stable_id == id);
    assert_eq!(program.functions.len(), 1);
    program.types.clear();
    program.module_uses.clear();
    format::canonical(&program)
}
fn one_type(source: &str, id: &str) -> String {
    let mut program = parse(source, "src/schema.spx").unwrap();
    program.types.retain(|t| t.stable_id == id);
    assert_eq!(program.types.len(), 1);
    program.functions.clear();
    program.module_uses.clear();
    format::canonical(&program)
}

#[test]
fn four_public_selectors_keep_every_retained_function_and_reject_cross_profile_source() {
    for utf8 in [false, true] {
        for stream in [false, true] {
            let root = setup("decode-only-public", utf8, stream);
            let (full, selected) = profiles(utf8, stream);
            let old = derive(&root, utf8, full);
            let current = derive(&root, utf8, selected);
            assert_ne!(old, current);
            assert!(current.len() < old.len()); // Source bytes only, not runtime gain.
            let parsed = parse(&current, "src/schema.spx").unwrap();
            let decode_id = format!(
                "{}.json.{}owned.decode",
                root_id(utf8),
                if utf8 { "utf8." } else { "" }
            );
            assert!(parsed.functions.iter().any(|f| f.stable_id == decode_id));
            for f in &parsed.functions {
                assert_eq!(
                    one_function(&current, &f.stable_id),
                    one_function(&old, &f.stable_id),
                    "{}",
                    f.stable_id
                );
            }
            for t in &parsed.types {
                assert_eq!(
                    one_type(&current, &t.stable_id),
                    one_type(&old, &t.stable_id),
                    "{}",
                    t.stable_id
                );
            }
            assert!(!parsed
                .functions
                .iter()
                .any(|f| f.stable_id.ends_with(".owned.encode")
                    || f.stable_id.ends_with(".owned.encoded-len")));
            assert!(!parsed
                .types
                .iter()
                .any(|t| t.stable_id.ends_with(".json.view-encode")));
            project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                let revision = snapshot.retain_revision();
                let old_forged =
                    format!("{old}\n@id(\"forged.decode-only\") fn forged()->i64{{0}}\n");
                let expected = project::verify_json_codec_source_with_profile(
                    &revision,
                    "src/schema.spx",
                    root_id(utf8),
                    &old_forged,
                    full,
                )
                .unwrap_err();
                for (source, wrong) in [(&old, selected), (&current, full)] {
                    assert_eq!(
                        diagnostic_fields(
                            &project::verify_json_codec_source_with_profile(
                                &revision,
                                "src/schema.spx",
                                root_id(utf8),
                                source,
                                wrong
                            )
                            .unwrap_err()
                        ),
                        diagnostic_fields(&expected)
                    );
                }
                let forged =
                    format!("{current}\n@id(\"forged.decode-only\") fn forged()->i64{{0}}\n");
                assert_eq!(
                    diagnostic_fields(
                        &project::verify_json_codec_source_with_profile(
                            &revision,
                            "src/schema.spx",
                            root_id(utf8),
                            &forged,
                            selected
                        )
                        .unwrap_err()
                    ),
                    diagnostic_fields(&expected)
                );
                Ok(())
            })
            .unwrap();
            if stream {
                let manifest = std::fs::read_to_string(root.join("semaprax.toml")).unwrap();
                std::fs::write(
                    root.join("semaprax.toml"),
                    manifest.replace("\"process.stdin.read\", ", ""),
                )
                .unwrap();
                assert!(
                    project::with_authenticated_project(&root.join("semaprax.toml"), |_| Ok(()))
                        .is_err()
                );
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

fn imports(utf8: bool) -> String {
    if utf8 {
        r#"module consumer.app;
use type @id("unicode.input.json.utf8.owned-result") from consumer.schema as Outcome;
use type @id("unicode.row") from consumer.schema as Row;
use function @id("unicode.input.json.utf8.owned.decode") from consumer.schema as decode;
"#
        .to_owned()
    } else {
        r#"module consumer.app;
use type @id("application.request.json.owned-result") from consumer.schema as Outcome;
use type @id("application.patient") from consumer.schema as Patient;
use function @id("application.request.json.owned.decode") from consumer.schema as decode;
"#
        .to_owned()
    }
}
fn app(utf8: bool) -> String {
    let mut source = imports(utf8);
    let (words, rows) = if utf8 {
        ("labels", "rows")
    } else {
        ("servers", "patients")
    };
    source.push_str(&format!(r#"@id("consumer.error") fn error(input:borrow Slice<u8>,wanted:i64,at:usize,target:i64)->i64{{
let decoded=decode(input);match own decoded{{Outcome::Decoded{{{words},{rows}}}=>1,Outcome::Error{{code,offset,field}}=>if code==wanted && offset==at && field==target{{0}}else{{1}},}}}}
@id("consumer.main") fn main()->i64{{let mut bad=0;
"#));
    let errors: Vec<(&[u8], i64, usize, i64)> = if utf8 {
        vec![
            (b"{", 1, 1, 0),
            (b"{}", 3, 2, 1),
            (br#"{"labels":["12345"]}"#, 6, 11, 1),
            (br#"{"labels":["12345"],"rows":[}"#, 1, 28, 0),
            (br#"{"labels":["","","","","","","","",""]}"#, 8, 35, 1),
            (b"{\"labels\":[\"\xc0\"]}", 1, 12, 0),
            (br#"{"labels":["\ud800"]}"#, 1, 12, 0),
        ]
    } else {
        vec![
            (b"{", 1, 1, 0),
            (b"{}", 3, 2, 1),
            (br#"{"servers":["A","\u0041"]}"#, 10, 16, 1),
            (br#"{"servers":["12345678901234567"]}"#, 6, 12, 1),
            (br#"{"servers":["\u0000"]}"#, 6, 12, 1),
        ]
    };
    for (i, (bytes, code, offset, field)) in errors.iter().enumerate() {
        source.push_str(&format!(
            "let bad_{i}={};bad=bad+error(array_as_slice(bad_{i}),{code},{offset}usize,{field});\n",
            array(bytes)
        ));
    }
    // The Bytes owner is local to this helper. Decoded String/row owners must
    // remain usable after its epilogue, under both complete codec profiles.
    let (input, check) = if utf8 {
        (br#"{"labels":["\udbff\udfff",""],"rows":[{"text":"\u0000","number":-1}]}"#.as_slice(),
         "let first=vec_clone_at<string>(labels,0usize);let row=vec_clone_at<Row>(rows,0usize);vec_len<string>(labels)==2usize && vec_len<Row>(rows)==1usize && string_len(first)==4 && first==string_from_char(char_from_i64(1114111)) && string_len(row.text)==1 && row.text==string_from_char(char_from_i64(0)) && row.number==0-1")
    } else {
        (br#"{"servers":["\u00531"],"patients":[{"id":"\u00501","arrival":1,"service":2,"priority":3,"deadline":4}]}"#.as_slice(),
         "let first=vec_clone_at<string>(servers,0usize);let row=vec_clone_at<Patient>(patients,0usize);vec_len<string>(servers)==1usize && first==\"S1\" && vec_len<Patient>(patients)==1usize && row.id==\"P1\" && row.arrival==1 && row.service==2 && row.priority==3 && row.deadline==4")
    };
    source.push_str(&format!(r#"let decoded=detached();bad=bad+match own decoded{{Outcome::Error{{code,offset,field}}=>1,Outcome::Decoded{{{words},{rows}}}=>{{let valid={{{check}}};if valid{{0}}else{{1}}}},}};if bad==0{{897}}else{{0-bad}}}}
@id("consumer.detached") fn detached()->Outcome{{let raw={};let owner=bytes_copy(array_as_slice(raw));decode(bytes_as_slice(owner))}}
"#,array(input)));
    source
}
fn wasm_run(root: &Path, status: u32, value: i64, refusal: &str) {
    let host =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/owned_data/owned_leaf_vec/host.js");
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
fn backend_parity(utf8: bool) {
    let mut steps = None;
    for profile in [profiles(utf8, false).0, profiles(utf8, false).1] {
        let root = setup("decode-only-backends", utf8, false);
        let generated = derive(&root, utf8, profile);
        std::fs::write(root.join("src/schema.spx"), generated).unwrap();
        std::fs::write(root.join("src/app.spx"), canonical(&app(utf8))).unwrap();
        let used = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let options = if utf8 {
                project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
                    .map_err(|e| vec![e])?
            } else {
                project::ProjectExecutionOptions::default()
            };
            let execution = snapshot.execute_entry(&options)?;
            assert_eq!(
                execution.outcome(),
                &project::ProjectExecutionOutcome::Returned(897)
            );
            let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|e| vec![e])?;
            for optimization in ["-O0", "-O2"] {
                super::super::compile_and_run_c(&c, &root, optimization, "897");
            }
            let bytes =
                wasm::emit_resolved_module(snapshot.entry_program()).map_err(|e| vec![e])?;
            wasmparser::Validator::new().validate_all(&bytes).unwrap();
            std::fs::write(root.join("app.wasm"), bytes).unwrap();
            Ok(execution.steps_used())
        })
        .unwrap();
        if let Some(old) = steps {
            assert_eq!(
                used, old,
                "same decoder and application fuel, not a new allowance"
            );
        } else {
            steps = Some(used);
        }
        wasm_run(&root, 0, 897, "none");
        std::fs::remove_dir_all(root).unwrap();
    }
}
#[test]
fn identifier_decode_only_preserves_detachment_diagnostics_and_three_backend_fuel() {
    backend_parity(false);
}
#[test]
fn utf8_decode_only_preserves_four_byte_bound_malformed_priority_and_three_backend_fuel() {
    backend_parity(true);
}

#[test]
fn decode_only_plan_replay_rejects_full_profile_forged_source_and_live_drift() {
    for utf8 in [false, true] {
        let root = setup("decode-only-plan", utf8, false);
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            let (full, selected) = profiles(utf8, false);
            let entry = project::JsonCodecPlanEntry {
                source_path: "src/schema.spx".into(),
                record_id: root_id(utf8).into(),
                profile: selected,
            };
            let plan = vec![entry.clone()];
            let bundle = project::derive_json_codec_plan(&revision, &plan)?;
            project::verify_json_codec_plan(&revision, &plan, &bundle)?;
            assert_eq!(
                bundle.entries[0].profile,
                if utf8 {
                    "utf8-owned-request-decode-only.v1"
                } else {
                    "owned-request-decode-only.v1"
                }
            );
            let full_plan = vec![project::JsonCodecPlanEntry {
                profile: full,
                ..entry
            }];
            assert!(project::verify_json_codec_plan(&revision, &full_plan, &bundle).is_err());
            let full_bundle = project::derive_json_codec_plan(&revision, &full_plan)?;
            assert!(project::verify_json_codec_plan(&revision, &plan, &full_bundle).is_err());
            for mutation in 0..4 {
                let mut forged = bundle.clone();
                match mutation {
                    0 => forged.authored_project_revision.push('x'),
                    1 => forged.entries[0].profile = full_bundle.entries[0].profile.clone(),
                    2 => forged.candidate_sources.clear(),
                    3 => {
                        let source = forged
                            .candidate_sources
                            .iter_mut()
                            .find(|s| s.path == "src/schema.spx")
                            .unwrap();
                        source.source.push('\n');
                        source.sha256 = format!(
                            "sha256:{:x}",
                            semaprax::digest_hex::LowerHex(Sha256::digest(
                                source.source.as_bytes()
                            ))
                        );
                    }
                    _ => unreachable!(),
                }
                assert!(
                    project::verify_json_codec_plan(&revision, &plan, &forged).is_err(),
                    "mutation {mutation}"
                );
            }
            Ok(())
        })
        .unwrap();
        let result = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let plan = vec![project::JsonCodecPlanEntry {
                source_path: "src/schema.spx".into(),
                record_id: root_id(utf8).into(),
                profile: profiles(utf8, false).1,
            }];
            let bundle = project::derive_json_codec_plan(&snapshot.retain_revision(), &plan)?;
            let path = root.join("src/schema.spx");
            let mut text = std::fs::read_to_string(&path).unwrap();
            text.push('\n');
            std::fs::write(path, text).unwrap();
            Ok(bundle)
        });
        assert_eq!(result.unwrap_err()[0].code, "SPX-J102");
        assert!(!root.join("candidate.json").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn four_plan_selector_spellings_and_cli_partial_failure_publish_no_candidate() {
    for (name, utf8, stream) in [
        ("owned-request-decode-only.v1", false, false),
        ("stream-owned-request-decode-only.v1", false, true),
        ("utf8-owned-request-decode-only.v1", true, false),
        ("stream-utf8-owned-request-decode-only.v1", true, true),
    ] {
        let mut entry =
            serde_json::json!({"source":"src/schema.spx","record":root_id(utf8),"profile":name});
        if utf8 {
            entry["max_string_bytes"] = 4.into();
        }
        let plan = serde_json::json!({"schema":"semaprax.json-codec-plan.v1","entries":[entry]});
        let (_, parsed) =
            project::decode_json_codec_plan(&serde_json::to_vec(&plan).unwrap()).unwrap();
        assert_eq!(parsed[0].profile, profiles(utf8, stream).1);
    }
    let root = setup("decode-only-no-partial", false, false);
    let manifest = std::fs::read_to_string(root.join("semaprax.toml"))
        .unwrap()
        .replace(
            "\"src/schema.spx\",",
            "\"src/schema.spx\", \"src/second.spx\",",
        );
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    std::fs::write(
        root.join("src/second.spx"),
        canonical("module consumer.second; @id(\"second.anchor\") fn anchor()->i64{0}"),
    )
    .unwrap();
    let original = std::fs::read(root.join("src/schema.spx")).unwrap();
    let one = serde_json::json!({"source":"src/schema.spx","record":"application.request","profile":"owned-request-decode-only.v1"});
    let plan = serde_json::json!({"schema":"semaprax.json-codec-plan.v1","entries":[one]});
    std::fs::write(root.join("plan.json"), serde_json::to_vec(&plan).unwrap()).unwrap();
    let run = |output: &str| {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&root)
            .args(["json-codec", ".", "--plan", "plan.json", "--output", output])
            .output()
            .unwrap()
    };
    let success = run("candidate.json");
    assert!(
        success.status.success(),
        "{}",
        String::from_utf8_lossy(&success.stderr)
    );
    let retained = std::fs::read(root.join("candidate.json")).unwrap();
    assert!(!run("candidate.json").status.success());
    assert_eq!(
        std::fs::read(root.join("candidate.json")).unwrap(),
        retained
    );
    let mut stale = plan.clone();
    stale["expected_project_revision"] = format!("sha256:{}", "0".repeat(64)).into();
    std::fs::write(root.join("plan.json"), serde_json::to_vec(&stale).unwrap()).unwrap();
    assert!(!run("stale.json").status.success());
    assert!(!root.join("stale.json").exists());
    let mut partial = plan;
    partial["entries"].as_array_mut().unwrap().push(serde_json::json!({"source":"src/second.spx","record":"missing.row","profile":"owned-request-decode-only.v1"}));
    std::fs::write(
        root.join("plan.json"),
        serde_json::to_vec(&partial).unwrap(),
    )
    .unwrap();
    assert!(!run("partial.json").status.success());
    assert!(!root.join("partial.json").exists());
    assert_eq!(
        std::fs::read(root.join("src/schema.spx")).unwrap(),
        original
    );
    assert!(!std::fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".json-codec-")));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn decode_only_partial_materialization_refusals_settle_owned_values_on_strict_wasm() {
    for utf8 in [false, true] {
        for profile in [profiles(utf8, false).0, profiles(utf8, false).1] {
            let root = setup("decode-only-partial-push", utf8, false);
            let generated = derive(&root, utf8, profile);
            std::fs::write(root.join("src/schema.spx"), generated).unwrap();
            let (input, words, rows) = if utf8 {
                (br#"{"labels":["a","b"],"rows":[{"number":0,"text":"x"},{"number":1,"text":"y"}]}"#.as_slice(),"labels","rows")
            } else {
                (br#"{"servers":["A","B"],"patients":[{"id":"P","arrival":0,"service":1,"priority":0,"deadline":0},{"id":"Q","arrival":0,"service":1,"priority":0,"deadline":0}]}"#.as_slice(),"servers","patients")
            };
            let app = imports(utf8)
                + &format!(
                    r#"@id("consumer.main") fn main()->i64{{let raw={};let outcome=decode(array_as_slice(raw));match own outcome{{Outcome::Error{{code,offset,field}}=>0,Outcome::Decoded{{{words},{rows}}}=>897,}}}}"#,
                    array(input)
                );
            std::fs::write(root.join("src/app.spx"), canonical(&app)).unwrap();
            project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                let bytes =
                    wasm::emit_resolved_module(snapshot.entry_program()).map_err(|e| vec![e])?;
                std::fs::write(root.join("app.wasm"), bytes).unwrap();
                Ok(())
            })
            .unwrap();
            wasm_run(&root, 0, 897, "none");
            for at in 1..=4 {
                wasm_run(&root, 15, 0, &format!("codec-push-{at}"));
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

fn stream_execute(binary: &Path, input: &[u8]) -> std::process::Output {
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut writer = child.stdin.take().unwrap();
    let input = input.to_vec();
    let thread = std::thread::spawn(move || writer.write_all(&input).unwrap());
    let output = child.wait_with_output().unwrap();
    thread.join().unwrap();
    output
}
#[test]
fn stream_decode_only_preserves_normalization_and_native_o0_o2_statuses_without_encoder() {
    for utf8 in [false, true] {
        let (words, rows, row) = if utf8 {
            ("labels", "rows", "Row")
        } else {
            ("servers", "patients", "Patient")
        };
        let mut reference = None;
        for profile in [profiles(utf8, true).0, profiles(utf8, true).1] {
            let root = setup("decode-only-stream-native", utf8, true);
            let source = derive(&root, utf8, profile);
            std::fs::write(root.join("src/schema.spx"), source).unwrap();
            let application = imports(utf8)
                + &format!(
                    r#"
use type @id("{}.json.stream-input") from consumer.schema as Input;
use function @id("{}.json.stream.normalize") from consumer.schema as normalize;
{PERMITS}
@id("consumer.receive") fn receive(bytes:own Bytes,length:usize)->Outcome {{
    decode(byte_range(bytes_as_slice(bytes),0usize,length))
}}
@id("consumer.diagnostic")
fn diagnostic(kind:borrow str,code:i64,offset:usize,field:i64)->i64 uses{{process.stderr.write}} {{
    let line=string_format("{{}}:{{}}:{{}}:{{}}\n",string_from_str(kind),code,offset,field);
    let view=string_as_str(line);
    let written=stderr_write(str_as_bytes(view));
    2
}}
@id("consumer.decide") fn decide(outcome:own Outcome)->i64 uses{{process.stderr.write}} {{
    match own outcome {{
        Outcome::Error{{code,offset,field}}=>{{
            let kind="schema";
            diagnostic(string_as_str(kind),code,offset,field)
        }},
        Outcome::Decoded{{{words},{rows}}}=>
            if vec_len<string>({words})==1usize && vec_len<{row}>({rows})==0usize {{0}} else {{3}},
    }}
}}
@id("consumer.command") fn command()->i64 uses{{process.stdin.read,process.stderr.write}} {{
    let normalized=normalize();
    match own normalized {{
        Input::Error{{code,offset,field}}=>{{
            let kind="stream";
            diagnostic(string_as_str(kind),code,offset,field)
        }},
        Input::Ready{{bytes,length}}=>{{
            let outcome=receive(bytes,length);
            decide(outcome)
        }},
    }}
}}
@id("consumer.main") fn main()->i64{{0}}
"#,
                    root_id(utf8),
                    root_id(utf8)
                );
            std::fs::write(root.join("src/app.spx"), canonical(&application)).unwrap();
            let c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                codegen::emit_hir_c_with_stdin_stream_owned_data(
                    snapshot.public_api_program(),
                    "consumer.command",
                )
                .map_err(|e| vec![e])
            })
            .unwrap();
            let valid = if utf8 {
                br#"{"labels":["\u00e9"],"rows":[]}"#.as_slice()
            } else {
                br#"{"servers":["\u00531"],"patients":[]}"#.as_slice()
            };
            let malformed = if utf8 {
                br#"{"labels":["\ud800"],"rows":[]}"#.as_slice()
            } else {
                br#"{"servers":["A","\u0041"],"patients":[]}"#.as_slice()
            };
            let mut cases = vec![
                (valid.to_vec(), 0, Some(String::new())),
                (
                    [vec![b' '; 70_000], valid.to_vec(), vec![b'\n'; 5_000]].concat(),
                    0,
                    Some(String::new()),
                ),
                (b"{".to_vec(), 2, None),
                (malformed.to_vec(), 2, None),
            ];
            for (length, diagnostic) in [
                (131_072, "schema:4:1:0\n"),
                (131_073, "stream:9:131072:0\n"),
            ] {
                let mut input = br#"{"x":""#.to_vec();
                input.extend(std::iter::repeat_n(b'A', length - 8));
                input.extend_from_slice(b"\"}");
                assert_eq!(input.len(), length);
                cases.push((input.clone(), 2, Some(diagnostic.to_owned())));
                input[length - 1] = b']';
                cases.push((input, 2, Some(format!("stream:1:{}:0\n", length - 1))));
            }
            let mut observations = Vec::new();
            for optimization in ["-O0", "-O2"] {
                let c_path = root.join("stream.c");
                let binary = root.join(format!("stream{}", optimization));
                std::fs::write(&c_path, &c).unwrap();
                let compile = Command::new("clang")
                    .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
                    .arg(&c_path)
                    .arg("-o")
                    .arg(&binary)
                    .output()
                    .unwrap();
                assert!(
                    compile.status.success(),
                    "{}",
                    String::from_utf8_lossy(&compile.stderr)
                );
                for (input, expected, diagnostic) in &cases {
                    let output = stream_execute(&binary, input);
                    assert_eq!(output.status.code(), Some(*expected));
                    assert!(output.stdout.is_empty());
                    if let Some(diagnostic) = diagnostic {
                        assert_eq!(output.stderr, diagnostic.as_bytes());
                    } else {
                        assert!(!output.stderr.is_empty());
                    }
                    observations.push((output.status.code(), output.stdout, output.stderr));
                }
            }
            if let Some(old) = &reference {
                assert_eq!(&observations, old);
            } else {
                reference = Some(observations);
            }
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
