//! Exact private Map/record transport across retained Project v25 routes.
use super::*;

const BODY: &str = r#"
    let labels0=make();
    let labels1=roundtrip(labels0);
    let labels=cut(labels1);
    let text=read(labels);
    if string_len(text)==3 && map_len<i64,string>(labels)==1usize {0}else{1}
"#;

fn collection_fixture() -> PathBuf {
    let root = text_fixture();
    // No bundled dependency is needed by this bounded collection fixture.
    std::fs::write(root.join(MANIFEST_FILE), manifest()).unwrap();
    let imports = r#"
use function @id("labels.make") from stream.input as make;
use function @id("labels.cut") from stream.input as cut;
use function @id("labels.read") from stream.input as read;
use function @id("labels.roundtrip") from stream.input as roundtrip;
"#;
    let app = format!(
        r#"module stream.app;
{imports}
permit {{ process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }}
@id("stream.command") fn command()->i64 uses {{ process.stdin.read, process.stdout.write }} {{
    let reader=stdin_stream_open();
    let empty=stdin_stream_eof(reader);
    let labels0=make();
    let labels1=roundtrip(labels0);
    let labels=cut(labels1);
    let text=read(labels);
    let view=string_as_str(text);
    let written=stdout_write(str_as_bytes(view));
    if empty && map_len<i64,string>(labels)==1usize {{0}}else{{1}}
}}
@id("stream.app.main") fn main()->i64 {{{BODY}}}
"#
    );
    let input = r#"module stream.input;
@id("labels.Bag") record Bag {
    @id("labels.Bag.map") labels:Map<i64,string>,
    @id("labels.Bag.title") title:string,
}
@id("labels.make") fn make()->Map<i64,string> {
    let labels0=map_new<i64,string>(2usize);
    let labels=map_set<i64,string>(labels0,2,"tail");
    map_set<i64,string>(labels,-1,"é\u{0}")
}
@id("labels.cut") fn cut(labels:own Map<i64,string>)->Map<i64,string> {
    map_remove<i64,string>(labels,2)
}
@id("labels.read") fn read(labels:borrow Map<i64,string>)->string {
    map_get_or<i64,string>(labels,-1,"missing")
}
@id("labels.relay") fn relay(bag:own Bag)->Bag {bag}
@id("labels.roundtrip") fn roundtrip(incoming:own Map<i64,string>)->Map<i64,string> {
    let bag=Bag{labels:incoming,title:"bag"};
    let moved=relay(bag);
    match own moved {Bag{labels,title}=>labels,}
}
"#;
    let cases = format!(
        "module stream.tests;\n{imports}\n@id(\"stream.tests.main\") fn main()->i64 {{{BODY}}}\n@id(\"stream.tests.collections\") fn test_collections()->i64 {{{BODY}}}\n"
    );
    for (path, source) in [
        ("a/app.spx", app.as_str()),
        ("b/input.spx", input),
        ("c/tests.spx", cases.as_str()),
    ] {
        std::fs::write(root.join(path), canonical_source(path, source)).unwrap();
    }
    root
}

#[test]
fn v25_collections_transport_executes_retained_and_native_routes() {
    let root = collection_fixture();
    let output = root.with_extension("collections-v25-native");
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        let options = ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.public_api_program().permits,
            PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2
                .iter()
                .map(|effect| (*effect).to_owned())
                .collect::<Vec<_>>()
        );
        assert!(snapshot.execute_entry(&options)?.command_succeeded());
        let test = snapshot.execute_test(&options)?;
        assert!(test.command_succeeded());
        assert_eq!(test.cases().len(), 1);
        let revision = snapshot.retain_revision();
        let cancelled =
            revision.execute_test_cancellable(&options, &ProjectExecutionCancellation::new())?;
        assert!(matches!(cancelled,
            super::super::super::super::execution::CancellableProjectExecution::Completed(test)
            if test.command_succeeded()));
        let prepared = revision.prepare_interpreter(PreparedProjectInterpreterOptions::default())?;
        let result = prepared.execute_test(
            &PreparedProjectExecutionOptions::default(),
            &ProjectExecutionCancellation::new(),
        )?;
        assert_eq!(
            result.outcome(),
            &ProjectPreparedExecutionOutcome::Returned(0)
        );
        snapshot.build_native(&output)?;
        Ok(())
    })
    .unwrap();
    let result = Command::new(&output).stdin(Stdio::null()).output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"\xc3\xa9\0");
    assert!(result.stderr.is_empty());
    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn collection_transport_does_not_widen_v24_or_public_scalar_profiles() {
    let root = collection_fixture();
    with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap();
    std::fs::write(root.join(MANIFEST_FILE), exit_manifest()).unwrap();
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G172"
        && error
            .message
            .contains("declarations outside the selected project linker profile")));

    // Remove the unrelated command capabilities so scalar admission reaches the
    // collection signatures themselves, rather than refusing ambient effects.
    let app = format!(
        r#"module stream.app;
use function @id("labels.make") from stream.input as make;
use function @id("labels.cut") from stream.input as cut;
use function @id("labels.read") from stream.input as read;
use function @id("labels.roundtrip") from stream.input as roundtrip;
@id("stream.app.main") fn main()->i64 {{{BODY}}}
"#
    );
    std::fs::write(root.join("a/app.spx"), canonical_source("a/app.spx", &app)).unwrap();
    let scalar = "schema = \"semaprax.project.v1\"\nname = \"collection-scalar-refusal\"\nentry = \"stream.app\"\nsources = [\"a/app.spx\", \"b/input.spx\", \"c/tests.spx\"]\nweb_exports = [\"labels.make\"]\ntests = [\"stream.tests\"]\n";
    ProjectManifest::parse(scalar).unwrap();
    std::fs::write(root.join(MANIFEST_FILE), scalar).unwrap();
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G174"
        && error
            .message
            .contains("signature outside the selected profile")));
    let _ = std::fs::remove_dir_all(root);
}
