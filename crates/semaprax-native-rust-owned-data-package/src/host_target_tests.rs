use super::*;

#[test]
fn owned_package_host_policy_retains_exact_five_targets() {
    for (host, package, triple, archive) in [
        (
            NativeHostTarget::X86_64LinuxGnu,
            HostTarget::X86_64LinuxGnu,
            "x86_64-unknown-linux-gnu",
            "libsemaprax_native_rust_owned_data_sdk.a",
        ),
        (
            NativeHostTarget::Aarch64LinuxGnu,
            HostTarget::Aarch64LinuxGnu,
            "aarch64-unknown-linux-gnu",
            "libsemaprax_native_rust_owned_data_sdk.a",
        ),
        (
            NativeHostTarget::X86_64Darwin,
            HostTarget::X86_64Darwin,
            "x86_64-apple-darwin",
            "libsemaprax_native_rust_owned_data_sdk.a",
        ),
        (
            NativeHostTarget::Aarch64Darwin,
            HostTarget::Aarch64Darwin,
            "aarch64-apple-darwin",
            "libsemaprax_native_rust_owned_data_sdk.a",
        ),
        (
            NativeHostTarget::X86_64WindowsMsvc,
            HostTarget::X86_64WindowsMsvc,
            "x86_64-pc-windows-msvc",
            "semaprax_native_rust_owned_data_sdk.lib",
        ),
    ] {
        assert_eq!(
            HostTarget::from_native_host_target(Some(host)),
            Some(package)
        );
        assert_eq!(package.triple(), triple);
        assert_eq!(package.archive_name(), archive);
    }
    const ARM_WINDOWS: Option<HostTarget> =
        HostTarget::from_native_host_target(Some(NativeHostTarget::Aarch64WindowsMsvc));
    const UNSUPPORTED: Option<HostTarget> = HostTarget::from_native_host_target(None);
    assert_eq!(ARM_WINDOWS, None);
    assert_eq!(UNSUPPORTED, None);
    const CURRENT: Option<HostTarget> = HostTarget::current();
    assert_eq!(
        CURRENT,
        HostTarget::from_native_host_target(current_native_host_target())
    );
}
