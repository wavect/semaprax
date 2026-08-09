#![cfg(target_os = "ios")]

use semaprax_native_loader::{
    register_admitted_ios_static_settlement_exact, IosStaticTarget, NativeStaticSettlementLease,
    StaticDescriptorGetter, StaticExecuteEntry, StaticSettleEntry,
    StaticSettlementRegistrationError,
};

type RegisterStaticSettlement =
    unsafe fn(
        IosStaticTarget,
        &'static [u8],
        StaticDescriptorGetter,
        StaticExecuteEntry,
        StaticSettleEntry,
    ) -> Result<NativeStaticSettlementLease, StaticSettlementRegistrationError>;

const REGISTER_STATIC_SETTLEMENT: RegisterStaticSettlement =
    register_admitted_ios_static_settlement_exact;

#[cfg(all(
    target_arch = "aarch64",
    not(any(target_abi = "macabi", target_abi = "sim"))
))]
const _: () = assert!(matches!(
    IosStaticTarget::current(),
    Some(IosStaticTarget::DeviceArm64)
));

#[cfg(all(target_arch = "aarch64", target_abi = "sim"))]
const _: () = assert!(matches!(
    IosStaticTarget::current(),
    Some(IosStaticTarget::SimulatorArm64)
));

#[cfg(all(target_arch = "x86_64", target_abi = "sim"))]
const _: () = assert!(matches!(
    IosStaticTarget::current(),
    Some(IosStaticTarget::SimulatorX86_64)
));

#[cfg(all(target_arch = "aarch64", target_abi = "macabi"))]
const _: () = assert!(matches!(
    IosStaticTarget::current(),
    Some(IosStaticTarget::MacCatalystArm64)
));

#[cfg(all(target_arch = "x86_64", target_abi = "macabi"))]
const _: () = assert!(matches!(
    IosStaticTarget::current(),
    Some(IosStaticTarget::MacCatalystX86_64)
));

#[test]
fn ios_consumer_sees_the_exact_static_registration_surface() {
    let _register = REGISTER_STATIC_SETTLEMENT;
    let current =
        IosStaticTarget::current().expect("CI installs only supported iOS-family targets");

    #[cfg(all(target_arch = "aarch64", target_abi = "macabi"))]
    assert_eq!(current, IosStaticTarget::MacCatalystArm64);
    #[cfg(all(target_arch = "x86_64", target_abi = "macabi"))]
    assert_eq!(current, IosStaticTarget::MacCatalystX86_64);
    #[cfg(all(target_arch = "aarch64", target_abi = "sim"))]
    assert_eq!(current, IosStaticTarget::SimulatorArm64);
    #[cfg(all(target_arch = "x86_64", target_abi = "sim"))]
    assert_eq!(current, IosStaticTarget::SimulatorX86_64);
    #[cfg(all(
        target_arch = "aarch64",
        not(any(target_abi = "macabi", target_abi = "sim"))
    ))]
    assert_eq!(current, IosStaticTarget::DeviceArm64);
}
