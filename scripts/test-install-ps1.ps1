<#
.SYNOPSIS
    Native tests for scripts/install.ps1 (no Pester). Exits nonzero on any failure.

.DESCRIPTION
    Pure-function tests run on any OS. End-to-end tests build a fixture release
    in a temp directory (stub executables compiled with csc.exe) and run the
    real install.ps1 in a child PowerShell of the SAME edition as this script;
    they run only on Windows and print a SKIP line elsewhere. The end-to-end
    tests redirect the user-PATH registry value to a throwaway HKCU subkey via
    SEMAPRAX_INSTALL_ENV_SUBKEY, so a real PATH is never touched.

    Run: powershell -NoProfile -File scripts\test-install-ps1.ps1
         pwsh -NoProfile -File scripts/test-install-ps1.ps1
#>
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
try { Add-Type -AssemblyName System.IO.Compression; Add-Type -AssemblyName System.IO.Compression.FileSystem } catch { }
$script:Passed = 0
$script:Failed = 0
$script:Skipped = 0
$IsWindowsHost = ([Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT)
$installPs1 = [System.IO.Path]::GetFullPath([System.IO.Path]::Combine($PSScriptRoot, 'install.ps1'))

# Dot-source the installer's functions without running its main.
$env:SEMAPRAX_INSTALL_PS1_NO_MAIN = '1'
. $installPs1
Remove-Item Env:\SEMAPRAX_INSTALL_PS1_NO_MAIN

# -------------------------------------------------------------- harness

function Test-Case {
    param([string]$Name, [scriptblock]$Body)
    try {
        & $Body
        $script:Passed++
        Write-Host "PASS  $Name"
    } catch {
        $script:Failed++
        Write-Host "FAIL  $Name"
        Write-Host ("      " + $_.Exception.Message)
    }
}

function Skip-Case {
    param([string]$Name, [string]$Reason)
    $script:Skipped++
    Write-Host "SKIP  $Name ($Reason)"
}

function Assert-True {
    param($Condition, [string]$Message)
    if (-not $Condition) { throw "assertion failed: $Message" }
}

function Assert-Equal {
    param($Actual, $Expected, [string]$Message)
    if ($Actual -cne $Expected) { throw "assertion failed: $Message`n  expected: $Expected`n  actual:   $Actual" }
}

function Assert-Throws {
    param([scriptblock]$Body, [string]$Pattern, [string]$Message)
    $msg = $null
    try { & $Body } catch { $msg = $_.Exception.Message }
    if ($null -eq $msg) { throw "assertion failed: expected an error ($Message)" }
    if ($msg -notmatch $Pattern) { throw "assertion failed: error did not match /$Pattern/ ($Message)`n  actual: $msg" }
}

function New-TempDir {
    param([string]$Suffix = '')
    $p = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), 'spx-ps1-' + [Guid]::NewGuid().ToString('N').Substring(0, 10) + $Suffix)
    [void][System.IO.Directory]::CreateDirectory($p)
    return $p
}

function New-ZipFromEntries {
    # $Entries: ordered list of @{ Name; Text } ; optional Mode for symlink tests.
    param([string]$Path, $Entries)
    try { Add-Type -AssemblyName System.IO.Compression; Add-Type -AssemblyName System.IO.Compression.FileSystem } catch { }
    $fs = [System.IO.File]::Create($Path)
    try {
        $zip = New-Object System.IO.Compression.ZipArchive($fs, [System.IO.Compression.ZipArchiveMode]::Create)
        try {
            foreach ($e in $Entries) {
                $entry = $zip.CreateEntry($e.Name)
                if ($e.ContainsKey('Mode')) { $entry.ExternalAttributes = [int]$e.Mode }
                $w = New-Object System.IO.StreamWriter($entry.Open())
                try { $w.Write($e.Text) } finally { $w.Dispose() }
            }
        } finally { $zip.Dispose() }
    } finally { $fs.Dispose() }
}

$x64 = @{ ProcArch = 'AMD64'; ProcArchW6432 = $null; OsArch = 'X64'; NativeMachine = 'AMD64' }

# ------------------------------------------------------- pure tests

Test-Case 'supported targets equal the contract entry' {
    $t = @($script:SemapraxTargets)
    Assert-Equal $t.Count 1 'exactly one Windows target'
    Assert-Equal $t[0].Target 'x86_64-pc-windows-msvc' 'target'
    Assert-Equal $t[0].Extension 'zip' 'extension'
    $reconcile = [System.IO.Path]::Combine($PSScriptRoot, 'release-reconcile.py')
    if ([System.IO.File]::Exists($reconcile)) {
        $text = [System.IO.File]::ReadAllText($reconcile)
        Assert-True ($text -match '\("x86_64-pc-windows-msvc", "zip"\)') 'release-reconcile.py ARCHIVE_TARGETS has the same windows entry'
    }
}

Test-Case 'parameter binding: invalid -Version fails before any work (child process)' {
    $hostExe = (Get-Process -Id $PID).Path
    $r = Invoke-SemapraxNative -FilePath $hostExe -Arguments @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', $installPs1, '-Version', 'not-a-version', '-InstallDir', 'x y') -TimeoutSeconds 120
    Assert-True ($r.ExitCode -ne 0) "nonzero exit (got $($r.ExitCode))"
    Assert-True (($r.Output + $r.Error) -match 'invalid version') "message mentions invalid version: $($r.Output) $($r.Error)"
}

Test-Case 'iex mode: env fallback is read, failure throws instead of exiting the host' {
    $hostExe = (Get-Process -Id $PID).Path
    $cmd = "`$env:SEMAPRAX_INSTALL_VERSION='bogus'; try { Invoke-Expression ([System.IO.File]::ReadAllText('$($installPs1.Replace("'", "''"))')) } catch { Write-Host 'CAUGHT' }; Write-Host 'ALIVE'"
    $r = Invoke-SemapraxNative -FilePath $hostExe -Arguments @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', $cmd) -TimeoutSeconds 120
    Assert-True ($r.Output -match 'invalid version') "env version was read: $($r.Output)"
    Assert-True ($r.Output -match 'CAUGHT') 'failure surfaced as an exception'
    Assert-True ($r.Output -match 'ALIVE') 'host session survived'
}

Test-Case 'Resolve-SemapraxOptions: tag normalisation, env fallback, flags' {
    $saved = @{}
    foreach ($k in 'SEMAPRAX_INSTALL_VERSION', 'SEMAPRAX_INSTALL_DIR', 'SEMAPRAX_INSTALL_NO_MODIFY_PATH', 'SEMAPRAX_INSTALL_UNINSTALL', 'SEMAPRAX_INSTALL_REQUIRE_PUBLISHER_VERIFICATION') {
        $saved[$k] = [Environment]::GetEnvironmentVariable($k)
        [Environment]::SetEnvironmentVariable($k, $null)
    }
    try {
        $o = Resolve-SemapraxOptions -Version '0.9.0' -InstallDir 'a b' -NoModifyPath $false -Uninstall $false -RequirePublisherVerification $false
        Assert-Equal $o.Tag 'v0.9.0' 'v prefix added'
        Assert-Equal $o.InstallDir 'a b' 'dir passthrough'
        Assert-True (-not $o.NoModifyPath -and -not $o.Uninstall -and -not $o.Require) 'flags default off'
        $env:SEMAPRAX_INSTALL_VERSION = 'v1.2.3-rc.1'
        $env:SEMAPRAX_INSTALL_DIR = 'from env'
        $env:SEMAPRAX_INSTALL_NO_MODIFY_PATH = '1'
        $env:SEMAPRAX_INSTALL_UNINSTALL = 'true'
        $env:SEMAPRAX_INSTALL_REQUIRE_PUBLISHER_VERIFICATION = 'yes'
        $o = Resolve-SemapraxOptions -Version '' -InstallDir '' -NoModifyPath $false -Uninstall $false -RequirePublisherVerification $false
        Assert-Equal $o.Tag 'v1.2.3-rc.1' 'env version'
        Assert-Equal $o.InstallDir 'from env' 'env dir'
        Assert-True ($o.NoModifyPath -and $o.Uninstall -and $o.Require) 'env flags'
        $o = Resolve-SemapraxOptions -Version 'v2.0.0' -InstallDir 'param wins' -NoModifyPath $false -Uninstall $false -RequirePublisherVerification $false
        Assert-Equal $o.Tag 'v2.0.0' 'param beats env'
        Assert-Equal $o.InstallDir 'param wins' 'param dir beats env'
        foreach ($bad in 'latest', 'v1', '../v1.0.0', 'v1.0.0/../x', 'v1.0.0 ', 'v1.0.0;rm') {
            if ($bad -eq 'v1.0.0 ') { continue } # surrounding whitespace is trimmed, not an error
            Assert-Throws { [void](ConvertTo-SemapraxTag $bad) } 'invalid version' "rejects '$bad'"
        }
    } finally {
        foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
    }
}

Test-Case 'SHA256SUMS parsing' {
    $a = ('a' * 64)
    $b = ('b' * 64)
    $text = "$a  semaprax-v1.0.0-x86_64-pc-windows-msvc.zip`n$b *other.tar.gz`r`n"
    Assert-Equal (Get-SemapraxChecksumFromText -Text $text -Name 'semaprax-v1.0.0-x86_64-pc-windows-msvc.zip') $a 'exact line'
    Assert-Equal (Get-SemapraxChecksumFromText -Text $text -Name 'other.tar.gz') $b 'binary marker and CRLF'
    Assert-Throws { Get-SemapraxChecksumFromText -Text $text -Name 'semaprax-v1.0.0' } 'no entry' 'prefix of a name is not a match'
    Assert-Throws { Get-SemapraxChecksumFromText -Text '' -Name 'x.zip' } 'no entry' 'empty file'
    Assert-Throws { Get-SemapraxChecksumFromText -Text "$a  x.zip`n$b  x.zip`n" -Name 'x.zip' } 'conflicting' 'conflicting duplicates'
    Assert-Equal (Get-SemapraxChecksumFromText -Text "$a  x.zip`n$a  x.zip`n" -Name 'x.zip') $a 'identical duplicates tolerated'
    Assert-Throws { Get-SemapraxChecksumFromText -Text "short  x.zip`n" -Name 'x.zip' } 'no entry' 'malformed digest'
    Assert-Equal (Get-SemapraxChecksumFromText -Text ("A" * 64 + "  x.zip") -Name 'x.zip') $a 'digest is lowercased'
}

Test-Case 'manifest artifact matching' {
    $name = 'semaprax-v1.0.0-x86_64-pc-windows-msvc.zip'
    $sha = ('c' * 64)
    $json = '{"schema":"semaprax.release-manifest.v1","tag":"v1.0.0","artifacts":[{"name":"other.tar.gz","platform":"x86_64-unknown-linux-gnu","size":1,"digest":"sha256:' + ('d' * 64) + '"},{"name":"' + $name + '","platform":"x86_64-pc-windows-msvc","size":123,"digest":"sha256:' + $sha + '"}]}'
    $m = $json | ConvertFrom-Json
    Assert-SemapraxManifestArtifact -Manifest $m -Tag 'v1.0.0' -Name $name -Target 'x86_64-pc-windows-msvc' -Size 123 -Sha256 $sha
    Assert-Throws { Assert-SemapraxManifestArtifact -Manifest $m -Tag 'v1.0.1' -Name $name -Target 'x86_64-pc-windows-msvc' -Size 123 -Sha256 $sha } 'not v1.0.1' 'tag mismatch'
    Assert-Throws { Assert-SemapraxManifestArtifact -Manifest $m -Tag 'v1.0.0' -Name $name -Target 'x86_64-pc-windows-msvc' -Size 124 -Sha256 $sha } 'size' 'size mismatch'
    Assert-Throws { Assert-SemapraxManifestArtifact -Manifest $m -Tag 'v1.0.0' -Name $name -Target 'x86_64-pc-windows-msvc' -Size 123 -Sha256 ('e' * 64) } 'digest' 'digest mismatch'
    Assert-Throws { Assert-SemapraxManifestArtifact -Manifest $m -Tag 'v1.0.0' -Name $name -Target 'aarch64-apple-darwin' -Size 123 -Sha256 $sha } 'platform' 'platform mismatch'
    Assert-Throws { Assert-SemapraxManifestArtifact -Manifest $m -Tag 'v1.0.0' -Name 'missing.zip' -Target 'x86_64-pc-windows-msvc' -Size 1 -Sha256 $sha } 'exactly once' 'missing artifact'
    Assert-Throws { Assert-SemapraxManifestArtifact -Manifest $null -Tag 'v1.0.0' -Name $name -Target 'x86_64-pc-windows-msvc' -Size 1 -Sha256 $sha } 'malformed' 'null manifest'
    $bad = ('{"schema":"other","tag":"v1.0.0","artifacts":[]}' | ConvertFrom-Json)
    Assert-Throws { Assert-SemapraxManifestArtifact -Manifest $bad -Tag 'v1.0.0' -Name $name -Target 'x86_64-pc-windows-msvc' -Size 1 -Sha256 $sha } 'schema' 'wrong schema'
    $dup = ($json.Replace('"artifacts":[', '"artifacts":[{"name":"' + $name + '","platform":"x86_64-pc-windows-msvc","size":123,"digest":"sha256:' + $sha + '"},')) | ConvertFrom-Json
    Assert-Throws { Assert-SemapraxManifestArtifact -Manifest $dup -Tag 'v1.0.0' -Name $name -Target 'x86_64-pc-windows-msvc' -Size 123 -Sha256 $sha } 'exactly once' 'duplicate entry'
    $old = ('{"schema":"semaprax.release-manifest.v1","tag":"v0.8.0","artifacts":[{"name":"' + $name + '","platform":"x86_64-pc-windows-msvc","size":123,"digest":"sha256:' + $sha + '"}],"installers":[{"name":"install.ps1","size":1,"digest":"sha256:' + $sha + '"}]}') | ConvertFrom-Json
    Assert-SemapraxManifestArtifact -Manifest $old -Tag 'v0.8.0' -Name $name -Target 'x86_64-pc-windows-msvc' -Size 123 -Sha256 $sha
}

Test-Case 'target selection and ARM64 refusal through injected architecture' {
    Assert-Equal (Resolve-SemapraxTarget -Arch $x64) 'x86_64-pc-windows-msvc' 'x64'
    Assert-Equal (Resolve-SemapraxTarget -Arch @{ ProcArch = 'AMD64' }) 'x86_64-pc-windows-msvc' 'only process arch known'
    Assert-Equal (Resolve-SemapraxTarget -Arch @{ ProcArch = 'x86'; ProcArchW6432 = 'AMD64' }) 'x86_64-pc-windows-msvc' '32-bit host on x64 (WOW64)'
    $arm = @(
        @{ ProcArch = 'ARM64'; ProcArchW6432 = $null; OsArch = 'Arm64'; NativeMachine = 'ARM64' },
        @{ ProcArch = 'AMD64'; ProcArchW6432 = $null; OsArch = 'Arm64'; NativeMachine = $null },
        @{ ProcArch = 'AMD64'; ProcArchW6432 = $null; OsArch = 'X64'; NativeMachine = 'ARM64' },
        @{ ProcArch = 'x86'; ProcArchW6432 = 'ARM64'; OsArch = $null; NativeMachine = $null }
    )
    foreach ($a in $arm) {
        $msg = $null
        try { [void](Resolve-SemapraxTarget -Arch $a -Tag 'v0.9.0') } catch { $msg = $_.Exception.Message }
        Assert-True ($null -ne $msg) 'ARM64 is refused'
        Assert-True ($msg -match 'ARM64') 'message names ARM64'
        Assert-True ($msg -match 'does not assume that x64 emulation works') 'no silent emulation claim'
        Assert-True ($msg -match 'cargo install --locked --git https://github.com/wavect/semaprax --tag v0.9.0 semaprax-toolchain --bin semaprax-full') 'source alternative'
    }
    Assert-Throws { [void](Resolve-SemapraxTarget -Arch @{ ProcArch = 'x86' }) } 'unsupported Windows architecture' 'x86 refused'
    Assert-Throws { [void](Resolve-SemapraxTarget -Arch @{}) } 'could not determine' 'no signal refused'
}

Test-Case 'zip entry name validation' {
    $top = 'semaprax-v1.0.0-x86_64-pc-windows-msvc'
    foreach ($ok in "$top/", "$top/semaprax.exe", "$top/smoke/meaning.spx", "$top\semaprax.exe") {
        Assert-Equal ([string](Test-SemapraxZipEntryName -Name $ok -TopLevel $top)) '' "accepts $ok"
    }
    foreach ($bad in '', '/etc/passwd', '\evil.exe', 'C:\evil.exe', "$top/../evil.exe", "$top\..\..\evil.exe", '../evil', "$top/a/../../b", "$top/file.exe:stream", 'other-top/semaprax.exe', "$top/CON", "$top/nul.txt", "$top//x", "$top/x.", "$top/./x", "$top/a`0b") {
        Assert-True ($null -ne (Test-SemapraxZipEntryName -Name $bad -TopLevel $top)) "refuses '$bad'"
    }
}

Test-Case 'zip extraction refuses hostile archives and extracts good ones' {
    $root = New-TempDir
    try {
        $top = 'semaprax-v1.0.0-x86_64-pc-windows-msvc'
        $hostile = @(
            @{ Name = 'traversal'; Entries = @(@{ Name = "$top/semaprax.exe"; Text = 'x' }, @{ Name = "$top/../evil.txt"; Text = 'evil' }) },
            @{ Name = 'absolute'; Entries = @(@{ Name = '/tmp/evil.txt'; Text = 'evil' }) },
            @{ Name = 'backslash traversal'; Entries = @(@{ Name = "$top\..\..\evil.txt"; Text = 'evil' }) },
            @{ Name = 'drive'; Entries = @(@{ Name = 'C:/evil.txt'; Text = 'evil' }) },
            @{ Name = 'wrong top'; Entries = @(@{ Name = 'other/semaprax.exe'; Text = 'x' }) },
            @{ Name = 'duplicate'; Entries = @(@{ Name = "$top/a"; Text = '1' }, @{ Name = "$top/A"; Text = '2' }) }
        )
        foreach ($h in $hostile) {
            $zip = [System.IO.Path]::Combine($root, ($h.Name -replace ' ', '_') + '.zip')
            New-ZipFromEntries -Path $zip -Entries $h.Entries
            $dest = [System.IO.Path]::Combine($root, 'out-' + ($h.Name -replace ' ', '_'))
            Assert-Throws { [void](Expand-SemapraxZip -ZipPath $zip -Destination $dest -TopLevel $top) } 'refusing archive' "hostile: $($h.Name)"
            Assert-True (-not [System.IO.File]::Exists([System.IO.Path]::Combine($root, 'evil.txt'))) "nothing escaped ($($h.Name))"
            if ($h.Name -eq 'traversal') {
                Assert-True (-not [System.IO.File]::Exists([System.IO.Path]::Combine($dest, $top, 'semaprax.exe'))) 'validation happens before any extraction'
            }
        }
        # symlink member (Unix mode 0120000 in the high 16 bits)
        $zip = [System.IO.Path]::Combine($root, 'symlink.zip')
        $symlinkOk = $true
        try { New-ZipFromEntries -Path $zip -Entries @(@{ Name = "$top/link"; Text = 'target'; Mode = [BitConverter]::ToInt32([BitConverter]::GetBytes([uint32]2684354560), 0) }) } catch { $symlinkOk = $false }
        if ($symlinkOk) {
            Assert-Throws { [void](Expand-SemapraxZip -ZipPath $zip -Destination ([System.IO.Path]::Combine($root, 'out-symlink')) -TopLevel $top) } 'symbolic link' 'symlink member'
        }
        # not a zip at all
        $junk = [System.IO.Path]::Combine($root, 'junk.zip')
        [System.IO.File]::WriteAllText($junk, 'this is not a zip')
        Assert-Throws { [void](Expand-SemapraxZip -ZipPath $junk -Destination ([System.IO.Path]::Combine($root, 'out-junk')) -TopLevel $top) } 'not a readable zip' 'garbage'
        # good archive, backslash and slash styles
        $good = [System.IO.Path]::Combine($root, 'good.zip')
        New-ZipFromEntries -Path $good -Entries @(@{ Name = "$top/"; Text = '' }, @{ Name = "$top/semaprax.exe"; Text = 'A' }, @{ Name = "$top\semapraxd.exe"; Text = 'B' }, @{ Name = "$top/smoke/meaning.spx"; Text = 'C' })
        $files = @(Expand-SemapraxZip -ZipPath $good -Destination ([System.IO.Path]::Combine($root, 'out good')) -TopLevel $top)
        Assert-Equal $files.Count 3 'three files extracted'
        Assert-Equal ([System.IO.File]::ReadAllText([System.IO.Path]::Combine($root, 'out good', $top, 'smoke', 'meaning.spx'))) 'C' 'content'
        Assert-Equal ([System.IO.File]::ReadAllText([System.IO.Path]::Combine($root, 'out good', $top, 'semapraxd.exe'))) 'B' 'backslash member extracted at the right place'
    } finally { Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue }
}

Test-Case 'argument quoting for child processes' {
    Assert-Equal (ConvertTo-SemapraxArgString @('a', 'b')) 'a b' 'plain'
    Assert-Equal (ConvertTo-SemapraxArgString @('C:\Program Files\x', '--k')) '"C:\Program Files\x" --k' 'spaces'
    Assert-Equal (ConvertTo-SemapraxArgString @('')) '""' 'empty'
    Assert-Equal (ConvertTo-SemapraxArgString @('say "hi"')) '"say \"hi\""' 'quotes'
    Assert-Equal (ConvertTo-SemapraxArgString @('C:\a b\')) '"C:\a b\\"' 'trailing backslash doubled'
}

Test-Case 'PATH string add/remove is idempotent and preserves unrelated entries' {
    $entry = 'C:\Users\a b\AppData\Local\Programs\Semaprax\bin'
    $orig = '%SystemRoot%\system32;C:\Tools;;D:\x y\bin;'
    $added = Add-SemapraxPathEntry -Path $orig -Entry $entry
    Assert-Equal $added "$entry;$orig" 'prepended once, original text intact'
    Assert-Equal (Add-SemapraxPathEntry -Path $added -Entry $entry) $added 'second add is a no-op'
    Assert-Equal (Add-SemapraxPathEntry -Path ($orig + ';' + $entry.ToUpperInvariant() + '\') -Entry $entry) ($orig + ';' + $entry.ToUpperInvariant() + '\') 'case/trailing-slash duplicate recognised'
    Assert-Equal (Remove-SemapraxPathEntry -Path $added -Entry $entry) $orig 'remove restores the original exactly'
    Assert-Equal (Remove-SemapraxPathEntry -Path $orig -Entry $entry) $orig 'remove of absent entry is a no-op'
    Assert-Equal (Add-SemapraxPathEntry -Path '' -Entry $entry) $entry 'empty path'
    Assert-Equal (Add-SemapraxPathEntry -Path $null -Entry $entry) $entry 'null path'
    Assert-Equal (Remove-SemapraxPathEntry -Path $entry -Entry $entry) '' 'sole entry removed'
    $middle = "C:\one;$entry;C:\two"
    Assert-Equal (Remove-SemapraxPathEntry -Path $middle -Entry $entry) 'C:\one;C:\two' 'middle entry removed, order kept'
    $twice = "$entry;C:\one;$entry"
    Assert-Equal (Remove-SemapraxPathEntry -Path $twice -Entry $entry) 'C:\one' 'every copy of our own entry removed'
    $long = (1..400 | ForEach-Object { "C:\Some Long Directory Name $_\bin" }) -join ';'
    Assert-True ($long.Length -gt 8000) 'fixture path exceeds the 1024-character setx limit'
    $grown = Add-SemapraxPathEntry -Path $long -Entry $entry
    Assert-Equal $grown.Length ($long.Length + $entry.Length + 1) 'no truncation'
    Assert-Equal (Remove-SemapraxPathEntry -Path $grown -Entry $entry) $long 'long path round trip'
    Assert-True (-not (Test-SemapraxPathHasEntry -Path 'C:\a\bin2;C:\a' -Entry 'C:\a\bin')) 'prefix is not a match'
}

Test-Case 'receipt round trip and refusal of foreign or unsafe receipts' {
    $dir = New-TempDir ' r e'
    try {
        $files = @('bin/semaprax.exe', 'bin/semapraxd.exe', 'versions/v1.0.0/semaprax.exe')
        $text = New-SemapraxReceiptText -Tag 'v1.0.0' -Target 'x86_64-pc-windows-msvc' -Source 'https://example.invalid/a.zip' -ArchiveSha256 ('f' * 64) -Publisher 'not-verified' -Files $files -PathKind 'user-path'
        Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($dir, 'install-receipt.json')) -Text $text
        $bytes = [System.IO.File]::ReadAllBytes([System.IO.Path]::Combine($dir, 'install-receipt.json'))
        Assert-True (-not ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB)) 'no UTF-8 BOM'
        $r = Read-SemapraxReceipt -InstallDir $dir
        Assert-Equal $r.schema 'semaprax.install-receipt.v1' 'schema'
        Assert-Equal $r.installer 'install.ps1' 'installer'
        Assert-Equal $r.version '1.0.0' 'version'
        Assert-Equal $r.tag 'v1.0.0' 'tag'
        Assert-Equal $r.target 'x86_64-pc-windows-msvc' 'target'
        Assert-Equal $r.archive_sha256 ('f' * 64) 'sha'
        Assert-Equal $r.publisher_verification 'not-verified' 'publisher'
        Assert-Equal (@($r.files) -join '|') ($files -join '|') 'files'
        Assert-Equal $r.path_modification.kind 'user-path' 'kind'
        Assert-Equal $r.path_modification.location 'HKCU\Environment\Path' 'location'
        $none = New-SemapraxReceiptText -Tag 'v1.0.0' -Target 't' -Source 's' -ArchiveSha256 'x' -Publisher 'verified' -Files @('bin/semaprax.exe') -PathKind 'none'
        Assert-True ($none -match '"files":\s*\[') 'single file stays an array'
        Assert-True ($none -match '"location":\s*null') 'location null for none'
        Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($dir, 'install-receipt.json')) -Text ($text -replace 'install.ps1', 'install.sh')
        Assert-Throws { [void](Read-SemapraxReceipt -InstallDir $dir) } 'not install.ps1' 'foreign installer'
        Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($dir, 'install-receipt.json')) -Text ($text -replace 'bin/semapraxd.exe', '../evil.exe')
        Assert-Throws { [void](Read-SemapraxReceipt -InstallDir $dir) } 'unsafe' 'traversal in receipt files'
        Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($dir, 'install-receipt.json')) -Text '{not json'
        Assert-Throws { [void](Read-SemapraxReceipt -InstallDir $dir) } 'not valid JSON' 'garbage receipt'
        [System.IO.File]::Delete([System.IO.Path]::Combine($dir, 'install-receipt.json'))
        Assert-Equal ([string](Read-SemapraxReceipt -InstallDir $dir)) '' 'absent receipt is $null'
    } finally { Remove-Item -LiteralPath $dir -Recurse -Force -ErrorAction SilentlyContinue }
}

Test-Case 'asset URLs, file:// download and latest-tag resolution (spaces, non-ASCII)' {
    $root = New-TempDir (' sp ' + [char]0x00E9 + [char]0x4E2D)
    try {
        $tag = 'v1.2.3'
        $d = [System.IO.Path]::Combine($root, $tag)
        [void][System.IO.Directory]::CreateDirectory($d)
        [System.IO.File]::WriteAllText([System.IO.Path]::Combine($d, 'a b.txt'), 'hello')
        foreach ($base in @($root, (New-Object System.Uri $root).AbsoluteUri)) {
            $url = Get-SemapraxAssetUrl -Base $base -Tag $tag -Name 'a b.txt'
            Assert-True ($url -match '^file:') 'file URL'
            $out = [System.IO.Path]::Combine($root, 'copy.txt')
            Copy-SemapraxAsset -Url $url -OutFile $out
            Assert-Equal ([System.IO.File]::ReadAllText($out)) 'hello' 'copied through file://'
        }
        Assert-Throws { Copy-SemapraxAsset -Url (Get-SemapraxAssetUrl -Base $root -Tag $tag -Name 'missing') -OutFile ([System.IO.Path]::Combine($root, 'x')) } 'download failed' 'missing asset'
        Assert-Equal (Resolve-SemapraxLatestTag -LatestUrl ((New-Object System.Uri $root).AbsoluteUri.TrimEnd('/') + '/releases/tag/v3.4.5')) 'v3.4.5' 'file URL with /tag/<tag>'
        $tagFile = [System.IO.Path]::Combine($root, 'latest.txt')
        [System.IO.File]::WriteAllText($tagFile, "3.4.6`n")
        Assert-Equal (Resolve-SemapraxLatestTag -LatestUrl (New-Object System.Uri $tagFile).AbsoluteUri) 'v3.4.6' 'file URL naming a tag file'
        Assert-Throws { [void](Resolve-SemapraxLatestTag -LatestUrl ((New-Object System.Uri $root).AbsoluteUri)) } 'cannot resolve' 'unresolvable file URL'
    } finally { Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue }
}

Test-Case 'user-facing text names the exact current-session command and source alternative' {
    $src = [System.IO.File]::ReadAllText($installPs1)
    Assert-True ($src.Contains('$env:Path = "')) 'session command template present'
    Assert-True ($src.Contains('semaprax-toolchain --bin semaprax-full')) 'full-build source alternative present'
    Assert-True (-not ($src -match '(?i)setx')) 'no setx'
    Assert-True (-not ($src -match 'Set-ExecutionPolicy')) 'does not change execution policy'
    Assert-True (-not ($src -match 'Stop-Process|taskkill')) 'never kills processes'
}

# ---------------------------------- orchestration tests (any OS, mocked)
# The architecture probe and the staged-binary smoke test are mocked so the
# download -> verify -> extract -> activate -> receipt -> uninstall flow runs
# on any OS with text "executables". User PATH is never touched (-NoModifyPath).

function New-MockRelease {
    param([string]$Root, [string]$Tag, [switch]$CorruptZip, [switch]$NoAttestation, [switch]$BadChecksum)
    $target = 'x86_64-pc-windows-msvc'
    $top = "semaprax-$Tag-$target"
    $out = [System.IO.Path]::Combine($Root, $Tag)
    [void][System.IO.Directory]::CreateDirectory($out)
    $zipName = "$top.zip"
    $zipPath = [System.IO.Path]::Combine($out, $zipName)
    $ver = $Tag.TrimStart('v')
    New-ZipFromEntries -Path $zipPath -Entries @(
        @{ Name = "$top/semaprax.exe"; Text = "VER:$ver" },
        @{ Name = "$top/semapraxd.exe"; Text = "DAEMON:$ver" },
        @{ Name = "$top/LICENSE"; Text = 'license' },
        @{ Name = "$top/smoke/meaning.spx"; Text = 'module app;' }
    )
    if ($CorruptZip) { [System.IO.File]::WriteAllText($zipPath, 'PK not really a zip') }
    $sha = Get-SemapraxFileSha256 $zipPath
    $size = ([System.IO.FileInfo]$zipPath).Length
    $sumSha = $sha
    if ($BadChecksum) { $sumSha = ('0' * 64) }
    Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($out, 'SHA256SUMS')) -Text "$sumSha  $zipName`n"
    Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($out, 'release-manifest.json')) -Text ('{"schema":"semaprax.release-manifest.v1","tag":"' + $Tag + '","artifacts":[{"name":"' + $zipName + '","platform":"' + $target + '","size":' + $size + ',"digest":"sha256:' + $sha + '"}]}')
    if (-not $NoAttestation) {
        Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($out, "release-attestation-$target.json")) -Text '{"fixture":true}'
    }
}

function Invoke-MockedInstall {
    param([string]$Root, [string]$InstallDir, [string]$Tag)
    function Get-SemapraxArchInputs { return @{ ProcArch = 'AMD64'; ProcArchW6432 = $null; OsArch = 'X64'; NativeMachine = 'AMD64' } }
    function Invoke-SemapraxNative {
        param([string]$FilePath, [string[]]$Arguments = @(), [int]$TimeoutSeconds = 120)
        $ver = ([System.IO.File]::ReadAllText($FilePath)) -replace '^VER:', ''
        return [pscustomobject]@{ ExitCode = 0; Output = ('{"version":"' + $ver.Trim() + '"}'); Error = ''; TimedOut = $false; Started = $true }
    }
    $savedBase = $env:SEMAPRAX_INSTALL_DOWNLOAD_BASE
    $savedPath = $env:PATH
    $env:SEMAPRAX_INSTALL_DOWNLOAD_BASE = $Root
    $env:PATH = [System.IO.Path]::GetTempPath() # hides any real gh: checksum-only mode
    try {
        $opts = @{ Tag = $Tag; InstallDir = $InstallDir; NoModifyPath = $true; Require = $false }
        return (& { Invoke-SemapraxInstall -Options $opts } 6>&1)
    } finally {
        $env:SEMAPRAX_INSTALL_DOWNLOAD_BASE = $savedBase
        $env:PATH = $savedPath
    }
}

Test-Case 'orchestration: install, reinstall, upgrade, receipt, prune, uninstall' {
    $work = New-TempDir
    try {
        $rel = [System.IO.Path]::Combine($work, 'rel')
        New-MockRelease -Root $rel -Tag 'v1.0.0'
        New-MockRelease -Root $rel -Tag 'v1.1.0'
        $dir = [System.IO.Path]::Combine($work, 'Sem prax ' + [char]0x00E9 + [char]0x4E2D)
        $out = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag 'v1.0.0'
        $text = ($out | ForEach-Object { [string]$_ }) -join "`n"
        Assert-True ($text -match 'publisher: not verified \(gh not found\) - checksum only') "honest checksum-only line: $text"
        Assert-True ($text.Contains('$env:Path = "' + $dir + '\bin;" + $env:Path')) 'exact session command'
        Assert-True ($text -match 'installed to') 'success line'
        $bin = [System.IO.Path]::Combine($dir, 'bin')
        Assert-Equal ([System.IO.File]::ReadAllText([System.IO.Path]::Combine($bin, 'semaprax.exe'))) 'VER:1.0.0' 'bin semaprax'
        Assert-Equal ([System.IO.File]::ReadAllText([System.IO.Path]::Combine($bin, 'semapraxd.exe'))) 'DAEMON:1.0.0' 'bin semapraxd'
        Assert-True ([System.IO.File]::Exists([System.IO.Path]::Combine($dir, 'versions', 'v1.0.0', 'smoke', 'meaning.spx'))) 'package under versions'
        $rc = Read-SemapraxReceipt -InstallDir $dir
        Assert-Equal $rc.tag 'v1.0.0' 'receipt tag'
        Assert-Equal $rc.path_modification.kind 'none' 'NoModifyPath recorded'
        Assert-True (@($rc.files) -contains 'versions/v1.0.0/smoke/meaning.spx') 'receipt lists package files'
        Assert-True (@($rc.files) -contains 'bin/semaprax.exe') 'receipt lists bin files'
        Assert-Equal @([System.IO.Directory]::GetFileSystemEntries($dir) | Where-Object { [System.IO.Path]::GetFileName($_) -like '.staging-*' }).Count 0 'no staging left'
        # same-version reinstall: no swap
        $out = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag 'v1.0.0'
        Assert-True ((($out | ForEach-Object { [string]$_ }) -join "`n") -match 'already installed and up to date') 'idempotent'
        # upgrade
        $null = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag 'v1.1.0'
        Assert-Equal ([System.IO.File]::ReadAllText([System.IO.Path]::Combine($bin, 'semaprax.exe'))) 'VER:1.1.0' 'upgraded'
        Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($dir, 'versions', 'v1.0.0'))) 'old version pruned after activation'
        Assert-Equal (Read-SemapraxReceipt -InstallDir $dir).tag 'v1.1.0' 'receipt updated'
        # uninstall keeps unowned data
        [System.IO.File]::WriteAllText([System.IO.Path]::Combine($dir, 'mine.txt'), 'x')
        $code = @(Invoke-SemapraxUninstall -InstallDir $dir)[-1]
        Assert-Equal ([string]$code) '0' 'uninstall exit'
        Assert-True ([System.IO.File]::Exists([System.IO.Path]::Combine($dir, 'mine.txt'))) 'unowned file retained'
        Assert-Equal @([System.IO.Directory]::GetFileSystemEntries($dir)).Count 1 'only the unowned file remains'
        Remove-Item -LiteralPath ([System.IO.Path]::Combine($dir, 'mine.txt'))
        # empty dir + no receipt: uninstall is a no-op
        Assert-Equal ([string]@(Invoke-SemapraxUninstall -InstallDir $dir)[-1]) '0' 'uninstall of empty dir'
    } finally { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
}

Test-Case 'orchestration: failures leave the previous install untouched' {
    $work = New-TempDir
    try {
        $rel = [System.IO.Path]::Combine($work, 'rel')
        New-MockRelease -Root $rel -Tag 'v1.0.0'
        $dir = [System.IO.Path]::Combine($work, 'inst dir')
        $null = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag 'v1.0.0'
        $bin = [System.IO.Path]::Combine($dir, 'bin')
        $cases = @(
            @{ Name = 'v2.0.0'; Flag = 'CorruptZip'; Pattern = 'not a readable zip' },
            @{ Name = 'v2.0.1'; Flag = 'BadChecksum'; Pattern = 'checksum mismatch' },
            @{ Name = 'v2.0.2'; Flag = 'NoAttestation'; Pattern = 'download failed' }
        )
        foreach ($c in $cases) {
            if ($c.Flag -eq 'CorruptZip') { New-MockRelease -Root $rel -Tag $c.Name -CorruptZip }
            elseif ($c.Flag -eq 'BadChecksum') { New-MockRelease -Root $rel -Tag $c.Name -BadChecksum }
            else { New-MockRelease -Root $rel -Tag $c.Name -NoAttestation }
            Assert-Throws { $null = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag $c.Name } $c.Pattern $c.Flag
            Assert-Equal ([System.IO.File]::ReadAllText([System.IO.Path]::Combine($bin, 'semaprax.exe'))) 'VER:1.0.0' "old bin intact after $($c.Flag)"
            Assert-Equal (Read-SemapraxReceipt -InstallDir $dir).tag 'v1.0.0' "receipt intact after $($c.Flag)"
            Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($dir, 'versions', $c.Name))) "no version dir after $($c.Flag)"
        }
        Assert-Throws { $null = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag 'v9.9.9' } 'download failed' 'missing release'
        # fresh dir + failure leaves nothing behind
        $fresh = [System.IO.Path]::Combine($work, 'fresh')
        Assert-Throws { $null = Invoke-MockedInstall -Root $rel -InstallDir $fresh -Tag 'v2.0.0' } 'not a readable zip' 'fresh corrupt'
        Assert-True (-not [System.IO.Directory]::Exists($fresh)) 'no directory left by a failed fresh install'
        # unowned files: non-empty dir without receipt, and an unowned semaprax*.exe in bin
        $foreign = [System.IO.Path]::Combine($work, 'foreign')
        [void][System.IO.Directory]::CreateDirectory($foreign)
        [System.IO.File]::WriteAllText([System.IO.Path]::Combine($foreign, 'notes.txt'), 'mine')
        Assert-Throws { $null = Invoke-MockedInstall -Root $rel -InstallDir $foreign -Tag 'v1.0.0' } 'refusing to install over files' 'no receipt'
        Assert-True ([System.IO.File]::Exists([System.IO.Path]::Combine($foreign, 'notes.txt'))) 'foreign file untouched'
        Assert-Throws { $null = Invoke-SemapraxUninstall -InstallDir $foreign } 'no install-receipt.json' 'uninstall refuses without receipt'
        New-MockRelease -Root $rel -Tag 'v1.1.0'
        $imposter = [System.IO.Path]::Combine($bin, 'semaprax-other.exe')
        [System.IO.File]::WriteAllText($imposter, 'not ours')
        Assert-Throws { $null = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag 'v1.1.0' } 'was not installed by this installer' 'unowned bin file'
        Assert-True ([System.IO.File]::Exists($imposter)) 'unowned exe untouched'
        [System.IO.File]::Delete($imposter)
        # a locked installed executable: clear message, nothing changed, nothing killed
        $lock = [System.IO.File]::Open([System.IO.Path]::Combine($bin, 'semaprax.exe'), [System.IO.FileMode]::Open, [System.IO.FileAccess]::ReadWrite, [System.IO.FileShare]::None)
        try {
            Assert-Throws { $null = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag 'v1.1.0' } 'Close every running semaprax/semapraxd' 'locked binary'
        } finally { $lock.Dispose() }
        Assert-Equal ([System.IO.File]::ReadAllText([System.IO.Path]::Combine($bin, 'semaprax.exe'))) 'VER:1.0.0' 'bin intact after lock failure'
        Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($dir, 'versions', 'v1.1.0'))) 'no new version dir after lock failure'
        Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($dir, 'bin.new'))) 'no bin.new'
        $null = Invoke-MockedInstall -Root $rel -InstallDir $dir -Tag 'v1.1.0'
        Assert-Equal ([System.IO.File]::ReadAllText([System.IO.Path]::Combine($bin, 'semaprax.exe'))) 'VER:1.1.0' 'retry after unlock succeeds'
    } finally { Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue }
}

# -------------------------------------------------- Windows end-to-end

$e2eReason = $null
$cscPath = $null
if (-not $IsWindowsHost) {
    $e2eReason = 'not running on Windows'
} else {
    $cscCandidates = @(
        [System.IO.Path]::Combine($env:windir, 'Microsoft.NET', 'Framework64', 'v4.0.30319', 'csc.exe'),
        [System.IO.Path]::Combine($env:windir, 'Microsoft.NET', 'Framework', 'v4.0.30319', 'csc.exe')
    )
    foreach ($c in $cscCandidates) { if ([System.IO.File]::Exists($c)) { $cscPath = $c; break } }
    if ($null -eq $cscPath) { $e2eReason = 'csc.exe (.NET Framework compiler) not found' }
}

$e2eNames = @(
    'e2e: fresh install into a path with spaces and non-ASCII, PATH added once',
    'e2e: same-version reinstall is idempotent',
    'e2e: upgrade switches version, prunes the old one, PATH stays single',
    'e2e: corrupted zip, checksum mismatch, missing attestation, missing release',
    'e2e: gh present: verified / failing / required-but-missing',
    'e2e: locked binary keeps the old install, retry succeeds',
    'e2e: refuses unowned directory and unowned bin files',
    'e2e: -NoModifyPath leaves PATH alone',
    'e2e: iex mode installs from SEMAPRAX_INSTALL_* variables',
    'e2e: uninstall removes only owned files and PATH entry'
)

if ($null -ne $e2eReason) {
    foreach ($n in $e2eNames) { Skip-Case $n $e2eReason }
} else {
    $work = New-TempDir
    $subKey = 'Software\SemapraxInstallTest-' + [Guid]::NewGuid().ToString('N')
    $hostExe = (Get-Process -Id $PID).Path
    $releaseRoot = [System.IO.Path]::Combine($work, 'releases')
    [void][System.IO.Directory]::CreateDirectory($releaseRoot)
    $stubSource = @'
using System;
using System.Threading;
public static class Stub {
    public static int Main(string[] a) {
        if (a.Length > 0 && a[0] == "version") {
            Console.WriteLine("{\"schema\":\"semaprax.version.v1\",\"version\":\"@VER@\",\"commit\":null,\"maturity\":\"beta\",\"rust_min\":\"1.88\"}");
            return 0;
        }
        if (a.Length > 0 && a[0] == "sleep") { Thread.Sleep(180000); return 0; }
        if (a.Length > 0 && a[0] == "run") { Console.WriteLine("42"); return 0; }
        Console.WriteLine("@NAME@ @VER@");
        return 0;
    }
}
'@

    function New-StubExe {
        param([string]$Path, [string]$Ver, [string]$Name)
        $cs = $Path + '.cs'
        [System.IO.File]::WriteAllText($cs, $stubSource.Replace('@VER@', $Ver).Replace('@NAME@', $Name))
        $r = Invoke-SemapraxNative -FilePath $cscPath -Arguments @('/nologo', '/target:exe', ('/out:' + $Path), $cs) -TimeoutSeconds 120
        if ($r.ExitCode -ne 0 -or -not [System.IO.File]::Exists($Path)) { throw "stub compile failed: $($r.Output) $($r.Error)" }
    }

    function New-Fixture {
        # Builds <releaseRoot>\<tag>\{SHA256SUMS,release-manifest.json,<zip>,attestation}
        param([string]$Tag, [switch]$CorruptZip, [switch]$BadChecksum, [switch]$NoAttestation)
        $target = 'x86_64-pc-windows-msvc'
        $top = "semaprax-$Tag-$target"
        $ver = $Tag.TrimStart('v')
        $stubs = [System.IO.Path]::Combine($work, 'stubs-' + $ver)
        if (-not [System.IO.Directory]::Exists($stubs)) {
            [void][System.IO.Directory]::CreateDirectory($stubs)
            New-StubExe -Path ([System.IO.Path]::Combine($stubs, 'semaprax.exe')) -Ver $ver -Name 'semaprax'
            New-StubExe -Path ([System.IO.Path]::Combine($stubs, 'semapraxd.exe')) -Ver $ver -Name 'semapraxd'
        }
        $out = [System.IO.Path]::Combine($releaseRoot, $Tag)
        if ([System.IO.Directory]::Exists($out)) { Remove-Item -LiteralPath $out -Recurse -Force }
        [void][System.IO.Directory]::CreateDirectory($out)
        $zipName = "$top.zip"
        $zipPath = [System.IO.Path]::Combine($out, $zipName)
        $fs = [System.IO.File]::Create($zipPath)
        try {
            $zip = New-Object System.IO.Compression.ZipArchive($fs, [System.IO.Compression.ZipArchiveMode]::Create)
            try {
                foreach ($f in 'semaprax.exe', 'semapraxd.exe') {
                    $e = $zip.CreateEntry("$top/$f")
                    $s = $e.Open()
                    try { $b = [System.IO.File]::ReadAllBytes([System.IO.Path]::Combine($stubs, $f)); $s.Write($b, 0, $b.Length) } finally { $s.Dispose() }
                }
                foreach ($pair in @(@('LICENSE', 'license'), @('README.md', 'readme'), @('release-manifest.json', '{}'), @('smoke/meaning.spx', 'module app;'))) {
                    $e = $zip.CreateEntry("$top/$($pair[0])")
                    $w = New-Object System.IO.StreamWriter($e.Open())
                    try { $w.Write($pair[1]) } finally { $w.Dispose() }
                }
            } finally { $zip.Dispose() }
        } finally { $fs.Dispose() }
        $sha = Get-SemapraxFileSha256 $zipPath
        $size = ([System.IO.FileInfo]$zipPath).Length
        if ($CorruptZip) {
            # Keep the published checksum/manifest consistent with a damaged archive.
            [System.IO.File]::WriteAllText($zipPath, 'PK damaged archive, not a zip')
            $sha = Get-SemapraxFileSha256 $zipPath
            $size = ([System.IO.FileInfo]$zipPath).Length
        }
        $sumSha = $sha
        if ($BadChecksum) { $sumSha = ('0' * 64) }
        Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($out, 'SHA256SUMS')) -Text "$sumSha  $zipName`n"
        $manifest = '{"schema":"semaprax.release-manifest.v1","version":"' + $ver + '","tag":"' + $Tag + '","artifacts":[{"name":"' + $zipName + '","platform":"' + $target + '","size":' + $size + ',"digest":"sha256:' + $sha + '"}]}'
        Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($out, 'release-manifest.json')) -Text $manifest
        if (-not $NoAttestation) {
            Write-SemapraxTextFile -Path ([System.IO.Path]::Combine($out, "release-attestation-$target.json")) -Text '{"fixture":true}'
        }
    }

    function Get-PathWithoutGh {
        $keep = @()
        foreach ($seg in $env:Path.Split(';')) {
            if ($seg -ne '' -and ([System.IO.File]::Exists([System.IO.Path]::Combine($seg, 'gh.exe')) -or [System.IO.File]::Exists([System.IO.Path]::Combine($seg, 'gh.cmd')))) { continue }
            $keep += $seg
        }
        return ($keep -join ';')
    }

    $script:CleanPath = Get-PathWithoutGh

    function Invoke-TestInstaller {
        param([string[]]$Arguments, [hashtable]$ExtraEnv = @{}, [string]$PathValue = $script:CleanPath, [string]$CommandText = '')
        $envMap = @{
            SEMAPRAX_INSTALL_DOWNLOAD_BASE = $releaseRoot
            SEMAPRAX_INSTALL_ENV_SUBKEY    = $subKey
            Path                           = $PathValue
        }
        foreach ($k in $ExtraEnv.Keys) { $envMap[$k] = $ExtraEnv[$k] }
        $saved = @{}
        foreach ($k in $envMap.Keys) { $saved[$k] = [Environment]::GetEnvironmentVariable($k); [Environment]::SetEnvironmentVariable($k, $envMap[$k]) }
        try {
            if ($CommandText -ne '') {
                return (Invoke-SemapraxNative -FilePath $hostExe -Arguments @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', $CommandText) -TimeoutSeconds 300)
            }
            return (Invoke-SemapraxNative -FilePath $hostExe -Arguments (@('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', $installPs1) + $Arguments) -TimeoutSeconds 300)
        } finally {
            foreach ($k in $saved.Keys) { [Environment]::SetEnvironmentVariable($k, $saved[$k]) }
        }
    }

    function Get-InstalledVersion {
        param([string]$Dir)
        $r = Invoke-SemapraxNative -FilePath ([System.IO.Path]::Combine($Dir, 'bin', 'semaprax.exe')) -Arguments @('version', '--json')
        return (($r.Output | ConvertFrom-Json).version)
    }

    function Assert-Ok {
        param($Result, [string]$What)
        if ($Result.ExitCode -ne 0) { throw "$What failed (exit $($Result.ExitCode)):`n$($Result.Output)`n$($Result.Error)" }
    }

    function Assert-Fail {
        param($Result, [string]$What)
        if ($Result.ExitCode -eq 0) { throw "$What unexpectedly succeeded:`n$($Result.Output)" }
        if ($Result.Output -match 'installed to') { throw "$What printed a success line:`n$($Result.Output)" }
    }

    function Get-PathCount {
        param([string]$Entry)
        $v = (Get-SemapraxUserPath).Value
        return @($v.Split(';') | Where-Object { $_ -ne '' -and (ConvertTo-SemapraxPathSegmentKey $_) -eq (ConvertTo-SemapraxPathSegmentKey $Entry) }).Count
    }

    $unrelated = '%SystemRoot%\system32;C:\SemapraxTestUnrelated One;D:\Unrelated Two\bin'
    $dirName = 'Sem prax ' + [char]0x00E9 + [char]0x4E2D
    $installDir = [System.IO.Path]::Combine($work, $dirName)
    $binEntry = $installDir + '\bin'
    $key = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($subKey)
    $key.SetValue('Path', $unrelated, [Microsoft.Win32.RegistryValueKind]::ExpandString)
    $key.Close()
    # The parent's own PATH reads (Get-PathCount, Get-SemapraxUserPath) must see
    # the same throwaway subkey the child installer writes, never the real
    # HKCU\Environment Path.
    $savedSubKey = $env:SEMAPRAX_INSTALL_ENV_SUBKEY
    $env:SEMAPRAX_INSTALL_ENV_SUBKEY = $subKey

    try {
        New-Fixture -Tag 'v1.0.0'
        New-Fixture -Tag 'v1.1.0'

        Test-Case $e2eNames[0] {
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.0.0', '-InstallDir', $installDir)
            Assert-Ok $r 'install'
            Assert-True ($r.Output -match 'publisher: not verified \(gh not found\) - checksum only') 'publisher line honest about checksum only'
            # Child console encoding may not round-trip the non-ASCII directory name; the exact text is asserted in the mocked test.
            Assert-True ($r.Output -match '\$env:Path = ".*\\bin;" \+ \$env:Path') 'session command printed'
            Assert-Equal (Get-InstalledVersion $installDir) '1.0.0' 'bin\semaprax.exe version'
            Assert-True ([System.IO.File]::Exists([System.IO.Path]::Combine($installDir, 'bin', 'semapraxd.exe'))) 'semapraxd present'
            Assert-True ([System.IO.File]::Exists([System.IO.Path]::Combine($installDir, 'versions', 'v1.0.0', 'release-manifest.json'))) 'versions dir'
            Assert-Equal (Get-SemapraxFileSha256 ([System.IO.Path]::Combine($installDir, 'bin', 'semaprax.exe'))) (Get-SemapraxFileSha256 ([System.IO.Path]::Combine($installDir, 'versions', 'v1.0.0', 'semaprax.exe'))) 'bin copy equals package file'
            $rc = Read-SemapraxReceipt -InstallDir $installDir
            Assert-Equal $rc.tag 'v1.0.0' 'receipt tag'
            Assert-Equal $rc.path_modification.kind 'user-path' 'receipt path kind'
            Assert-Equal $rc.publisher_verification 'not-verified' 'receipt publisher'
            Assert-True (@($rc.files) -contains 'bin/semapraxd.exe') 'receipt lists semapraxd'
            $p = Get-SemapraxUserPath
            Assert-Equal (Get-PathCount $binEntry) 1 'PATH entry once'
            Assert-Equal $p.Value "$binEntry;$unrelated" 'unrelated PATH content intact and raw %SystemRoot% preserved'
            Assert-Equal ([string]$p.Kind) 'ExpandString' 'REG_EXPAND_SZ preserved'
            Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'bin.new'))) 'no bin.new left'
            $leftovers = @([System.IO.Directory]::GetFileSystemEntries($installDir) | Where-Object { [System.IO.Path]::GetFileName($_) -like '.staging-*' })
            Assert-Equal $leftovers.Count 0 'no staging left'
        }

        Test-Case $e2eNames[1] {
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.0.0', '-InstallDir', $installDir)
            Assert-Ok $r 'reinstall'
            Assert-True ($r.Output -match 'already installed and up to date') 'idempotent message'
            Assert-Equal (Get-InstalledVersion $installDir) '1.0.0' 'still 1.0.0'
            Assert-Equal (Get-PathCount $binEntry) 1 'PATH entry still once'
            Assert-Equal (Get-SemapraxUserPath).Value "$binEntry;$unrelated" 'PATH unchanged byte for byte'
        }

        Test-Case $e2eNames[2] {
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.1.0', '-InstallDir', $installDir)
            Assert-Ok $r 'upgrade'
            Assert-Equal (Get-InstalledVersion $installDir) '1.1.0' 'upgraded'
            Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'versions', 'v1.0.0'))) 'old version pruned'
            Assert-True ([System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'versions', 'v1.1.0'))) 'new version present'
            Assert-Equal (Read-SemapraxReceipt -InstallDir $installDir).tag 'v1.1.0' 'receipt tag'
            Assert-Equal (Get-PathCount $binEntry) 1 'PATH entry once after upgrade'
            Assert-Equal (Get-SemapraxUserPath).Value "$binEntry;$unrelated" 'unrelated PATH content survives upgrade'
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.0.0', '-InstallDir', $installDir)
            Assert-Ok $r 'explicit downgrade'
            Assert-Equal (Get-InstalledVersion $installDir) '1.0.0' 'downgraded'
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.1.0', '-InstallDir', $installDir)
            Assert-Ok $r 'back to 1.1.0'
        }

        Test-Case $e2eNames[3] {
            # failures into a NEW directory leave nothing behind
            $fresh = [System.IO.Path]::Combine($work, 'fresh failure dir')
            New-Fixture -Tag 'v2.0.0' -CorruptZip
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v2.0.0', '-InstallDir', $fresh)
            Assert-Fail $r 'corrupt zip'
            Assert-True ($r.Output -match 'not a readable zip') "corrupt zip message: $($r.Output)"
            Assert-True (-not [System.IO.Directory]::Exists($fresh)) 'failed fresh install leaves no directory'
            New-Fixture -Tag 'v2.0.0' -BadChecksum
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v2.0.0', '-InstallDir', $installDir)
            Assert-Fail $r 'checksum mismatch'
            Assert-True ($r.Output -match 'checksum mismatch') 'checksum message'
            New-Fixture -Tag 'v2.0.0' -NoAttestation
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v2.0.0', '-InstallDir', $installDir)
            Assert-Fail $r 'missing attestation'
            Assert-True ($r.Output -match 'download failed') 'attestation message'
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v9.9.9', '-InstallDir', $installDir)
            Assert-Fail $r 'missing release'
            $r = Invoke-TestInstaller -Arguments @('-InstallDir', $installDir)
            Assert-Fail $r 'no -Version with only a download-base override'
            Assert-True ($r.Output -match 'pass -Version') 'version required message'
            # the old installation is untouched by every failure above
            Assert-Equal (Get-InstalledVersion $installDir) '1.1.0' 'old install intact'
            Assert-Equal (Read-SemapraxReceipt -InstallDir $installDir).tag 'v1.1.0' 'receipt intact'
            Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'versions', 'v2.0.0'))) 'no half-installed version'
        }

        Test-Case $e2eNames[4] {
            $ghDir = New-TempDir ' gh dir'
            try {
                $ghCmd = [System.IO.Path]::Combine($ghDir, 'gh.cmd')
                $argLog = [System.IO.Path]::Combine($ghDir, 'args.txt')
                [System.IO.File]::WriteAllText($ghCmd, "@echo off`r`necho %* > `"$argLog`"`r`nexit /b 0`r`n")
                $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.1.0', '-InstallDir', $installDir, '-RequirePublisherVerification') -PathValue ($ghDir + ';' + $script:CleanPath)
                Assert-Ok $r 'install with passing gh'
                Assert-True ($r.Output -match 'publisher: verified') 'publisher verified line'
                $ghArgs = [System.IO.File]::ReadAllText($argLog)
                Assert-True ($ghArgs -match 'attestation verify') 'gh attestation verify invoked'
                Assert-True ($ghArgs -match '--repo wavect/semaprax') 'repo pinned'
                Assert-True ($ghArgs -match '--signer-workflow wavect/semaprax/.github/workflows/ci.yml') 'signer workflow pinned'
                Assert-True ($ghArgs -match '--source-ref refs/tags/v1.1.0') 'source ref pinned to the tag'
                Assert-True ($ghArgs -match '--deny-self-hosted-runners') 'self-hosted denied'
                Assert-Equal (Read-SemapraxReceipt -InstallDir $installDir).publisher_verification 'verified' 'receipt records verified'
                [System.IO.File]::WriteAllText($ghCmd, "@echo off`r`necho attestation mismatch 1>&2`r`nexit /b 1`r`n")
                New-Fixture -Tag 'v1.3.0'
                $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.3.0', '-InstallDir', $installDir) -PathValue ($ghDir + ';' + $script:CleanPath)
                Assert-Fail $r 'failing gh'
                Assert-True ($r.Output -match 'publisher verification failed') 'failure reported, never downgraded to checksum-only'
                Assert-Equal (Get-InstalledVersion $installDir) '1.1.0' 'old install intact after failed verification'
                $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.3.0', '-InstallDir', $installDir, '-RequirePublisherVerification')
                Assert-Fail $r 'required but gh missing'
                Assert-True ($r.Output -match 'gh \(GitHub CLI\) was not found') 'required-but-missing message'
                Assert-Equal (Get-InstalledVersion $installDir) '1.1.0' 'old install intact'
            } finally { Remove-Item -LiteralPath $ghDir -Recurse -Force -ErrorAction SilentlyContinue }
        }

        Test-Case $e2eNames[5] {
            New-Fixture -Tag 'v1.2.0'
            $sleeper = Start-Process -FilePath ([System.IO.Path]::Combine($installDir, 'bin', 'semaprax.exe')) -ArgumentList 'sleep' -PassThru -WindowStyle Hidden
            try {
                Start-Sleep -Milliseconds 700
                $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.2.0', '-InstallDir', $installDir)
                Assert-Fail $r 'upgrade over a running binary'
                Assert-True ($r.Output -match 'Close every running semaprax/semapraxd') "clear close-and-retry message: $($r.Output)"
                Assert-Equal (Get-InstalledVersion $installDir) '1.1.0' 'old bin intact'
                Assert-True ([System.IO.File]::Exists([System.IO.Path]::Combine($installDir, 'bin', 'semapraxd.exe'))) 'semapraxd intact (no half update)'
                Assert-Equal (Read-SemapraxReceipt -InstallDir $installDir).tag 'v1.1.0' 'receipt intact'
                Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'versions', 'v1.2.0'))) 'new version dir rolled back'
                Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'bin.new'))) 'bin.new removed'
                Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'bin.old'))) 'bin.old not left'
                Assert-True (-not $sleeper.HasExited) 'the running process was not killed'
            } finally {
                if (-not $sleeper.HasExited) { $sleeper.Kill() }
                $sleeper.WaitForExit()
            }
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.2.0', '-InstallDir', $installDir)
            Assert-Ok $r 'retry after closing the process'
            Assert-Equal (Get-InstalledVersion $installDir) '1.2.0' 'upgrade succeeds on retry'
        }

        Test-Case $e2eNames[6] {
            $foreign = New-TempDir ' foreign'
            try {
                [System.IO.File]::WriteAllText([System.IO.Path]::Combine($foreign, 'notes.txt'), 'mine')
                $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.0.0', '-InstallDir', $foreign, '-NoModifyPath')
                Assert-Fail $r 'install into non-empty dir without receipt'
                Assert-True ($r.Output -match 'refusing to install over files') 'refusal message'
                Assert-True ([System.IO.File]::Exists([System.IO.Path]::Combine($foreign, 'notes.txt'))) 'foreign file untouched'
                $r = Invoke-TestInstaller -Arguments @('-Uninstall', '-InstallDir', $foreign)
                Assert-Fail $r 'uninstall of a dir without receipt'
                Assert-True ([System.IO.File]::Exists([System.IO.Path]::Combine($foreign, 'notes.txt'))) 'foreign file survives uninstall refusal'
            } finally { Remove-Item -LiteralPath $foreign -Recurse -Force -ErrorAction SilentlyContinue }
            # an unowned semaprax*.exe inside an owned bin is never replaced
            $imposter = [System.IO.Path]::Combine($installDir, 'bin', 'semaprax-other.exe')
            [System.IO.File]::WriteAllText($imposter, 'not ours')
            try {
                $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.1.0', '-InstallDir', $installDir)
                Assert-Fail $r 'unowned bin file'
                Assert-True ($r.Output -match 'was not installed by this installer') 'ownership message'
                Assert-True ([System.IO.File]::Exists($imposter)) 'unowned exe untouched'
                Assert-Equal (Get-InstalledVersion $installDir) '1.2.0' 'install intact'
            } finally { [System.IO.File]::Delete($imposter) }
        }

        Test-Case $e2eNames[7] {
            $nm = [System.IO.Path]::Combine($work, 'no modify path')
            $before = (Get-SemapraxUserPath).Value
            $r = Invoke-TestInstaller -Arguments @('-Version', 'v1.0.0', '-InstallDir', $nm, '-NoModifyPath')
            Assert-Ok $r 'install -NoModifyPath'
            Assert-Equal (Get-SemapraxUserPath).Value $before 'PATH untouched'
            Assert-Equal (Read-SemapraxReceipt -InstallDir $nm).path_modification.kind 'none' 'receipt kind none'
            $r = Invoke-TestInstaller -Arguments @('-Uninstall', '-InstallDir', $nm)
            Assert-Ok $r 'uninstall'
            Assert-True (-not [System.IO.Directory]::Exists($nm)) 'directory removed when empty'
            Assert-Equal (Get-SemapraxUserPath).Value $before 'PATH untouched by uninstall'
        }

        Test-Case $e2eNames[8] {
            $iexDir = [System.IO.Path]::Combine($work, 'iex install dir')
            $cmd = "try { Invoke-Expression ([System.IO.File]::ReadAllText('$($installPs1.Replace("'", "''"))')) } catch { Write-Host `"FAILED: `$(`$_.Exception.Message)`"; exit 3 }"
            $r = Invoke-TestInstaller -Arguments @() -ExtraEnv @{ SEMAPRAX_INSTALL_VERSION = 'v1.0.0'; SEMAPRAX_INSTALL_DIR = $iexDir; SEMAPRAX_INSTALL_NO_MODIFY_PATH = '1' }
            # Empty Arguments runs install.ps1 via -File with the env fallback; then the iex form.
            Assert-Ok $r 'env-var driven install (-File)'
            $r2 = Invoke-TestInstaller -Arguments @() -CommandText $cmd -ExtraEnv @{ SEMAPRAX_INSTALL_VERSION = 'v1.0.0'; SEMAPRAX_INSTALL_DIR = $iexDir; SEMAPRAX_INSTALL_NO_MODIFY_PATH = '1' }
            Assert-Ok $r2 'iex-form install'
            Assert-True ($r2.Output -match 'already installed and up to date') 'iex form reinstalls idempotently'
            [void](Invoke-TestInstaller -Arguments @('-Uninstall', '-InstallDir', $iexDir))
        }

        Test-Case $e2eNames[9] {
            $keepFile = [System.IO.Path]::Combine($installDir, 'projects-keep.txt')
            [System.IO.File]::WriteAllText($keepFile, 'user data')
            $r = Invoke-TestInstaller -Arguments @('-Uninstall', '-InstallDir', $installDir)
            Assert-Ok $r 'uninstall'
            Assert-Equal (Get-PathCount $binEntry) 0 'PATH entry removed'
            Assert-Equal (Get-SemapraxUserPath).Value $unrelated 'unrelated PATH content restored exactly'
            Assert-Equal ([string](Get-SemapraxUserPath).Kind) 'ExpandString' 'kind still REG_EXPAND_SZ'
            Assert-True ([System.IO.File]::Exists($keepFile)) 'unowned file retained'
            Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'bin'))) 'bin removed'
            Assert-True (-not [System.IO.Directory]::Exists([System.IO.Path]::Combine($installDir, 'versions'))) 'versions removed'
            Assert-True (-not [System.IO.File]::Exists([System.IO.Path]::Combine($installDir, 'install-receipt.json'))) 'receipt removed'
            $entries = @([System.IO.Directory]::GetFileSystemEntries($installDir))
            Assert-Equal $entries.Count 1 'only the user file remains'
            $r = Invoke-TestInstaller -Arguments @('-Uninstall', '-InstallDir', $installDir)
            Assert-Fail $r 'second uninstall of a dir that now has no receipt'
            [System.IO.File]::Delete($keepFile)
            $r = Invoke-TestInstaller -Arguments @('-Uninstall', '-InstallDir', $installDir)
            Assert-Ok $r 'uninstall of an empty dir is a no-op'
        }
    } finally {
        $env:SEMAPRAX_INSTALL_ENV_SUBKEY = $savedSubKey
        try { [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($subKey, $false) } catch { }
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Write-Host ''
Write-Host "install.ps1 tests: $($script:Passed) passed, $($script:Failed) failed, $($script:Skipped) skipped"
if ($script:Failed -gt 0) { exit 1 }
exit 0
