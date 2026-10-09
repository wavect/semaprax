# Checked Literal Format v1

Status: source implementation; current-head executable gate pending.

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
stores no parsed recipe; validation reparses the template, checks the children
and fixed result, and cached-module restoration independently resolves the
synthetic source before Project replay binds it to canonical authored source.

Dynamic arguments evaluate once from left to right. Every scalar is captured
before the next argument runs. Each owned String enters its canonical
CallArgument epoch; one complete CallCommit transfers the group before the
first rendering allocation. A private worker constructs the empty accumulator,
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
fallible allocation imports and the same worker recipe. Its default owner
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
cleanup census. A current-head
pass is required before this row can be marked implemented.
