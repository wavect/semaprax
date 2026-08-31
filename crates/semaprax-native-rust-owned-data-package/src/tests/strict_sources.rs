//! Scalar-only selections still own and close their provider context, but must
//! not emit unused result-owner machinery. Compile metadata, never a provider.
use super::auto_traits::{compiler, success, write_new};
use super::*;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn scalar_descriptor(result: &str, mode: PackageMode) -> Vec<u8> {
    let bytes = descriptor_bytes(result);
    if mode == PackageMode::ProjectV10OwnedUtf8 {
        // Keep the canonical field order and scalar export; only change the
        // schema pair. Replay below authenticates the actual v10 digest domain.
        String::from_utf8(bytes)
            .unwrap()
            .replace(
                "semaprax.public-owned-data-api.v1",
                "semaprax.public-owned-utf8-api.v1",
            )
            .replace("semaprax.project.v8", "semaprax.project.v10")
            .into_bytes()
    } else {
        bytes
    }
}

fn context_without_result_owner(ffi: &str, library: &str) {
    for required in [
        "pub(super)struct Context",
        "pub fn invoke",
        "fn close",
        "struct Invocation",
        "impl Drop for Invocation",
        "impl Drop for Context",
        "self.known_open=false;",
        "if unsafe{spx_owned_data_context_drop_v1(self.raw.as_ptr())}!=0{std::process::abort()}",
        "PhantomData<Rc<()>>",
    ] {
        assert!(
            ffi.contains(required),
            "missing context protocol: {required}"
        );
    }
    assert!(library.contains("self.context.invoke(|context|"));
    for forbidden in [
        "type Handle=",
        "struct RawCall",
        "struct Guard",
        "pub fn copy_and_settle",
        "pub fn discard",
        "fn spx_owned_bytes_len_v1",
        "fn spx_owned_bytes_copy_v1",
        "fn spx_owned_bytes_drop_v1",
    ] {
        assert!(
            !ffi.contains(forbidden),
            "unused owner fragment: {forbidden}"
        );
    }
}

#[test]
fn scalar_only_generated_sdks_compile_without_warnings_and_keep_context_settlement() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-sdk-strict-scalars-{}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let target = HostTarget::current().unwrap();
    // Retain nine independent fixtures. No linking, recursive cleanup, or
    // dependence on a provider archive; each consumer uses its own SDK rmeta.
    for (label, mode) in [
        ("v8-standalone", PackageMode::StandaloneEvidence),
        ("v8-project", PackageMode::ProjectV8),
        ("v10-project", PackageMode::ProjectV10OwnedUtf8),
    ] {
        for result in ["i64", "bool", "usize"] {
            let bytes = scalar_descriptor(result, mode);
            let digest = descriptor_digest_for_bytes(&bytes).unwrap();
            let descriptor =
                descriptor::replay(&bytes, &digest, &["fixture.value".to_owned()]).unwrap();
            assert_eq!(descriptor.exports_len(), 1);
            let sources = render::render_sources(&descriptor, target, mode);
            context_without_result_owner(&sources.ffi_rs, &sources.lib_rs);
            let directory = root.join(format!("{label}-{result}"));
            fs::create_dir(&directory).unwrap();
            let library = directory.join("lib.rs");
            write_new(&library, &sources.lib_rs);
            write_new(&directory.join("owned_data_ffi.rs"), &sources.ffi_rs);
            let metadata = directory.join("libgenerated_sdk.rmeta");
            success(
                compiler(&library, &metadata, "generated_sdk")
                    .output()
                    .unwrap(),
                &metadata,
            );
            let consumer = directory.join("consumer.rs");
            // Public usize carriers are portable u64, not the host Rust usize.
            let rust_result = if result == "usize" { "u64" } else { result };
            write_new(&consumer, &format!(
                "#![forbid(unsafe_code)]\nuse generated_sdk::{{NativeRustOwnedDataSdk,CallError}};\npub fn call()->Result<{rust_result},CallError>{{NativeRustOwnedDataSdk::new()?.spx_fixture_dot_value()}}\n"
            ));
            let consumer_metadata = directory.join("consumer.rmeta");
            success(
                compiler(&consumer, &consumer_metadata, "consumer")
                    .arg("--extern")
                    .arg(format!("generated_sdk={}", metadata.display()))
                    .output()
                    .unwrap(),
                &consumer_metadata,
            );
        }
    }
}
