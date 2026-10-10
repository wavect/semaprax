//! Successor-profile routing for owned vectors nested in ordinary records.
use super::*;

const PUBLIC_ORDER_MANIFEST: &str =
    include_str!("../../../../examples/collection-record-order/semaprax.toml");
const PUBLIC_ORDER_APP: &str =
    include_str!("../../../../examples/collection-record-order/src/app.spx");
const PUBLIC_ORDER_DATA: &str =
    include_str!("../../../../examples/collection-record-order/src/data.spx");
const PUBLIC_ORDER_TESTS: &str =
    include_str!("../../../../examples/collection-record-order/src/tests.spx");

fn manifest(profile: &str) -> String {
    format!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"collection-records\"\nversion = \"0.1.0\"\nprofile = \"{profile}\"\n\n[modules]\nentry = \"collection.app\"\nsources = [\"a/app.spx\", \"b/data.spx\", \"c/tests.spx\"]\ntests = [\"collection.tests\"]\n\n[exports]\nweb = [\"collection.command\"]\n\n[command]\nfunction = \"collection.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n"
    )
}

fn fixture(profile: &str, helper_in_app: bool) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-collection-records-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    for directory in ["a", "b", "c"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(root.join(MANIFEST_FILE), manifest(profile)).unwrap();
    let app = if helper_in_app {
        r#"module collection.app;
permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}
@id("collection.command") fn command()->i64 uses {process.stdin.read,process.stdout.write} {
 let mut reader=stdin_stream_open();
 while !stdin_stream_eof(reader) {let chunk=stdin_stream_chunk(reader); let nonempty=byte_len(chunk)>0usize; reader=stdin_stream_next(reader); if nonempty {0}else{0}}
 0
}
@id("collection.app.main") fn main()->i64 {0}
@id("collection.metrics") record Metrics {@id("collection.metrics.selected") selected:i64,}
@id("collection.report") record Report {@id("collection.report.items") items:Vec<string>,@id("collection.report.metrics") metrics:Metrics,}
@id("collection.unused") fn unused(value:borrow Report)->i64 {i64_from_usize(vec_len<string>(value.items))+value.metrics.selected}
"#
    } else {
        r#"module collection.app;
permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}
@id("collection.command") fn command()->i64 uses {process.stdin.read,process.stdout.write} {
 let mut reader=stdin_stream_open();
 while !stdin_stream_eof(reader) {let chunk=stdin_stream_chunk(reader); let nonempty=byte_len(chunk)>0usize; reader=stdin_stream_next(reader); if nonempty {0}else{0}}
 0
}
@id("collection.app.main") fn main()->i64 {0}
"#
    };
    let provider = if helper_in_app {
        "module collection.data;\n"
    } else {
        r#"module collection.data;
@id("collection.metrics") record Metrics {@id("collection.metrics.selected") selected:i64,}
@id("collection.report") record Report {@id("collection.report.items") items:Vec<string>,@id("collection.report.metrics") metrics:Metrics,}
@id("collection.unused") fn unused(value:borrow Report)->i64 {i64_from_usize(vec_len<string>(value.items))+value.metrics.selected}
"#
    };
    let tests = "module collection.tests;\n@id(\"collection.tests.main\") fn main()->i64 {0}\n@id(\"collection.tests.case\") fn test_route()->i64 {0}\n";
    for (path, source) in [
        ("a/app.spx", app),
        ("b/data.spx", provider),
        ("c/tests.spx", tests),
    ] {
        std::fs::write(root.join(path), canonical_source(path, source)).unwrap();
    }
    root.canonicalize().unwrap()
}

fn command_app() -> &'static str {
    r#"module collection.app;
use function @id("collection.verify") from collection.data as verify;
permit {process.args.read,process.stderr.write,process.stdin.read,process.stdout.write}
@id("collection.app.main") fn main()->i64 {if verify()==12 {0}else{1}}
@id("collection.command") fn command()->i64 uses {process.stdin.read,process.stdout.write} {
 let mut reader=stdin_stream_open();
 let mut saw_input=false;
 while !stdin_stream_eof(reader) {let chunk=stdin_stream_chunk(reader); saw_input=byte_len(chunk)>0usize; reader=stdin_stream_next(reader); 0}
 let score=verify();
 if score==12 && saw_input {let text=string_from_i64(score); let view=string_as_str(text); let written=stdout_write(str_as_bytes(view)); 0}else{1}
}
"#
}

fn command_data() -> &'static str {
    r#"module collection.data;
@id("collection.metrics") record Metrics {@id("collection.metrics.selected") selected:i64,}
@id("collection.report") record Report {@id("collection.report.items") items:Vec<string>,@id("collection.report.metrics") metrics:Metrics,}
@id("collection.inspect") fn inspect(value:borrow Report)->i64 {i64_from_usize(vec_len<string>(value.items))+value.metrics.selected}
@id("collection.verify") fn verify()->i64 {
 let items=vec_with_capacity<string>(2usize);
 let items=vec_push<string>(items,"one");
 let items=vec_push<string>(items,"two");
 let report=Report{items,metrics:Metrics{selected:10}};
 inspect(report)
}
"#
}

fn command_tests() -> &'static str {
    r#"module collection.tests;
use function @id("collection.verify") from collection.data as verify;
@id("collection.tests.main") fn main()->i64 {if verify()==12 {0}else{1}}
@id("collection.tests.case") fn test_route()->i64 {if verify()==12 {0}else{1}}
"#
}

fn valid_fixture() -> PathBuf {
    let root = fixture(PROJECT_PROFILE_STDIN_STREAM_COLLECTION_RECORD_COMMAND_IO_V1, false);
    for (path, source) in [
        ("a/app.spx", command_app()),
        ("b/data.spx", command_data()),
        ("c/tests.spx", command_tests()),
    ] {
        std::fs::write(root.join(path), canonical_source(path, source)).unwrap();
    }
    root
}

fn public_order_example_fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-public-record-order-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join(MANIFEST_FILE), PUBLIC_ORDER_MANIFEST).unwrap();
    for (name, source) in [
        ("app.spx", PUBLIC_ORDER_APP),
        ("data.spx", PUBLIC_ORDER_DATA),
        ("tests.spx", PUBLIC_ORDER_TESTS),
    ] {
        std::fs::write(root.join("src").join(name), source).unwrap();
        let parsed = crate::parse(source, Path::new(name)).unwrap();
        assert_eq!(crate::format::canonical(&parsed), source);
    }
    root.canonicalize().unwrap()
}

#[test]
fn v31_public_typed_record_order_example() {
    let root = public_order_example_fixture();
    let output = root.with_extension("native-v31-record-order");
    let graph = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(snapshot.manifest().schema(), PROJECT_SCHEMA_V31);
        assert_eq!(
            snapshot.manifest().project_profile(),
            ProjectProfile::StdinStreamCollectionRecordCommandIoV1
        );
        assert!(snapshot
            .execute_entry(&ProjectExecutionOptions::default())?
            .command_succeeded());
        let cases = snapshot.execute_test(&ProjectExecutionOptions::default())?;
        assert!(cases.command_succeeded());
        assert_eq!(cases.cases().len(), 1);
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
    child.stdin.take().unwrap().write_all(b"run\n").unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"4321");
    assert!(result.stderr.is_empty());
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn v31_nested_collection_record_executes_project_routes_and_native_command() {
    let root = valid_fixture();
    let output = root.with_extension("native-v31-collection-records");
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(snapshot.manifest().schema(), PROJECT_SCHEMA_V31);
        assert_eq!(
            snapshot.manifest().project_profile(),
            ProjectProfile::StdinStreamCollectionRecordCommandIoV1
        );
        assert!(snapshot
            .manifest()
            .capabilities()
            .iter()
            .map(String::as_str)
            .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2));
        assert!(snapshot
            .execute_entry(&ProjectExecutionOptions::default())?
            .command_succeeded());
        let cases = snapshot.execute_test(&ProjectExecutionOptions::default())?;
        assert!(cases.command_succeeded());
        assert_eq!(cases.cases().len(), 1);
        let prepared = snapshot
            .retain_revision()
            .prepare_interpreter(PreparedProjectInterpreterOptions::default())?;
        assert_eq!(
            prepared
                .execute_test(
                    &PreparedProjectExecutionOptions::default(),
                    &ProjectExecutionCancellation::new()
                )?
                .outcome(),
            &ProjectPreparedExecutionOutcome::Returned(0)
        );
        snapshot.build_native(&output)?;
        Ok(())
    })
    .unwrap();
    let mut child = Command::new(&output)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"input").unwrap();
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"12");
    assert!(result.stderr.is_empty());
    for web in [true, false] {
        let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
            if web {
                snapshot
                    .build_web_inline(MAX_PROJECT_WEB_BUILD_BYTES)
                    .map(|_| ())
            } else {
                snapshot
                    .build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)
                    .map(|_| ())
            }
        })
        .unwrap_err();
        assert_eq!(errors[0].code, "SPX-W120");
        assert!(errors[0]
            .message
            .contains(PROJECT_PROFILE_STDIN_STREAM_COLLECTION_RECORD_COMMAND_IO_V1));
    }
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn v29_and_v30_reject_unused_nested_collection_helpers_before_module_cropping() {
    for profile in [
        PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V2,
        PROJECT_PROFILE_STDIN_STREAM_OWNED_DATA_COMMAND_IO_V1,
    ] {
        for helper_in_app in [true, false] {
            let root = fixture(profile, helper_in_app);
            let before = file_inventory(&root);
            let errors =
                with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
            assert!(errors.iter().any(|error| {
                error.code == "SPX-G172"
                    && error.message
                        == "nested collection-record runtime requires the explicitly selected language-command-io.collection-record.v1 profile"
            }), "{profile}, helper_in_app={helper_in_app}: {errors:?}");
            assert_eq!(file_inventory(&root), before);
            let _ = std::fs::remove_dir_all(root);
        }
    }
}
