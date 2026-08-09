use semaprax_native_loader::{open_admitted_callable_exact, open_admitted_exact, OpenError};

const GETTER_SYMBOL: &[u8] = b"semaprax_descriptor_v3";
const CALLABLE_SYMBOL: &[u8] = b"semaprax_callable_v3";

fn descriptor_v3_header() -> Vec<u8> {
    let mut descriptor = Vec::new();
    descriptor.extend_from_slice(b"SPXNABI3");
    descriptor.extend_from_slice(&3_u32.to_le_bytes());
    descriptor.extend_from_slice(&20_u32.to_le_bytes());
    descriptor.extend_from_slice(&20_u32.to_le_bytes());
    descriptor
}

fn assert_rejected_by_both_loaders(descriptor: &[u8]) {
    let absent_library = std::env::current_dir()
        .expect("current directory")
        .join("semaprax-callable-v3-must-not-be-opened");
    assert!(absent_library.is_absolute());

    // SAFETY: Input validation rejects the v3 magic before path
    // canonicalization, loading, symbol lookup, or invocation, so no foreign
    // code is reached.
    let descriptor_only =
        unsafe { open_admitted_exact(&absent_library, GETTER_SYMBOL, descriptor) };
    // SAFETY: The same shared input validation rejects v3 before the callable
    // loader can classify it as v2 or reach any foreign code.
    let callable = unsafe {
        open_admitted_callable_exact(&absent_library, GETTER_SYMBOL, CALLABLE_SYMBOL, descriptor)
    };

    assert!(matches!(
        descriptor_only,
        Err(OpenError::CallableV3DescriptorNotLoadable)
    ));
    assert!(matches!(
        callable,
        Err(OpenError::CallableV3DescriptorNotLoadable)
    ));
}

#[test]
fn callable_v3_descriptor_is_rejected_before_any_path_or_library_access() {
    assert_rejected_by_both_loaders(&descriptor_v3_header());
}

#[test]
fn malformed_v3_headers_cannot_fall_through_to_callable_v2_admission() {
    let canonical = descriptor_v3_header();
    let mut malformed = Vec::new();

    let mut version_two = canonical.clone();
    version_two[8..12].copy_from_slice(&2_u32.to_le_bytes());
    malformed.push(version_two);

    let mut wrong_header_size = canonical.clone();
    wrong_header_size[12..16].copy_from_slice(&21_u32.to_le_bytes());
    malformed.push(wrong_header_size);

    let mut wrong_total_size = canonical.clone();
    wrong_total_size[16..20].copy_from_slice(&19_u32.to_le_bytes());
    malformed.push(wrong_total_size);

    let mut trailing_byte = canonical;
    trailing_byte.push(0);
    malformed.push(trailing_byte);

    for descriptor in malformed {
        assert_rejected_by_both_loaders(&descriptor);
    }
}
