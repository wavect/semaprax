#![cfg(any(target_os = "linux", target_os = "macos"))]

use semaprax_native_loader::{
    open_admitted_exact, OpenError, MAX_DESCRIPTOR_BYTES, MAX_GETTER_SYMBOL_BYTES,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const EXPECTED: &[u8] = b"SPX-native-descriptor-v1";
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

static const uint8_t good_descriptor[] = {{
  0x53, 0x50, 0x58, 0x2d, 0x6e, 0x61, 0x74, 0x69,
  0x76, 0x65, 0x2d, 0x64, 0x65, 0x73, 0x63, 0x72,
  0x69, 0x70, 0x74, 0x6f, 0x72, 0x2d, 0x76, 0x31
}};
static const uint8_t wrong_descriptor[] = {{ 0x00 }};

const uint8_t *spx_descriptor_good(void) {{ return good_descriptor; }}
const uint8_t *spx_descriptor_null(void) {{ return NULL; }}
const uint8_t *spx_descriptor_wrong(void) {{ return wrong_descriptor; }}

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
