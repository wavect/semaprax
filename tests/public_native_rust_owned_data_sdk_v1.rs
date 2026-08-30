use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    derive_public_api_descriptor, PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};

const SOURCE: &str = include_str!("../examples/owned-data-rust/owned_data.spx");
const REVISION: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const SELECTED: [&str; 3] = [
    "frame.payload",
    "frame.payload-maybe",
    "frame.payload-result",
];
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[path = "public_native_rust_owned_data_sdk_v1/handle_identity.rs"]
mod handle_identity;

fn configured_tool(variable: &str, candidates: &[&str]) -> PathBuf {
    if let Some(configured) = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
    {
        #[cfg(windows)]
        if variable == "SEMAPRAX_ARCHIVER" {
            return configured;
        }
        if let Ok(canonical) = configured.canonicalize() {
            return canonical;
        }
    }
    candidates
        .iter()
        .map(PathBuf::from)
        .filter_map(|path| path.canonicalize().ok())
        .find(|path| path.is_absolute() && path.is_file())
        .unwrap_or_else(|| panic!("{variable} must name an installed absolute tool"))
}

struct Fixture(PathBuf);

impl Fixture {
    fn new(label: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "semaprax-owned-data-sdk-{label}-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn program(source: &str) -> semaprax::hir::ResolvedProgram {
    semaprax::hir::resolve(&semaprax::check(source, Path::new("owned_data.spx")).unwrap()).unwrap()
}

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: REVISION,
        workspace_revision: REVISION,
        project_graph_digest: REVISION,
    }
}

fn artifact(source: &str) -> semaprax::codegen::NativeOwnedDataProviderArtifact {
    let program = program(source);
    let selected = SELECTED.map(str::to_owned);
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap()
}

fn run(command: &mut Command, label: &str) -> Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn provider_uses_compiler_layouts_and_rejects_hostile_handles_at_o0_and_o2() {
    assert!(
        Command::new("clang")
            .arg("--version")
            .output()
            .unwrap()
            .status
            .success(),
        "WP12 requires clang"
    );
    let artifact = artifact(SOURCE);
    assert!(artifact
        .source()
        .contains("typedef uint64_t spx_owned_bytes_handle_v1;"));
    assert!(artifact.source().contains("spx_owned_bytes_len_v1"));
    assert!(artifact.source().contains("spx_owned_bytes_copy_v1"));
    assert!(artifact.source().contains("spx_owned_bytes_drop_v1"));

    let fixture = Fixture::new("provider");
    let probe = r#"
int main(void) {
    uint64_t size = spx_owned_data_context_size_v1();
    void *first_storage = malloc((size_t)size);
    void *second_storage = malloc((size_t)size);
    if (first_storage == NULL || second_storage == NULL) return 10;
    if (spx_owned_data_context_init_v1(first_storage, size) != 0 || spx_owned_data_context_init_v1(second_storage, size) != 0) return 11;
    spx_context_v1 *first = (spx_context_v1 *)first_storage;
    spx_context_v1 *second = (spx_context_v1 *)second_storage;
    uint8_t input[3] = { UINT8_C(0), UINT8_C(42), UINT8_C(255) };
    uint32_t tag = UINT32_MAX; uint64_t handle = UINT64_C(0); int64_t error = INT64_C(0);
    if (spx_owned_data_call_spx_frame_dot_payload_v1(first, input, UINT64_C(3), &tag, &handle, &error) != 0 || tag != 0 || handle == 0) return 12;
    uint64_t length = UINT64_MAX;
    if (spx_owned_bytes_len_v1(first, handle, &length) != 0 || length != 3) return 13;
    if (spx_owned_bytes_len_v1(second, handle, &length) != SPX_OWNED_DATA_INVALID_HANDLE) return 14;
    uint8_t output[3] = {0};
    if (spx_owned_bytes_copy_v1(first, handle, output, UINT64_C(2)) != SPX_OWNED_DATA_COPY_FAILURE) return 15;
    if (spx_owned_bytes_copy_v1(first, handle, output, UINT64_C(3)) != 0 || memcmp(input, output, 3) != 0) return 16;
    if (spx_owned_bytes_drop_v1(first, handle) != 0) return 17;
    if (spx_owned_bytes_len_v1(first, handle, &length) != SPX_OWNED_DATA_INVALID_HANDLE) return 18;
    if (spx_owned_bytes_drop_v1(first, handle) != SPX_OWNED_DATA_INVALID_HANDLE) return 19;

    tag = UINT32_MAX; handle = 0;
    if (spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1(first, NULL, 0, &tag, &handle, &error) != 0 || tag != 0 || handle != 0) return 20;
    if (spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1(first, input, 3, &tag, &handle, &error) != 0 || tag != 1 || handle == 0) return 21;
    spx_owned_data_test_fault_v1(first, 1);
    if (spx_owned_bytes_copy_v1(first, handle, output, 3) != SPX_OWNED_DATA_COPY_FAILURE) return 22;
    if (spx_owned_bytes_drop_v1(first, handle) != 0) return 23;

    tag = UINT32_MAX; handle = 0; error = 0;
    if (spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1(first, input, 1, &tag, &handle, &error) != 0 || tag != 1 || handle != 0 || error != -7) return 24;
    if (spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1(first, input, 3, &tag, &handle, &error) != 0 || tag != 0 || handle == 0) return 25;
    spx_owned_data_test_fault_v1(first, 2);
    if (spx_owned_bytes_drop_v1(first, handle) != SPX_OWNED_DATA_SETTLEMENT_FAILURE) return 26;
    if (spx_owned_data_context_drop_v1(first) != SPX_OWNED_DATA_SETTLEMENT_FAILURE) return 27;
    if (spx_owned_bytes_drop_v1(first, handle) != 0) return 28;
    for (uint32_t iteration = 0; iteration < UINT32_C(5000); ++iteration) {
        handle = 0;
        if (spx_owned_data_call_spx_frame_dot_payload_v1(first, input, 3, &tag, &handle, &error) != 0 || handle == 0) return 30;
        if (spx_owned_bytes_drop_v1(first, handle) != 0) return 31;
    }
    tag = UINT32_C(77); handle = UINT64_C(0); error = INT64_C(88);
    if (spx_owned_data_call_spx_frame_dot_payload_v1(first, NULL, 1, &tag, &handle, &error) != SPX_OWNED_DATA_ADAPTER_FAILURE) return 32;
    if (tag != UINT32_C(77) || handle != UINT64_C(0) || error != INT64_C(88)) return 33;
    uint64_t aliased[2] = { UINT64_C(0), UINT64_C(0) };
    if (spx_owned_data_call_spx_frame_dot_payload_v1(first, input, 3, (uint32_t *)&aliased[0], &aliased[0], (int64_t *)&aliased[1]) != SPX_OWNED_DATA_ADAPTER_FAILURE) return 34;
    if (aliased[0] != UINT64_C(0) || aliased[1] != UINT64_C(0)) return 35;
    if (spx_owned_data_context_drop_v1(first) != 0 || spx_owned_data_context_drop_v1(second) != 0) return 29;
    free(first_storage); free(second_storage); return 0;
}

"#;
    for optimization in ["-O0", "-O2"] {
        let c = fixture.0.join(format!("provider-{optimization}.c"));
        let executable = fixture.0.join(format!("provider-{optimization}"));
        std::fs::write(&c, format!("{}\n{probe}", artifact.source())).unwrap();
        run(
            Command::new("clang")
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_OWNED_DATA_TESTING",
                ])
                .arg(&c)
                .arg("-o")
                .arg(&executable),
            "compile hostile provider harness",
        );
        run(
            &mut Command::new(executable),
            "run hostile provider harness",
        );
    }
    #[cfg(target_os = "linux")]
    {
        let c = fixture.0.join("provider-sanitized.c");
        let executable = fixture.0.join("provider-sanitized");
        std::fs::write(&c, format!("{}\n{probe}", artifact.source())).unwrap();
        run(
            Command::new("clang")
                .args([
                    "-std=c11",
                    "-O2",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_OWNED_DATA_TESTING",
                    "-fsanitize=address,undefined",
                    "-fno-sanitize-recover=all",
                ])
                .arg(&c)
                .arg("-o")
                .arg(&executable),
            "compile ASan/UBSan provider harness",
        );
        run(
            &mut Command::new(executable),
            "run ASan/UBSan provider harness",
        );
    }
}

#[test]
fn borrow_str_rejects_invalid_utf8_before_semantic_execution() {
    assert!(
        Command::new("clang")
            .arg("--version")
            .output()
            .unwrap()
            .status
            .success(),
        "WP12 requires clang"
    );
    let source = r#"module owned.utf8;
@id("utf8.payload") fn payload(input: borrow str, data: borrow Slice<u8>) -> Bytes { bytes_copy(data) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = program(source);
    let selected = vec!["utf8.payload".to_owned()];
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    let artifact = semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
    let fixture = Fixture::new("utf8");
    let probe = r#"int main(void){uint64_t size=spx_owned_data_context_size_v1();void *storage=malloc((size_t)size);if(storage==NULL)return 1;if(spx_owned_data_context_init_v1(storage,size)!=0)return 2;spx_context_v1 *context=(spx_context_v1*)storage;uint8_t invalid[2]={UINT8_C(0xc0),UINT8_C(0x80)};uint32_t tag=91;uint64_t handle=0;int64_t error=92;if(spx_owned_data_call_spx_utf8_dot_payload_v1(context,invalid,2,NULL,0,&tag,&handle,&error)!=SPX_OWNED_DATA_ADAPTER_FAILURE)return 3;if(tag!=91||handle!=0||error!=92)return 4;if(context->invocation!=0)return 5;if(spx_owned_data_context_drop_v1(context)!=0)return 6;free(storage);return 0;}"#;
    for optimization in ["-O0", "-O2"] {
        let c = fixture.0.join(format!("utf8-{optimization}.c"));
        let executable = fixture.0.join(format!("utf8-{optimization}"));
        std::fs::write(&c, format!("{}\n{probe}", artifact.source())).unwrap();
        run(
            Command::new("clang")
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&c)
                .arg("-o")
                .arg(&executable),
            "compile UTF-8 provider harness",
        );
        run(&mut Command::new(executable), "run UTF-8 provider harness");
    }
}

#[test]
fn descriptor_replay_is_exact_and_display_rename_preserves_the_provider_api() {
    let original = artifact(SOURCE);
    let renamed_source = SOURCE
        .replacen("fn frame_payload(", "fn renamed_payload(", 1)
        .replacen("fn optional_payload(", "fn renamed_optional(", 1)
        .replacen("fn parse_payload(", "fn renamed_parse(", 1);
    let renamed = artifact(&renamed_source);
    assert_eq!(original.descriptor(), renamed.descriptor());
    for symbol in [
        "spx_owned_data_call_spx_frame_dot_payload_v1",
        "spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1",
        "spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1",
    ] {
        assert!(original.source().contains(symbol));
        assert!(renamed.source().contains(symbol));
    }

    let program = program(SOURCE);
    let selected = SELECTED.map(str::to_owned);
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    let mut mutated = descriptor.canonical_bytes();
    mutated[0] = b'[';
    assert!(semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(),
        &mutated,
        &descriptor.digest(),
    )
    .is_err());
}

#[test]
fn published_safe_package_builds_offline_and_fail_stops_on_unsettled_handles() {
    assert!(
        Command::new("clang")
            .arg("--version")
            .output()
            .unwrap()
            .status
            .success(),
        "WP12 requires clang"
    );
    let archiver_candidates: &[&str] = if cfg!(windows) {
        &[]
    } else if cfg!(target_os = "macos") {
        &["/usr/bin/libtool"]
    } else {
        &["/usr/bin/ar", "/bin/ar"]
    };
    let archiver = configured_tool("SEMAPRAX_ARCHIVER", archiver_candidates);
    let clang_path = configured_tool("CLANG", &["/usr/bin/clang"]);
    let fixture = Fixture::new("package");
    let generated = fixture.0.join("generated-sdk");
    let setup_manifest =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/owned-data-rust/Cargo.toml");
    run(
        Command::new(env!("CARGO"))
            .args(["run", "--locked", "--offline", "--quiet", "--manifest-path"])
            .arg(&setup_manifest)
            .arg("--")
            .arg(&generated)
            .env("CLANG", &clang_path)
            .env("SEMAPRAX_ARCHIVER", &archiver)
            .env("CARGO_TARGET_DIR", fixture.0.join("setup-target")),
        "publish owned-data SDK",
    );

    let mut inventory = std::fs::read_dir(&generated)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    inventory.sort();
    let archive = if cfg!(windows) {
        "semaprax_native_rust_owned_data_sdk.lib"
    } else {
        "libsemaprax_native_rust_owned_data_sdk.a"
    };
    let mut expected = vec![
        "Cargo.toml",
        "build.rs",
        "descriptor.json",
        "lib.rs",
        "owned_data_ffi.rs",
        "semaprax.native-rust-owned-data-sdk.json",
        archive,
    ];
    expected.sort();
    assert_eq!(inventory, expected);
    let public = std::fs::read_to_string(generated.join("lib.rs")).unwrap();
    let ffi = std::fs::read_to_string(generated.join("owned_data_ffi.rs")).unwrap();
    assert!(public.contains("#![forbid(unsafe_code)]"));
    assert!(!public.contains("unsafe{"));
    assert!(ffi.contains("#![allow(unsafe_code)]"));
    assert!(ffi.contains("PhantomData<Rc<()>"));
    assert!(!ffi.contains("spx_owned_data_test_fault_v1"));
    assert!(!public.contains("Handle"));

    let consumer = fixture.0.join("consumer");
    std::fs::create_dir(&consumer).unwrap();
    std::fs::create_dir(consumer.join("src")).unwrap();
    let consumer_source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/owned-data-rust/consumer");
    std::fs::copy(
        consumer_source.join("Cargo.toml"),
        consumer.join("Cargo.toml"),
    )
    .unwrap();
    std::fs::copy(
        consumer_source.join("src/main.rs"),
        consumer.join("src/main.rs"),
    )
    .unwrap();
    run(
        Command::new(env!("CARGO"))
            .args(["generate-lockfile", "--offline"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", fixture.0.join("consumer-target")),
        "lock safe consumer",
    );
    let consumer_output = run(
        Command::new(env!("CARGO"))
            .args(["run", "--locked", "--offline", "--quiet"])
            .current_dir(&consumer)
            .env("CARGO_TARGET_DIR", fixture.0.join("consumer-target")),
        "run safe consumer",
    );
    assert_eq!(consumer_output.stdout, b"42\n");

    #[cfg(not(windows))]
    {
        let testing = fixture.0.join("testing-provider.c");
        let object = fixture.0.join("testing-provider.o");
        let archive_path = fixture.0.join("libtesting_provider.a");
        std::fs::write(&testing, artifact(SOURCE).source()).unwrap();
        run(
            Command::new("clang")
                .args([
                    "-std=c11",
                    "-O2",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_OWNED_DATA_TESTING",
                    "-c",
                ])
                .arg(&testing)
                .arg("-o")
                .arg(&object),
            "compile test-only fault provider",
        );
        if cfg!(target_os = "macos") {
            run(
                Command::new("/usr/bin/libtool")
                    .args(["-static", "-D", "-o"])
                    .arg(&archive_path)
                    .arg(&object),
                "archive test-only provider",
            );
        } else {
            run(
                Command::new(&archiver)
                    .args(["rcsD"])
                    .arg(&archive_path)
                    .arg(&object),
                "archive test-only provider",
            );
        }
        let test_ffi = ffi
            .replace("fn spx_owned_bytes_drop_v1(context:*mut RawContext,handle:Handle)->Status;", "fn spx_owned_bytes_drop_v1(context:*mut RawContext,handle:Handle)->Status;fn spx_owned_data_test_fault_v1(context:*mut RawContext,fault:u32);")
            .replace("\nstruct Guard<'a>", "\nimpl Context{pub(super) fn inject_fault(&mut self,fault:u32){unsafe{spx_owned_data_test_fault_v1(self.raw.as_ptr(),fault)}}}\nstruct Guard<'a>");
        let test_ffi_path = fixture.0.join("testing_ffi.rs");
        std::fs::write(&test_ffi_path, test_ffi).unwrap();
        let harness = fixture.0.join("settlement.rs");
        std::fs::write(&harness, format!("#[path={:?}]mod ffi;fn main(){{let mode=std::env::args().nth(1).unwrap();let mut context=match ffi::Context::new(){{Ok(v)=>v,Err(_)=>std::process::exit(10)}};let result=context.invoke(|context|{{let raw=match context.call_spx_frame_dot_payload(b\"abc\"){{Ok(v)=>v,Err(_)=>std::process::exit(11)}};context.inject_fault(if mode==\"copy\"{{1}}else{{2}});context.copy_and_settle(raw.handle)}});if mode==\"copy\"{{if !matches!(result,Ok(Err(_))){{std::process::exit(12)}}println!(\"copy-settled\")}}else{{println!(\"value-published\")}}}}", test_ffi_path.display().to_string())).unwrap();
        let executable = fixture.0.join("settlement");
        run(
            Command::new("rustc")
                .args(["--edition=2021"])
                .arg(&harness)
                .arg("-L")
                .arg(format!("native={}", fixture.0.display()))
                .args(["-l", "static=testing_provider", "-o"])
                .arg(&executable),
            "compile safe settlement subprocess",
        );
        let copy = run(
            Command::new(&executable).arg("copy"),
            "copy failure settles exactly once",
        );
        assert_eq!(copy.stdout, b"copy-settled\n");
        let failed = Command::new(&executable).arg("drop").output().unwrap();
        assert!(!failed.status.success());
        assert!(failed.stdout.is_empty(), "value published before fail-stop");
    }
}
