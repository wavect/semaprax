# Linux native provider provisioning for the live pilot

`agent/pilot_linux_provider.py` provisions **Claude Code 2.1.286** for the
separately admitted Linux arm64 Apple Container guest. It does not implement a
model-dispatch route. The helper accepts only version, auth-status, login-help
and interactive subscription-login operations; `--print`, prompts, `setup-token`
and arbitrary token arguments are refused by its command vocabulary.

## Release provenance

The [official installation documentation](https://code.claude.com/docs/en/setup#verify-the-manifest-signature)
owns the release-manifest verification process. This packet pins:

- Version: `2.1.286`, native platform: `linux-arm64`.
- Release signing fingerprint: `31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE`.
- Binary SHA-256: `0292fa22ac2fd43e16be9d0e511ddd8347280d6e0ebaca744ef5b27e05d8d0f8`.
- Binary length: `241033208` bytes; ELF64 little-endian AArch64.
- Release root: `https://downloads.claude.ai/claude-code-releases/2.1.286`.
- Public signing key: `https://downloads.claude.ai/keys/claude-code.asc`.

Acquire `manifest.json`, `manifest.json.sig`, `linux-arm64/claude` (saved as
`claude`), and `claude-code.asc` into a private input directory. Use HTTPS,
bounded downloads, and do not execute an installer or downloaded binary before
verification. The provisioning helper verifies the fingerprint and detached
signature in a fresh temporary GPG home, validates the exact manifest row and
binary, and records the original public receipts and their hashes. It never
imports into the user's GPG keyring.

The input directory and destination must be absolute, canonical paths. The
destination must be fresh and outside the repository. An existing destination
refuses before mutation. Provision with an explicit already reviewed Linux
scoring-host receipt and its independently supplied digest:

```sh
PYTHONPATH=benchmarks/cross-language-v1 python3 -m agent.pilot_linux_provider provision \
  --inputs /absolute/private/release-inputs \
  --root /absolute/new/provider-root \
  --linux-provision /absolute/linux-host.json \
  --linux-digest REVIEWED_LINUX_HOST_RECEIPT_SHA256 \
  --gpg /absolute/path/to/gpg
```

The result prints only the new public provision-receipt digest and zero model
dispatches. No credentials, auth URL or token is included in that receipt.

## Private persistent guest

The root contains separate `public`, `private-home` and `scratch` directories,
all mode 0700. The authenticated binary is mode 0500. No Mac home, Keychain,
credential file or login token is copied. `private-home` is initially empty and
persists guest-created authentication independently of the container lifecycle.
It must never be committed or copied into evidence.

Start the explicitly pinned guest:

```sh
PYTHONPATH=benchmarks/cross-language-v1 python3 -m agent.pilot_linux_provider start \
  --root /absolute/provider-root --receipt-sha256 REVIEWED_PROVIDER_RECEIPT_SHA256
```

The named guest `semaprax-issue332-provider` uses the reviewed Debian 12 image,
one CPU, at most 2 GiB memory, a read-only root, non-root UID/GID and no Linux
capabilities. It mounts only the public binary at `/opt/claude` read-only,
private authentication home at `/home/pilot`, and scratch at `/work`. A closed
environment excludes ambient API keys, OAuth tokens, Mac settings and agent
sockets. Update paths and nonessential traffic are disabled.

This **provider** guest has an explicitly enabled network for OAuth and later
separately authorized provider transport. It does not inherit or claim the
candidate scorer's network-denied Landlock/seccomp profile. Its trusted CLI and
provider authentication are a distinct authority grant. Starting it checks the
exact CLI version, Linux arm64/kernel identity and boot ID, retaining only those
nonsecret observations. Restarting changes the boot ID and requires refreshing
the host record before any frozen trial.

## User sign-in

Provisioning creates a private mode-0700 `login.sh` with shell-quoted, exact
paths and receipt digest. This is a local private artifact outside the repository,
recreated by the provision command on each fresh setup. The start command records
the newly observed guest boot identity; it never reuses a prior boot ID. Run the
script in your own interactive terminal:

```sh
/absolute/provider-root/login.sh
```

The wrapper rechecks provision/binary/control-plane identities and the live
guest's executable hash and recorded boot ID. It refuses noninteractive stdin
or stdout, then attaches directly to `claude auth login --claudeai`. It does not
capture or retain the authentication stream. Open the link shown in your own
terminal, sign in to the intended subscription, and paste any returned code
**only into that terminal, never into chat or evidence**. No setup token is
requested. The helper's direct equivalent is:

```sh
container exec --interactive --tty --workdir /work semaprax-issue332-provider \
  /usr/bin/env -i HOME=/home/pilot USER=pilot LOGNAME=pilot PATH=/usr/bin:/bin \
  TMPDIR=/work LANG=C LC_ALL=C DISABLE_AUTOUPDATER=1 DISABLE_UPDATES=1 \
  CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 CLAUDE_CODE_SAFE_MODE=1 \
  /opt/claude/claude auth login --claudeai
```

After sign-in, inspect only sanitized auth status and file-permission metadata;
never print credential files or bearer tokens. Authentication alone does not
authorize model calls. Native generation remains owned by the separately frozen
pilot transport, with explicit models, host authority, budget and one-use cells.
The container may stay idle while awaiting user login. Stopping it preserves the
private home but invalidates the recorded boot identity.

## Focused evidence

The pure owning gate is:

```sh
python3 -m unittest discover -s benchmarks/cross-language-v1/agent/tests \
  -p test_pilot_linux_provider.py -v
```

On 2026-10-02 it passed 5/5 in 0.004s. Separate physical provisioning verified the
signed release, binary checksum and guest-native `2.1.286 (Claude Code)` output.
Fresh guest auth status was `loggedIn: false`, `authMethod: none`. A bounded
15-second login probe reached the OAuth URL and waited for authentication before
timing out;
only boolean readiness was retained, and its temporary raw output was deleted.
No provider model call occurred. User sign-in remains pending.
