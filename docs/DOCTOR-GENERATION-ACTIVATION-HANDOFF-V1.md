# Doctor active-generation-to-provisioner handoff v1

Status: **specified, not implemented.** No code in this repository yet turns
an installed [signed generation](DOCTOR-SIGNED-INSTALL-V1.md) into the fixed
descriptor inventory the
[production provisioner](DOCTOR-PRODUCTION-PROVISIONER-V1.md#fixed-process-handoff)
consumes. This document records the design so implementation does not have to
rediscover it, per issue #61's required sequencing (specify before
implementing). It grants no authority by existing.

Audience: whoever implements the residual "active-generation integration"
item on issue #61.

## The gap this closes

[Doctor signed install v1](DOCTOR-SIGNED-INSTALL-V1.md) turns one
independently authenticated release into an immutable local generation and
lets a caller `activate`/`rollback`/`inspect_active` it. Its nine release
members (`request`, `bundle`, `launcher`, `worker`, `collector`,
`provisioner`, the signed capsule, the release manifest, and the manifest
signature — see `crates/semaprax-doctor-release/src/directory.rs`) sit as
ordinary files under a held store directory.

The [production provisioner](DOCTOR-PRODUCTION-PROVISIONER-V1.md) never reads
a pathname. It is a dedicated process with **no arguments** whose only
admitted state is exactly eleven inherited descriptors (0–2 pipes, 3–5 sealed
capsule/request/bundle, 6–8 executable launcher/worker/collector images, 9 an
empty delegated cgroup-v2 directory, 10 a trusted procfs root) — see
`crates/semaprax-native-rust-interop-platform-sys/src/doctor/offline_provisioner.rs::provisioned_doctor_provisioner_entry`.

Nothing in the repository connects the two. `crates/semaprax-doctor-release/src/main.rs`
exposes only `create` / `verify` / `key-info`; `install` / `activate` /
`rollback` / `recover` / `inspect_active` are library functions with unit-test
callers only (`crates/semaprax-doctor-release/src/install/unix.rs::tests`).
That is the residual named "active-generation integration" in issue #61's
reframed scope.

## Design

A new **generation activation supervisor** (proposed binary or library entry,
Linux-only, `#[cfg(all(target_os = "linux", target_pointer_width = "64",
target_endian = "little", any(target_arch = "x86_64", target_arch =
"aarch64")))]` matching the provisioner's own gate) does, in order, refusing
before any mutation on failure of any step:

1. **Resolve the generation.** Open the store (`open_store`), call
   `inspect_active`. Absent `ACTIVE` refuses; it never falls back to an
   arbitrary generation or an ambient binary.
2. **Hold and revalidate every member.** Reuse the store's existing
   held-descriptor revalidation shape (`hold_members` /
   `revalidate_members` in `install/unix.rs`, currently `pub(super)`; widen
   their visibility rather than re-implementing the no-follow,
   single-link-regular-file, digest-bound read). This is the "digest/signature/
   policy bound, rejects substitution" property the issue's required evidence
   names: the generation directory name is already the SHA-256 digest binding
   every member's name, executable bit, length and bytes
   (`DOCTOR-SIGNED-INSTALL-V1.md#layout-and-publication`), so re-deriving that
   digest from the held descriptors before use is substitution detection, not
   new trust.
3. **Reseal for the fixed handoff.** Feed the held `capsule` / `request` /
   `bundle` bytes through the existing
   `create_doctor_offline_input` factory (produces the sealed, immutable,
   non-executable memory file the provisioner's descriptors 3–5 require) and
   the held `launcher` / `worker` / `collector` bytes through
   `create_doctor_offline_executable` (descriptors 6–8). Both factories
   already exist and are HOSTED GREEN
   ([DOCTOR-SEALED-INPUT-V1.md](DOCTOR-SEALED-INPUT-V1.md)); this step reuses
   them rather than adding a second sealing path. Reading generation-store
   bytes into these factories is **not** the same operation as installing a
   release from a caller-supplied directory — the store is trusted local
   state under an exclusive lock, not an externally supplied path — and that
   distinction must stay explicit in any implementation and its tests.
4. **Delegate the cgroup-v2 scope (descriptor 9).** This is genuinely new
   host-privileged work, not a reuse of an existing library function: create
   an empty scope under an operator-prepared, already-delegated parent
   (`SEMAPRAX_DOCTOR_GATE_PARENT` is the precedent from
   `scripts/doctor-provisioned-linux-provision.sh`, for the *test* gate only)
   and write `+cpu +memory +pids` before handoff. The production supervisor
   must not invent its own delegation source (no walking `/sys/fs/cgroup` for
   a writable parent, no `sudo`); an undelegated parent refuses. The
   provisioner independently rereads the scope's control-file inventory after
   inheriting descriptor 9
   (`offline_provisioner/admission.rs::validate_cgroup_files`), so the
   supervisor's own correctness here is defense in depth, not the only check.
5. **Open the trusted procfs root (descriptor 10).** A plain open of the
   supervisor's own `/proc`; the provisioner independently requires it to
   report `PROC_SUPER_MAGIC`.
6. **Exec the generation's own `provisioner` image** — never the checked-out
   `doctor_provisioner` binary — as the new process image with exactly
   descriptors 0–10 open (anonymous pipes 0–2, the five items above at 3–10)
   and every other descriptor closed first. This is the same
   "closed inventory, held images, no pathname exec" discipline
   `DOCTOR-PRODUCTION-PROVISIONER-V1.md#namespace-and-cgroup-provisioning`
   already requires of the provisioner's own launcher handoff; the supervisor
   must meet it for its own exec too, including using `fexecve`/`execveat`
   over the held executable descriptor rather than a path, exactly as
   `create_doctor_offline_executable`'s sealed, `0500`, no-`PT_INTERP` image
   is designed to support.

## Refusal behavior (fail-closed, no ambient fallback)

- No `ACTIVE` generation, a corrupted/substituted member (digest disagreement
  on reread), a locked/contended store, a non-Linux/non-x86-64/AArch64 host,
  an undelegated or non-empty cgroup parent, or any sealing/exec failure all
  refuse before invoking the provisioner. None of these conditions may select
  an ambient tool, a different generation, or the checked-out provisioner
  binary — matching `DOCTOR-SIGNED-INSTALL-V1.md`'s "no pathname-only
  fallback" invariant and the ordinary CLI's existing "reports unavailable
  checks" behavior in `src/doctor.rs`, which this supervisor does not touch.
- Rollback and restart: because `ACTIVE` is a single held record whose pivot
  is proven sticky under injected faults
  (`post_pivot_faults_are_sticky_and_resolvable_by_inspection`,
  `crates/semaprax-doctor-release/src/install/unix.rs`), a supervisor that
  merely reads `inspect_active` before each invocation — rather than caching
  a generation id across restarts — automatically picks up an operator's
  `rollback` and never reactivates a revoked generation from stale state.
  That "read `ACTIVE` fresh every invocation, never cache" rule is the whole
  restart-safety argument and should be stated as a requirement on the
  supervisor, not left implicit.

## Nonclaims

- This does **not** activate the ordinary `semaprax doctor` CLI. `src/doctor.rs`
  has no reference to the install store, `ACTIVE`, or the provisioner and this
  document does not change that; ordinary CLI activation remains a separately
  gated, unimplemented decision (issue #61's acceptance criterion "Ordinary
  CLI activation remains unavailable until its own acceptance criteria pass").
- This does **not** implement macOS or Windows confinement. It is Linux-only
  by construction, same as the provisioner it feeds.
- This does **not** grant cgroup delegation authority; it consumes a
  precondition the trusted host/operator must already have prepared, the same
  precondition `scripts/doctor-provisioned-linux-provision.sh` states for the
  test gate.
- This document is not implementation evidence. No supervisor code exists;
  every module referenced above under `#[cfg(target_os = "linux", ...)]`
  cannot even be type-checked from a non-Linux development host, so writing
  and claiming this code without a Linux build/test cycle would be exactly
  the "unrun evidence presented as evidence" pattern issue #61's reframing
  exists to prevent. Implementation needs a session with real Linux
  build/test access (the hosted `ubuntu-latest` runner used for the
  provisioned-gate work is precedent that this is available, just not from
  this worktree), its own hostile-input fixtures for every refusal case
  above, and its own executable evidence gate before any status line here or
  in the owning specs may claim more than "specified."

## Open judgement calls for whoever implements this

- Whether the supervisor is a new dedicated binary (matching the provisioner's
  own "no argument surface" discipline) or a mode of an existing tool; a new
  dedicated binary is the safer default given the fixed-descriptor contract's
  "no environment-selected behavior" requirement.
- Whether `hold_members` / `revalidate_members` should be widened to `pub`
  (crate-visible) for reuse, or whether the activation supervisor should live
  inside `semaprax-doctor-release` itself to keep them private. Keeping them
  private and adding the supervisor to the same crate avoids widening a
  security-relevant internal API's visibility for a single external caller.
