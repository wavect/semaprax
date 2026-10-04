# Bend 2 law benchmark (v1)

This is the reproducible benchmark harness for issue #392. It records a
comparison only when a local command file pins both checked-out source trees,
their exact commits, tools, and commands. The committed manifest pins Bend 2
at `947db722640c86247849343657bf2f7ef01cb7f1` and forces
`BEND_NO_TELEMETRY=1` for every execution.

Run an admitted 30-repetition trial on a quiet host with a local command file:

```sh
python3 benchmarks/bend2-law-v1/run.py \
  --commands /secure/local/bend2-commands.json \
  --output /secure/local/bend2-result.json
```

The command file follows [`commands.example.json`](commands.example.json). It
is deliberately not committed because executable paths, the exact SEMAPRAX
commit, hardware/OS, toolchain and dependency identities, backend, flags, and
fixture digests are host inputs. The runner refuses a missing or drifted Git
identity, records every raw warm sample plus output digests, labels smaller
runs as pilots, and never emits a winner field.

For each of the six cells, it separately executes `bend_normal`,
`bend_verdict`, `semaprax_smt`, `semaprax_lean`, and `semaprax_runtime`.
Every path must reject the cell's law-gaming attack before its timing is
accepted. Thus an empty sort cannot pass a sorting cell and a no-op transfer
cannot pass a balance cell. Numeric domains are declared per cell; a command
file must not substitute Bend `Nat` semantics for the declared checked `u32`
domain.

The scalar-contract cell is a matched microcell: it uses total Boolean
negation, where both languages have the same two-value domain. It remains
unexecuted until a pinned Bend toolchain and local fixture drivers are supplied.
Every command receives its canonical fixture path through `{fixture}` and the
requested `success` or attack case through `{case}`. This prevents a local
driver from timing an unbound substitute. The other five cells retain their
checked-`u32` domain and remain unavailable until SEMAPRAX has an equal `u32`
surface; they cannot be replaced with an `i32` benchmark.

`fixtures/` is the committed language-neutral source-input corpus: every cell
has one accepted witness and one rejected attack witness, and their digests
are bound into the result. The current SEMAPRAX scalar surface has no `u32`
type (it admits `i32`, `i64`, and `u8`), while Bend uses `U32`; an `i32`
substitute would narrow the domain and is therefore not presented as an equal
source fixture. No pinned Bend executable is installed on this host, so the
corresponding Bend source/proof files and a runnable local command
configuration remain explicitly unavailable.

The harness records local evidence only. It does not provision tools, clone
repositories, generate source fixtures, publish results, or make a
superiority claim. Unimplemented fixture/tool combinations remain
`unavailable` in the result rather than a favorable score.

The deterministic no-tool input receipt is useful for review before a local
toolchain run:

```sh
python3 benchmarks/bend2-law-v1/fixture_receipt.py \
  --output /tmp/bend2-fixture-receipt.json
```
