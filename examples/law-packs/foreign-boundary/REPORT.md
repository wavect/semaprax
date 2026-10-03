# Reviewed result and evidence boundary

The owning physical gate passed 1/1 on the v080 base `69ac8f29a` with this
example (zero failed or ignored). It derives reports from the authenticated Project and
published native package. [`report-expectations.json`](report-expectations.json)
is a checked projection of that derived conditional strict report, not a
certificate or an independently authoritative saved report.

| Report field | Expected value |
| --- | --- |
| `accepted` | `true`, under the explicitly selected conditional host policy |
| `foreign_internals_proved` | `false` |
| `runtime_call_observed` | `false` |
| `accepted_conditions` | `law09.add.behavior:no_effects`, `:no_callbacks`, `:no_panics`, `:no_shared_state` |

The separate caller certificate records `source_route_proved: true`: the
checked Semaprax body directly forwards to the selected guarded import. This
bounded source fact does not prove the Rust body. The native consumers provide
separate concrete execution evidence for 42, rejected 1042, repaired 42, and
accepted 0. The static report deliberately continues to say that it did not
observe a runtime call.

The frontier status is `conditional_on_foreign_assumptions`. Ordinary core
strict-law reports remain open; the builder's conditional route replays each
accepted condition and the exact caller, adapter, summary and package identities.
A forged report or omitted condition refuses. A policy mismatch leaves no
published output. Changing the guard source invalidates package replay.

## Versioning and invalidation

`assumptions.json` version 1 and its exact bytes participate in the declared
proposition digest. The owning test changes its version to 2 and refuses use of
the old bound package. This pack also binds exact Project source, selected index,
package source/version, target, feature selection, compiler and generated adapter
identities through the existing frontier and package routes.

Each changed Rust fixture needs a new index, package and caller binding. The
harness tries the original bundle against the changed caller and requires
refusal, then replays fresh evidence. It checks that repair did not modify the
law, assumptions or saved Semaprax source. Changes to implementation, assumptions
or version must be reviewed and regenerated; copying an old success report
cannot authorize them.

These are local native observations and bounded conditional evidence. They do
not establish arbitrary-input addition correctness, absence of foreign effects,
backend equivalence, or a general Rust theorem.
