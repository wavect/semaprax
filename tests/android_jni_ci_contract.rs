use std::fs;
use std::path::Path;

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}

#[test]
fn private_android_jni_project_is_offline_closed_and_source_locked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ignore = read(root, "platform-tests/android-jni/.gitignore");
    let settings = read(root, "platform-tests/android-jni/settings.gradle.kts");
    let build = read(root, "platform-tests/android-jni/build.gradle.kts");
    let lock = read(root, "platform-tests/android-jni/toolchain.lock");
    let manifest = read(root, "platform-tests/android-jni/AndroidManifest.xml");
    let package = read(root, "platform-tests/android-jni/package.sh");
    let bridge = read(
        root,
        "platform-tests/android-jni/src/dev/semaprax/runtime/NativeBridge.kt",
    );
    let runtime = read(
        root,
        "platform-tests/android-jni/src/dev/semaprax/runtime/NativeRuntime.kt",
    );
    let cleaner = read(
        root,
        "platform-tests/android-jni/src/dev/semaprax/runtime/CleanerCompat.kt",
    );
    let session = read(
        root,
        "platform-tests/android-jni/src/dev/semaprax/runtime/OwnedSession.kt",
    );
    let status = read(
        root,
        "platform-tests/android-jni/src/dev/semaprax/runtime/StatusWord.kt",
    );
    let handle = read(
        root,
        "platform-tests/android-jni/src/dev/semaprax/runtime/OpaqueHandle.kt",
    );
    let instrumentation = read(
        root,
        "platform-tests/android-jni/src/dev/semaprax/instrumentation/ContractInstrumentation.kt",
    );

    assert_eq!(
        settings.trim(),
        "rootProject.name = \"semaprax-private-android-jni\""
    );
    assert!(ignore.contains("/.gradle/"));
    assert!(ignore.contains("/build/"));
    assert!(!build.contains("plugins {"));
    assert!(!build.contains("repositories {"));
    assert!(!build.contains("maven"));
    for required in [
        "tasks.register<Exec>(\"assembleAndroidJni\")",
        "commandLine(\"bash\", \"package.sh\")",
        "dependsOn(\"assembleAndroidJni\")",
    ] {
        assert!(
            build.contains(required),
            "offline Gradle gate lost `{required}`"
        );
    }
    for required in [
        "gradle.major=9",
        "kotlin.major=2",
        "android.compile_api=35",
        "android.target_api=35",
        "android.minimum_api=28",
        "android.build_tools=35.0.0",
        "android.ndk=27.2.12479018",
        "android.abi=x86_64",
    ] {
        assert!(lock.contains(required), "toolchain lock lost `{required}`");
    }
    for forbidden in [
        "http://",
        "https://",
        "mavenCentral",
        "google()",
        "SNAPSHOT",
        "+\"",
    ] {
        assert!(!settings.contains(forbidden));
        assert!(!build.contains(forbidden));
        assert!(!package.contains(forbidden));
    }

    for required in [
        "android:minSdkVersion=\"28\"",
        "android:targetSdkVersion=\"35\"",
        "android:extractNativeLibs=\"true\"",
        "android:debuggable=\"true\"",
        "android:name=\"dev.semaprax.instrumentation.ContractInstrumentation\"",
        "android:targetPackage=\"dev.semaprax.runtime\"",
    ] {
        assert!(manifest.contains(required), "manifest lost `{required}`");
    }
    for forbidden in [
        "<activity",
        "<service",
        "<receiver",
        "<provider",
        "android.permission",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "manifest gained `{forbidden}`"
        );
    }

    for required in [
        "build_tools_version=\"35.0.0\"",
        "kotlinc-jvm 2[.]",
        "-jvm-target 11",
        "-language-version 2.2",
        "-api-version 2.2",
        "-no-reflect",
        "d8\"",
        "aapt2\" link",
        "zipalign\" -P 16",
        "apksigner\" sign",
        "lib/x86_64/libsemaprax_jni.so",
        "lib/x86_64/libsemaprax_provider_o0.so",
        "lib/x86_64/libsemaprax_provider_o2.so",
        "lib/x86_64/libsemaprax_provider_rf_o0.so",
        "lib/x86_64/libsemaprax_provider_rf_o2.so",
    ] {
        assert!(
            package.contains(required),
            "packaging gate lost `{required}`"
        );
    }

    for required in [
        "private external fun nativeOpen(providerPathUtf8: ByteArray, selector: Int): Long",
        "private external fun nativeAdoptPair(",
        "private external fun nativeAdoptSingle(payload: Long, outHandle: LongArray): Long",
        "private external fun nativeConsume(handle: Long, outEvidence: LongArray): Long",
        "private external fun nativeExecuteRequiresFalse(handle: Long, outEvidence: LongArray): Long",
        "private external fun nativeCloseRuntime(): Long",
        "private external fun nativeProbeException(callback: Runnable): Long",
        "private external fun nativeConsumeRawWrongThread(handle: Long, outEvidence: LongArray): Long",
        "libsemaprax_jni.so",
        "EVIDENCE_WORDS = 8",
        "EXPECTED_FIRST_FINALIZER = (1L shl 32) or 13L",
        "EXPECTED_SECOND_FINALIZER = 11L",
        "SELECTOR_DISCARD = 0",
        "SELECTOR_REQUIRES_FALSE = 1",
        "REQUIRE_FALSE_OWNER_PAYLOAD = -1L",
        "REQUIRE_FALSE_STATUS_WORD = 1L",
        "REQUIRE_FALSE_FINALIZER_COUNT = 1L",
        "REQUIRE_FALSE_FINALIZER =\n            (0L shl 32) or REQUIRE_FALSE_OWNER_PAYLOAD",
        "fun requireRequiresFalseExact()",
    ] {
        assert!(bridge.contains(required), "Kotlin JNI contract lost `{required}`");
    }
    for required in [
        "HandlerThread",
        "enqueueCleaner",
        "closeRuntime",
        "barrier()",
        "Drain every cleanup command already accepted",
        "Accepted ownership work cannot be abandoned",
        "Thread.currentThread().interrupt()",
    ] {
        assert!(
            runtime.contains(required),
            "runtime dispatcher lost `{required}`"
        );
    }
    for required in [
        "PhantomReference",
        "ReferenceQueue",
        "AtomicBoolean",
        "compareAndSet(false, true)",
        "isDaemon = true",
    ] {
        assert!(
            cleaner.contains(required),
            "CleanerCompat lost `{required}`"
        );
    }
    for required in [
        "cleanupRequested = true",
        "isPrecommitAndroidRejection",
        "finishPrecommit(handle)?.let(runtime::enqueueCleaner)",
        "finishTerminal(handle)",
        "Reference.reachabilityFence(this)",
    ] {
        assert!(
            session.contains(required),
            "owned wrapper lost `{required}`"
        );
    }
    for required in [
        "KAT_ANDROID_ADAPTER = 0x0000002d00000001L",
        "KAT_WRONG_THREAD = 0x0000002d00000002L",
        "KAT_INVALID_HANDLE = 0x0000002d00000007L",
        "KAT_STALE_HANDLE = 0x0000002d00000008L",
        "KAT_CROSS_RUNTIME = 0x0000002d00000009L",
        "KAT_REENTRANT = 0x0000002d0000000bL",
        "KAT_DECLARED_FIXTURE = 0x0000006b00000007L",
        "KAT_UNEXPECTED_ADAPTER = 0x0000004500000001L",
        "RESERVED_SHIFT = 53",
        "statusClass in 1..5",
        "retry in 0..2",
    ] {
        assert!(
            status.contains(required),
            "SPXAJS01 projection lost `{required}`"
        );
    }
    for required in [
        "KNOWN_ANSWER = 0x0001000001000001L",
        "runtimeTag in 1..MAX_TAG",
        "generation in 1..MAX_COMPONENT",
        "slot in 1..MAX_COMPONENT",
        "fun requireKnownAnswer()",
    ] {
        assert!(
            handle.contains(required),
            "SPXAJH01 projection lost `{required}`"
        );
    }
    for required in [
        "runConsumeCleanerRace",
        "runRequiresFalseWitness",
        "requireRequiresFalseExact",
        "SELECTOR_REQUIRES_FALSE",
        "executeRequiresFalse",
        "libsemaprax_provider_rf_o0.so",
        "libsemaprax_provider_rf_o2.so",
        "runInterruptedAcceptedWork",
        "provider trace reset is per-consume",
        "consumeWrongThread",
        "crossRuntime",
        "handle == NativeBridge.HANDLE_KAT",
        "DeclaredFixtureException",
        "IllegalStateException",
        "requireUntouchedFailure",
        "semaprax-android-jni-v1.txt",
        "SEMAPRAX_ANDROID_JNI_V1_OK",
        "handles=0 rf=1",
    ] {
        assert!(
            instrumentation.contains(required),
            "instrumentation lost `{required}`"
        );
    }
}

#[test]
fn private_android_jni_hosted_gate_is_mandatory_and_fail_closed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = read(root, ".github/workflows/ci.yml");
    let gate = read(root, "scripts/android-jni-app-v3.sh");
    for required in [
        "Private Android JNI/Kotlin application runtime",
        "Build offline and run the private JNI/Kotlin instrumentation APK",
        "ReactiveCircus/android-emulator-runner@e89f39f1abbbd05b1113a29cf4db69e7540cae5a # v2.37.0",
        "api-level: 35",
        "arch: x86_64",
        "ndk: 27.2.12479018",
        "script: scripts/android-jni-app-v3.sh",
    ] {
        assert!(
            workflow.contains(required),
            "hosted JNI gate lost `{required}`"
        );
    }
    for required in [
        "gradle --offline",
        "--features unstable-android-jni-harness",
        "--bin private-android-jni-v3-fixture",
        "x86-discard.c",
        "arm64-discard.c",
        "x86-requires-false.c",
        "arm64-requires-false.c",
        "libsemaprax_provider_rf_o0.so",
        "libsemaprax_provider_rf_o2.so",
        "handles=0 rf=1",
        "x86_64-linux-android",
        "aarch64-linux-android",
        "--version-script=",
        "JNI_OnLoad",
        "defined_exports",
        "jni_needed",
        "adb uninstall",
        "adb install --no-streaming",
        "adb shell am instrument -w",
        "adb shell run-as",
        "SEMAPRAX_ANDROID_JNI_V1_OK",
    ] {
        assert!(
            gate.contains(required),
            "hosted JNI script lost `{required}`"
        );
    }
}
