#!/usr/bin/env python3
"""Kernel-0 Lean proof gate (issue #188).

`proofs/kernel0-lean/Kernel0.lean` is a hole-free Lean 4 mechanization of
Progress and Preservation for the whole Kernel-0 language, including `Let`
and non-recursive `Call`, plus fuel-bounded progress over real `Step`s -- see
`docs/KERNEL-PROOF-MECHANIZATION-V1.md` for the design record and
`docs/SEMANTIC-KERNEL-V1.md` for what Kernel-0 is. Before this script
existed, nothing in the repository re-checked that file: no CI job ran
`lake build`, it was not in `scripts/quality.sh`, and nothing would fail if
the proof were reverted, weakened, or silently made to stop building.

This script is that executable gate. It is invoked from `scripts/quality.sh`
(the `full` profile) and, where a Lean toolchain happens to be available,
from CI -- see docs/QUALITY-GATES.md's "Kernel-0 Lean proof gate" section for
which half runs where.

What it catches, and how
-------------------------
1. The proof stops building at all.
   -> `lake build`'s exit code, when a Lean toolchain is on PATH.
2. A `sorry`/`admit` tactic, or a new `axiom`/`constant` declaration, appears in the
   source outside a comment or string literal.
   -> A nesting-aware Lean comment/string stripper (Lean 4 block comments
      nest) followed by a whole-token scan. This runs unconditionally, with
      or without a Lean toolchain, and exists specifically because grepping
      raw source for "sorry" is wrong on this exact file: six of its lines
      say the word "sorry" inside doc comments discussing this very gate,
      none of them a real tactic invocation (see
      docs/KERNEL-PROOF-MECHANIZATION-V1.md, "A concrete pitfall").
      This check is deliberately *not* the authoritative one -- (3) below,
      which inspects the elaborated proof term rather than source text, is
      -- but it is real, cheap, and needs no toolchain.
3. A headline theorem starts depending on a custom axiom, or on `sorryAx`
   (Lean's marker for an admitted hole).
   -> After `lake build`, the gate writes an unpredictable-marker audit
      driver that imports the registered proof modules and issues every owned
      `#print axioms`
      commands itself. Only reports inside that invocation's owned marker
      interval are parsed; missing, duplicate, forged source-owned, or
      unexpected reports fail. Each set must be a subset of `propext`,
      `Classical.choice`, and `Quot.sound`. Requires a Lean toolchain.
4. A headline theorem is deleted, renamed, or its *statement* is weakened
   while the file still builds and still looks axiom-clean -- the failure
   mode a bare `lake build` gate misses entirely.
   -> (a) Presence: each headline name must resolve to exactly one report in
          the gate-owned audit invocation (requires a toolchain).
      (b) Signature pin (no toolchain needed, always runs): each headline
          theorem's exact statement -- from `theorem NAME` through the
          token that starts its proof -- is re-extracted from the source by
          the same algorithm used to produce the frozen copies pinned
          below, after comments and strings have been stripped, and compared
          byte for byte. Changing a hypothesis, a
          conclusion, or a binder fails the gate even if the edited theorem
          still typechecks against its new, weaker statement.
5. A named semantic judgment is weakened while every headline theorem keeps
   the same statement -- for example, by adding a universal `FaultRedex`
   constructor, a zero-cost `Steps.teleport` constructor, or a local notation
   that rebinds `FaultRedex` to `fun _ => True` for later declarations.
   -> The SHA-256 pin of the complete comment/string-stripped live source
      authenticates every command and every gap between declarations. Fifteen
      narrower exact pins identify changes to the main semantic regions. Any
      live command or proof-body change therefore requires a deliberate full
      source repin. Always-run hostile self-tests inject all three attacks and
      require the appropriate pin to fail.

Exit behavior
-------------
The source-level checks (2, 4b, 5, and a name-presence check independent of
`lake`) always run and can fail the gate with no Lean toolchain installed.
The build-dependent checks (1, 3, 4a) require `lake` on PATH; when it is
absent, this script prints an unambiguous `SKIP` line naming exactly what
was not checked and exits 0 for that half only -- it never reports a build
it did not run as a pass, and the overall run only exits 0 if every
applicable check (source-level, always; build-level, when possible) passed.
No network access is attempted; if Lean is not installed, install it
yourself (e.g. via https://leanprover-community.github.io/get_started.html)
-- this script will not fetch it.

`--require-kernel` removes the skip half: an absent `lake` becomes a hard
failure, and `PASS-PARTIAL` stops being an accepted outcome. Skip-on-absence
is right on a developer machine that may have no Lean, and wrong on a runner
that has just provisioned one -- there, a silently skipped build is a green
check over an unbuilt proof, which is precisely the hole this gate was
written to close. Hosted CI passes the flag; `scripts/quality.sh` does not.
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

TAG = "KERNEL0-LEAN-GATE"

SCRIPT_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPT_DIR.parent
PROOF_DIR = REPO_ROOT / "proofs" / "kernel0-lean"
SOURCE = PROOF_DIR / "Kernel0.lean"
TRANSACTION_SOURCE = PROOF_DIR / "TransactionReplay.lean"
TRANSACTION_SOURCE_SHA256 = "ccb2b1cd0ab1a6bad6407260368f84581178d35bbaac54364fbb92068db16bcc"
GRAPH_SOURCE = PROOF_DIR / "GraphProjection.lean"
GRAPH_SOURCE_SHA256 = "b5698e7fd6bb24e0592f005d0307d8e63fdc588be3b9f2a6da64c1303f55c75f"
RECURSIVE_CONTROL = PROOF_DIR / "negative" / "RecursiveCallGraph.lean"
FUEL_CONTROL = PROOF_DIR / "negative" / "InsufficientNormalizationFuel.lean"
STRUCTURAL_CONTROL = PROOF_DIR / "negative" / "ForgedStructuralDecrease.lean"
NAMED_SCOPE_CONTROL = PROOF_DIR / "negative" / "ForgedNamedScope.lean"

# Fully-qualified headline theorem names this gate certifies are present,
# axiom-clean, and unchanged. This gate owns the audit driver; proof-source
# `#print` commands and their output are deliberately not trusted.
HEADLINE_THEOREMS = [
    "progress_scalarIf",
    "progress_scalarIf_closed",
    "preservation",
    "subst_preserves_type",
    "subst_preserves_type_args",
    "hastype_weaken_right",
    "hastype_weaken_right_args",
    "bool_equality_has_type",
    "bool_equality_steps",
    "bool_inequality_steps",
    "bool_ordering_is_not_typed",
    "call_path_rank_bound",
    "ranked_call_graph_acyclic",
    "ranked_call_chain_terminates",
    "recursive_call_fixture_rejected",
    "acyclic_call_fixture_ranked",
    "progress_full",
    "progress_full_args",
    "bounded_step_progress",
    "helper_call_takes_two_steps",
    "helper_call_normalizes_within_two_steps",
    "nodeCount_substEnvAt_values",
    "callTargets_substEnvAt_values",
    "call_beta_substitution_targets_lower",
    "contextual_call_beta_has_ranked_expansion",
    "step_decreases_nodes_or_contextual_call_beta",
    "helper_first_step_is_contextual_call_beta",
    "helper_first_step_not_node_decrease",
    "helper_first_beta_targets_have_lower_rank",
    "weightedPotential_substEnvAt_values",
    "step_decreases_weighted_potential",
    "normalizes_from_weighted_certificate",
    "steps_spend_weighted_potential",
    "normalizes_within_weighted_potential",
    "acyclic_call_fixture_weighted",
    "helper_call_globally_normalizes",
    "named_lookup_resolves",
    "named_lower_preserves_type",
    "named_lower_preserves_type_args",
    "named_lower_output_has_type",
    "named_lower_closed_progress",
    "named_lower_closed_normalizes",
    "named_shadow_lowering",
    "named_unbound_initializer_refused",
    "named_unknown_callee_refused",
    "named_call_preserves_argument_order",
    "named_shadow_has_type",
    "named_helper_reaches_value",
]

GRAPH_HEADLINE_THEOREMS = [
    "project_preserves_exact_inventory",
    "lookupStable_metadata_rewrite",
    "project_metadata_rewrite",
    "compiler_fixture_projection_exact",
    "compiler_fixture_rename_move_preserves_projection",
]

# Frozen, byte-exact expected statement text for each headline theorem,
# extracted from the committed proof by `extract_signature` below (verified
# self-consistent against the file at the time this script was written).
# A change to any of these -- a hypothesis, a conclusion, a binder -- means
# the theorem now proves something different, even if the file still builds
# and still reports a clean axiom set. Do not "fix" a failure here by
# updating the pin without checking why the statement changed.
PINNED_SIGNATURES = {
    "progress_scalarIf": (
        "theorem progress_scalarIf {P Γ e T} (hs : ScalarIf e) (ht : HasType P Γ e T) :\n"
        "    IsValue e ∨ (∃ e', Step P e e') ∨ FaultRedex e"
    ),
    "progress_scalarIf_closed": (
        "theorem progress_scalarIf_closed {P e T} (hs : ScalarIf e) (ht : HasType P [] e T) :\n"
        "    IsValue e ∨ (∃ e', Step P e e') ∨ FaultRedex e"
    ),
    "preservation": (
        "theorem preservation {P Γ e e' T} (hwf : WellFormedProgram P) (ht : HasType P Γ e T)\n"
        "    (hs : Step P e e') : HasType P Γ e' T"
    ),
    "subst_preserves_type": (
        "theorem subst_preserves_type {P : Program} (Γ1 ΓMid Γ2 : List Ty) (env : List Expr) :\n"
        "    ∀ (e : Expr) {T : Ty}, (∀ v ∈ env, IsValue v) →\n"
        "      ArgsHaveTypes P (Γ1 ++ Γ2) env ΓMid →\n"
        "      HasType P (Γ1 ++ ΓMid ++ Γ2) e T →\n"
        "      HasType P (Γ1 ++ Γ2) (substEnvAt Γ1.length env e) T"
    ),
    "subst_preserves_type_args": (
        "theorem subst_preserves_type_args {P : Program} (Γ1 ΓMid Γ2 : List Ty) (env : List Expr) :\n"
        "    ∀ (args : List Expr) {Ts : List Ty}, (∀ v ∈ env, IsValue v) →\n"
        "      ArgsHaveTypes P (Γ1 ++ Γ2) env ΓMid →\n"
        "      ArgsHaveTypes P (Γ1 ++ ΓMid ++ Γ2) args Ts →\n"
        "      ArgsHaveTypes P (Γ1 ++ Γ2) (args.map (substEnvAt Γ1.length env)) Ts"
    ),
    "hastype_weaken_right": (
        "theorem hastype_weaken_right {P : Program} (Γ1 Γ2 : List Ty) :\n"
        "    ∀ (e : Expr) {T : Ty}, HasType P Γ1 e T → HasType P (Γ1 ++ Γ2) e T"
    ),
    "hastype_weaken_right_args": (
        "theorem hastype_weaken_right_args {P : Program} (Γ1 Γ2 : List Ty) :\n"
        "    ∀ (args : List Expr) {Ts : List Ty},\n"
        "      ArgsHaveTypes P Γ1 args Ts → ArgsHaveTypes P (Γ1 ++ Γ2) args Ts"
    ),
    "bool_equality_has_type": (
        "theorem bool_equality_has_type {P Γ a b} :\n"
        "    HasType P Γ (.cmp .eq (.boolLit a) (.boolLit b)) .bool"
    ),
    "bool_equality_steps": (
        "theorem bool_equality_steps {P} :\n"
        "    Step P (.cmp .eq (.boolLit true) (.boolLit false)) (.boolLit false)"
    ),
    "bool_inequality_steps": (
        "theorem bool_inequality_steps {P} :\n"
        "    Step P (.cmp .ne (.boolLit true) (.boolLit false)) (.boolLit true)"
    ),
    "bool_ordering_is_not_typed": (
        "theorem bool_ordering_is_not_typed {P Γ a b} :\n"
        "    ¬ HasType P Γ (.cmp .lt (.boolLit a) (.boolLit b)) .bool"
    ),
    "call_path_rank_bound": (
        "theorem call_path_rank_bound {P rank f g n} (hr : CallGraphRanked P rank)\n"
        "    (hp : CallPath P f g n) : n + rank g ≤ rank f"
    ),
    "ranked_call_graph_acyclic": (
        "theorem ranked_call_graph_acyclic {P rank f n} (hr : CallGraphRanked P rank)\n"
        "    (hp : CallPath P f f n) : n = 0"
    ),
    "ranked_call_chain_terminates": (
        "theorem ranked_call_chain_terminates {P rank} (hr : CallGraphRanked P rank) :\n"
        "    ¬ ∃ chain : Nat → Nat, ∀ n, CallEdge P (chain n) (chain (n + 1))"
    ),
    "recursive_call_fixture_rejected": (
        "theorem recursive_call_fixture_rejected (rank : Nat → Nat) :\n"
        "    ¬ CallGraphRanked recursiveCallFixture rank"
    ),
    "acyclic_call_fixture_ranked": (
        "theorem acyclic_call_fixture_ranked :\n"
        "    CallGraphRanked acyclicCallFixture (fun f => if f = 0 then 1 else 0)"
    ),
    "progress_full": (
        "theorem progress_full {P : Program} :\n"
        "    ∀ (e : Expr) {T : Ty}, HasType P [] e T →\n"
        "      IsValue e ∨ (∃ e', Step P e e') ∨ FaultRedex e"
    ),
    "progress_full_args": (
        "theorem progress_full_args {P : Program} :\n"
        "    ∀ (args : List Expr) {Ts : List Ty}, ArgsHaveTypes P [] args Ts → ArgsProgress P args"
    ),
    "bounded_step_progress": (
        "theorem bounded_step_progress {P e T} (hwf : WellFormedProgram P)\n"
        "    (ht : HasType P [] e T) : ∀ fuel,\n"
        "      NormalizesWithin P e fuel ∨\n"
        "        ∃ frontier next, Steps P e frontier fuel ∧ Step P frontier next"
    ),
    "helper_call_normalizes_within_two_steps": (
        "theorem helper_call_normalizes_within_two_steps :\n"
        "    NormalizesWithin acyclicCallFixture (.call 0 []) 2"
    ),
    "helper_call_takes_two_steps": (
        "theorem helper_call_takes_two_steps :\n"
        "    Steps acyclicCallFixture (.call 0 []) (.intLit 42) 2"
    ),
    "nodeCount_substEnvAt_values": (
        "theorem nodeCount_substEnvAt_values (base : Nat) (env : List Expr) :\n"
        "    (∀ v ∈ env, IsValue v) → ∀ e,\n"
        "      nodeCount (substEnvAt base env e) = nodeCount e"
    ),
    "callTargets_substEnvAt_values": (
        "theorem callTargets_substEnvAt_values (base : Nat) (env : List Expr) :\n"
        "    (∀ v ∈ env, IsValue v) → ∀ e,\n"
        "      callTargets (substEnvAt base env e) = callTargets e"
    ),
    "call_beta_substitution_targets_lower": (
        "theorem call_beta_substitution_targets_lower {P rank f args fd}\n"
        "    (hr : CallGraphRanked P rank) (hf : P[f]? = some fd)\n"
        "    (hargs : ∀ v ∈ args, IsValue v) :\n"
        "    ∀ g ∈ callTargets (substEnvAt 0 args fd.body), rank g < rank f"
    ),
    "contextual_call_beta_has_ranked_expansion": (
        "theorem contextual_call_beta_has_ranked_expansion {P rank e e' f}\n"
        "    (hr : CallGraphRanked P rank) (hb : ContextualCallBeta P e e' f) :\n"
        "    ∃ args fd, P[f]? = some fd ∧ (∀ v ∈ args, IsValue v) ∧\n"
        "      ∀ g ∈ callTargets (substEnvAt 0 args fd.body), rank g < rank f"
    ),
    "step_decreases_nodes_or_contextual_call_beta": (
        "theorem step_decreases_nodes_or_contextual_call_beta {P e e'} (hs : Step P e e') :\n"
        "    nodeCount e' < nodeCount e ∨ ∃ f, ContextualCallBeta P e e' f"
    ),
    "helper_first_step_is_contextual_call_beta": (
        "theorem helper_first_step_is_contextual_call_beta :\n"
        "    ContextualCallBeta acyclicCallFixture (.call 0 []) (.call 1 []) 0"
    ),
    "helper_first_step_not_node_decrease": (
        "theorem helper_first_step_not_node_decrease :\n"
        "    ¬ nodeCount (.call 1 []) < nodeCount (.call 0 [])"
    ),
    "helper_first_beta_targets_have_lower_rank": (
        "theorem helper_first_beta_targets_have_lower_rank :\n"
        "    ∀ g ∈ callTargets (substEnvAt 0 [] (Expr.call 1 [])),\n"
        "      (if g = 0 then 1 else 0) < (if (0 : Nat) = 0 then 1 else 0)"
    ),
    "weightedPotential_substEnvAt_values": (
        "theorem weightedPotential_substEnvAt_values (weight : Nat → Nat) (base : Nat)\n"
        "    (env : List Expr) : (∀ v ∈ env, IsValue v) → ∀ e,\n"
        "      weightedPotential weight (substEnvAt base env e) = weightedPotential weight e"
    ),
    "step_decreases_weighted_potential": (
        "theorem step_decreases_weighted_potential {P weight e e'}\n"
        "    (hc : WeightedCallCertificate P weight) (hs : Step P e e') :\n"
        "    weightedPotential weight e' < weightedPotential weight e"
    ),
    "normalizes_from_weighted_certificate": (
        "theorem normalizes_from_weighted_certificate {P weight e T}\n"
        "    (hwf : WellFormedProgram P) (hc : WeightedCallCertificate P weight)\n"
        "    (ht : HasType P [] e T) :\n"
        "    ∃ out n, Steps P e out n ∧ Terminal out"
    ),
    "steps_spend_weighted_potential": (
        "theorem steps_spend_weighted_potential {P weight e out n}\n"
        "    (hc : WeightedCallCertificate P weight) (hs : Steps P e out n) :\n"
        "    n + weightedPotential weight out ≤ weightedPotential weight e"
    ),
    "normalizes_within_weighted_potential": (
        "theorem normalizes_within_weighted_potential {P weight e T}\n"
        "    (hwf : WellFormedProgram P) (hc : WeightedCallCertificate P weight)\n"
        "    (ht : HasType P [] e T) :\n"
        "    NormalizesWithin P e (weightedPotential weight e)"
    ),
    "acyclic_call_fixture_weighted": (
        "theorem acyclic_call_fixture_weighted :\n"
        "    WeightedCallCertificate acyclicCallFixture acyclicCallWeight"
    ),
    "helper_call_globally_normalizes": (
        "theorem helper_call_globally_normalizes :\n"
        "    ∃ out n, Steps acyclicCallFixture (.call 0 []) out n ∧ Terminal out"
    ),
    "named_lookup_resolves": (
        "theorem named_lookup_resolves {id Γ T} (h : lookupNamedType id Γ = some T) :\n"
        "    ∃ i, resolveIdentity id (Γ.map Prod.fst) = some i ∧\n"
        "      (Γ.map Prod.snd)[i]? = some T"
    ),
    "named_lower_preserves_type": (
        "theorem named_lower_preserves_type {P fs Γ term T}\n"
        "    (ht : NamedHasType P fs Γ term T) :\n"
        "    ∃ e, lowerNamed fs (Γ.map Prod.fst) term = some e ∧ HasType P (Γ.map Prod.snd) e T"
    ),
    "named_lower_preserves_type_args": (
        "theorem named_lower_preserves_type_args {P fs Γ terms Ts}\n"
        "    (ht : NamedArgsHaveTypes P fs Γ terms Ts) :\n"
        "    ∃ es, lowerNamedArgs fs (Γ.map Prod.fst) terms = some es ∧\n"
        "      ArgsHaveTypes P (Γ.map Prod.snd) es Ts"
    ),
    "named_lower_output_has_type": (
        "theorem named_lower_output_has_type {P fs Γ term T e}\n"
        "    (ht : NamedHasType P fs Γ term T)\n"
        "    (he : lowerNamed fs (Γ.map Prod.fst) term = some e) :\n"
        "    HasType P (Γ.map Prod.snd) e T"
    ),
    "named_lower_closed_progress": (
        "theorem named_lower_closed_progress {P fs term T e}\n"
        "    (ht : NamedHasType P fs [] term T) (he : lowerNamed fs [] term = some e) :\n"
        "    IsValue e ∨ (∃ e', Step P e e') ∨ FaultRedex e"
    ),
    "named_lower_closed_normalizes": (
        "theorem named_lower_closed_normalizes {P fs term T e weight}\n"
        "    (ht : NamedHasType P fs [] term T) (he : lowerNamed fs [] term = some e)\n"
        "    (hwf : WellFormedProgram P) (hc : WeightedCallCertificate P weight) :\n"
        "    ∃ out n, Steps P e out n ∧ Terminal out"
    ),
    "named_shadow_lowering": (
        "theorem named_shadow_lowering :\n"
        "    lowerNamed [] [] namedShadowFixture = some\n"
        "      (.letIn (.intLit 40) (.letIn (.arith .add (.var 0) (.intLit 2))\n"
        "        (.letIn (.boolLit true) (.var 1))))"
    ),
    "named_unbound_initializer_refused": (
        "theorem named_unbound_initializer_refused :\n"
        "    lowerNamed [] [] (.letIn 7 (.var 7) (.intLit 0)) = none"
    ),
    "named_unknown_callee_refused": (
        "theorem named_unknown_callee_refused :\n"
        "    lowerNamed [11, 13] [] (.call 12 [.intLit 1]) = none"
    ),
    "named_call_preserves_argument_order": (
        "theorem named_call_preserves_argument_order :\n"
        "    lowerNamed [11, 13] [5, 7] (.call 13 [.var 7, .var 5]) =\n"
        "      some (.call 1 [.var 1, .var 0])"
    ),
    "named_shadow_has_type": (
        "theorem named_shadow_has_type : NamedHasType [] [] [] namedShadowFixture .int"
    ),
    "named_helper_reaches_value": (
        "theorem named_helper_reaches_value :\n"
        "    ∃ e, lowerNamed [11, 13] [] (.call 11 []) = some e ∧\n"
        "      HasType acyclicCallFixture [] e .int ∧\n"
        "      Steps acyclicCallFixture e (.intLit 42) 2"
    ),
}

# Comment/string-stripped exact code regions that define the judgments named
# by the headline theorems. Statement pins alone are insufficient: adding a
# universal FaultRedex case or a Steps.teleport constructor would preserve all
# theorem signatures while making Progress/normalization vacuous.
SEMANTIC_REGION_PINS = [
    ("scalar_types", "inductive Ty where", "inductive Expr where",
     "b768c629fdcc0220431fd5940f4d0491a7259c2446e1c019dcad75ca7694ead7"),
    ("expr", "inductive Expr where", "structure FunDef where",
     "2770163eb3f2db8db74f5264c1add39df8159e8c61c6784eb149b18bf576a6dc"),
    ("program", "structure FunDef where", "  def callTargets",
     "9c815bfe67a32ec43e542689c29b6c6cbe7553d92660305b1c70bc02f279ee15"),
    ("values", "inductive IsValue", "def i64Min",
     "cb9c67b24a5a946efc020e7bfb51d63f71b133b24347eff1c02d75b1fce897e1"),
    ("arithmetic", "def i64Min", "def BoolEquality",
     "2d31cece6ea66e1aeedc7a4aab8b05cc12411cfb2e690a6f2c4740aae183b1ee"),
    ("typing", "  inductive HasType", "def WellFormedProgram",
     "a4c726eb70e81d4217a8712a05be8e6af112951c5041959ee3b1dcbf9768d66c"),
    ("well_formed", "def WellFormedProgram", "def substEnvAt",
     "265a2b2394bff6850650abbd88523854f44373f408f1e60393809dd003154427"),
    ("substitution", "def substEnvAt", "inductive Step",
     "08a5f3734bbc5f038380402f5cc9df8db4056a285abe89184aed5d9696424dcf"),
    ("step", "inductive Step", "inductive FaultRedex",
     "7e75f1760a0257449089adf8a6511fe5a7908bee5625203baa07c3958566f33d"),
    ("fault", "inductive FaultRedex", "theorem canonical_int",
     "22ccb6ec2c8b7147092e193369028f385b355f71ffdcfa76c4751b2502fa43f7"),
    ("args_progress", "inductive ArgsProgress", "theorem progress_full",
     "99cf370b72cdeab1b541f46a3f766c27ceff9ec050e13c7ca718adc989ca1ae5"),
    ("structural_decrease", "  def nodeCount : Expr → Nat", "inductive Steps",
     "3eb5482b7c5bda52c0e3261103b36c6115ceb4b22ad1f175ad854af9b3d8ce24"),
    ("bounded_steps", "inductive Steps", "theorem bounded_step_progress",
     "d37830caa89da3799719efc5842d0a6566498ff3c00ada15faee6297ac2c0edc"),
    ("weighted_normalization", "theorem bounded_step_progress",
     "theorem helper_call_takes_two_steps",
     "1a44f8f073693c3665ea353e7df42c006341bdebbcb6722a4ed88148a9eeba2d"),
    ("named_lowering", "inductive NamedTerm where", "end Kernel0",
     "23523eeb06b8e0da92ab996547b0e532e49ced058b849f89a9a9e2a1ec0b4988"),
]

# This closes the gaps between the targeted regions above. Lean commands in a
# gap can change how a later declaration elaborates without changing that
# declaration's source text; for example, a local notation can rebind the token
# `FaultRedex` to an always-true predicate. Keep the narrower pins for precise
# diagnostics, but require this digest of every live command and proof body as
# the authoritative environment pin. Comments and string contents are removed
# by the same lexer used for signatures and token checks. Do not update this
# value merely to make the gate green: every live-source change needs review.
PINNED_LIVE_SOURCE_SHA256 = (
    "d8e648c73da8da7a12224b8b5b1a7b88fae371c87c155e69670e6b6d1ed4a55d"
)

PINNED_RECURSIVE_CONTROL = """import Kernel0
open Kernel0
theorem forged_recursive_call_rank :
CallGraphRanked recursiveCallFixture (fun _ => 0) := by
intro caller callee edge
exact Nat.le_refl 0"""

PINNED_FUEL_CONTROL = """import Kernel0
open Kernel0
theorem forged_one_step_normalization :
NormalizesWithin acyclicCallFixture (.call 0 []) 1 := by
refine ⟨.intLit 42, 2, ?_, helper_call_takes_two_steps, Or.inl (.intLit 42)⟩
exact Nat.le_refl 2"""

PINNED_STRUCTURAL_CONTROL = """import Kernel0
open Kernel0
theorem forged_helper_beta_structural_decrease :
nodeCount (.call 1 []) < nodeCount (.call 0 []) := by
show 1 < 1
exact Nat.lt_succ_self 0"""

PINNED_NAMED_SCOPE_CONTROL = """import Kernel0
open Kernel0
theorem forged_named_scope :
lowerNamed [] [8, 7] (.var 7) = some (.var 0) := by
change some (Expr.var 1) = some (Expr.var 0)
exact (rfl : some (Expr.var 1) = some (Expr.var 1))"""

ALLOWED_AXIOMS = {"propext", "Classical.choice", "Quot.sound"}

AXIOM_INFO_RE = re.compile(r"'([\w.]+)' depends on axioms: \[([^\]]*)\]")
AXIOM_FREE_INFO_RE = re.compile(r"'([\w.]+)' does not depend on any axioms")


def fail(msg: str) -> None:
    print(f"{TAG}: FAIL: {msg}", file=sys.stderr)


def extract_signature(lines: list[str], name: str) -> str | None:
    """Re-implements, against arbitrary source lines, the same extraction
    used to produce PINNED_SIGNATURES: from `theorem NAME` up to (but not
    including) whichever comes first -- a ` := by` on some line, a bare
    trailing `:=`, or an equation-compiler arm (`  | ...`) starting the
    proof. Returns None if the name is not declared as a top-level theorem
    at all (deleted or renamed)."""
    start_re = re.compile(r"^theorem " + re.escape(name) + r"\b")
    start = None
    for i, line in enumerate(lines):
        if start_re.match(line):
            start = i
            break
    if start is None:
        return None
    sig_lines: list[str] = []
    i = start
    while i < len(lines):
        line = lines[i]
        if i > start and line.startswith("  | "):
            return "\n".join(sig_lines)
        if " := by" in line:
            sig_lines.append(line.split(" := by")[0])
            return "\n".join(sig_lines)
        stripped = line.rstrip()
        if stripped.endswith(" :="):
            sig_lines.append(stripped[: -len(" :=")])
            return "\n".join(sig_lines)
        if stripped == ":=":
            return "\n".join(sig_lines)
        sig_lines.append(line)
        i += 1
    return None  # ran off the end of the file without a terminator


def strip_comments_and_strings(text: str) -> str:
    """Lean 4 line comments (`--` to end of line), block comments (`/- -/`,
    which NEST -- including `/--`/`/-!` doc comments, the same lexeme
    family), and string literals (`"..."` with `\\"` escapes), replaced with
    spaces so token positions/line count are preserved but no comment or
    string content can masquerade as a real `sorry`/`admit`/`axiom` token."""
    out = []
    i = 0
    n = len(text)
    depth = 0  # block-comment nesting depth
    while i < n:
        if depth > 0:
            if text[i : i + 2] == "/-":
                depth += 1
                out.append("  ")
                i += 2
                continue
            if text[i : i + 2] == "-/":
                depth -= 1
                out.append("  ")
                i += 2
                continue
            out.append(" " if text[i] != "\n" else "\n")
            i += 1
            continue
        if text[i : i + 2] == "/-":
            depth = 1
            out.append("  ")
            i += 2
            continue
        if text[i : i + 2] == "--":
            j = text.find("\n", i)
            j = n if j == -1 else j
            out.append(" " * (j - i))
            i = j
            continue
        if text[i] == '"':
            out.append(" ")
            i += 1
            while i < n and text[i] != '"':
                if text[i] == "\\" and i + 1 < n:
                    out.append("  ")
                    i += 2
                    continue
                out.append(" " if text[i] != "\n" else "\n")
                i += 1
            if i < n:
                out.append(" ")
                i += 1
            continue
        out.append(text[i])
        i += 1
    return "".join(out)


TOKEN_RE = re.compile(r"\b(sorry|admit)\b")
AXIOM_DECL_RE = re.compile(r"\b(?:axiom|constant)\s+\w")


def canonical_live_source(stripped: str) -> str:
    """Drop comment-only gaps and insignificant trailing whitespace."""
    return "\n".join(
        line.rstrip() for line in stripped.splitlines() if line.strip()
    )


def semantic_region_digest(stripped: str, start_marker: str, end_marker: str) -> str | None:
    start = stripped.find(start_marker)
    if start < 0:
        return None
    end = stripped.find(end_marker, start + len(start_marker))
    if end < 0:
        return None
    # Comments/strings are already spaces. Discard empty lines and trailing
    # whitespace so documentation wording/length is not part of the semantic
    # pin, while every live token and indentation remains exact.
    canonical = canonical_live_source(stripped[start:end])
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def live_source_pin_failure(stripped: str) -> str | None:
    actual = hashlib.sha256(canonical_live_source(stripped).encode("utf-8")).hexdigest()
    if actual == PINNED_LIVE_SOURCE_SHA256:
        return None
    return (
        "complete live Lean source changed: expected "
        f"sha256:{PINNED_LIVE_SOURCE_SHA256}, actual sha256:{actual}; "
        "review every live command/proof-body change before deliberately repinning"
    )


def semantic_pin_failures(stripped: str) -> list[str]:
    failures: list[str] = []
    for name, start, end, expected in SEMANTIC_REGION_PINS:
        actual = semantic_region_digest(stripped, start, end)
        if actual is None:
            failures.append(f"semantic boundary `{name}` markers are missing or reordered")
        elif actual != expected:
            failures.append(
                f"semantic boundary `{name}` changed: expected sha256:{expected}, "
                f"actual sha256:{actual}"
            )
    return failures


def check_source_level(source_text: str) -> list[str]:
    """Runs every check that needs only the source text, no Lean toolchain.
    Returns a list of failure messages (empty means this half passed)."""
    failures: list[str] = []

    # Strip before declaration lookup as well as before token scanning. A
    # commented copy of a pinned signature must never shadow a weakened live
    # theorem later in the file.
    stripped = strip_comments_and_strings(source_text)
    stripped_lines = stripped.split("\n")

    # (4) presence + (4b) signature pin.
    missing = []
    changed = []
    for name in HEADLINE_THEOREMS:
        actual = extract_signature(stripped_lines, name)
        if actual is None:
            missing.append(name)
            continue
        expected = PINNED_SIGNATURES[name]
        if actual != expected:
            changed.append((name, expected, actual))
    if missing:
        failures.append(
            "headline theorem(s) not found in source (deleted or renamed): "
            + ", ".join(missing)
        )
    for name, expected, actual in changed:
        failures.append(
            f"headline theorem `{name}` signature changed.\n"
            f"    expected:\n      "
            + "\n      ".join(expected.splitlines())
            + "\n    actual:\n      "
            + "\n      ".join(actual.splitlines())
        )
    if not missing and not changed:
        print(
            f"{TAG}: source-presence-and-signature OK "
            f"({len(HEADLINE_THEOREMS)}/{len(HEADLINE_THEOREMS)} headline "
            "theorems present with unchanged pinned statements)"
        )

    # (2) sorry/admit/axiom/constant token scan, comment- and string-aware.
    bad_tokens = sorted(set(m.group(1) for m in TOKEN_RE.finditer(stripped)))
    axiom_decls = AXIOM_DECL_RE.findall(stripped)
    if bad_tokens:
        failures.append(
            "real (non-comment, non-string) tactic token(s) found: "
            + ", ".join(bad_tokens)
        )
    if axiom_decls:
        failures.append(
            f"{len(axiom_decls)} custom `axiom`/`constant` declaration(s) found outside "
            "comments/strings; this proof is meant to depend on no axiom "
            "beyond Lean's own propext/Classical.choice/Quot.sound"
        )
    if not bad_tokens and not axiom_decls:
        print(
            f"{TAG}: sorry/admit/axiom/constant token scan OK (0 found outside "
            "comments and string literals)"
        )

    semantic_failures = semantic_pin_failures(stripped)
    failures.extend(semantic_failures)
    if not semantic_failures:
        print(
            f"{TAG}: semantic-boundary pins OK "
            f"({len(SEMANTIC_REGION_PINS)}/{len(SEMANTIC_REGION_PINS)} exact code regions)"
        )

    source_pin_failure = live_source_pin_failure(stripped)
    if source_pin_failure is not None:
        failures.append(source_pin_failure)
    else:
        print(f"{TAG}: complete live-source environment pin OK (sha256 exact)")

    if not RECURSIVE_CONTROL.is_file():
        failures.append("recursive-call negative control is missing")
    else:
        control = strip_comments_and_strings(RECURSIVE_CONTROL.read_text(encoding="utf-8"))
        normalized = "\n".join(line.strip() for line in control.splitlines() if line.strip())
        if normalized != PINNED_RECURSIVE_CONTROL:
            failures.append("recursive-call negative control changed from its pinned forged certificate")

    if not FUEL_CONTROL.is_file():
        failures.append("normalization-fuel negative control is missing")
    else:
        control = strip_comments_and_strings(FUEL_CONTROL.read_text(encoding="utf-8"))
        normalized = "\n".join(line.strip() for line in control.splitlines() if line.strip())
        if normalized != PINNED_FUEL_CONTROL:
            failures.append("normalization-fuel negative control changed from its pinned forged certificate")

    if not STRUCTURAL_CONTROL.is_file():
        failures.append("structural-decrease negative control is missing")
    else:
        control = strip_comments_and_strings(STRUCTURAL_CONTROL.read_text(encoding="utf-8"))
        normalized = "\n".join(line.strip() for line in control.splitlines() if line.strip())
        if normalized != PINNED_STRUCTURAL_CONTROL:
            failures.append("structural-decrease negative control changed from its pinned forged certificate")

    if not NAMED_SCOPE_CONTROL.is_file():
        failures.append("named-scope negative control is missing")
    else:
        control = strip_comments_and_strings(NAMED_SCOPE_CONTROL.read_text(encoding="utf-8"))
        normalized = "\n".join(line.strip() for line in control.splitlines() if line.strip())
        if normalized != PINNED_NAMED_SCOPE_CONTROL:
            failures.append("named-scope negative control changed from its pinned forged scope")

    return failures


def render_axiom_audit_driver(begin: str, end: str) -> str:
    commands = "\n".join(f"#print axioms Kernel0.{name}" for name in HEADLINE_THEOREMS)
    graph_commands = "\n".join(
        f"#print axioms Kernel0.GraphProjection.{name}"
        for name in GRAPH_HEADLINE_THEOREMS
    )
    return (
        "import Kernel0\n"
        "import GraphProjection\n"
        f'#eval IO.println "{begin}"\n'
        f"{commands}\n"
        f"{graph_commands}\n"
        f'#eval IO.println "{end}"\n'
    )


def validate_owned_axiom_output(output: str, begin: str, end: str) -> list[str]:
    """Validate only reports bracketed by this run's unpredictable markers.

    Imported source can print arbitrary text while elaborating, but cannot
    remove the gate-owned commands. Duplicate/missing reports inside the
    owned interval fail closed.
    """
    failures: list[str] = []
    if output.count(begin) != 1 or output.count(end) != 1:
        return ["gate-owned axiom audit markers are missing or duplicated"]
    start = output.index(begin) + len(begin)
    finish = output.index(end, start)
    if finish <= start:
        return ["gate-owned axiom audit markers are out of order"]
    owned = output[start:finish]
    if "sorryAx" in owned:
        failures.append("`sorryAx` appears in gate-owned axiom audit output")

    found: dict[str, list[list[str]]] = {}
    for match in AXIOM_INFO_RE.finditer(owned):
        name, axioms_str = match.group(1), match.group(2)
        axioms = [a.strip() for a in axioms_str.split(",") if a.strip()]
        found.setdefault(name, []).append(axioms)
    for match in AXIOM_FREE_INFO_RE.finditer(owned):
        found.setdefault(match.group(1), []).append([])

    expected = {f"Kernel0.{name}" for name in HEADLINE_THEOREMS} | {
        f"Kernel0.GraphProjection.{name}" for name in GRAPH_HEADLINE_THEOREMS
    }
    unexpected = sorted(set(found) - expected)
    if unexpected:
        failures.append("unexpected theorem report(s) in owned axiom audit: " + ", ".join(unexpected))
    for qualified in sorted(expected):
        occurrences = found.get(qualified, [])
        if len(occurrences) != 1:
            failures.append(
                f"`{qualified}` produced {len(occurrences)} owned axiom reports; expected exactly one"
            )
            continue
        axioms = occurrences[0]
        extra = [axiom for axiom in axioms if axiom not in ALLOWED_AXIOMS]
        if extra:
            failures.append(
                f"`{qualified}` depends on axiom(s) outside "
                f"{sorted(ALLOWED_AXIOMS)}: {axioms} (unexpected: {extra})"
            )
    return failures


def hostile_gate_self_tests(source_text: str) -> list[str]:
    """Exercise source, report, and semantic-relation spoofing regressions."""
    failures: list[str] = []
    commented = """/-
theorem victim : True := by
-/
theorem victim : False := by
  trivial"""
    actual = extract_signature(strip_comments_and_strings(commented).split("\n"), "victim")
    if actual != "theorem victim : False":
        failures.append("self-test: commented signature shadowed the live theorem")

    constant_attack = strip_comments_and_strings("/- constant decoy : Prop -/\nconstant forged : Prop")
    if len(AXIOM_DECL_RE.findall(constant_attack)) != 1:
        failures.append("self-test: live `constant` declaration evaded the custom-axiom scan")

    begin = "KERNEL0-OWNED-AUDIT-BEGIN-self-test"
    end = "KERNEL0-OWNED-AUDIT-END-self-test"
    forged = "\n".join(
        f"info: 'Kernel0.{name}' depends on axioms: [propext]"
        for name in HEADLINE_THEOREMS
    )
    # This is exactly what a proof source can emit after removing its own
    # #print commands. Without the gate-owned unpredictable markers it is not
    # audit evidence.
    if not validate_owned_axiom_output(forged, begin, end):
        failures.append("self-test: unowned forged axiom reports were accepted")
    # Lean prints a different sentence for empty axiom sets. Accept it as
    # an audited result, but count duplicates across both output forms.
    empty = "\n".join(
        f"info: 'Kernel0.{name}' does not depend on any axioms"
        for name in HEADLINE_THEOREMS
    )
    graph_empty = "\n".join(
        f"info: 'Kernel0.GraphProjection.{name}' does not depend on any axioms"
        for name in GRAPH_HEADLINE_THEOREMS
    )
    empty = f"{empty}\n{graph_empty}"
    if validate_owned_axiom_output(f"{begin}\n{empty}\n{end}", begin, end):
        failures.append("self-test: genuine empty axiom reports were refused")
    duplicate = f"{begin}\n{empty}\n{forged}\n{end}"
    if not validate_owned_axiom_output(duplicate, begin, end):
        failures.append("self-test: mixed-format duplicate axiom reports were accepted")
    driver = render_axiom_audit_driver(begin, end)
    driver_lines = driver.splitlines()
    if any(driver_lines.count(f"#print axioms Kernel0.{name}") != 1 for name in HEADLINE_THEOREMS):
        failures.append("self-test: gate-owned driver omitted or duplicated a headline theorem")
    if any(
        driver_lines.count(f"#print axioms Kernel0.GraphProjection.{name}") != 1
        for name in GRAPH_HEADLINE_THEOREMS
    ):
        failures.append(
            "self-test: gate-owned driver omitted or duplicated a graph-projection theorem"
        )

    teleport = source_text.replace(
        "inductive Steps (P : Program) : Expr → Expr → Nat → Prop where\n",
        "inductive Steps (P : Program) : Expr → Expr → Nat → Prop where\n"
        "  | teleport (from to) : Steps P from to 0\n",
        1,
    )
    teleport_failures = semantic_pin_failures(strip_comments_and_strings(teleport))
    if teleport == source_text or not any("`bounded_steps` changed" in failure
                                         for failure in teleport_failures):
        failures.append("self-test: injected `Steps.teleport` constructor evaded semantic pins")

    universal_fault = source_text.replace(
        "inductive FaultRedex : Expr → Prop where\n",
        "inductive FaultRedex : Expr → Prop where\n"
        "  | universal (e) : FaultRedex e\n",
        1,
    )
    fault_failures = semantic_pin_failures(strip_comments_and_strings(universal_fault))
    if universal_fault == source_text or not any("`fault` changed" in failure
                                                  for failure in fault_failures):
        failures.append("self-test: universal `FaultRedex` constructor evaded semantic pins")

    notation_attack = source_text.replace(
        "inductive ArgsProgress (P : Program) : List Expr → Prop where\n",
        'local notation "FaultRedex" => (fun _ : Expr => True)\n\n'
        "inductive ArgsProgress (P : Program) : List Expr → Prop where\n",
        1,
    )
    notation_stripped = strip_comments_and_strings(notation_attack)
    notation_failure = live_source_pin_failure(notation_stripped)
    if notation_attack == source_text:
        failures.append("self-test: could not inject local-notation semantic-gap attack")
    elif semantic_pin_failures(notation_stripped):
        failures.append(
            "self-test: local-notation attack no longer isolates the disjoint-region gap"
        )
    elif notation_failure is None:
        failures.append("self-test: local-notation `FaultRedex := True` attack evaded full-source pin")

    scope_attack = source_text.replace(
        "(← lowerNamed functions locals value)",
        "(← lowerNamed functions (id :: locals) value)",
        1,
    )
    scope_failures = semantic_pin_failures(strip_comments_and_strings(scope_attack))
    if scope_attack == source_text or not any("`named_lowering` changed" in failure
                                             for failure in scope_failures):
        failures.append("self-test: let initializer scope capture evaded named-lowering pin")
    return failures


def check_build_level(require_kernel: bool, workspace_test_build: bool) -> tuple[list[str], bool]:
    """Runs `lake build` and audits its output. Returns (failures, ran) --
    `ran` is False when no toolchain was available at all, in which case
    `failures` is empty unless `require_kernel` was asked for, which turns
    the absence itself into the failure."""
    lake = shutil.which("lake")
    if lake is None:
        if require_kernel:
            return (
                [
                    "`lake` not found on PATH and --require-kernel was "
                    "passed: the proof was NOT rebuilt and its axiom set "
                    "was NOT re-audited. A caller that promises a "
                    "provisioned kernel and then finds none has a "
                    "provisioning failure, not a skippable check."
                ],
                False,
            )
        print(
            f"{TAG}: SKIP: `lake` not found on PATH -- the proof was NOT "
            "rebuilt and its axiom set was NOT re-audited this run. This is "
            "a skip, not a pass: failure modes 1 (build breaks) and 3 "
            "(custom/sorryAx axiom dependency) were not checked. Install a "
            "Lean 4 toolchain (see "
            "https://leanprover-community.github.io/get_started.html) to "
            "get full coverage; this script will not fetch one itself. "
            "See docs/QUALITY-GATES.md's \"Kernel-0 Lean proof gate\" "
            "section."
        )
        return [], False

    proc = subprocess.run(
        ["lake", "build"],
        cwd=str(PROOF_DIR),
        capture_output=True,
        text=True,
    )
    failures: list[str] = []

    if proc.returncode != 0:
        combined = proc.stdout + proc.stderr
        failures.append(
            f"`lake build` exited {proc.returncode} (proof does not build):\n"
            + "\n".join("    " + l for l in combined.splitlines())
        )
        # The independent audit driver needs an importable built module.
        return failures, True

    nonce = secrets.token_hex(24)
    begin = f"KERNEL0-OWNED-AUDIT-BEGIN-{nonce}"
    end = f"KERNEL0-OWNED-AUDIT-END-{nonce}"
    with tempfile.TemporaryDirectory(prefix="semaprax-kernel0-audit-") as temporary:
        driver = Path(temporary) / "GateOwnedAxiomAudit.lean"
        driver.write_text(render_axiom_audit_driver(begin, end), encoding="utf-8")
        audit = subprocess.run(
            [lake, "env", "lean", str(driver)],
            cwd=str(PROOF_DIR),
            capture_output=True,
            text=True,
        )
    audit_output = audit.stdout + audit.stderr
    if audit.returncode != 0:
        failures.append(
            f"gate-owned axiom audit exited {audit.returncode}:\n"
            + "\n".join("    " + line for line in audit_output.splitlines())
        )
    else:
        failures.extend(validate_owned_axiom_output(audit_output, begin, end))

    if not failures:
        audited = len(HEADLINE_THEOREMS) + len(GRAPH_HEADLINE_THEOREMS)
        print(f"{TAG}: `lake build` OK (exit 0 in {PROOF_DIR})")
        print(
            f"{TAG}: gate-owned axiom-set audit OK for {audited}/{audited} "
            "headline theorems (each a subset of "
            f"{sorted(ALLOWED_AXIOMS)})"
        )

    # The transaction theorem is useful only when exact compiler-produced
    # rename/replace fixtures elaborate too. The Rust test fresh-replays the
    # engine, writes a gate-owned create-new fixture, invokes this pinned Lean,
    # and audits every general and concrete theorem's axiom report.
    if not failures:
        cargo = shutil.which("cargo")
        lean_lookup = subprocess.run(
            [lake, "env", "which", "lean"],
            cwd=str(PROOF_DIR),
            capture_output=True,
            text=True,
        )
        lean = lean_lookup.stdout.strip() if lean_lookup.returncode == 0 else ""
        if not lean or cargo is None:
            failures.append(
                "transaction replay fixture gate requires the pinned `lake env lean` and `cargo`"
            )
        else:
            with tempfile.TemporaryDirectory(prefix="semaprax-transaction-lean-") as temporary:
                fixture = Path(temporary) / "TransactionReplayFixture.lean"
                environment = dict(os.environ)
                environment["SEMAPRAX_TRANSACTION_LEAN"] = lean
                environment["SEMAPRAX_TRANSACTION_FIXTURE"] = str(fixture)
                selection = (
                    ["--workspace", "--all-features"]
                    if workspace_test_build
                    else ["-p", "semaprax"]
                )
                test_name = (
                    "kernel_zero::transaction::lean::tests::"
                    "exact_rename_and_replace_fixtures_are_deterministic_and_kernel_checkable"
                )
                transaction = subprocess.run(
                    [cargo, "test", "--locked", *selection, "--lib", test_name, "--", "--exact"],
                    cwd=str(REPO_ROOT),
                    env=environment,
                    capture_output=True,
                    text=True,
                )
                fixture_written = fixture.is_file() and fixture.stat().st_size > 0
            output = transaction.stdout + transaction.stderr
            passed = sum(
                int(count)
                for count in re.findall(r"^test result: ok\. ([0-9]+) passed;", output, re.MULTILINE)
            )
            if transaction.returncode != 0 or passed != 1 or not fixture_written:
                failures.append(
                    "transaction replay fixture gate did not execute exactly one passing test:\n"
                    + output
                )
            else:
                print(f"{TAG}: transaction replay real-fixture Lean gate OK (1 passed)")

    # The built module supplies the real CallGraphRanked definition. A forged
    # constant rank for its nested recursive fixture must be rejected by the
    # kernel, specifically at the <= proof offered where strict < is required.
    # Import/build failures and unrelated syntax errors are not a passing control.
    if not failures:
        control = subprocess.run(
            [lake, "env", "lean", str(RECURSIVE_CONTROL.relative_to(PROOF_DIR))],
            cwd=str(PROOF_DIR), capture_output=True, text=True,
        )
        output = control.stdout + control.stderr
        errors = [line for line in output.splitlines() if "error:" in line]
        normalized_output = re.sub(r"\s+", " ", output)
        # Lean can display the expected constant-rank application before
        # beta reduction. Accept exactly that equivalent form as well as 0.
        normalized_output = re.sub(
            r"\(fun \w+ => 0\) (?:caller|callee)\b", "0", normalized_output
        )
        if (
            control.returncode == 0
            or len(errors) != 1
            or "Type mismatch" not in errors[0]
            or "Nat.le_refl 0 has type 0 ≤ 0" not in normalized_output
            or "expected to have type 0 < 0" not in normalized_output
            or "sorryAx" in output
        ):
            failures.append(
                "recursive-call negative control did not fail at the expected strict-rank "
                "type mismatch:\n" + output
            )
        else:
            print(f"{TAG}: recursive-call negative control OK (forged rank rejected)")

    # The positive fixture takes two real Step witnesses.  Reusing those
    # witnesses under a one-step budget must fail specifically at 2 ≤ 1.
    if not failures:
        control = subprocess.run(
            [lake, "env", "lean", str(FUEL_CONTROL.relative_to(PROOF_DIR))],
            cwd=str(PROOF_DIR), capture_output=True, text=True,
        )
        output = control.stdout + control.stderr
        errors = [line for line in output.splitlines() if "error:" in line]
        normalized_output = re.sub(r"\s+", " ", output)
        if (
            control.returncode == 0
            or len(errors) != 1
            or "Type mismatch" not in errors[0]
            or "Nat.le_refl 2 has type 2 ≤ 2" not in normalized_output
            or "expected to have type 2 ≤ 1" not in normalized_output
            or "sorryAx" in output
        ):
            failures.append(
                "normalization-fuel negative control did not fail at the expected "
                "2 ≤ 1 type mismatch:\n" + output
            )
        else:
            print(f"{TAG}: normalization-fuel negative control OK (one-step forgery rejected)")

    # Raw syntax size alone cannot orient call beta: the positive helper's
    # first real step maps one call node to one call node. A forged strict
    # decrease must fail exactly at the normalized 1 < 1 obligation.
    if not failures:
        control = subprocess.run(
            [lake, "env", "lean", str(STRUCTURAL_CONTROL.relative_to(PROOF_DIR))],
            cwd=str(PROOF_DIR), capture_output=True, text=True,
        )
        output = control.stdout + control.stderr
        errors = [line for line in output.splitlines() if "error:" in line]
        normalized_output = re.sub(r"\s+", " ", output)
        if (
            control.returncode == 0
            or len(errors) != 1
            or "Type mismatch" not in errors[0]
            or "Nat.lt_succ_self 0" not in normalized_output
            or "has type 0 < Nat.succ 0" not in normalized_output
            or "expected to have type 1 < 1" not in normalized_output
            or "sorryAx" in output
        ):
            failures.append(
                "structural-decrease negative control did not fail at the expected "
                "1 < 1 type mismatch:\n" + output
            )
        else:
            print(f"{TAG}: structural-decrease negative control OK (false size decrease rejected)")

    if not failures:
        control = subprocess.run(
            [lake, "env", "lean", str(NAMED_SCOPE_CONTROL.relative_to(PROOF_DIR))],
            cwd=str(PROOF_DIR), capture_output=True, text=True,
        )
        output = control.stdout + control.stderr
        errors = [line for line in output.splitlines() if "error:" in line]
        normalized_output = re.sub(r"\s+", " ", output)
        if (
            control.returncode == 0
            or len(errors) != 1
            or "Type mismatch" not in errors[0]
            or "some (Expr.var 1) = some (Expr.var 1)" not in normalized_output
            or "expected to have type some (Expr.var 1) = some (Expr.var 0)" not in normalized_output
            or "sorryAx" in output
        ):
            failures.append(
                "named-scope negative control did not fail at the expected "
                "outer-slot 1 versus inner-slot 0 mismatch:\n" + output
            )
        else:
            print(f"{TAG}: named-scope negative control OK (captured outer identity rejected)")

    return failures, True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--require-kernel",
        action="store_true",
        help=(
            "treat a missing Lean toolchain as a failure rather than a "
            "skip, so PASS-PARTIAL can never be reported as a pass"
        ),
    )
    parser.add_argument(
        "--workspace-test-build",
        action="store_true",
        help="use the workspace all-features library build for the Rust fixture",
    )
    arguments = parser.parse_args()

    if not SOURCE.is_file():
        fail(f"expected proof source not found at {SOURCE}")
        return 1
    if not TRANSACTION_SOURCE.is_file():
        fail(f"expected transaction proof source not found at {TRANSACTION_SOURCE}")
        return 1
    if not GRAPH_SOURCE.is_file():
        fail(f"expected graph-projection proof source not found at {GRAPH_SOURCE}")
        return 1

    source_text = SOURCE.read_text(encoding="utf-8")
    transaction_bytes = TRANSACTION_SOURCE.read_bytes()
    transaction_text = transaction_bytes.decode("utf-8")
    transaction_stripped = strip_comments_and_strings(transaction_text)
    transaction_failures = []
    actual_transaction_digest = hashlib.sha256(transaction_bytes).hexdigest()
    if actual_transaction_digest != TRANSACTION_SOURCE_SHA256:
        transaction_failures.append(
            "transaction replay proof changed from its exact source pin: "
            f"expected sha256:{TRANSACTION_SOURCE_SHA256}, actual sha256:{actual_transaction_digest}"
        )
    if TOKEN_RE.search(transaction_stripped) or AXIOM_DECL_RE.search(transaction_stripped):
        transaction_failures.append(
            "transaction replay proof contains sorry/admit/axiom/constant outside comments or strings"
        )
    if not transaction_failures:
        print(
            f"{TAG}: transaction replay exact-source pin and hole scan OK"
        )
    graph_bytes = GRAPH_SOURCE.read_bytes()
    graph_text = graph_bytes.decode("utf-8")
    graph_stripped = strip_comments_and_strings(graph_text)
    graph_failures = []
    actual_graph_digest = hashlib.sha256(graph_bytes).hexdigest()
    if actual_graph_digest != GRAPH_SOURCE_SHA256:
        graph_failures.append(
            "graph projection proof changed from its exact source pin: "
            f"expected sha256:{GRAPH_SOURCE_SHA256}, actual sha256:{actual_graph_digest}"
        )
    if TOKEN_RE.search(graph_stripped) or AXIOM_DECL_RE.search(graph_stripped):
        graph_failures.append(
            "graph projection proof contains sorry/admit/axiom/constant outside comments or strings"
        )
    if not graph_failures:
        print(f"{TAG}: graph projection exact-source pin and hole scan OK")
    self_test_failures = hostile_gate_self_tests(source_text)
    if not self_test_failures:
        print(
            f"{TAG}: hostile gate self-tests OK "
            "(commented signature, forged report/removal, constant declaration, "
            "Steps.teleport, universal FaultRedex, local-notation FaultRedex rebinding, "
            "let initializer scope capture)"
        )

    source_failures = check_source_level(source_text)
    build_failures, build_ran = check_build_level(
        arguments.require_kernel, arguments.workspace_test_build
    )

    all_failures = (
        self_test_failures
        + transaction_failures
        + graph_failures
        + source_failures
        + build_failures
    )
    if all_failures:
        for msg in all_failures:
            fail(msg)
        print(f"{TAG}: RESULT: FAIL ({len(all_failures)} check(s) failed)")
        return 1

    if build_ran:
        print(f"{TAG}: RESULT: PASS (source checks + lake build + axiom audit)")
    else:
        print(
            f"{TAG}: RESULT: PASS-PARTIAL (source-level checks only -- "
            "`lake build`/axiom audit SKIPPED, no Lean toolchain on PATH)"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
