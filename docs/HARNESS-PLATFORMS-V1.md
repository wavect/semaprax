# Harness supported platforms v1 (HN-18)

Status: additive development-harness specification; results are executed evidence only, dated 2026-10-04.

Audience: harness users, packagers and release engineers.

Cells are `pass`, `fail` or `untested`. A cell is `pass` only when it ran on that platform; an absent dependency
is `untested` with its reason and never counts as success. `scripts/harness_dist.sh smoke-core` prints one `CELL` line
per cell and exits 3 (`SMOKE INCOMPLETE`) while any cell is untested; only all-pass prints `SMOKE PASSED`.

## Declared first matrix

| Platform | Status |
| --- | --- |
| macOS arm64 (aarch64) | supported: every cell passes (local) |
| Linux x86_64 | supported: every cell passes on GitHub-hosted `ubuntu-latest` |
| Linux arm64 (aarch64) | supported: every cell passes on GitHub-hosted `ubuntu-24.04-arm` |
| Windows (native) | **unavailable**: the host is Unix-only (`process_group`, `rustix`); WSL is not native Windows evidence |

## Results

Hosted evidence: workflow `.github/workflows/harness-platforms.yml`, run
https://github.com/wavect/semaprax/actions/runs/37231313077 (commit
`432e5c633`, wavect/v090 with main merged), artifacts `harness-cells-x86_64`
and `harness-cells-aarch64`. Ubuntu 24.04 needs
`kernel.apparmor_restrict_unprivileged_userns=0` for unprivileged `bwrap`;
the workflow sets it and probes `bwrap` before the suite.

| Cell | macOS arm64 (local) | Linux x86_64 (hosted) | Linux arm64 (hosted) |
| --- | --- | --- | --- |
| Harness unit + integration suite (`--lib --test harness_v1`) | pass | pass (56 + 382) | pass (56 + 382) |
| Bridge cancellation of a blocked hostile adapter, grandchild reaped, concurrency, floods, crash, uncertain-effect journal (`bridge::lifecycle`) | pass | pass | pass |
| Required isolation without a sandbox refuses; plain subprocess never called isolated | pass | pass | pass |
| OS-enforced isolation actually applied | pass (`sandbox-exec`) | pass (`bwrap` 0.9.0) | pass (`bwrap`) |
| Fresh install of the tarball outside the checkout (empty HOME, `PATH=/nonexistent`) | pass | pass | pass |
| Default skills list and load offline | pass | pass | pass |
| Relocated distribution and asset paths | pass | pass | pass |
| Offline reuse: repeat setup is a no-op, harness home byte-identical | pass | pass | pass |
| Native-only workflow task from the installed package (compiler built on the runner) | pass | pass | pass |

Earlier local evidence in an Apple `container` Debian image (Linux arm64) is
superseded by the hosted run; real third-party tools (Graft, Graphify, RTK)
are evidenced on macOS only. "Offline" means no network is attempted by
design; the hosted run does not deny it at the OS level.

## Reproduce

```sh
scripts/harness_dist.sh build --binary <semaprax-harness> --out <dir> [--platform <os-arch>]
scripts/harness_dist.sh smoke-core --tarball <tarball> [--compiler <semaprax>]
cargo test --offline -p semaprax-harness --test harness_v1 bridge::lifecycle      # needs python3
SEMAPRAX_COMPILER=<semaprax> cargo test -p semaprax-harness --test real_tools_v1 hn18_core -- --ignored
```
