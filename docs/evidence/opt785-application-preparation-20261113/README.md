# OPT #785 historical Project preparation

Three fresh processes per arm called the production `project::with_authenticated_project` entry point against the same on-disk manifests. The before arm is `bd0d131cbcb3e0528804a16b1d3e5c952a16051e`, the parent of the shared-backing change. The after arm is `a3d51efe842285213f69b90f186721c0a031ad35` with the two scoped-task lookup dereferences from `8c7c67968` applied. Without those dereferences the after arm fails to compile because the new borrowed declaration key makes the lookup type ambiguous. Both revisions have identical application trees, Cargo manifests, and lockfile.

| Application | Reserved at first refusal | Requested allocation bytes, before → after | Peak live requested bytes, before → after |
| --- | ---: | ---: | ---: |
| Catalog | 67,108,650 | 304,871,408 → 304,448,979 | 5,262,275 → 5,238,653 |
| ShiftSim | 67,104,967 | 204,483,812 → 204,019,799 | 5,292,916 → 5,288,590 |
| Catalog Normalizer | 67,108,748 | 48,367,549 → 48,329,988 | 6,190,230 → 6,166,614 |

Both arms produced the same complete diagnostic for each application. All 18 runs reached the first sticky HIR final-validation refusal at the existing 67,108,864-byte limit. Reserved bytes are the limit minus the remaining balance in that diagnostic, with a zero floor. Allocation and peak columns are medians from the thread-local allocator observer. It counts successful Rust layout requests and computes the live total from tracked allocations. Reallocation uses the system allocator and records the resulting requested layout. Allocator internals and other threads are outside this observation.

The samples cover preparation and teardown through refusal. Successful application admission, graph parity on these applications, and live-agent outcomes remain unqualified. Elapsed time is retained per sample. The host was also running builds, so these samples do not establish a timing improvement.

To reproduce, use separate checkouts of the two recorded revisions and apply the supplied patch only in the after checkout. Build the library in each checkout with the same arguments and profile in `observations.json`. Copy the probe to a `.rs` path and link it against each checkout's `libsemaprax.rlib` with the corresponding `target/debug/deps` and native build output search paths. Pass the same original `semaprax.toml` path to both binaries and alternate arms. The receipt records the manifest and application tree digests, probe and binary SHA-256 hashes, and each sample's stdout hash, allocation tuple, and elapsed time.

[Observations](observations.json) · [Probe source](project-allocation-probe.rs.txt) · [After-arm build patch](scoped-task-compile-fix.patch)

## Current compiler application checks

The production Project reporting route admitted Catalog, ShiftSim, and Catalog Normalizer at the shipping 128 MiB limit on `7ba538cc2`. At the original 64 MiB limit, an isolated build of `0a68f5420` changed only the limit supplied to that same reporting route. Catalog completed with 48,615,173 bytes of cumulative core reservations. ShiftSim refused during live construction after reserving 67,104,702 bytes. Normalizer refused during static admission before entering a resolved-core attempt.

The current checks use the existing compiler profiling feature and exercise Project preparation and `snapshot.check()`. The results give per-attempt reservations where construction ran. The historical allocator observations above cover a different pair of builds and cannot be compared to these reservations as allocation or peak measurements.

[Builder receipts](current-builder-limits.json) · [Project probe](current-project-probe.rs.txt) · [64 MiB build patch](current-builder-limit.patch)
