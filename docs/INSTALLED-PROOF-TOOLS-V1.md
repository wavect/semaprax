# Installed proof tools v1

This opt-in source-proof profile runs an explicitly selected installed Lean or
Z3 executable. It is separate from complete law coverage and never executes
an application entry point, foreign library, or application build script.

## Host selection and process bounds

`proof_export::installed::InstalledProofTool::open` requires absolute executable
and working-directory paths, a tool kind, the exact expected version line,
an explicit host profile, limits, and a monotonic cancellation handle.
Acquisition holds executable/cwd handles through the existing registered
process provider. There is no PATH lookup, inherited environment, tool install,
network fetch or implicit proof-artifact provider.

`TrustedLocal` explicitly authorizes that installed executable and its runtime
libraries with the host user's rights. It is not a sandbox: filesystem/network
access, memory, and a deliberately escaped descendant are not confined.
`Confined` refuses before acquisition because this adapter cannot enforce that
profile. The host must trust the selected executable, toolchain libraries and
source translation; a version string does not authenticate a malicious tool.

Linux and macOS use the existing held executable process provider. Other hosts
refuse. Arguments are closed to `--version`, Lean `--stdin`, or Z3 `-in -smt2`.
The child environment is empty. Use the actual installed Lean executable, not
an automatic-installation shim. All scripts pass through bounded stdin;
the adapter creates no proof files and does not load arbitrary caller scripts
from a proof-reference string.

Default version/run limits are 2/10 seconds. Each may be selected within
1–30,000 ms. Combined argv wire and stdin are at most 65,536 bytes; the result
wire is at most 65,536 bytes including its 32-byte header. Each stream has a
maximum of 32,752 bytes. One tool capability permits at most 16 launches and
1 MiB of cumulative input plus reserved output. Version probes use the same
runner, bounds, cancellation and settlement as proof execution. No unbounded
`Command::output` is used by this product route.

The provider kills/settles owned process groups before a successful return.
Timeout, cancellation, output overflow, nonzero exit, malformed or partial
output never produces checked proof. Failed settlement follows the existing
quarantine/fail-stop contract in [Bounded Process I/O v1](BOUNDED-PROCESS-IO-V1.md),
including its absence of a hard OS reap deadline. Abrupt host termination and
descendants that deliberately escape the trusted group are not confinement
claims.

## Exact Project evidence

`installed_project::prove_postcondition` selects an exact source/declaration/
postcondition in a retained Project, resolves it, and uses the existing closed
source translation. Missing, unselected or unsupported subjects refuse before proof work. The
declaration must belong to an existing entry/public/test HIR view; proof
requests cannot silently expand Project assurance coverage.
Lean generates the existing certificate, kernel-checks the exact export, then
replays its ProgramRoot binding through the existing opaque attachment path.
The semantic Lean pin is `leanprover/lean4:v4.34.0`, separate from the host's
exact version-line pin.

Z3 receives the existing checked-arithmetic SMT formula with only the trailing
model-retrieval command omitted. A strict success requires exit 0 and exactly
`unsat` plus surrounding whitespace. `sat`, `unknown`, surplus output and
partial answers refuse. The opaque Project attachment binds script bytes,
source revision/digest, Project/ProgramRoot, postcondition and exact version.
It claims source-level checking under the trusted translator, not proved
lowering or independently checked SMT proof objects.

Strict Law Assurance's `pinned_smt_source` requirement additionally requires
the exact tool version and the accepted frozen SMT translation profile. The
unpinned `smt_source` requirement continues to refuse. Native relational-law
synthesis and complete protected-route configuration remain open #379 work.

## CLI

```text
semaprax project-proof-check /absolute/semaprax.toml \
  --tool lean|z3 --executable /absolute/installed/tool \
  --version-line "exact version output" --host-profile trusted-local \
  --source src/app.spx --declaration app.function --ensures 0
```

All options are required; duplicates and unknown options are syntax errors.
The result schema is `semaprax.installed-project-proof-check.v1`, including the
exact Project assurance report, host limits and explicit nonclaims. This route
checks one selected postcondition; it does not claim complete law coverage,
application execution, build admission or publication authority. A false or
unsupported proof exits unsuccessfully without mutating source or Git.

## Focused physical gate

The ignored Workspace selector `project_assurance_manifest::law_set::installed_law`
requires explicit `SEMAPRAX_LAW_LEAN`, `SEMAPRAX_LAW_LEAN_VERSION`,
`SEMAPRAX_LAW_Z3`, `SEMAPRAX_LAW_Z3_VERSION` and `CLANG` provisioning, then
runs with `--ignored`. Missing provisioning fails; it never silently skips.
The gate covers both real kernels and CLI calls against a newly authored
postcondition plus false/stale/trust changes, hanging/overflow/malformed probes,
cooperative cancellation and owned descendant settlement. Recorded callbacks
cannot substitute for these physical invocations.

Local gate result (2026-10-03): 3 passed, 0 failed, 211 filtered; 10.30s
using Lean 4.34.0 (commit `293d5d0c0c3f3dded4688b3ccd6a33939ac5102b`) and
Z3 4.12.5 on arm64 macOS. The exact invocation was the Workspace selector
above with `--offline --locked`, `--ignored --test-threads=1`, one Cargo job,
debug info disabled and explicit installed paths/version pins. This is local
source-proof/process evidence, not cross-platform confinement or complete
LAW-04 admission.
