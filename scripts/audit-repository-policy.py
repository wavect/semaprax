#!/usr/bin/env python3
"""Read-only audit: does GitHub's live `main` ruleset match the policy that
docs/CI-REQUIRED-CHECKS-V1.md documents as the desired end state?

This script mutates nothing. It issues exactly one read (`gh api
repos/<owner>/<repo>/rules/branches/main`, a GET) and compares the result
against the rule types and required-status-check context that the doc's
"Proposed rule" table names. It cannot apply, change, or bypass the ruleset;
doing that needs GitHub organization-administration authority this script
does not have and must not acquire. See
docs/CI-REQUIRED-CHECKS-V1.md#applying-the-proposal for the (also read-only
until executed by an administrator) mutating request that would apply it.

Exit codes distinguish three states that must not be confused with one
another, because "unapplied" and "applied but drifted" call for different
maintainer action:

  0  the documented ruleset is active for `main` with no detected drift.
  1  a ruleset is active for `main`, but it drifts from the documented rule
     types or required-status-check context. This is the failure this
     script exists to catch automatically, per issue #169's acceptance
     criterion "Repository-setting drift is detected automatically."
  2  no ruleset rule is active for `main` at all -- the state this repository
     is in as of this writing (`main` is unprotected). Distinguished from
     exit 1 so "never applied" is never read as "applied then broke".
  3  the read itself failed (network, auth, `gh` not installed, or a
     malformed response) -- an audit inconclusive result, not a policy
     verdict either way.

`rules/branches/main` is deliberately the endpoint this script reads, not
`branches/main` (whose `protected` flag can still read `false` under a
ruleset, per docs/CI-REQUIRED-CHECKS-V1.md's own "Read-back" section) and not
`rulesets?includes_parents=true` (which lists configured rulesets, not the
rules GitHub actually evaluates for the branch). `rules/branches/main` is the
one the doc calls "the decisive one".
"""

import argparse
import json
import re
import subprocess
import sys

DEFAULT_OWNER = "wavect"
DEFAULT_REPO = "semaprax"
DEFAULT_REQUIRED_CONTEXT = "Release gate"

# The three rule types docs/CI-REQUIRED-CHECKS-V1.md's "Proposed rule" table
# names for the `main change integrity` ruleset. Anything else that table
# lists (bypass actors, strict_required_status_checks_policy, etc.) is a
# parameter of these rules, not a fourth rule this script checks for
# independently.
REQUIRED_RULE_TYPES = ("deletion", "non_fast_forward", "required_status_checks")

# Matches the two GitHub personal/app token prefixes this repository's `gh`
# usage could plausibly emit in an error message, plus the generic
# `Authorization: <scheme> <token>` header shape. Defensive: `gh api` does not
# echo credentials in normal operation, but a network or auth error message is
# not a controlled surface, so any accidental token substring is scrubbed
# before this script prints anything a log could retain.
_TOKEN_PATTERN = re.compile(
    r"(gh[oprsu]_[A-Za-z0-9]{20,})|(?i:(authorization:\s*\S+\s+)(\S+))"
)


def redact(text):
    """Scrub anything that looks like a token out of text before printing it."""

    def _scrub(match):
        if match.group(1):
            return "***REDACTED-TOKEN***"
        return f"{match.group(2)}***REDACTED-TOKEN***"

    return _TOKEN_PATTERN.sub(_scrub, text)


def audit(rules, required_context=DEFAULT_REQUIRED_CONTEXT):
    """Compare live `rules/branches/main` entries against the documented
    desired state. Returns `(applied, reasons)`: `applied` is whether any
    ruleset rule is active at all, and `reasons` is every drift finding, in a
    stable order. An empty `reasons` list with `applied=False` means "not yet
    applied, and that is the only finding" -- callers must not read that as
    success.
    """
    if not isinstance(rules, list):
        kind = type(rules).__name__
        return False, [f"expected a JSON array from `rules/branches/main`, got {kind}"]

    if not rules:
        return False, []

    present_types = {
        rule.get("type") for rule in rules if isinstance(rule, dict) and "type" in rule
    }
    reasons = []
    for required_type in REQUIRED_RULE_TYPES:
        if required_type not in present_types:
            reasons.append(f"required rule type `{required_type}` is not active for `main`")

    contexts = set()
    for rule in rules:
        if not isinstance(rule, dict) or rule.get("type") != "required_status_checks":
            continue
        parameters = rule.get("parameters")
        checks = parameters.get("required_status_checks") if isinstance(parameters, dict) else None
        for check in checks or []:
            if isinstance(check, dict) and isinstance(check.get("context"), str):
                contexts.add(check["context"])

    if "required_status_checks" in present_types and required_context not in contexts:
        reasons.append(
            f"`required_status_checks` is active but does not require context "
            f"{required_context!r}; found {sorted(contexts)!r}"
        )

    return True, reasons


def _gh_api_get(endpoint):
    """The one network call this script makes: a plain, unauthenticated-by-us
    GET through `gh api` (which supplies its own stored credentials). No
    `--method`, so this can only ever be a read.
    """
    result = subprocess.run(
        ["gh", "api", endpoint],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RuntimeError(redact(result.stderr.strip() or f"gh api {endpoint} failed"))
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"`gh api {endpoint}` did not return JSON: {error}") from error


def main(argv=None, fetch=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--owner", default=DEFAULT_OWNER, help="repository owner")
    parser.add_argument("--repo", default=DEFAULT_REPO, help="repository name")
    parser.add_argument(
        "--required-context",
        default=DEFAULT_REQUIRED_CONTEXT,
        help="the required-status-check context that must be present",
    )
    arguments = parser.parse_args(argv)
    endpoint = f"repos/{arguments.owner}/{arguments.repo}/rules/branches/main"
    fetch = fetch or _gh_api_get

    try:
        rules = fetch(endpoint)
    except RuntimeError as error:
        # Redacted here too, not only inside `_gh_api_get`: any injected
        # `fetch` (including a test double) may raise a message this script
        # did not construct, and a token substring must never reach stdout
        # or stderr regardless of which layer raised it.
        print(f"policy audit: could not read {endpoint}: {redact(str(error))}", file=sys.stderr)
        return 3

    applied, reasons = audit(rules, arguments.required_context)

    for reason in reasons:
        print(f"policy audit: DRIFT: {reason}", file=sys.stderr)
    if reasons:
        return 1
    if not applied:
        print(
            "policy audit: no ruleset rule is active for `main` -- branch "
            "protection is not yet applied. See "
            "docs/CI-REQUIRED-CHECKS-V1.md#what-remains-unapplied. "
            "This is a known, documented state, not a false pass: "
            "treat exit code 2 as distinct from exit code 0."
        )
        return 2
    print(
        f"policy audit: `main` matches the documented ruleset "
        f"({len(rules)} rules active, `{arguments.required_context}` required)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
