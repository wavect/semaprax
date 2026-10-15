# Bounded base-invocation receiving audit

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Scope: actual CoreProviderProofs-to-base receiver, source-before-producer ordering, same-ledger custody, typed work error and returned nominal payload lifetime. This is a read-only source assessment of an actively changing private worktree. The accompanying hashes bind the source observed after the reported correction; they are not a production approval, runtime receipt or full allocation proof. No Cargo or tests were run.

Concrete finding resolved during review: CaseFields is a struct containing CaseStorage, but its Deref implementation initially matched nonexistent Self::Owned. The source author corrected this to match &self.storage and CaseStorage variants. The current snapshot includes that correction.

The known ledger-affinity finding was already assigned to the source author. Current workspace_graph actual caller passes full CoreProviderProofs through resolve_module_with_core. bind_base_owner checks the original core ledger current identity plus poison state, and records typed Detached on the retained invocation before ensure; a freshly acquired module report owner cannot establish authority for an unrelated retained bundle.

resolve_source_checked_previous_with_base executes full source_verify::verify_presenting before Invocation::ensure. Authored/decoded matching IDs do not select canonical payloads: base_type_for_source requires the exact compiler-owned prelude prefix and exact declaration pointer. Actual returned HIR holds Arc-backed NominalPayload clones, which retain payload ownership after the outer invocation drops.

A question about new map backing was discharged by nominal_rollback::Workspace::acquire: complete current BTree node upper bounds for parameter, variant and case-field maps, including base prelude entries, are reserved through the same callback before build_nominal_index inserts them. The separately named base receiver prebound pays declaration/name maps and additional receiving carriers. This observation does not establish full physical funding for every old/current pipeline allocation.

FixedSchemaPayer retains its first typed work failure; Invocation preserves it without another callback query. The actual root adapter maps that typed Work failure to the already-acquired fixed report slot/text. It neither formats an invented Budget refusal nor claims that the remaining legacy outward Vec/semantic/setup adapters are fully paid.

No further concrete blocker was found in this bounded scope. Source custody joins, whole-source inverses, all feature configurations, exact/short/zero controls, complete allocation/Drop observations and actual execution remain the owning source author's and root's pending gates.
