use std::fs;
use std::path::Path;

#[test]
fn private_native_ui_is_platform_real_feature_gated_and_source_locked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = read(root, "crates/semaprax-native-host/Cargo.toml");
    let macos_script = read(root, "platform-tests/desktop-native/package-ui-macos.sh");
    let macos_source = read(root, "platform-tests/desktop-native/ui-macos.m");
    let macos_plist = read(root, "platform-tests/desktop-native/Info-ui.plist");
    let windows_script = read(root, "platform-tests/desktop-native/package-ui-windows.ps1");
    let windows_source = read(root, "platform-tests/desktop-native/ui-windows.c");
    let lock = read(root, "platform-tests/desktop-native/toolchain.lock");
    let workflow = read(root, ".github/workflows/ci.yml");
    let diagnostics = read(root, "src/codegen.rs");

    assert_contains_all(
        "private engine feature",
        &cargo,
        &[
            "unstable-desktop-app-harness = []",
            "name = \"private-desktop-v3-app\"",
            "required-features = [\"unstable-desktop-app-harness\"]",
        ],
    );
    assert!(diagnostics.contains("SPX-B104"));
    assert_contains_all(
        "private UI toolchain lock",
        &lock,
        &[
            "network=forbidden-cargo-offline",
            "macos.ld.project=ld-1167.5",
            "macos.sdk.build=24F74",
            "windows.clang.version=20.1.8",
            "windows.vswhere.version=3.1.7.39155",
            "windows.visual-studio.major=18",
            "windows.sdk.version=10.0.26100.0",
            "windows.ui.libraries=libcmt.lib,libvcruntime.lib,libucrt.lib,oldnames.lib,ucrt.lib,kernel32.lib,user32.lib,oleacc.lib,ole32.lib,oleaut32.lib,shell32.lib,uuid.lib,bcrypt.lib",
        ],
    );

    macos_contract(&format!("{macos_script}\n{macos_source}\n{macos_plist}"))
        .unwrap_or_else(|error| panic!("{error}"));
    windows_contract(&format!("{windows_script}\n{windows_source}"))
        .unwrap_or_else(|error| panic!("{error}"));

    let desktop_job = workflow
        .split("  desktop-native-product:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  ios-static-cross-check:").next())
        .expect("desktop workflow job must remain structurally delimited");
    assert_contains_all(
        "native UI workflow",
        desktop_job,
        &[
            "platform-tests/desktop-native/package-ui-macos.sh",
            "platform-tests/desktop-native/package-ui-windows.ps1",
            "semaprax-private-desktop-v3",
            "semaprax-private-desktop-ui-v1",
        ],
    );
    assert_eq!(desktop_job.matches("package-ui-macos.sh").count(), 1);
    assert_eq!(desktop_job.matches("package-ui-windows.ps1").count(), 1);
}

#[test]
fn native_ui_source_locks_reject_hostile_gate_removal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let macos = format!(
        "{}\n{}\n{}",
        read(root, "platform-tests/desktop-native/package-ui-macos.sh"),
        read(root, "platform-tests/desktop-native/ui-macos.m"),
        read(root, "platform-tests/desktop-native/Info-ui.plist")
    );
    let windows = format!(
        "{}\n{}",
        read(root, "platform-tests/desktop-native/package-ui-windows.ps1"),
        read(root, "platform-tests/desktop-native/ui-windows.c")
    );
    macos_contract(&macos).expect("checked-in macOS native UI contract");
    windows_contract(&windows).expect("checked-in Windows native UI contract");

    for hostile in [
        macos.replace("-framework Cocoa", "-framework Foundation"),
        macos.replace(
            "self.button.accessibilityLabel = kButtonName",
            "removed accessibility name",
        ),
        macos.replace("[application run]", "removed application event loop"),
        macos.replace("self.window.visible", "removed window visibility"),
        macos.replace(
            "[self.button performClick:nil]",
            "removed native control event",
        ),
        macos.replace("SemapraxPrivateEngine", "AmbientEngine"),
        macos.replace(
            "build_ui \"$scratch/ui-second/SemapraxPrivate\"",
            "removed second UI build",
        ),
        macos.replace("cmd LC_UUID", "cmd LC_SOURCE_VERSION"),
        macos.replace("[NSApp stop:nil];", "[NSApp terminate:nil];"),
        macos.replace("load_commands=$(otool -l \"$binary\")", "load_commands=''"),
        macos.replace(
            "$(printf '%s\\n' \"$load_commands\" | sed -n '1p')",
            "$binary:",
        ),
        macos.replace("\"$load_commands\" | sed '1d'", "\"$load_commands\""),
        macos.replace("expected_inventory=", "removed_inventory="),
        macos.replace(
            "CC_SHA256(engineBytes.bytes",
            "RemovedDigest(engineBytes.bytes",
        ),
        macos.replace("write_engine_manifest", "removed_engine_manifest"),
        macos.replace(
            "printf '\\000' >>\"$mismatch_engine\"",
            "removed mismatch mutation",
        ),
        macos.replace("cp /usr/bin/yes", "removed timeout engine"),
        macos.replace("kEngineDeadlineSeconds", "removedEngineDeadline"),
        macos.replace("[task terminate]", "removed task termination"),
        macos.replace("SIGKILL", "REMOVED_SIGKILL"),
    ] {
        assert!(
            macos_contract(&hostile).is_err(),
            "hostile macOS native UI gate removal escaped the source lock"
        );
    }
    for (index, hostile) in [
        windows.replace(
            "& $clangPath @compileArguments",
            "clang @compileArguments",
        ),
        windows.replace(
            "& $lldLinkPath @linkArguments",
            "lld-link @linkArguments",
        ),
        windows.replace("'-c', $uiSource, '-o', $object", "$uiSource"),
        windows.replace(
            "'/entry:wWinMainCRTStartup'",
            "'/entry:mainCRTStartup'",
        ),
        windows.replace("'/WX'", "'/WX:NO'"),
        windows.replace(
            "$vswhereVersion -ne (Lock 'windows.vswhere.version')",
            "$false",
        ),
        windows.replace(
            "'^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)$'",
            "'.*'",
        ),
        windows.replace("$Matches[1] -ne $visualStudioMajor", "$false"),
        windows.replace(
            "Exact-Library 'libcmt.lib' $vcLibRoot",
            "Exact-Library 'libcmt.lib' $sdkUcrtLibRoot",
        ),
        windows.replace(
            "Exact-Library 'libvcruntime.lib' $vcLibRoot",
            "Exact-Library 'libvcruntime.lib' $sdkUcrtLibRoot",
        ),
        windows.replace(
            "Exact-Library 'libucrt.lib' $sdkUcrtLibRoot",
            "Exact-Library 'libucrt.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'oldnames.lib' $vcLibRoot",
            "Exact-Library 'oldnames.lib' $sdkUcrtLibRoot",
        ),
        windows.replace(
            "Exact-Library 'ucrt.lib' $sdkUcrtLibRoot",
            "Exact-Library 'ucrt.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'kernel32.lib' $sdkUmLibRoot",
            "Exact-Library 'kernel32.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'user32.lib' $sdkUmLibRoot",
            "Exact-Library 'user32.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'oleacc.lib' $sdkUmLibRoot",
            "Exact-Library 'oleacc.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'ole32.lib' $sdkUmLibRoot",
            "Exact-Library 'ole32.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'oleaut32.lib' $sdkUmLibRoot",
            "Exact-Library 'oleaut32.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'shell32.lib' $sdkUmLibRoot",
            "Exact-Library 'shell32.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'uuid.lib' $sdkUmLibRoot",
            "Exact-Library 'uuid.lib' $vcLibRoot",
        ),
        windows.replace(
            "Exact-Library 'bcrypt.lib' $sdkUmLibRoot",
            "Exact-Library 'bcrypt.lib' $vcLibRoot",
        ),
        windows.replace("CreateWindowExW", "RemovedWindowFactory"),
        windows.replace("IsWindowVisible", "RemovedVisibilityProbe"),
        windows.replace("AccessibleObjectFromWindow", "RemovedAccessibilityProbe"),
        windows.replace("BM_CLICK", "REMOVED_CLICK"),
        windows.replace("GetMessageW", "RemovedMessageLoop"),
        windows.replace("SemapraxPrivateEngine.exe", "AmbientEngine.exe"),
        windows.replace("$image.Subsystem -ne 2", "$image.Subsystem -ne 3"),
        windows.replace(
            "Build-Ui (Join-Path $scratch 'ui-second')",
            "Removed-BuildUi",
        ),
        windows.replace(
            "$startInfo.ArgumentList.Add($ResultPath)",
            "$startInfo.Arguments = $ResultPath",
        ),
        windows.replace(
            "$expectedImports = @('bcrypt.dll', 'kernel32.dll', 'ole32.dll', 'oleacc.dll', 'oleaut32.dll', 'shell32.dll', 'user32.dll')",
            "$expectedImports = @('bcrypt.dll', 'kernel32.dll', 'ole32.dll', 'oleacc.dll', 'oleaut32.dll', 'shell32.dll', 'user32.dll', 'ws2_32.dll')",
        ),
        windows.replace(
            "$exportDirectory.Rva -ne 0 -or $exportDirectory.Size -ne 0",
            "$false",
        ),
        windows.replace("$exports.FunctionCount -ne 0", "$false"),
        windows.replace(
            "--coff-exports --coff-resources $ui",
            "--coff-exports --coff-resources -- $ui",
        ),
        windows.replace("BCryptHashData(hash, buffer, bytes_read, 0)", "RemovedHashData(hash, buffer, bytes_read, 0)"),
        windows.replace("!semaprax_verify_engine_digest(engine_path, &locked_engine)", "FALSE"),
        windows.replace("[System.IO.File]::WriteAllText(", "Removed-ManifestWrite("),
        windows.replace("$append.WriteByte(0)", "removed mismatch mutation"),
        windows.replace("Assert-ExactInventory", "Removed-Inventory"),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            windows_contract(&hostile).is_err(),
            "hostile Windows native UI gate removal {index} escaped the source lock"
        );
    }
}

fn macos_contract(source: &str) -> Result<(), String> {
    if source.contains("waitUntilExit") {
        return Err("unbounded AppKit engine wait escaped the source lock".to_owned());
    }
    if source.contains("otool -l \"$ui\" \"$engine\" \"$provider\"") {
        return Err(
            "macOS native UI otool filename headers must not enter the load-path scan".to_owned(),
        );
    }
    require_all(
        "macOS native UI",
        source,
        &[
            "package-ui-macos.sh ABSOLUTE_NEW_OUTPUT_DIRECTORY ABSOLUTE_ENGINE_PACKAGE",
            "Apple clang version 17.0.0 (clang-1700.0.13.5)",
            "@(#)PROGRAM:ld PROJECT:ld-1167.5",
            "--show-sdk-build-version",
            "--ld-path=\"$ld_tool\"",
            "SOURCE_DATE_EPOCH=1 ZERO_AR_DATE=1",
            "-fvisibility=hidden",
            "-framework Cocoa",
            "build_ui \"$scratch/ui-first/SemapraxPrivate\"",
            "build_ui \"$scratch/ui-second/SemapraxPrivate\"",
            "cmp -s",
            "cmd LC_UUID",
            "cmd LC_BUILD_VERSION",
            "load_commands=$(otool -l \"$binary\")",
            "[ \"$(printf '%s\\n' \"$load_commands\" | sed -n '1p')\" != \"$binary:\" ]",
            "printf '%s\\n' \"$load_commands\" | sed '1d' | grep -E 'LC_RPATH|@loader_path|@executable_path|/private/|/Users/|/Volumes/|target/'",
            "expected_ui_images=",
            "expected_inventory=",
            "CFBundlePackageType</key><string>APPL",
            "CFBundleExecutable</key><string>SemapraxPrivate",
            "plutil -extract LSBackgroundOnly",
            "NSApplicationActivationPolicyRegular",
            "NSWindowStyleMaskTitled",
            "buttonWithTitle:kButtonName",
            "self.button.accessibilityLabel = kButtonName",
            "self.window.visible",
            "[self.button performClick:nil]",
            "[application run]",
            "[NSApp stop:nil];",
            "applicationWillTerminate",
            "SemapraxPrivateEngine",
            "SemapraxPrivateEngine.sha256",
            "semaprax.private-desktop-engine-sha256.v1 ",
            "shasum -a 256",
            "write_engine_manifest",
            "printf '\\000' >>\"$mismatch_engine\"",
            "assert_rejected_without_result",
            "cp /usr/bin/yes",
            "CC_SHA256(engineBytes.bytes",
            "kEngineDeadlineSeconds",
            "deadline.timeIntervalSinceNow > 0",
            "[task terminate]",
            "kEngineTerminationGraceSeconds",
            "kill(task.processIdentifier, SIGKILL)",
            "kEngineKillGraceSeconds",
            "killDeadline.timeIntervalSinceNow > 0",
            "SEMAPRAX_DESKTOP_UI_V1_OK platform=macos",
            "lifecycle=launch,window,shown,control,close,terminate",
            "accessibility=button-name engine=calls-2-replay-exact",
        ],
    )?;
    require_ordered(
        source,
        &[
            "CC_SHA256(engineBytes.bytes",
            "[task launchAndReturnError:&launchError]",
            "deadline.timeIntervalSinceNow > 0",
            "[output.fileHandleForReading readDataToEndOfFile]",
        ],
    )
}

fn windows_contract(source: &str) -> Result<(), String> {
    for forbidden in [
        "\n  clang @arguments",
        "-fuse-ld=lld",
        "$fixedImports",
        "ws2_32.dll",
    ] {
        if source.contains(forbidden) {
            return Err(format!(
                "ambient Windows native UI tool fallback: {forbidden}"
            ));
        }
    }
    require_all(
        "Windows native UI",
        source,
        &[
            "[Parameter(Mandatory = $true)][string]$EngineRoot",
            "Resolve-CanonicalNonReparsePath",
            "Get-Command clang.exe",
            "Get-Command lld-link.exe",
            "$vswhereVersion -ne (Lock 'windows.vswhere.version')",
            "$visualStudioMajor -notmatch '^(0|[1-9][0-9]*)$'",
            "$visualStudioVersion -notmatch '^(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)\\.(0|[1-9][0-9]*)$'",
            "$Matches[1] -ne $visualStudioMajor",
            "'-c', $uiSource, '-o', $object",
            "& $clangPath @compileArguments",
            "$linkArguments = @('/Brepro', '/nodefaultlib', '/subsystem:windows'",
            "'/entry:wWinMainCRTStartup', '/machine:x64', '/WX'",
            "\"/out:$destination\"",
            "& $lldLinkPath @linkArguments",
            "windows.ui.libraries",
            "Exact-Library 'libcmt.lib' $vcLibRoot",
            "Exact-Library 'libvcruntime.lib' $vcLibRoot",
            "Exact-Library 'libucrt.lib' $sdkUcrtLibRoot",
            "Exact-Library 'oldnames.lib' $vcLibRoot",
            "Exact-Library 'ucrt.lib' $sdkUcrtLibRoot",
            "Exact-Library 'kernel32.lib' $sdkUmLibRoot",
            "Exact-Library 'user32.lib' $sdkUmLibRoot",
            "Exact-Library 'oleacc.lib' $sdkUmLibRoot",
            "Exact-Library 'ole32.lib' $sdkUmLibRoot",
            "Exact-Library 'oleaut32.lib' $sdkUmLibRoot",
            "Exact-Library 'shell32.lib' $sdkUmLibRoot",
            "Exact-Library 'uuid.lib' $sdkUmLibRoot",
            "Exact-Library 'bcrypt.lib' $sdkUmLibRoot",
            "Build-Ui (Join-Path $scratch 'ui-first')",
            "Build-Ui (Join-Path $scratch 'ui-second')",
            "$image.Subsystem -ne 2",
            "Get-PeImports",
            "Get-PeExports",
            "$llvmReadObjPath --file-headers --coff-imports --coff-exports --coff-resources $ui",
            "$expectedImports = @('bcrypt.dll', 'kernel32.dll', 'ole32.dll', 'oleacc.dll', 'oleaut32.dll', 'shell32.dll', 'user32.dll')",
            "Assert-SequenceEqual 'private Windows native UI imports' $imports $expectedImports",
            "$exportDirectory.Rva -ne 0 -or $exportDirectory.Size -ne 0",
            "$exports.FunctionCount -ne 0",
            "ordinal-only exports",
            "Get-EffectiveManifest",
            "Assert-ExactInventory",
            "[System.Diagnostics.ProcessStartInfo]::new()",
            "$startInfo.UseShellExecute = $false",
            "$startInfo.ArgumentList.Add($ResultPath)",
            "[System.Diagnostics.Process]::Start($startInfo)",
            "CreateWindowExW",
            "WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON",
            "IsWindowVisible",
            "AccessibleObjectFromWindow",
            "IAccessible_get_accName",
            "SetTimer",
            "BM_CLICK",
            "GetMessageW",
            "WM_DESTROY",
            "PostQuitMessage",
            "SemapraxPrivateEngine.exe",
            "SemapraxPrivateEngine.sha256",
            "semaprax.private-desktop-engine-sha256.v1 ",
            "[System.IO.File]::WriteAllText(",
            "BCryptOpenAlgorithmProvider",
            "BCryptHashData(hash, buffer, bytes_read, 0)",
            "semaprax_verify_engine_digest(engine_path, &locked_engine)",
            "$append.WriteByte(0)",
            "digest-mismatched private Windows engine was not rejected before result publication",
            "SEMAPRAX_DESKTOP_UI_V1_OK platform=windows",
            "lifecycle=create,window,shown,control,close,terminate",
            "accessibility=button-name engine=calls-2-replay-exact",
        ],
    )?;
    require_ordered(
        source,
        &[
            "!semaprax_verify_engine_digest(engine_path, &locked_engine)",
            "CreateProcessW(engine_path",
        ],
    )
}

fn require_ordered(source: &str, needles: &[&str]) -> Result<(), String> {
    let mut offset = 0;
    for needle in needles {
        let relative = source[offset..]
            .find(needle)
            .ok_or_else(|| format!("missing ordered native UI lock: {needle}"))?;
        offset += relative + needle.len();
    }
    Ok(())
}

fn require_all(label: &str, source: &str, needles: &[&str]) -> Result<(), String> {
    for needle in needles {
        if !source.contains(needle) {
            return Err(format!("missing {label} lock: {needle}"));
        }
    }
    Ok(())
}

fn assert_contains_all(label: &str, source: &str, needles: &[&str]) {
    require_all(label, source, needles).unwrap_or_else(|error| panic!("{error}"));
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}
