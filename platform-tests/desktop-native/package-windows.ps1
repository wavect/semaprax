param([Parameter(Mandatory = $true)][string]$OutputRoot)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$lockPath = Join-Path $PSScriptRoot 'toolchain.lock'
. (Join-Path $PSScriptRoot 'inspect-windows-package.ps1')

if (-not [System.IO.Path]::IsPathFullyQualified($OutputRoot)) { throw 'output directory must be absolute' }
if (Test-Path -LiteralPath $OutputRoot) { throw 'output directory must not already exist' }
if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) { throw 'Windows package gate must run on Windows' }
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne [System.Runtime.InteropServices.Architecture]::X64) { throw 'Windows package gate requires an x86_64 runner' }

$lock = @{}
foreach ($line in (Get-Content -LiteralPath $lockPath)) {
  if ($line.Length -eq 0) { continue }
  if ($line -notmatch '^([a-z0-9.-]+)=([^\s=]+)$') { throw "invalid toolchain lock line: $line" }
  if ($lock.ContainsKey($Matches[1])) { throw "duplicate toolchain lock key: $($Matches[1])" }
  $lock[$Matches[1]] = $Matches[2]
}
function Lock([string]$Name) {
  if (-not $lock.ContainsKey($Name)) { throw "toolchain lock is missing $Name" }
  return [string]$lock[$Name]
}

function Resolve-CanonicalNonReparsePath([string]$Path, [string]$Label) {
  if (-not [System.IO.Path]::IsPathFullyQualified($Path)) { throw "$Label path is not fully qualified: $Path" }
  $full = [System.IO.Path]::GetFullPath($Path)
  $root = [System.IO.Path]::GetPathRoot($full)
  if ([string]::IsNullOrEmpty($root)) { throw "$Label path has no volume or share root: $full" }
  $current = $root
  $components = @('') + @($full.Substring($root.Length) -split '[\\/]' | Where-Object { $_.Length -gt 0 })
  foreach ($component in $components) {
    if ($component.Length -gt 0) { $current = Join-Path $current $component }
    if (-not (Test-Path -LiteralPath $current)) { throw "$Label path component is missing: $current" }
    $item = Get-Item -LiteralPath $current -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw "$Label path contains a reparse point: $current"
    }
  }
  return (Resolve-Path -LiteralPath $full).Path
}

if ((Lock 'schema') -ne 'semaprax.private-desktop-toolchain.v1' -or
    (Lock 'network') -ne 'forbidden-cargo-offline' -or
    (Lock 'reproducibility') -ne 'two-independent-target-directories-byte-equal' -or
    (Lock 'windows.host') -ne 'x86_64' -or
    (Lock 'windows.package') -ne 'portable-PE-with-external-manifest') {
  throw 'Windows desktop toolchain contract changed'
}

$rustVersion = Lock 'rust.version'
$rustCommit = Lock 'rust.commit'
$expectedRustRelease = 'rustc 1.97.1 (8bab26f4f 2026-07-14)'
$expectedRustLlvm = 'LLVM version: 22.1.6'
if ($rustCommit -notmatch '^([0-9a-f]{9})-([0-9]{4}-[0-9]{2}-[0-9]{2})$') { throw 'invalid pinned Rust commit contract' }
$expectedRustLine = "rustc $rustVersion ($($Matches[1]) $($Matches[2]))"
if ($expectedRustLine -ne $expectedRustRelease -or "LLVM version: $(Lock 'rust.llvm')" -ne $expectedRustLlvm) { throw 'checked-in Rust pin and toolchain lock diverged' }
$actualRustLine = ((& rustc --version) -join "`n").Trim()
if ($LASTEXITCODE -ne 0 -or $actualRustLine -ne $expectedRustLine) { throw "Rust pin mismatch: '$actualRustLine' != '$expectedRustLine'" }
$rustVerbose = ((& rustc -vV) -join "`n").Trim()
if ($LASTEXITCODE -ne 0 -or $rustVerbose -notmatch '(?m)^host: x86_64-pc-windows-msvc$' -or
    $rustVerbose -notmatch "(?m)^LLVM version: $([regex]::Escape((Lock 'rust.llvm')))$") {
  throw "Rust host/LLVM pin mismatch:`n$rustVerbose"
}

$llvmBin = Resolve-CanonicalNonReparsePath (Join-Path ${env:ProgramFiles} 'LLVM/bin') 'LLVM root'
$clangPath = Resolve-CanonicalNonReparsePath (Get-Command clang.exe -CommandType Application -ErrorAction Stop).Source 'clang.exe'
$llvmReadObjPath = Resolve-CanonicalNonReparsePath (Get-Command llvm-readobj.exe -CommandType Application -ErrorAction Stop).Source 'llvm-readobj.exe'
$lldLinkPath = Resolve-CanonicalNonReparsePath (Get-Command lld-link.exe -CommandType Application -ErrorAction Stop).Source 'lld-link.exe'
foreach ($tool in @($clangPath, $llvmReadObjPath, $lldLinkPath)) {
  if ([System.IO.Path]::GetDirectoryName($tool) -ne $llvmBin) {
    throw "LLVM tool did not resolve from the exact pinned distribution: $tool"
  }
}

$clangVersion = Lock 'windows.clang.version'
$expectedClangRelease = 'clang version 20.1.8'
if ("clang version $clangVersion" -ne $expectedClangRelease) { throw 'checked-in Clang pin and toolchain lock diverged' }
$clangOutput = ((& $clangPath --version) -join "`n").Trim()
if ($LASTEXITCODE -ne 0 -or $clangOutput -notmatch "(?m)^clang version $([regex]::Escape($clangVersion))(?:\s|$)") { throw "Clang pin mismatch:`n$clangOutput" }
$clangTarget = ((& $clangPath -dumpmachine) -join "`n").Trim()
if ($LASTEXITCODE -ne 0 -or $clangTarget -ne 'x86_64-pc-windows-msvc') { throw "Clang target mismatch: $clangTarget" }
$llvmOutput = ((& $llvmReadObjPath --version) -join "`n").Trim()
if ($LASTEXITCODE -ne 0 -or $llvmOutput -notmatch "(?m)^\s*LLVM version $([regex]::Escape($clangVersion))\s*$") { throw "llvm-readobj pin mismatch:`n$llvmOutput" }
$lldOutput = ((& $lldLinkPath --version) -join "`n").Trim()
if ($LASTEXITCODE -ne 0 -or $lldOutput -notmatch "\b$([regex]::Escape($clangVersion))\b") { throw "lld-link pin mismatch: $lldOutput" }

$vswherePath = Resolve-CanonicalNonReparsePath (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe') 'vswhere.exe'
if (-not (Test-Path -LiteralPath $vswherePath -PathType Leaf)) { throw 'the pinned Visual Studio locator is missing' }
$vswhereVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($vswherePath).FileVersion
if ($vswhereVersion -ne (Lock 'windows.vswhere.version')) {
  throw "vswhere pin mismatch: $vswhereVersion"
}
$visualStudioRoot = ((& $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath) -join '').Trim()
$visualStudioVersion = ((& $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion) -join '').Trim()
if ($LASTEXITCODE -ne 0 -or $visualStudioRoot.Length -eq 0 -or $visualStudioVersion -ne (Lock 'windows.visual-studio.version')) {
  throw "Visual Studio pin mismatch: root='$visualStudioRoot' version='$visualStudioVersion'"
}
$visualStudioRoot = Resolve-CanonicalNonReparsePath $visualStudioRoot 'Visual Studio root'
$vcToolsVersionFile = Resolve-CanonicalNonReparsePath (Join-Path $visualStudioRoot 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt') 'MSVC tools version file'
$vcToolsVersion = (Get-Content -LiteralPath $vcToolsVersionFile -Raw).Trim()
if ($vcToolsVersion -ne (Lock 'windows.msvc.tools.version')) { throw "MSVC tools pin mismatch: $vcToolsVersion" }
$vcToolsRoot = Resolve-CanonicalNonReparsePath (Join-Path $visualStudioRoot "VC/Tools/MSVC/$vcToolsVersion") 'MSVC tools root'
$linkExe = Resolve-CanonicalNonReparsePath (Join-Path $vcToolsRoot 'bin/Hostx64/x64/link.exe') 'link.exe'
$linkVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($linkExe).FileVersion
if ($linkVersion -ne (Lock 'windows.link.version')) { throw "link.exe identity mismatch: $linkVersion" }
$linkOutput = ((& $linkExe '/?') -join "`n").Trim()
if ($LASTEXITCODE -ne 0 -or $linkOutput -notmatch "(?m)^Microsoft \(R\) Incremental Linker Version $([regex]::Escape($linkVersion))") {
  throw "link.exe runtime version mismatch:`n$linkOutput"
}

$kitsRegistry = 'Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows Kits\Installed Roots'
$kitsRoot = Resolve-CanonicalNonReparsePath (Get-ItemPropertyValue -LiteralPath $kitsRegistry -Name KitsRoot10) 'Windows Kits root'
$sdkVersion = Lock 'windows.sdk.version'
$sdkLibRoot = Resolve-CanonicalNonReparsePath (Join-Path $kitsRoot "Lib/$sdkVersion") 'Windows SDK library root'
$vcLibRoot = Resolve-CanonicalNonReparsePath (Join-Path $vcToolsRoot 'lib/x64') 'MSVC x64 library root'
$sdkUcrtLibRoot = Resolve-CanonicalNonReparsePath (Join-Path $sdkLibRoot 'ucrt/x64') 'Windows SDK UCRT x64 root'
$sdkUmLibRoot = Resolve-CanonicalNonReparsePath (Join-Path $sdkLibRoot 'um/x64') 'Windows SDK UM x64 root'

function Assert-ExactLibrary([string]$Path, [string]$ExpectedRoot) {
  $resolved = Resolve-CanonicalNonReparsePath $Path 'import library'
  $rootPrefix = $ExpectedRoot.TrimEnd([System.IO.Path]::DirectorySeparatorChar) + [System.IO.Path]::DirectorySeparatorChar
  if (-not $resolved.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase) -or
      [System.IO.Path]::GetDirectoryName($resolved) -ne $ExpectedRoot) {
    throw "import library escaped its exact pinned root: $resolved"
  }
  $header = [System.IO.File]::ReadAllBytes($resolved)[0..7]
  if ([System.Text.Encoding]::ASCII.GetString($header) -ne "!<arch>`n") { throw "import library is not a COFF archive: $resolved" }
  return $resolved
}

$providerLibraryNames = @((Lock 'windows.provider.libraries').Split(','))
if (($providerLibraryNames -join ',') -ne 'libcmt.lib,libvcruntime.lib,libucrt.lib,oldnames.lib,ucrt.lib,kernel32.lib') {
  throw 'provider import-library contract changed'
}
$providerLibraries = @(
  Assert-ExactLibrary (Join-Path $vcLibRoot 'libcmt.lib') $vcLibRoot
  Assert-ExactLibrary (Join-Path $vcLibRoot 'libvcruntime.lib') $vcLibRoot
  Assert-ExactLibrary (Join-Path $vcLibRoot 'libucrt.lib') $vcLibRoot
  Assert-ExactLibrary (Join-Path $vcLibRoot 'oldnames.lib') $vcLibRoot
  Assert-ExactLibrary (Join-Path $sdkUcrtLibRoot 'ucrt.lib') $sdkUcrtLibRoot
  Assert-ExactLibrary (Join-Path $sdkUmLibRoot 'kernel32.lib') $sdkUmLibRoot
)
$exactLibraryPath = @($vcLibRoot, $sdkUcrtLibRoot, $sdkUmLibRoot) -join ';'

$scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("semaprax-desktop-v3-" + [guid]::NewGuid().ToString('N'))
$manifestSource = Join-Path $repo 'platform-tests/desktop-native/private-desktop-v3-app.exe.manifest'
$app = Join-Path $OutputRoot 'SemapraxPrivate'
New-Item -ItemType Directory -Path $scratch | Out-Null

function Build-Once([Parameter(Mandatory = $true)][string]$Label, [Parameter(Mandatory = $true)][string]$BuildRoot) {
  New-Item -ItemType Directory -Path $BuildRoot | Out-Null
  $sourceFile = Join-Path $BuildRoot 'provider.c'
  $descriptorFile = Join-Path $BuildRoot 'SemapraxPrivateProvider.spxnabi3'
  $providerFile = Join-Path $BuildRoot 'SemapraxPrivateProvider.dll'
  $targetDirectory = Join-Path $BuildRoot 'cargo-target'
  $previousTargetDirectory = [System.Environment]::GetEnvironmentVariable('CARGO_TARGET_DIR', 'Process')
  $previousTargetLinker = [System.Environment]::GetEnvironmentVariable('CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER', 'Process')
  $previousLibraryPath = [System.Environment]::GetEnvironmentVariable('LIB', 'Process')
  try {
    [System.Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $targetDirectory, 'Process')
    [System.Environment]::SetEnvironmentVariable('CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER', $linkExe, 'Process')
    [System.Environment]::SetEnvironmentVariable('LIB', $exactLibraryPath, 'Process')
    cargo run --quiet --locked --offline -p semaprax-native-host --features unstable-desktop-app-harness --bin private-desktop-v3-fixture -- $sourceFile $descriptorFile
    if ($LASTEXITCODE -ne 0) { throw "$Label desktop fixture emission failed" }
    & $clangPath -std=c11 -pedantic-errors -Wall -Wextra -Werror -O2 "--ld-path=$lldLinkPath" '-Wl,/Brepro,/nodefaultlib' -shared $sourceFile @providerLibraries -o $providerFile
    if ($LASTEXITCODE -ne 0) { throw "$Label desktop provider compilation failed" }
    cargo build --quiet --locked --offline --release -p semaprax-native-host --features unstable-desktop-app-harness --bin private-desktop-v3-app
    if ($LASTEXITCODE -ne 0) { throw "$Label desktop application build failed" }
  } finally {
    [System.Environment]::SetEnvironmentVariable('CARGO_TARGET_DIR', $previousTargetDirectory, 'Process')
    [System.Environment]::SetEnvironmentVariable('CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER', $previousTargetLinker, 'Process')
    [System.Environment]::SetEnvironmentVariable('LIB', $previousLibraryPath, 'Process')
  }
  $executableFile = Join-Path $targetDirectory 'release/private-desktop-v3-app.exe'
  foreach ($artifact in @($sourceFile, $descriptorFile, $providerFile, $executableFile)) {
    if (-not (Test-Path -LiteralPath $artifact -PathType Leaf)) { throw "independent build omitted $artifact" }
  }
  return [pscustomobject]@{
    Root = $BuildRoot
    Source = $sourceFile
    Descriptor = $descriptorFile
    Provider = $providerFile
    Executable = $executableFile
  }
}

function Provider-Exports([string]$Source) {
  $text = Get-Content -LiteralPath $Source -Raw
  $names = @([regex]::Matches($text, '\b(spx_[0-9a-f]{48}_(?:descriptor|execute|settle)_v3)\s*\(') | ForEach-Object { $_.Groups[1].Value } | Sort-Object -Unique)
  if ($names.Count -ne 3 -or @($names | Where-Object { $_ -match '_descriptor_v3$' }).Count -ne 1 -or
      @($names | Where-Object { $_ -match '_execute_v3$' }).Count -ne 1 -or
      @($names | Where-Object { $_ -match '_settle_v3$' }).Count -ne 1) {
    throw "generated provider does not contain exactly three callable-v3 exports: $($names -join ',')"
  }
  return $names
}

function Assert-ByteEqual([string]$Label, [string]$First, [string]$Second) {
  $firstHash = (Get-FileHash -LiteralPath $First -Algorithm SHA256).Hash
  $secondHash = (Get-FileHash -LiteralPath $Second -Algorithm SHA256).Hash
  if ($firstHash -ne $secondHash) { throw "$Label is not reproducible: $firstHash != $secondHash" }
}

function Invoke-LlvmReadObj([string]$Path) {
  $ignored = @(& $llvmReadObjPath --file-headers --coff-imports --coff-exports --coff-resources -- $Path)
  if ($LASTEXITCODE -ne 0 -or $ignored.Count -eq 0) { throw "pinned llvm-readobj rejected $Path" }
}

Push-Location $repo
try {
  $first = Build-Once -Label 'first' -BuildRoot (Join-Path $scratch 'first')
  $second = Build-Once -Label 'second' -BuildRoot (Join-Path $scratch 'second')
  Assert-ByteEqual 'generated provider source' $first.Source $second.Source
  Assert-ByteEqual 'provider descriptor' $first.Descriptor $second.Descriptor
  Assert-ByteEqual 'provider DLL' $first.Provider $second.Provider
  Assert-ByteEqual 'desktop executable' $first.Executable $second.Executable
  $expectedExports = @(Provider-Exports $first.Source)
  Assert-SequenceEqual 'independent provider source exports' @(Provider-Exports $second.Source) $expectedExports

  New-Item -ItemType Directory -Path $app | Out-Null
  Copy-Item -LiteralPath $first.Executable -Destination (Join-Path $app 'SemapraxPrivate.exe')
  Copy-Item -LiteralPath $first.Provider -Destination (Join-Path $app 'SemapraxPrivateProvider.dll')
  Copy-Item -LiteralPath $first.Descriptor -Destination (Join-Path $app 'SemapraxPrivateProvider.spxnabi3')
  Copy-Item -LiteralPath $manifestSource -Destination (Join-Path $app 'SemapraxPrivate.exe.manifest')

  $rootItems = @(Get-ChildItem -LiteralPath $OutputRoot -Force)
  if ($rootItems.Count -ne 1 -or -not $rootItems[0].PSIsContainer -or $rootItems[0].Name -ne 'SemapraxPrivate' -or
      (($rootItems[0].Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)) {
    throw 'Windows package root inventory is not exactly SemapraxPrivate/'
  }
  $expectedInventory = @('SemapraxPrivate.exe', 'SemapraxPrivate.exe.manifest', 'SemapraxPrivateProvider.dll', 'SemapraxPrivateProvider.spxnabi3')
  Assert-ExactInventory $app $expectedInventory

  $packagedExecutable = Join-Path $app 'SemapraxPrivate.exe'
  $packagedProvider = Join-Path $app 'SemapraxPrivateProvider.dll'
  $packagedManifest = Join-Path $app 'SemapraxPrivate.exe.manifest'
  $exeBytes = [System.IO.File]::ReadAllBytes($packagedExecutable)
  $dllBytes = [System.IO.File]::ReadAllBytes($packagedProvider)
  if ($exeBytes.Length -lt 2 -or $exeBytes[0] -ne 0x4d -or $exeBytes[1] -ne 0x5a) { throw 'packaged application is not MZ' }
  if ($dllBytes.Length -lt 2 -or $dllBytes[0] -ne 0x4d -or $dllBytes[1] -ne 0x5a) { throw 'packaged provider is not MZ' }
  $exeContract = Assert-PeContract $packagedExecutable $false @() ''
  $providerContract = Assert-PeContract $packagedProvider $true $expectedExports 'SemapraxPrivateProvider.dll'
  $expectedPeSignature = 0x00004550
  $expectedPeMachine = 0x8664
  $expectedPe32Plus = 0x020b
  if ($exeContract.Image.Signature -ne $expectedPeSignature -or $providerContract.Image.Signature -ne $expectedPeSignature -or
      $exeContract.Image.Machine -ne $expectedPeMachine -or $providerContract.Image.Machine -ne $expectedPeMachine -or
      $exeContract.Image.OptionalMagic -ne $expectedPe32Plus -or $providerContract.Image.OptionalMagic -ne $expectedPe32Plus) {
    throw 'packaged PE/COFF identity changed after structural inspection'
  }
  $inspectedExeImports = @(Get-PeImports $exeContract.Image)
  $inspectedProviderImports = @(Get-PeImports $providerContract.Image)
  $inspectedExeExports = @(Get-PeExports $exeContract.Image)
  $inspectedProviderExports = @(Get-PeExports $providerContract.Image)
  if ($inspectedExeImports.Count -eq 0 -or $inspectedProviderImports.Count -eq 0 -or
      $inspectedExeExports.Names.Count -ne 0 -or $inspectedProviderExports.Names.Count -ne 3) {
    throw 'independent import/export inspection changed'
  }
  if ($providerContract.HasManifest) { throw 'provider DLL unexpectedly embeds an application manifest' }
  $effectiveManifest = Get-EffectiveManifest $packagedExecutable $packagedManifest
  if ($null -eq $effectiveManifest) { throw 'effective manifest inspection returned no document' }
  Invoke-LlvmReadObj $packagedExecutable
  Invoke-LlvmReadObj $packagedProvider
  foreach ($artifact in @($packagedExecutable, $packagedProvider)) {
    Assert-NoEmbeddedPath $artifact @($repo, $scratch, $first.Root, $second.Root, $OutputRoot)
  }

  $actual = @(& $packagedExecutable)
  if ($LASTEXITCODE -ne 0) { throw 'packaged desktop application failed' }
  $expected = 'SEMAPRAX_DESKTOP_V3_OK platform=windows calls=2 owner=0 payloads=41,43 replay=exact'
  if ($actual.Count -ne 1 -or $actual[0] -ne $expected) { throw "unexpected packaged Windows result: $($actual -join ' | ')" }
  Write-Output $actual[0]
} finally {
  Pop-Location
}
