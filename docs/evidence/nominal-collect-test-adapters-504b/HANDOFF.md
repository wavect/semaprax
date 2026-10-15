# Explicit legacy collector type adapters

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

IndexMap::insert accepts impl Into<V>; both Vec and NominalPayload implement FromIterator. Two existing test collectors therefore lose their former contextual Vec type. Only their local binding annotations now say Vec<crate::hir::ResolvedFieldDeclaration>. The complete iterator, field clones, metadata publication, insertion order, hostile mutations, physical comparisons and assertions remain unchanged. No production API or implicit materialization is introduced.

Both complete prior files are archived byte-for-byte from504b8381e and fixed by literal SHA256. A test in the existing text-classifier harness reconstructs both full originals through exact byte-position splices. Both comparisons pass mechanically; rustfmt parses current source via stdout without rewriting original literal bytes; diff-check passes. Current enclosing inventories already include both same child paths, so no child relocation/source-join removal occurs. No prior protected archive hash was changed. Three selectors are pending; no Cargo, build, tests, runtime or root edits occurred.
