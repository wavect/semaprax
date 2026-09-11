//! Real, C-hosted evidence for the native C11 physical adapter (issue #154):
//! compiles the rendered reference provider together with `probe.c` as one
//! pure-C translation unit and runs it, exactly like
//! `tests/native/bytes_call_staging.rs`'s existing pattern for other native
//! providers. This validates the reference provider template itself; the
//! generated production C client is #158's separate acceptance surface.
//!
//! Known limitation, stated once here rather than hidden in prose: the
//! trusted descriptor bytes below are a FIXTURE placeholder, not real bytes
//! produced by `descriptor::verify` against a checked, admitted public
//! generic export. No such export can be built yet: #119 (owned-record
//! ownership evidence) still blocks the resolver/HIR path a real public
//! generic function would need. The provider's descriptor/binding replay
//! behavior — the property under test here — is exactly the same whichever
//! trusted bytes it is generated from; only the *source* of those bytes
//! (compiler-verified vs. hand-constructed) differs, and only that half is
//! deferred to #119's unblock.
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::public_generic_abi::carrier::{CarrierBindingV1, TargetProfile};
use semaprax::public_generic_abi::native::binding::NativeProviderBindingV1;
use semaprax::public_generic_abi::native::template::render_reference_provider;

static NEXT: AtomicU64 = AtomicU64::new(0);

const FIXTURE_DESCRIPTOR_BYTES: &[u8] = b"fixture-public-generic-descriptor-bytes-issue-154";

fn fixture_binding() -> NativeProviderBindingV1 {
    NativeProviderBindingV1::new(
        CarrierBindingV1::new(
            "sha256:1111111111111111111111111111111111111111111111111111111111111111",
            TargetProfile::NativeC11,
            "runtime:native-c11-fixture-issue-154",
        ),
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        "spx_pg_endpoint_reverse_bytes_v1",
        "semaprax-0.4.1",
    )
}

fn run(sanitized: bool) {
    let compiler = if sanitized {
        let path = PathBuf::from(
            std::env::var_os("SEMAPRAX_STRING_SANITIZER_CLANG")
                .expect("selected gate requires provisioned SEMAPRAX_STRING_SANITIZER_CLANG"),
        );
        assert!(path.is_absolute() && path.is_file());
        path
    } else {
        std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from)
    };

    let provider_source = render_reference_provider(FIXTURE_DESCRIPTOR_BYTES, &fixture_binding());

    let root = std::env::temp_dir().join(format!(
        "semaprax-public-generic-native-adapter-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    eprintln!(
        "retained native public generic adapter evidence: {}",
        root.display()
    );

    let source = root.join("probe.c");
    fs::write(
        &source,
        format!(
            "{}\n{}\n{}\n{}",
            include_str!("../support/native_fixture_stdio.c"),
            include_str!("allocations.c"),
            provider_source,
            include_str!("probe.c"),
        ),
    )
    .unwrap();

    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let mut compile = Command::new(&compiler);
        compile
            .current_dir(&root)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"]);
        if sanitized {
            compile.args([
                "-fsanitize=address,undefined",
                "-fno-sanitize-recover=all",
                "-fno-omit-frame-pointer",
            ]);
        }
        let built = compile
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}: {}",
            root.display(),
            String::from_utf8_lossy(&built.stderr)
        );
        let mut execute = Command::new(&executable);
        execute.current_dir(&root);
        if sanitized {
            execute
                .env("ASAN_OPTIONS", "halt_on_error=1")
                .env("UBSAN_OPTIONS", "halt_on_error=1:print_stacktrace=1");
        }
        let result = execute.output().unwrap();
        assert!(
            result.status.success(),
            "{}: stdout={} stderr={}",
            root.display(),
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"native-public-generic-adapter-settled\n");
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn native_public_generic_adapter_settles_at_o0_and_o2() {
    run(false);
}

#[test]
#[ignore = "requires explicitly provisioned Clang ASan/UBSan runtime and external process bounds"]
fn provisioned_native_public_generic_adapter_asan_ubsan() {
    run(true);
}

#[test]
fn header_compiles_standalone_as_c11() {
    let compiler = std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from);
    let root = std::env::temp_dir().join(format!(
        "semaprax-public-generic-native-adapter-header-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let source = root.join("header_only.c");
    fs::write(
        &source,
        "#include \"spx_pg_v1.h\"\nint main(void) { return SPX_PG_STATUS_OK; }\n",
    )
    .unwrap();
    fs::write(
        root.join("spx_pg_v1.h"),
        semaprax::public_generic_abi::native::template::HEADER_V1,
    )
    .unwrap();
    let object = root.join("header_only.o");
    let built = Command::new(&compiler)
        .current_dir(&root)
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-c"])
        .arg(&source)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        built.status.success(),
        "{}: {}",
        root.display(),
        String::from_utf8_lossy(&built.stderr)
    );
}
