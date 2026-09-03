# Linux production offline doctor provisioner v1

Status: private Linux implementation contract; physical distribution evidence
and ordinary CLI activation remain unrun and unpromoted.

Audience: release engineers, platform maintainers, and security reviewers.

## Purpose and boundary

This contract closes the previously external bootstrap and aggregate-settlement
boundary around the private offline doctor launcher, worker, and collector.  It
does not make an ordinary `semaprax doctor --profile` selector authoritative.
The production entry consumes one dedicated, single-threaded process with a
closed descriptor inventory, verifies one signed release capsule, creates the
private namespace context, installs aggregate cgroup limits, launches only held
images, and releases report bytes only after the complete owned cgroup is empty.

The first implementation is native 64-bit little-endian Linux x86-64 and
AArch64. Other hosts reject before interpreting capsule contents or changing
namespace/cgroup state. macOS and Windows need separate native confinement and
settlement contracts. Linux evidence never promotes those hosts.

The provisioner is a private distribution component, not an embedding API. It
never discovers a profile, executable, loader, configuration file, trust key,
or authority-bearing directory by pathname, `PATH`, environment, current
directory, registry, or request bytes. There is no unsandboxed fallback.

## Fixed process handoff

The dedicated process has no arguments and no environment-selected behavior.
Its only admitted inherited descriptors are:

| Descriptor | Exact object |
| --- | --- |
| 0, 1, 2 | Exclusive anonymous standard pipes; descriptor 1 is the final report sink |
| 3 | Immutable sealed signed release capsule |
| 4 | Immutable sealed worker request |
| 5 | Immutable sealed offline bundle |
| 6 | Immutable executable launcher memory file |
| 7 | Immutable executable worker memory file |
| 8 | Immutable executable collector memory file |
| 9 | Empty delegated cgroup-v2 directory for this invocation |
| 10 | Trusted procfs root used only for the pinned child's namespace maps |

No caller-owned ordinary filesystem object is admitted. The input and image
descriptors retain the sealed-input and executable-image properties from
[Doctor sealed input v1](DOCTOR-SEALED-INPUT-V1.md). Descriptor 9 must identify
cgroup v2, expose the exact required regular control-file inventory, be empty,
and be delegated for the fixed writes below. Descriptor 10 must identify procfs.
The child remains unreaped while its numeric proc entry is used, so the pidfd
and zombie identity exclude PID reuse. A string containing a PID never grants
authority by itself.

Closing an inherited descriptor can dispatch object-specific effects. The
caller therefore owns the clean fixed-inventory handoff and excludes foreign
descriptors, threads, reapers, signal handlers, and descriptor mutators before
entry. The provisioner independently checks every property it can observe; an
unobservable disagreement is a violated trusted-launch precondition, not a
reason to continue or retry.

## Signed release capsule

The canonical capsule has one versioned binary body followed by an Ed25519
signature. Its bounded body binds:

- native OS and architecture;
- the exact profile selector, doctor target, and required role mask;
- exact lengths and SHA-256 digests for the request and bundle;
- exact lengths and SHA-256 digests for launcher, worker, and collector images;
- the fixed capsule schema and no extensible or ignored trailing fields.

Verification uses the exact Ed25519 public key compiled through the release
builder's `SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX` input. Missing, malformed, or
noncanonical key material makes the production entry unavailable before clone
or cgroup mutation. The build input is a release trust anchor supplied by the
trusted build/review process; it is not authenticated merely because the binary
contains it. Developer builds without a production key fail closed.

Signature success authenticates only the canonical capsule under that release
key. The implementation separately reacquires every sealed carrier, compares
its complete bytes to the signed length/digest, parses the existing request and
bundle with their sole validators, and requires exact agreement on selector,
architecture, target, roles, bundle association, and image role. A digest,
nonce, filename, environment variable, descriptor number, or structurally valid
ELF image alone never mints admission.

All capsule parsing, signature verification, sealed acquisition, authenticated-
input allocation, and launch-inventory construction finish before namespace or
cgroup mutation. Limits bound each image and aggregate authenticated bytes, file
count, path bytes, signature work, and admission allocation. Cgroup-control
rereads and bounded report capture may allocate later. No partial admission
object is returned.

## Namespace and cgroup provisioning

The supervisor remains outside the tool cgroup and retains its pidfd and cgroup
directory for the complete invocation. It creates one child with fresh user,
mount, network, IPC, and UTS namespaces, a private descriptor table, and direct
placement into the admitted cgroup. Network, IPC, hostname, and mount-propagation
identities are private. Before executing the held launcher, the child overmounts
`/` with one fixed 64 KiB tmpfs, pivots into it without creating a pathname in the
inherited tree, detaches the old root, fixes both root and current directory,
remounts it read-only, and authenticates an empty
`readonly,nosuid,nodev,noexec` tmpfs. No old-root path or
descriptor survives. The existing worker later materializes and enters the
independently authenticated bundle root before executing an inspected tool.

Before releasing the child setup barrier, the supervisor uses the authenticated
procfs root and pinned child identity to install exact one-ID UID/GID maps and a
denied setgroups policy. It installs and rereads fixed cgroup-v2 limits for
process count, memory, and CPU. Required controllers and `cgroup.kill` support
must already be delegated; a missing kernel feature or permission rejects. The
provisioner never modifies host-wide policy or searches for another cgroup.

The child verifies parent-death ownership, resets inherited signal state, makes
mount propagation private, fixes its hostname, applies no-new-privileges, and
reconstructs the fixed descriptor table. The parent installs the exact ID maps
before releasing the setup barrier; those maps, namespace identities, and the
capability set are not redundantly reauthenticated by the child. It enters the
existing launcher through the held executable image with fixed arguments and a
fixed empty environment. The initial launcher, worker, and collector
images must be native static ELF images without `PT_INTERP`; a dynamic loader
cannot be reopened from the ambient root during held-image execution. The
launcher then retains its existing exact
request/bundle/image handoff to the worker and collector. No pathname exec,
shell, script fallback, or dynamic policy discovery is permitted. The kernel's
binfmt registration and helper policy remains a trusted launch precondition:
ELF structure and a held descriptor alone cannot prove the absence of a
matching externally registered handler.

Per-tool syscall admission is selected from the authenticated request role.
Every role shares a mandatory deny floor for process/namespace creation,
network, external IPC, tracing, descriptor acquisition, and privilege/resource
widening. Role-specific allowances are closed tables, not their union. Pointer-
argument authority such as `clone3` remains denied unless a later versioned
contract can validate it safely and supplies independent physical evidence.
Tool incompatibility selects an unsupported/failure row; it never retries under
a wider filter.

## Capture, settlement, and failure selection

The supervisor begins one absolute deadline before child creation, fairly
drains bounded stdout/stderr, and retains no more than the report limit. EOF is
not exit and a complete report frame is not settlement. The first selected
failure is sticky over cleanup. Output overflow, timeout, malformed report,
unexpected stderr, an exit outside the authenticated report policy's ordinary
zero/one statuses, a disagreed exit, descriptor uncertainty, cgroup
disagreement, or any authentication failure forbids an ordinary report.

Every post-clone path owns the child through a retained pidfd and the invocation
cgroup through descriptor 9. Failure attempts one cgroup kill, exact leader
observation/reap, and an empty-cgroup proof before the dedicated process exits
without publishing captured bytes. It does not claim a failure-path capture
drain or ordinary descriptor-close proof. Ordinary success likewise requires an exact authenticated exit status (zero for a
healthy report or one for an ordinary failed-check report), reap, complete EOF,
authenticated collector report bytes, and a reread proving `cgroup.events` reports
`populated 0`. Leader exit or process-group quiescence alone is insufficient.
Inability to prove kill, reap, closure, or empty-cgroup state is fail-stop: the
dedicated process terminates without later output or another tool action.

The settlement allowance is separately bounded from the operation deadline so
a timed-out invocation can still be quiesced. Neither bound is a hard real-time
guarantee for kernel calls. The provisioner does not delete cgroup directories,
change the parent's delegation, or infer settlement from a write to
`cgroup.kill`.

## Distribution and evidence gate

Production publication requires the Linux release archive to carry exact
provisioner, launcher, worker, collector, signed capsule, and public-key identity
metadata in a closed manifest. The gate must unpack that archive outside the
checkout, independently authenticate the release/capsule association, and run
real packaged Clang, Node, and Rust roles through the production provisioner.
Target-directory binaries, synthetic ELF fixtures, ordinary containers, BPF
interpreter tests, or a caller-supplied expected version string are insufficient.

The physical corpus must cover every forbidden filesystem/network/IPC/process/
capability route; role and image swaps; wrong key/signature/architecture/target/
selector; request/bundle/image mutation; loader/config omission; exact and
plus-one output; timeout; supervisor/launcher/worker/collector death; cgroup
limit and controller disagreement; post-leader descendants; close/kill/reap/
empty-cgroup uncertainty; and immutable request/bundle reacquisition. Missing
namespace, cgroup, sealing, or kernel prerequisites fail rather than skip.

The local packaging helper accepts only explicit absolute release, tar and gzip
tools plus artifact paths, builds a fresh no-clobber directory, verifies it,
then emits one ustar archive with an explicit sorted inventory, fixed modes, a
fixed 2000-01-01 timestamp, numeric root ownership and timestamp-free gzip
framing. It unpacks into a fresh directory and repeats verification against the
caller-supplied release identity and public key. Two invocations over identical
bytes must produce identical archive bytes. This is deterministic packaging of
already supplied artifacts;
it is not a reproducible compiler-build claim and does not establish that the
provisioner was compiled with the matching release trust anchor. Only the
required executable gate can establish that association.

The authority-free capsule/parser/policy/lifecycle tests and strict workspace
Clippy are necessary but not promotion evidence. WP-05 remains Partial until the
unpacked Linux distribution gate passes and equivalent separately specified
native boundaries exist for every platform the product claims.

## Nonclaims

This contract does not activate ordinary profile discovery, prove host-wide
network silence, eliminate a hostile kernel/binfmt/helper policy, trust the
kernel/LSM/VM, authenticate an arbitrary build host,
publish private crates, sign a release archive, support macOS or Windows, or
make SEMAPRAX production-ready by itself. It supplies one bounded Linux
least-authority execution and whole-cgroup settlement boundary for later
promotion.
