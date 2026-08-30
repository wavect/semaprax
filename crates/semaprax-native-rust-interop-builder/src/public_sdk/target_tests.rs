use super::*;
use crate::platform::NativeHostTarget;

#[test]
fn public_sdk_host_policy_retains_exact_five_targets() {
    for (host, expected) in [
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
    ] {
        assert_eq!(target_triple_for(Some(host)), Some(expected));
    }
    assert_eq!(
        target_triple_for(Some(NativeHostTarget::Aarch64WindowsMsvc)),
        None
    );
    assert_eq!(target_triple_for(None), None);
    assert_eq!(
        target_triple(),
        target_triple_for(crate::platform::current_native_host_target())
    );
}
