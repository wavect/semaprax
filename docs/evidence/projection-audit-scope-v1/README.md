# Complete projection source-audit scopes

Status: local source-contract verification; hosted owning gate pending.

Audience: compiler maintainers and reviewers.

The private-layout source inventory joined 90 ordered inputs, including external
HIR/replay implementations, historical receivers and its own inventory source.
The projection purity assertion applied its wildcard-import and authority rules
to that broader union. Unrelated modules contain wildcard imports, so the
unchanged gate failed before examining the actual private-layout boundary.

The broad inventory retains all 90 complete inputs, in the same order and with
the same duplicates. Its existing 20-file owning-module expression now has a
separate named macro, reused inside the broad expression. The purity input uses
that complete owning-module view. Every original purity, facade, placement,
shared-helper and authority assertion remains active and unchanged. The broad
view continues to participate in the original structural assertions.

A new independent inventory assertion compares the broad expansion with every
complete current file in `legacy-inputs.tsv`. It discovers every Rust file in
the owning subtree and follows recursive literal module-path attributes,
including children stored outside that physical directory. Each discovered file
must occur in the broad inventory and its complete bytes must occur in the
purity input. Missing/new nested files, missing owned bytes, removed/reordered
broad inputs and unjoined out-of-directory children fail. Symlinks and paths
outside the repository fail closed. This is a test source-coverage guard, not a
production capability or a general Rust parser.

The complete original assertion body reproduced the wildcard failure with the
broad input and passed with all 20 current owning files. The corrected complete
assertion, the actual inventory assertion and four positive/hostile coverage
controls pass on Rust 1.88 and 1.98. Only immutable input initializers are staged
as prejoined complete file bytes for the bounded local compiler; their order and
duplicates are preserved. Full compiler and hosted qualification remain required.

Three complete predecessor sources and two newly bound guard inputs have exact
hashes and inverses. Live predecessor chains for the changed owners compose the correction. Historical
source-before/candidate copies, `*.before.json` snapshots, original archives and
the formatting registry remain unchanged. The original prelude reconstruction
assertion and all current source-custody controls remain required. Production
code, graph/report pins, quotas, backends, manifests and CI routing are unchanged.

The original prelude source-reconstruction assertion exposed a stale binding
already present at the baseline. Its complete expected input is recovered from
the retained whole-source edit and subsequent original edit, then replayed
through every original inverse to the unchanged archive. Other touched live
chains likewise retain their complete original edit prefixes. Sealed copies of
the old rows and the new custody control verify every touched chain end to end.
The complete original prelude assertion passes with the corrected input binding.

One complete source-history control exceeds the unchanged 200 MiB local process
limit and remains for normal hosted execution. Its repository assertion and CI
selection remain active. No local limit was raised to qualify that control.
