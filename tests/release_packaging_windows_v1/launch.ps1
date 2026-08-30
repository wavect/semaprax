Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
Set-Location -LiteralPath $env:RELEASE_FIXTURE_REPOSITORY
[System.Environment]::CurrentDirectory = $env:RELEASE_FIXTURE_PROCESS_CWD
if ((Get-Location).ProviderPath -ceq [System.Environment]::CurrentDirectory) {
    throw 'fixture requires distinct PowerShell and process current directories'
}
& $env:RELEASE_FIXTURE_SCRIPT -Tag 'v0.2.0' -Commit $env:RELEASE_FIXTURE_COMMIT -Target 'x86_64-pc-windows-msvc' -OutputRoot $env:RELEASE_FIXTURE_OUTPUT
