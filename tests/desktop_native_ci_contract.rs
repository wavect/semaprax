use std::fs;
use std::path::Path;

const PROVIDER_ID: &str = "@rpath/SemapraxPrivateProvider.dylib";

#[test]
fn private_desktop_packages_are_feature_gated_native_and_source_locked() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo = read(root, "crates/semaprax-native-host/Cargo.toml");
    let macos = read(root, "platform-tests/desktop-native/package-macos.sh");
    let windows = read(root, "platform-tests/desktop-native/package-windows.ps1");
    let windows_inspector = read(
        root,
        "platform-tests/desktop-native/inspect-windows-package.ps1",
    );
    let windows_contract_source = format!("{windows}\n{windows_inspector}");
    let plist = read(root, "platform-tests/desktop-native/Info.plist");
    let manifest = read(
        root,
        "platform-tests/desktop-native/private-desktop-v3-app.exe.manifest",
    );
    let lock = read(root, "platform-tests/desktop-native/toolchain.lock");
    let generator = read(
        root,
        "crates/semaprax-native-host/src/bin/private_desktop_v3_fixture.rs",
    );
    let runner = read(
        root,
        "crates/semaprax-native-host/src/desktop_app_harness.rs",
    );
    let workflow = read(root, ".github/workflows/ci.yml");

    assert_contains_all(
        "Cargo",
        &cargo,
        &[
            "unstable-desktop-app-harness = []",
            "name = \"private-desktop-v3-fixture\"",
            "name = \"private-desktop-v3-app\"",
            "required-features = [\"unstable-desktop-app-harness\"]",
        ],
    );
    macos_contract(&macos).unwrap_or_else(|error| panic!("{error}"));
    windows_contract(&windows_contract_source).unwrap_or_else(|error| panic!("{error}"));

    assert!(plist.contains("<key>CFBundlePackageType</key><string>APPL</string>"));
    assert!(plist.contains("<key>LSBackgroundOnly</key><true/>"));
    assert!(manifest.contains("requestedExecutionLevel level=\"asInvoker\""));
    assert_contains_all(
        "desktop toolchain lock",
        &lock,
        &[
            "schema=semaprax.private-desktop-toolchain.v1",
            "rust.version=1.97.1",
            "rust.commit=8bab26f4f-2026-07-14",
            "rust.llvm=22.1.6",
            "macos.clang.version=21.0.0",
            "macos.clang.build=2100.1.1.101",
            "macos.ld.project=ld-1267",
            "macos.ld.build-version=1267.0",
            "macos.sdk.version=26.5",
            "macos.sdk.build=25F70",
            "macos.deployment-target=11.0",
            "windows.clang.version=20.1.8",
            "windows.vswhere.version=3.1.7.0",
            "windows.visual-studio.version=18.7.11925.98",
            "windows.msvc.tools.version=14.51.36231",
            "windows.link.version=14.51.36231.0",
            "windows.sdk.version=10.0.26100.0",
            "windows.provider.libraries=libcmt.lib,libvcruntime.lib,libucrt.lib,oldnames.lib,ucrt.lib,kernel32.lib",
            "network=forbidden-cargo-offline",
            "reproducibility=two-independent-target-directories-byte-equal",
        ],
    );
    assert!(generator.contains("DeclarationId::new(\"token.identity\")"));
    assert!(generator.contains("PrivateNativeCallableV3Fixture::OwnedIdentity"));
    assert!(runner.contains("pub unsafe fn private_desktop_v3_app_main"));
    assert!(runner.contains("execute_owned_success(&[original], &[41])"));
    assert!(runner.contains("execute_owned_success(&[refreshed], &[43])"));

    let verify_job = workflow
        .split("  verify:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  ios-static-cross-check:").next())
        .expect("verify workflow job must remain structurally delimited");
    assert_contains_all(
        "desktop workflow",
        verify_job,
        &[
            "timeout-minutes: 25",
            "toolchain: 1.97.1",
            "platform-tests/desktop-native/package-macos.sh",
            "platform-tests/desktop-native/package-windows.ps1",
        ],
    );
    assert_eq!(verify_job.matches("package-macos.sh").count(), 1);
    assert_eq!(verify_job.matches("package-windows.ps1").count(), 1);
}

#[test]
fn macos_source_lock_rejects_hostile_gate_removal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = read(root, "platform-tests/desktop-native/package-macos.sh");
    macos_contract(&source).expect("checked-in macOS packaging contract");

    for hostile in [
        source.replace(" --offline", ""),
        source.replace(PROVIDER_ID, "/tmp/provider.dylib"),
        source.replace("@(#)PROGRAM:ld PROJECT:ld-1267", "ambient-ld"),
        source.replace("--show-sdk-build-version", "--show-sdk-version"),
        source.replace("CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER", "REMOVED_LINKER"),
        source.replace("--ld-path=$ld_tool", "-fuse-ld=lld"),
        source.replace("cmd LC_UUID", "cmd LC_SOURCE_VERSION"),
        source.replace("cmd LC_BUILD_VERSION", "cmd LC_SOURCE_VERSION"),
        source.replace("otool -hv", "otool -l"),
        source.replace("otool -D", "otool -L"),
        source.replace("otool -L", "otool -l"),
        source.replace("nm -gjU", "nm"),
        source.replace("build_once second", ": second build removed"),
        source.replace("cmp -s", "test -s"),
        source.replace("expected_inventory=", "removed_inventory="),
    ] {
        assert!(
            macos_contract(&hostile).is_err(),
            "hostile macOS gate removal escaped the source lock"
        );
    }
}

#[test]
fn windows_source_lock_rejects_hostile_gate_removal() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let package = read(root, "platform-tests/desktop-native/package-windows.ps1");
    let inspector = read(
        root,
        "platform-tests/desktop-native/inspect-windows-package.ps1",
    );
    let source = format!("{package}\n{inspector}");
    windows_contract(&source).expect("checked-in Windows packaging contract");

    for hostile in [
        source.replace("--offline", ""),
        source.replace("0x50", "0x4d"),
        source.replace("0x8664", "0x014c"),
        source.replace("VC/Tools/MSVC/$vcToolsVersion", "ambient-msvc"),
        source.replace("Windows Kits\\Installed Roots", "Ambient Windows Kits"),
        source.replace("Assert-ExactLibrary", "Removed-ExactLibrary"),
        source.replace(
            "Resolve-CanonicalNonReparsePath",
            "Removed-NonReparseValidation",
        ),
        source.replace(
            "Resolve-CanonicalNonReparsePath (Join-Path $vcToolsRoot 'bin/Hostx64/x64/link.exe') 'link.exe'",
            "Resolve-Path (Join-Path $vcToolsRoot 'bin/Hostx64/x64/link.exe')",
        ),
        source.replace(
            "Resolve-CanonicalNonReparsePath (Join-Path $sdkLibRoot 'ucrt/x64') 'Windows SDK UCRT x64 root'",
            "Resolve-Path (Join-Path $sdkLibRoot 'ucrt/x64')",
        ),
        source.replace(
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
            "REMOVED_TARGET_LINKER",
        ),
        source.replace("$exactLibraryPath", "$ambientLibraryPath"),
        source.replace("& $clangPath -std=c11", "clang -std=c11"),
        source.replace("\"--ld-path=$lldLinkPath\"", "-fuse-ld=lld"),
        source.replace("Get-PeImports", "Removed-PeImports"),
        source.replace("Get-PeExports", "Removed-PeExports"),
        source.replace(
            "Assert-ExternalManifestIsEffective",
            "Removed-EffectiveManifest",
        ),
        source.replace(
            "Build-Once -Label 'second'",
            "Removed-Build -Label 'second'",
        ),
        source.replace("Assert-ByteEqual", "Removed-ByteEqual"),
        source.replace("Assert-ExactInventory", "Removed-Inventory"),
    ] {
        assert!(
            windows_contract(&hostile).is_err(),
            "hostile Windows gate removal escaped the source lock"
        );
    }
}

fn macos_contract(source: &str) -> Result<(), String> {
    require_all(
        "macOS",
        source,
        &[
            "rustc 1.97.1 (8bab26f4f 2026-07-14)",
            "LLVM version: 22.1.6",
            "Apple clang version 21.0.0 (clang-2100.1.1.101)",
            "@(#)PROGRAM:ld PROJECT:ld-1267",
            "readonly_sdk_version='26.5'",
            "readonly_sdk_build='25F70'",
            "readonly_deployment_target='11.0'",
            "readonly_ld_build_version='1267.0'",
            PROVIDER_ID,
            "output directory must not already exist or be a symbolic link",
            "cargo run --quiet --offline --locked",
            "cargo build --quiet --offline --locked --release",
            "xcrun --sdk macosx --find clang",
            "xcrun --sdk macosx --find ld",
            "xcrun --sdk macosx --show-sdk-path",
            "xcrun --sdk macosx --show-sdk-version",
            "xcrun --sdk macosx --show-sdk-build-version",
            "CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER",
            "SOURCE_DATE_EPOCH=1 ZERO_AR_DATE=1",
            "RUSTFLAGS=\"-C link-arg=--ld-path=$ld_tool\"",
            "-isysroot \"$sdk_path\"",
            "-mmacosx-version-min=\"$readonly_deployment_target\"",
            "--ld-path=\"$ld_tool\"",
            "-Wl,-install_name",
            "build_once first",
            "build_once second",
            "cmp -s",
            "shasum -a 256",
            "Mach-O 64-bit",
            "otool -hv",
            "MH_MAGIC_64",
            "EXECUTE",
            "DYLIB",
            "cmd LC_UUID",
            "cmd LC_BUILD_VERSION",
            "platform 1",
            "tool 3",
            "version $readonly_ld_build_version",
            "otool -D",
            "otool -l",
            "LC_RPATH|@loader_path|@executable_path|/private/|/Users/|/Volumes/|target/",
            "otool -L",
            "actual_executable_images",
            "actual_provider_images",
            "nm -gjU",
            "actual_provider_exports",
            "actual_app_exports",
            "expected_inventory=",
            "find \"$app\" -type l",
            "SEMAPRAX_DESKTOP_V3_OK platform=macos",
        ],
    )?;
    require_ordered(source, &["build_once first", "build_once second", "cmp -s"])
}

fn windows_contract(source: &str) -> Result<(), String> {
    for forbidden in ["\n    clang -std=c11", "-fuse-ld=lld"] {
        if source.contains(forbidden) {
            return Err(format!(
                "forbidden ambient Windows linker fallback: {forbidden}"
            ));
        }
    }
    require_all(
        "Windows",
        source,
        &[
            "Lock 'rust.version'",
            "Lock 'rust.commit'",
            "Lock 'rust.llvm'",
            "$expectedRustLine",
            "Rust pin mismatch",
            "host: x86_64-pc-windows-msvc",
            "Lock 'windows.clang.version'",
            "$clangPath -dumpmachine",
            "Get-Command clang.exe",
            "Get-Command llvm-readobj.exe",
            "Get-Command lld-link.exe",
            "$llvmReadObjPath --version",
            "$lldLinkPath --version",
            "& $clangPath -std=c11",
            "\"--ld-path=$lldLinkPath\"",
            "windows.vswhere.version",
            "windows.visual-studio.version",
            "windows.msvc.tools.version",
            "windows.link.version",
            "Microsoft.VCToolsVersion.default.txt",
            "VC/Tools/MSVC/$vcToolsVersion",
            "bin/Hostx64/x64/link.exe",
            "Incremental Linker Version",
            "Windows Kits\\Installed Roots",
            "windows.sdk.version",
            "Lib/$sdkVersion",
            "Assert-ExactLibrary",
            "import library is not a COFF archive",
            "windows.provider.libraries",
            "libcmt.lib,libvcruntime.lib,libucrt.lib,oldnames.lib,ucrt.lib,kernel32.lib",
            "function Resolve-CanonicalNonReparsePath",
            "[System.IO.Path]::GetPathRoot($full)",
            "$components = @('') +",
            "foreach ($component in $components)",
            "path contains a reparse point",
            "Resolve-CanonicalNonReparsePath (Join-Path ${env:ProgramFiles} 'LLVM/bin') 'LLVM root'",
            "Resolve-CanonicalNonReparsePath (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe') 'vswhere.exe'",
            "Resolve-CanonicalNonReparsePath $visualStudioRoot 'Visual Studio root'",
            "Resolve-CanonicalNonReparsePath (Join-Path $visualStudioRoot 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt') 'MSVC tools version file'",
            "Resolve-CanonicalNonReparsePath (Join-Path $visualStudioRoot \"VC/Tools/MSVC/$vcToolsVersion\") 'MSVC tools root'",
            "Resolve-CanonicalNonReparsePath (Join-Path $vcToolsRoot 'bin/Hostx64/x64/link.exe') 'link.exe'",
            "Resolve-CanonicalNonReparsePath (Get-ItemPropertyValue -LiteralPath $kitsRegistry -Name KitsRoot10) 'Windows Kits root'",
            "Resolve-CanonicalNonReparsePath (Join-Path $kitsRoot \"Lib/$sdkVersion\") 'Windows SDK library root'",
            "Resolve-CanonicalNonReparsePath (Join-Path $vcToolsRoot 'lib/x64') 'MSVC x64 library root'",
            "Resolve-CanonicalNonReparsePath (Join-Path $sdkLibRoot 'ucrt/x64') 'Windows SDK UCRT x64 root'",
            "Resolve-CanonicalNonReparsePath (Join-Path $sdkLibRoot 'um/x64') 'Windows SDK UM x64 root'",
            "Resolve-CanonicalNonReparsePath $Path 'import library'",
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER",
            "$exactLibraryPath",
            "'-Wl,/Brepro,/nodefaultlib'",
            "cargo run --quiet --locked --offline",
            "cargo build --quiet --locked --offline --release",
            "Build-Once -Label 'first'",
            "Build-Once -Label 'second'",
            "Assert-ByteEqual",
            "$peOffset = [long](Read-U32 $bytes 0x3c)",
            "$bytes[[int]$peOffset] -ne 0x50",
            "0x8664",
            "0x20b",
            "IMAGE_SUBSYSTEM_WINDOWS_CUI",
            "Get-PeImports",
            "Get-PeExports",
            "Assert-SystemImportAllowlist",
            "Assert-ExternalManifestIsEffective",
            "Test-PeHasManifestResource",
            "CreateActCtx",
            "ActivateActCtx",
            "Assert-ExactInventory",
            "Assert-NoEmbeddedPath",
            "$llvmReadObjPath --file-headers --coff-imports --coff-exports --coff-resources",
            "SEMAPRAX_DESKTOP_V3_OK platform=windows",
        ],
    )?;
    require_ordered(
        source,
        &[
            "Build-Once -Label 'first'",
            "Build-Once -Label 'second'",
            "Assert-ByteEqual",
            "Assert-ExternalManifestIsEffective",
        ],
    )
}

fn require_all(label: &str, source: &str, needles: &[&str]) -> Result<(), String> {
    for needle in needles {
        if !source.contains(needle) {
            return Err(format!("missing {label} lock: {needle}"));
        }
    }
    Ok(())
}

fn require_ordered(source: &str, needles: &[&str]) -> Result<(), String> {
    let mut offset = 0;
    for needle in needles {
        let relative = source[offset..]
            .find(needle)
            .ok_or_else(|| format!("missing ordered lock: {needle}"))?;
        offset += relative + needle.len();
    }
    Ok(())
}

fn assert_contains_all(label: &str, source: &str, needles: &[&str]) {
    require_all(label, source, needles).unwrap_or_else(|error| panic!("{error}"));
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap_or_else(|error| panic!("read {path}: {error}"))
}
