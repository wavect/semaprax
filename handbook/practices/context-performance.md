# Measure context size and reuse checked work

Two different improvements help a tool-driven workflow. Smaller context reduces
what an agent must read. A semantic cache reuses checked compiler work between
processes. Measure them separately so you know which change helped.

## Choose the smallest useful view

Start with source when the file is already small. Use `query` to locate a
declaration and `context` to inspect its neighborhood. Use the full `graph`
when a tool needs the complete representation.

Compact projections offer `text`, `binary`, and `model-text` encodings for
supported subjects. `model-text` is the model-facing text form; binary output
belongs with a binary-aware consumer.

From the repository root:

```sh
semaprax compact context examples/meaning.spx math.add --max-bytes 4096 --encoding model-text
semaprax compact api-surface examples/calculator-project/semaprax.toml --encoding model-text
```

The first selects one declaration's context. The second selects the project's
API surface. Other command forms cover graphs, agent definitions, task context,
and candidate diffs. Run `semaprax help compact` for their exact inputs.

## Measure a real input

The repository's `scripts/token_report.py` compares exact payloads using a
locally available tokenizer. In a macOS or Linux shell:

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

On Windows, replace the executable expression with your absolute Semaprax path.
Choose a new output filename when repeating the exercise, or explicitly use
`--overwrite` for a report you intend to replace.

The helper uses locally installed `tiktoken` and cached assets for
`cl100k_base` or `o200k_base`. It does not download tokenizer assets during
measurement. With `--allow-bytes-only`, unavailable token counts remain empty
while byte measurements are still reported. Remove that flag when your check
must fail unless token measurement is available.

## Read the comparison correctly

A projection report compares the compact payload with the **same selected JSON
content**. That answers a different question from comparing a narrow context
with a whole repository dump.

| Field or result | How to read it |
| --- | --- |
| Baseline | The explicit reference payload being compared. |
| Actual payload | The selected output produced for this measurement. |
| Positive token delta | Fewer tokens than that baseline. |
| Negative token delta | More tokens than that baseline. |
| Tokenizer fingerprint | The exact tokenizer vocabulary used for the comparison. |
| Source revision | The source snapshot associated with the report. |

Byte counts, local token counts, and provider-reported usage are different
measurements. Planner choices such as `byte-v1` or `lexical-v1` are also distinct
from an exact model-tokenizer measurement. Keep the measurement kind with any
number you share.

## Summarize a recorded session

When your integration has produced an event stream, the `session` subcommand
aggregates it and `show` renders the resulting report. The implementation groups
compatible tokenizer and measurement-boundary data instead of mixing unrelated
counts. Only paired successful measurements contribute to paired reductions.

See the [MCP session recorder](https://github.com/wavect/semaprax/blob/main/scripts/token-report-mcp-session.mjs)
for the repository's recording example. Open the resulting local snapshot in
VS Code with **SEMAPRAX: Show Token Report**. It remains a snapshot until a new
report is recorded.

## Reuse compiler work with a semantic cache

A semantic cache keeps compiler-created checked HIR. **HIR** is the compiler's
resolved representation of the program. Reusing it can avoid repeating some
frontend work when a fresh process opens the same project.

On a supported Unix host, create a new dedicated, owner-only directory. For
example, from the repository root:

```sh
mkdir -p target
mkdir -m 700 target/handbook-cache
CACHE="$(cd target/handbook-cache && pwd -P)"
semaprax semantic-cache-init "$CACHE"
semaprax semantic-cache-persist examples/calculator-project/semaprax.toml "$CACHE"
```

The second directory must be new and empty. Initialization creates a private
store key; keep the store protected and out of source control. The persistence
receipt contains the `entry_digest` you need for a later operation.

Use the following as templates, replacing the digest with that receipt value:

```text
semaprax semantic-cache-warm-open <manifest> <absolute-store-root> <entry-digest>
semaprax semantic-cache-refresh <manifest> <absolute-store-root> <entry-digest>
```

`warm-open` authenticates the selected cache and admits the current project.
`refresh` creates a successor entry after the supported current-source check.
A historical `load` and a current-source `warm-open` are different operations.
If a warm open is rejected, an explicit `semantic-cache-cold-open` is the recovery
route; a failed command does not silently pretend a different cache worked.

A compiler upgrade can invalidate an entry even when the package version string
has not changed. Store compatibility includes the executable identity. Keep the
compiler build, source revision, cache state, and measured task fixed when
comparing cold and warm runs.

**Next:** [Configure the editor report view](../getting-started/editor.md).
References: [token reporting implementation](https://github.com/wavect/semaprax/blob/main/scripts/token_report.py),
[compact projection v2](https://github.com/wavect/semaprax/blob/main/docs/COMPACT-SEMANTIC-PROJECTION-V2.md),
and [Persistent Semantic Cache v1](https://github.com/wavect/semaprax/blob/main/docs/PERSISTENT-SEMANTIC-CACHE-V1.md).
