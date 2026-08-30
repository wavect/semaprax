use super::*;
use crate::platform::NativeHostTarget;

#[test]
fn private_target_mapping_preserves_all_six_targets_and_fixed_fields() {
    for (host, triple) in [
        (NativeHostTarget::X86_64LinuxGnu, "x86_64-unknown-linux-gnu"),
        (
            NativeHostTarget::Aarch64LinuxGnu,
            "aarch64-unknown-linux-gnu",
        ),
        (NativeHostTarget::X86_64Darwin, "x86_64-apple-darwin"),
        (NativeHostTarget::Aarch64Darwin, "aarch64-apple-darwin"),
        (
            NativeHostTarget::X86_64WindowsMsvc,
            "x86_64-pc-windows-msvc",
        ),
        (
            NativeHostTarget::Aarch64WindowsMsvc,
            "aarch64-pc-windows-msvc",
        ),
    ] {
        let target = target_from_native_host(Some(host)).unwrap();
        assert_eq!(target.triple, triple);
        assert_eq!(target.pointer_width, 64);
        assert_eq!(target.endian, "little");
        assert_eq!(target.panic_strategy, "unwind");
        assert_eq!(target.thread_policy, "same_thread");
    }
    assert!(target_from_native_host(None).is_none());
    assert!(
        current_target() == target_from_native_host(crate::platform::current_native_host_target())
    );
}

#[test]
fn explicit_test_target_override_still_precedes_host_admission_and_resets() {
    let before = current_target();
    // Deliberately not a native profile: this existing test-only facility must
    // preserve the complete supplied Target, not silently rebuild its fields.
    let target = Target {
        triple: "test-only-target".to_owned(),
        pointer_width: 32,
        endian: "big".to_owned(),
        panic_strategy: "abort".to_owned(),
        thread_policy: "test-only-policy".to_owned(),
    };
    with_test_target(target.clone(), || {
        assert!(current_target() == Some(target.clone()));
    });
    assert!(current_target() == before);
    let result = std::panic::catch_unwind(|| {
        with_test_target(target.clone(), || {
            assert!(current_target() == Some(target.clone()));
            panic!("test-only override unwind");
        });
    });
    assert!(result.is_err());
    assert!(current_target() == before);
}
