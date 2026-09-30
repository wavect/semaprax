# Matching

`match` destructures scalars and variants with checked exhaustiveness. Arms
are ordered, first match wins, and every form ends with a catch-all.

## Scalar matching

```semaprax
module app.sign;

@id("refutable.sign_class")
fn sign_class(value: i64) -> i64
{
    match value { 0 => 0, -1 | -2 => -9, n if n < 0 => -1, n => 1, }
}

@id("app.main")
fn main() -> i64
{
    sign_class(-2)
}
```

- **Literals**: `0 => …`. **Alternatives**: `-1 | -2 => …`.
- **Guards**: `n if n < 0 => …` — the binding is usable in the guard and arm.
- **Catch-all**: a final `_` or bare binding (`n`) **without** a guard.
  Missing it is `SPX-T257`.
- **Order matters**: `-1` hits the alternative arm before the later guard arm
  could claim it. Put specific arms first, general ones last.

## Variant matching

Bind payloads by field name. Construction spells type arguments; matching
doesn't:

```semaprax
module app.pick;

@id("data.first_positive")
fn first_positive(left: i64, right: i64) -> Option<i64>
{
    if left > 0 { Option<i64>::Some { value: left } } else { if right > 0 { Option<i64>::Some { value: right } } else { Option<i64>::None {} } }
}

@id("app.main")
fn main() -> i64
{
    match first_positive(0, 4) { Option::Some { value: v } => v, Option::None {} => 0, }
}
```

- Payload-less cases match as `Shape::Dot {}` — never bare `Dot`.
- The compiler checks that all cases are covered; for open scalar matches
  the catch-all provides that proof.
- Arms yield scalars and calls, not nominal aggregates: an arm that builds a
  record/variant is `SPX-T258`. Bind scalars out of the match, then construct
  after — or use `if` for the construction.

## match own: stepping owned values

`match own` moves the scrutinee into the arms. Its main use is driving
`IterStep` from `iter_next` (see [Loops](loops.md)):

```semaprax
match own step { IterStep::Done {} => false, IterStep::Yield { item, rest } => keep(item), }
```

`Yield` binds a Copy `item` plus the owning `rest` — pass `rest` onward or
let scope cleanup settle it.

## Best practices

1. **Match at the boundary.** Destructure `Option`/`Result` where the value
   arrives; work with plain scalars inside.
2. **Let exhaustiveness review your logic.** Adding a variant case turns every
   match over it into a compile error listing exactly what to update.
3. **Keep arms flat.** A nested match inside an arm usually wants to be a
   helper function with its own `@id` and contract.

Exact rules: [Refutable Match v1](https://github.com/wavect/semaprax/blob/main/docs/REFUTABLE-MATCH-V1.md),
[RFC 0002](https://github.com/wavect/semaprax/blob/main/docs/RFC-0002-ALGEBRAIC-DATA.md).
