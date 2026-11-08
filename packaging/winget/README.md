# WinGet channel

Status: submitted and locally qualified; Microsoft moderation is pending. The channel is **pending** until
[`../channels.json`](../channels.json) says `"winget": {"status": "published"}`;
no document may advertise `winget install` before then
(`scripts/release-reconcile.py` fails if one does). A local manifest or an open
pull request is not public availability.

The package identity is `Wavect.Semaprax`. It reuses the release's portable
Windows x64 ZIP (`InstallerType: zip`, `NestedInstallerType: portable`) and maps
the archive's real paths to two command aliases:

| Nested file | Alias |
| --- | --- |
| `semaprax-<tag>-x86_64-pc-windows-msvc\semaprax.exe` | `semaprax` |
| `semaprax-<tag>-x86_64-pc-windows-msvc\semapraxd.exe` | `semapraxd` |

It declares `Architecture: x64`, `MinimumOSVersion: 10.0.17763.0`
(Windows 10 1809 / Server 2019) and the schema `1.28.0`. It does not claim
native ARM64 support. It omits `Scope` because `winget validate` warns that scope is unsupported for portable installers (portable installs are per-user). The URL is the exact-tag release asset and
`InstallerSha256` is the uppercase digest from that release's `SHA256SUMS`.

## Generate manifests for a release

```sh
gh release download v0.9.0 -R wavect/semaprax -p SHA256SUMS -p release-manifest.json
python3 scripts/channel-manifests.py winget --tag v0.9.0 \
  --sums SHA256SUMS --manifest release-manifest.json --out out
# out/winget/manifests/w/Wavect/Semaprax/0.9.0/Wavect.Semaprax{,.installer,.locale.en-US}.yaml
```

The generator fails closed (writes nothing) when the Windows archive is missing
or any tag, version, name or digest disagrees between `SHA256SUMS` and
`release-manifest.json`. Output is deterministic.

## Update process after each release

1. Confirm the identity is free or already ours:
   `winget search Wavect.Semaprax` and a search of `microsoft/winget-pkgs`
   `manifests/w/Wavect/`.
2. Generate the three files, copy the `manifests/w/Wavect/Semaprax/<version>/`
   directory into a `winget-pkgs` fork, and validate on Windows:

   ```powershell
   winget validate --manifest .\manifests\w\Wavect\Semaprax\0.9.0
   winget settings --enable LocalManifestFiles
   winget install --manifest .\manifests\w\Wavect\Semaprax\0.9.0
   semaprax --version; semapraxd --help
   winget uninstall --manifest .\manifests\w\Wavect\Semaprax\0.9.0
   ```

   Run the beginner journey (`new`, `check`, `test`, `run` printing `42`) from a
   directory with spaces in a new terminal, and check that both aliases resolve
   to the same version. Test an upgrade from the previous package version and
   alongside a manual install.
3. Open the pull request on `microsoft/winget-pkgs`. State stays **pending**
   through review.
4. When the community index serves it (`winget show --id Wavect.Semaprax --exact`
   on a clean machine, no local manifest), set `winget.status` to `published`
   (with `version`, the release tag, and `verified_at`) in
   `packaging/channels.json` and add the install, upgrade and uninstall commands
   to the install guide. A rejected or delayed submission leaves the guide on
   the manual/PowerShell route.

The v0.8.0 local-manifest journey passed on Windows 2025 in
[run 37454638162](https://github.com/wavect/semaprax/actions/runs/37454638162):
validation, both aliases, starter result 42, daemon handshake, paths with spaces
and uninstall. The [submission](https://github.com/microsoft/winget-pkgs/pull/447495)
passed Microsoft manifest, URL and installer checks; moderator approval remains
pending. The submission was updated to v0.9.0 at `7cce7c45fdb03ad5543458df3bc0808df4994d68`.
Its local v0.8.0 to v0.9.0 upgrade, both version-matched aliases, starter result
`42`, bounded daemon protocol/shutdown and local-manifest removal passed in
[run 38156173827](https://github.com/wavect/semaprax/actions/runs/38156173827).
The v0.9.0 manifests are retained under
[`../releases/v0.9.0/winget`](../releases/v0.9.0/winget). The Package channels
workflow validates and installs new proposals on Windows before retention.
Public availability and public upgrade/removal are still unverified.
