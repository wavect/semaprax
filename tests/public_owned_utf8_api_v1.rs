use semaprax::project::{
    derive_public_api_descriptor, replay_public_api_descriptor, PublicApiResultType,
    PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA, PUBLIC_OWNED_UTF8_API_SCHEMA,
    PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
};
use std::fs;
use std::process::Command;

const FACT: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const SOURCE: &str = r#"module utf8.api;
@id("utf8.greeting")
fn greeting() -> string { "hello\0世界" }
@id("utf8.forward")
fn forward() -> string { greeting() }
@id("bytes.raw")
fn raw(value: borrow Slice<u8>) -> Bytes { bytes_copy(value) }
@id("app.main")
fn main() -> i64 { 0 }
"#;

fn subject(project_schema: &'static str) -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema,
        project_revision: FACT,
        workspace_revision: FACT,
        project_graph_digest: FACT,
    }
}

fn program() -> semaprax::hir::ResolvedProgram {
    let checked = semaprax::check(SOURCE, "owned-utf8.spx").unwrap();
    semaprax::hir::resolve(&checked).unwrap()
}

#[test]
fn utf8_descriptor_is_distinct_canonical_and_exactly_replayable() {
    let program = program();
    let selected = vec!["bytes.raw".to_owned(), "utf8.forward".to_owned()];
    let descriptor = derive_public_api_descriptor(
        &program,
        &selected,
        subject(PUBLIC_OWNED_UTF8_PROJECT_SCHEMA),
    )
    .unwrap();
    assert_eq!(descriptor.schema(), PUBLIC_OWNED_UTF8_API_SCHEMA);
    assert_eq!(
        descriptor
            .exports()
            .iter()
            .map(|export| export.result())
            .collect::<Vec<_>>(),
        [
            PublicApiResultType::OwnedBytes,
            PublicApiResultType::OwnedUtf8
        ]
    );
    let bytes = descriptor.canonical_bytes();
    assert!(String::from_utf8_lossy(&bytes).contains("\"result\":\"owned-utf8\""));
    assert!(replay_public_api_descriptor(
        &program,
        &selected,
        subject(PUBLIC_OWNED_UTF8_PROJECT_SCHEMA),
        &bytes,
        &descriptor.digest(),
    )
    .is_ok());
}

#[test]
fn v8_cannot_describe_or_replay_owned_utf8() {
    let program = program();
    let selected = vec!["utf8.greeting".to_owned()];
    assert!(derive_public_api_descriptor(
        &program,
        &selected,
        subject(PUBLIC_OWNED_DATA_PROJECT_SCHEMA),
    )
    .is_err());

    let v10 = derive_public_api_descriptor(
        &program,
        &selected,
        subject(PUBLIC_OWNED_UTF8_PROJECT_SCHEMA),
    )
    .unwrap();
    assert!(replay_public_api_descriptor(
        &program,
        &selected,
        subject(PUBLIC_OWNED_DATA_PROJECT_SCHEMA),
        &v10.canonical_bytes(),
        &v10.digest(),
    )
    .is_err());
}

#[test]
fn native_provider_carries_embedded_nul_by_exact_length_and_settles() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = program();
    let selected = vec!["utf8.greeting".to_owned()];
    let descriptor = derive_public_api_descriptor(
        &program,
        &selected,
        subject(PUBLIC_OWNED_UTF8_PROJECT_SCHEMA),
    )
    .unwrap();
    let provider = semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(PUBLIC_OWNED_UTF8_PROJECT_SCHEMA),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
    assert!(provider.source().contains("spx_string_length_v10"));
    assert!(provider.source().contains("spx_owned_data_utf8_v1"));
    assert!(!provider.source().contains("strlen(result)"));

    let directory =
        std::env::temp_dir().join(format!("semaprax-utf8-provider-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir(&directory).unwrap();
    let c = directory.join("provider.c");
    let executable = directory.join("provider");
    let probe = r#"
int main(void) {
    uint64_t size = spx_owned_data_context_size_v1();
    void *storage = malloc((size_t)size);
    if (storage == NULL || spx_owned_data_context_init_v1(storage, size) != 0) return 10;
    spx_context_v1 *context = (spx_context_v1 *)storage;
    uint32_t tag = UINT32_MAX; uint64_t handle = 0; int64_t error = -1;
    if (spx_owned_data_call_spx_utf8_dot_greeting_v1(context, &tag, &handle, &error) != 0 || tag != 0 || handle == 0) return 11;
    uint64_t length = 0;
    if (spx_owned_bytes_len_v1(context, handle, &length) != 0 || length != UINT64_C(12)) return 12;
    uint8_t output[12] = {0};
    if (spx_owned_bytes_copy_v1(context, handle, output, length) != 0) return 13;
    const uint8_t expected[12] = {'h','e','l','l','o',0,0xe4,0xb8,0x96,0xe7,0x95,0x8c};
    if (memcmp(output, expected, 12) != 0) return 14;
    if (spx_owned_bytes_drop_v1(context, handle) != 0) return 15;
    if (spx_owned_data_context_drop_v1(context) != 0) return 16;
    free(storage); return 0;
}
"#;
    fs::write(&c, format!("{}\n{probe}", provider.source())).unwrap();
    let compiled = Command::new("clang")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&c)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let status = Command::new(&executable).status().unwrap();
    let _ = fs::remove_dir_all(&directory);
    assert!(status.success());
}
