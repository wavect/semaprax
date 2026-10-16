# Structural call cursor storage

Status: private implementation evidence; current-head qualification follows CI.

Audience: compiler maintainers and reviewers.

The current hosted STD-08 job on `1e3a14ce7` refused a 24,672-byte request during
HIR validation (`116577327478`). The original structural call cursor requests
514 lexical child-cursor slots before lookup; its actual production layout
matches that request. The shared wrapper alone cannot prove it was the only
remaining cost. Caller-coordinate propagation makes later refusals attributable.

Follow-up hosted evidence on `6dec6f011` identifies the same-sized refused
request in supplemental receiving through `ordinary_inventory/storage.rs`.
The original numerical match did not identify its caller. The structural-call
change is a separately tested storage optimization; it did not resolve that
STD-08 refusal. [Supplemental cursor growth](../supplemental-cursor-growth-v1/README.md)
records the actual receiving owner and its separate qualification.

`calls.before.rs.txt` retains the complete original module. The independently
compiled `cursor_before.rs` contains its full original cursor owner, plus a
lookup forwarding function for controls. The source test reconstructs the entire
old module from current receivers and that original owner, and restores the
complete original lookup body from the new lookup with only storage operations
reversed. Every downstream signature, parameter, transfer and leaf check remains
byte-identical. `source-receipt.json` records complete-file hashes.

The new owner pays its inline header and every full replacement plus two vector
controls while the old backing is live. Each copied or initialized slot consumes
one existing work unit. Slots grow through 2, 4, 8, 16, 32, 64, 128, 256, 512 and
514. The raw traversal ceiling remains 514. Reuse checks the same payer's sticky
state. No limit, fixture, diagnostic expectation, quota refund or backend check
is weakened.

Focused local probes compiled actual new and complete original cursor code,
actual production ReplayBudget payment methods, PhysicalReservation and bounded
output with **synthetic HIR and child-cursor carriers**. They passed all growth
frontiers, exact full growth, every one-short frontier and sticky reuse, zero,
work refusal before backing acquisition, allocator overlap/release, shallow
bill reduction, source custody, every fixture lookup and missing diagnostic,
and raw 513/514/515 lookup boundaries. They also ran the authored caller-wrapper
control using a synthetic clone payload. Rust 1.97 and 1.88 compiler peaks were
below 156 MiB under a 240 MiB aggregate process cutoff. The isolated Clippy probe
passed with the repository's existing result_large_err allowance; omitted-code
unused imports/macros are allowed only in that scratch harness.

The authored real-HIR parity/depth cases and full owning unit/integration gates
have **not** run locally. These probes do not qualify actual HIR layouts,
application budget fit, runtime results, public SDK behavior or whole CI. Deep
walks pay more cumulative replacement bytes than the old eager buffer. Hosted
current-head qualification remains required.
