# ShiftSim typed-record successor source

This isolated source draft carries scheduler values as named `Patient`,
`Server`, `Assignment`, and `Metrics` records. `Patient` declaration order
encodes the existing dispatch key `(priority, arrival, id)`; `Server` order
encodes server-ID ordering. The helpers in `src/order.spx` use the admitted
stable owned-record sort, so ordering is derived from declared fields rather
than a hand-written comparator over parallel vectors.

This is an application-source successor draft, not a benchmark candidate or a
qualified result. It does not change the retained SEM03 candidate, its 15-case
SPEC, corpus, oracle, accepted output, or qualification records. Input decoding
and the full event loop are not yet bound: the nested-request codec and v32
owned nested-outcome route must be available before this source can become the
stdin application. Keep all original acceptance behavior and compare exact
outputs when the owning current-head gate is run.

The model follows the v31 private collection-record carrier boundary. It makes
no claim that this example has passed compiler, backend, or benchmark
qualification.
