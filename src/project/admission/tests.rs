use super::*;

#[test]
fn every_legacy_marker_reports_one_exact_closed_profile() {
    let legacy = [
        (
            PreparedProjectAdmission::UsefulTextConsumerV1,
            ProjectProfile::UsefulTextConsumerV1,
        ),
        (
            PreparedProjectAdmission::UsefulDataV1,
            ProjectProfile::UsefulDataV1,
        ),
        (
            PreparedProjectAdmission::UsefulDataCommandV1,
            ProjectProfile::UsefulDataCommandV1,
        ),
        (
            PreparedProjectAdmission::UsefulDataCommandV2,
            ProjectProfile::UsefulDataCommandV2,
        ),
        (
            PreparedProjectAdmission::LanguageCommandIoV1,
            ProjectProfile::LanguageCommandIoV1,
        ),
        (
            PreparedProjectAdmission::LineCommandIoV1,
            ProjectProfile::LineCommandIoV1,
        ),
        (
            PreparedProjectAdmission::NetworkCommandIoV1,
            ProjectProfile::NetworkCommandIoV1,
        ),
        (
            PreparedProjectAdmission::HttpsCommandIoV1,
            ProjectProfile::HttpsCommandIoV1,
        ),
    ];
    for (prepared, expected) in legacy {
        assert_eq!(prepared.profile(), expected);
        assert!(prepared.owned_descriptor().is_none());
        assert!(prepared.flat_record_descriptor().is_none());
    }
}

#[test]
fn useful_data_v2_keeps_public_contract_abi_and_private_owned_calls_separate() {
    use crate::project::{PROJECT_PROFILE_USEFUL_DATA_V2, PROJECT_SCHEMA_V16};
    use crate::workspace_graph::{self, WorkspaceSource};
    let source = r#"
module data.app;
use type @id("data.reader") from data.owner as Reader;
@id("data.main") fn main()->i64 {0}
@id("data.size") fn size(value:usize)->usize ensures result==value {value}
@id("data.length") fn length(value:borrow Slice<u8>)->usize {size(byte_len(value))}
@id("data.private") fn private_cursor(value:own Reader)->Reader {value}
"#;
    let provider = r#"
module data.owner;
@id("data.reader") record Reader { @id("data.reader.data") data:Bytes, @id("data.reader.position") position:usize, }
@id("data.owner.main") fn main()->i64 {0}
"#;
    let sources = [("app.spx", source), ("owner.spx", provider)]
        .into_iter()
        .map(|(path, source)| {
            let parsed = crate::parse(source, std::path::Path::new(path)).unwrap();
            WorkspaceSource {
                path: path.to_owned(),
                source: crate::format::canonical(&parsed),
            }
        })
        .collect();
    let graph = workspace_graph::build_owned(sources).unwrap();
    let roots = vec!["data.length".to_owned()];
    let program = graph
        .linked_scalar_program_with_roots("data.app", &roots, ProjectProfile::UsefulDataV2, false)
        .unwrap();
    assert!(!program
        .functions
        .iter()
        .any(|function| function.id.as_str() == "data.private"));
    let text=format!("schema = \"{PROJECT_SCHEMA_V16}\"\nname = \"data-package\"\nversion = \"1.0.0\"\nprofile = \"{PROJECT_PROFILE_USEFUL_DATA_V2}\"\nentry = \"data.app\"\nsources = [\"app.spx\", \"owner.spx\"]\nweb_exports = [\"data.length\"]\ntests = [\"data.tests\"]\n");
    let manifest = ProjectManifest::parse(&text).unwrap();
    assert_eq!(manifest.to_canonical_toml(), text);
    let subject = PublicApiSubject {
        project_schema: PROJECT_SCHEMA_V16,
        project_revision: &"0".repeat(64),
        workspace_revision: &"1".repeat(64),
        project_graph_digest: &"2".repeat(64),
    };
    let prepared = prepare(&manifest, &program, subject).unwrap();
    assert_eq!(prepared.profile(), ProjectProfile::UsefulDataV2);
    assert!(prepared.owned_descriptor().is_none());
    assert!(crate::wasm::emit_resolved_module_with_byte_exports(&program, &roots).is_ok());
    assert!(graph
        .linked_scalar_program_with_roots("data.app", &roots, ProjectProfile::UsefulDataV1, false)
        .is_err());
    let owned_roots = vec!["data.private".to_owned()];
    let owned = graph
        .linked_scalar_program_with_roots(
            "data.app",
            &owned_roots,
            ProjectProfile::UsefulDataV2,
            false,
        )
        .unwrap();
    let hostile = ProjectManifest::parse(&text.replace(
        "web_exports = [\"data.length\"]",
        "web_exports = [\"data.private\"]",
    ))
    .unwrap();
    assert!(
        prepare(&hostile, &owned, subject).is_err(),
        "internal owning type is not a public byte descriptor"
    );
    assert!(
        ProjectManifest::parse(&text.replace(PROJECT_SCHEMA_V16, "semaprax.project.v3")).is_err()
    );
    assert!(ProjectManifest::parse(
        &text.replace(PROJECT_PROFILE_USEFUL_DATA_V2, "useful-data.v1")
    )
    .is_err());
    let private_text = text.replace("web_exports = [\"data.length\"]", "web_exports = []");
    let private_manifest = ProjectManifest::parse(&private_text).unwrap();
    let private_source = source.replace("fn main()->i64 {0}", "fn main()->i64 { let raw=[1u8]; let reader=Reader { data:bytes_copy(array_as_slice(raw)), position:0usize }; let moved=private_cursor(reader); 0 }");
    let private_sources = [
        ("app.spx", private_source.as_str()),
        ("owner.spx", provider),
    ]
    .into_iter()
    .map(|(path, text)| {
        let parsed = crate::parse(text, std::path::Path::new(path)).unwrap();
        WorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&parsed),
        }
    })
    .collect();
    let private_graph = workspace_graph::build_owned(private_sources).unwrap();
    let private_program = private_graph
        .linked_scalar_program_with_roots("data.app", &[], ProjectProfile::UsefulDataV2, false)
        .unwrap();
    assert!(private_program
        .functions
        .iter()
        .any(|function| function.id.as_str() == "data.private"));
    let prepared = prepare(&private_manifest, &private_program, subject).unwrap();
    assert!(prepared.owned_descriptor().is_none());
    let run =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&private_program, "data.main", 100_000)
            .unwrap();
    assert!(matches!(
        run.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(0)
    ));
}
