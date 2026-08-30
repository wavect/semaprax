use semaprax::hir;
use semaprax::project::{
    derive_flat_owned_record_api_descriptor, render_flat_owned_record_metadata,
    render_flat_owned_record_rust, render_flat_owned_record_typescript,
    replay_flat_owned_record_api_descriptor, replay_flat_owned_record_metadata,
    FlatOwnedRecordFieldType, FlatOwnedRecordSettlement, ProjectManifest, ProjectProfile,
    PublicApiSubject, FLAT_OWNED_RECORD_API_SCHEMA, FLAT_OWNED_RECORD_METADATA_SCHEMA,
    FLAT_OWNED_RECORD_PROJECT_SCHEMA,
};
use sha2::{Digest, Sha256};

#[path = "project_flat_owned_record_api_v1/admission.rs"]
mod admission;

const PROJECT_REVISION: &str =
    "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const WORKSPACE_REVISION: &str =
    "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const GRAPH_DIGEST: &str =
    "sha256:3333333333333333333333333333333333333333333333333333333333333333";

const MANIFEST: &str = "schema = \"semaprax.project.v9\"\nname = \"frame-info\"\nversion = \"0.1.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"frame.app\"\nsources = [\"src/app.spx\", \"src/core.spx\"]\nweb_exports = [\"frame.info\"]\ntests = [\"frame.tests\"]\n";

const SOURCE: &str = r#"module frame.api;

@id("frame.info.type")
record FrameInfo {
    @id("frame.info.payload") payload: Bytes,
    @id("frame.info.kind") kind: i64,
    @id("frame.info.valid") valid: bool,
    @id("frame.info.size") size: usize,
}

@id("frame.info")
fn frame_info(value: borrow Slice<u8>, valid: bool) -> FrameInfo {
    FrameInfo {
        payload: bytes_copy(value),
        kind: 7,
        valid: valid,
        size: byte_len(value),
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: FLAT_OWNED_RECORD_PROJECT_SCHEMA,
        project_revision: PROJECT_REVISION,
        workspace_revision: WORKSPACE_REVISION,
        project_graph_digest: GRAPH_DIGEST,
    }
}

fn resolve(source: &str) -> hir::ResolvedProgram {
    let checked = semaprax::check(source, "flat-owned-record.spx").unwrap();
    hir::resolve(&checked).unwrap()
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.public-flat-owned-record-api.digest.v1\0");
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        use std::fmt::Write as _;
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("sha256:{hex}")
}

#[test]
fn project_v9_manifest_is_exact_and_schema_bound() {
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    assert_eq!(manifest.to_canonical_toml(), MANIFEST);
    assert_eq!(
        manifest.project_profile(),
        ProjectProfile::FlatOwnedRecordApiV1
    );
    assert!(manifest.is_v9());

    for hostile in [
        MANIFEST.replace("semaprax.project.v9", "semaprax.project.v8"),
        MANIFEST.replace("flat-owned-record-api.v1", "owned-data-api.v1"),
        MANIFEST.replace("version = \"0.1.0\"\n", ""),
        MANIFEST.replace("entry =", "unknown = \"x\"\nentry ="),
        MANIFEST.trim_end().to_owned(),
    ] {
        assert!(ProjectManifest::parse(&hostile).is_err(), "{hostile}");
    }
}

#[test]
fn descriptor_binds_exact_record_fields_and_replays() {
    let program = resolve(SOURCE);
    let selected = vec!["frame.info".to_owned()];
    let descriptor =
        derive_flat_owned_record_api_descriptor(&program, &selected, subject()).unwrap();
    let bytes = descriptor.canonical_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains(FLAT_OWNED_RECORD_API_SCHEMA));
    let export = &descriptor.exports()[0];
    assert_eq!(
        export
            .fields()
            .iter()
            .map(|field| field.ty())
            .collect::<Vec<_>>(),
        [
            FlatOwnedRecordFieldType::OwnedBytes,
            FlatOwnedRecordFieldType::I64,
            FlatOwnedRecordFieldType::Bool,
            FlatOwnedRecordFieldType::Usize,
        ]
    );
    assert_eq!(descriptor.carrier_plans()[0].owned_field_ordinal, 0);
    assert_eq!(
        descriptor.carrier_plans()[0].scalar_field_ordinals,
        [1, 2, 3]
    );
    assert!(descriptor.carrier_plans()[0].copy_before_settle);
    assert!(descriptor.carrier_plans()[0].publish_after_settle);
    assert_eq!(
        replay_flat_owned_record_api_descriptor(
            &program,
            &selected,
            subject(),
            &bytes,
            &descriptor.digest(),
        )
        .unwrap(),
        descriptor
    );
}

#[test]
fn flat_record_provider_reuses_the_context_bound_issuer_without_descriptor_changes() {
    let program = resolve(SOURCE);
    let selected = vec!["frame.info".to_owned()];
    let descriptor =
        derive_flat_owned_record_api_descriptor(&program, &selected, subject()).unwrap();
    let bytes = descriptor.canonical_bytes();
    let digest = descriptor.digest();
    let provider = semaprax::codegen::emit_project_v9_native_flat_owned_record_provider(
        &program,
        &selected,
        subject(),
        &bytes,
        &digest,
    )
    .unwrap();
    assert_eq!(provider.descriptor(), bytes);
    assert_eq!(provider.descriptor_digest(), digest);
    assert!(provider
        .source()
        .contains("slot->issuance_serial == serial"));
    assert!(provider.source().contains("handle & UINT64_C(0x1fff)"));
    assert_eq!(
        provider
            .source()
            .matches("atomic_compare_exchange_strong_explicit")
            .count(),
        1
    );
    assert_eq!(descriptor.canonical_bytes(), bytes);
}

#[test]
fn descriptor_rejects_mutation_even_with_a_reminted_digest() {
    let program = resolve(SOURCE);
    let selected = vec!["frame.info".to_owned()];
    let descriptor =
        derive_flat_owned_record_api_descriptor(&program, &selected, subject()).unwrap();
    let mut bytes = descriptor.canonical_bytes();
    let position = bytes.iter().position(|byte| *byte == b'7').unwrap();
    bytes[position] = b'8';
    assert!(replay_flat_owned_record_api_descriptor(
        &program,
        &selected,
        subject(),
        &bytes,
        &digest(&bytes),
    )
    .is_err());
}

#[test]
fn admission_rejects_every_wider_record_family() {
    for (field, ty) in [
        (0, hir::ResolvedType::I64),
        (1, hir::ResolvedType::Bytes),
        (1, hir::ResolvedType::String),
        (1, hir::ResolvedType::Str),
        (1, hir::ResolvedType::ArrayU8(1)),
        (1, hir::ResolvedType::SliceU8),
        (
            1,
            hir::ResolvedType::Nominal {
                declaration: hir::DeclarationId::new("nested.record"),
                arguments: Vec::new(),
            },
        ),
    ] {
        let mut program = resolve(SOURCE);
        let hir::ResolvedTypeDeclarationKind::Record { fields } = &mut program.types[0].kind else {
            panic!("fixture result type is not a record")
        };
        fields[field].ty = ty;
        assert!(derive_flat_owned_record_api_descriptor(
            &program,
            &["frame.info".to_owned()],
            subject(),
        )
        .is_err());
    }
}

#[test]
fn generated_mappings_are_safe_and_hide_the_carrier() {
    let program = resolve(SOURCE);
    let descriptor =
        derive_flat_owned_record_api_descriptor(&program, &["frame.info".to_owned()], subject())
            .unwrap();
    let typescript = render_flat_owned_record_typescript(&descriptor);
    let rust = render_flat_owned_record_rust(&descriptor);
    let export = &descriptor.exports()[0];
    let record = export.record_host_name();
    let payload = export
        .fields()
        .iter()
        .find(|field| field.stable_id().as_str() == "frame.info.payload")
        .unwrap()
        .host_name();
    let kind = export
        .fields()
        .iter()
        .find(|field| field.stable_id().as_str() == "frame.info.kind")
        .unwrap()
        .host_name();
    let valid = export
        .fields()
        .iter()
        .find(|field| field.stable_id().as_str() == "frame.info.valid")
        .unwrap()
        .host_name();
    assert!(typescript.contains("readonly"));
    assert!(typescript.contains(&format!("interface {record}")));
    assert!(typescript.contains(&format!("readonly {payload}: Uint8Array")));
    assert!(typescript.contains(&format!("readonly {kind}: bigint")));
    assert!(typescript.contains(&format!("readonly {valid}: boolean")));
    assert!(typescript.contains("Uint8Array"));
    assert!(typescript.contains("bigint"));
    assert!(rust.starts_with("#![forbid(unsafe_code)]"));
    assert!(rust.contains(&format!("pub struct {record}")));
    assert!(rust.contains(&format!("pub {payload}: Vec<u8>")));
    assert!(rust.contains(&format!("pub {kind}: i64")));
    assert!(rust.contains(&format!("pub {valid}: bool")));
    assert!(rust.contains("Vec<u8>"));
    for forbidden in ["handle", "pointer", "offset", "unsafe", "repr(C)"] {
        assert!(!typescript.contains(forbidden), "{forbidden}");
        if forbidden != "unsafe" {
            assert!(!rust.contains(forbidden), "{forbidden}");
        }
    }
}

#[test]
fn display_renames_preserve_stable_id_derived_host_identities() {
    let original = derive_flat_owned_record_api_descriptor(
        &resolve(SOURCE),
        &["frame.info".to_owned()],
        subject(),
    )
    .unwrap();
    let renamed_source = SOURCE
        .replace("FrameInfo", "RenamedFrame")
        .replace("payload:", "renamed_payload:");
    let renamed = derive_flat_owned_record_api_descriptor(
        &resolve(&renamed_source),
        &["frame.info".to_owned()],
        subject(),
    )
    .unwrap();
    let original_export = &original.exports()[0];
    let renamed_export = &renamed.exports()[0];
    assert_eq!(
        original_export.record_host_name(),
        renamed_export.record_host_name()
    );
    assert_eq!(
        original_export.record_host_name(),
        "SpxRecordId6672616d652e696e666f2e74797065"
    );
    assert_eq!(
        original_export
            .fields()
            .iter()
            .map(|field| (field.stable_id(), field.host_name()))
            .collect::<Vec<_>>(),
        renamed_export
            .fields()
            .iter()
            .map(|field| (field.stable_id(), field.host_name()))
            .collect::<Vec<_>>()
    );
    assert_ne!(original.canonical_bytes(), renamed.canonical_bytes());
    assert_eq!(
        original_export
            .fields()
            .iter()
            .find(|field| field.stable_id().as_str() == "frame.info.payload")
            .unwrap()
            .host_name(),
        "spx_field_id_6672616d652e696e666f2e7061796c6f6164"
    );
}

#[test]
fn publication_requires_authenticate_copy_settle_then_publish() {
    let mut settled = FlatOwnedRecordSettlement::received();
    settled.authenticated().unwrap();
    settled.copy_completed().unwrap();
    settled.settlement_completed().unwrap();
    settled.publish().unwrap();
    assert!(settled.is_published());

    for advance in [1_u8, 2, 3] {
        let mut hostile = FlatOwnedRecordSettlement::received();
        let result = match advance {
            1 => hostile.copy_completed(),
            2 => hostile.settlement_completed(),
            3 => hostile.publish(),
            _ => unreachable!(),
        };
        assert!(result.is_err());
        assert!(!hostile.is_published());
        assert!(hostile.authenticated().is_err(), "failure must be sticky");
    }

    let mut failed = FlatOwnedRecordSettlement::received();
    failed.authenticated().unwrap();
    failed.fail().unwrap();
    assert!(failed.copy_completed().is_err());

    assert!(settled.fail().is_err());
    assert!(settled.is_published());
}

#[test]
fn npm_metadata_binds_one_descriptor_and_private_settlement() {
    let program = resolve(SOURCE);
    let descriptor =
        derive_flat_owned_record_api_descriptor(&program, &["frame.info".to_owned()], subject())
            .unwrap();
    let wasm_digest = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let npm = render_flat_owned_record_metadata(&descriptor, wasm_digest).unwrap();
    replay_flat_owned_record_metadata(&descriptor, wasm_digest, &npm).unwrap();
    assert!(String::from_utf8_lossy(&npm).contains(FLAT_OWNED_RECORD_METADATA_SCHEMA));
    assert!(npm
        .windows(descriptor.digest().len())
        .any(|window| { window == descriptor.digest().as_bytes() }));
    assert!(render_flat_owned_record_metadata(&descriptor, "sha256:bad").is_err());
    let mut tampered = npm.clone();
    tampered.insert(tampered.len() - 2, b' ');
    assert!(replay_flat_owned_record_metadata(&descriptor, wasm_digest, &tampered).is_err());
}

#[test]
fn transport_v4_rejects_v9_before_target_or_carrier_selection() {
    let workflow = include_str!("../src/project_transport/session/workflow.rs");
    let rejection = workflow
        .find("ProjectProfile::FlatOwnedRecordApiV1")
        .expect("Transport v4 must close Project v9 builds");
    let target = workflow[rejection..]
        .find("take_string(&mut params, \"target\")")
        .expect("target parsing remains after the Project v9 rejection");
    let carrier = workflow[rejection..]
        .find("snapshot.build_npm_inline(max_bytes)")
        .expect("carrier selection remains after the Project v9 rejection");
    assert!(target > 0 && carrier > target);
    assert!(workflow[rejection..rejection + target].contains("not admitted by Agent Transport v4"));
}
