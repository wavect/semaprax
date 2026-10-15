# Source-backed disposition, qualification pending

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

One tools-disabled review of the distinct receiving seam completed in72.098s,
exit0, peak326123520 bytes. Raw verdict, exact8426-byte packet, source hashes and
provider usage remain unmodified. This record preserves its conditional objections;
it is not an approval or compile/runtime receipt. The four controls were not in
the review packet and are not represented as independently reviewed.

1. **B1: possible escape from active byte payer.** Actual
   `src/cleanup_plan/replay/allocation.rs:84..124` shows both `new` and
   `with_skeleton_limit` select ActiveBuilder whenever the existing active limit
   exists. They initialize the same payer logic separately (the latter does not
   literally call Self::new). `physical_budget.rs:27..105` dispatches positive
   and zero requests to reserve_active_required and checks sticky refusal.
   The two preexisting fresh work counters are unchanged from the complete old
   cross_check. They cannot refund or reset the enclosing byte ledger. The new
   hook's outer budget pays/authenticates the Storage receiver; the factored
   budget pays any genuinely new frame/nested allocation on that same current
   enclosing ledger. No hypothetical independent free byte quota is introduced.

2. **B2: physical errors become disagreement assertions.** This behavior is
   byte-identical in the retained complete original `factored.rs::cross_check`:
   an enumerated Ok(true) requires the independent factored verdict Ok(Some(true)),
   and every Err remains visibly failed. Actual be8f owner3 already failed with
   that exact assertion before this receiving optimization. The new hook returns
   early program/foreign authentication failures through its Result before
   constructing either comparison; later allocation failure still fails the
   original comparison assertion. It is not reclassified as semantic success,
   silently skipped, or caught by a permissive fallback. This inherited panic
   behavior remains a limit of this private test comparison boundary.

3. **B3: self.program inside acquire cannot authenticate an incoming program.**
   Correct in isolation: that check is ledger/sticky reauthentication. The actual
   mandatory call path first uses the incoming context:
   `replay.rs:2967..2981` checks storage.matches(program) before acquiring, and
   the new hook authenticates the real incoming program before the comparison
   pair. Neither code path substitutes self.program for that external check.
   acquire remains an internal implementation seam; it grants no source admission.

4. **B4: possible mutable current signature.** Actual
   `bounded_output/classifier_string.rs:204..220` defines current(&self), comparing
   Rc identity and poisoning through interior Cell state. as_ref is correct.

5. **Old oracle independence and duplicate current body.** The complete old
   factored cross_check remains compiled and byte-exact in its original owner,
   separately authenticated by full source archives/inverses. The new current
   counterpart preserves its complete enumerating algorithm, sorting/equality,
   fresh factored algorithm and final assertion; only empty backing is borrowed.
   The original and current full pairs both execute inside the new whole owning
   control. The actual required owner3 was not removed or narrowed. Explicit
   opted independent Oracle accounts follow the existing original route and
   never borrow ordinary Storage. Empty physical backing is not a path, HIR fact,
   semantic verdict or proof cache: expression_skeleton clears all owned payloads
   before release. Runtime equivalence and whole funding remain to be qualified.

6. **Sticky first error, empty/no-owner programs and accounting uncertainties.**
   Foreign/program failure is intentionally sticky across restoration, including
   before first backing acquisition. for_program only installs the new receiver
   for ActiveBuilder and a real function; empty and other-payer programs keep the
   explicit old no-receiver route. It captures its Rc only after the actual two
   simultaneously live Storage initializers and returned Result layout acquisition.
   It changes no source/depth/work/physical cap. Zero checks allocate nothing and
   do not bootstrap report text; actual positive setup prepared the receiver's
   original refusal buffer. Exact/short/zero, foreign/restored, old/current complete
   automata/work/diagnostics and whole Drop controls are source-authored, unexecuted.
   A source preflight additionally limits has_receiving_owner to cfg(test), its
   sole actual caller. No broad dead-code suppression is introduced.

7. **Unwind instrumentation.** The copied comparison retains the original
   success-path restoration of SKELETON_MATERIALIZATIONS and its existing panic
   behavior. This packet does not claim to repair unrelated unwind instrumentation.
   Source language evaluation and cleanup order remain unchanged.
