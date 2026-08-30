//! Shared real compiler subject. No Project activation or packaging authority.
use semaprax::project::{self, PublicApiSubject};

const SOURCE: &str = include_str!("source.spx");
const FACT: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";

pub struct Artifact {
    pub provider: String,
    pub descriptor: Vec<u8>,
    pub digest: String,
    pub selected: Vec<String>,
}

pub fn artifact(flat: bool) -> Artifact {
    let checked = semaprax::check(SOURCE, "standalone-sdk-strings.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    let rechecked = semaprax::check(&canonical, "standalone-sdk-strings.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&rechecked), canonical);
    let program = semaprax::hir::resolve(&checked).unwrap();
    let selected = if flat {
        vec!["s.record".to_owned()]
    } else {
        [
            "s.callee",
            "s.clone",
            "s.concat",
            "s.empty",
            "s.late",
            "s.local",
            "s.loop",
            "s.mixed",
            "s.mixed-late",
            "s.mixed-reverse",
            "s.nul",
        ]
        .map(str::to_owned)
        .to_vec()
    };
    let subject = PublicApiSubject {
        project_schema: if flat {
            project::FLAT_OWNED_RECORD_PROJECT_SCHEMA
        } else {
            project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA
        },
        project_revision: FACT,
        workspace_revision: FACT,
        project_graph_digest: FACT,
    };
    let (descriptor, digest, emitted) = if flat {
        let descriptor =
            project::derive_flat_owned_record_api_descriptor(&program, &selected, subject).unwrap();
        let bytes = descriptor.canonical_bytes();
        let digest = descriptor.digest();
        assert_eq!(
            project::replay_flat_owned_record_api_descriptor(
                &program, &selected, subject, &bytes, &digest,
            )
            .unwrap(),
            descriptor
        );
        let emitted = semaprax::codegen::emit_project_v9_native_flat_owned_record_provider(
            &program, &selected, subject, &bytes, &digest,
        )
        .unwrap();
        (bytes, digest, emitted)
    } else {
        let descriptor =
            project::derive_public_api_descriptor(&program, &selected, subject).unwrap();
        let bytes = descriptor.canonical_bytes();
        let digest = descriptor.digest();
        assert_eq!(
            project::replay_public_api_descriptor(&program, &selected, subject, &bytes, &digest,)
                .unwrap(),
            descriptor
        );
        let emitted = semaprax::codegen::emit_native_owned_data_provider(
            &program, &selected, subject, &bytes, &digest,
        )
        .unwrap();
        (bytes, digest, emitted)
    };
    assert_eq!(emitted.descriptor(), descriptor);
    assert_eq!(emitted.descriptor_digest(), digest);
    Artifact {
        provider: emitted.source().to_owned(),
        descriptor,
        digest,
        selected,
    }
}
