# macOS doctor confinement and settlement contract v1

Status: implemented confinement/settlement primitive with local evidence only.
**Not HOSTED GREEN, not wired to any CLI route, not a completion-matrix
promotion.** This document is the macOS half of the split
[Issue #61](https://github.com/wavect/semaprax/issues/61) asked for; the
Windows half is [DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1](DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1.md).

Audience: release engineers, platform maintainers, and security reviewers.

## Purpose and boundary

[DOCTOR-PRODUCTION-PROVISIONER-V1](DOCTOR-PRODUCTION-PROVISIONER-V1.md) states
that macOS and Windows "need separate native confinement and settlement
contracts" and that "Linux evidence never promotes those hosts." This contract
is that separate definition for macOS. It does not extend, weaken, or reuse
the Linux contract's namespace/cgroup design: macOS has no unprivileged
namespace or cgroup-v2 equivalent, and this document does not pretend
otherwise. It also does not activate an ordinary `semaprax doctor --profile`
selector, does not become the production provisioner entry point
(`provisioned_doctor_provisioner_entry` remains Linux-only), and does not by
itself satisfy WP-05 for macOS.

What it does define, and what
`crates/semaprax-native-rust-interop-platform-sys/src/doctor/darwin_confinement.rs`
implements today:

1. A confinement primitive: a per-invocation compiled Seatbelt profile that
   denies filesystem writes and network access by default, and admits writes
   only under one explicit scratch root.
2. A settlement contract: four outcomes -- `Completed`, `Failed`, `Cancelled`,
   `Uncertain` -- that never collapse into each other, with a macOS-specific
   proof of "settled" analogous to the Linux contract's empty-cgroup check.
3. A sticky failure-selection state machine (`StickySettlement`) matching the
   non-negotiable invariant that cleanup can never replace an already-selected
   status.

It does not yet define: a signed release capsule, a fixed multi-descriptor
process handoff analogous to the Linux provisioner's descriptors 0..=10, a
sealed-input/executable-image format for this platform, or a dedicated
worker/collector split. Those remain open follow-on work, tracked by this
document rather than silently assumed complete.

## Confinement primitive

### Mechanism and why

The chosen mechanism is Apple's Seatbelt sandbox, applied through
`/usr/bin/sandbox-exec -f <profile-file> <command> <args...>`, not a direct
`sandbox_init(3)` FFI call. Both apply the identical kernel enforcement
(`man sandbox`, `man sandbox_init`); the difference is entirely in how the
calling process reaches it:

- `sandbox_init` is declared in a private, unheadered interface with no
  stable ABI or symbol-visibility contract across macOS releases, and Apple
  has progressively restricted direct callers of it outside its own daemons.
  Linking it from a general-purpose Rust binary is exactly the kind of
  "structurally valid but unattested" primitive this project's invariants
  warn against relying on.
- `sandbox-exec` is Apple's own shipped, documented command (`man
  sandbox-exec`) built on the same primitive. It is marked **DEPRECATED** in
  its own man page as of this writing (Developers are told to prefer the App
  Sandbox entitlement model for shipped applications). It remains present and
  functional on every tested host, including a current macOS 26.5.1 arm64
  host (`Darwin 25.5.0`, `xnu-12377.121.6~2`) as of 2026-09-12. This
  contract's [nonclaims](#nonclaims) record that dependency explicitly so a
  future Apple removal is a known risk, not a surprise.

The profile compiled for one invocation is:

```
(version 1)
(deny default)
(allow process-exec)
(allow process-fork)
(allow file-read*)
(allow sysctl-read)
(allow mach-lookup)
(allow file-write*
  (subpath "<canonical scratch root>"))
```

`deny default` is the base: every operation not explicitly allowed below it
is denied, including all outbound and inbound network operations (there is no
`(allow network*)` clause). `file-read*` is granted broadly because dynamic
linking a real tool (`rustc`, `clang`, `node`) requires reading shared caches
and libraries whose exact closure is not enumerated by this primitive today;
narrowing that to an explicit read closure, the way the Linux contract's
pivoted tmpfs root narrows the whole filesystem, is out of scope for this
version and is recorded as a real narrowing gap, not asserted as done.
`file-write*` is the one resource this primitive actually confines today: it
is admitted only under one `subpath`, everything else is denied.

### The symlink trap this primitive must not fall into

`std::env::temp_dir()` on macOS commonly resolves to a path under `/var`,
which is itself a symlink to `/private/var`. Seatbelt's `subpath` matcher
compares against the kernel's symlink-resolved path, not the literal string a
caller embeds in the profile. An earlier version of this implementation
embedded the unresolved scratch path directly and every write inside the
scratch root -- not just escapes -- failed closed with `EPERM`, because the
profile's literal `/var/...` text never matched the resolved `/private/var/...`
vnode path the kernel actually opened. `confined_spawn` therefore
canonicalizes the scratch root (`Path::canonicalize`) before compiling it into
the profile. The regression this fixed is exactly the "reads as denied for
the wrong reason" failure mode this project's invariants warn about: a naive
implementation would have looked *more* confined than it was, by accidentally
also denying legitimate writes, and a test that only checked "writes outside
scratch are denied" without also checking "writes inside scratch succeed"
would not have caught it. `healthy_confined_process_settles_completed_and_writes_only_inside_scratch`
is the regression test for this.

### Profile-injection resistance

The scratch root is embedded inside a Seatbelt string literal
(`(subpath "...")`). `escape_seatbelt_string` escapes `\` and `"` and rejects
an embedded NUL before that interpolation, so a scratch path containing a
literal `"` cannot close the string literal early and splice attacker-
controlled text into the compiled profile.
`scratch_root_containing_seatbelt_metacharacters_is_escaped_not_injected`
constructs a real scratch directory whose name contains `"` and `)` and proves
both that writes inside it still succeed and that writes outside it are still
denied -- i.e., that the metacharacters did not widen write authority.

### Process handoff (what is fixed today)

`confined_spawn(exe, args, scratch_root)`:

- Rejects a non-absolute `exe` or `scratch_root` before any spawn (`Invalid`).
- Rejects an absent or non-regular-file confinement launcher
  (`/usr/bin/sandbox-exec`) before any spawn (`Unsupported`) -- see
  [Fail-closed, never skipped](#fail-closed-never-skipped).
- Clears the child's environment (`env_clear`) rather than inheriting the
  caller's.
- Places the child in its own process group (`setpgid(0, 0)` in a
  `pre_exec` hook, which runs after `fork` and before `exec` and calls only
  the async-signal-safe `setpgid`), so the settlement step below can observe
  the confined *group*, not merely the direct child.

This is a fixed, deterministic handoff for the one child `sandbox-exec`
launches. It is not yet the Linux contract's fixed multi-descriptor handoff:
there is no sealed capsule, no separate worker/collector split, and no
explicit descriptor inventory beyond the three standard streams `Command`
already manages. Building that out is future work this document does not
claim is done.

### Fail-closed, never skipped

If `/usr/bin/sandbox-exec` is absent or not a regular file,
`confined_spawn_via` returns `ConfinementError::Unsupported` before writing a
profile or spawning anything. There is no fallback to an unconfined spawn.
`absent_confinement_launcher_fails_closed_never_silently_unconfined` asserts
this directly against a fixed nonexistent path, and additionally asserts that
the scratch directory it was given remains empty -- i.e., that the rejected
path performed no side effect it could have performed under partial admission.
This mirrors the existing doctor harness's documented property that "missing
provisioning is failure, never a skip"
(`crates/semaprax-doctor-collector/tests/provisioned.rs`).

## Settlement contract

```rust
enum Settlement {
    Completed,
    Failed(FailureReason),   // ExitCode(i32) | Signal(i32)
    Cancelled,
    Uncertain(UncertainReason), // WaitFailed | GroupStillPresent | KillAmbiguous
}
```

These four outcomes are deliberately distinct and never collapse:

- **Completed**: the confined process exited with status zero, and the whole
  confined process group is provably empty afterward (see below). This is the
  only outcome that reports success.
- **Failed**: the confined process exited nonzero, or was killed by a signal
  it did not choose to ignore, on its own -- not because the supervisor's
  deadline fired.
- **Cancelled**: the supervisor's own deadline fired before the process
  settled on its own. The supervisor sends `SIGKILL` to the negated process
  group (`kill(-pgid, SIGKILL)`), reaps it, and records `Cancelled` -- never
  `Failed`, because the operation did not fail by itself, it was stopped.
- **Uncertain**: settlement could not be proven. This is selected when: (a)
  the wait-equivalent observation itself errors; (b) the post-timeout kill
  could not be distinguished from "the process was already gone" (a kill that
  fails for a reason other than `ESRCH`); or (c) most importantly, **the
  confined process group still has a member after the primary process's own
  exit was otherwise decided**.

### The group-emptiness proof

The Linux contract's settlement rests on rereading `cgroup.events` and
requiring `populated 0` -- the whole delegated scope, not merely the direct
child, must be empty. macOS has no cgroup to reread. The analog this
contract uses is `kill(-pgid, 0)`: signal 0 performs no action beyond an
existence check, and `ESRCH` on the negated process group id is the kernel's
proof that no process in that group remains. This check runs unconditionally,
after every other settlement decision, and its result can *still* override a
tentatively decided `Completed` (see [Failure
selection](#failure-selection-is-sticky) below) -- because a live descendant
found only at this last step means the invocation was never actually settled,
regardless of what the primary process's own exit status said.

`a_descendant_left_behind_in_the_confined_group_settles_uncertain_not_completed`
is the regression test: a fixture spawns a detached `sleep 2` and exits
immediately with status zero. Without the group check, this would read as
`Completed`. With it, the still-running descendant is caught and the result
is `Uncertain(GroupStillPresent)`, exercising exactly the failure mode this
project's invariants call out ("a skip that reads as green is the defect to
avoid" generalizes here to "an escaped descendant that reads as complete").

### Failure selection is sticky

`StickySettlement` accepts exactly one terminal, non-`Completed` selection: once
`Failed`, `Cancelled`, or `Uncertain` has been selected, no later call --
including the group-emptiness proof, and including a redundant re-selection of
`Completed` -- can move it. A pending `Completed` is the only value a later
call may still override, because by itself it was never a complete proof of
settlement; the group-emptiness check that runs after it is part of proving
settlement, not cleanup that follows it.
`sticky_settlement_preserves_first_terminal_status_over_later_cleanup_attempts`
and `sticky_settlement_lets_a_pending_completed_be_overridden_by_a_later_terminal_status`
are the two regression tests for these two halves of the rule.

## Evidence

All tests below are `crates/semaprax-native-rust-interop-platform-sys/src/doctor/darwin_confinement/tests.rs`,
compiled and executed locally on this session's macOS host: `Darwin 25.5.0`
(`xnu-12377.121.6~2`, `arm64`, macOS 26.5.1 `ProductVersion`), Rust 1.98.0
(Homebrew). This is **local, this-mechanism-only evidence**. It is not hosted
CI evidence, and it says nothing about any other macOS version, architecture,
or Seatbelt implementation detail Apple has not documented.

```sh
cargo test --locked -p semaprax-native-rust-interop-platform-sys --lib doctor::darwin_confinement
```

12 tests selected, 12 passed, 0 failed, 0 ignored, 78 filtered out (the crate's
other doctor tests). The deliberate-escape pair
(`deliberate_confinement_escape_write_outside_scratch_is_denied_with_eperm`,
`deliberate_confinement_escape_network_connect_is_denied_with_eperm`) each run
the identical fixture argument once unconfined and once confined: unconfined,
the write succeeds and the network connect returns `ECONNREFUSED` (errno 61,
proving the kernel actually attempted the connect against a fixed closed
loopback port); confined, both instead return the specific Seatbelt denial
`EPERM` (errno 1) before the operation is attempted at all. The pairing is
what makes "denied" an attributable, specific outcome rather than merely "an
error occurred" -- an unconfined run that also failed would prove nothing
about confinement.

## Proposed gate integration (not landed; `.github/workflows/**` is out of
## scope for this contract's authoring session)

Unlike the Linux contract, this primitive needs no privileged provisioning:
`/usr/bin/sandbox-exec` is present on every stock macOS host, including
GitHub's hosted `macos-15` runners already used elsewhere in `ci.yml`'s
matrices (for example the `agent-proposal-clients` job at
`.github/workflows/ci.yml`). It performs no destructive namespace, cgroup, or
signed-capsule mutation, and does not require a disposable, dispatch-only
environment the way the Linux gate's `clone`/`pivot_root`/cgroup-delegation
sequence does. The proposed addition is therefore a normal PR-gated step, not
a separate `workflow_dispatch`-only workflow:

```yaml
      - name: Doctor macOS confinement primitive (real Seatbelt enforcement)
        if: runner.os == 'macOS'
        run: cargo test --locked --offline -p semaprax-native-rust-interop-platform-sys --lib doctor::darwin_confinement
```

This delta is not applied to any workflow file by this contract's authoring
session; `.github/workflows/**` is reserved for a maintainer's review, per
this repository's change protocol.

## Nonclaims

This contract does not: prove host-wide network silence beyond the outbound
connect this primitive's own tests exercise; confine or narrow the broad
`file-read*` allowance; define a signed release capsule, sealed-input format,
or fixed multi-descriptor handoff for macOS; activate an ordinary `semaprax
doctor --profile` selector; wire any new path into the CLI; run on any host
this session did not directly execute on; promote `docs/COMPLETION-MATRIX.md`
WP-05 for macOS; or assert that `sandbox-exec` will remain available in a
future macOS release, given its own man page already marks it deprecated. It
supplies one bounded local macOS confinement and settlement primitive,
analogous in shape (not in mechanism) to the Linux contract's confinement and
empty-scope settlement proof, for later extension into a full production
provisioner.
