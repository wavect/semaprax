<#
.SYNOPSIS
    Per-user SEMAPRAX installer for Windows x64 (no administrator rights).

.DESCRIPTION
    Resolves one release tag, downloads the Windows x64 archive plus its
    verification material, verifies it BEFORE anything is activated, and
    installs semaprax.exe and semapraxd.exe as one managed unit:

        <InstallDir>\versions\<tag>\      extracted package
        <InstallDir>\bin\                 copies of both executables (swapped as one directory)
        <InstallDir>\install-receipt.json ownership receipt (written last)

    Verification order (all before activation): SHA256SUMS line, release
    manifest artifacts[] entry, attestation file present, then
    `gh attestation verify` when gh is on PATH. A checksum is never reported as
    a signature. Extraction refuses absolute, `..`, drive and symlink members.
    The staged semaprax.exe must answer `version --json` with the chosen version.

    The script never changes the PowerShell execution policy, never needs
    elevation, and never stops other processes. If an installed executable is
    running, the install stops with a close-and-retry message and keeps the old
    installation.

    Concise (the script runs with `irm | iex`, so there are no parameters;
    use the SEMAPRAX_INSTALL_* environment variables below instead):

        powershell -ExecutionPolicy Bypass -c "irm https://github.com/wavect/semaprax/releases/latest/download/install.ps1 | iex"

    Inspectable: download install.ps1, read it, then run
        .\install.ps1 -Version v0.9.0

.PARAMETER Version
    Release tag to install ("v0.9.0" or "0.9.0"). Default: the latest release.
    Env fallback: SEMAPRAX_INSTALL_VERSION.

.PARAMETER InstallDir
    Install location. Default: %LOCALAPPDATA%\Programs\Semaprax.
    Env fallback: SEMAPRAX_INSTALL_DIR.

.PARAMETER NoModifyPath
    Do not add <InstallDir>\bin to the user PATH.
    Env fallback: SEMAPRAX_INSTALL_NO_MODIFY_PATH=1.

.PARAMETER Uninstall
    Remove only what the receipt owns, including this installer's PATH entry.
    Env fallback: SEMAPRAX_INSTALL_UNINSTALL=1.

.PARAMETER RequirePublisherVerification
    Fail unless `gh attestation verify` ran and passed.
    Env fallback: SEMAPRAX_INSTALL_REQUIRE_PUBLISHER_VERIFICATION=1.

    Test-only transport overrides (not for normal use):
      SEMAPRAX_INSTALL_DOWNLOAD_BASE  asset URL = <base>/<tag>/<asset>; file:// or a local directory works.
      SEMAPRAX_INSTALL_LATEST_URL     resolved once; the tag is taken from a trailing /tag/<tag>
                                      (a file:// URL naming an existing file reads the tag from its text).
      With a DOWNLOAD_BASE override and no LATEST_URL override, -Version is required.
      SEMAPRAX_INSTALL_ENV_SUBKEY     HKCU subkey holding the Path value (default Environment); tests only.
#>
[CmdletBinding()]
param(
    [string]$Version,
    [string]$InstallDir,
    [switch]$NoModifyPath,
    [switch]$Uninstall,
    [switch]$RequirePublisherVerification
)

$script:SemapraxRepo = 'wavect/semaprax'
$script:SemapraxDefaultDownloadBase = 'https://github.com/wavect/semaprax/releases/download'
$script:SemapraxDefaultLatestUrl = 'https://github.com/wavect/semaprax/releases/latest'
$script:SemapraxReceiptSchema = 'semaprax.install-receipt.v1'
$script:SemapraxManifestSchema = 'semaprax.release-manifest.v1'
$script:SemapraxReceiptName = 'install-receipt.json'
# Must equal the x86_64-pc-windows-msvc entry of ARCHIVE_TARGETS in scripts/release-reconcile.py.
$script:SemapraxTargets = @(
    @{ Target = 'x86_64-pc-windows-msvc'; Extension = 'zip' }
)

# ---------------------------------------------------------------- utilities

function Stop-SemapraxInstall {
    param([string]$Message)
    throw (New-Object System.InvalidOperationException $Message)
}

function Write-SemapraxLine {
    param([string]$Text)
    Write-Host $Text
}

function Test-SemapraxEnvFlag {
    param([string]$Name)
    $v = [Environment]::GetEnvironmentVariable($Name)
    if ([string]::IsNullOrEmpty($v)) { return $false }
    return ($v -match '^(1|true|yes|on)$')
}

function Get-SemapraxSourceInstallHelp {
    param([string]$Tag)
    $t = $Tag
    if ([string]::IsNullOrEmpty($t)) { $t = 'vX.Y.Z' }
    $lines = @(
        'Source-install alternative (needs a Rust toolchain):',
        "  cargo install --locked --git https://github.com/$($script:SemapraxRepo) --tag $t semaprax",
        '    installs the STANDALONE semaprax plus semapraxd.',
        "  cargo install --locked --git https://github.com/$($script:SemapraxRepo) --tag $t semaprax-toolchain --bin semaprax-full",
        '    installs the FULL build, which the archive ships as semaprax, under the name semaprax-full.'
    )
    return ($lines -join "`n")
}

function ConvertTo-SemapraxTag {
    param([string]$Value)
    $v = ''
    if ($null -ne $Value) { $v = $Value.Trim() }
    if ($v -notmatch '^v?\d+\.\d+\.\d+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$') {
        Stop-SemapraxInstall "invalid version '$Value': expected a release tag such as v0.9.0"
    }
    if (-not $v.StartsWith('v')) { $v = 'v' + $v }
    return $v
}

function ConvertTo-SemapraxArgString {
    param([string[]]$Arguments)
    $parts = @()
    foreach ($a in $Arguments) {
        if ($null -eq $a -or $a -eq '') {
            $parts += '""'
        } elseif ($a -notmatch '[\s"]') {
            $parts += $a
        } else {
            $s = $a -replace '(\\*)"', '$1$1\"'
            $s = $s -replace '(\\+)$', '$1$1'
            $parts += ('"' + $s + '"')
        }
    }
    return ($parts -join ' ')
}

function Invoke-SemapraxNative {
    param(
        [string]$FilePath,
        [string[]]$Arguments = @(),
        [int]$TimeoutSeconds = 120
    )
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = ConvertTo-SemapraxArgString $Arguments
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $proc = $null
    try {
        $proc = [System.Diagnostics.Process]::Start($psi)
    } catch {
        return [pscustomobject]@{ ExitCode = -1; Output = ''; Error = $_.Exception.Message; TimedOut = $false; Started = $false }
    }
    $outTask = $proc.StandardOutput.ReadToEndAsync()
    $errTask = $proc.StandardError.ReadToEndAsync()
    if (-not $proc.WaitForExit($TimeoutSeconds * 1000)) {
        try { $proc.Kill() } catch { }
        return [pscustomobject]@{ ExitCode = -1; Output = ''; Error = 'timed out'; TimedOut = $true; Started = $true }
    }
    $proc.WaitForExit()
    return [pscustomobject]@{ ExitCode = $proc.ExitCode; Output = $outTask.Result; Error = $errTask.Result; TimedOut = $false; Started = $true }
}

# ------------------------------------------------- architecture and target

function Initialize-SemapraxNative {
    if ('SemapraxNative' -as [type]) { return }
    $source = @'
using System;
using System.Runtime.InteropServices;
public static class SemapraxNative {
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool IsWow64Process2(IntPtr process, out ushort processMachine, out ushort nativeMachine);
    [DllImport("kernel32.dll")]
    public static extern IntPtr GetCurrentProcess();
    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint flags, uint timeout, out UIntPtr result);
}
'@
    Add-Type -TypeDefinition $source
}

function Get-SemapraxNativeMachine {
    # IsWow64Process2 (Windows 10 1709+) reports the real machine even for an
    # emulated x64 process on ARM64. Returns $null when unavailable.
    try {
        Initialize-SemapraxNative
        $proc = [uint16]0
        $native = [uint16]0
        $ok = [SemapraxNative]::IsWow64Process2([SemapraxNative]::GetCurrentProcess(), [ref]$proc, [ref]$native)
        if (-not $ok) { return $null }
        switch ([int]$native) {
            34404 { return 'AMD64' }
            43620 { return 'ARM64' }
            332 { return 'X86' }
            default { return $null }
        }
    } catch {
        return $null
    }
}

function Get-SemapraxArchInputs {
    $osArch = $null
    try {
        $rt = [System.Runtime.InteropServices.RuntimeInformation]
        if ($null -ne $rt) { $osArch = [string]$rt::OSArchitecture }
    } catch { $osArch = $null }
    return @{
        ProcArch      = $env:PROCESSOR_ARCHITECTURE
        ProcArchW6432 = $env:PROCESSOR_ARCHITEW6432
        OsArch        = $osArch
        NativeMachine = (Get-SemapraxNativeMachine)
    }
}

function ConvertTo-SemapraxArchName {
    param([string]$Value)
    if ([string]::IsNullOrEmpty($Value)) { return $null }
    switch -Regex ($Value.Trim().ToLowerInvariant()) {
        '^(amd64|x64|x86_64)$' { return 'x64' }
        '^(arm64|aarch64)$' { return 'arm64' }
        '^(x86|i386|i686)$' { return 'x86' }
        default { return $Value.Trim().ToLowerInvariant() }
    }
}

function Resolve-SemapraxTarget {
    # Pure: $Arch is a hashtable of the four signals so tests can inject them.
    param([hashtable]$Arch, [string]$Tag)
    $signals = @()
    foreach ($key in @('NativeMachine', 'ProcArchW6432', 'OsArch', 'ProcArch')) {
        $n = ConvertTo-SemapraxArchName ([string]$Arch[$key])
        if ($null -ne $n) { $signals += $n }
    }
    $help = Get-SemapraxSourceInstallHelp -Tag $Tag
    if ($signals -contains 'arm64') {
        Stop-SemapraxInstall ("native Windows ARM64 detected. SEMAPRAX publishes no Windows ARM64 build, and this installer does not assume that x64 emulation works, so it will not install the x64 build here. Tested choices: build from source, or use WSL 2 with the Linux installer.`n" + $help)
    }
    if ($signals.Count -eq 0) {
        Stop-SemapraxInstall ("could not determine the Windows processor architecture.`n" + $help)
    }
    if ($signals[0] -ne 'x64') {
        Stop-SemapraxInstall ("unsupported Windows architecture '$($signals[0])': only x86_64-pc-windows-msvc is published.`n" + $help)
    }
    return 'x86_64-pc-windows-msvc'
}

function Get-SemapraxTargetExtension {
    param([string]$Target)
    foreach ($t in $script:SemapraxTargets) {
        if ($t.Target -ceq $Target) { return $t.Extension }
    }
    Stop-SemapraxInstall "unsupported target $Target"
}

# ---------------------------------------------------------- verification

function Get-SemapraxChecksumFromText {
    param([string]$Text, [string]$Name)
    $found = @()
    foreach ($line in ($Text -split "`r?`n")) {
        if ($line -match '^([0-9A-Fa-f]{64})[ \t]+\*?(.+?)\s*$') {
            if ($Matches[2] -ceq $Name) { $found += $Matches[1].ToLowerInvariant() }
        }
    }
    if ($found.Count -eq 0) { Stop-SemapraxInstall "SHA256SUMS has no entry for $Name" }
    $distinct = @($found | Sort-Object -Unique)
    if ($distinct.Count -gt 1) { Stop-SemapraxInstall "SHA256SUMS lists conflicting digests for $Name" }
    return $distinct[0]
}

function Assert-SemapraxManifestArtifact {
    param($Manifest, [string]$Tag, [string]$Name, [string]$Target, [int64]$Size, [string]$Sha256)
    if ($null -eq $Manifest) { Stop-SemapraxInstall 'release-manifest.json is empty or malformed' }
    if ($Manifest.schema -cne $script:SemapraxManifestSchema) {
        Stop-SemapraxInstall "release-manifest.json has an unexpected schema '$($Manifest.schema)'"
    }
    if ($Manifest.tag -cne $Tag) {
        Stop-SemapraxInstall "release-manifest.json is for tag '$($Manifest.tag)', not $Tag"
    }
    $artifacts = @($Manifest.artifacts)
    $matching = @($artifacts | Where-Object { $null -ne $_ -and $_.name -ceq $Name })
    if ($matching.Count -ne 1) {
        Stop-SemapraxInstall "release-manifest.json must list $Name exactly once (found $($matching.Count))"
    }
    $a = $matching[0]
    if ($a.platform -cne $Target) { Stop-SemapraxInstall "manifest platform for $Name is '$($a.platform)', expected $Target" }
    $manifestSize = 0L
    if (-not [int64]::TryParse([string]$a.size, [ref]$manifestSize) -or $manifestSize -ne $Size) {
        Stop-SemapraxInstall "manifest size for $Name ($($a.size)) does not equal the downloaded size ($Size)"
    }
    if ($a.digest -cne "sha256:$Sha256") {
        Stop-SemapraxInstall "manifest digest for $Name ($($a.digest)) does not equal the archive SHA-256 (sha256:$Sha256)"
    }
}

function Get-SemapraxFileSha256 {
    param([string]$Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Invoke-SemapraxPublisherVerification {
    # Returns 'verified' or 'not-verified'; throws on failure.
    param([string]$ArchivePath, [string]$AttestationPath, [string]$Tag, [bool]$Require)
    $gh = Get-Command gh -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $gh) {
        if ($Require) {
            Stop-SemapraxInstall 'publisher verification was required but gh (GitHub CLI) was not found on PATH'
        }
        Write-SemapraxLine 'publisher: not verified (gh not found) - checksum only'
        return 'not-verified'
    }
    $ghArgs = @(
        'attestation', 'verify', $ArchivePath,
        '--bundle', $AttestationPath,
        '--repo', $script:SemapraxRepo,
        '--signer-workflow', "$($script:SemapraxRepo)/.github/workflows/ci.yml",
        '--source-ref', "refs/tags/$Tag",
        '--deny-self-hosted-runners'
    )
    $r = Invoke-SemapraxNative -FilePath $gh.Source -Arguments $ghArgs -TimeoutSeconds 180
    if ($r.ExitCode -ne 0) {
        $detail = (($r.Output + "`n" + $r.Error).Trim())
        Stop-SemapraxInstall "publisher verification failed (gh attestation verify exit $($r.ExitCode)):`n$detail"
    }
    Write-SemapraxLine 'publisher: verified (gh attestation verify)'
    return 'verified'
}

# ------------------------------------------------------------ zip handling

function Test-SemapraxZipEntryName {
    # Returns $null when acceptable, otherwise the reason it is refused.
    param([string]$Name, [string]$TopLevel)
    if ([string]::IsNullOrEmpty($Name)) { return 'empty member name' }
    if ($Name -match '[\x00-\x1f]') { return 'control character in member name' }
    if ($Name.StartsWith('/') -or $Name.StartsWith('\')) { return 'absolute member path' }
    if ($Name -match '^[A-Za-z]:') { return 'drive-qualified member path' }
    if ($Name.Contains(':')) { return 'colon in member path' }
    $segments = @($Name.Replace('\', '/').Split('/'))
    for ($i = 0; $i -lt $segments.Count; $i++) {
        $s = $segments[$i]
        if ($s -eq '') {
            if ($i -eq $segments.Count - 1) { continue }
            return 'empty path segment'
        }
        if ($s -eq '..') { return 'path traversal segment' }
        if ($s -eq '.') { return 'dot path segment' }
        if ($s.EndsWith('.') -or $s.EndsWith(' ')) { return 'segment ends with dot or space' }
        if ($s -match '^(?i:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(\..*)?$') { return 'reserved device name' }
    }
    if ($segments[0] -cne $TopLevel) { return "member is outside the expected top-level directory $TopLevel" }
    return $null
}

function Expand-SemapraxZip {
    # Validates every member first, then extracts with a final containment check.
    # Returns the extracted file paths relative to $Destination, '/'-separated.
    param([string]$ZipPath, [string]$Destination, [string]$TopLevel)
    try { Add-Type -AssemblyName System.IO.Compression.FileSystem } catch { }
    $destFull = [System.IO.Path]::GetFullPath($Destination).TrimEnd('\', '/')
    $prefix = $destFull + [System.IO.Path]::DirectorySeparatorChar
    [void][System.IO.Directory]::CreateDirectory($destFull)
    $zip = $null
    try {
        $zip = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    } catch {
        Stop-SemapraxInstall "archive is not a readable zip: $($_.Exception.Message)"
    }
    $files = @()
    try {
        $seen = @{}
        foreach ($e in $zip.Entries) {
            $reason = Test-SemapraxZipEntryName -Name $e.FullName -TopLevel $TopLevel
            if ($null -ne $reason) { Stop-SemapraxInstall "refusing archive member '$($e.FullName)': $reason" }
            $key = $e.FullName.Replace('\', '/').TrimEnd('/').ToLowerInvariant()
            if ($seen.ContainsKey($key)) { Stop-SemapraxInstall "refusing archive: duplicate member '$($e.FullName)'" }
            $seen[$key] = $true
            if ($null -ne $e.PSObject.Properties['ExternalAttributes']) {
                $mode = ([int64]$e.ExternalAttributes -shr 16) -band 61440
                if ($mode -eq 40960) { Stop-SemapraxInstall "refusing archive member '$($e.FullName)': symbolic link" }
            }
        }
        foreach ($e in $zip.Entries) {
            $rel = $e.FullName.Replace('\', '/')
            $native = $rel.Replace('/', [string][System.IO.Path]::DirectorySeparatorChar)
            $target = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($destFull, $native))
            if (-not $target.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
                Stop-SemapraxInstall "refusing archive member '$($e.FullName)': resolves outside the staging directory"
            }
            if ($rel.EndsWith('/')) {
                [void][System.IO.Directory]::CreateDirectory($target)
                continue
            }
            [void][System.IO.Directory]::CreateDirectory([System.IO.Path]::GetDirectoryName($target))
            $in = $e.Open()
            try {
                $out = [System.IO.File]::Open($target, [System.IO.FileMode]::CreateNew, [System.IO.FileAccess]::Write, [System.IO.FileShare]::None)
                try { $in.CopyTo($out) } finally { $out.Dispose() }
            } finally { $in.Dispose() }
            $files += $rel
        }
    } finally {
        $zip.Dispose()
    }
    return $files
}

# ----------------------------------------------------------- user PATH

function Get-SemapraxEnvSubKey {
    $s = $env:SEMAPRAX_INSTALL_ENV_SUBKEY
    if ([string]::IsNullOrEmpty($s)) { return 'Environment' }
    return $s
}

function ConvertTo-SemapraxPathSegmentKey {
    param([string]$Segment)
    $s = $Segment.Trim().TrimEnd('\', '/')
    return $s.ToLowerInvariant()
}

function Test-SemapraxPathHasEntry {
    param([string]$Path, [string]$Entry)
    $want = ConvertTo-SemapraxPathSegmentKey $Entry
    foreach ($seg in $Path.Split(';')) {
        if ($seg -eq '') { continue }
        if ((ConvertTo-SemapraxPathSegmentKey $seg) -eq $want) { return $true }
        if ($seg.Contains('%')) {
            $expanded = [Environment]::ExpandEnvironmentVariables($seg)
            if ((ConvertTo-SemapraxPathSegmentKey $expanded) -eq $want) { return $true }
        }
    }
    return $false
}

function Add-SemapraxPathEntry {
    # Pure. Prepends $Entry once; every existing segment is kept verbatim.
    param([string]$Path, [string]$Entry)
    if ($null -eq $Path) { $Path = '' }
    if (Test-SemapraxPathHasEntry -Path $Path -Entry $Entry) { return $Path }
    if ($Path.Trim() -eq '') { return $Entry }
    return ($Entry + ';' + $Path)
}

function Remove-SemapraxPathEntry {
    # Pure. Removes only segments equal to $Entry; unrelated segments keep order and text.
    param([string]$Path, [string]$Entry)
    if ([string]::IsNullOrEmpty($Path)) { return '' }
    $want = ConvertTo-SemapraxPathSegmentKey $Entry
    $kept = @()
    foreach ($seg in $Path.Split(';')) {
        if ($seg -ne '' -and (ConvertTo-SemapraxPathSegmentKey $seg) -eq $want) { continue }
        $kept += $seg
    }
    return ($kept -join ';')
}

function Get-SemapraxUserPath {
    # Raw (unexpanded) HKCU Path value and its registry kind.
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey((Get-SemapraxEnvSubKey), $false)
    $result = [pscustomobject]@{ Exists = $false; Value = ''; Kind = [Microsoft.Win32.RegistryValueKind]::ExpandString }
    if ($null -eq $key) { return $result }
    try {
        $v = $key.GetValue('Path', $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        if ($null -ne $v) {
            $result.Exists = $true
            $result.Value = [string]$v
            $result.Kind = $key.GetValueKind('Path')
        }
    } finally { $key.Close() }
    return $result
}

function Set-SemapraxUserPath {
    param([string]$Value, $Kind)
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey((Get-SemapraxEnvSubKey))
    try { $key.SetValue('Path', $Value, $Kind) } finally { $key.Close() }
}

function Send-SemapraxEnvironmentChange {
    if ((Get-SemapraxEnvSubKey) -ne 'Environment') { return }
    try {
        Initialize-SemapraxNative
        $result = [UIntPtr]::Zero
        [void][SemapraxNative]::SendMessageTimeout([IntPtr]0xffff, 0x1A, [UIntPtr]::Zero, 'Environment', 2, 5000, [ref]$result)
    } catch {
        Write-SemapraxLine "warning: could not broadcast the environment change: $($_.Exception.Message)"
    }
}

function Add-SemapraxUserPathEntry {
    # Returns $true when the registry value was changed.
    param([string]$Entry)
    $cur = Get-SemapraxUserPath
    $new = Add-SemapraxPathEntry -Path $cur.Value -Entry $Entry
    if ($new -ceq $cur.Value -and $cur.Exists) { return $false }
    $kind = $cur.Kind
    Set-SemapraxUserPath -Value $new -Kind $kind
    Send-SemapraxEnvironmentChange
    return $true
}

function Remove-SemapraxUserPathEntry {
    param([string]$Entry)
    $cur = Get-SemapraxUserPath
    if (-not $cur.Exists) { return $false }
    $new = Remove-SemapraxPathEntry -Path $cur.Value -Entry $Entry
    if ($new -ceq $cur.Value) { return $false }
    Set-SemapraxUserPath -Value $new -Kind $cur.Kind
    Send-SemapraxEnvironmentChange
    return $true
}

# --------------------------------------------------------------- receipt

function New-SemapraxReceiptText {
    param([string]$Tag, [string]$Target, [string]$Source, [string]$ArchiveSha256, [string]$Publisher, [string[]]$Files, [string]$PathKind)
    $location = $null
    if ($PathKind -eq 'user-path') { $location = 'HKCU\Environment\Path' }
    $receipt = [ordered]@{
        schema                = $script:SemapraxReceiptSchema
        installer             = 'install.ps1'
        version               = $Tag.TrimStart('v')
        tag                   = $Tag
        target                = $Target
        source                = $Source
        archive_sha256        = $ArchiveSha256
        publisher_verification = $Publisher
        files                 = @($Files)
        path_modification     = [ordered]@{ kind = $PathKind; location = $location }
    }
    return ((ConvertTo-Json -InputObject $receipt -Depth 6) + "`n")
}

function Write-SemapraxTextFile {
    param([string]$Path, [string]$Text)
    $enc = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($Path, $Text, $enc)
}

function Read-SemapraxReceipt {
    # Returns $null when there is no receipt; throws when it is unreadable or foreign.
    param([string]$InstallDir)
    $path = [System.IO.Path]::Combine($InstallDir, $script:SemapraxReceiptName)
    if (-not [System.IO.File]::Exists($path)) { return $null }
    $r = $null
    try {
        $r = [System.IO.File]::ReadAllText($path, [System.Text.Encoding]::UTF8) | ConvertFrom-Json
    } catch {
        Stop-SemapraxInstall "$path is not valid JSON; refusing to touch this directory"
    }
    if ($null -eq $r -or $r.schema -cne $script:SemapraxReceiptSchema) {
        Stop-SemapraxInstall "$path is not a $($script:SemapraxReceiptSchema) receipt; refusing to touch this directory"
    }
    if ($r.installer -cne 'install.ps1') {
        Stop-SemapraxInstall "$path was written by '$($r.installer)', not install.ps1; refusing to touch this directory"
    }
    foreach ($f in @($r.files)) {
        $reason = $null
        if ($f -isnot [string] -or $f -eq '' -or $f.StartsWith('/') -or $f.StartsWith('\') -or $f -match '^[A-Za-z]:' -or ($f.Replace('\', '/').Split('/') -contains '..')) {
            $reason = 'unsafe path'
        }
        if ($null -ne $reason) { Stop-SemapraxInstall "receipt lists an unsafe file path; refusing to touch this directory" }
    }
    return $r
}

# --------------------------------------------------------- file helpers

function ConvertTo-SemapraxUri {
    param([string]$Value)
    if ($Value -match '^[A-Za-z][A-Za-z0-9+.-]+://') { return $Value }
    return (New-Object System.Uri $Value).AbsoluteUri
}

function Get-SemapraxAssetUrl {
    param([string]$Base, [string]$Tag, [string]$Name)
    return ((ConvertTo-SemapraxUri $Base).TrimEnd('/') + '/' + $Tag + '/' + $Name)
}

function Copy-SemapraxAsset {
    param([string]$Url, [string]$OutFile)
    if ($Url -match '^file:') {
        $local = (New-Object System.Uri $Url).LocalPath
        if (-not [System.IO.File]::Exists($local)) { Stop-SemapraxInstall "download failed: $Url (file not found)" }
        [System.IO.File]::Copy($local, $OutFile, $true)
        return
    }
    try {
        try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12 } catch { }
        Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $OutFile -Headers @{ 'User-Agent' = 'semaprax-install.ps1' }
    } catch {
        Stop-SemapraxInstall "download failed: $Url ($($_.Exception.Message))"
    }
}

function Resolve-SemapraxLatestTag {
    param([string]$LatestUrl)
    $url = ConvertTo-SemapraxUri $LatestUrl
    if ($url -match '^file:') {
        $uri = New-Object System.Uri $url
        $path = $uri.LocalPath
        if ($uri.AbsolutePath -match '/tag/([^/]+)/?$') {
            return (ConvertTo-SemapraxTag ([System.Uri]::UnescapeDataString($Matches[1])))
        }
        if ([System.IO.File]::Exists($path)) {
            return (ConvertTo-SemapraxTag ([System.IO.File]::ReadAllText($path).Trim()))
        }
        Stop-SemapraxInstall "cannot resolve the latest release from $LatestUrl"
    }
    try { [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12 } catch { }
    $current = $url
    for ($hop = 0; $hop -lt 6; $hop++) {
        $req = [System.Net.HttpWebRequest]::Create($current)
        $req.AllowAutoRedirect = $false
        $req.UserAgent = 'semaprax-install.ps1'
        $resp = $null
        try {
            try { $resp = $req.GetResponse() } catch [System.Net.WebException] { $resp = $_.Exception.Response; if ($null -eq $resp) { throw } }
            $code = [int]$resp.StatusCode
            $loc = $resp.Headers['Location']
        } catch {
            Stop-SemapraxInstall "could not resolve the latest release from ${current}: $($_.Exception.Message)"
        } finally {
            if ($null -ne $resp) { $resp.Close() }
        }
        if ($code -ge 300 -and $code -lt 400 -and $loc) {
            $next = (New-Object System.Uri ((New-Object System.Uri $current), $loc)).AbsoluteUri
            if ($next -match '/tag/([^/?#]+)') {
                return (ConvertTo-SemapraxTag ([System.Uri]::UnescapeDataString($Matches[1])))
            }
            $current = $next
            continue
        }
        break
    }
    Stop-SemapraxInstall "$LatestUrl did not redirect to a release tag"
}

function Test-SemapraxFileLocked {
    param([string]$Path)
    if (-not [System.IO.File]::Exists($Path)) { return $false }
    try {
        $fs = [System.IO.File]::Open($Path, [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
        $fs.Dispose()
        return $false
    } catch {
        return $true
    }
}

function Remove-SemapraxTree {
    param([string]$Path, [switch]$Quiet)
    if (-not (Test-Path -LiteralPath $Path)) { return $true }
    try {
        Remove-Item -LiteralPath $Path -Recurse -Force -ErrorAction Stop
        return $true
    } catch {
        if (-not $Quiet) { Write-SemapraxLine "warning: could not remove ${Path}: $($_.Exception.Message)" }
        return $false
    }
}

function Remove-SemapraxEmptyDirs {
    # Removes $Path and empty children, never anything that still holds a file.
    param([string]$Path)
    if (-not [System.IO.Directory]::Exists($Path)) { return }
    foreach ($d in [System.IO.Directory]::GetDirectories($Path)) { Remove-SemapraxEmptyDirs -Path $d }
    if (@([System.IO.Directory]::GetFileSystemEntries($Path)).Count -eq 0) {
        try { [System.IO.Directory]::Delete($Path) } catch { }
    }
}

function Get-SemapraxInUseMessage {
    param([string]$Detail)
    return ("could not replace the installed executables ($Detail). A semaprax or semapraxd process is probably still running from the installation. Close every running semaprax/semapraxd (terminals, editors, the daemon) and run this installer again. The existing installation was left unchanged.")
}

# --------------------------------------------------------------- options

function Resolve-SemapraxOptions {
    param([string]$Version, [string]$InstallDir, [bool]$NoModifyPath, [bool]$Uninstall, [bool]$RequirePublisherVerification)
    $v = $Version
    if ([string]::IsNullOrEmpty($v)) { $v = $env:SEMAPRAX_INSTALL_VERSION }
    $tag = $null
    if (-not [string]::IsNullOrEmpty($v)) { $tag = ConvertTo-SemapraxTag $v }
    $dir = $InstallDir
    if ([string]::IsNullOrEmpty($dir)) { $dir = $env:SEMAPRAX_INSTALL_DIR }
    return @{
        Tag          = $tag
        InstallDir   = $dir
        NoModifyPath = ($NoModifyPath -or (Test-SemapraxEnvFlag 'SEMAPRAX_INSTALL_NO_MODIFY_PATH'))
        Uninstall    = ($Uninstall -or (Test-SemapraxEnvFlag 'SEMAPRAX_INSTALL_UNINSTALL'))
        Require      = ($RequirePublisherVerification -or (Test-SemapraxEnvFlag 'SEMAPRAX_INSTALL_REQUIRE_PUBLISHER_VERIFICATION'))
    }
}

function Resolve-SemapraxInstallDir {
    param([string]$Dir)
    if ([string]::IsNullOrEmpty($Dir)) {
        if ([string]::IsNullOrEmpty($env:LOCALAPPDATA)) {
            Stop-SemapraxInstall 'LOCALAPPDATA is not set; pass -InstallDir'
        }
        $Dir = [System.IO.Path]::Combine($env:LOCALAPPDATA, 'Programs', 'Semaprax')
    }
    $full = [System.IO.Path]::GetFullPath($Dir).TrimEnd('\', '/')
    if ($full -eq '' -or $full -match '^[A-Za-z]:$' -or $full -eq [System.IO.Path]::GetPathRoot($full).TrimEnd('\', '/')) {
        Stop-SemapraxInstall "refusing to install into a drive or filesystem root: $Dir"
    }
    return $full
}

function Assert-SemapraxWindows {
    if ([Environment]::OSVersion.Platform -ne [System.PlatformID]::Win32NT) {
        Stop-SemapraxInstall 'install.ps1 supports Windows only. On macOS and Linux use install.sh.'
    }
}

function Assert-SemapraxPrerequisites {
    foreach ($cmd in @('Get-FileHash', 'ConvertFrom-Json', 'Invoke-WebRequest')) {
        if ($null -eq (Get-Command $cmd -ErrorAction SilentlyContinue)) {
            Stop-SemapraxInstall "required PowerShell command $cmd is missing; use Windows PowerShell 5.1 or PowerShell 7"
        }
    }
    try { Add-Type -AssemblyName System.IO.Compression.FileSystem } catch {
        Stop-SemapraxInstall "the .NET zip support (System.IO.Compression.FileSystem) is unavailable: $($_.Exception.Message)"
    }
}

# ------------------------------------------------------------- uninstall

function Invoke-SemapraxUninstall {
    param([string]$InstallDir)
    if (-not [System.IO.Directory]::Exists($InstallDir)) {
        Write-SemapraxLine "nothing to uninstall: $InstallDir does not exist"
        return 0
    }
    $receipt = Read-SemapraxReceipt -InstallDir $InstallDir
    if ($null -eq $receipt) {
        if (@([System.IO.Directory]::GetFileSystemEntries($InstallDir)).Count -eq 0) {
            Write-SemapraxLine "nothing to uninstall: $InstallDir is empty"
            return 0
        }
        Stop-SemapraxInstall "$InstallDir has no $($script:SemapraxReceiptName); refusing to remove files this installer does not own"
    }
    $bin = [System.IO.Path]::Combine($InstallDir, 'bin')
    foreach ($exe in @('semaprax.exe', 'semapraxd.exe')) {
        if (Test-SemapraxFileLocked ([System.IO.Path]::Combine($bin, $exe))) {
            Stop-SemapraxInstall (Get-SemapraxInUseMessage "bin\$exe is in use")
        }
    }
    foreach ($rel in @($receipt.files)) {
        $p = [System.IO.Path]::Combine($InstallDir, $rel.Replace('/', [string][System.IO.Path]::DirectorySeparatorChar))
        if ([System.IO.File]::Exists($p)) {
            try { [System.IO.File]::Delete($p) } catch {
                Stop-SemapraxInstall (Get-SemapraxInUseMessage "could not delete $rel")
            }
        }
    }
    foreach ($leftover in @('bin.new', 'bin.old')) {
        [void](Remove-SemapraxTree -Path ([System.IO.Path]::Combine($InstallDir, $leftover)) -Quiet)
    }
    foreach ($d in [System.IO.Directory]::GetDirectories($InstallDir, '.staging-*')) {
        [void](Remove-SemapraxTree -Path $d -Quiet)
    }
    Remove-SemapraxEmptyDirs -Path ([System.IO.Path]::Combine($InstallDir, 'versions'))
    Remove-SemapraxEmptyDirs -Path $bin
    $pathKind = $null
    if ($null -ne $receipt.path_modification) { $pathKind = [string]$receipt.path_modification.kind }
    if ($pathKind -eq 'user-path') {
        $entry = $InstallDir + '\bin'
        if (Remove-SemapraxUserPathEntry -Entry $entry) {
            Write-SemapraxLine "removed $entry from the user PATH"
        }
    }
    $receiptPath = [System.IO.Path]::Combine($InstallDir, $script:SemapraxReceiptName)
    [System.IO.File]::Delete($receiptPath)
    if (@([System.IO.Directory]::GetFileSystemEntries($InstallDir)).Count -eq 0) {
        try { [System.IO.Directory]::Delete($InstallDir) } catch { }
    } else {
        Write-SemapraxLine "kept $InstallDir because it still contains files this installer does not own"
    }
    Write-SemapraxLine "semaprax $($receipt.tag) uninstalled from $InstallDir"
    return 0
}

# --------------------------------------------------------------- install

function Invoke-SemapraxActivate {
    # Installs the extracted package as versions\<tag> and swaps bin\ as one unit.
    # On failure everything is rolled back and an exception is thrown.
    param([string]$InstallDir, [string]$Tag, [string]$PackageDir, $OldReceipt)
    $versionsRoot = [System.IO.Path]::Combine($InstallDir, 'versions')
    [void][System.IO.Directory]::CreateDirectory($versionsRoot)
    $verDir = [System.IO.Path]::Combine($versionsRoot, $Tag)
    $bin = [System.IO.Path]::Combine($InstallDir, 'bin')
    $binNew = [System.IO.Path]::Combine($InstallDir, 'bin.new')
    $binOld = [System.IO.Path]::Combine($InstallDir, 'bin.old')
    $ownedBin = @()
    if ($null -ne $OldReceipt) {
        $ownedBin = @($OldReceipt.files | Where-Object { $_ -like 'bin/*' } | ForEach-Object { $_.Substring(4).ToLowerInvariant() })
    }
    if ([System.IO.Directory]::Exists($bin)) {
        foreach ($f in [System.IO.Directory]::GetFiles($bin)) {
            $leaf = [System.IO.Path]::GetFileName($f)
            if ($leaf -like 'semaprax*' -and ($ownedBin -notcontains $leaf.ToLowerInvariant())) {
                Stop-SemapraxInstall "refusing to replace ${f}: it was not installed by this installer"
            }
        }
        foreach ($exe in @('semaprax.exe', 'semapraxd.exe')) {
            if (Test-SemapraxFileLocked ([System.IO.Path]::Combine($bin, $exe))) {
                Stop-SemapraxInstall (Get-SemapraxInUseMessage "bin\$exe is in use")
            }
        }
    }
    $verOld = $null
    $verPlaced = $false
    $binMovedAside = $false
    $binInstalled = $false
    try {
        if ([System.IO.Directory]::Exists($verDir)) {
            $verOld = $verDir + '.old-' + [Guid]::NewGuid().ToString('N').Substring(0, 8)
            [System.IO.Directory]::Move($verDir, $verOld)
        }
        [System.IO.Directory]::Move($PackageDir, $verDir)
        $verPlaced = $true
        [void][System.IO.Directory]::CreateDirectory($binNew)
        foreach ($exe in @('semaprax.exe', 'semapraxd.exe')) {
            $src = [System.IO.Path]::Combine($verDir, $exe)
            $dst = [System.IO.Path]::Combine($binNew, $exe)
            [System.IO.File]::Copy($src, $dst, $false)
            if ((Get-SemapraxFileSha256 $src) -ne (Get-SemapraxFileSha256 $dst)) {
                Stop-SemapraxInstall "copy of $exe into bin does not match the verified file"
            }
        }
        if ([System.IO.Directory]::Exists($bin)) {
            foreach ($f in [System.IO.Directory]::GetFiles($bin)) {
                $leaf = [System.IO.Path]::GetFileName($f)
                if ($leaf -notlike 'semaprax*') {
                    [System.IO.File]::Copy($f, [System.IO.Path]::Combine($binNew, $leaf), $false)
                }
            }
            try {
                [System.IO.Directory]::Move($bin, $binOld)
            } catch {
                Stop-SemapraxInstall (Get-SemapraxInUseMessage $_.Exception.Message)
            }
            $binMovedAside = $true
        }
        [System.IO.Directory]::Move($binNew, $bin)
        $binInstalled = $true
    } catch {
        $failure = $_
        if ($binMovedAside -and -not $binInstalled -and -not [System.IO.Directory]::Exists($bin)) {
            try { [System.IO.Directory]::Move($binOld, $bin) } catch { }
        }
        [void](Remove-SemapraxTree -Path $binNew -Quiet)
        if ($verPlaced) {
            [void](Remove-SemapraxTree -Path $verDir -Quiet)
            if ($null -ne $verOld) { try { [System.IO.Directory]::Move($verOld, $verDir) } catch { } }
        } elseif ($null -ne $verOld -and -not [System.IO.Directory]::Exists($verDir)) {
            try { [System.IO.Directory]::Move($verOld, $verDir) } catch { }
        }
        throw $failure
    }
    if ($binMovedAside) { [void](Remove-SemapraxTree -Path $binOld) }
    if ($null -ne $verOld) { [void](Remove-SemapraxTree -Path $verOld) }
}

function Invoke-SemapraxInstall {
    param($Options)
    $installDir = $Options.InstallDir
    $tag = $Options.Tag
    $arch = Get-SemapraxArchInputs
    $target = Resolve-SemapraxTarget -Arch $arch -Tag $tag
    $ext = Get-SemapraxTargetExtension $target

    $downloadOverride = $env:SEMAPRAX_INSTALL_DOWNLOAD_BASE
    $latestOverride = $env:SEMAPRAX_INSTALL_LATEST_URL
    $base = $script:SemapraxDefaultDownloadBase
    if (-not [string]::IsNullOrEmpty($downloadOverride)) { $base = $downloadOverride }
    $latestUrl = $script:SemapraxDefaultLatestUrl
    if (-not [string]::IsNullOrEmpty($latestOverride)) { $latestUrl = $latestOverride }
    if ($null -eq $tag) {
        if ((-not [string]::IsNullOrEmpty($downloadOverride)) -and [string]::IsNullOrEmpty($latestOverride)) {
            Stop-SemapraxInstall 'SEMAPRAX_INSTALL_DOWNLOAD_BASE is set without SEMAPRAX_INSTALL_LATEST_URL: pass -Version'
        }
        $tag = Resolve-SemapraxLatestTag -LatestUrl $latestUrl
        Write-SemapraxLine "latest release: $tag"
    }

    $archiveName = "semaprax-$tag-$target.$ext"
    $topLevel = "semaprax-$tag-$target"
    $attestationName = "release-attestation-$target.json"

    $dirExisted = [System.IO.Directory]::Exists($installDir)
    $receipt = $null
    if ($dirExisted) {
        $receipt = Read-SemapraxReceipt -InstallDir $installDir
        if ($null -eq $receipt -and @([System.IO.Directory]::GetFileSystemEntries($installDir)).Count -gt 0) {
            Stop-SemapraxInstall "$installDir is not empty and has no $($script:SemapraxReceiptName); refusing to install over files this installer does not own. Choose another -InstallDir."
        }
    }
    if ($null -ne $receipt) {
        # Recover from an interrupted earlier run, then drop stale staging.
        $bin = [System.IO.Path]::Combine($installDir, 'bin')
        $binOld = [System.IO.Path]::Combine($installDir, 'bin.old')
        if (-not [System.IO.Directory]::Exists($bin) -and [System.IO.Directory]::Exists($binOld)) {
            [System.IO.Directory]::Move($binOld, $bin)
        }
        foreach ($leftover in @('bin.new', 'bin.old')) {
            [void](Remove-SemapraxTree -Path ([System.IO.Path]::Combine($installDir, $leftover)) -Quiet)
        }
        foreach ($d in [System.IO.Directory]::GetDirectories($installDir, '.staging-*')) {
            [void](Remove-SemapraxTree -Path $d -Quiet)
        }
    }

    [void][System.IO.Directory]::CreateDirectory($installDir)
    $staging = [System.IO.Path]::Combine($installDir, '.staging-' + [Guid]::NewGuid().ToString('N').Substring(0, 12))
    $done = $false
    try {
        $dl = [System.IO.Path]::Combine($staging, 'dl')
        [void][System.IO.Directory]::CreateDirectory($dl)
        Write-SemapraxLine "installing semaprax $tag ($target) into $installDir"
        $sumsPath = [System.IO.Path]::Combine($dl, 'SHA256SUMS')
        $manifestPath = [System.IO.Path]::Combine($dl, 'release-manifest.json')
        $archivePath = [System.IO.Path]::Combine($dl, $archiveName)
        $attestPath = [System.IO.Path]::Combine($dl, $attestationName)
        foreach ($pair in @(
                @('SHA256SUMS', $sumsPath), @('release-manifest.json', $manifestPath),
                @($archiveName, $archivePath), @($attestationName, $attestPath))) {
            Copy-SemapraxAsset -Url (Get-SemapraxAssetUrl -Base $base -Tag $tag -Name $pair[0]) -OutFile $pair[1]
        }
        $archiveUrl = Get-SemapraxAssetUrl -Base $base -Tag $tag -Name $archiveName

        # 1. SHA256SUMS, 2. manifest, 3. attestation file + publisher verification.
        $sha = Get-SemapraxFileSha256 $archivePath
        $expected = Get-SemapraxChecksumFromText -Text ([System.IO.File]::ReadAllText($sumsPath)) -Name $archiveName
        if ($sha -ne $expected) {
            Stop-SemapraxInstall "checksum mismatch for ${archiveName}: SHA256SUMS says $expected but the download is $sha"
        }
        $manifest = $null
        try { $manifest = [System.IO.File]::ReadAllText($manifestPath, [System.Text.Encoding]::UTF8) | ConvertFrom-Json } catch {
            Stop-SemapraxInstall "release-manifest.json is malformed: $($_.Exception.Message)"
        }
        $size = ([System.IO.FileInfo]$archivePath).Length
        Assert-SemapraxManifestArtifact -Manifest $manifest -Tag $tag -Name $archiveName -Target $target -Size $size -Sha256 $sha
        if (([System.IO.FileInfo]$attestPath).Length -eq 0) {
            Stop-SemapraxInstall "$attestationName is empty"
        }
        $publisher = Invoke-SemapraxPublisherVerification -ArchivePath $archivePath -AttestationPath $attestPath -Tag $tag -Require $Options.Require
        Write-SemapraxLine "checksum: verified (SHA256SUMS and release manifest)"

        # 4. Extract with traversal refusal; require both executables.
        $extractRoot = [System.IO.Path]::Combine($staging, 'x')
        $members = Expand-SemapraxZip -ZipPath $archivePath -Destination $extractRoot -TopLevel $topLevel
        $pkg = [System.IO.Path]::Combine($extractRoot, $topLevel)
        foreach ($exe in @('semaprax.exe', 'semapraxd.exe')) {
            if (-not [System.IO.File]::Exists([System.IO.Path]::Combine($pkg, $exe))) {
                Stop-SemapraxInstall "archive does not contain $topLevel/$exe"
            }
        }

        # 5. Smoke the staged binary before activation.
        $smoke = Invoke-SemapraxNative -FilePath ([System.IO.Path]::Combine($pkg, 'semaprax.exe')) -Arguments @('version', '--json') -TimeoutSeconds 60
        if ($smoke.ExitCode -ne 0) {
            Stop-SemapraxInstall "the staged semaprax.exe failed its smoke test (version --json exit $($smoke.ExitCode)): $($smoke.Error.Trim())"
        }
        $reported = $null
        try { $reported = ($smoke.Output | ConvertFrom-Json).version } catch { }
        if ($reported -cne $tag.TrimStart('v')) {
            Stop-SemapraxInstall "the staged semaprax.exe reports version '$reported', expected $($tag.TrimStart('v'))"
        }

        # Same-version reinstall of identical bytes needs no swap (and cannot hit a lock).
        $upToDate = $false
        $binDir = [System.IO.Path]::Combine($installDir, 'bin')
        if ($null -ne $receipt -and $receipt.tag -ceq $tag -and $receipt.archive_sha256 -ceq $sha -and $receipt.target -ceq $target) {
            $upToDate = $true
            foreach ($exe in @('semaprax.exe', 'semapraxd.exe')) {
                $installed = [System.IO.Path]::Combine($binDir, $exe)
                $inVer = [System.IO.Path]::Combine($installDir, 'versions', $tag, $exe)
                $staged = [System.IO.Path]::Combine($pkg, $exe)
                if (-not ([System.IO.File]::Exists($installed) -and [System.IO.File]::Exists($inVer) -and
                        (Get-SemapraxFileSha256 $installed) -eq (Get-SemapraxFileSha256 $staged) -and
                        (Get-SemapraxFileSha256 $inVer) -eq (Get-SemapraxFileSha256 $staged))) {
                    $upToDate = $false
                }
            }
        }
        $verFiles = @()
        foreach ($m in $members) { $verFiles += ('versions/' + $tag + '/' + $m.Substring($topLevel.Length + 1)) }
        if ($upToDate) {
            Write-SemapraxLine "semaprax $tag is already installed and up to date"
        } else {
            Invoke-SemapraxActivate -InstallDir $installDir -Tag $tag -PackageDir $pkg -OldReceipt $receipt
        }

        # PATH, then receipt (written last).
        $binEntry = $installDir + '\bin'
        $pathKind = 'none'
        if ($null -ne $receipt -and $null -ne $receipt.path_modification -and $receipt.path_modification.kind -eq 'user-path') { $pathKind = 'user-path' }
        $pathAdded = $false
        if (-not $Options.NoModifyPath) {
            $had = Test-SemapraxPathHasEntry -Path (Get-SemapraxUserPath).Value -Entry $binEntry
            try {
                $pathAdded = Add-SemapraxUserPathEntry -Entry $binEntry
                if ($pathAdded -or ($had -and $pathKind -eq 'user-path')) { $pathKind = 'user-path' }
                if ($pathAdded) { $pathKind = 'user-path' }
            } catch {
                Write-SemapraxLine "warning: could not update the user PATH: $($_.Exception.Message)"
            }
        }
        $files = @('bin/semaprax.exe', 'bin/semapraxd.exe') + $verFiles
        $text = New-SemapraxReceiptText -Tag $tag -Target $target -Source $archiveUrl -ArchiveSha256 $sha -Publisher $publisher -Files $files -PathKind $pathKind
        $receiptPath = [System.IO.Path]::Combine($installDir, $script:SemapraxReceiptName)
        try {
            Write-SemapraxTextFile -Path ($receiptPath + '.tmp') -Text $text
            [System.IO.File]::Copy(($receiptPath + '.tmp'), $receiptPath, $true)
            [System.IO.File]::Delete($receiptPath + '.tmp')
        } catch {
            if ($pathAdded) { try { [void](Remove-SemapraxUserPathEntry -Entry $binEntry) } catch { } }
            Stop-SemapraxInstall "could not write the install receipt: $($_.Exception.Message)"
        }
        $done = $true

        # Prune the superseded version only after successful activation.
        if ($null -ne $receipt -and $receipt.tag -cne $tag) {
            $old = [System.IO.Path]::Combine($installDir, 'versions', [string]$receipt.tag)
            if ($old -ne [System.IO.Path]::Combine($installDir, 'versions', $tag)) { [void](Remove-SemapraxTree -Path $old) }
        }

        Write-SemapraxLine "semaprax $tag installed to $installDir"
        Write-SemapraxLine "  semaprax.exe, semapraxd.exe: $binEntry"
        if ($Options.NoModifyPath) {
            Write-SemapraxLine "user PATH: not modified (-NoModifyPath)"
        } elseif ($pathAdded) {
            Write-SemapraxLine "user PATH: added $binEntry. Terminals and editors that are already open must be restarted."
        } else {
            Write-SemapraxLine "user PATH: $binEntry is already present"
        }
        $sessionBin = $binEntry
        if ($sessionBin -match '[$`]') {
            Write-SemapraxLine ("Use semaprax in this terminal now:`n  " + '$env:Path = ''' + $sessionBin.Replace("'", "''") + ';'' + $env:Path')
        } else {
            Write-SemapraxLine ("Use semaprax in this terminal now:`n  " + '$env:Path = "' + $sessionBin + ';" + $env:Path')
        }
        return 0
    } finally {
        [void](Remove-SemapraxTree -Path $staging -Quiet)
        if (-not $done -and -not $dirExisted) {
            try {
                if (@([System.IO.Directory]::GetFileSystemEntries($installDir)).Count -eq 0) { [System.IO.Directory]::Delete($installDir) }
            } catch { }
        }
    }
}

function Invoke-SemapraxInstaller {
    param([string]$Version, [string]$InstallDir, [bool]$NoModifyPath, [bool]$Uninstall, [bool]$RequirePublisherVerification)
    $ErrorActionPreference = 'Stop'
    $ProgressPreference = 'SilentlyContinue'
    try {
        $opts = Resolve-SemapraxOptions -Version $Version -InstallDir $InstallDir -NoModifyPath $NoModifyPath -Uninstall $Uninstall -RequirePublisherVerification $RequirePublisherVerification
        Assert-SemapraxWindows
        $opts.InstallDir = Resolve-SemapraxInstallDir $opts.InstallDir
        if ($opts.Uninstall) {
            $r = @(Invoke-SemapraxUninstall -InstallDir $opts.InstallDir)
        } else {
            Assert-SemapraxPrerequisites
            $r = @(Invoke-SemapraxInstall -Options $opts)
        }
        return [int]$r[-1]
    } catch {
        Write-Host ("error: " + $_.Exception.Message)
        return 1
    }
}

if ($env:SEMAPRAX_INSTALL_PS1_NO_MAIN -ne '1' -and $MyInvocation.InvocationName -ne '.') {
    $semapraxExit = Invoke-SemapraxInstaller -Version $Version -InstallDir $InstallDir -NoModifyPath ([bool]$NoModifyPath) -Uninstall ([bool]$Uninstall) -RequirePublisherVerification ([bool]$RequirePublisherVerification)
    if ($PSCommandPath) {
        exit $semapraxExit
    } elseif ($semapraxExit -ne 0) {
        # Under `irm | iex` an `exit` would close the user's terminal; throw instead.
        throw "semaprax install failed (see the error above)"
    }
}
