# Provisioned Linux offline doctor lifecycle gate v1

Status: authored and **never executed**. No provisioned host exists, no run
has ever been performed, and nothing in this document is evidence that the
private Linux doctor boundary was demonstrated at runtime. It records what a
maintainer must provision and what the gate would then observe.

Audience: release engineers and security reviewers who can supply one
disposable, trusted Linux x86-64 host.

Owning contract: [Linux production offline doctor provisioner
v1](DOCTOR-PRODUCTION-PROVISIONER-V1.md). This document adds the executable
gate that contract's distribution and evidence section requires; it changes no
admission rule, activates no ordinary CLI route, and promotes no completion
row.

## What is true today

The private Linux doctor boundary has substantial implementation and
source-layout coverage, and no runtime confinement evidence.

- `tests/doctor_production_provisioner_v1.rs` reads the provisioner's source
  text: it pins the fixed descriptor inventory, the capsule parser, the clone
  flags, the pivoted read-only tmpfs root, and the absence of an ordinary CLI
  activation path, and it proves those tripwires reject representative
  widening mutations. It is a textual gate. It cannot observe a namespace, a
  cgroup, a syscall, or a settled descendant.
- Twenty-six lifecycle tests are `#[ignore]`d because they need a host nobody
  has provisioned. They are listed under [test selection](#test-selection)
  below, and each `#[ignore]` reason names the missing prerequisite rather
  than disabling the case.
- `docs/DOCTOR-PRODUCTION-PROVISIONER-V1.md` already states that physical
  distribution evidence and ordinary CLI activation remain unrun and
  unpromoted, and that missing namespace, cgroup, sealing, or kernel
  prerequisites fail rather than skip. Nothing here weakens that.
- `docs/COMPLETION-MATRIX.md` keeps WP-05 `doctor` unpromoted, and this
  document does not change it. A status change requires a run.

## The gate

`scripts/doctor-provisioned-linux-gate.py` is the whole gate.
`.github/workflows/doctor-provisioned-linux.yml` is one dispatch-only job that
invokes it on a dedicated self-hosted runner.

The workflow is deliberately outside `ci.yml`. Issue #61 scopes the run to a
disposable trusted environment and does not authorize a privileged workflow
over arbitrary fork input, so the gate is never triggered by `push` or
`pull_request` and never sees a fork's contents.

### Failing closed is the point

The single behaviour this gate exists to guarantee is that **absent
provisioning is a failure, not a skip**. GitHub treats a skipped check as a
satisfied required status check, so a gate that skips when its host is
unprovisioned reports the same green as a gate that proved confinement. That
is the exact confusion the owning contract forbids.

Accordingly:

- the job carries no `if:` condition, no `continue-on-error`, and no
  conditional execution step;
- `precondition_failures` returns a nonempty list for every unmet
  precondition, and an unmet precondition exits 1 before any test runs;
- a precondition the probe could not *observe* is treated exactly like one
  observed to be false — the contract calls an unobservable disagreement a
  violated trusted-launch precondition, not a reason to continue;
- `libtest_failures` rejects a harness outcome in which the named tests did
  not actually run: a nonzero `ignored` count, a `0 passed` summary, an
  unmatched filter, a narrowed selection, a missing `test result:` line, or a
  test that ran outside the selection are each a failure at exit code 0.

`--self-test` drives those functions with synthetic inputs and needs no Linux
host. It proves rejection of a missing kernel feature, absent cgroup-v2
delegation, a wrong architecture, a missing or dynamically linked binary,
unprovisioned context and fixture variables, a release built at another
commit, a production trust anchor, an unsettled cgroup, and a
skipped-rather-than-run harness outcome. It is evidence that the gate
refuses. It is not, and must never be reported as, evidence that the gate
passed.

## Host provisioning

The gate does **not** create the environment. It asserts one. The trusted host
owner supplies a wrapper that establishes the end state below and then execs
the script; the contract already makes the caller, not the provisioner, the
owner of the clean fixed-inventory handoff.

Required end state when the script starts:

| Requirement | Why |
| --- | --- |
| Linux x86-64, 64-bit little-endian | The gate admits exactly one target. AArch64, Windows and macOS stay separately tracked and are never generalized from this run. |
| A disposable, dedicated host | The harness is a *trusted* provisioner outside both offline guarantees; it must not run beside other workloads or over untrusted input. |
| Kernel features: `clone3`, `CLONE_INTO_CGROUP`, `close_range`, `MFD_EXEC`, memfd sealing, `no_new_privs`, `openat2`, `pidfd_open`, `pidfd_send_signal`, `pivot_root`, seccomp filters, unprivileged user namespaces | Each is used by the provisioner, the worker policy, or the sealed-memfd surrogates. |
| A private mapped user + mount namespace | `SEMAPRAX_DOCTOR_ROOT_TEST_CONTEXT` and `SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT` acknowledge it; the acknowledgement is not attestation. |
| An empty delegated cgroup-v2 scope, path in `SEMAPRAX_DOCTOR_GATE_CGROUP` | Must expose `cgroup.controllers`, `cgroup.events`, `cgroup.kill`, `cgroup.procs`, `cgroup.subtree_control`, `cpu.max`, `memory.max`, `pids.max`; carry the `cpu`, `memory` and `pids` controllers; accept the fixed writes; and report `populated 0` with no members. |
| Immutable current-head `SEMAPRAX_DOCTOR_LAUNCHER`, `SEMAPRAX_DOCTOR_WORKER`, `SEMAPRAX_DOCTOR_COLLECTOR` | Absolute physical regular files, executable, native static ELF without `PT_INTERP`. A dynamic loader cannot be reopened from the ambient root during held-image execution. |
| A real bundle in `SEMAPRAX_DOCTOR_REAL_BUNDLE` with its exact `SEMAPRAX_DOCTOR_REAL_SELECTOR`, plus independent `SEMAPRAX_DOCTOR_EXPECTED_{CLANG,NODE,RUST}_DETAIL` | The real-distribution fixture must not manufacture its own oracle from observed output. |
| A signed release archive unpacked outside the checkout, identified by `SEMAPRAX_DOCTOR_GATE_RELEASE_{ROOT,ARCHIVE,MANIFEST,CAPSULE,COMMIT}` | Target-directory binaries and synthetic ELF fixtures are explicitly insufficient. |
| `SEMAPRAX_DOCTOR_GATE_TRUST_ANCHOR=test-only`, no production signing material in the environment | The gate signs disposable fixtures only. No production signing material belongs in test output. |
| A clean checkout at the same commit the release archive was built from | The evidence binds exact source bytes. |
| `SEMAPRAX_DOCTOR_GATE_DISPOSABLE=yes` | An explicit operator declaration that this host is disposable and trusted. It is a declaration, not a proof, and every other check still applies. |

The gate derives most kernel-feature answers from the reported kernel release
and records that basis in the evidence under `kernel_feature_basis`. A version
is a necessary condition, not a runtime capability proof: a distribution
kernel can carry the version with a feature compiled out or policy-blocked.
The fixtures themselves still fail rather than skip when a syscall is
unavailable, so the probe rejects early without claiming to be the authority.

No wrapper is checked in, and no wrapper has been written or tested. Supplying
one — and proving its namespace, cgroup delegation and image immutability
properties — is the maintainer's remaining work.

## Test selection

The gate runs two suites serially, in this order, with an exact `--exact
--ignored --test-threads=1` selection and no wildcard. `--plan` prints the
exact commands.

`cargo test --locked --offline -p semaprax-doctor-collector --test provisioned`

| Test | Case the issue enumerates |
| --- | --- |
| `actual_worker_materializes_executes_and_settles_before_canonical_report` | Healthy materialize/execute/settle |
| `literal_reply_surrogates_reject_cross_binding_and_malformed_frames` | Forged, cross-bound and truncated replies |
| `complete_literal_frame_followed_by_nonzero_exit_never_becomes_a_report` | A complete frame is not settlement |
| `complete_frame_and_capture_eof_each_still_require_worker_exit` | Deadlines; two real 60-second budgets |
| `created_handoff::production_created_native_and_all_files_reach_worker_and_reject_digest_drift` | Sealed carrier handoff and digest drift |
| `prepared_handoff::prepared_native_and_all_role_handoffs_preserve_literal_wire_and_reject_transport_drift` | Immutable request/bundle reacquisition |
| `launched_handoff::production_launcher_reports_native_and_all_from_literal_transport_files` | Real launcher, native and all roles |
| `launched_handoff::production_launcher_rejects_both_image_defects_and_digest_drift` | Image swap and mutation |
| `launched_handoff::production_launcher_rejects_structural_collector_with_missing_loader` | Failure during bootstrap; loader omission |
| `physical_reports::all_three_roles_settle_and_tool_failure_is_an_ordinary_exit_one_report` | Ordinary failed-check report at exit one |
| `physical_reports::closed_report_sink_fails_after_collection_without_successful_delivery` | Output overflow and closed report sink |
| `nonchild::nonchild_pidfd_rejects_without_killing_or_stopping_the_owned_sentinel` | Descendant survival; no foreign signal |
| `real_launched_handoff::production_launcher_reports_all_roles_from_provisioned_real_distributions` | Real packaged Clang, Node and Rust roles |

`cargo test --locked --offline -p semaprax-native-rust-interop-platform-sys
--lib`

| Test | Case the issue enumerates |
| --- | --- |
| `doctor::offline_worker::tests::provisioned_materializer_exec_and_socket_denial` | Forbidden process and network routes |
| `doctor::offline_worker::tests::provisioned_overflow_and_timeout_publish_only_settled_failure` | Output overflow and timeout settlement |
| `doctor::offline_worker::tests::provisioned_missing_role_bad_hash_and_invalid_request_emit_no_frame` | Role swap, wrong digest, invalid request |
| `doctor::offline_worker::tests::provisioned_real_clang_node_rust_distributions` | Real tool distributions through the worker |
| `doctor::offline_worker::tests::hostile::provisioned_capability_operations_and_process_creation_are_denied` | Capability and process-creation denial |
| `doctor::offline_worker::tests::hostile::provisioned_stdin_is_eof_and_nonstandard_descriptors_are_closed` | Descriptor closure |
| `doctor::offline_worker::tests::hostile::provisioned_root_hides_real_outside_file_and_rejects_write_opens` | Filesystem confinement |
| `doctor::offline_worker::tests::lifecycle::post_exec_capabilities_and_supervisor_death_are_observed_externally` | Supervisor/child lifecycle and cancellation |
| `doctor::offline_root::linux::tests::provisioned_detached_root_bytes_modes_and_read_only` | Detached read-only root materialization |
| `doctor::offline_root::linux::tests::provisioned_wrong_page_cost_stops_before_tree_writes` | Pre-effect bounds |
| `doctor::offline_root::linux::tests::provisioned_setup_and_exact_write_failures_return_no_root` | Failure during bootstrap |
| `doctor::offline_root::linux::tests::provisioned_metadata_mismatches_feed_actual_admission` | Metadata disagreement |
| `doctor::offline_root::linux::tests::provisioned_close_uncertainty_is_fail_stop` | Close uncertainty is fail-stop |

These are the existing hostile fixtures. The gate substitutes no weaker smoke
test for any of them, and adding a case here means adding it to the owning
harness first.

`selection_drift` parses the `#[ignore]` inventory of each owning file and
fails when the selection and the inventory disagree in either direction, so a
new ignored lifecycle case that nobody adds here is a failure rather than a
silently narrower gate. `--self-test` runs that comparison against the real
tree and separately proves a synthetic narrowing is detected, so the check
cannot pass vacuously against an inventory the parser never found.

Two `#[ignore]`d functions in the doctor tree are deliberately excluded:
`doctor::offline_input::create::tests` and its executable-fault sibling are
private subprocess helpers selected by their own parent tests, not gates. So
are the Windows revision-store and `owned_npm` symlink fixtures, which belong
to separately tracked hosts.

Residual gap: `selection_drift` compares against a checked-in list of owning
files. A brand-new *file* of ignored lifecycle tests, added without extending
that list, would not be noticed. The list is small and sits beside the
selection it guards, but it is a list, not a directory walk.

## Evidence

A run writes one JSON document, `semaprax.doctor.provisioned-linux-gate.v1`,
binding:

- `probe.revision`: the checked-out 40-digit commit and whether the tree was
  clean;
- `probe.host`: system, architecture, pointer width, byte order, kernel
  release and version;
- `probe.kernel_features` and `probe.kernel_feature_basis`: each required
  feature and how it was decided;
- `probe.cgroup`: the delegated scope's path, filesystem, controllers, control
  files, writability, `populated` value and membership before the run;
- `probe.images`: each held image's absolute path, size, SHA-256, ELF machine,
  static/`PT_INTERP` status;
- `probe.release`: the unpacked release root, whether it lies outside the
  checkout, archive/manifest/capsule digests, the build commit, and the trust
  anchor;
- `probe.fixture`: the real bundle's path, SHA-256 and selector;
- `probe.tools`: exact `cargo -Vv` and `rustc -vV` output and resolved paths;
- `suites[]`: the exact argv, the expected test list, exit code, elapsed time,
  and a bounded capture of stdout and stderr with byte count, SHA-256 of the
  full stream, and an explicit `truncated` flag;
- `final_cgroup`: the same scope reread after the run, proving `populated 0`
  and no surviving members;
- `failures[]` and `selected_failure`: every reason in order, with the first
  one selected.

Failure selection is sticky. Cleanup and settlement observations are appended
after execution and can never replace the first selected failure, so a run
that failed while executing and *also* failed to prove an empty cgroup still
reports the execution failure as its verdict.

## What a maintainer must do to execute this gate

1. Provision one disposable, trusted Linux x86-64 host satisfying every row of
   [host provisioning](#host-provisioning), and write the wrapper that
   establishes the private namespace and the empty delegated cgroup-v2 scope.
2. Build the current-head launcher, worker, collector and provisioner with the
   test-only `SEMAPRAX_DOCTOR_RELEASE_PUBLIC_KEY_HEX` anchor, package them with
   `scripts/package-doctor-release.sh`, and unpack the archive outside the
   checkout.
3. Acquire dependencies once with `cargo fetch --locked`; both suites then run
   `--locked --offline`, so the gate performs no build-time network access.
4. Register the host as a self-hosted runner labelled `self-hosted`, `linux`,
   `x64`, `semaprax-doctor-provisioned`.
5. Dispatch `.github/workflows/doctor-provisioned-linux.yml`, or run
   `python3 scripts/doctor-provisioned-linux-gate.py --evidence <path>` under
   the wrapper directly.
6. Record the run link, commit and evidence digest in
   [DOCTOR-PRODUCTION-PROVISIONER-V1.md](DOCTOR-PRODUCTION-PROVISIONER-V1.md)
   and only then consider a WP-05 status change in
   [COMPLETION-MATRIX.md](COMPLETION-MATRIX.md).

## Nonclaims

This gate does not activate ordinary `semaprax doctor --profile` selection,
wire the private provisioner into any CLI route, prove host-wide network
silence, trust the kernel, LSM or VM, authenticate an arbitrary build host, or
support AArch64, Windows or macOS. It grants no promotion by existing. Until a
run is recorded, every claim about the private Linux doctor boundary's runtime
behaviour stays exactly where the owning contract left it: unrun and
unpromoted.
