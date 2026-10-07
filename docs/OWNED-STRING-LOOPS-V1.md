# Owned String Loops v1

Audience: language users, agent authors, and compiler contributors.

Status: Partial — implemented for the reference interpreter, generated C11,
and the String-settling Core Wasm profile (`--profile internal-strings-v1`).
It widens [Bounded While-Loops v1](WHILE-LOOPS-V1.md) and
[Explicit Mutation v1](EXPLICIT-MUTATION-V1.md); every shape outside this
page keeps its earlier diagnostic. The additive
[Owned String Loops v2](OWNED-STRING-LOOPS-V2.md) admits user String calls
and Copy-scrutinee matches in loop bodies. [Whole String Replacement v1](STRING-REPLACEMENT-V1.md) separately admits general whole String assignment with a guarded old-owner release.

## Objective

Before this profile a loop could not accumulate text: a `string` binding could
not be reassigned (`SPX-U105`), and string literals and String-producing calls
were refused in `while` bodies (`SPX-T252`). Recursion is bounded at 256
frames, so it was no substitute. v1 admits the idiom ordinary programs need:

```text
let mut out = string_from_i64(0);
let mut i = 1;
while i < 1000 {
    out = string_concat(out, ",");
    out = string_concat(out, string_from_i64(i));
    i = i + 1;
    0
}
```

## Admitted shapes

1. **Same-owner append.** `text = string_concat(text, more);` where `text` is
   a whole `let mut` binding of type `string`, the assignment target and the
   first operand are the same binding, and `more` is any admitted `string`
   expression. It is admitted in straight-line code and in loop bodies,
   including inside `if` branches. This is the third same-owner reopen shape,
   after `values = vec_push<T>(values, v)` and
   `buffer = bytes_set(buffer, i, v)`.
2. **Strings in loop bodies.** String literals and every compiler-owned
   `string_*` operation are admitted in `while` bodies and in `for item in
   values` bodies, together with `let` bindings of `string` type declared in
   the body. Each iteration's body block is its own cleanup region: values
   created and not consumed in an iteration are released when that iteration
   ends.

User functions called in a loop body keep the existing scalar-signature rule
(`SPX-T252`, "only scalar functions qualify"); call a String helper outside
the loop, or build the text with the compiler-owned operations.

## Ownership and cleanup semantics

`string_concat` consumes both operands. Every other owning read of a `string`
place allocates a clone, but the first operand of a same-owner append moves
the binding's current generation into the call: the canonical CleanupPlan
transfers the binding's `value` storage into the call's first argument epoch
and never initializes a clone temporary there. Arguments stage left to right
and transfer together at the call's commit boundary, exactly as for any owned
call. After a successful call the assignment publishes the result into the
binding slot, which is dead at that point, so no value is released at the
assignment and no second owner is ever live.

If anything fails after the operand was staged (a later argument, a checked
operation, a contract), the selected failure is sticky and cleanup releases
the staged old generation from its call epoch; the binding is not revived.

The published generation keeps the cleanup position its previous generation
held when it was staged. The builder records that position when it stages the
operand and restores it on publication; the independent replay re-derives the
same append set from typed HIR and checks the same order. Loop iterations and
branch joins therefore see one stable initialization history, and every exit
finalizes its live leaves in that canonical order. No CleanupPlan schema,
transition kind, or graph version changes: the plan carries ordinary
`transfer`, `call_commit`, and `initialize` facts.

## Loop conditions

The additive [Named String Length Conditions v1](STRING-LENGTH-CONDITIONS-V1.md)
admits `string_len(text)` where `text` is an available whole named String
binding. It inspects the current owner without allocating a clone, including
after a same-owner append in the body. Allocating expressions such as
`string_len("literal")` and String operations outside the named borrowed-read
profiles retain `SPX-T252`. The additive
[Borrowed String Predicate Conditions v1](BORROWED-STRING-PREDICATE-CONDITIONS-V1.md)
also admits exact named-owner calls to `string_is_empty`,
`string_starts_with`, and `string_contains`:

```text
string values are not admitted in while conditions; compute a scalar such as `string_len(text)` in the loop body and test that
```

Keep a scalar loop variable instead:

```text
let mut size = string_len(log);
while size < 1000 {
    log = string_concat(log, "a");
    size = string_len(log);
    0
}
```

## Frozen v1 boundaries

The first two shapes below are admitted by additive [Whole String Replacement v1](STRING-REPLACEMENT-V1.md), with its separate cleanup and Wasm profile selection.

| Shape | Diagnostic |
| --- | --- |
| Whole replacement `text = "other"` | `SPX-U105` |
| Owner not the first operand, `text = string_concat("p", text)` | `SPX-U105` |
| An allocating String expression or a String operation outside the named length and borrowed-predicate profiles in a `while` condition | `SPX-T252` |
| Consuming an outer `string` binding inside a loop body (`let t = outer;`, `string_concat(outer, …)`) | `SPX-T252` (ownership changes inside a loop); malformed HIR still fails independently with `SPX-H006` |
| `yield` in a function whose loop carries a `string` | `SPX-T303` |

Whole replacement needs a release at the assignment, which v1 does not lower.
The source verifier checks each `while` body as an ordinary block and refuses
ownership drift before HIR resolution. HIR validation independently requires
body-exit liveness to equal loop-entry liveness.

## Backends

| Backend | Behavior |
| --- | --- |
| Reference interpreter (`run`) | Executes; Rust ownership releases each value. |
| Native C11 (`run --native`, `build --target native`) | The append applies the plan's transfer into the binding slot; the operand read moves the carrier without `spx_string_clone`. Every String temporary of a body statement, including one with no String binding (`total = total + string_len("ab");`), settles at the end of its iteration. |
| Core Wasm, `--profile internal-strings-v1` | The operand moves its carrier into the call epoch; the trusted runtime settles its arena after every call, so a leaked or twice-released owner poisons the instance. Numeric text (`string_from_i64`) stays outside this profile (`SPX-W116`), as before. |
| Legacy scalar Web/Wasm packages | Unchanged; they still refuse String programs (`SPX-W116`). |

The ordinary scalar interpreter profile refuses user functions with `string`
signatures (`SPX-F102`); `run` can select the internal String profile for
these programs. Numeric text stays outside the String-settling Wasm profile.

## Evidence

`tests/language/owned_string_loops_v1.rs` round-trips the corpus through the
canonical formatter and graph, asserts the moving `transfer` fact, executes
the same cases on the interpreter, on C11 at `-O0` and `-O2` under an
allocation-counting allocator that rejects duplicate and foreign frees and
requires zero live allocations after every case (including arithmetic and
contract failures inside the loop), and on the String-settling Wasm profile
in Node with repeated calls. It also pins the still-refused shapes above.
