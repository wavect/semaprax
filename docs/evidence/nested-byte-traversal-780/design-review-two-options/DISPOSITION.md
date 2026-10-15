# Bounded design second opinion (source only)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

The single tools-disabled review completed in 86.34 seconds with exit 0, below the 120-second/768-MiB limits. The raw payload, answer and process receipt are retained unchanged. No Rust compilation or runtime result is claimed.

Select option A: independently validate the actual primitive lowering, all ancestor freezes, current provenance and exact loop targets. Do not add an origin descriptor or forbid unrelated Vec/Iter operations. Mixed source-for syntax remains closed at the typed source gate; a safe handwritten While is not rejected merely because its surface origin cannot be recovered.

The review's concrete integration risks are required controls: every inner subtree remains under every enclosing freeze; every context-bearing consumer receives the exact immutable function census; separate same-root freeze loans survive inner exit, back edges and swapped views. Contextless region replay remains the complete original strict receiver and cannot grant nested authority.

The suggested prohibition on an intervening manual While is not adopted. It supplies no concrete counterexample to independently checked nearest-target and cleanup semantics, and would contradict the approved source profile. Exact lexical target replay remains mandatory. The suggestion to reject existing row/own descriptors is interpreted through their existing independent validators, rather than as an unrelated blanket body ban.

Outermost-entry availability must use binding identity, never just name/type. The current source gate reads the immutable entry scope and the ordinary verifier still rejects shadowing; new HIR census must independently bind exact definition references and positions. Closure captures are evaluated in the caller; closure bodies receive no caller census authority. New storage is paid only under an actual active owner; standalone semantic replay stays explicitly unqualified.

This is a design disposition, not implementation approval. The private HIR helpers remain unregistered until the complete census and actual caller wiring exist. Complete old receiver, full diagnostic precedence, hostile current-context tests, exact/short/zero/sticky and complete Drop controls remain pending executable gates. #780 stays open.
