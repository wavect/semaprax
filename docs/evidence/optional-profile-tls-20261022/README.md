# Optional PLAN profiling storage and low-stack diagnosis

The original Linux unit-profile run 39816691203 at main 86c08798e7
faulted in lexical_exit::walk::scan during HirValidator::new inside the
unchanged 128 KiB indexed-match admission test. This capture is a failing
diagnostic, not a passing test or a full-public-validation qualification.

The optional PLAN observer now keeps a pointer in thread-local storage and
prepares its fixed-capacity backing only on explicit Capture::begin. Inactive
compiler callbacks do not allocate. The entire State declaration and initializer,
all row capacities, and the Report layout remain byte-identical. Existing
counter, overflow, reentry, unwind, and thread-isolation tests are retained.

Local checks use actual production ledger owners and all ten original/current
PLAN observer test bodies in a focused harness; unrelated compiler test modules
are excluded. The new inactive-path case uses 128 KiB and observes zero actual
allocations. This is narrower than the complete compiler test binary.
All-features production Clippy and library compilation are checked separately.
The Linux diagnostic also records the real executable's ELF TLS layout.
Current-head Linux execution and the full CI matrix remain separate gates.
