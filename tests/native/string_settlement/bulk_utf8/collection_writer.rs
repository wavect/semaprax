//! OPT-737 actual generated CollectionResponse phases; opt-in, never a cost trial.
use super::{directory, symbol};
use semaprax::{codegen, format, hir, parse, project};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command, time::Instant};
const SCHEMA: &str = r#"module consumer.schema;
@id("work.row") record Row {@id("work.row.text") text:string,@id("work.row.number") number:i64,}
@id("work.metrics") record Metrics {@id("work.metrics.count") count:usize,@id("work.metrics.ok") ok:bool,}
@id("work.report") record Report {@id("work.report.metrics") metrics:Metrics,@id("work.report.rows") rows:Vec<Row>,}
@id("work.anchor") fn anchor()->i64{0}
"#;
fn canonical(text: &str) -> String {
    format::canonical(&parse(text, "collection-work.spx").unwrap())
}
fn sha(bytes: &[u8]) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
fn app() -> String {
    r#"module consumer.app;
use type @id("work.row") from consumer.schema as Row;
use type @id("work.metrics") from consumer.schema as Metrics;
use type @id("work.report") from consumer.schema as Report;
use type @id("work.report.json.collection-response.encode-result") from consumer.schema as Encoded;
use function @id("work.report.json.collection-response.encoded-len") from consumer.schema as length;
use function @id("work.report.json.collection-response.encode") from consumer.schema as encode;
@id("work.payload") fn payload()->string {
let mut text="";let mut at=0;
while at<32{text=string_concat(text,string_from_char(char_from_i64(at)));at=at+1;at<32}
string_concat(text,"é😀\"\\")
}
@id("work.prepare") fn prepare(count:usize)->Report requires count<=256usize {
let seed=Row{text:payload(),number:-9223372036854775808};
let seeds=vec_push<Row>(vec_with_capacity<Row>(1usize),seed);
let mut rows=vec_with_capacity<Row>(256usize);let mut at=0usize;
while at<count{let row=vec_clone_at<Row>(seeds,0usize);rows=vec_push<Row>(rows,row);at=at+1usize;at<count}
Report{metrics:Metrics{count:count,ok:false},rows:rows}
}
@id("work.length") fn required(value:borrow Report)->usize{length(value)}
@id("work.run") fn run(value:borrow Report,limit:usize,expected:borrow Slice<u8>)->i64{
let outcome=encode(value,limit);match own outcome {
Encoded::Refused{required:n}=>if n==byte_len(expected){1}else{0},
Encoded::Encoded{text}=>{
let actual=str_as_bytes(string_as_str(text));let mut at=0usize;
let mut same=byte_len(actual)==byte_len(expected);
while same && at<byte_len(actual){same=match byte_get(actual,at){Option::Some{value:a}=>match byte_get(expected,at){Option::Some{value:b}=>a==b,Option::None{}=>false,},Option::None{}=>false,};at=at+1usize;same && at<byte_len(actual)}
if same{2}else{0}
},}}
@id("work.consume") fn consume(value:own Report)->i64{0}
@id("consumer.main") fn main()->i64{
let value=prepare(0usize);let n=required(value);let empty=[0u8];let outcome=run(value,n,array_as_slice(empty));let dropped=consume(value);outcome+dropped
}
"#.to_owned()
}
fn generated(root: &Path) -> String {
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
        canonical("module consumer.tests;@id(\"consumer.tests\") fn main()->i64{0}"),
    )
    .unwrap();
    let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let policy = project::JsonCodecProfile::CollectionResponse {
            max_string_bytes: 64,
        };
        let first = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "work.report",
            policy,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "work.report",
            &first,
            policy,
        )?;
        assert_eq!(
            first,
            project::derive_json_codec_source_with_profile(
                &revision,
                "src/schema.spx",
                "work.report",
                policy
            )?
        );
        assert_eq!(first, canonical(&first));
        Ok(first)
    })
    .unwrap();
    fs::write(root.join("src/schema.spx"), source).unwrap();
    fs::write(root.join("src/app.spx"), canonical(&app())).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        hir::validate(snapshot.entry_program()).map_err(|e| vec![e])?;
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|e| vec![e])?;
        assert_eq!(
            c,
            codegen::emit_hir_c(snapshot.entry_program()).map_err(|e| vec![e])?
        );
        for id in ["work.prepare", "work.length", "work.run", "work.consume"] {
            assert!(c.contains(&symbol(id)));
        }
        Ok(c)
    })
    .unwrap()
}
fn git(args: &[&str]) -> String {
    let output = Command::new("git").args(args).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
fn validate_rows(rows: &[serde_json::Value], lengths: &[(u64, usize)]) -> Result<(), String> {
    if rows.len() != 12 || lengths.len() != 4 {
        return Err("expected four cases with exactly three phases each".into());
    }
    let phases = [
        "preflight",
        "one-short-refusal",
        "exact-generated-encode-and-byte-oracle",
    ];
    for (case, &(count, length)) in lengths.iter().enumerate() {
        for (phase, name) in phases.iter().enumerate() {
            let row = &rows[case * 3 + phase];
            if row["rows"].as_u64() != Some(count) || row["phase"].as_str() != Some(*name) {
                return Err(format!(
                    "case {case} phase {phase}: row identity/order mismatch"
                ));
            }
            let exact = phase == 2;
            let expected = [
                ("allocations", if exact { 2 } else { 0 }),
                ("frees", if exact { 2 } else { 0 }),
                ("bytes_reservations", u64::from(exact)),
                (
                    "reserved_payload_bytes",
                    if exact { length as u64 } else { 0 },
                ),
                ("string_allocations", u64::from(exact)),
                // The selected ordinary native String ABI stores one u64
                // length immediately before payload and one trailing NUL.
                (
                    "string_physical_bytes",
                    if exact { length as u64 + 8 + 1 } else { 0 },
                ),
                ("final_buffer_copy_calls", u64::from(exact)),
                (
                    "final_buffer_copy_bytes",
                    if exact { length as u64 } else { 0 },
                ),
            ];
            for (key, value) in expected {
                if row[key].as_u64() != Some(value) {
                    return Err(format!(
                        "case {case} phase {phase}: {key} expected {value}, got {}",
                        row[key]
                    ));
                }
            }
            for (total, final_copy) in [
                ("explicit_memcpy_calls", "final_buffer_copy_calls"),
                ("explicit_memcpy_bytes", "final_buffer_copy_bytes"),
            ] {
                if !matches!((row[total].as_u64(), row[final_copy].as_u64()), (Some(all), Some(part)) if all >= part)
                {
                    return Err(format!("case {case} phase {phase}: invalid copy domains"));
                }
            }
            if row.get("payload_store_count") != Some(&serde_json::Value::Null) {
                return Err("unobserved payload stores must remain actual null".into());
            }
        }
    }
    Ok(())
}
#[test]
#[ignore = "explicit native physical work: fresh external SEMAPRAX_COLLECTION_WRITER_OUTPUT required"]
fn generated_collection_response_exact_physical_work() {
    let (Some(build_head), Some(build_tree)) = (
        option_env!("SEMAPRAX_COLLECTION_WRITER_BUILD_HEAD"),
        option_env!("SEMAPRAX_COLLECTION_WRITER_BUILD_TREE"),
    ) else {
        panic!("recompile with mandatory SEMAPRAX_COLLECTION_WRITER_BUILD_HEAD/TREE bindings");
    };
    assert_eq!(
        build_head,
        git(&["rev-parse", "HEAD"]),
        "stale test build HEAD"
    );
    assert_eq!(
        build_tree,
        git(&["rev-parse", "HEAD^{tree}"]),
        "stale test build tree"
    );
    assert!(git(&["status", "--porcelain", "--untracked-files=all"]).is_empty());
    let output = std::path::PathBuf::from(
        std::env::var_os("SEMAPRAX_COLLECTION_WRITER_OUTPUT")
            .expect("fresh external output required"),
    );
    assert!(
        output.is_absolute()
            && !output.exists()
            && !output.starts_with(std::env::current_dir().unwrap())
    );
    fs::create_dir(&output).unwrap();
    let binding = json!({"head":build_head,"tree":build_tree});
    let binary_sha = sha(&fs::read(std::env::current_exe().unwrap()).unwrap());
    fs::write(
        output.join("receipt.json"),
        serde_json::to_vec_pretty(&json!({
            "schema":"semaprax.generated-collection-writer-physical.v1",
            "status":"started","executed":false,"embedded_build_binding":binding,
            "test_executable_sha256":binary_sha
        }))
        .unwrap(),
    )
    .unwrap();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let root = directory("collection-writer-project");
        let generated = generated(&root);
        fs::write(output.join("generated.c"), &generated).unwrap();
        for file in [
            "semaprax.toml",
            "src/schema.spx",
            "src/app.spx",
            "src/tests.spx",
        ] {
            let destination = output.join(file);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::copy(root.join(file), destination).unwrap();
        }
        let payload: String = (0..32)
            .map(char::from)
            .chain(['é', '😀', '"', '\\'])
            .collect();
        let row = format!(
            "{{\"text\":{},\"number\":-9223372036854775808}}",
            serde_json::to_string(&payload).unwrap()
        );
        let mut cases = String::new();
        let mut expected_lengths = Vec::new();
        for rows in [0, 1, 8, 256] {
            let expected = format!(
                "{{\"metrics\":{{\"count\":{rows},\"ok\":false}},\"rows\":[{}]}}",
                vec![row.clone(); rows].join(",")
            );
            assert!(expected.len() <= 131072);
            expected_lengths.push((rows as u64, expected.len()));
            fs::write(output.join(format!("expected-{rows}.json")), &expected).unwrap();
            let bytes = expected
                .bytes()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join(",");
            cases.push_str(&format!("{{static const uint8_t expected[]={{ {bytes} }};case_run(&context,{rows},expected,sizeof expected);}}\n"));
        }
        let observer = include_str!("../../../native_owned_utf8_settlement_v1/allocations.c");
        let copies = include_str!("copies.c");
        let report = format!(
            "spx_record_{}",
            "work.report"
                .bytes()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let hooks = include_str!("collection_writer/hooks.c");
        let probe = include_str!("collection_writer/probe.c");
        let c=format!("{}\n#define FIXTURE_TRACK_CALLOC\n{observer}\n{copies}\n{hooks}\n#define main fixture_generated_main\n{generated}\n#undef main\n#undef malloc\n#undef calloc\n#undef free\n#undef memcpy\n#define PREPARE {}\n#define REQUIRED {}\n#define RUN {}\n#define CONSUME {}\n#define REPORT {report}\n{probe}\nint main(void){{REQUIRE(fixture_binary_stdout());struct spx_context context={{0}};struct spx_status_entry entries[16];REQUIRE(spx_context_init(&context,737,entries,16,NULL,NULL,NULL));{cases}return 0;}}", super::super::STDIO,symbol("work.prepare"),symbol("work.length"),symbol("work.run"),symbol("work.consume"));
        fs::write(output.join("probe.c"), &c).unwrap();
        let mut receipt = json!({"schema":"semaprax.generated-collection-writer-physical.v1","status":"started","executed":false,"proof_kind":"private-native-owning","source_checkout_head":git(&["rev-parse","HEAD"]),"source_tree":git(&["rev-parse","HEAD^{tree}"]),"cargo_lock_sha256":sha(&fs::read("Cargo.lock").unwrap()),"test_executable_sha256":binary_sha,"embedded_build_binding":binding,"generated_c_sha256":sha(generated.as_bytes()),"probe_c_sha256":sha(c.as_bytes()),"input_preparation":"outside every measured phase","profile_max_rows":256,"measured_rows":[0,1,8,256],"expected_output_lengths":expected_lengths,"materialization_cap":"ordinary native context; no Fixed counters reset","physical_string_oom_recovery":null,"pending_obligations":["OPT-724 production returned String OOM settlement","OPT-737 matched application/cost evidence"],"results":[]});
        fs::write(
            output.join("receipt.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
        for optimization in ["-O0", "-O2"] {
            let executable = output.join(format!(
                "probe{optimization}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let compiler = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
            let args = [
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ];
            let start = Instant::now();
            let build = Command::new(&compiler)
                .args(args)
                .arg(output.join("probe.c"))
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            fs::write(
                output.join(format!("compile{optimization}.stdout")),
                &build.stdout,
            )
            .unwrap();
            fs::write(
                output.join(format!("compile{optimization}.stderr")),
                &build.stderr,
            )
            .unwrap();
            if !build.status.success() {
                receipt["status"] = json!("compile-refused");
                receipt["failed_optimization"] = json!(optimization);
                receipt["compile_exit_code"] = json!(build.status.code());
                fs::write(
                    output.join("receipt.json"),
                    serde_json::to_vec_pretty(&receipt).unwrap(),
                )
                .unwrap();
            }
            assert!(
                build.status.success(),
                "{}",
                String::from_utf8_lossy(&build.stderr)
            );
            let compilation_wall = start.elapsed().as_secs_f64();
            let start = Instant::now();
            let run = Command::new(&executable).output().unwrap();
            let wall = start.elapsed().as_secs_f64();
            fs::write(
                output.join(format!("run{optimization}.stdout")),
                &run.stdout,
            )
            .unwrap();
            fs::write(
                output.join(format!("run{optimization}.stderr")),
                &run.stderr,
            )
            .unwrap();
            receipt["executed"] = json!(true);
            if !run.status.success() || !run.stderr.is_empty() {
                receipt["status"] = json!("probe-refused");
                receipt["failed_optimization"] = json!(optimization);
                receipt["run_exit_code"] = json!(run.status.code());
            }
            fs::write(
                output.join("receipt.json"),
                serde_json::to_vec_pretty(&receipt).unwrap(),
            )
            .unwrap();
            assert!(
                run.status.success() && run.stderr.is_empty(),
                "{}",
                String::from_utf8_lossy(&run.stderr)
            );
            let rows = String::from_utf8(run.stdout.clone())
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
                .collect::<Vec<_>>();
            validate_rows(&rows, &expected_lengths)
                .expect("independent physical metric validation");
            receipt["results"].as_array_mut().unwrap().push(json!({"optimization":optimization,"compiler":compiler.to_string_lossy(),"compile_argv":args.into_iter().map(str::to_owned).chain([output.join("probe.c").display().to_string(),"-o".to_owned(),executable.display().to_string()]).collect::<Vec<_>>(),"compile_wall_seconds":compilation_wall,"wall_seconds":wall,"executable_sha256":sha(&fs::read(&executable).unwrap()),"stdout_sha256":sha(&run.stdout),"rows":rows}));
            fs::write(
                output.join("receipt.json"),
                serde_json::to_vec_pretty(&receipt).unwrap(),
            )
            .unwrap();
        }
        assert_eq!(
            receipt["source_checkout_head"].as_str().unwrap(),
            git(&["rev-parse", "HEAD"])
        );
        assert_eq!(
            receipt["source_tree"].as_str().unwrap(),
            git(&["rev-parse", "HEAD^{tree}"])
        );
        assert_eq!(
            receipt["cargo_lock_sha256"].as_str().unwrap(),
            sha(&fs::read("Cargo.lock").unwrap())
        );
        assert!(git(&["status", "--porcelain", "--untracked-files=all"]).is_empty());
        receipt["status"] = json!("complete");
        fs::write(
            output.join("receipt.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
        fs::remove_dir_all(root).unwrap();
    }));
    if let Err(failure) = outcome {
        let mut receipt: serde_json::Value =
            serde_json::from_slice(&fs::read(output.join("receipt.json")).unwrap()).unwrap();
        receipt["preceding_status"] = receipt["status"].clone();
        receipt["status"] = json!("failed");
        let detail = failure
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| failure.downcast_ref::<&str>().copied())
            .unwrap_or("non-string panic");
        receipt["failure_detail"] = json!(detail);
        fs::write(
            output.join("receipt.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
        std::panic::resume_unwind(failure);
    }
}
