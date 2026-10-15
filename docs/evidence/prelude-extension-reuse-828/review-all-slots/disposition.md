# Source-only disposition

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

One actual review of the distinct complete-slot work/partial-variant seam, terminal exit0,108.957s; no tools or code execution. Raw verdict and actual usage retained. This is not compile/runtime qualification.

- B1 resolved from `src/hir/ids.rs:27..33`: DeclarationId Clone retains either a static str or Arc<str>, allocating no backing. A source comment states this at the parameter clone. Inline resolved roots are already charged; no clone backing payment is invented.
- B2 resolved from `src/hir/nodes.rs`: ResolvedVariantCaseDeclaration.fields is Vec<ResolvedFieldDeclaration>, ResolvedType::Nominal.arguments is Vec<ResolvedType>; Record.fields is NominalPayload. These are deliberately different carriers. Complete cases own their nested Vec fields, with one funded outer nominal owner.
- U1 corrected: actual mapped Result<StagedType,Failure<P::Error>> and Result<Fields,Failure<P::Error>> receiving layouts are now charged before constructing partial roots. Vector backing and per-element simultaneous initializers are distinct, not duplicate backing credit. Option<ResolvedType> is the inline iterative disposal cursor, separately live from returned guard/root.
- U2 strengthened the existing all-prefix control with exact nine names, IDs, and variant/empty-record kind mapping against actual compiler producer objects. The actual production AST pointer authentication and per-slot exact used==limit controls remain intact.
- U3 actual IDs are const &str in iterator_ops/list_ops/map_ops/stdin_stream_ops.
- Work algebra and guarded transitions checked by reviewer; no work bound/cap/quota changed. Existing all-slot exact/short/zero, full suffix every acquisition, sticky foreign/restored and whole Drop controls remain source-authored and unexecuted. No runtime, peak, gain, or sole-cause claim.
