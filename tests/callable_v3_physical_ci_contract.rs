use std::fs;
use std::path::Path;

#[test]
fn private_callable_v3_physical_ci_evidence_is_mandatory() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .expect("read the pinned CI workflow");
    let provider_main = fs::read_to_string(root.join("src/codegen/native_callable_provider_v3.rs"))
        .expect("read the private callable-v3 provider");
    let provider_tests =
        fs::read_to_string(root.join("src/codegen/native_callable_provider_v3/tests.rs"))
            .unwrap_or_default();
    let provider = format!("{provider_main}\n{provider_tests}");
    let joint = fs::read_to_string(
        root.join("crates/semaprax-native-host/src/settlement_host_v3_integration.rs"),
    )
    .expect("read the private callable-v3 joint integration");
    let ios_simulator = fs::read_to_string(root.join("scripts/ios-simulator-v3.sh"))
        .expect("read the private callable-v3 iOS Simulator gate");
    let android_emulator = fs::read_to_string(root.join("scripts/android-emulator-v3.sh"))
        .expect("read the private callable-v3 Android Emulator gate");
    let android_jni = fs::read_to_string(root.join("scripts/android-jni-app-v3.sh"))
        .expect("read the private Android JNI application gate");

    for required in [
        "Require Windows callable-v2 and private callable-v3 physical evidence",
        "Require private callable-v3 provider sanitizers (Linux)",
        "SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS: \"1\"",
        "CLANG: clang",
        "codegen::native_callable_provider_v3::tests::authoritative_fourteen_case_graph_providers_execute_and_settle_at_o0_o2",
        "settlement_host_v3_integration::generated_provider_loader_host_v3_end_to_end_is_exact",
        "settlement_host_v3_integration::generated_provider_loader_host_v3_physical_failures_are_durable_at_o0_o2",
        "Run dynamically loaded callable-v2 and private callable-v3 paths under ASan and UBSan",
        "Private iOS static loader + host runtime",
        "Require every private iOS static target to compile",
        "Run generated callable-v3 through the arm64 iOS Simulator",
        "run: scripts/ios-simulator-v3.sh",
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "x86_64-apple-ios",
        "aarch64-apple-ios-macabi",
        "x86_64-apple-ios-macabi",
        "cargo check --locked -p semaprax-native-loader --target \"$target\" --all-targets",
        "cargo check --locked -p semaprax-native-host --target \"$target\" --all-targets",
        "loader_tree=\"$(cargo tree --locked -p semaprax-native-loader --target \"$target\" -e normal)\"",
        "host_tree=\"$(cargo tree --locked -p semaprax-native-host --target \"$target\" -e normal)\"",
        "if grep -q libloading <<<\"$loader_tree\" || grep -q libloading <<<\"$host_tree\"",
        "iOS static loader/host target unexpectedly resolved libloading",
        "Private Android dynamic loader + host runtime",
        "Enable Android emulator hardware acceleration",
        "Run generated callable-v3 through the x86_64 Android Emulator",
        "ReactiveCircus/android-emulator-runner@a421e43855164a8197daf9d8d40fe71c6996bb0d # v2.38.0",
        "targets: x86_64-linux-android,aarch64-linux-android",
        "api-level: 35",
        "target: default",
        "arch: x86_64",
        "ndk: 27.2.12479018",
        "script: scripts/android-emulator-v3.sh",
        "Private Android JNI/Kotlin application runtime",
        "Enable Android JNI emulator hardware acceleration",
        "Build offline and run the private JNI/Kotlin instrumentation APK",
        "script: scripts/android-jni-app-v3.sh",
    ] {
        assert!(
            workflow.contains(required),
            "CI lost mandatory private callable-v3 evidence: {required}"
        );
    }
    assert!(provider
        .contains("fn authoritative_fourteen_case_graph_providers_execute_and_settle_at_o0_o2()"));
    assert!(provider.contains("SEMAPRAX_REQUIRE_CALLABLE_V3_SANITIZERS"));
    assert!(joint.contains("fn generated_provider_loader_host_v3_end_to_end_is_exact()"));
    assert!(joint
        .contains("fn generated_provider_loader_host_v3_physical_failures_are_durable_at_o0_o2()"));
    assert!(joint.contains("REQUIRED_SANITIZERS_ENV"));
    for required in [
        "set -euo pipefail",
        "--target aarch64-apple-ios-sim",
        "--features unstable-ios-simulator-harness",
        "--bin private-ios-simulator-v3-fixture",
        "--lib --crate-type staticlib",
        "minimum_ios_version=\"15.0\"",
        "xcrun --sdk iphonesimulator clang",
        "Mach-O 64-bit executable arm64",
        "otool -L",
        "codesign --verify --strict",
        "xcrun simctl bootstatus",
        "xcrun simctl spawn",
        "SEMAPRAX_IOS_SIM_V3_OK O$optimization target=arm64-simulator finalizers=1:13,0:11 publication=no-owned allocations=0",
    ] {
        assert!(
            ios_simulator.contains(required),
            "iOS Simulator gate lost mandatory physical evidence: {required}"
        );
    }
    for required in [
        "set -euo pipefail",
        "android_ndk_version=\"27.2.12479018\"",
        "android_api_level=\"35\"",
        "android_minimum_api=\"28\"",
        "--features unstable-android-emulator-harness",
        "--bin private-android-emulator-v3-fixture",
        "x86_64-linux-android",
        "aarch64-linux-android",
        "arm64_provider_source",
        "libsemaprax_android_v3_arm64.so",
        "grep -F 'AArch64'",
        "libloading v0.9.0",
        "--lib --crate-type staticlib",
        "-fPIC -shared",
        "llvm-readelf",
        "adb shell getprop ro.product.cpu.abi",
        "adb shell realpath",
        "SEMAPRAX_ANDROID_V3_MARKER",
        "SEMAPRAX_ANDROID_EMULATOR_V3_OK O$optimization target=x86_64-android finalizers=1:13,0:11 publication=no-owned allocations=0",
    ] {
        assert!(
            android_emulator.contains(required),
            "Android Emulator gate lost mandatory physical evidence: {required}"
        );
    }
    for required in [
        "set -euo pipefail",
        "android_ndk_version=\"27.2.12479018\"",
        "android_api_level=\"35\"",
        "android_minimum_api=\"28\"",
        "--features unstable-android-jni-harness",
        "--bin private-android-jni-v3-fixture",
        "x86_64-linux-android",
        "aarch64-linux-android",
        "libsemaprax_provider_o0.so",
        "libsemaprax_provider_o2.so",
        "libsemaprax_jni.so",
        "JNI_OnLoad",
        "gradle --offline",
        "adb shell am instrument -w",
        "adb shell run-as",
        "SEMAPRAX_ANDROID_JNI_V1_OK",
    ] {
        assert!(
            android_jni.contains(required),
            "Android JNI application gate lost mandatory evidence: {required}"
        );
    }
}
