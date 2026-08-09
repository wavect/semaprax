#![cfg(any(target_os = "linux", target_os = "macos"))]

use semaprax_native_loader::{
    open_admitted_callable_exact, open_admitted_exact, CallWireError, OpenError,
    MAX_CALLABLE_SYMBOL_BYTES, MAX_CALL_WIRE_BYTES, MAX_DESCRIPTOR_BYTES, MAX_GETTER_SYMBOL_BYTES,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const EXPECTED: &[u8] = b"SPX-native-descriptor-v1";
const CALLABLE_EXPECTED: &[u8] = &[
    b'S', b'P', b'X', b'N', b'A', b'B', b'I', b'2', 2, 0, 0, 0, 20, 0, 0, 0, 20, 0, 0, 0,
];
const CALLABLE_V3_METADATA: &[u8] = &[
    b'S', b'P', b'X', b'N', b'A', b'B', b'I', b'3', 3, 0, 0, 0, 20, 0, 0, 0, 20, 0, 0, 0,
];
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct Fixture {
    directory: PathBuf,
    library: PathBuf,
    unload_marker: PathBuf,
}

impl Fixture {
    fn build() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-native-loader-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&directory).expect("create isolated fixture directory");
        let directory = fs::canonicalize(directory).expect("canonical fixture directory");
        let source = directory.join("provider.c");
        let unload_marker = directory.join("unloaded.marker");
        let library = directory.join(library_filename());

        let marker_literal = c_string_literal(&unload_marker);
        let provider = format!(
            r#"#include <stdint.h>
#include <stdio.h>
#include <string.h>

static const uint8_t good_descriptor[] = {{
  0x53, 0x50, 0x58, 0x2d, 0x6e, 0x61, 0x74, 0x69,
  0x76, 0x65, 0x2d, 0x64, 0x65, 0x73, 0x63, 0x72,
  0x69, 0x70, 0x74, 0x6f, 0x72, 0x2d, 0x76, 0x31
}};
static const uint8_t wrong_descriptor[] = {{ 0x00 }};
static const uint8_t callable_descriptor[] = {{
  0x53, 0x50, 0x58, 0x4e, 0x41, 0x42, 0x49, 0x32,
  0x02, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00,
  0x14, 0x00, 0x00, 0x00
}};

const uint8_t *spx_descriptor_good(void) {{ return good_descriptor; }}
const uint8_t *spx_descriptor_null(void) {{ return NULL; }}
const uint8_t *spx_descriptor_wrong(void) {{ return wrong_descriptor; }}
const uint8_t *spx_descriptor_callable(void) {{ return callable_descriptor; }}

uint32_t spx_callable_echo(
    const uint8_t *request,
    uint32_t request_len,
    uint8_t *response,
    uint32_t response_capacity
) {{
  if (request == NULL || response == NULL) return UINT32_C(1);
  if (request_len > response_capacity) return UINT32_C(2);
  memcpy(response, request, request_len);
  return UINT32_C(0);
}}

__attribute__((destructor)) static void spx_on_unload(void) {{
  FILE *marker = fopen({marker_literal}, "wb");
  if (marker != NULL) {{
    fputs("unloaded", marker);
    fclose(marker);
  }}
}}
"#
        );
        fs::write(&source, provider).expect("write fixture provider");

        let mut compiler = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()));
        #[cfg(target_os = "macos")]
        compiler.args(["-dynamiclib", "-fPIC"]);
        #[cfg(not(target_os = "macos"))]
        compiler.args(["-shared", "-fPIC"]);
        let output = compiler
            .arg(&source)
            .arg("-o")
            .arg(&library)
            .output()
            .expect("run C compiler for runtime-loaded fixture");
        assert!(
            output.status.success(),
            "fixture compiler failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let library = fs::canonicalize(library).expect("canonical fixture library");

        Self {
            directory,
            library,
            unload_marker,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.directory).expect("remove isolated fixture directory");
    }
}

fn library_filename() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "libprovider.dylib"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "libprovider.so"
    }
}

fn c_string_literal(path: &Path) -> String {
    let text = path
        .to_str()
        .expect("fixture path must be representable as UTF-8");
    let mut escaped = String::with_capacity(text.len() + 2);
    escaped.push('"');
    for character in text.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            character => escaped.push(character),
        }
    }
    escaped.push('"');
    escaped
}

unsafe fn open(
    fixture: &Fixture,
    symbol: &[u8],
    expected: &[u8],
) -> Result<semaprax_native_loader::NativeModuleLease, OpenError> {
    // SAFETY: The test has just compiled this exact canonical fixture. Each
    // selected symbol has the declared ABI, returns static bytes (or null for
    // its dedicated negative test), and cannot unwind.
    unsafe { open_admitted_exact(&fixture.library, symbol, expected) }
}

fn require_error(
    result: Result<semaprax_native_loader::NativeModuleLease, OpenError>,
    context: &str,
) -> OpenError {
    match result {
        Ok(_) => panic!("{context}"),
        Err(error) => error,
    }
}

fn require_callable_error(
    result: Result<semaprax_native_loader::NativeCallableModuleLease, OpenError>,
) -> OpenError {
    match result {
        Ok(_) => panic!("callable admission unexpectedly succeeded"),
        Err(error) => error,
    }
}

#[test]
fn runtime_load_validates_exact_bytes_and_metadata() {
    let fixture = Fixture::build();
    // SAFETY: `open` documents this generated fixture's complete admission.
    let lease = unsafe { open(&fixture, b"spx_descriptor_good", EXPECTED) }.expect("load fixture");

    assert_eq!(lease.canonical_path(), fixture.library);
    assert_eq!(lease.descriptor_len(), EXPECTED.len());
    assert!(!fixture.unload_marker.exists());
}

#[test]
fn callable_admission_invokes_one_exact_private_symbol_with_preallocated_wires() {
    let fixture = Fixture::build();
    // SAFETY: This fixture defines the exact getter and synchronous bounded
    // byte-wire callable and has no foreign unwind path.
    let lease = unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_echo",
            CALLABLE_EXPECTED,
        )
    }
    .expect("admit callable fixture");
    let retained = lease.retain();
    assert!(lease.is_same_instance(&retained));
    assert_eq!(lease.canonical_path(), fixture.library);
    assert_eq!(lease.descriptor_len(), CALLABLE_EXPECTED.len());

    let request = b"SPXNREQ1-canonical-test".to_vec();
    let mut call = lease
        .prepare_call(request.clone(), request.len())
        .expect("prepare bounded call");
    assert_eq!(lease.invoke(&mut call), Ok(0));
    assert_eq!(call.response_storage(), request);
    assert_eq!(lease.invoke(&mut call), Err(CallWireError::AlreadyInvoked));
}

#[test]
fn prepared_call_is_bound_to_the_exact_loaded_instance() {
    let fixture = Fixture::build();
    // SAFETY: Both opens target the exact generated fixture and ABI. They are
    // deliberately separate platform opens with distinct logical identities.
    let first = unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_echo",
            CALLABLE_EXPECTED,
        )
    }
    .unwrap();
    let second = unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_echo",
            CALLABLE_EXPECTED,
        )
    }
    .unwrap();
    assert_ne!(first.instance_id(), second.instance_id());
    let request = b"instance-bound".to_vec();
    let mut call = first.prepare_call(request.clone(), request.len()).unwrap();
    assert_eq!(
        second.invoke(&mut call),
        Err(CallWireError::WrongModuleInstance)
    );
    assert_eq!(first.invoke(&mut call), Ok(0));
    assert_eq!(call.response_storage(), request);
}

#[test]
fn callable_schema_and_symbols_fail_closed_before_or_during_admission() {
    let fixture = Fixture::build();
    // SAFETY: Input validation rejects descriptor v1 before opening the file.
    let error = require_callable_error(unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_good",
            b"spx_callable_echo",
            EXPECTED,
        )
    });
    assert!(matches!(error, OpenError::InvalidCallableDescriptorSchema));
    assert!(!fixture.unload_marker.exists());

    let mut malformed_envelopes = vec![
        CALLABLE_EXPECTED[..19].to_vec(),
        [CALLABLE_EXPECTED, &[0]].concat(),
    ];
    for offset in [8_usize, 12, 16] {
        let mut descriptor = CALLABLE_EXPECTED.to_vec();
        descriptor[offset] ^= 1;
        malformed_envelopes.push(descriptor);
    }
    for descriptor in malformed_envelopes {
        // SAFETY: Every malformed v2 envelope is rejected before library load.
        let error = require_callable_error(unsafe {
            open_admitted_callable_exact(
                &fixture.library,
                b"spx_descriptor_callable",
                b"spx_callable_echo",
                &descriptor,
            )
        });
        assert!(matches!(error, OpenError::InvalidCallableDescriptorSchema));
        assert!(!fixture.unload_marker.exists());
    }

    for symbol in [
        Vec::new(),
        b"spx_descriptor_callable".to_vec(),
        b"bad\0symbol".to_vec(),
        vec![b'x'; MAX_CALLABLE_SYMBOL_BYTES + 1],
    ] {
        // SAFETY: Every malformed/same-as-getter symbol is rejected before load.
        let error = require_callable_error(unsafe {
            open_admitted_callable_exact(
                &fixture.library,
                b"spx_descriptor_callable",
                &symbol,
                CALLABLE_EXPECTED,
            )
        });
        assert!(matches!(error, OpenError::InvalidCallableSymbol));
        assert!(!fixture.unload_marker.exists());
    }
}

#[test]
fn callable_v3_metadata_rejection_does_not_open_a_native_image() {
    let descriptor_fixture = Fixture::build();
    // SAFETY: Shared input validation rejects v3 before the trusted fixture is
    // opened. If that ordering regresses, the exact v1 getter still provides a
    // readable range longer than the 20-byte expected metadata.
    let descriptor_error = require_error(
        unsafe {
            open_admitted_exact(
                &descriptor_fixture.library,
                b"spx_descriptor_good",
                CALLABLE_V3_METADATA,
            )
        },
        "callable v3 metadata unexpectedly reached descriptor-only loading",
    );
    assert!(matches!(
        descriptor_error,
        OpenError::CallableV3DescriptorNotLoadable
    ));
    assert!(
        !descriptor_fixture.unload_marker.exists(),
        "descriptor-only v3 rejection must happen before image load"
    );

    let callable_fixture = Fixture::build();
    // SAFETY: Shared input validation rejects v3 before image or symbol access.
    // The trusted fixture exposes exact 20-byte descriptor storage if that
    // ordering regresses, so even the failure path remains within its contract.
    let callable_error = require_callable_error(unsafe {
        open_admitted_callable_exact(
            &callable_fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_echo",
            CALLABLE_V3_METADATA,
        )
    });
    assert!(matches!(
        callable_error,
        OpenError::CallableV3DescriptorNotLoadable
    ));
    assert!(
        !callable_fixture.unload_marker.exists(),
        "callable v3 rejection must happen before image load or symbol lookup"
    );
}

#[test]
fn missing_callable_is_rejected_after_exact_descriptor_comparison() {
    let fixture = Fixture::build();
    // SAFETY: The descriptor getter is exact; lookup of the deliberately absent
    // callable must fail before an instance identity/lease is returned.
    let error = require_callable_error(unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_missing",
            CALLABLE_EXPECTED,
        )
    });
    assert!(matches!(error, OpenError::CallableLookup(_)));
    assert!(
        fixture.unload_marker.exists(),
        "failed callable lookup must release the temporary platform library"
    );
}

#[test]
fn callable_retain_controls_exact_final_unload() {
    let fixture = Fixture::build();
    // SAFETY: The generated fixture has the exact synchronous byte-wire ABI.
    let lease = unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_echo",
            CALLABLE_EXPECTED,
        )
    }
    .unwrap();
    let retained = lease.retain();
    drop(lease);
    assert!(!fixture.unload_marker.exists());
    drop(retained);
    assert!(fixture.unload_marker.exists());
}

#[test]
fn callable_nonzero_return_is_one_shot_and_preserves_zeroed_response() {
    let fixture = Fixture::build();
    // SAFETY: The generated fixture has the exact synchronous byte-wire ABI.
    let lease = unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_echo",
            CALLABLE_EXPECTED,
        )
    }
    .unwrap();
    let mut call = lease.prepare_call(vec![0x5a; 2], 1).unwrap();
    assert_eq!(lease.invoke(&mut call), Ok(2));
    assert_eq!(call.response_storage(), [0]);
    assert_eq!(lease.invoke(&mut call), Err(CallWireError::AlreadyInvoked));
}

#[test]
fn callable_accepts_one_byte_and_exact_maximum_wires() {
    let fixture = Fixture::build();
    // SAFETY: The generated fixture has the exact synchronous byte-wire ABI.
    let lease = unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_echo",
            CALLABLE_EXPECTED,
        )
    }
    .unwrap();
    for request in [vec![0xa5], vec![0x3c; MAX_CALL_WIRE_BYTES]] {
        let mut call = lease.prepare_call(request.clone(), request.len()).unwrap();
        assert_eq!(lease.invoke(&mut call), Ok(0));
        assert_eq!(call.response_storage(), request);
    }
}

#[test]
fn callable_wire_bounds_fail_before_any_invocation() {
    let fixture = Fixture::build();
    // SAFETY: This fixture defines the exact bounded callable ABI.
    let lease = unsafe {
        open_admitted_callable_exact(
            &fixture.library,
            b"spx_descriptor_callable",
            b"spx_callable_echo",
            CALLABLE_EXPECTED,
        )
    }
    .unwrap();
    assert!(matches!(
        lease.prepare_call(Vec::new(), 1),
        Err(CallWireError::InvalidRequestLength { actual: 0, .. })
    ));
    assert!(matches!(
        lease.prepare_call(vec![0; MAX_CALL_WIRE_BYTES + 1], 1),
        Err(CallWireError::InvalidRequestLength { .. })
    ));
    assert!(matches!(
        lease.prepare_call(vec![1], 0),
        Err(CallWireError::InvalidResponseCapacity { actual: 0, .. })
    ));
    assert!(matches!(
        lease.prepare_call(vec![1], MAX_CALL_WIRE_BYTES + 1),
        Err(CallWireError::InvalidResponseCapacity { .. })
    ));
}

#[cfg(target_os = "linux")]
#[test]
fn unix_admission_resolves_dependency_relocations_eagerly() {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-native-loader-eager-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir(&directory).expect("create eager-resolution fixture directory");
    let directory = fs::canonicalize(directory).expect("canonical eager fixture directory");
    let source = directory.join("provider.c");
    let library = directory.join("libprovider-eager.so");
    fs::write(
        &source,
        r#"#include <stdint.h>
static const uint8_t descriptor[] = {
  0x53, 0x50, 0x58, 0x2d, 0x6e, 0x61, 0x74, 0x69,
  0x76, 0x65, 0x2d, 0x64, 0x65, 0x73, 0x63, 0x72,
  0x69, 0x70, 0x74, 0x6f, 0x72, 0x2d, 0x76, 0x31
};
extern int spx_missing_dependency(void);
const uint8_t *spx_descriptor_good(void) { return descriptor; }
int spx_deferred_reference(void) { return spx_missing_dependency(); }
"#,
    )
    .expect("write eager-resolution provider");
    let output = Command::new(std::env::var_os("CC").unwrap_or_else(|| "cc".into()))
        .args(["-shared", "-fPIC", "-Wl,--allow-shlib-undefined"])
        .arg(&source)
        .arg("-o")
        .arg(&library)
        .output()
        .expect("compile eager-resolution fixture");
    assert!(
        output.status.success(),
        "eager fixture compiler failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let library = fs::canonicalize(library).expect("canonical eager fixture library");

    // SAFETY: The exact fixture is trusted; admission must fail before the
    // descriptor getter can establish a lease because one relocation is not
    // resolvable under RTLD_NOW.
    let error = require_error(
        unsafe { open_admitted_exact(&library, b"spx_descriptor_good", EXPECTED) },
        "lazy dependency resolution incorrectly admitted the fixture",
    );
    assert!(matches!(error, OpenError::LibraryOpen(_)));
    fs::remove_dir_all(directory).expect("remove eager-resolution fixture directory");
}

#[test]
fn explicit_retain_preserves_identity_until_the_last_drop() {
    let fixture = Fixture::build();
    // SAFETY: `open` documents this generated fixture's complete admission.
    let lease = unsafe { open(&fixture, b"spx_descriptor_good", EXPECTED) }.expect("load fixture");
    let retained = lease.retain();

    assert!(lease.is_same_instance(&retained));
    assert_eq!(lease.instance_id(), retained.instance_id());
    drop(lease);
    assert!(
        !fixture.unload_marker.exists(),
        "dropping a non-final lease must not unload the module"
    );

    drop(retained);
    assert!(
        fixture.unload_marker.exists(),
        "dropping the final lease must release the platform library handle"
    );
}

#[test]
fn separate_opens_have_distinct_instance_identity() {
    let fixture = Fixture::build();
    // SAFETY: `open` documents this generated fixture's complete admission.
    let first = unsafe { open(&fixture, b"spx_descriptor_good", EXPECTED) }.expect("first open");
    // SAFETY: `open` documents this generated fixture's complete admission.
    let second = unsafe { open(&fixture, b"spx_descriptor_good", EXPECTED) }.expect("second open");

    assert!(!first.is_same_instance(&second));
    assert_ne!(first.instance_id(), second.instance_id());
}

#[test]
fn missing_path_fails_before_loading() {
    let fixture = Fixture::build();
    let missing = fixture.directory.join("missing-library");
    // SAFETY: No code can execute because the path does not exist.
    let error = require_error(
        unsafe { open_admitted_exact(&missing, b"spx_descriptor_good", EXPECTED) },
        "missing path must fail",
    );
    assert!(matches!(error, OpenError::PathCanonicalization(_)));
}

#[test]
fn existing_absolute_noncanonical_path_is_rejected_before_loading() {
    let fixture = Fixture::build();
    let symlink = fixture.directory.join("provider-link");
    std::os::unix::fs::symlink(&fixture.library, &symlink).expect("create fixture symlink");

    // SAFETY: Canonical-form validation rejects this absolute symlink before
    // the platform loader can execute code from the trusted fixture.
    let error = require_error(
        unsafe { open_admitted_exact(&symlink, b"spx_descriptor_good", EXPECTED) },
        "noncanonical existing path must fail",
    );

    assert!(matches!(error, OpenError::PathNotCanonical));
    assert!(
        !fixture.unload_marker.exists(),
        "path rejection must happen before the module is loaded"
    );
}

#[test]
fn missing_symbol_is_rejected() {
    let fixture = Fixture::build();
    // SAFETY: The generated fixture is trusted; lookup fails before invocation.
    let error = require_error(
        unsafe { open(&fixture, b"spx_descriptor_missing", EXPECTED) },
        "missing symbol must fail",
    );
    assert!(matches!(error, OpenError::GetterLookup(_)));
}

#[test]
fn null_descriptor_is_rejected() {
    let fixture = Fixture::build();
    // SAFETY: The generated null fixture has the exact declared ABI and is the
    // dedicated null-check case permitted by the admission contract.
    let error = require_error(
        unsafe { open(&fixture, b"spx_descriptor_null", EXPECTED) },
        "null descriptor must fail",
    );
    assert!(matches!(error, OpenError::NullDescriptor));
}

#[test]
fn wrong_descriptor_bytes_are_rejected() {
    let fixture = Fixture::build();
    // Use one byte so the generated wrong fixture provides the promised readable
    // range without relying on memory beyond its static allocation.
    // SAFETY: The generated fixture has the exact declared ABI and one readable byte.
    let error = require_error(
        unsafe { open(&fixture, b"spx_descriptor_wrong", b"S") },
        "wrong descriptor must fail",
    );
    assert!(matches!(error, OpenError::DescriptorMismatch));
}

#[test]
fn input_bounds_and_exact_symbol_rules_precede_loading() {
    let relative = Path::new("relative-library");
    // SAFETY: Validation rejects the relative path before native loading.
    let error = require_error(
        unsafe { open_admitted_exact(relative, b"getter", EXPECTED) },
        "relative path must fail",
    );
    assert!(matches!(error, OpenError::PathNotAbsolute));

    let fixture = Fixture::build();
    for symbol in [b"".as_slice(), b"bad\0symbol".as_slice()] {
        // SAFETY: Symbol validation rejects these inputs before native loading.
        let error = require_error(
            unsafe { open_admitted_exact(&fixture.library, symbol, EXPECTED) },
            "invalid symbol must fail",
        );
        assert!(matches!(error, OpenError::InvalidGetterSymbol));
    }

    let oversized_symbol = vec![b'x'; MAX_GETTER_SYMBOL_BYTES + 1];
    // SAFETY: Symbol validation rejects this input before native loading.
    let error = require_error(
        unsafe { open_admitted_exact(&fixture.library, &oversized_symbol, EXPECTED) },
        "oversized symbol must fail",
    );
    assert!(matches!(error, OpenError::InvalidGetterSymbol));

    for descriptor in [&[][..], vec![0_u8; MAX_DESCRIPTOR_BYTES + 1].as_slice()] {
        // SAFETY: Descriptor validation rejects these inputs before native loading.
        let error = require_error(
            unsafe { open_admitted_exact(&fixture.library, b"spx_descriptor_good", descriptor) },
            "invalid descriptor length must fail",
        );
        assert!(matches!(
            error,
            OpenError::InvalidExpectedDescriptorLength { .. }
        ));
    }
}
