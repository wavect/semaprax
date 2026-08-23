# Freestanding Object Profile v1

`semaprax freestanding-object <file.spx>` is a deterministic, read-only
projection that derives one complete freestanding C11 translation unit from a
verified effect-free scalar module — the same admission profile as C Header
Emission v1 and Canonical ABI Report v1, applied to the whole module — and
records explicit profile assertions over it. It is the first executable slice
of the completion-matrix row "Embedded and real-time". The translation-unit
bytes start from the production native C11 projection (`codegen::emit_c`) with
the documented host-process scaffolding excluded, so every remaining byte of
program logic is exactly what the native backend emits.

## Command

```sh
semaprax freestanding-object <file> [--max-bytes N]
```

- The whole module must sit inside the freestanding scalar admission profile;
  there are no per-function selections. Any capability permit, interface or
  import, external module use, type declaration, or function outside the
  closed scalar gate (explicit identity, monomorphic, no effects, by-value
  direct `i64`/`bool` parameters, direct `i64`/`bool` result) fails closed
  with `SPX-A102`.
- `--max-bytes` (default 512 KiB, bounds follow the Agent Context byte limits;
  the larger default reflects the embedded full translation unit) bounds the
  whole envelope. Overflow fails closed with `SPX-A103`; output is never
  truncated or repaired.
- The command prints one canonical compact JSON envelope plus one trailing
  newline. It invokes no compiler.

## Host-scaffolding exclusions and substitutions

The freestanding unit differs from the hosted native projection by exactly
four recorded exclusions and two recorded substitutions; every edit is
anchored on exact unique markers in the produced bytes, so drift in the
production lane fails closed (`SPX-A104`) instead of emitting stale artifacts.

Exclusions:

- `entry_wrapper` — the `#ifndef SPX_NO_ENTRY_WRAPPER int main(void) …`
  hosted process wrapper with its `printf` result printing.
- `stdio_include` — the `<stdio.h>` include used only by hosted reporting.
- `stdlib_include` — the `<stdlib.h>` include used only to declare `abort`.
- `public_failure_reporter` — `spx_public_failure`, which maps statuses to
  hosted process exit codes and is referenced only by the removed wrapper.

Substitutions:

- `invariant_failstop` — the hosted stderr/abort invariant reporter is
  replaced by a same-signature closed failstop loop. Failure-path behavior is
  intentionally different from the hosted lane; no execution equivalence is
  claimed for this tranche.
- `external_function_linkage` — each admitted module function's prototype and
  definition drop their leading `static` so the relocatable object actually
  exports callable symbols (the hosted wrapper was the only referencer of the
  internal functions). Runtime helpers remain internal.

## Profile assertions

Four assertions are computed by explicit deterministic textual checks over the
emitted bytes and re-checked during independent replay; generation fails
closed rather than recording a false assertion:

- `no_runtime` — no host entry wrapper, no stdio/stdlib includes, no printf/
  fprintf/fputs/stderr/abort references, no public-failure reporter; the
  failstop substitution is present.
- `no_allocation` — no malloc/calloc/realloc/aligned_alloc/alloca/free call
  text; status arenas remain caller-provided storage.
- `no_blocking` — no sleep/nanosleep/pthread/thrd/waitpid/sched_yield or
  other blocking-primitive call text.
- `no_libc_dependency` — modulo the declared exceptions below, no libc
  headers or hosted-library references remain.

The envelope declares the exact allowed undefined-symbol surface an
`-ffreestanding -nostdlib` link may still require: `memcpy` (a
compiler-emitted memory primitive that ISO C freestanding environments are
expected to provide) and `strcmp` (production status-runtime schema/domain
validation kept verbatim from the native lane). Executable evidence compiles
the real emitted bytes and checks the real symbol table against this set.

## Envelope and verification

`freestanding_object::generate` returns canonical compact JSON with fixed key
order:

- outer wrapper `{"schema","digest","bytes","payload"}` where `digest` is the
  domain-separated SHA-256 of the exact payload bytes
  (`semaprax.freestanding.payload.v1`) and `bytes` is their length;
- payload members in order: `schema`, `source` (`path`, `revision`,
  domain-separated source digest), `limits`, `module` (`functions_total`,
  `admitted`, `entry_point`), `functions` (sorted bytewise by stable identity,
  each with `stable_id`, `name`, `symbol`),
  `profile_assertions`, `allowed_undefined_symbols` (with justifications),
  `scaffolding_exclusions`, `scaffolding_substitutions`, `object_recipe` (the
  documented verification flags and `command_compiles_nothing: true`),
  embedded `translation_unit_sha256`
  (`semaprax.freestanding.translation-unit.v1`), embedded `translation_unit`
  text, and fixed `nonclaims`.

`freestanding_object::verify_envelope` independently recomputes the outer
payload digest over the exact serialized payload bytes, re-checks the declared
byte count, re-authenticates the embedded translation-unit digest, and replays
every profile assertion against the embedded text before returning it.
Mutations that keep all digests consistent (forged-but-re-signed payloads) are
still rejected whenever they touch anything an assertion observes.

Source bytes are snapshotted before parsing and re-checked after rendering;
drift fails the whole command closed. All diagnostics use the previously
unused `SPX-A1xx` family: `SPX-A101` options, `SPX-A102` module admission,
`SPX-A103` budget exhaustion, `SPX-A104` envelope/native consistency.

## Nonclaims

The tranche claims no MMIO/volatile/atomics support, no linker-script
control, no hardware or emulator execution, no interrupt or RTOS model, and
no board targets. The artifact is a relocatable object for one effect-free
scalar profile only; this command performs no toolchain invocation, executes
nothing, and changes no source. No completion-row status beyond "Partial" for
this bounded slice is claimed.

## Evidence

Executable evidence lives in `tests/freestanding_object_v1.rs` plus module
tests in `src/freestanding_object.rs`: pinned golden envelope and
path-independent translation-unit digests over `examples/meaning.spx`,
determinism double-runs in-process and through the CLI, per-digest-field
tamper rejection including forged-but-re-signed payloads caught by assertion
replay, budget exhaustion, every admission rejection reason exercised against
real programs, CLI exit codes, the documented host-delta proof (zero
host/main references plus verbatim-line accounting), and — unlike the sibling
projection tranches — an actual toolchain gate: the emitted translation unit
is compiled with `cc`/`clang -std=c11 -O0 -ffreestanding -nostdlib
-fno-stack-protector -D_FORTIFY_SOURCE=0 -c` into a relocatable object in a
temporary directory, twice (byte-identical objects), and `nm` must show no
undefined symbols beyond the declared allowed set while every module symbol is
externally defined. Compiler discovery follows the existing native lanes
(`CC`, then `CLANG`, then `cc`/`clang` probes); when no compiler or `nm`
exists the toolchain gate skips with an explicit message. No target is ever
executed.

See also [C-HEADER-V1.md](C-HEADER-V1.md) and [ABI-REPORT-V1.md](ABI-REPORT-V1.md)
for the sibling read-only scalar tranches sharing this admission profile.
