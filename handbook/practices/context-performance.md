# Measure context size and reuse checked work

Two things make a tool-driven workflow cheaper: a smaller context to read, and
a cache that reuses checked compiler work. After this page you can produce a
compact view, measure it, and warm a semantic cache. Measure the two
separately so you know which change helped.

## Pick the smallest view

1. Source, when the file is small.
2. `query` to find a declaration, then `context` for its neighborhood.
3. `compact` for a model-facing encoding.
4. `graph` only when a tool needs the whole thing.

`compact` has five forms. Each takes `--encoding text|binary|model-text` and
`--replay <encoded>`, which rebuilds from current source and compares with an
encoding you already hold:

| Form | Selects |
| --- | --- |
| `compact context <file> <stable-id> [--max-bytes N]` | One declaration's context. |
| `compact task-context <file> <stable-id> [--goal text] [--seed id]... [--max-tokens N] [--tokenizer byte-v1\|lexical-v1]` | Context ranked for a stated goal and budget. |
| `compact graph <file>` | The whole graph. |
| `compact api-surface <project>` | Public API of an `owned-data-api.v1` project only (`SPX-J105` otherwise). |
| `compact candidate-diff <project> <capsule>` | What a candidate changes. |
| `compact agent-definition <file>` | An agent definition (canonical JSON input). |

`model-text` is the text form meant for models. `binary` belongs with a
binary-aware consumer. From the repository root:

```sh
semaprax compact context examples/meaning.spx math.add --max-bytes 4096 --encoding model-text
semaprax compact api-surface examples/frame-payload-project/semaprax.toml --encoding model-text
```

## Measure a real input

`scripts/token_report.py` compares exact payloads with a local tokenizer. It
needs locally installed `tiktoken` and cached `cl100k_base` or `o200k_base`
assets, and never downloads anything.

```sh
python3 scripts/token_report.py projection \
  --semaprax "$(command -v semaprax)" \
  --input examples/meaning.spx \
  --profile context \
  --selection math.add \
  --encoding model-text \
  --measurement-tokenizer cl100k_base \
  --allow-bytes-only \
  --output target/meaning-token-report.json

python3 scripts/token_report.py show target/meaning-token-report.json --format text
```

`--output` must be a new file unless you pass `--overwrite`. With
`--allow-bytes-only`, missing token counts stay empty while byte counts are
reported. Drop the flag when the check must fail without token counts. On
Windows, give the absolute path to `semaprax`.

Other subcommands: `compare` (two reports) and `session` (aggregate a recorded
event stream; `show` renders it). The
[MCP session recorder](https://github.com/wavect/semaprax/blob/main/scripts/token-report-mcp-session.mjs)
is a recording example. VS Code shows a snapshot with **SEMAPRAX: Show Token
Report**.

## Read a report

A projection report compares the compact payload with the same selected JSON
content. That is not the same as comparing a narrow context with a whole
repository dump.

| Field | Meaning |
| --- | --- |
| Baseline | The explicit reference payload. |
| Actual payload | The output produced for this measurement. |
| Positive token delta | Fewer tokens than the baseline. |
| Negative token delta | More tokens than the baseline. |
| Tokenizer fingerprint | The exact vocabulary used. |
| Source revision | The source snapshot measured. |

Bytes, local token counts and provider-reported usage are different
measurements. Planner choices (`byte-v1`, `lexical-v1`) are not model-tokenizer
counts. Keep the measurement kind with any number you share.

## Reuse compiler work with a semantic cache

A semantic cache stores checked HIR (the compiler's resolved program) so a new
process can skip some frontend work on the same project. It carries no source
authority. On a supported Unix host, from the repository root:

```sh
mkdir -p target
mkdir -m 700 target/handbook-cache
CACHE="$(cd target/handbook-cache && pwd -P)"
semaprax semantic-cache-init "$CACHE"
semaprax semantic-cache-persist examples/calculator-project/semaprax.toml "$CACHE"
```

The store directory must be new, empty and owner-only. Init creates a private
key: keep it protected and out of Git. The persist receipt holds the
`entry_digest` you need next.

```text
semaprax semantic-cache-warm-open <manifest> <absolute-store-root> <entry-digest>
semaprax semantic-cache-refresh <manifest> <absolute-store-root> <entry-digest>
semaprax semantic-cache-cold-open <manifest>
semaprax semantic-cache-load <store-root> <entry-digest>
semaprax semantic-cache-evict <store-root> <entry-digest>
semaprax semantic-cache-lifecycle <manifest> <empty-store-root>
```

| Command | Does |
| --- | --- |
| `warm-open` | Authenticates the entry and admits the current project. |
| `refresh` | Writes a successor entry after a current-source check. |
| `cold-open` | Opens without a cache: the recovery route after a rejected warm open. It reports which sources it invalidated. |
| `load` | Reads a historical entry. Not the same as `warm-open`. |
| `evict` | Removes an entry. |
| `lifecycle` | One receipt over the cold, restored, refreshed, evicted and cold-rebuilt stages, with retained byte counts. Needs an empty private store. |

A compiler upgrade can invalidate an entry even when the version string is
unchanged: compatibility includes the executable identity. For a fair cold and
warm comparison, fix the compiler build, source revision, cache state and task.
`retention-metadata-*` commands manage retention checkpoints for stores; see
[Semantic Retention Metadata CLI v1](https://github.com/wavect/semaprax/blob/main/docs/SEMANTIC-RETENTION-METADATA-CLI-V1.md).

## Benchmark context size

`semaprax context-benchmark <benchmark-manifest>` measures agent-context sizes
from a tab-separated manifest that must start with
`schema<TAB>semaprax.agent-context-benchmark.v1` (`SPX-G005` otherwise). See
[Agent Context v1](https://github.com/wavect/semaprax/blob/main/benchmarks/agent-context-v1) for fixtures.

**Next:** [Configure the editor report view](../getting-started/editor.md).
References: [token reporting](https://github.com/wavect/semaprax/blob/main/scripts/token_report.py),
[Compact Projection v2](https://github.com/wavect/semaprax/blob/main/docs/COMPACT-SEMANTIC-PROJECTION-V2.md),
[Persistent Semantic Cache v1](https://github.com/wavect/semaprax/blob/main/docs/PERSISTENT-SEMANTIC-CACHE-V1.md).
