//! OPT-723's explicitly selected application profile, with authenticated type imports.
use super::*;
use crate::hir;

const HELPERS: &str = r#"module stream.jobs;
@id("job.type") record Job {
 @id("job.priority") priority:i64, @id("job.arrival") arrival:i64,
 @id("job.sequence") sequence:i64, @id("job.id") id:i64,
 @id("job.duration") duration:i64, @id("job.worker") worker:i64,
}
@id("job.make") fn make(id:i64)->Vec<Job> {
 let one=Job{priority:2,arrival:10,sequence:1,id:7,duration:1,worker:0};
 let two=Job{priority:1,arrival:9,sequence:0,id:id,duration:1,worker:0};
 let v=vec_with_capacity<Job>(2usize); let w=vec_push<Job>(v,one); vec_push<Job>(w,two)
}
@id("job.order") fn order(values:own Vec<Job>)->Vec<Job> {vec_sort<Job>(values)}
@id("job.first") fn first(values:borrow Vec<Job>)->Job {vec_get<Job>(values,0usize)}
@id("job.identity") fn identity(value:Job)->Job {value}
"#;
const IMPORTS: &str = r#"use type @id("job.type") from stream.jobs as Job;
use function @id("job.make") from stream.jobs as make;
use function @id("job.order") from stream.jobs as order;
use function @id("job.first") from stream.jobs as first;
use function @id("job.identity") from stream.jobs as identity;
"#;
const PURE: &str =
    "let values=order(make(4)); let job=identity(first(values)); if job.id==4 {0}else{1}";
const LOGICAL_SCHEMA: &str = r#"
@id("schema.patient") record PatientSchema {
 @id("schema.id") id:string, @id("schema.arrival") arrival:i64,
 @id("schema.service") service:i64, @id("schema.priority") priority:i64,
 @id("schema.deadline") deadline:i64,
}
@id("schema.request") record RequestSchema {
 @id("schema.servers") servers:Vec<string>, @id("schema.patients") patients:Vec<PatientSchema>,
}
"#;
fn manifest() -> String {
    format!("schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"stream-records\"\nversion = \"0.1.0\"\nprofile = \"{PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V2}\"\n\n[modules]\nentry = \"stream.app\"\nsources = [\"a/app.spx\", \"b/jobs.spx\", \"c/tests.spx\"]\ntests = [\"stream.tests\"]\n\n[exports]\nweb = [\"stream.command\"]\n\n[command]\nfunction = \"stream.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n")
}
fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-stream-records-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    for d in ["a", "b", "c"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    std::fs::write(root.join(MANIFEST_FILE), manifest()).unwrap();
    let app = format!(
        r#"module stream.app;
{IMPORTS}
permit {{process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}}
@id("stream.app.main") fn main()->i64 {{{PURE}}}
@id("stream.command") fn command()->i64 uses {{process.stdin.read,process.stdout.write}} {{
 let mut reader=stdin_stream_open(); let mut size=0;
 while !stdin_stream_eof(reader) {{
  let n={{let chunk=stdin_stream_chunk(reader); byte_len(chunk)}};
  size=size+i64_from_usize(n); reader=stdin_stream_next(reader); 0
 }}
 let values=order(make(size)); let job=identity(first(values));
 let text=string_from_i64(job.id); let view=string_as_str(text);
 let written=stdout_write(str_as_bytes(view)); 0
}}
"#
    );
    let cases=format!("module stream.tests;\n{IMPORTS}\n@id(\"stream.tests.main\") fn main()->i64 {{{PURE}}}\n@id(\"stream.tests.records\") fn test_records()->i64 {{{PURE}}}");
    for (path, source) in [
        ("a/app.spx", app.as_str()),
        ("b/jobs.spx", HELPERS),
        ("c/tests.spx", cases.as_str()),
    ] {
        std::fs::write(root.join(path), canonical_source(path, source)).unwrap();
    }
    root.canonicalize().unwrap()
}
#[test]
fn v29_records_execute_pure_prepared_tests_and_streamed_native_command() {
    let root = fixture();
    let output = root.with_extension("native-v29");
    let graph = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(snapshot.manifest().schema(), PROJECT_SCHEMA_V29);
        assert_eq!(
            snapshot.manifest().project_profile(),
            ProjectProfile::StdinStreamDataCommandIoV2
        );
        let options = ProjectExecutionOptions::default();
        assert!(snapshot.execute_entry(&options)?.command_succeeded());
        let tests = snapshot.execute_test(&options)?;
        assert!(tests.command_succeeded());
        assert_eq!(tests.cases().len(), 1);
        let prepared = snapshot
            .retain_revision()
            .prepare_interpreter(PreparedProjectInterpreterOptions::default())?;
        let result = prepared.execute_test(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )?;
        assert_eq!(
            result.outcome(),
            &ProjectPreparedExecutionOutcome::Returned(0)
        );
        let command = hir::DeclarationId::new("stream.command");
        hir::validate_stream_record_program(snapshot.public_api_program(), Some(&command)).unwrap();
        assert!(
            hir::validate_stream_data_program(snapshot.public_api_program(), Some(&command))
                .is_err()
        );
        let mut forged = snapshot.public_api_program().clone();
        let helper = forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "job.first")
            .unwrap();
        helper.params[0].ownership = hir::OwnershipMode::Value;
        assert!(hir::validate_stream_record_program(&forged, Some(&command)).is_err());
        assert!(
            crate::codegen::emit_hir_c_with_stdin_stream_records(&forged, "stream.command")
                .is_err()
        );
        snapshot.build_native(&output)?;
        Ok(snapshot.semantic_graph().to_owned())
    })
    .unwrap();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(snapshot.semantic_graph(), graph);
        Ok(())
    })
    .unwrap();
    let mut child = Command::new(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"abcd").unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"4");
    assert!(result.stderr.is_empty());
    // A fresh authoritative source type changes the retained graph; a prior cache cannot freeze its field facts.
    std::fs::write(
        root.join("b/jobs.spx"),
        canonical_source(
            "b/jobs.spx",
            &HELPERS
                .replace("duration:i64", "duration:u8")
                .replace("duration:1,", "duration:1u8,"),
        ),
    )
    .unwrap();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_ne!(snapshot.semantic_graph(), graph);
        assert!(snapshot
            .execute_entry(&ProjectExecutionOptions::default())?
            .command_succeeded());
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}
#[test]
fn v29_records_keep_v27_and_other_backends_closed() {
    let root = fixture();
    let original = manifest();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(
            snapshot
                .build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
                .unwrap_err()[0]
                .code,
            "SPX-W120"
        );
        assert_eq!(
            snapshot
                .build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)
                .unwrap_err()[0]
                .code,
            "SPX-W120"
        );
        Ok(())
    })
    .unwrap();
    std::fs::write(
        root.join(MANIFEST_FILE),
        original.replace(
            PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V2,
            PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V1,
        ),
    )
    .unwrap();
    assert!(with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn stream_record_logical_schema_is_source_bound_and_cannot_hide_unused_runtime_uses() {
    let root = fixture();
    let path = root.join("b/jobs.spx");
    std::fs::write(
        &path,
        canonical_source("b/jobs.spx", &format!("{HELPERS}{LOGICAL_SCHEMA}")),
    )
    .unwrap();
    let output = root.with_extension("logical-schema-native");
    let graph = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert!(snapshot
            .execute_entry(&ProjectExecutionOptions::default())?
            .command_succeeded());
        assert!(!snapshot
            .public_api_program()
            .types
            .iter()
            .any(|t| t.id.as_str() == "schema.request"));
        snapshot.build_native(&output)?;
        Ok(snapshot.semantic_graph().to_owned())
    })
    .unwrap();
    std::fs::write(
        &path,
        canonical_source(
            "b/jobs.spx",
            &format!(
                "{HELPERS}{}",
                LOGICAL_SCHEMA.replace("priority:i64", "priority:i32")
            ),
        ),
    )
    .unwrap();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_ne!(snapshot.semantic_graph(), graph);
        Ok(())
    })
    .unwrap();
    // Prior valid graph/cache data cannot authorize this uncalled source use.
    let invalid=format!("{HELPERS}{LOGICAL_SCHEMA}@id(\"unused.schema\") fn unused_schema(value:own RequestSchema)->i64 {{0}}");
    let invalid = crate::format::canonical(&crate::parse(&invalid, "b/jobs.spx").unwrap());
    std::fs::write(&path, &invalid).unwrap();
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert!(errors.iter().any(|d| d.code == "SPX-T281"), "{errors:?}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), invalid);
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}
