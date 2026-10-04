# Harness supported platforms v1 (HN-18)

Status: additive development-harness specification; results are executed evidence only, dated 2026-10-04.

Audience: harness users, packagers and release engineers.

Cells are `pass`, `fail` or `untested`. A cell is `pass` only when it ran on that platform; an absent dependency
is `untested` with its reason and never counts as success. `scripts/harness_dist.sh smoke-core` prints one `CELL` line
per cell and exits 3 (`SMOKE INCOMPLETE`) while any cell is untested; only all-pass prints `SMOKE PASSED`.

## Declared first matrix

| Platform | Status |
| --- | --- |
| macOS arm64 (aarch64) | supported: every cell below passes |
| Linux arm64 (aarch64) | partially evidenced: distribution cells pass; adapter and compiler cells untested (below) |
| Linux x86_64 | declared in adapter descriptors, **untested**: no x86_64 host was executed (Rosetta on arm64 is not evidence) |
| Windows (native) | **unavailable**: the host is Unix-only (`process_group`, `rustix`); WSL is not native Windows evidence |

## Results

| Cell | macOS arm64 | Linux arm64 (Debian bookworm container, Apple `container`) |
| --- | --- | --- |
| Fresh install of the tarball outside the checkout (empty HOME, scrubbed env) | pass | pass |
| Default skills list and load offline | pass | pass |
| Relocated distribution and asset paths (no old path recorded) | pass | pass |
| Offline reuse: repeat setup is a no-op, harness home byte-identical | pass | pass |
| Native-only workflow task from the installed package | pass (prebuilt compiler) | untested: no Linux semaprax compiler; building it was excluded (size, disk) |
| Bridge cancellation of a blocked hostile adapter, grandchild reaped, concurrency, floods, crash, uncertain-effect journal (`harness_v1 bridge::lifecycle`, 8 tests) | pass | untested: the image has no `python3` (the hostile adapter is Python) and no package was fetched |
| Required isolation without a sandbox refuses; plain subprocess never called isolated | pass (`sandbox-exec` present; refusal tested with an unavailable backend) | untested (same Python dependency); note `bwrap` is absent in the image, so a real required-isolation request would be refused |
| OS-enforced isolation actually applied | pass (`sandbox-exec`) | untested: no `bwrap` |

Notes. Offline means the run needs and attempts no network by design; it is not network-denied by the OS.
The Linux arm64 tarball was assembled on the host with the binary built natively inside the container
(`rust:1.98.0-slim-bookworm`, offline from the host cargo registry), because the catalog step needs Python;
the smoke ran in a fresh container. The shipped adapter descriptors list `macos-aarch64` and `linux-x86_64` only,
so a bundled Python/Node adapter is not selectable on `linux-aarch64` until a descriptor lists it (descriptor
change requested; descriptors are outside this lane).

## Reproduce

```sh
scripts/harness_dist.sh build --binary <semaprax-harness> --out <dir> [--platform <os-arch>]
scripts/harness_dist.sh smoke-core --tarball <tarball> [--compiler <semaprax>]
cargo test --offline -p semaprax-harness --test harness_v1 bridge::lifecycle      # needs python3
SEMAPRAX_COMPILER=<semaprax> cargo test -p semaprax-harness --test real_tools_v1 hn18_core -- --ignored
```
