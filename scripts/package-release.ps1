param(
    [Parameter(Mandatory = $true)][string]$Tag,
    [Parameter(Mandatory = $true)][string]$Commit,
    [Parameter(Mandatory = $true)][string]$Target,
    [Parameter(Mandatory = $true)][string]$OutputRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Reject([string]$Message) {
    throw "release package rejected: $Message"
}

$versions = @(
    Get-Content -LiteralPath 'Cargo.toml' | ForEach-Object {
        if ($_ -cmatch '^version = "([^"]*)"$') { $Matches[1] }
    }
)
if ($versions.Count -ne 1 -or [string]::IsNullOrEmpty($versions[0])) { Reject 'Cargo package version is missing or ambiguous' }
$version = $versions[0]
if ($Tag -cne "v$version") { Reject 'tag does not equal v plus the Cargo package version' }
if ($Commit.Length -ne 40 -or $Commit -cnotmatch '^[0-9a-f]{40}$') { Reject 'commit must be exactly 40 lowercase hexadecimal characters' }
if ($Target -cne 'x86_64-pc-windows-msvc') { Reject 'unsupported Windows release target' }
$hostLine = @(rustc -vV | Where-Object { $_ -cmatch '^host: ' })
if ($LASTEXITCODE -ne 0 -or $hostLine.Count -ne 1 -or $hostLine[0] -cne "host: $Target") { Reject 'Rust host does not equal the requested release target' }

# PowerShell's filesystem location can differ from the process current directory.
# Resolve through its provider before using .NET filesystem APIs below.
if ((Get-Location).Provider.Name -cne 'FileSystem') { Reject 'release location must use the filesystem provider' }
$outputProvider = $null
$outputDrive = $null
$output = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($OutputRoot, [ref]$outputProvider, [ref]$outputDrive)
if ($outputProvider.Name -cne 'FileSystem' -or -not [System.IO.Path]::IsPathRooted($output)) { Reject 'output root must resolve to an absolute filesystem path' }
$packageName = "semaprax-$Tag-$Target"
$packageRoot = Join-Path $output $packageName
$archive = Join-Path $output "$packageName.zip"
$smokeRoot = Join-Path $output "smoke-$Target"
$buildRoot = Join-Path $output "build-$Target"
foreach ($path in @($packageRoot, $archive, $smokeRoot, $buildRoot)) {
    $existing = $null
    try {
        $existing = Get-Item -LiteralPath $path -Force -ErrorAction Stop
    } catch [System.Management.Automation.ItemNotFoundException] {
        # Absence is expected; Get-Item also exposes a dangling link as an item.
    }
    if ($null -ne $existing) { Reject "output path already exists: $path" }
}
[System.IO.Directory]::CreateDirectory($output) | Out-Null
New-Item -ItemType Directory -Path $packageRoot -ErrorAction Stop | Out-Null
New-Item -ItemType Directory -Path (Join-Path $packageRoot 'smoke') -ErrorAction Stop | Out-Null
New-Item -ItemType Directory -Path $smokeRoot -ErrorAction Stop | Out-Null
New-Item -ItemType Directory -Path $buildRoot -ErrorAction Stop | Out-Null

$env:SEMAPRAX_BUILD_COMMIT = $Commit
cargo build --locked --release --target $Target --target-dir $buildRoot -p semaprax -p semaprax-toolchain --bin semaprax-full --bin semapraxd
if ($LASTEXITCODE -ne 0) { Reject 'cargo build failed' }
$releaseRoot = Join-Path (Join-Path $buildRoot $Target) 'release'
Copy-Item -LiteralPath (Join-Path $releaseRoot 'semaprax-full.exe') -Destination (Join-Path $packageRoot 'semaprax.exe')
Copy-Item -LiteralPath (Join-Path $releaseRoot 'semapraxd.exe') -Destination (Join-Path $packageRoot 'semapraxd.exe')
Copy-Item -LiteralPath 'LICENSE' -Destination $packageRoot
Copy-Item -LiteralPath 'README.md' -Destination $packageRoot

$manifest = @(
    '{',
    '  "schema": "semaprax.release-artifact.v1",',
    "  `"version`": `"$version`",",
    "  `"commit`": `"$Commit`",",
    "  `"target`": `"$Target`",",
    '  "maturity": "pre-alpha",',
    '  "binaries": ["semaprax", "semapraxd"],',
    '  "nonclaims": [',
    '    "production-ready",',
    '    "stable language ABI",',
    '    "stable public protocol",',
    '    "safety-critical suitability"',
    '  ]',
    '}'
) -join "`n"
$utf8NoBom = [System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText((Join-Path $packageRoot 'release-manifest.json'), "$manifest`n", $utf8NoBom)
$smoke = "module release.smoke;`n`n@id(`"release.smoke.main`")`nfn main() -> i64 { 42 }`n"
[System.IO.File]::WriteAllText((Join-Path $packageRoot 'smoke/meaning.spx'), $smoke, $utf8NoBom)

Compress-Archive -LiteralPath $packageRoot -DestinationPath $archive -CompressionLevel Optimal
Expand-Archive -LiteralPath $archive -DestinationPath $smokeRoot
$unpacked = Join-Path $smokeRoot $packageName
$binary = Join-Path $unpacked 'semaprax.exe'
$human = @(& $binary --version)
if ($LASTEXITCODE -ne 0 -or $human.Count -ne 1 -or $human[0] -cne "semaprax $version ($Commit)") { Reject 'human version smoke disagrees' }
$json = @(& $binary version --json)
$expectedJson = "{`"schema`":`"semaprax.version.v1`",`"version`":`"$version`",`"commit`":`"$Commit`",`"maturity`":`"pre-alpha`",`"rust_min`":`"1.88`"}"
if ($LASTEXITCODE -ne 0 -or $json.Count -ne 1 -or $json[0] -cne $expectedJson) { Reject 'JSON version smoke disagrees' }
& $binary check (Join-Path $unpacked 'smoke/meaning.spx')
if ($LASTEXITCODE -ne 0) { Reject 'check smoke failed' }
$run = @(& $binary run (Join-Path $unpacked 'smoke/meaning.spx'))
if ($LASTEXITCODE -ne 0 -or $run.Count -ne 1 -or $run[0] -cne '42') { Reject 'run smoke disagrees' }
Write-Output $archive
