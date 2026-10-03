# Cross-language pilot Linux scoring host v1

This additive profile owns `agent/pilot_linux_host.py`,
`agent/pilot_linux_launcher.c`, and the dedicated
`agent/tests/test_pilot_linux_host.py` gates. It is a local Linux arm64 Apple
Container VM controlled by the admitted Darwin host. It is **not independent
physical hardware**. Scoring admission does not authorize model dispatch or
establish that a second host generated a candidate.

## Explicit provision

`LinuxCandidateSession(directory, expected_provision_sha256=...)` requires a
canonical `linux-host.json` and an independently supplied digest. Its exact
fields are:

- `schema`: `benchmark.cross_language.linux_host_provision.v1`
- `profile`: `apple-container-linux-arm64-typescript-pilot.v1`
- `container_path`, `container_sha256`: canonical control-plane executable and hash
- `image`: `docker.io/library/rust@sha256:<index hash>`
- `arm64_manifest_sha256`: exact Linux arm64 image manifest hash
- `kernel_release`: exact observed numeric guest kernel version
- `node_binary_sha256`: exact selected ELF64 little-endian AArch64 Node binary
- `launcher_sha256`, `launcher_source_sha256`: reviewed compiled launcher and source
- `review_reference`: bounded operator review reference, not a proof of review

There are no default executable, image, launcher or binary hashes. The provision
directory also contains `node-v22.12.0-linux-arm64.tar.xz`,
`node22.12.0-SHASUMS256.txt`, `typescript-5.8.3.tgz`,
`typescript5.8.3-registry.json`, and `pilot-linux-launcher`. The Node checksum
receipt and TypeScript metadata/archive use the existing independently frozen
v3 receipt identities. The Linux archive checksum is selected from that exact
Node receipt; it is not copied from Darwin's archive hash. The selected Node
binary additionally matches the provision pin. Extraction is bounded and
materializes only the selected executable and the exact TypeScript files.

Provisioning is a separate operator action. Download the official Linux Node
archive, verify its checksum against the admitted receipt, inspect the selected
ELF identity, and compile the launcher with the pinned guest compiler/image
(e.g. `cc -O2 -Wall -Wextra -Werror`). Record both source and output hashes.
Trials never download dependencies, install packages or compile the launcher.
The immutable image reference must already be cached. Apple Container may need
an explicit digest alias created from an inspected cached image; the code
checks both index and arm64 manifest before launch. A missing receipt, mismatch,
unsupported host, missing kernel mechanism or failed probe refuses admission.

The Apple Container service, its host installation and VM kernel are trusted
control-plane components. Their host OS identity, CLI digest and observed guest
kernel release are retained; this does not claim cryptographic attestation of
the hypervisor or a measured guest boot.

## Physical authority boundary

Every compiler or candidate command uses a fresh named VM container with no
network, no DNS, a read-only image, all capabilities dropped, non-root UID/GID,
one CPU, 512 MiB memory, 64-process, 128-descriptor and 8 MiB file limits. Only the
private runtime (read-only) and current phase (writable) are mounted. Repository,
home, credentials, sibling phases and host source snapshots are not mounted.
An empty guest environment excludes `NODE_OPTIONS`, `NODE_PATH`, loader and
preload variables. `OPENSSL_CONF=/dev/null` prevents an ambient guest OpenSSL
configuration read without broadening filesystem authority. The exact named container is force-deleted after every
command, including attached CLI timeout; unconfirmed cleanup is an error.

Before executing Node, the launcher requires Landlock ABI 3 or newer and installs
`no_new_privs`, filesystem restrictions and a seccomp filter. Landlock permits
runtime and guest loader-library reads and only phase writes. No phase executable,
symlink, socket, FIFO or device creation is granted. Seccomp denies sockets,
non-thread process cloning, namespace changes, mounts, ptrace, BPF and cross-
process memory access. `clone3` returns `ENOSYS` so glibc uses the checked `clone`
path; `CLONE_THREAD` is required for allowed Node worker threads. This is a
syscall denylist: `execve` and `io_uring` are not denied by this filter. The
controller selects Node as the application entry, while Landlock and the outer
network-free VM retain their separate constraints. Evidence claims the tested
`fork()` and `socket()` denials, not universal process-execution denial or a
syscall allowlist. This is OS enforcement; the numeric VM
bridge only protects the assertion harness and is not the security boundary.

Each session physically verifies positive guest canary-open, fork/wait and
socket-create controls **before** restriction. Under the same policy it then
requires permitted phase read/write, `EACCES` for the existing sibling canary's
read/write, and `EPERM` for fork and socket. The canary is unchanged afterward.
The normal candidate launch omits the probe-only canary mount. Guest identity,
Node `v22.12.0` and TypeScript `5.8.3` are checked through the restricted launcher
before scoring. Failure is never converted into an admitted or passing result.

## Scoring and evidence

The session inherits the fixed source manifest, correction, frozen TypeScript
scorer, public/hidden candidate placement and numeric assertion bridge. Both
phases receive identical `validate.ts` bytes. The caller's shared plan must bind
`execution_profiles["linux-arm64"]` to this profile and the provision digest.
The session retains exact candidate/compiled/bridge/harness bytes, both outcomes,
container command arguments and streams, runtime inventory, pins and physical
probe observations. An unsuccessful candidate remains a failed row.

A guest-native Claude generation path with its own executable digest and fresh,
explicit login authority is required separately for a genuine second-environment
model trial. A candidate generated by Darwin and merely scored here is a scoring
replay, not independent model generation. This module performs no provider calls,
copies no credentials and does not claim second-environment live generation.

## Focused gates

The default gate exercises canonical receipt admission, immutable image and
source pins, bounded selected ELF extraction, policy construction and refusals:

```sh
python3 -m unittest discover -s benchmarks/cross-language-v1/agent/tests \
  -p test_pilot_linux_host.py -v
```

Set `SEMAPRAX_LINUX_LAUNCHER` to an explicitly provisioned launcher to run the
actual guest authority probe. Set `SEMAPRAX_LINUX_PROVISION` and
`SEMAPRAX_LINUX_PROVISION_SHA256` for the official full session success, wrong
result and early-exit gates. An optional create-new
`SEMAPRAX_LINUX_FIXTURE_EVIDENCE` path retains that fixture evidence. These gates
make zero provider requests. Skipped physical gates must not be described as
physical admission or current-head live trial evidence.

The 2026-10-02 local implementation gate ran all five selectors (no skips) in
42.296 seconds against the working implementation: actual official Linux
Node/TypeScript admission, positive candidate in both phases, wrong-result and
early-exit negatives in both phases, and physical authority probes. The evidence
is a local fixture run with zero provider calls; it does not close the independent
live-generation requirement. Its retained command streams and implementation
hash manifest distinguish this tested snapshot from later revisions.
