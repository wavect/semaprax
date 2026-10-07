//! Ordinary dependency-consumer coverage for every package added to the
//! compiler-bundled registry by issue #619.

use super::*;

#[test]
fn every_issue_619_package_resolves_for_an_ordinary_dependency_consumer() {
    struct Case {
        directory: &'static str,
        dependency: &'static str,
        profile: &'static str,
        source: &'static str,
        transitive: &'static [&'static str],
    }
    let cases = [
        Case {
            directory: "async",
            dependency: "std.async",
            profile: "useful-data.v1",
            source: "module consumer.app;\nuse function @id(\"std.async.clamp_wait_ms\") from std.async as clamp_wait_ms;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    if clamp_wait_ms(70000usize) == 30000usize { 0 } else { 1 }\n}\n",
            transitive: &[],
        },
        Case {
            directory: "email",
            dependency: "std.email",
            profile: "useful-data.v2",
            source: "module consumer.app;\nuse function @id(\"std.email.recipient_count_admitted\") from std.email as recipient_count_admitted;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    if recipient_count_admitted(1usize) { 0 } else { 1 }\n}\n",
            transitive: &["std.log.redact"],
        },
        Case {
            directory: "encoding-base64",
            dependency: "std.encoding.base64",
            profile: "owned-data-api.v1",
            source: "module consumer.app;\nuse type @id(\"std.io.writer\") from std.io as Writer;\nuse function @id(\"std.encoding.base64.decode-into\") from std.encoding.base64 as decode_into;\nuse function @id(\"std.encoding.base64.decoded-len\") from std.encoding.base64 as decoded_len;\nuse function @id(\"std.encoding.base64.len\") from std.encoding.base64 as base64_len;\nuse function @id(\"std.io.writer.finish\") from std.io as writer_finish;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let encoded = [90u8, 109u8, 56u8, 61u8];\n    let view = array_as_slice(encoded);\n    let written = decode_into(view, Writer { data: bytes_zeroed(2usize), position: 0usize });\n    let output = writer_finish(written);\n    let first = match byte_get(bytes_as_slice(output), 0usize) { Option::Some { value } => value, Option::None {} => 0u8, };\n    if base64_len(2usize) == 4usize && decoded_len(view) == 2usize && byte_len(bytes_as_slice(output)) == 2usize && first == 102u8 { 0 } else { 1 }\n}\n",
            transitive: &["std.encoding", "std.io"],
        },
        Case {
            directory: "env-policy",
            dependency: "std.env.policy",
            profile: "owned-data-api.v1",
            source: "module consumer.app;\nuse function @id(\"std.env.policy.name-is-valid\") from std.env.policy as name_is_valid;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let name = [65u8];\n    if name_is_valid(array_as_slice(name)) { 0 } else { 1 }\n}\n",
            transitive: &[],
        },
        Case {
            directory: "io-lines",
            dependency: "std.io.lines",
            profile: "owned-data-api.v1",
            source: r#"module consumer.app;
use type @id("std.io.reader") from std.io as Reader;
use type @id("std.io.writer") from std.io as Writer;
use function @id("std.io.reader.finish") from std.io as reader_finish;
use function @id("std.io.reader.from-bytes") from std.io as reader_from_bytes;
use function @id("std.io.reader.remaining") from std.io as reader_remaining;
use function @id("std.io.writer.finish") from std.io as writer_finish;
use function @id("std.io.writer.from-bytes") from std.io as writer_from_bytes;
use function @id("std.io.lines.reader.line-into") from std.io.lines as reader_line_into;
use function @id("std.io.lines.reader.next-line") from std.io.lines as reader_next_line;

@id("consumer.main")
fn main() -> i64
{
    let input = [97u8, 10u8, 98u8];
    let mut reader = reader_from_bytes(bytes_copy(array_as_slice(input)));
    let mut writer = writer_from_bytes(bytes_zeroed(2usize));
    let mut lines = 0usize;
    while reader_remaining(reader) > 0usize {
        writer = reader_line_into(reader, writer);
        reader = reader_next_line(reader);
        lines = lines + 1usize;
        reader_remaining(reader) > 0usize
    }
    let retained = reader_finish(reader);
    let output = writer_finish(writer);
    if lines == 2usize && byte_len(bytes_as_slice(retained)) == 3usize && byte_len(bytes_as_slice(output)) == 2usize { 0 } else { 1 }
}
"#,
            transitive: &["std.io"],
        },
        Case {
            directory: "net",
            dependency: "std.net",
            profile: "useful-data.v1",
            source: "module consumer.app;\nuse function @id(\"std.net.port_is_valid\") from std.net as port_is_valid;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    if port_is_valid(443usize) { 0 } else { 1 }\n}\n",
            transitive: &[],
        },
        Case {
            directory: "path-normalize",
            dependency: "std.path.normalize",
            profile: "owned-data-api.v1",
            source: "module consumer.app;\nuse function @id(\"std.path.normalize.normalized-len\") from std.path.normalize as normalized_len;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let path = [97u8];\n    if normalized_len(array_as_slice(path), 1usize) == 1usize { 0 } else { 1 }\n}\n",
            transitive: &["std.path.value"],
        },
    ];
    let scratch = temporary("issue-619-dependencies");
    for case in cases {
        let project_root = scratch.join(case.directory);
        let web_exports = if case.directory == "encoding-base64" {
            "[]"
        } else {
            "[\"consumer.main\"]"
        };
        std::fs::create_dir_all(project_root.join("src")).unwrap();
        std::fs::write(
            project_root.join("semaprax.toml"),
            format!(
                "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"consumer-{}\"\nversion = \"0.1.0\"\nprofile = \"{}\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = {}\n\n[dependencies]\n{} = \"=0.1.0\"\n",
                case.directory, case.profile, web_exports, case.dependency
            ),
        )
        .unwrap();
        std::fs::write(project_root.join("src/app.spx"), case.source).unwrap();
        std::fs::write(
            project_root.join("src/tests.spx"),
            "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
        )
        .unwrap();
        project::with_authenticated_project(&project_root.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            let options = project::ProjectExecutionOptions::default();
            assert_eq!(
                snapshot.execute_entry(&options)?.outcome(),
                &project::ProjectExecutionOutcome::Returned(0),
                "{} entry failed",
                case.dependency
            );
            assert_eq!(
                snapshot.execute_test(&options)?.outcome(),
                &project::ProjectExecutionOutcome::Returned(0),
                "{} tests failed",
                case.dependency
            );
            let workspace = snapshot.workspace_manifest();
            assert!(
                workspace.contains(&format!("dependencies/{}/0.1.0/", case.dependency)),
                "{} missing from workspace: {workspace}",
                case.dependency
            );
            for dependency in case.transitive {
                assert!(
                    workspace.contains(&format!("dependencies/{dependency}/0.1.0/")),
                    "{} missing transitive {dependency}: {workspace}",
                    case.dependency
                );
            }
            Ok(())
        })
        .unwrap();
    }
    let _ = std::fs::remove_dir_all(scratch);
}
