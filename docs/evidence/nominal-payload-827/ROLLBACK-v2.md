# Construction rollback follow-up v2

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

This private source-only follow-up addresses two identified checkpoint gaps. One prepaid `Arc<Mutex<Vec<ResolvedType>>>` source-shaped frontier now covers the index, local case/field vectors, and the field type awaiting fallible identity acquisition. Each local guard shares that same scratch without allocating. Every current changed BTree map receives the full existing current-layout node upper bound per possible insertion, in addition to the unchanged caller prebound; existing paid primitive builders also use their actual enlarged value layouts. No old-map or clone-removal credit is subtracted. Full simultaneous outer/field/type guard initializers and receivers are reserved before construction.

After all immutable owners and case coordinates are acquired, construction scratch is released before publication. Published host mutation therefore cannot reuse a stale source frontier. Immutable payload mutation requires explicit paid materialization, with a typed capacity error rather than a saturated request followed by an unsupported clone. Case-coordinate mutation never invokes hidden copy-on-write. Legacy host/decoded values retain their compatibility mutation and disposal path.

Three new full-receiver controls are authored: complete independently saved index plus frozen resolver HIR/wire/order/pointer projections; every actual constructor acquisition refusal including nested nominal fields and exact no-allocation-after-selected-error counters; complete old/current semantic diagnostics for missing parents, inheritance cycles and duplicate method IDs. These controls have not run. Existing allocation/work/refusal/Drop controls remain required; source-custody joins, remaining mechanical Rust consumers, final-linker/public/no-owner activation and full physical qualification remain unfinished. No integration, completion, work-saving or app-fit claim is made.
