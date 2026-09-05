use semaprax::hir;
use semaprax::project::{
    derive_flat_owned_record_api_descriptor, render_flat_owned_record_c_header,
    render_flat_owned_record_cpp_header, render_flat_owned_record_metadata,
    render_flat_owned_record_rust, render_flat_owned_record_typescript,
    replay_flat_owned_record_api_descriptor, replay_flat_owned_record_metadata,
    FlatOwnedRecordFieldType, FlatOwnedRecordSettlement, ProjectManifest, ProjectProfile,
    PublicApiSubject, FLAT_OWNED_RECORD_API_SCHEMA, FLAT_OWNED_RECORD_METADATA_SCHEMA,
    FLAT_OWNED_RECORD_PROJECT_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../project_flat_owned_record_api_v1/admission.rs"]
mod admission;

#[path = "../project_flat_owned_record_api_v1/semantic_replay.rs"]
mod semantic_replay;

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
    let c = render_flat_owned_record_c_header(&descriptor);
    let cpp = render_flat_owned_record_cpp_header(&descriptor);
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
    assert!(c.starts_with("#ifndef SEMAPRAX_FLAT_OWNED_RECORD_V1_H\n"));
    assert!(c.contains("SPX_FLAT_RECORD_EXPORT_0_FIELD_COUNT UINT32_C(4)"));
    assert!(c.contains("SPX_FLAT_RECORD_EXPORT_0_FIELD_0_KIND SPX_FLAT_RECORD_OWNED_BYTES"));
    assert!(c.contains("#define SPX_FLAT_RECORD_STATIC(N) static N"));
    assert!(c.contains("uint64_t[SPX_FLAT_RECORD_STATIC(4)]"));
    assert!(cpp.contains(&format!("struct {record} {{")));
    assert!(cpp.contains(&format!("Bytes {payload};")));
    assert!(cpp.contains(&format!("std::int64_t {kind};")));
    assert!(cpp.contains(&format!("bool {valid};")));
    assert!(cpp.contains("Client(const Client&)=delete"));
    let public_record = cpp
        .split_once(&format!("struct {record} {{"))
        .unwrap()
        .1
        .split_once("};")
        .unwrap()
        .0;
    assert!(!public_record.contains("handle"));
    assert!(!public_record.contains("spx_owned_"));
    assert_eq!(c, render_flat_owned_record_c_header(&descriptor));
    assert_eq!(cpp, render_flat_owned_record_cpp_header(&descriptor));
    for forbidden in ["handle", "pointer", "offset", "unsafe", "repr(C)"] {
        assert!(!typescript.contains(forbidden), "{forbidden}");
        if forbidden != "unsafe" {
            assert!(!rust.contains(forbidden), "{forbidden}");
        }
    }
}

#[test]
fn generated_cpp17_adapter_returns_values_after_settling_the_actual_provider_owner() {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "semaprax-flat-record-cpp-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
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
    let export = &descriptor.exports()[0];
    let field = |stable_id: &str| {
        export
            .fields()
            .iter()
            .find(|field| field.stable_id().as_str() == stable_id)
            .unwrap()
            .host_name()
    };
    fs::write(
        root.join("semaprax_flat_owned_record.h"),
        render_flat_owned_record_c_header(&descriptor),
    )
    .unwrap();
    fs::write(
        root.join("semaprax_flat_owned_record.hpp"),
        render_flat_owned_record_cpp_header(&descriptor),
    )
    .unwrap();
    fs::write(root.join("provider.c"), provider.source()).unwrap();
    fs::write(
        root.join("consumer.cpp"),
        format!(
            r#"#include "semaprax_flat_owned_record.hpp"
#include <cstdint>
int main() {{
    using namespace semaprax::flat_owned_record_v1;
    const std::uint8_t input[]={{9,8,7}};
    Client client;
    auto value=client.{}(ByteView(input,sizeof(input)),true);
    if(value.{}.size()!=sizeof(input))return 1;
    for(std::size_t i=0;i<sizeof(input);++i)if(value.{}[i]!=input[i])return 2;
    if(value.{}!=7||!value.{}||value.{}!=sizeof(input))return 3;
    auto second=client.{}(ByteView(input,sizeof(input)),false);
    if(second.{}||second.{}.size()!=sizeof(input))return 4;
    return 0;
}}
"#,
            export.rust_method_name(),
            field("frame.info.payload"),
            field("frame.info.payload"),
            field("frame.info.kind"),
            field("frame.info.valid"),
            field("frame.info.size"),
            export.rust_method_name(),
            field("frame.info.valid"),
            field("frame.info.payload"),
        ),
    )
    .unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    let clangxx = std::env::var_os("CXX").unwrap_or_else(|| "clang++".into());
    for optimization in ["-O0", "-O2"] {
        let provider_object = format!("provider.c-{optimization}.o");
        let output = Command::new(&clang)
            .current_dir(&root)
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-c",
                "provider.c",
                "-o",
                provider_object.as_str(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let consumer_object = format!("consumer.cpp-{optimization}.o");
        let output = Command::new(&clangxx)
            .current_dir(&root)
            .args([
                "-std=c++17",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-c",
                "consumer.cpp",
                "-o",
                consumer_object.as_str(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let executable = format!("consumer-{optimization}");
        let output = Command::new(&clangxx)
            .current_dir(&root)
            .args([
                provider_object,
                consumer_object,
                "-o".to_owned(),
                executable.clone(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(root.join(executable))
            .status()
            .unwrap()
            .success());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_c11_header_links_the_flat_record_provider_and_settles_its_owner() {
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "semaprax-flat-record-c-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
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
    let export = &descriptor.exports()[0];
    fs::write(
        root.join("semaprax_flat_owned_record.h"),
        render_flat_owned_record_c_header(&descriptor),
    )
    .unwrap();
    fs::write(root.join("provider.c"), provider.source()).unwrap();
    fs::write(
        root.join("consumer.c"),
        format!(
            r#"#include "semaprax_flat_owned_record.h"
#include <stddef.h>
#include <stdint.h>
#include <string.h>
union aligned_context {{ max_align_t alignment; uint8_t bytes[UINT64_C(1) << 20]; }};
int main(void) {{
    union aligned_context storage;
    uint64_t size=spx_owned_data_context_size_v1(),align=spx_owned_data_context_align_v1();
    if(!size||size>sizeof(storage.bytes)||!align||align>_Alignof(max_align_t))return 1;
    if(spx_owned_data_context_init_v1(storage.bytes,size)!=SPX_OWNED_DATA_SUCCESS)return 2;
    spx_context_v1*context=(spx_context_v1*)storage.bytes;
    const uint8_t input[]={{9,8,7}};
    uint64_t carrier[SPX_FLAT_RECORD_EXPORT_0_FIELD_COUNT];
    for(uint32_t i=0;i<SPX_FLAT_RECORD_EXPORT_0_FIELD_COUNT;++i)carrier[i]=UINT64_MAX;
    if(spx_owned_data_call_{}_v1(context,input,sizeof(input),UINT8_C(2),carrier)!=SPX_OWNED_DATA_ADAPTER_FAILURE)return 3;
    for(uint32_t i=0;i<SPX_FLAT_RECORD_EXPORT_0_FIELD_COUNT;++i)if(carrier[i]!=UINT64_MAX)return 4;
    if(spx_owned_data_call_{}_v1(context,input,sizeof(input),UINT8_C(1),carrier)!=SPX_OWNED_DATA_SUCCESS)return 5;
    spx_owned_bytes_handle_v1 handle=carrier[SPX_FLAT_RECORD_EXPORT_0_FIELD_0];
    int64_t count=0;uint64_t count_bits=carrier[SPX_FLAT_RECORD_EXPORT_0_FIELD_1];memcpy(&count,&count_bits,sizeof(count));
    if(!handle||count!=INT64_C(7)||carrier[SPX_FLAT_RECORD_EXPORT_0_FIELD_2]!=UINT64_C(1)||carrier[SPX_FLAT_RECORD_EXPORT_0_FIELD_3]!=sizeof(input))return 6;
    uint64_t length=UINT64_MAX;if(spx_owned_bytes_len_v1(context,handle,&length)!=SPX_OWNED_DATA_SUCCESS||length!=sizeof(input))return 7;
    uint8_t output[3]={{0}};if(spx_owned_bytes_copy_v1(context,handle,output,length)!=SPX_OWNED_DATA_SUCCESS||memcmp(input,output,sizeof(input)))return 8;
    if(spx_owned_bytes_drop_v1(context,handle)!=SPX_OWNED_DATA_SUCCESS)return 9;
    if(spx_owned_bytes_drop_v1(context,handle)!=SPX_OWNED_DATA_INVALID_HANDLE)return 10;
    if(spx_owned_data_context_drop_v1(context)!=SPX_OWNED_DATA_SUCCESS)return 11;
    return 0;
}}
"#,
            export.rust_method_name(),
            export.rust_method_name(),
        ),
    )
    .unwrap();
    let clang = std::env::var_os("CLANG").unwrap_or_else(|| "clang".into());
    for optimization in ["-O0", "-O2"] {
        for input in ["provider.c", "consumer.c"] {
            let object = format!("{input}-{optimization}.o");
            let output = Command::new(&clang)
                .current_dir(&root)
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-c",
                    input,
                    "-o",
                    object.as_str(),
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let executable = format!("consumer-{optimization}");
        let output = Command::new(&clang)
            .current_dir(&root)
            .args([
                format!("provider.c-{optimization}.o"),
                format!("consumer.c-{optimization}.o"),
                "-o".to_owned(),
                executable.clone(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(Command::new(root.join(executable))
            .status()
            .unwrap()
            .success());
    }
    fs::remove_dir_all(root).unwrap();
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
    let workflow = include_str!("../../src/project_transport/session/workflow.rs");
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
