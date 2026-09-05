#!/usr/bin/env python3
"""Fail the aggregate CI gate unless every release blocker really succeeded.

GitHub treats a check run whose conclusion is `skipped` as a satisfied required
status check. An aggregate job guarded by `if: ${{ success() }}` is skipped
exactly when an upstream blocker did not succeed, so it reports `skipped` --
and therefore *satisfies* a required check -- precisely in the failure case it
exists to catch. The gate job runs under `if: ${{ always() }}` instead and
asserts every upstream result here, where the assertion is testable.

`toJSON(needs)` reaches this script through the environment rather than the
command line so an upstream job name can never be spliced into the shell word
list. `--min-jobs` rejects a vacuous `{}`, which is what an accidentally
emptied `needs:` list would otherwise produce.
"""

import argparse
import json
import os
import sys

NEEDS_ENVIRONMENT = "SEMAPRAX_CI_NEEDS"
SUCCESS = "success"
COMMIT_DIGITS = 40
HEXADECIMAL = frozenset("0123456789abcdef")


def commit_failure(label, value):
    """Reject anything that is not an exact lowercase 40-digit Git commit."""
    if len(value) != COMMIT_DIGITS or not set(value) <= HEXADECIMAL:
        return f"{label} is not a 40-digit lowercase hexadecimal commit: {value!r}"
    return None


def failures(needs, minimum, sha, head_sha):
    """Every reason this gate must not report success, in a stable order."""
    if not isinstance(needs, dict):
        kind = type(needs).__name__
        return [f"upstream job results must be a JSON object, got {kind}"]
    reasons = []
    if minimum < 1:
        reasons.append(f"--min-jobs must require at least one job, got {minimum}")
    commits = [
        commit_failure("reported commit", sha),
        commit_failure("checked-out commit", head_sha),
    ]
    reasons.extend(reason for reason in commits if reason is not None)
    if not any(commits) and sha != head_sha:
        reasons.append(
            f"gate ran on checked-out commit {head_sha}, "
            f"not the reported commit {sha}"
        )
    if len(needs) < minimum:
        reasons.append(
            f"expected at least {minimum} upstream jobs, saw {len(needs)}: "
            f"{sorted(needs)}"
        )
    for name in sorted(needs):
        entry = needs[name]
        result = entry.get("result") if isinstance(entry, dict) else None
        if result != SUCCESS:
            reasons.append(
                f"upstream job {name!r} result is {result!r}, not {SUCCESS!r}"
            )
    return reasons


def main(argv=None, environment=None):
    parser = argparse.ArgumentParser(description="Aggregate CI gate verdict.")
    parser.add_argument(
        "--min-jobs",
        type=int,
        required=True,
        help="exact number of jobs the gate must aggregate",
    )
    parser.add_argument("--sha", required=True, help="the commit CI reports")
    parser.add_argument("--head-sha", required=True, help="the commit checked out")
    arguments = parser.parse_args(argv)
    environment = os.environ if environment is None else environment

    raw = environment.get(NEEDS_ENVIRONMENT)
    if raw is None:
        print(
            f"release gate: {NEEDS_ENVIRONMENT} must carry toJSON(needs)",
            file=sys.stderr,
        )
        return 1
    try:
        needs = json.loads(raw)
    except json.JSONDecodeError as error:
        print(
            f"release gate: {NEEDS_ENVIRONMENT} is not valid JSON: {error}",
            file=sys.stderr,
        )
        return 1

    reasons = failures(needs, arguments.min_jobs, arguments.sha, arguments.head_sha)
    for reason in reasons:
        print(f"release gate: {reason}", file=sys.stderr)
    if reasons:
        return 1
    print(f"release gate: {len(needs)} upstream jobs succeeded at {arguments.sha}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
