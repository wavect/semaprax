# Whole String Replacement v1

Audience: language users and compiler contributors.

Status: authored additive profile; executable verification pending. This page
owns whole mutable String replacement, CleanupPlan v16 and Graph v68. It does
not promote Text Toolkit operations to Core Wasm or widen public String ABIs.

## Source and ownership

A whole available `let mut` binding of type `string` can receive any ordinarily
admitted owning String expression. Literals, named owners, user calls, blocks,
`if` and exhaustive `match` results are supported, including in loop bodies:

```semaprax
module app.replacement;
@id("app.main")
fn main() -> i64 {
    let mut text = "initial";
    let mut index = 0;
    while index < 3 {
        text = if index == 1 { string_concat(text, "!") } else { "new" };
        index = index + 1;
        0
    }
    string_len(text)
}
```

The target must be available before evaluation starts. The assignment does not
reinitialize an already moved owner. The RHS follows normal ownership rules:
a named owning result is consumed, an owning call stages its arguments left to
right and commits them together, and an immutable view preserves its owner.
Only the assignment target becomes available after successful publication.
Unrelated RHS consumption remains consumed, including conditional consumption.
A loop still requires its surrounding owner availability to agree at the
backedge. Immutable bindings, wrong types, field replacement and other owned
collection replacement retain their existing refusals. A live overlapping
shared loan prevents replacement (`SPX-T265`); replacement grants no lifetime
or capability exception. Bounds and effects of calls are unchanged.

Exact same-owner append (`text = string_concat(text, more)`) keeps its existing
[Owned String Loops v1](OWNED-STRING-LOOPS-V1.md) plan and graph selection.
This additive profile covers every other whole String replacement shape.

## Declared successful boundary

CleanupPlan v16 uses existing `reserve_renewal` and `renew` carriers with a new,
explicitly selected String contract. Before the RHS begins, the reservation
records the target's live leaf and canonical initialization history. Evaluation
may leave that old leaf live or consume it through an authenticated owning
call. Failure before publication follows the existing canonical failure exit;
cleanup preserves the selected status and publishes no new value.

After a complete owning RHS exists, `renew` performs a guarded finalization of
the old target leaf, then transfers the new owner to that target. A dead old
leaf is not physically finalized again. The completed RHS and target must be
distinct storage. The target resumes its reserved position among surviving
prior owners. Any new RHS temporary owners remain in their existing completion
order after those prior survivors and are settled by their lexical regions.
Consumed unrelated owners remain absent. This is the declared replacement
boundary; no backend sorts, repairs or infers a plan vector.

The builder and independent path replay derive the exact mutable binding and
assignment site from typed HIR, authenticate the reservation and source/destination
leaf, and independently reproduce the history boundary. Missing reservations,
ordinary transfers at replacement sites, source/destination aliases, changed
mutability or ownership, stale schemas and forged histories fail closed.
Existing iterator and ordinary Vec renewal keep their frozen contracts; they
are not interpreted as String replacement. LoanPlan v1 and its limits remain
unchanged. No runtime finalizer is authorized by an unauthenticated plan.

## Projection and backends

A program requiring this profile selects `semaprax.graph.v68`, composes all
preceding supported facts and adds `string_replacement` with schema
`semaprax.string-replacement.v1`. Its `updates` vector contains the exact
function, RHS expression identity and target binding identity in deterministic
function/site order. Programs without replacement preserve prior schemas.
Parser, formatter, type identities and prelude versions are unchanged.

The interpreter and generated C11 execute ordinarily admitted replacement
expressions with existing String behavior, checked failures and cleanup. The
standalone Wasm library entry
`wasm::internal_strings::emit_string_replacement_module` explicitly selects the
private `string-replacement-v1` profile, existing authenticated Copy-variant
match support, scalar exports and the same quota/error protocol. Frozen
`emit_module` and `emit_copy_variant_module` reject a replacement closure with
`SPX-W111` and retain their old bytes for previously admitted programs.
Text Toolkit operations, including `string_slice`, remain `SPX-W116` in this
Wasm profile. No CLI profile selector is implied by the library entry.

## Owning executable gate

`tests/language/string_replacement.rs` owns canonical round-trip/Graph facts,
source and hostile-HIR refusals, interpreter outcomes, repeated C11 O0/O2
allocation settlement, repeated admitted Wasm execution and frozen-profile/
Text Toolkit refusals. It covers consuming helper calls, mixed branches,
scalar matches, nested replacement, zero and repeated loop iterations,
arithmetic/contract/text failures and embedded-NUL UTF-8 values.
`cleanup_plan::replay::string_replacement::tests` independently exercises
builder/oracle parity and replay rejection of altered transition metadata.
These are authored regressions until the consolidated verification pass runs.
