# String Condition Lifetimes v1

Audience: language users and compiler contributors.

Status: authored; consolidated OPT verification is pending.

A Boolean `while` condition can create temporary Strings through literals,
String operations, helper calls, nested blocks, lazy Boolean operands and
admitted matches. Ordinary loop shape, effect, type and ownership rules still
apply. For example:

```semaprax
module example.condition;
fn main() -> i64 {
    let mut i = 0;
    while i < 3 && string_len(string_concat("n", string_from_i64(i))) > 0 {
        i = i + 1;
        0
    }
    i
}
```

Each physical evaluation releases condition-local owners before deciding to
enter the body or leave the loop. Lazy operands execute only when needed.
Failures settle live condition temporaries and enclosing owners through the
ordinary sticky-status failure route. No failed condition publishes a result.
Both source verifiers and independent HIR validation reject changes to the
availability or owned places of enclosing bindings before body evaluation;
consuming an enclosing String in the condition reports `SPX-T252` with the
binding name. Mutable scalar counters may change normally.

The builder derives a condition child region when core HIR creates String
storage beyond authenticated allocation-free named reads. Its normal
`ScopeExit` precedes both `BooleanResult` edges and must restore the entry
cleanup state. No new schema carrier is needed. Independent replay walks the
condition's ordinary effects and authenticates finalizer order, liveness and
Boolean observations; it does not trust the builder's output. Removing a
condition finalizer is a hostile regression. Named length/predicate reads
retain their existing allocation-free path and prior plan shape.

Native C and aggregate Wasm bind the exact canonical normal scope exit to the
condition Boolean decision, using the existing scoped Boolean cleanup hooks.
The interpreter drops evaluated temporaries when condition evaluation
completes. The separately selected Wasm text profile owns executable support;
frozen Wasm selectors retain their documented refusals.

Authored source/graph/round-trip, interpreter, allocation-counting native O0/O2,
failure and hostile replay cases live in `language::string_conditions`; the
older named-read and ownership-drift corpora remain regression obligations.
