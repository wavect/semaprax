# Checked Literal Format v1

Audience: coding agents, compiler contributors and reviewers.

Status: partial. Focused local checks passed. OPT-719 still requires the
ordinary-HIR helper and a passing full quality profile.

`string_format(template, values...) -> string` is a compiler-owned operation.
`template` must be an actual source string literal. Its decoded UTF-8 is at most
65,536 bytes and contains at most 32 sequential `{}` fields. `{{` emits `{`;
`}}` emits `}`. Other braces are errors. The number of fields must equal the
number of dynamic arguments. The permitted argument types are `i64`, `u8`,
`usize`, `bool`, and owned `string`; the result is owned `string`. The name and
stable identity `core.string.format.literal.v1` are reserved.

For example, `string_format("{}: {{}}", 7)` produces `7: {}`. The source
formatter preserves the ordinary call spelling. The graph projects the
dedicated `literal_format` node with its decoded raw `template` and dynamic
`args`, and the HIR cache uses additive expression tag 33. The retained HIR
keeps dynamic child identities at their original source argument indexes
(starting at one because the template is source argument zero). It stores no
parsed recipe; validation reparses the template, checks the children
and fixed result, and cached-module restoration independently resolves the
synthetic source before Project replay binds it to canonical authored source.

Dynamic arguments evaluate once from left to right. Every scalar is captured
before the next argument runs. Each owned String enters its canonical
CallArgument epoch; one complete CallCommit transfers the group before the
first rendering allocation. A direct owned String Place argument transfers its
existing generation rather than initializing the ordinary String-read clone.
Cleanup construction, independent replay, interpreter, native C11 and Wasm
all derive this exception from the dedicated source-bound operation; ordinary
String reads retain their prior cloning behavior. If a later dynamic argument
fails, the already staged prefix settles through the ordinary pre-commit exit.
A private worker constructs the empty accumulator,
then each literal or converted field, then joins each piece in source order.
If a helper fails, it drops the accumulator, current piece and every remaining
owned argument before the ordinary failure cleanup. The selected failure is
sticky and no partial result is published. The worker creates no source
function, capability, provider, or public String ABI.

The aggregate Wasm host adds one format-only private import,
`spx_format_step_v1`. It returns zero for checked String allocation refusal;
the generated worker then releases its owned inputs and reports
`semaprax.string-format.v1/1`. Existing byte and String imports retain their
prior behavior. The selected standalone String arena uses its existing
fallible allocation imports and the same worker recipe. The authenticated
selected closure binds its format-status handling once: checked worker refusal
is internal status 34 and requires a recorded arena capacity cause after all
owners settle; conversion status 21 retains its existing separate meaning.
Non-format profiles do not admit status 34. Its default owner
census additionally charges three worker handles per active function containing
formatting, before the existing call-path bound. Workers in the same function
cannot overlap: dynamic children finish before rendering, which calls no
source functions. Modules without formatting retain their prior owner bound.
Each emitted worker failure guard also charges its three scratch drops and
all staged String argument drops to the existing 262,144 cleanup-action bound.
The browser host recognizes that exact private import in the authenticated
module and admits up to 64 live owned byte entries for this selected shape, so
all 32 owned fields and the worker's scratch owners fit. Earlier modules keep
their 16-entry host bound. An explicit smaller host limit remains a checked
resource policy and can trigger `semaprax.string-format.v1/1` after commit.

The first admission is monomorphic ordinary functions, including blocks,
branches, loops and contracts where existing String profiles allow the
operation. Generic templates and closure bodies are refused. Runtime
templates, named or positional fields, width/precision, floats, locale,
traits, JSON escaping and implicit coercion are outside this version.

The focused gate is `cargo test --locked -p semaprax --test language
checked_literal_format -- --nocapture`, followed by the full quality profile.
It covers canonical and graph round trips, refusals, left-to-right staging,
native allocation failure, repeated standalone Core-Wasm settlement, and
aggregate Wasm host allocation refusal and reentry. The `literal_format`
unit selectors additionally cover raw-template cache tag 33, independent source
and synthetic-AST replay, malformed ownership/commit proofs, and private worker
cleanup census.

## Local qualification

At source revision `b2269e0d3f919ebf9638cb7fbdd372e8b8683880`, all eight
focused language tests and all nineteen `literal_format` library tests
passed. The same emitted C11 probe produced identical O0 and O2 observations
for success, pre-commit failure, and worker allocation failure after commit.
The standalone Core Wasm and aggregate Wasm tests exercised allocation refusal
and reentry. `scripts/quality.sh full` stopped at `fmt-check` on existing
formatting differences in `src/source_reflows/ci_inputs/successor_fronts.rs`.

OPT-719 calls for an independently authenticated private monomorphic helper
expressed using ordinary typed HIR. The dedicated HIR node and backend worker
described above still need that replacement and the corresponding
source-reconstruction and precharged-expansion checks.

### Application comparison

Each pair used the same local macOS arm64 `semaprax-full` compiler. Its
SHA-256 was `92066a187bdca4beeecc142ea7ab99124c85676c480904b186d981eabda0fcee`.
The concatenation arm restored just the earlier renderer expressions in a
temporary copy of each project. LogLens used the `loglens.report` body from
`403c261e2^`. ShiftSim used the command and declared-test expressions from
`1667a4538^`, and both arms were canonicalized and installed through
`semaprax-full new` with the same codec plan and collection-record command
profile.

| Application | Exact result across both arms | Concatenation binary SHA-256 | Checked-format binary SHA-256 |
| --- | --- | --- | --- |
| LogLens selected-file | Both declared native tests passed in each arm. The [unchanged driver](../experiments/selected-file-input/loglens/tests.mjs) observed identical exit status, stdout, and stderr for all 207 invocations. | `5a2a243599bcc9fd356be9a2118c7b61844c7a96232d918f2d734ee09a204976` | `30bb2619a0e138af6ffec58e588664b02f43f50022476ab7c0801c4145b9d248` |
| ShiftSim typed record | All three declared typed native tests passed in each arm. The [original 15 cases](../benchmarks/event-sim-tokens-v1/acceptance/corpus.json) produced identical exit status, stdout, and stderr, including the escaped-input and leading-whitespace cases. | `3a821baf726a6a8ab414a4e119437d68a13becb1ed9b66a3daebd174fbb70419` | `981d7bf04246be721a67bd81882401739cd062e28c90c7fb0409ea5574477423` |

The changed LogLens source file measured 10,771 bytes with concatenation and
9,207 bytes with nine checked-format calls. The two changed ShiftSim files
measured 6,858 and 6,829 bytes respectively, with two checked-format calls
in the latter.
