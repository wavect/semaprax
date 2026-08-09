param(
  [Parameter(Mandatory = $true)][string]$OutputRoot,
  [Parameter(Mandatory = $true)][string]$EngineRoot
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$lockPath = Join-Path $PSScriptRoot 'toolchain.lock'
. (Join-Path $PSScriptRoot 'inspect-windows-package.ps1')

if (-not [System.IO.Path]::IsPathFullyQualified($OutputRoot)) { throw 'output directory must be absolute' }
if (-not [System.IO.Path]::IsPathFullyQualified($EngineRoot)) { throw 'engine package must be absolute' }
if (Test-Path -LiteralPath $OutputRoot) { throw 'output directory must not already exist' }
if (-not (Test-Path -LiteralPath (Join-Path $EngineRoot 'SemapraxPrivate') -PathType Container)) { throw 'engine package is missing' }
if (-not [System.Runtime.InteropServices.RuntimeInformation]::IsOSPlatform([System.Runtime.InteropServices.OSPlatform]::Windows)) { throw 'Windows UI package gate must run on Windows' }
if ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture -ne [System.Runtime.InteropServices.Architecture]::X64) { throw 'Windows UI package gate requires an x86_64 runner' }

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
    (Lock 'windows.host') -ne 'x86_64' -or
    (Lock 'network') -ne 'forbidden-cargo-offline') {
  throw 'Windows UI toolchain contract changed'
}

$llvmBin = Resolve-CanonicalNonReparsePath (Join-Path ${env:ProgramFiles} 'LLVM/bin') 'LLVM root'
$clangPath = Resolve-CanonicalNonReparsePath (Get-Command clang.exe -CommandType Application -ErrorAction Stop).Source 'clang.exe'
$llvmReadObjPath = Resolve-CanonicalNonReparsePath (Get-Command llvm-readobj.exe -CommandType Application -ErrorAction Stop).Source 'llvm-readobj.exe'
$lldLinkPath = Resolve-CanonicalNonReparsePath (Get-Command lld-link.exe -CommandType Application -ErrorAction Stop).Source 'lld-link.exe'
foreach ($tool in @($clangPath, $llvmReadObjPath, $lldLinkPath)) {
  if ([System.IO.Path]::GetDirectoryName($tool) -ne $llvmBin) { throw "LLVM UI tool escaped its exact distribution: $tool" }
}
$clangVersion = Lock 'windows.clang.version'
$clangOutput = ((& $clangPath --version) -join "`n").Trim()
$lldOutput = ((& $lldLinkPath --version) -join "`n").Trim()
if ($LASTEXITCODE -ne 0 -or
    $clangOutput -notmatch "(?m)^clang version $([regex]::Escape($clangVersion))(?:\s|$)" -or
    $lldOutput -notmatch "\b$([regex]::Escape($clangVersion))\b") {
  throw "Windows UI LLVM pin mismatch:`n$clangOutput`n$lldOutput"
}

$vswherePath = Resolve-CanonicalNonReparsePath (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe') 'vswhere.exe'
$vswhereVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($vswherePath).FileVersion
if ($vswhereVersion -ne (Lock 'windows.vswhere.version')) {
  throw "Windows UI vswhere pin mismatch: $vswhereVersion"
}
$visualStudioRoot = ((& $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath) -join '').Trim()
$visualStudioVersion = ((& $vswherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion) -join '').Trim()
if ($LASTEXITCODE -ne 0 -or $visualStudioRoot.Length -eq 0 -or $visualStudioVersion -ne (Lock 'windows.visual-studio.version')) {
  throw "Windows UI Visual Studio pin mismatch: root='$visualStudioRoot' version='$visualStudioVersion'"
}
$visualStudioRoot = Resolve-CanonicalNonReparsePath $visualStudioRoot 'Visual Studio root'
$vcToolsVersionFile = Resolve-CanonicalNonReparsePath (Join-Path $visualStudioRoot 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt') 'MSVC tools version file'
$vcToolsVersion = (Get-Content -LiteralPath $vcToolsVersionFile -Raw).Trim()
if ($vcToolsVersion -ne (Lock 'windows.msvc.tools.version')) { throw "Windows UI MSVC tools pin mismatch: $vcToolsVersion" }
$vcToolsRoot = Resolve-CanonicalNonReparsePath (Join-Path $visualStudioRoot "VC/Tools/MSVC/$vcToolsVersion") 'MSVC tools root'
$kitsRegistry = 'Registry::HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows Kits\Installed Roots'
$kitsRoot = Resolve-CanonicalNonReparsePath (Get-ItemPropertyValue -LiteralPath $kitsRegistry -Name KitsRoot10) 'Windows Kits root'
$sdkVersion = Lock 'windows.sdk.version'
$sdkLibRoot = Resolve-CanonicalNonReparsePath (Join-Path $kitsRoot "Lib/$sdkVersion") 'Windows SDK library root'
$vcLibRoot = Resolve-CanonicalNonReparsePath (Join-Path $vcToolsRoot 'lib/x64') 'MSVC x64 library root'
$sdkUcrtLibRoot = Resolve-CanonicalNonReparsePath (Join-Path $sdkLibRoot 'ucrt/x64') 'Windows SDK UCRT x64 root'
$sdkUmLibRoot = Resolve-CanonicalNonReparsePath (Join-Path $sdkLibRoot 'um/x64') 'Windows SDK UM x64 root'

function Exact-Library([string]$Name, [string]$Root) {
  $path = Resolve-CanonicalNonReparsePath (Join-Path $Root $Name) "Windows UI import library $Name"
  if ([System.IO.Path]::GetDirectoryName($path) -ne $Root) { throw "Windows UI library escaped its exact root: $path" }
  $bytes = [System.IO.File]::ReadAllBytes($path)
  if ($bytes.Length -lt 8 -or [System.Text.Encoding]::ASCII.GetString($bytes, 0, 8) -ne "!<arch>`n") {
    throw "Windows UI import library is not a COFF archive: $path"
  }
  return $path
}

$uiLibraryNames = @((Lock 'windows.ui.libraries').Split(','))
if (($uiLibraryNames -join ',') -ne 'libcmt.lib,libvcruntime.lib,libucrt.lib,oldnames.lib,ucrt.lib,kernel32.lib,user32.lib,oleacc.lib,ole32.lib,oleaut32.lib,shell32.lib,uuid.lib,bcrypt.lib') {
  throw 'Windows UI import-library contract changed'
}
$uiLibraries = @(
  Exact-Library 'libcmt.lib' $vcLibRoot
  Exact-Library 'libvcruntime.lib' $vcLibRoot
  Exact-Library 'libucrt.lib' $vcLibRoot
  Exact-Library 'oldnames.lib' $vcLibRoot
  Exact-Library 'ucrt.lib' $sdkUcrtLibRoot
  Exact-Library 'kernel32.lib' $sdkUmLibRoot
  Exact-Library 'user32.lib' $sdkUmLibRoot
  Exact-Library 'oleacc.lib' $sdkUmLibRoot
  Exact-Library 'ole32.lib' $sdkUmLibRoot
  Exact-Library 'oleaut32.lib' $sdkUmLibRoot
  Exact-Library 'shell32.lib' $sdkUmLibRoot
  Exact-Library 'uuid.lib' $sdkUmLibRoot
  Exact-Library 'bcrypt.lib' $sdkUmLibRoot
)

$scratch = Join-Path ([System.IO.Path]::GetTempPath()) ("semaprax-desktop-ui-v1-" + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$EngineRoot = Resolve-CanonicalNonReparsePath $EngineRoot 'private Windows engine package root'

$uiSource = Join-Path $PSScriptRoot 'ui-windows.c'
function Build-Ui([string]$BuildRoot) {
  New-Item -ItemType Directory -Path $BuildRoot | Out-Null
  $destination = Join-Path $BuildRoot 'SemapraxPrivate.exe'
  $arguments = @('-std=c11', '-pedantic-errors', '-Wall', '-Wextra', '-Werror',
    '-O2', '-municode', "--ld-path=$lldLinkPath",
    '-Wl,/Brepro,/nodefaultlib,/subsystem:windows', $uiSource) +
    $uiLibraries + @('-o', $destination)
  & $clangPath @arguments
  if ($LASTEXITCODE -ne 0) { throw 'private Windows native UI compilation failed' }
  return $destination
}
$firstUi = Build-Ui (Join-Path $scratch 'ui-first')
$secondUi = Build-Ui (Join-Path $scratch 'ui-second')
if ((Get-FileHash -LiteralPath $firstUi -Algorithm SHA256).Hash -ne
    (Get-FileHash -LiteralPath $secondUi -Algorithm SHA256).Hash) {
  throw 'private Windows native UI executable is not byte-reproducible'
}

$app = Join-Path $OutputRoot 'SemapraxPrivateUI'
New-Item -ItemType Directory -Path $app | Out-Null
$engineApp = Join-Path $EngineRoot 'SemapraxPrivate'
$engineExecutable = Resolve-CanonicalNonReparsePath (Join-Path $engineApp 'SemapraxPrivate.exe') 'private Windows engine executable'
$engineProvider = Resolve-CanonicalNonReparsePath (Join-Path $engineApp 'SemapraxPrivateProvider.dll') 'private Windows engine provider'
$engineDescriptor = Resolve-CanonicalNonReparsePath (Join-Path $engineApp 'SemapraxPrivateProvider.spxnabi3') 'private Windows engine descriptor'
Copy-Item -LiteralPath $firstUi -Destination (Join-Path $app 'SemapraxPrivate.exe')
Copy-Item -LiteralPath $engineExecutable -Destination (Join-Path $app 'SemapraxPrivateEngine.exe')
Copy-Item -LiteralPath $engineProvider -Destination (Join-Path $app 'SemapraxPrivateProvider.dll')
Copy-Item -LiteralPath $engineDescriptor -Destination (Join-Path $app 'SemapraxPrivateProvider.spxnabi3')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'private-desktop-v3-app.exe.manifest') -Destination (Join-Path $app 'SemapraxPrivate.exe.manifest')
$packagedEngine = Join-Path $app 'SemapraxPrivateEngine.exe'
$engineDigest = (Get-FileHash -LiteralPath $packagedEngine -Algorithm SHA256).Hash.ToLowerInvariant()
if ($engineDigest -notmatch '^[0-9a-f]{64}$') { throw 'private Windows UI engine digest is not canonical SHA-256' }
$engineDigestManifest = Join-Path $app 'SemapraxPrivateEngine.sha256'
$manifestEncoding = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText(
  $engineDigestManifest,
  "semaprax.private-desktop-engine-sha256.v1 $engineDigest`n",
  $manifestEncoding
)

$expectedInventory = @('SemapraxPrivate.exe', 'SemapraxPrivate.exe.manifest', 'SemapraxPrivateEngine.exe', 'SemapraxPrivateEngine.sha256', 'SemapraxPrivateProvider.dll', 'SemapraxPrivateProvider.spxnabi3')
Assert-ExactInventory $app $expectedInventory
$ui = Join-Path $app 'SemapraxPrivate.exe'
$manifest = Join-Path $app 'SemapraxPrivate.exe.manifest'
$image = Read-PeImage $ui
if ($image.Signature -ne 0x00004550 -or $image.Machine -ne 0x8664 -or
    $image.OptionalMagic -ne 0x020b -or $image.Subsystem -ne 2 -or
    ($image.Characteristics -band 0x0002) -eq 0 -or
    ($image.Characteristics -band 0x2000) -ne 0 -or $image.EntryPoint -eq 0) {
  throw 'private Windows native UI is not an x64 PE32+ GUI executable'
}
$imports = @(Get-PeImports $image)
$expectedImports = @('bcrypt.dll', 'kernel32.dll', 'ole32.dll', 'oleacc.dll', 'oleaut32.dll', 'shell32.dll', 'user32.dll')
Assert-SequenceEqual 'private Windows native UI imports' $imports $expectedImports
$exportDirectory = $image.Directories[0]
$exports = Get-PeExports $image
if ($exportDirectory.Rva -ne 0 -or $exportDirectory.Size -ne 0 -or
    $exports.FunctionCount -ne 0 -or $exports.ModuleName.Length -ne 0 -or
    @($exports.Names).Count -ne 0) {
  throw 'private Windows native UI must have no export directory, named exports, or ordinal-only exports'
}
if (Test-PeHasManifestResource $image) { throw 'private Windows native UI unexpectedly embeds a manifest' }
if ($null -eq (Get-EffectiveManifest $ui $manifest)) { throw 'private Windows native UI external manifest was ineffective' }
$ignored = @(& $llvmReadObjPath --file-headers --coff-imports --coff-exports --coff-resources -- $ui)
if ($LASTEXITCODE -ne 0 -or $ignored.Count -eq 0) { throw 'pinned llvm-readobj rejected private Windows native UI' }
Assert-NoEmbeddedPath $ui @($repo, $scratch, $OutputRoot)

function Start-UiProcess([string]$Executable, [string]$ResultPath) {
  $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
  $startInfo.FileName = $Executable
  $startInfo.UseShellExecute = $false
  $startInfo.ArgumentList.Add($ResultPath)
  $started = [System.Diagnostics.Process]::Start($startInfo)
  if ($null -eq $started) { throw 'private Windows native UI process did not start' }
  return $started
}

function Wait-UiProcess($Process, [string]$Label) {
  if (-not $Process.WaitForExit(60000)) {
    $Process.Kill($true)
    throw "$Label did not terminate after its lifecycle deadline"
  }
}

$mismatchApp = Join-Path $scratch 'mismatch-engine-package'
Copy-Item -LiteralPath $app -Destination $mismatchApp -Recurse
$mismatchEngine = Join-Path $mismatchApp 'SemapraxPrivateEngine.exe'
$append = [System.IO.File]::Open($mismatchEngine, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Write, [System.IO.FileShare]::Read)
try {
  if ($append.Seek(0, [System.IO.SeekOrigin]::End) -lt 1) { throw 'private Windows hostile engine was unexpectedly empty' }
  $append.WriteByte(0)
  $append.Flush($true)
} finally {
  $append.Dispose()
}
$mismatchResult = Join-Path $scratch 'mismatch-result.txt'
$mismatchProcess = Start-UiProcess (Join-Path $mismatchApp 'SemapraxPrivate.exe') $mismatchResult
Wait-UiProcess $mismatchProcess 'hostile private Windows native UI'
if ($mismatchProcess.ExitCode -eq 0 -or (Test-Path -LiteralPath $mismatchResult)) {
  throw 'digest-mismatched private Windows engine was not rejected before result publication'
}

$result = Join-Path $scratch 'ui-result.txt'
$process = Start-UiProcess $ui $result
Wait-UiProcess $process 'private Windows native UI'
if ($process.ExitCode -ne 0) { throw "private Windows native UI failed with exit code $($process.ExitCode)" }
$expected = "SEMAPRAX_DESKTOP_UI_V1_OK platform=windows lifecycle=create,window,shown,control,close,terminate accessibility=button-name engine=calls-2-replay-exact`n"
if (-not (Test-Path -LiteralPath $result -PathType Leaf) -or
    [System.IO.File]::ReadAllText($result) -cne $expected) {
  throw 'unexpected packaged Windows native UI result'
}
Write-Output $expected.TrimEnd("`n")
