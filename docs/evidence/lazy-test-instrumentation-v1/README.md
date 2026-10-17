# Lazy backing for test instrumentation

Status: private test infrastructure; original hosted low-stack case and full CI qualification pending.

Audience: compiler contributors and reviewers.

The original 128 KiB indexed-match test still faulted during constructor setup
in diagnostic run 38978690028, job 117234182109, at main `3d5bef332`. GDB stopped
in the third page probe of the lexical scan. Its prologue reserves 12,360 bytes,
plus saved registers; the next five reported frames are small. The thread
address and the outer captured frame differ by 380,544 bytes. These observations
point to thread-local storage as a contributor; they do not measure the ELF TLS
segment or qualify the test.

Two test-only registries held large inline arrays in thread-local state. On the
local 64-bit component target, the allocator observer's state was 262,240 bytes
and the forecast observer's state was 104,544 bytes. Each state is now 120 bytes.
Their fixed backing is acquired lazily before observation starts and reused.
Unused threads acquire neither backing. These are Rust layout measurements,
not process RSS or measurements of the hosted ELF TLS segment.

The allocation observer still tracks exactly 16,384 live pointers. The forecast
ring still holds 256 records with 256-byte paths. Allocator callbacks, checked
counters, replacement overlap, last Drop, invalid evidence, overflow, unwinding
and forecast capture are unchanged. Complete original modules are archived;
whole-input hashes and inverses bind every byte. Source controls additionally
compare the entire allocator callbacks, observation windows and forecast
callbacks, and retain every original Python test, selected case and GDB frame
probe.

Actual before/current modules produce identical complete allocation Metrics for
the component corpus, including returned owners and nested forecast recording.
All fifteen current observer, free-order and forecast tests pass on Rust 1.98;
all thirteen allocator/free-order tests pass on Rust 1.88. New 128 KiB controls
check lazy initialization, backing reuse, fixed capacities and exclusion of
metadata backing from the measured application window. These component results
do not qualify the complete compiler or the hosted original 512-level case.

The diagnostic script also reads bounded ELF64 program headers and reports the
actual artifact's TLS segment layout before the original debugger command.
Artifact provenance, toolchain, feature unification, optimization, thread size,
test depth and debugger instructions are unchanged. A successful capture job
alone remains diagnostic evidence.
