# Installer v1

Status: normative contract for `scripts/install.sh` (macOS and GNU/Linux) and,
for the shared layout, receipt, verification order and release-asset rules,
`scripts/install.ps1` (Windows). The installer is published as a release
asset only from the first release whose inventory lists it; releases up to
v0.8.0 do not carry it, but `install.sh --version v0.8.0` installs v0.8.0
because it needs only assets that release already publishes.

Audience: users installing SEMAPRAX, release engineers, and security reviewers.

## Commands

```sh
curl -fsSL https://github.com/wavect/semaprax/releases/latest/download/install.sh | sh
curl -fsSL https://github.com/wavect/semaprax/releases/latest/download/install.sh | sh -s -- --version v0.9.0
```

To read before running, download `install.sh`, inspect it, then run
`sh install.sh [options]`. The installer never prompts, never uses `sudo`,
needs no Rust, Cargo, Git checkout, Node or model credential, and reads nothing
from standard input.

## Options

| Option | Meaning |
|--------|---------|
| `--version <tag>` | Install exactly this release (`v0.9.0`; `0.9.0` is accepted). Without it, the latest published release is resolved once. |
| `--prefix <dir>` | Install under `<dir>` instead of `$HOME/.semaprax`. Spaces are fine. The prefix may not contain a newline, `"`, `$`, backtick or backslash. |
| `--no-modify-path` | Do not edit any shell profile. |
| `--yes` | Accepted for noninteractive use; behavior is unchanged. |
| `--uninstall` | Remove an installation made by this installer. |
| `--require-publisher-verification` | Fail unless `gh attestation verify` succeeds. |
| `--help` | Print usage. |

Unknown options and malformed tags fail before any network access.

### Test-only overrides

These exist for fixtures and CI and are never needed by users. They are read
from the environment and change transport or detection only; they do not relax
any verification step.

| Variable | Meaning |
|----------|---------|
| `SEMAPRAX_INSTALL_DOWNLOAD_BASE` | Asset base URL; an asset is `$BASE/<tag>/<name>`. Default `https://github.com/wavect/semaprax/releases/download`. `file://` works with `curl`. |
| `SEMAPRAX_INSTALL_LATEST_URL` | URL whose redirect ends in `/tag/<tag>`. Default `https://github.com/wavect/semaprax/releases/latest`. |
| `SEMAPRAX_INSTALL_TEST_UNAME_S`, `_UNAME_M` | Replace `uname -s` / `uname -m`. |
| `SEMAPRAX_INSTALL_TEST_LIBC` | Replace libc detection: `glibc 2.35`, `musl`, or empty for unknown. |
| `SEMAPRAX_INSTALL_TEST_ROSETTA` | Replace `sysctl -n sysctl.proc_translated` (`0` or `1`). |

With a download-base override and no latest-URL override, `--version` is
required, so a fixture can never silently resolve the real latest release.

## Supported targets and runtime baselines

The installer's `SUPPORTED_TARGETS` line plus the Windows target owned by
`install.ps1` must equal `ARCHIVE_TARGETS` in `scripts/release-reconcile.py`;
`scripts/test-install-sh.py` asserts it. There is no second registry.

| Target | Host | Runtime baseline |
|--------|------|------------------|
| `x86_64-unknown-linux-gnu` | Linux x86_64 | GNU/Linux, glibc 2.35 or newer |
| `aarch64-unknown-linux-gnu` | Linux aarch64 | GNU/Linux, glibc 2.35 or newer |
| `aarch64-apple-darwin` | macOS arm64 | macOS 11.0 or newer |
| `x86_64-apple-darwin` | macOS x86_64 | the minimum OS the archive binary records |

Detection rules:

- macOS on Apple silicon selects `aarch64-apple-darwin`, including from a shell
  running under Rosetta (`sysctl -n sysctl.proc_translated` is `1`). Emulation
  is never chosen when a native build exists.
- Linux requires GNU libc (`getconf GNU_LIBC_VERSION`) at or above the baseline.
  musl (for example Alpine), an unknown libc, and an older glibc are refused;
  musl is never treated as GNU/Linux.
- Any other OS or CPU is refused.

A refusal prints the source-install alternative and exits nonzero before any
download. The libc and OS baseline is re-proved before activation by the staged
smoke run below.

## Source-install alternative

```sh
cargo install --locked --git https://github.com/wavect/semaprax --tag <tag> semaprax
```

This builds the root package and installs the **standalone** `semaprax` and
`semapraxd`. The release archive's `semaprax` is the **full** build, the
`semaprax-toolchain` package's `semaprax-full` binary renamed. To build the
same product from source:

```sh
cargo install --locked --git https://github.com/wavect/semaprax --tag <tag> semaprax-toolchain --bin semaprax-full
```

which installs it under the name `semaprax-full`, not `semaprax`.

## Layout

```text
<prefix>/versions/<tag>/            extracted package: semaprax, semapraxd, LICENSE,
                                    README.md, release-manifest.json, smoke/
<prefix>/current -> versions/<tag>  activation point
<prefix>/bin/semaprax  -> ../current/semaprax
<prefix>/bin/semapraxd -> ../current/semapraxd
<prefix>/install-receipt.json
```

The default prefix is `$HOME/.semaprax`. Nothing outside the prefix is written
except the optional profile block below. Package-managed installations defer
upgrade and removal to their package manager; this installer manages only its
own prefix.

## Receipt

`install-receipt.json` is UTF-8 JSON with schema `semaprax.install-receipt.v1`,
written last and atomically:

```json
{
  "schema": "semaprax.install-receipt.v1",
  "installer": "install.sh",
  "version": "0.9.0",
  "tag": "v0.9.0",
  "target": "aarch64-apple-darwin",
  "source": "https://github.com/wavect/semaprax/releases/download/v0.9.0/semaprax-v0.9.0-aarch64-apple-darwin.tar.gz",
  "archive_sha256": "<64 lowercase hex>",
  "publisher_verification": "verified",
  "files": ["versions/v0.9.0/semaprax", "bin/semaprax", "current"],
  "path_modification": {"kind": "profile", "location": "/home/me/.zshrc"}
}
```

- `publisher_verification` is `verified` only when `gh attestation verify`
  succeeded for this install, otherwise `not-verified`.
- `files` lists every relative path the installer owns for the active version,
  including the two `bin/` links and `current`. `--uninstall` removes exactly
  these.
- `path_modification` is `{"kind": "profile", "location": <first profile
  written>}`, or `{"kind": "none", "location": null}` with `--no-modify-path`.

## Verification order

Everything below happens before activation. A failure at any step leaves the
previous installation untouched and prints no success line.

1. Preflight: platform, tools (`tar`, `awk`, `sed`, `grep`, `curl` or `wget`,
   `sha256sum` or `shasum -a 256`, ...), prefix state, and
   `--require-publisher-verification` availability. Missing tools fail here.
2. Resolve the tag once (`--version`, or the redirect of the latest-release
   URL). Every later URL uses that exact tag; two resolutions are never mixed.
3. Download only `SHA256SUMS`, `release-manifest.json`, the one selected
   `semaprax-<tag>-<target>.tar.gz`, and `release-attestation-<target>.json`.
   Nothing for other hosts is requested.
4. Checksum: the archive's SHA-256 must equal its exact-name line in
   `SHA256SUMS` (exactly one line) **and** the matching `artifacts[]` entry in
   `release-manifest.json`: `name`, `platform` equal to the target, `size`,
   and `digest`. The manifest `schema` and `tag` must match. Missing,
   malformed, duplicated or inconsistent material fails. The manifest is read
   without `jq`; releases without an `installers` key parse unchanged.
5. Publisher verification, if `gh` with the `attestation` command is on PATH:

   ```sh
   gh attestation verify <archive> --bundle <release-attestation-target.json> \
     --repo wavect/semaprax \
     --signer-workflow wavect/semaprax/.github/workflows/ci.yml \
     --source-ref refs/tags/<tag> --deny-self-hosted-runners
   ```

   Failure aborts the install; the installer never falls back to
   checksum-only after a failed verification. Without `gh` it reports
   `publisher: not verified (gh not found) - checksum only` and continues,
   unless `--require-publisher-verification` was given, in which case it fails
   during preflight. **A checksum proves the download matches what the release
   page lists; it does not prove who published it.** Only the attestation check
   establishes publisher identity, and the downloaded `semaprax` is never used to
   verify itself. The attestation file must be present and non-empty even when
   `gh` is unavailable.
6. Extraction into staging inside the prefix: every member must be a regular
   file or directory, with no absolute path, no `..`, no backslash, only
   `[A-Za-z0-9._/-]`, and a path inside `semaprax-<tag>-<target>/`. Links,
   special members and any other top-level entry are rejected, as are archives
   missing a regular executable `semaprax` or `semapraxd`.
7. Staged smoke: the staged `semaprax version --json` must exit zero and report
   the selected version. This also catches an incompatible libc or OS before
   exposure.

## Activation

1. Move the staged package to `versions/<tag>` (kept if the same tag with the
   same archive digest is already there; a same-tag, different-digest archive is
   refused).
2. Create `bin/semaprax` and `bin/semapraxd` as links to `../current/...`. An
   existing `bin/semaprax*` that is not the installer's link is never replaced
   and aborts the install.
3. Atomically switch `current`: a temporary symlink is created and renamed over
   `current` (`mv -T` on GNU, `mv -h` on BSD and macOS, discovered by probe,
   otherwise `ln -sfn`).
4. Add or refresh the profile block, if requested.
5. Write `install-receipt.json` last. Only then is the install complete.
6. Prune other `versions/*` directories, after success only. Pruning failure is
   a warning.

A failure after step 3 and before step 5 restores the previous `current` target
(or removes `current` on a fresh install), removes what this run created, and
exits nonzero. An `EXIT`, `INT`, `TERM` and `HUP` trap removes the staging
directory in every case; a failed fresh install leaves no prefix (or restores
the pre-existing empty one).

Rules: a non-empty prefix without a receipt is refused; rerunning the same
version is idempotent (it re-verifies and reports `already installed and
active`); upgrade and explicit downgrade both switch `current` and prune the
other version; `current` that is not a symlink is refused.

## PATH behavior

A child process cannot change the environment of the shell that started it, so
installing never changes the current terminal. The installer always prints the
exact command for the current terminal:

```sh
export PATH="<prefix>/bin:$PATH"
```

Unless `--no-modify-path` is given it also appends one marked block to the
login shell's profile (from `$SHELL`):

| Shell | Files |
|-------|-------|
| zsh | `~/.zshrc` |
| bash | `~/.bashrc`, and on macOS also `~/.bash_profile` |
| fish | `~/.config/fish/conf.d/semaprax.fish` |
| other or unset | `~/.profile` |

```text
# >>> semaprax installer >>>
...
# <<< semaprax installer <<<
```

The block is idempotent: rerunning replaces it rather than adding another, and
content outside the markers is preserved byte for byte. Only one block exists
per profile, so installing a second prefix points that profile at the newer
prefix. Future terminals pick it up; already-open terminals do not. If another
`semaprax` is first on PATH the installer warns.

## Uninstall

`install.sh --uninstall [--prefix <dir>]` requires a valid receipt and removes
only: the files the receipt lists, the directories that become empty, the
`current` link, the receipt, and the installer's own PATH block (only a block
naming this prefix's `bin`, in the standard profiles). It then removes the
prefix only if it is empty; otherwise it says why it was left. Receipt entries
with absolute or `..` paths abort the uninstall untouched. Projects, agent
configuration, credentials, unrelated binaries and other prefixes are never
touched.

## Verification of the installer itself

- `scripts/test-install-sh.py` (fixture releases, no network): platform
  detection, exact selection, every checksum/manifest/attestation failure,
  unsafe archive members, failed smoke, interrupted download, ownership and
  refusal cases, paths with spaces, profile idempotence, uninstall scope, the
  target-list contract, and `shellcheck -s sh`.
- `scripts/install-journey.sh <assets-dir> <tag>`: runs the candidate
  directory's own `install.sh` against its exact assets, then the beginner
  journey (`semaprax --version`, `new`, `check`, `test`, `run` printing `42`)
  from a fresh directory with PATH reduced to the prefix and system tools,
  byte-compares both executables with the archive members, checks `semapraxd`
  with a bounded `protocol`/`shutdown` stdio handshake against the starter
  project, reinstalls, proves a damaged archive is rejected with the previous
  install still working, and uninstalls. It ends with a
  `JOURNEY-RESULT tag=... target=... archive_sha256=... publisher=... outcome=...`
  line. `SEMAPRAX_JOURNEY_REQUIRE_PUBLISHER=1` adds
  `--require-publisher-verification`.

## Nonclaims

The installer does not sign anything, run a background updater, configure
third-party tools or agent harnesses, or install the standalone source build.
Checksum-only installs are labelled as such and must not be described as
signature-verified.
