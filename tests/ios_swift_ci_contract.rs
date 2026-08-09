use std::fs;
use std::path::Path;

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

#[test]
fn private_ios_swift_project_is_offline_closed_and_source_locked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = root.join("platform-tests/ios-swift");
    let ignore = read(root, "platform-tests/ios-swift/.gitignore");
    let lock = read(root, "platform-tests/ios-swift/toolchain.lock");
    let header = read(
        root,
        "platform-tests/ios-swift/include/SemapraxPrivateSwift.h",
    );
    let module = read(root, "platform-tests/ios-swift/include/module.modulemap");
    let plist = read(root, "platform-tests/ios-swift/Info.plist.in");
    let package = read(root, "platform-tests/ios-swift/package.sh");
    let generator = read(
        root,
        "crates/semaprax-native-host/src/bin/private_apple_swift_v1_fixture.rs",
    );
    let types = read(root, "platform-tests/ios-swift/Sources/ContractTypes.swift");
    let fifo = read(
        root,
        "platform-tests/ios-swift/Sources/StableFifoThread.swift",
    );
    let runtime = read(root, "platform-tests/ios-swift/Sources/NativeRuntime.swift");
    let session = read(root, "platform-tests/ios-swift/Sources/OwnedSession.swift");
    let app = read(root, "platform-tests/ios-swift/Sources/ContractApp.swift");

    assert!(ignore.contains("/build/"));
    for required in [
        "schema=semaprax.ios-swift-toolchain.v1",
        "xcode.major=26",
        "swift.major=6",
        "rust.version=1.97.1",
        "ios.minimum=15.0",
        "ios.device.arch=arm64",
        "ios.simulator.archs=arm64,x86_64",
        "ios.app.arch=arm64",
    ] {
        assert!(lock.contains(required), "toolchain lock lost `{required}`");
    }
    for forbidden in [
        "http://",
        "https://",
        "Package.swift",
        "swift package",
        "pod install",
        "CocoaPods",
        "brew install",
        "curl ",
        "wget ",
    ] {
        for (name, source) in [
            ("toolchain lock", lock.as_str()),
            ("header", header.as_str()),
            ("module map", module.as_str()),
            ("plist", plist.as_str()),
            ("packager", package.as_str()),
        ] {
            assert!(!source.contains(forbidden), "{name} gained `{forbidden}`");
        }
    }

    for required in [
        "uint64_t words[8]",
        "spx_private_apple_swift_fixture_v1_open(void)",
        "spx_private_apple_swift_v1_adopt_pair",
        "spx_private_apple_swift_v1_consume",
        "uint32_t evidence_len",
        "spx_private_apple_swift_v1_close_runtime(void)",
    ] {
        assert!(header.contains(required), "C bridge lost `{required}`");
    }
    assert!(module.contains("module SemapraxPrivateSwift"));
    assert!(module.contains("header \"SemapraxPrivateSwift.h\""));
    assert!(!header.contains("spx_private_apple_swift_v1_open("));
    for required in [
        "spx_private_apple_swift_fixture_register_v1",
        "visibility(\"hidden\")",
        "spx_private_apple_swift_fixture_reset_v1",
        "spx_private_apple_swift_fixture_snapshot_v1",
    ] {
        assert!(
            generator.contains(required),
            "generated hook binding lost `{required}`"
        );
    }

    for required in [
        "adapterKAT: UInt64 = 0x0000002d00000001",
        "wrongThreadKAT: UInt64 = 0x0000002d00000002",
        "invalidHandleKAT: UInt64 = 0x0000002d00000007",
        "staleHandleKAT: UInt64 = 0x0000002d00000008",
        "crossRuntimeKAT: UInt64 = 0x0000002d00000009",
        "uncertaintyKAT: UInt64 = 0x0000002d80000001",
        "raw >> 53 == 0",
        "code != 0",
        "zero-code hostile status was accepted",
        "(1...5).contains(statusClass)",
        "(0...2).contains(retryability)",
        "isPrecommitAppleRejection",
        "knownAnswer: UInt64 = 0x0001000001000001",
        "byteCount: UInt32 = 64",
        "0x000000010000000d",
        "0x000000000000000b",
    ] {
        assert!(
            types.contains(required),
            "Swift KAT projection lost `{required}`"
        );
    }
    for required in [
        "final class StableFifoThread",
        "@unchecked Sendable",
        "private let thread: Thread",
        "commands.removeFirst()",
        "state.owner === Thread.current",
        "stable FIFO cannot join itself",
    ] {
        assert!(fifo.contains(required), "stable FIFO lost `{required}`");
    }
    assert!(!fifo.contains("DispatchQueue"));
    for required in [
        "spx_private_apple_swift_fixture_v1_open()",
        "spx_private_apple_swift_v1_adopt_pair(11, 13, &output)",
        "spx_private_apple_swift_v1_consume",
        "spx_private_apple_swift_v1_close_runtime()",
        "enqueueCleanup",
        "cleanupResults.append(result)",
        "ARC fallback is deliberately nonthrowing and never retries",
    ] {
        assert!(
            runtime.contains(required),
            "native runtime lost `{required}`"
        );
    }
    for required in [
        "cleanupRequested = true",
        "status.isPrecommitAppleRejection",
        "An unclassified host-side failure is terminal",
        "Exercises the identical action used by deinit",
        "deinit",
        "cell.requestCleanup()",
    ] {
        assert!(
            session.contains(required),
            "owned wrapper lost `{required}`"
        );
    }

    for required in [
        "#if SEMAPRAX_EXPLICIT",
        "#elseif SEMAPRAX_DEINIT",
        "mode=explicit optimization=O0",
        "mode=deinit optimization=O2",
        "adversarialPrecommit",
        "consumeRawWrongThread",
        "assertStale",
        "consumed handle became retryable",
        "runConsumeDeinitRace",
        "ARC deinit cleanup was not deterministic",
        "assertLiveClose",
        "SEMAPRAX_IOS_SWIFT_V1_FAIL",
        "semaprax-ios-swift-v1.txt",
        ".documentDirectory",
        ".atomic",
    ] {
        assert!(
            app.contains(required),
            "application contract lost `{required}`"
        );
    }
    assert!(!app.contains("crossRuntime("));
    assert!(plist.contains("__SEMAPRAX_BUNDLE_IDENTIFIER__"));
    assert!(plist.contains("<string>APPL</string>"));
    assert!(plist.contains("<string>15.0</string>"));
    assert!(!plist.contains("UIBackgroundModes"));

    let swift_files = fs::read_dir(project.join("Sources"))
        .expect("read private Swift sources")
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "swift"))
        .count();
    assert!(swift_files >= 5);
}

#[test]
fn private_ios_swift_hosted_gate_is_mandatory_and_fail_closed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = read(root, ".github/workflows/ci.yml");
    let script = read(root, "scripts/ios-swift-app-v3.sh");
    let package = read(root, "platform-tests/ios-swift/package.sh");
    let rust_harness = read(root, "crates/semaprax-native-host/src/ios_swift_harness.rs");
    let swift_job = workflow
        .split_once("  ios-swift-app-cross-check:")
        .and_then(|(_, tail)| tail.split_once("\n  android-emulator-cross-check:"))
        .map(|(job, _)| job)
        .expect("private Swift job remains a distinct fail-closed CI job");

    for required in [
        "Private Swift/iOS application + XCFramework runtime",
        "runs-on: macos-26",
        "timeout-minutes: 40",
        "actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7",
        "dtolnay/rust-toolchain@6c977a6ca4077a0ceb28ffbe03f59d46e9ac8772 # master",
        "toolchain: 1.97.1",
        "targets: aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios",
        "Build and inspect the private device + Simulator XCFramework, then run the arm64 Swift application",
        "run: scripts/ios-swift-app-v3.sh",
    ] {
        assert!(swift_job.contains(required), "hosted Swift gate lost `{required}`");
    }
    for required in [
        "set -euo pipefail",
        "uname -s",
        "uname -m",
        "Xcode 26",
        "Swift version 6",
        "--features unstable-apple-swift-harness",
        "--bin private-apple-swift-v1-fixture",
        "device-arm64.c",
        "simulator-arm64.c",
        "simulator-x86_64.c",
        "aarch64-apple-ios",
        "aarch64-apple-ios-sim",
        "x86_64-apple-ios",
        "--lib --crate-type staticlib",
        "xcrun simctl bootstatus",
        "xcrun simctl install",
        "xcrun simctl launch",
        "xcrun simctl get_app_container",
        "xcrun simctl uninstall",
        "SEMAPRAX_IOS_SWIFT_V1_OK mode=explicit optimization=O0",
        "SEMAPRAX_IOS_SWIFT_V1_OK mode=deinit optimization=O2",
    ] {
        assert!(script.contains(required), "hosted script lost `{required}`");
    }
    for required in [
        "compile_fixture iphoneos",
        "compile_fixture iphonesimulator",
        "arm64-apple-ios${minimum_ios_version}",
        "arm64-apple-ios${minimum_ios_version}-simulator",
        "x86_64-apple-ios${minimum_ios_version}-simulator",
        "-Wall -Wextra -Werror -pedantic",
        "xcrun lipo -create",
        "lib-simulator-universal.a",
        "xcodebuild -create-xcframework",
        "SemapraxPrivateSwift.xcframework",
        "SupportedPlatform",
        "simulator",
        "xcrun --sdk \"$sdk\" swiftc",
        "-module-name SemapraxPrivateSwiftContractApp",
        "-swift-version 6 -strict-concurrency=complete -warnings-as-errors",
        "Mach-O 64-bit executable arm64",
        "platform IOSSIMULATOR",
        "platform IOS",
        "otool -L",
        "nm -gjU",
        "private Swift app has an unexpected dependency",
        "codesign --verify --strict",
    ] {
        assert!(
            package.contains(required),
            "packaging gate lost `{required}`"
        );
    }
    for required in [
        "const CODE_WRONG_THREAD: u32 = 2",
        "const CODE_CROSS_RUNTIME: u32 = 9",
        "const CODE_UNCERTAIN: u32 = 0x8000_0001",
        "words: [u64; 8]",
        "mem::size_of::<PrivateAppleSwiftEvidenceV1>() == 64",
        "spx_private_apple_swift_fixture_register_v1",
        "reset: spx_private_apple_swift_fixture_reset_v1",
        "snapshot: spx_private_apple_swift_fixture_snapshot_v1",
    ] {
        assert!(
            rust_harness.contains(required),
            "Rust-side proof boundary lost `{required}`"
        );
    }
    assert!(!rust_harness.contains("fn spx_private_apple_swift_v1_open("));
    for required in [
        "expected_app_exports",
        "_spx_private_apple_swift_fixture_v1_open",
        "_spx_private_apple_swift_v1_adopt_pair",
        "_spx_private_apple_swift_v1_consume",
        "_spx_private_apple_swift_v1_close_runtime",
        "-exported_symbols_list",
    ] {
        assert!(
            package.contains(required),
            "app export gate lost `{required}`"
        );
    }
}
