//! Project v27 keeps the external stream-command ABI closed while admitting
//! shared Copy-scalar vectors at authenticated private helper boundaries.
use super::*;
use crate::hir;

fn manifest() -> String {
    format!(
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"stream-data\"\nversion = \"0.1.0\"\nprofile = \"{PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V1}\"\n\n[modules]\nentry = \"stream.app\"\nsources = [\"a/app.spx\", \"b/helpers.spx\", \"c/tests.spx\"]\ntests = [\"stream.tests\"]\n\n[exports]\nweb = [\"stream.command\"]\n\n[command]\nfunction = \"stream.command\"\ninput = \"{PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1}\"\n\n[capabilities]\nrequired = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\n"
    )
}

const APP: &str = r#"module stream.app;
use function @id("data.count") from stream.helpers as count;
use function @id("data.scalars") from stream.helpers as scalars;
permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
@id("stream.command") fn command()->i64 uses { process.stdin.read, process.stdout.write } {
    let reader=stdin_stream_open();
    let empty=stdin_stream_eof(reader);
    let values=vec_push<i64>(vec_with_capacity<i64>(1usize),7);
    let observed=count(values);
    let scalar=scalars(1,2i32,3u8,4usize,'x',5.0f32,6.0,true);
    let message="ok";
    let view=string_as_str(message);
    let written=stdout_write(str_as_bytes(view));
    if empty && observed==1usize && scalar==0 {0}else{1}
}
@id("stream.app.main") fn main()->i64 {
    let values=vec_push<i64>(vec_with_capacity<i64>(1usize),7);
    if count(values)==1usize {0}else{1}
}
"#;

const HELPERS: &str = r#"module stream.helpers;
@id("data.at") fn at(values:borrow Vec<i64>,index:usize)->i64 {
    if index<vec_len<i64>(values) {vec_get<i64>(values,index)}else{0}
}
@id("data.count") fn count(values:borrow Vec<i64>)->usize {
    let mut index=0usize;
    let mut total=0;
    while index<vec_len<i64>(values) {
        total=total+at(values,index);
        index=index+1usize;
        0
    }
    if total==7 {index}else{0usize}
}
@id("data.ascii") fn ascii(ignored:i64)->char {'x'}
@id("data.ratio") fn ratio(value:f64)->f64 {value}
@id("data.scalars")
fn scalars(a:i64,b:i32,c:u8,d:usize,e:char,f:f32,g:f64,h:bool)->i64 {
    let character=ascii(a);
    let precise=ratio(g);
    if b==2i32 && c==3u8 && d==4usize && e=='x' && character=='x'
        && f==5.0f32 && precise==6.0 && h {0}else{1}
}
"#;

const TESTS: &str = r#"module stream.tests;
use function @id("data.count") from stream.helpers as count;
@id("stream.tests.main") fn main()->i64 {
    let values=vec_push<i64>(vec_with_capacity<i64>(1usize),7);
    if count(values)==1usize {0}else{1}
}
@id("stream.tests.data") fn test_data()->i64 {
    let values=vec_push<i64>(vec_with_capacity<i64>(1usize),7);
    if count(values)==1usize {0}else{1}
}
"#;

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-project-stream-v27-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    for directory in ["a", "b", "c"] {
        std::fs::create_dir_all(root.join(directory)).unwrap();
    }
    std::fs::write(root.join(MANIFEST_FILE), manifest()).unwrap();
    for (path, source) in [
        ("a/app.spx", APP),
        ("b/helpers.spx", HELPERS),
        ("c/tests.spx", TESTS),
    ] {
        std::fs::write(root.join(path), canonical_source(path, source)).unwrap();
    }
    root.canonicalize().unwrap()
}

fn assert_code(root: &Path, code: &str) {
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn private_vec_and_copy_scalar_helpers_keep_command_abi_closed() {
    let root = fixture();
    let output = root.with_extension("stream-v27-native");
    let (semantic_graph, semantic_graph_digest) =
        with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
            assert_eq!(snapshot.manifest().schema(), PROJECT_SCHEMA_V27);
            assert_eq!(
                snapshot.manifest().project_profile(),
                ProjectProfile::StdinStreamDataCommandIoV1
            );
            let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
            assert_eq!(graph["project_schema"].as_str(), Some(PROJECT_SCHEMA_V27));
            let graph_digest = graph["graph_digest"].as_str().unwrap().to_owned();
            assert_eq!(snapshot.manifest().web_exports(), ["stream.command"]);
            assert_eq!(
                snapshot.public_api_program().permits,
                PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2
                    .iter()
                    .map(|effect| (*effect).to_owned())
                    .collect::<Vec<_>>()
            );
            // The resolved profile independently refuses facts that cannot pass
            // the source import gate, including a lookalike carrier identity.
            for invalid in 0..3 {
                let mut forged = snapshot.public_api_program().clone();
                let helper = forged
                    .functions
                    .iter_mut()
                    .find(|function| function.id.as_str() == "data.count")
                    .unwrap();
                if invalid == 0 {
                    helper.params[0].ownership = hir::OwnershipMode::Own;
                } else if let hir::ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &mut helper.params[0].ty
                {
                    if invalid == 1 {
                        arguments[0] = hir::ResolvedType::Bytes;
                    } else {
                        *declaration = hir::DeclarationId::new("forged.Vec");
                    }
                } else {
                    panic!("checked helper retains the Vec carrier");
                }
                assert_eq!(
                    hir::validate_stream_data_program(
                        &forged,
                        Some(&hir::DeclarationId::new("stream.command"))
                    )
                    .unwrap_err()
                    .code,
                    "SPX-H006"
                );
            }
            let emitted = crate::codegen::emit_hir_c_with_stdin_stream_data(
                snapshot.public_api_program(),
                "stream.command",
            )
            .map_err(|error| vec![error])?;
            assert!(emitted.contains("spx_language_command_stream_run_v2"));
            assert!(emitted.contains("spx_vec_len"));
            snapshot.build_native(&output)?;
            Ok((snapshot.semantic_graph().to_owned(), graph_digest))
        })
        .unwrap();
    with_authenticated_project(&root.join(MANIFEST_FILE), |snapshot| {
        assert_eq!(snapshot.semantic_graph(), semantic_graph);
        let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
        assert_eq!(
            graph["graph_digest"].as_str(),
            Some(semantic_graph_digest.as_str())
        );
        Ok(())
    })
    .unwrap();
    let result = Command::new(&output).stdin(Stdio::null()).output().unwrap();
    assert_eq!(result.status.code(), Some(0));
    assert_eq!(result.stdout, b"ok");
    assert!(result.stderr.is_empty());

    let before = file_inventory(&root);
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
        assert_eq!(file_inventory(&root), before);
    }

    let v27 = manifest();
    for (old, code) in [
        (PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2, "SPX-G174"),
        (PROJECT_PROFILE_STDIN_STREAM_TEXT_COMMAND_IO_V1, "SPX-H006"),
    ] {
        std::fs::write(
            root.join(MANIFEST_FILE),
            v27.replace(PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V1, old),
        )
        .unwrap();
        assert_code(&root, code);
    }

    std::fs::write(root.join(MANIFEST_FILE), &v27).unwrap();
    let helpers = canonical_source(
        "b/helpers.spx",
        &HELPERS.replace("values:borrow Vec<i64>", "values:own Vec<i64>"),
    );
    std::fs::write(root.join("b/helpers.spx"), helpers).unwrap();
    assert_code(&root, "SPX-G172");

    std::fs::write(
        root.join("b/helpers.spx"),
        canonical_source(
            "b/helpers.spx",
            &HELPERS
                .replace("Vec<i64>", "Vec<Bytes>")
                .replace("vec_len<i64>", "vec_len<Bytes>"),
        ),
    )
    .unwrap();
    for path in ["a/app.spx", "c/tests.spx"] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        let source = source
            .replace(
                "vec_push<i64>(vec_with_capacity<i64>(1usize), 7)",
                "vec_push<Bytes>(vec_with_capacity<Bytes>(1usize), bytes_zeroed(1usize))",
            )
            .replace(
                "vec_push<i64>(vec_with_capacity<i64>(1usize),7)",
                "vec_push<Bytes>(vec_with_capacity<Bytes>(1usize),bytes_zeroed(1usize))",
            );
        std::fs::write(root.join(path), canonical_source(path, &source)).unwrap();
    }
    assert_code(&root, "SPX-G172");

    std::fs::write(
        root.join("b/helpers.spx"),
        canonical_source("b/helpers.spx", HELPERS),
    )
    .unwrap();
    std::fs::write(
        root.join("c/tests.spx"),
        canonical_source("c/tests.spx", TESTS),
    )
    .unwrap();
    std::fs::write(
        root.join("a/app.spx"),
        canonical_source(
            "a/app.spx",
            &APP.replace(
                "fn command()->i64",
                "fn command(root_values:borrow Vec<i64>)->i64",
            ),
        ),
    )
    .unwrap();
    let errors = with_authenticated_project(&root.join(MANIFEST_FILE), |_| Ok(())).unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-G172"
            && error.message
                == "command I/O profile command must have an explicit identity and exact signature fn() -> i64"),
        "{errors:?}"
    );

    let _ = std::fs::remove_file(output);
    let _ = std::fs::remove_dir_all(root);
}
