<#
.SYNOPSIS
    CI journey for the Windows installer against a directory of real release assets.

.DESCRIPTION
    Usage: install-journey.ps1 -AssetsDir <dir> -Tag <tag>

    <dir> is a flat directory holding the release assets (install.ps1, SHA256SUMS,
    release-manifest.json, semaprax-<tag>-x86_64-pc-windows-msvc.zip,
    release-attestation-x86_64-pc-windows-msvc.json, ...). The journey runs THAT
    directory's install.ps1 through a file:// download base (laid out as
    <base>/<tag>/<asset>) into a fresh install directory whose path contains a
    space, in a child PowerShell of the same edition as this script, then:

      1. asserts `Get-Command semaprax` resolves inside the install dir (the
         session Path is rebuilt so no other semaprax can win) and that both
         installed executables hash-equal the zip members;
      2. runs the literal beginner journey from a fresh directory and asserts 42;
      3. reinstalls the same version (idempotent, hashes unchanged);
      4. runs a damaged-zip control (nonzero, old install still works);
      5. uninstalls and asserts the owned files are gone.

    The user PATH is not modified unless SEMAPRAX_JOURNEY_MODIFY_PATH=1, in which
    case the journey also asserts the entry is added once and removed again.
    The runner hides gh from the installer (checksum-only) unless
    SEMAPRAX_JOURNEY_PUBLISHER_VERIFY=1, which keeps gh and passes
    -RequirePublisherVerification (needs a published tag with attestations).
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$AssetsDir,
    [Parameter(Mandatory = $true)][string]$Tag
)

$ErrorActionPreference = 'Stop'
try { Add-Type -AssemblyName System.IO.Compression.FileSystem } catch { }

$target = 'x86_64-pc-windows-msvc'
$topLevel = "semaprax-$Tag-$target"
$version = $Tag.TrimStart('v')
$modifyPath = ($env:SEMAPRAX_JOURNEY_MODIFY_PATH -eq '1')
$publisherVerify = ($env:SEMAPRAX_JOURNEY_PUBLISHER_VERIFY -eq '1')
$hostExe = (Get-Process -Id $PID).Path
$steps = New-Object System.Collections.ArrayList

function Step {
    param([string]$Message)
    [void]$steps.Add($Message)
    Write-Host "journey: $Message"
}

function Fail {
    param([string]$Message)
    throw $Message
}

function ConvertTo-ArgString {
    param([string[]]$Arguments)
    $parts = @()
    foreach ($a in $Arguments) {
        if ($a -eq '') { $parts += '""' }
        elseif ($a -notmatch '[\s"]') { $parts += $a }
        else {
            $s = $a -replace '(\\*)"', '$1$1\"'
            $s = $s -replace '(\\+)$', '$1$1'
            $parts += ('"' + $s + '"')
        }
    }
    return ($parts -join ' ')
}

function Invoke-Native {
    param([string]$FilePath, [string[]]$Arguments = @(), [string]$WorkingDirectory = '', [int]$TimeoutSeconds = 600)
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $FilePath
    $psi.Arguments = ConvertTo-ArgString $Arguments
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    if ($WorkingDirectory -ne '') { $psi.WorkingDirectory = $WorkingDirectory }
    $p = [System.Diagnostics.Process]::Start($psi)
    $o = $p.StandardOutput.ReadToEndAsync()
    $e = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit($TimeoutSeconds * 1000)) {
        try { $p.Kill() } catch { }
        Fail "timed out: $FilePath $($psi.Arguments)"
    }
    $p.WaitForExit()
    return [pscustomobject]@{ ExitCode = $p.ExitCode; Output = $o.Result; Error = $e.Result }
}

function Assert-Journey {
    param($Condition, [string]$Message)
    if (-not $Condition) { Fail "assertion failed: $Message" }
}

function Get-Sha256OfFile {
    param([string]$Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Get-ZipMemberSha256 {
    param([string]$ZipPath, [string]$Member)
    $zip = [System.IO.Compression.ZipFile]::OpenRead($ZipPath)
    try {
        $entry = $null
        foreach ($e in $zip.Entries) { if ($e.FullName.Replace('\', '/') -ceq $Member) { $entry = $e } }
        if ($null -eq $entry) { Fail "zip member $Member not found in $ZipPath" }
        $sha = [System.Security.Cryptography.SHA256]::Create()
        $s = $entry.Open()
        try { $h = $sha.ComputeHash($s) } finally { $s.Dispose(); $sha.Dispose() }
        return (($h | ForEach-Object { $_.ToString('x2') }) -join '')
    } finally { $zip.Dispose() }
}

function Get-UserPathRaw {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $false)
    if ($null -eq $key) { return '' }
    try {
        $v = $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        return [string]$v
    } finally { $key.Close() }
}

function Test-HasSemapraxExe {
    param([string]$Dir)
    if ($Dir -eq '') { return $false }
    try { return ([System.IO.File]::Exists([System.IO.Path]::Combine($Dir, 'semaprax.exe')) -or [System.IO.File]::Exists([System.IO.Path]::Combine($Dir, 'semapraxd.exe'))) } catch { return $false }
}

function Set-SessionPath {
    # Rebuild $env:Path so no other semaprax resolves first.
    param([string]$BinDir)
    $base = $script:OriginalPath
    if ($modifyPath) {
        $base = [Environment]::ExpandEnvironmentVariables((Get-UserPathRaw))
        $machine = [Environment]::GetEnvironmentVariable('Path', 'Machine')
        $base = $machine + ';' + $base
    }
    $keep = @()
    foreach ($seg in $base.Split(';')) {
        if ($seg -eq '') { continue }
        if ((Test-HasSemapraxExe $seg) -and ($seg.TrimEnd('\') -ine $BinDir.TrimEnd('\'))) { continue }
        $keep += $seg
    }
    $env:Path = ($BinDir + ';' + ($keep -join ';'))
}

function Invoke-Installer {
    param([string]$Base, [string[]]$Extra)
    $pathForChild = $script:OriginalPath
    if (-not $publisherVerify) {
        $keep = @()
        foreach ($seg in $pathForChild.Split(';')) {
            if ($seg -ne '' -and ([System.IO.File]::Exists([System.IO.Path]::Combine($seg, 'gh.exe')) -or [System.IO.File]::Exists([System.IO.Path]::Combine($seg, 'gh.cmd')))) { continue }
            $keep += $seg
        }
        $pathForChild = ($keep -join ';')
    }
    $savedPath = $env:Path
    $savedBase = $env:SEMAPRAX_INSTALL_DOWNLOAD_BASE
    $env:Path = $pathForChild
    $env:SEMAPRAX_INSTALL_DOWNLOAD_BASE = (New-Object System.Uri $Base).AbsoluteUri
    try {
        $a = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', $script:InstallScript) + $Extra
        $r = Invoke-Native -FilePath $hostExe -Arguments $a
    } finally {
        $env:Path = $savedPath
        $env:SEMAPRAX_INSTALL_DOWNLOAD_BASE = $savedBase
    }
    Write-Host ($r.Output.TrimEnd())
    if ($r.Error.Trim() -ne '') { Write-Host ($r.Error.TrimEnd()) }
    return $r
}

$script:OriginalPath = $env:Path
$assets = [System.IO.Path]::GetFullPath($AssetsDir)
$script:InstallScript = [System.IO.Path]::Combine($assets, 'install.ps1')
$work = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), 'spx journey ' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
$installDir = [System.IO.Path]::Combine($work, 'install dir')
$binDir = [System.IO.Path]::Combine($installDir, 'bin')
$exitCode = 1
$pathEntryAdded = $false

try {
    Assert-Journey ([System.IO.File]::Exists($script:InstallScript)) "install.ps1 not found in $assets"
    $zipName = "$topLevel.zip"
    Assert-Journey ([System.IO.File]::Exists([System.IO.Path]::Combine($assets, $zipName))) "$zipName not found in $assets"
    [void][System.IO.Directory]::CreateDirectory($work)

    # Lay the flat asset directory out as <base>/<tag>/<asset>.
    $base = [System.IO.Path]::Combine($work, 'download base')
    $tagDir = [System.IO.Path]::Combine($base, $Tag)
    [void][System.IO.Directory]::CreateDirectory($tagDir)
    foreach ($f in [System.IO.Directory]::GetFiles($assets)) {
        [System.IO.File]::Copy($f, [System.IO.Path]::Combine($tagDir, [System.IO.Path]::GetFileName($f)), $true)
    }

    $installArgs = @('-Version', $Tag, '-InstallDir', $installDir)
    if (-not $modifyPath) { $installArgs += '-NoModifyPath' }
    if ($publisherVerify) { $installArgs += '-RequirePublisherVerification' }

    Step "installing $Tag into '$installDir' (path contains a space; modify PATH: $modifyPath; publisher verification: $publisherVerify)"
    $r = Invoke-Installer -Base $base -Extra $installArgs
    Assert-Journey ($r.ExitCode -eq 0) "install.ps1 exited $($r.ExitCode)"
    Assert-Journey ($r.Output -match 'installed to') 'success line printed'
    if ($publisherVerify) { Assert-Journey ($r.Output -match 'publisher: verified') 'publisher verified' }

    if ($modifyPath) {
        $count = @((Get-UserPathRaw).Split(';') | Where-Object { $_.TrimEnd('\') -ieq $binDir }).Count
        Assert-Journey ($count -eq 1) "user PATH holds the install bin exactly once (found $count)"
        $pathEntryAdded = $true
    }

    Set-SessionPath -BinDir $binDir
    $resolved = (Get-Command semaprax -CommandType Application -ErrorAction Stop | Select-Object -First 1).Source
    Assert-Journey ($resolved.StartsWith($installDir, [System.StringComparison]::OrdinalIgnoreCase)) "Get-Command semaprax resolves inside the install dir (got $resolved)"
    Step "semaprax resolves to $resolved"

    $zipPath = [System.IO.Path]::Combine($assets, $zipName)
    foreach ($exe in 'semaprax.exe', 'semapraxd.exe') {
        $want = Get-ZipMemberSha256 -ZipPath $zipPath -Member "$topLevel/$exe"
        Assert-Journey ((Get-Sha256OfFile ([System.IO.Path]::Combine($binDir, $exe))) -eq $want) "bin\$exe hash equals the zip member"
        Assert-Journey ((Get-Sha256OfFile ([System.IO.Path]::Combine($installDir, 'versions', $Tag, $exe))) -eq $want) "versions\$Tag\$exe hash equals the zip member"
    }
    Step 'both executables hash-equal the zip members'
    $receiptPath = [System.IO.Path]::Combine($installDir, 'install-receipt.json')
    Assert-Journey ([System.IO.File]::Exists($receiptPath)) 'receipt written'
    $receipt = [System.IO.File]::ReadAllText($receiptPath) | ConvertFrom-Json
    Assert-Journey ($receipt.tag -ceq $Tag -and $receipt.archive_sha256 -ceq (Get-Sha256OfFile $zipPath)) 'receipt tag and archive digest'

    # The literal beginner journey, from a fresh directory with no checkout.
    $cwd = [System.IO.Path]::Combine($work, 'fresh project dir')
    [void][System.IO.Directory]::CreateDirectory($cwd)
    $journey = @(
        @('--version'),
        @('new', 'first-semaprax'),
        @('check', 'first-semaprax/semaprax.toml'),
        @('test', 'first-semaprax/semaprax.toml'),
        @('run', 'first-semaprax/semaprax.toml')
    )
    $last = $null
    foreach ($j in $journey) {
        $res = Invoke-Native -FilePath $resolved -Arguments $j -WorkingDirectory $cwd
        if ($res.ExitCode -ne 0) { Fail "semaprax $($j -join ' ') exited $($res.ExitCode):`n$($res.Output)`n$($res.Error)" }
        $last = $res
        Step "semaprax $($j -join ' ') ok"
    }
    Assert-Journey ($last.Output.Trim().Split("`n")[-1].Trim() -eq '42') "semaprax run printed 42 (got: $($last.Output.Trim()))"
    $ver = Invoke-Native -FilePath $resolved -Arguments @('version', '--json') -WorkingDirectory $cwd
    Assert-Journey (($ver.Output | ConvertFrom-Json).version -ceq $version) "semaprax version --json reports $version"

    # Same-version reinstall is idempotent.
    $before = Get-Sha256OfFile ([System.IO.Path]::Combine($binDir, 'semaprax.exe'))
    $r = Invoke-Installer -Base $base -Extra $installArgs
    Assert-Journey ($r.ExitCode -eq 0) "same-version reinstall exited $($r.ExitCode)"
    Assert-Journey ($r.Output -match 'already installed and up to date') 'reinstall reports up to date'
    Assert-Journey ((Get-Sha256OfFile ([System.IO.Path]::Combine($binDir, 'semaprax.exe'))) -eq $before) 'reinstall left the executable unchanged'
    if ($modifyPath) {
        $count = @((Get-UserPathRaw).Split(';') | Where-Object { $_.TrimEnd('\') -ieq $binDir }).Count
        Assert-Journey ($count -eq 1) "user PATH still holds the install bin exactly once after reinstall (found $count)"
    }
    Step 'same-version reinstall is idempotent'

    # Damaged-zip control: the published checksum no longer matches.
    $badBase = [System.IO.Path]::Combine($work, 'damaged base')
    $badTagDir = [System.IO.Path]::Combine($badBase, $Tag)
    [void][System.IO.Directory]::CreateDirectory($badTagDir)
    foreach ($f in [System.IO.Directory]::GetFiles($tagDir)) {
        [System.IO.File]::Copy($f, [System.IO.Path]::Combine($badTagDir, [System.IO.Path]::GetFileName($f)), $true)
    }
    $badZip = [System.IO.Path]::Combine($badTagDir, $zipName)
    $bytes = [System.IO.File]::ReadAllBytes($badZip)
    $idx = [int]($bytes.Length / 2)
    $bytes[$idx] = [byte]($bytes[$idx] -bxor 0xFF)
    [System.IO.File]::WriteAllBytes($badZip, $bytes)
    $r = Invoke-Installer -Base $badBase -Extra $installArgs
    Assert-Journey ($r.ExitCode -ne 0) 'damaged zip: nonzero exit'
    Assert-Journey ($r.Output -notmatch 'installed to') 'damaged zip: no success line'
    Assert-Journey ($r.Output -match 'checksum mismatch') 'damaged zip: checksum mismatch reported'
    Assert-Journey ((Get-Sha256OfFile ([System.IO.Path]::Combine($binDir, 'semaprax.exe'))) -eq $before) 'damaged zip: installed executable unchanged'
    $still = Invoke-Native -FilePath $resolved -Arguments @('--version') -WorkingDirectory $cwd
    Assert-Journey ($still.ExitCode -eq 0) 'damaged zip: old install still runs'
    Assert-Journey ([System.IO.File]::Exists($receiptPath)) 'damaged zip: receipt still present'
    Step 'damaged-zip control: nonzero, old install intact'

    # Uninstall removes the owned files and the PATH entry.
    $r = Invoke-Installer -Base $base -Extra @('-Uninstall', '-InstallDir', $installDir)
    Assert-Journey ($r.ExitCode -eq 0) "uninstall exited $($r.ExitCode)"
    Assert-Journey (-not [System.IO.File]::Exists([System.IO.Path]::Combine($binDir, 'semaprax.exe'))) 'uninstall removed bin\semaprax.exe'
    Assert-Journey (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'versions'))) 'uninstall removed versions'
    Assert-Journey (-not [System.IO.File]::Exists($receiptPath)) 'uninstall removed the receipt'
    if ($modifyPath) {
        $count = @((Get-UserPathRaw).Split(';') | Where-Object { $_.TrimEnd('\') -ieq $binDir }).Count
        Assert-Journey ($count -eq 0) 'uninstall removed the user PATH entry'
        $pathEntryAdded = $false
    }
    Step 'uninstall removed only the installation'

    $exitCode = 0
} catch {
    Write-Host ("journey: FAIL: " + $_.Exception.Message)
} finally {
    $env:Path = $script:OriginalPath
    if ($pathEntryAdded) {
        # Best-effort cleanup of the PATH entry when the journey failed midway.
        try { [void](Invoke-Installer -Base ([System.IO.Path]::Combine($work, 'download base')) -Extra @('-Uninstall', '-InstallDir', $installDir)) } catch { }
    }
    try { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue } catch { }
}

if ($exitCode -eq 0) {
    Write-Host "install-journey: PASS ($Tag, $($steps.Count) steps, modify PATH: $modifyPath)"
} else {
    Write-Host "install-journey: FAIL ($Tag, $($steps.Count) steps completed)"
}
exit $exitCode
