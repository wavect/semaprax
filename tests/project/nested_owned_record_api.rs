use semaprax::hir;
use semaprax::project::{
    derive_nested_owned_record_api_descriptor, replay_nested_owned_record_api_descriptor,
    NestedOwnedRecordFieldType, NestedOwnedRecordLeafType, PublicApiSubject,
    NESTED_OWNED_RECORD_API_SCHEMA, NESTED_OWNED_RECORD_PROJECT_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(std::path::PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const SOURCE: &str = r#"module nested.api;
@id("nested.payload") record Payload {
    @id("nested.payload.bytes") bytes: Bytes,
    @id("nested.payload.size") size: usize,
}
@id("nested.envelope") record Envelope {
    @id("nested.envelope.left") left: Payload,
    @id("nested.envelope.enabled") enabled: bool,
    @id("nested.envelope.right") right: Payload,
}
@id("nested.build") fn build(input: borrow Slice<u8>) -> Envelope {
    Envelope {
        left: Payload { bytes: bytes_copy(input), size: byte_len(input) },
        enabled: true,
        right: Payload { bytes: bytes_copy(input), size: byte_len(input) },
    }
}
@id("nested.app") fn main() -> i64 { 0 }
"#;

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: NESTED_OWNED_RECORD_PROJECT_SCHEMA,
        project_revision: "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        workspace_revision:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        project_graph_digest:
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    }
}

fn resolved() -> hir::ResolvedProgram {
    hir::resolve(&semaprax::check(SOURCE, "nested-api.spx").unwrap()).unwrap()
}

fn digest(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.public-nested-owned-record-api.digest.v1\0");
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

#[test]
fn descriptor_deduplicates_nominal_types_but_preserves_occurrence_paths() {
    let program = resolved();
    let selected = vec!["nested.build".to_owned()];
    let descriptor =
        derive_nested_owned_record_api_descriptor(&program, &selected, subject()).unwrap();
    assert_eq!(descriptor.records().len(), 2);
    let export = &descriptor.exports()[0];
    assert_eq!(export.leaves().len(), 5);
    assert_eq!(
        export
            .leaves()
            .iter()
            .map(|leaf| leaf.ty())
            .collect::<Vec<_>>(),
        [
            NestedOwnedRecordLeafType::OwnedBytes,
            NestedOwnedRecordLeafType::Usize,
            NestedOwnedRecordLeafType::Bool,
            NestedOwnedRecordLeafType::OwnedBytes,
            NestedOwnedRecordLeafType::Usize,
        ]
    );
    assert_eq!(
        export.leaves()[0]
            .field_path()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        ["nested.envelope.left", "nested.payload.bytes"]
    );
    assert_eq!(
        export.leaves()[3]
            .field_path()
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        ["nested.envelope.right", "nested.payload.bytes"]
    );
    let bytes = descriptor.canonical_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains(NESTED_OWNED_RECORD_API_SCHEMA));
    assert_eq!(
        replay_nested_owned_record_api_descriptor(
            &program,
            &selected,
            subject(),
            &bytes,
            &descriptor.digest()
        )
        .unwrap(),
        descriptor
    );
}

#[test]
fn descriptor_rejects_closed_shape_mutation_and_flat_legacy_does_not_widen() {
    let program = resolved();
    let selected = vec!["nested.build".to_owned()];
    let descriptor =
        derive_nested_owned_record_api_descriptor(&program, &selected, subject()).unwrap();
    let mut bytes = descriptor.canonical_bytes();
    let end = bytes.len() - 2;
    bytes.splice(end..end, b",\"foreign\":true".iter().copied());
    assert!(replay_nested_owned_record_api_descriptor(
        &program,
        &selected,
        subject(),
        &bytes,
        &digest(&bytes)
    )
    .is_err());
    let forged = String::from_utf8(descriptor.canonical_bytes())
        .unwrap()
        .replacen("nested.payload.bytes", "nested.payload.forged", 1)
        .into_bytes();
    assert!(replay_nested_owned_record_api_descriptor(
        &program,
        &selected,
        subject(),
        &forged,
        &digest(&forged),
    )
    .is_err());
    assert!(semaprax::project::derive_flat_owned_record_api_descriptor(
        &program,
        &selected,
        PublicApiSubject {
            project_schema: semaprax::project::FLAT_OWNED_RECORD_PROJECT_SCHEMA,
            ..subject()
        },
    )
    .is_err());
    assert!(descriptor
        .records()
        .iter()
        .flat_map(|record| record.fields())
        .any(|field| matches!(field.ty(), NestedOwnedRecordFieldType::Record(_))));
}

#[test]
fn excluded_nested_result_shapes_fail_before_descriptor_creation() {
    for (field_type, initializer) in [("string", "\"text\""), ("[u8; 1]", "[1u8]")] {
        let source = format!(
            r#"module hostile.api;
@id("hostile.inner") record Inner {{
    @id("hostile.inner.bytes") bytes: Bytes,
    @id("hostile.inner.closed") closed: {field_type},
}}
@id("hostile.outer") record Outer {{ @id("hostile.outer.inner") inner: Inner, }}
@id("hostile.make") fn make(input: borrow Slice<u8>) -> Outer {{
    Outer {{ inner: Inner {{ bytes: bytes_copy(input), closed: {initializer} }} }}
}}
@id("hostile.app") fn main() -> i64 {{ 0 }}
"#
        );
        assert!(semaprax::check(&source, "hostile-nested-api.spx").is_err());
    }
}

#[test]
fn retained_v11_subject_replays_the_same_descriptor() {
    let root = Fixture(std::env::temp_dir().join(format!(
        "semaprax-nested-record-api-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    )));
    std::fs::create_dir_all(root.0.join("src")).unwrap();
    let manifest = "schema = \"semaprax.project.v11\"\nname = \"nested-api\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"nested.api\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"nested.build\"]\ntests = [\"nested.tests\"]\n";
    let source =
        semaprax::format::canonical(&semaprax::parse(SOURCE, Path::new("src/app.spx")).unwrap());
    let tests = semaprax::format::canonical(
        &semaprax::parse(
            "module nested.tests;\n@id(\"nested.tests.main\") fn main() -> i64 { 0 }\n",
            Path::new("src/tests.spx"),
        )
        .unwrap(),
    );
    std::fs::write(root.0.join("semaprax.toml"), manifest).unwrap();
    std::fs::write(root.0.join("src/app.spx"), source).unwrap();
    std::fs::write(root.0.join("src/tests.spx"), tests).unwrap();
    let manifest_path = root.0.join("semaprax.toml").canonicalize().unwrap();
    let (retained, descriptor) =
        semaprax::project::with_authenticated_project(&manifest_path, |snapshot| {
            let descriptor = snapshot.nested_owned_record_api_descriptor()?;
            assert!(snapshot.flat_owned_record_api_descriptor().is_err());
            assert!(snapshot.public_api_descriptor().is_err());
            Ok((snapshot.retain_revision(), descriptor))
        })
        .unwrap();
    assert_eq!(
        retained.nested_owned_record_api_descriptor().unwrap(),
        descriptor
    );
}
