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
if ($Commit -cnotmatch '^[0-9a-f]{40}$') { Reject 'commit must be exactly 40 lowercase hexadecimal characters' }
if ($Target -cne 'x86_64-pc-windows-msvc') { Reject 'unsupported Windows release target' }
$hostLine = @(rustc -vV | Where-Object { $_ -cmatch '^host: ' })
if ($LASTEXITCODE -ne 0 -or $hostLine.Count -ne 1 -or $hostLine[0] -cne "host: $Target") { Reject 'Rust host does not equal the requested release target' }

$output = [System.IO.Path]::GetFullPath($OutputRoot)
$packageName = "semaprax-$Tag-$Target"
$packageRoot = Join-Path $output $packageName
$archive = Join-Path $output "$packageName.zip"
$smokeRoot = Join-Path $output "smoke-$Target"
foreach ($path in @($packageRoot, $archive, $smokeRoot)) {
    if (Test-Path -LiteralPath $path) { Reject "output path already exists: $path" }
}
[System.IO.Directory]::CreateDirectory((Join-Path $packageRoot 'smoke')) | Out-Null
[System.IO.Directory]::CreateDirectory($smokeRoot) | Out-Null

$env:SEMAPRAX_BUILD_COMMIT = $Commit
cargo build --locked --release --target $Target --bin semaprax --bin semapraxd
if ($LASTEXITCODE -ne 0) { Reject 'cargo build failed' }
Copy-Item -LiteralPath "target/$Target/release/semaprax.exe" -Destination (Join-Path $packageRoot 'semaprax.exe')
Copy-Item -LiteralPath "target/$Target/release/semapraxd.exe" -Destination (Join-Path $packageRoot 'semapraxd.exe')
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
