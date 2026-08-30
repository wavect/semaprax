use serde_json::Value;
use sha2::Sha256;

use super::*;

pub(crate) mod ffi_boundaries;

fn descriptor_bytes(result: &str) -> Vec<u8> {
    format!(
        "{{\"schema\":\"semaprax.public-owned-data-api.v1\",\"project_schema\":\"semaprax.project.v8\",\"project_revision\":\"sha256:{}\",\"workspace_revision\":\"sha256:{}\",\"project_graph_digest\":\"sha256:{}\",\"exports\":[{{\"stable_id\":\"fixture.value\",\"typescript_name\":\"fixture.value\",\"rust_method_name\":\"spx_fixture_dot_value\",\"parameters\":[],\"result\":\"{result}\"}}],\"limits\":{{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576}}}}\n",
        "1".repeat(64), "2".repeat(64), "3".repeat(64),
    ).into_bytes()
}

fn utf8_descriptor_bytes() -> Vec<u8> {
    format!(
        "{{\"schema\":\"semaprax.public-owned-utf8-api.v1\",\"project_schema\":\"semaprax.project.v10\",\"project_revision\":\"sha256:{}\",\"workspace_revision\":\"sha256:{}\",\"project_graph_digest\":\"sha256:{}\",\"exports\":[{{\"stable_id\":\"fixture.count\",\"typescript_name\":\"fixture.count\",\"rust_method_name\":\"spx_fixture_dot_count\",\"parameters\":[],\"result\":\"i64\"}},{{\"stable_id\":\"fixture.text\",\"typescript_name\":\"fixture.text\",\"rust_method_name\":\"spx_fixture_dot_text\",\"parameters\":[],\"result\":\"owned-utf8\"}}],\"limits\":{{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576}}}}\n",
        "1".repeat(64), "2".repeat(64), "3".repeat(64),
    ).into_bytes()
}

#[test]
fn descriptor_digest_has_the_frozen_length_prefix_and_rejects_scalar_row_drift() {
    let bytes = descriptor_bytes("usize");
    let digest = descriptor_digest(&bytes);
    let selected = vec!["fixture.value".to_owned()];
    assert_eq!(
        descriptor::replay(&bytes, &digest, &selected)
            .unwrap()
            .exports_len(),
        1
    );
    let mut without_length = Sha256::new();
    without_length.update(DESCRIPTOR_DIGEST_DOMAIN);
    without_length.update(&bytes);
    assert_ne!(
        digest,
        format!("sha256:{:x}", LowerHex(without_length.finalize()))
    );
    let missing = String::from_utf8(bytes.clone())
        .unwrap()
        .replace("\"result\":\"usize\"", "\"result\":null")
        .into_bytes();
    assert!(descriptor::replay(&missing, &descriptor_digest(&missing), &selected).is_err());
    let mut surplus: Value = serde_json::from_slice(&bytes).unwrap();
    surplus["exports"][0]["surplus"] = Value::Bool(true);
    let mut surplus = serde_json::to_vec(&surplus).unwrap();
    surplus.push(b'\n');
    assert!(descriptor::replay(&surplus, &descriptor_digest(&surplus), &selected).is_err());
}

#[test]
fn provider_binding_is_exact_unique_and_descriptor_specific() {
    let digest = descriptor_digest(&descriptor_bytes("owned-bytes"));
    let line = format!("#define SPX_OWNED_DATA_DESCRIPTOR_DIGEST_V1 \"{digest}\"");
    let provider = format!("#define SPX_NO_ENTRY_WRAPPER 1\n{line}\nint x;\n");
    assert!(provider_binds_descriptor(provider.as_bytes(), &digest));
    assert!(!provider_binds_descriptor(
        format!("{provider}{line}\n").as_bytes(),
        &digest
    ));
    assert!(!provider_binds_descriptor(
        provider.as_bytes(),
        &descriptor_digest(&descriptor_bytes("i64"))
    ));
}

#[test]
fn standalone_source_and_manifest_shape_remain_frozen_while_project_mode_is_additive() {
    let bytes = descriptor_bytes("owned-bytes");
    let digest = descriptor_digest(&bytes);
    let descriptor = descriptor::replay(&bytes, &digest, &["fixture.value".to_owned()]).unwrap();
    for target in [
        HostTarget::X86_64LinuxGnu,
        HostTarget::Aarch64LinuxGnu,
        HostTarget::X86_64Darwin,
        HostTarget::Aarch64Darwin,
        HostTarget::X86_64WindowsMsvc,
    ] {
        let standalone =
            render::render_sources(&descriptor, target, PackageMode::StandaloneEvidence);
        let project = render::render_sources(&descriptor, target, PackageMode::ProjectV8);
        assert!(standalone
            .ffi_rs
            .contains("if bytes.capacity()!=length{return Err(Failure::Host)}"));
        assert!(!project
            .ffi_rs
            .contains("if bytes.capacity()!=length{return Err(Failure::Host)}"));
        let archive = target.archive_name();
        let files = [
            ("Cargo.toml", standalone.cargo_toml.as_bytes()),
            ("build.rs", standalone.build_rs.as_bytes()),
            ("lib.rs", standalone.lib_rs.as_bytes()),
            ("owned_data_ffi.rs", standalone.ffi_rs.as_bytes()),
            (archive, b"archive".as_slice()),
            ("descriptor.json", bytes.as_slice()),
        ];
        let manifest = render::render_manifest(
            target,
            &bytes,
            &digest,
            archive,
            PackageMode::StandaloneEvidence,
            "sha256:provider",
            files,
        );
        let value: Value = serde_json::from_str(&manifest).unwrap();
        assert_eq!(value["provider"].as_object().unwrap().len(), 3);
        assert!(value["nonclaims"]
            .as_array()
            .unwrap()
            .iter()
            .any(|value| value == "no_project_v8_activation"));
    }
}

#[test]
fn v10_replay_uses_its_exact_domain_and_renders_mixed_safe_utf8_settlement() {
    let bytes = utf8_descriptor_bytes();
    let digest = utf8_descriptor_digest(&bytes);
    let selected = vec!["fixture.count".to_owned(), "fixture.text".to_owned()];
    let descriptor = descriptor::replay(&bytes, &digest, &selected).unwrap();
    assert_eq!(descriptor.exports_len(), 2);
    assert!(descriptor::replay(&bytes, &descriptor_digest(&bytes), &selected).is_err());
    for target in [
        HostTarget::X86_64LinuxGnu,
        HostTarget::Aarch64LinuxGnu,
        HostTarget::X86_64Darwin,
        HostTarget::Aarch64Darwin,
        HostTarget::X86_64WindowsMsvc,
    ] {
        let sources = render::render_sources(&descriptor, target, PackageMode::ProjectV10OwnedUtf8);
        assert!(sources.lib_rs.contains("pub fn spx_fixture_dot_count"));
        assert!(
            sources.lib_rs.find("copy_and_settle(raw.handle)").unwrap()
                < sources.lib_rs.find("String::from_utf8(bytes)").unwrap()
        );
        assert!(sources.ffi_rs.contains("pub fn call_spx_fixture_dot_text"));
    }
}

#[test]
fn unknown_descriptor_schema_cannot_select_a_digest_domain() {
    let bytes = String::from_utf8(utf8_descriptor_bytes())
        .unwrap()
        .replace(
            "semaprax.public-owned-utf8-api.v1",
            "semaprax.public-owned-utf8-api.v999",
        )
        .into_bytes();
    assert!(descriptor_digest_for_bytes(&bytes).is_none());
    assert!(descriptor::replay(
        &bytes,
        &utf8_descriptor_digest(&bytes),
        &["fixture.count".to_owned(), "fixture.text".to_owned()]
    )
    .is_err());
}

#[test]
fn package_mode_and_descriptor_schema_are_closed_before_publication_authority() {
    let v8 = descriptor_bytes("owned-bytes");
    let v10 = utf8_descriptor_bytes();
    let v8_descriptor =
        descriptor::replay(&v8, &descriptor_digest(&v8), &["fixture.value".to_owned()]).unwrap();
    let v10_descriptor = descriptor::replay(
        &v10,
        &utf8_descriptor_digest(&v10),
        &["fixture.count".to_owned(), "fixture.text".to_owned()],
    )
    .unwrap();
    assert_eq!(v8_descriptor.schema, PUBLIC_OWNED_DATA_API_SCHEMA);
    assert_eq!(v10_descriptor.schema, PUBLIC_OWNED_UTF8_API_SCHEMA);
    assert!(!mode_accepts_descriptor(
        PackageMode::ProjectV10OwnedUtf8,
        v8_descriptor.schema
    ));
    assert!(!mode_accepts_descriptor(
        PackageMode::ProjectV8,
        v10_descriptor.schema
    ));
    assert!(mode_accepts_descriptor(
        PackageMode::ProjectV10OwnedUtf8,
        v10_descriptor.schema
    ));
}

#[test]
fn windows_publication_freezes_the_explicit_toolchain_environment() {
    let source = include_str!("publication.rs");
    for required in [
        "required_environment(\"INCLUDE\")",
        "required_environment(\"LIB\")",
        "prepare_process_arena_plan_with_environment(1, include, libraries)",
        "materialize_process_arena_with_environment(",
    ] {
        assert!(
            source.contains(required),
            "owned-data publication is missing `{required}`"
        );
    }
}
