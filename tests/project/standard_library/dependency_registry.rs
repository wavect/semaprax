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
            source: "module consumer.app;\nuse function @id(\"std.encoding.base64.len\") from std.encoding.base64 as base64_len;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    if base64_len(1usize) == 4usize { 0 } else { 1 }\n}\n",
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
            source: "module consumer.app;\nuse function @id(\"std.io.lines.line-content-len\") from std.io.lines as line_content_len;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let line = [97u8, 13u8];\n    if line_content_len(array_as_slice(line), 0usize) == 2usize { 0 } else { 1 }\n}\n",
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
        std::fs::create_dir_all(project_root.join("src")).unwrap();
        std::fs::write(
            project_root.join("semaprax.toml"),
            format!(
                "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"consumer-{}\"\nversion = \"0.1.0\"\nprofile = \"{}\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.main\"]\n\n[dependencies]\n{} = \"=0.1.0\"\n",
                case.directory, case.profile, case.dependency
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
