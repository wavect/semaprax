"""Model-visible projection for the original specialization task inventory.

The benchmark EQUIVALENCE documents contain private vector/mutant discussion;
they are reviewer material, not model prompts. This additive projection keeps
public requirements, interface declarations and fixed public scaffolds only.
"""
from __future__ import annotations
import json
import re

from .local_ollama import LocalTransportError, canonical, digest
from .specialization_accounting import FROZEN_TASK_IDS

PREFIX = "benchmarks/cross-language-v1/"
CANDIDATES = {task: ("src/candidate.spx",) for task in FROZEN_TASK_IDS}
CANDIDATES.update({
    "module-import-refactor-v1": ("src/candidate.spx", "src/helper.spx"),
    "owned-byte-sentinel-balance-v1": ("src/app.spx",),
    "clean-install-calculator-v1": ("src/core.spx",),
})
# Requirements only: no sampled inputs, expected hidden answers, mutants or
# descriptions of what the hidden suite specifically distinguishes.
BRIEFS = {
    "module-import-refactor-v1": "Implement invoice_total(price, quantity, tax_rate, shipping) for nonnegative integers: subtotal = price * quantity; tax = floor(subtotal * tax_rate / 100); return subtotal + tax + shipping. Import tax_for_subtotal from the separate helper module and use it for tax on the whole subtotal.",
    "booking-window-conflict-v1": "Implement the declared booking-window predicate. Inputs are valid nonempty half-open windows. Return integer 1 exactly when a_start < b_end and b_start < a_end, otherwise 0; touching endpoints are not an overlap. Malformed windows are outside this task.",
    "cold-chain-release-gate-v1": "Implement the declared shipment release predicate. Return integer 1 exactly when core_temperature is in the inclusive interval [2, 8] AND seal_pressure is in [95, 105], otherwise 0. Both inputs are signed integers.",
    "stable-dispatch-order-v1": "Implement the declared ordering of three jobs by increasing priority, preserving their arrival order on equal priority. The fixed identifiers are a=1, b=2, c=3. Encode their sorted order as the three decimal digits of an integer. Equal priorities retain a before b before c.",
    "owned-byte-sentinel-balance-v1": "Implement sentinel_checksum for an owned byte sequence of at most eight bytes. Map 255 to 0, 0 to 255, and other bytes to 1. Return the sum of one-based position times mapped value. The borrowed evaluate wrapper must copy its input to owned Bytes before calling the private consuming operation. Settle owned resources according to the language's ownership rules.",
    "stale-edit-preservation-v1": "Unavailable: the written fractional-discount oracle and the frozen predecessor ports disagree. Do not generate a candidate until a separately reviewed correction resolves that discrepancy.",
    "telemetry-overflow-diagnosis-v1": "Implement the declared telemetry combiner using saturating signed 32-bit arithmetic. Compute the mathematical sum and clamp to the signed 32-bit minimum and maximum rather than wrapping or trapping. Use ordinary control flow and comparisons, with overflow checks before the addition; no library saturating-arithmetic shortcut. Preserve the public function interface.",
    "clean-install-calculator-v1": "Add subtract(left, right) to the calculator core while retaining the existing add(left, right) behavior and stable identifiers. Leave the generated project manifest, application and test scaffold unchanged.",
    "concurrent-delta-merge-v1": "Merge two independently arriving deltas against one common counter base. Add both deltas to the common base before clamping the combined value to [0, 1000000]. Do not clamp either intermediate delta result. Preserve the declared public interface.",
}
GUIDANCE = """SEMAPRAX language guidance (no task examples or tuning data)
One module declaration per file, first. Keep stable @id identities and the
provided function signatures. A block ends with its value expression; there
is no return statement, expression statement, for loop, or else if syntax.
Keep exact scalar types: i64 is default; i32/u8/usize need typed literals.
Use if/else expressions with compatible branch types. Match enum variants
exhaustively. Ownership is explicit: own transfers; borrow does not transfer.
Owned Bytes must be consumed exactly once. bytes_as_slice borrows a view,
bytes_copy creates owned Bytes, bytes_zeroed creates a zero-filled buffer,
bytes_set consumes the buffer and returns its replacement; retain that
replacement. byte_get returns Option. Explicitly handle both alternatives.
Do not add external dependencies, shell commands, filesystem or network APIs.
"""
UNAVAILABLE = {"stale-edit-preservation-v1": "unresolved_fractional_discount_oracle"}


def public_tree(sources: dict[str, bytes], task: str) -> dict[str, bytes]:
    if task not in FROZEN_TASK_IDS:
        raise LocalTransportError("task_outside_original_matrix")
    prefix = PREFIX + "tasks/" + task + "/public/semaprax/"
    result = {name[len(prefix):]: data for name, data in sources.items() if name.startswith(prefix)}
    if not result or not set(CANDIDATES[task]).issubset(result):
        raise LocalTransportError("missing_public_candidate_inventory")
    return result


def public_surface(data: bytes) -> str:
    """Expose only fixed corpus declarations, never candidate implementation.

    This is a narrow presentation projection, not a source parser or a new
    oracle. The containing source snapshot authenticates the full file first.
    Unsupported declaration shapes are refused rather than loosely parsed.
    """
    text = data.decode("utf-8")
    module = re.findall(r"^module [a-zA-Z0-9_.]+;$", text, re.M)
    declarations = re.findall(r'^(@id\("[^"\n]+"\)\nfn [^\n{]+)\n\{', text, re.M)
    if len(module) != 1 or not declarations or len(declarations) != len(re.findall(r"^fn ", text, re.M)):
        raise LocalTransportError("unsupported_public_interface_projection")
    return module[0] + "\n\n" + "\n\n".join(declarations)


def model_prompt(task: str, variant: str, sources: dict[str, bytes]) -> tuple[str, list[dict]]:
    if variant not in ("base", "guided", "constrained") or task in UNAVAILABLE:
        raise LocalTransportError("unavailable_task_or_control")
    files = public_tree(sources, task)
    paths = CANDIDATES[task]
    parts = ["Implement the task in SEMAPRAX.\n" + BRIEFS[task]]
    provenance = []
    for path, data in sorted(files.items()):
        # README/AGENTS content can itself import broad review instructions;
        # only the fixed manifest and actual public source are admitted here.
        if path != "semaprax.toml" and not path.endswith(".spx"):
            continue
        if path in paths:
            content = public_surface(data)
            kind = "interface_only"
        else:
            content = data.decode("utf-8")
            kind = "unchanged_public_scaffold"
        parts.append("## " + path + " (" + kind + ")\n" + content)
        provenance.append({"path": PREFIX + "tasks/" + task + "/public/semaprax/" + path,
                           "projection": kind, "projected_sha256": digest(content.encode())})
    if variant != "base":
        parts.append(GUIDANCE)
    parts.append('Return one JSON object with exactly a "files" object. Supply the complete contents '
                 'of exactly these relative files as JSON strings: ' + json.dumps(list(paths)) +
                 '. No Markdown fences, commentary, extra paths or executable tool requests.')
    return "\n\n".join(parts), provenance


def projection_identity():
    return digest(canonical({"candidates": CANDIDATES, "briefs": BRIEFS,
                             "guidance": GUIDANCE, "unavailable": UNAVAILABLE}))
