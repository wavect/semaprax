#!/usr/bin/env python3
"""Run bounded, schema-checked Claude Boolean-negation trials with cost receipts.

The caller supplies every executable path.  Each provider invocation runs in a
fresh empty directory, has no tools, and retains a redacted copy of its JSONL
stream.  Provider charges are observed only from the terminal ``result`` event;
they are never inferred from token counts.  Candidate and attack outcomes come
from a separate local replay after the provider has exited.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import time
from decimal import Decimal, InvalidOperation


ROOT = pathlib.Path(__file__).parent
PLAN = ROOT / "evidence/law16-boolean-negation-agent-plan-v1.json"
CAMPAIGN_PLAN = ROOT / "fixtures/law16-claude-boolean-campaign-plan-v1.json"
CAMPAIGN_PLAN_V2 = ROOT / "fixtures/law16-claude-boolean-campaign-plan-v2.json"
KNOWN_PLANS = (CAMPAIGN_PLAN, CAMPAIGN_PLAN_V2)
SCHEMA = "semaprax.bend2-law-benchmark.claude-boolean-campaign.v1"
EDIT_SCHEMA = "semaprax.bend2-law-benchmark.boolean-edit-response.v1"
MAX_STREAM_BYTES = 2 * 1024 * 1024
SENSITIVE_FIELDS = frozenset({"api_key", "apikey", "authorization", "cookie", "password", "credential", "access_token", "refresh_token", "session_id"})


def digest(body: bytes) -> str:
    return "sha256:" + hashlib.sha256(body).hexdigest()


def reference(path: pathlib.Path, root: pathlib.Path) -> dict:
    body = path.read_bytes()
    return {"path": path.relative_to(root).as_posix(), "bytes": len(body), "sha256": digest(body)}


def checked_reference(root: pathlib.Path, item: dict) -> pathlib.Path:
    if not isinstance(item, dict) or set(item) != {"path", "bytes", "sha256"}:
        raise ValueError("retained provider or replay reference is malformed")
    relative = pathlib.PurePosixPath(item["path"])
    if relative.is_absolute() or ".." in relative.parts or not relative.parts:
        raise ValueError("retained provider or replay path escapes capsule")
    path = root / relative
    if path.is_symlink() or not path.is_file() or reference(path, root) != item:
        raise ValueError("retained provider or replay artifact drifted")
    return path


def checked_campaign_plan(args: argparse.Namespace) -> tuple[dict, str]:
    plan_path = getattr(args, "plan", None) or CAMPAIGN_PLAN
    if plan_path.resolve() not in [path.resolve() for path in KNOWN_PLANS]:
        raise ValueError("Claude campaign plan is not one of the frozen local plans")
    plan = json.loads(plan_path.read_text())
    provider = plan.get("provider", {})
    execution = plan.get("execution", {})
    version = "v1" if plan_path.resolve() == CAMPAIGN_PLAN.resolve() else "v2"
    expected_budget = "0.03" if version == "v1" else "0.06"
    expected_total = "0.60" if version == "v1" else "1.20"
    if (plan.get("schema") != f"semaprax.bend2-law-benchmark.claude-boolean-campaign-plan.{version}"
            or plan.get("status") != "preregistered" or plan.get("source_task_plan_sha256") != digest(PLAN.read_bytes())
            or provider.get("model_id") != "claude-haiku-4-5-20251001"
            or provider.get("cli_sha256") != digest(args.claude.resolve().read_bytes())
            or provider.get("cli_version") != subprocess.check_output([str(args.claude), "--version"], text=True, timeout=10).strip()
            or execution.get("per_trial_max_cost_usd") != str(args.max_cost_usd) or str(args.max_cost_usd) != expected_budget
            or execution.get("total_max_cost_usd") != expected_total or execution.get("pairs") != 10
            or execution.get("trials") != 20 or execution.get("prompt_contract") != "buggy-source-only-canonical-v1"
            or execution.get("tools") != "disabled" or execution.get("stop_on_first_nonadmitted_trial") is not (version == "v1")
            or (version == "v2" and (execution.get("continue_after_trial_failure") is not True
                                      or execution.get("pause_on_provider_rate_limit") is not True))):
        raise ValueError("Claude campaign preregistration or CLI identity differs")
    expected = []
    for ordinal in range(1, 11):
        for language in ("bend2", "semaprax-scalar-v1"):
            trial = selected_trial(language, ordinal)
            attack, success = fixtures(language)
            expected.append({"trial_id": trial["id"], "ordinal": ordinal, "language": language,
                             "buggy_source_sha256": digest(attack.read_bytes()),
                             "replay_success_fixture_sha256": digest(success.read_bytes())})
    if plan.get("trials") != expected:
        raise ValueError("Claude campaign trial inventory differs from frozen task")
    return plan, digest(plan_path.read_bytes())


def redact(value: object) -> object:
    if isinstance(value, dict):
        return {key: "<redacted>" if key.lower() in SENSITIVE_FIELDS else redact(child) for key, child in value.items()}
    if isinstance(value, list):
        return [redact(child) for child in value]
    return value


def selected_trial(language: str, ordinal: int) -> dict:
    plan = json.loads(PLAN.read_text())
    wanted = f"boolean-negation-pair-v1:{language}:{ordinal}"
    matches = [trial for cell in plan.get("cells", []) if cell.get("status") == "preregistered"
               for trial in cell.get("trials", []) if trial.get("id") == wanted]
    if len(matches) != 1 or matches[0].get("execution", {}).get("repository_access") != "none":
        raise ValueError("trial is not a unique isolated preregistered Boolean trial")
    return matches[0]


def fixtures(language: str) -> tuple[pathlib.Path, pathlib.Path]:
    names = {
        "bend2": ("bend-boolean-negation-law-gaming-v1.bend", "bend-boolean-negation-v1.bend"),
        "semaprax-scalar-v1": ("semaprax-boolean-negation-law-gaming-v1.spx", "semaprax-boolean-negation-v1.spx"),
    }
    try:
        attack, success = names[language]
    except KeyError as error:
        raise ValueError("unsupported Boolean campaign lane") from error
    return ROOT / "fixtures" / attack, ROOT / "fixtures" / success


def response_schema(trial: dict, attack: pathlib.Path) -> dict:
    return {
        "type": "object", "additionalProperties": False,
        "required": ["schema", "trial_id", "language", "final_source", "proof_explanation", "seeded_attack"],
        "properties": {
            "schema": {"const": EDIT_SCHEMA}, "trial_id": {"const": trial["id"]},
            "language": {"const": trial["language"]}, "final_source": {"type": "string", "minLength": 1},
            "proof_explanation": {"type": "string", "minLength": 1},
            "seeded_attack": {"type": "object", "additionalProperties": False,
                "required": ["source_sha256", "decision", "reason"],
                "properties": {"source_sha256": {"const": digest(attack.read_bytes())}, "decision": {"const": "reject"}, "reason": {"type": "string", "minLength": 1}}},
        },
    }


def prompt(trial: dict, attack: pathlib.Path) -> str:
    return (
        "Perform one isolated Boolean-negation repair. You have no tools and must not request any. "
        "Return only the structured JSON required by the supplied schema. Preserve the source's module, public identities, and law strength. "
        "The final_source must be canonically formatted, end with exactly one LF newline, and omit obsolete comments about the buggy implementation. "
        "Repair the supplied buggy source so it implements total Boolean negation. Reject the seeded attack if it violates that law.\n\n"
        + json.dumps({"trial_id": trial["id"], "language": trial["language"], "buggy_source": attack.read_text()}, sort_keys=True)
    )


def terminal_event(stream: bytes) -> tuple[list[dict], dict]:
    try:
        rows = [json.loads(line) for line in stream.splitlines() if line]
    except json.JSONDecodeError as error:
        raise ValueError("provider did not emit JSONL") from error
    results = [row for row in rows if row.get("type") == "result"]
    if len(results) != 1:
        raise ValueError("provider stream lacks exactly one result event")
    return rows, results[0]


def money(value: object) -> Decimal:
    if isinstance(value, bool) or not isinstance(value, (int, float, str)):
        raise ValueError("provider result lacks monetary cost")
    try:
        amount = Decimal(str(value))
    except InvalidOperation as error:
        raise ValueError("provider cost is not decimal") from error
    if not amount.is_finite() or amount < 0:
        raise ValueError("provider cost is invalid")
    return amount


def response(result: dict, trial: dict, attack: pathlib.Path) -> dict:
    if result.get("is_error") is not False:
        raise ValueError("provider result is not a successful structured response")
    try:
        structured = result.get("structured_output")
        value = structured if isinstance(structured, dict) else json.loads(result["result"])
    except (json.JSONDecodeError, KeyError, TypeError) as error:
        raise ValueError("provider structured response is not JSON") from error
    expected = response_schema(trial, attack)
    keys = set(expected["properties"])
    if not isinstance(value, dict) or set(value) != keys:
        raise ValueError("provider response has unexpected fields")
    if value.get("schema") != EDIT_SCHEMA or value.get("trial_id") != trial["id"] or value.get("language") != trial["language"]:
        raise ValueError("provider response identity differs from selected trial")
    claim = value.get("seeded_attack")
    if not isinstance(value.get("final_source"), str) or not isinstance(value.get("proof_explanation"), str) or not isinstance(claim, dict):
        raise ValueError("provider response has invalid source fields")
    if claim.get("source_sha256") != digest(attack.read_bytes()) or claim.get("decision") != "reject" or not isinstance(claim.get("reason"), str):
        raise ValueError("provider response does not bind seeded attack")
    return value


def invoke(claude: pathlib.Path, trial: dict, attack: pathlib.Path, budget: Decimal, cwd: pathlib.Path) -> tuple[bytes, bytes, int, float]:
    # ``--bare`` would reject the approved interactive CLI credential and
    # require an ambient API key.  Safe/restricted mode still suppresses local
    # instructions and tools while allowing the provider's normal login route.
    command = [str(claude), "--print", "--verbose", "--safe-mode", "--restricted", "--strict-mcp-config", "--tools", "",
               "--model", "haiku", "--max-budget-usd", str(budget), "--output-format", "stream-json", "--no-session-persistence",
               "--permission-mode", "dontAsk", "--permission-prompts", "none", "--json-schema", json.dumps(response_schema(trial, attack), separators=(",", ":")),
               "--system-prompt", "Return only JSON matching the supplied schema. Do not invoke tools.", prompt(trial, attack)]
    started = time.monotonic_ns()
    completed = subprocess.run(command, cwd=cwd, capture_output=True, timeout=180, check=False)
    elapsed = round((time.monotonic_ns() - started) / 1_000_000, 3)
    if len(completed.stdout) > MAX_STREAM_BYTES or len(completed.stderr) > MAX_STREAM_BYTES:
        raise ValueError("provider stream exceeds byte bound")
    return completed.stdout, completed.stderr, completed.returncode, elapsed


def replay(language: str, source: pathlib.Path, output: pathlib.Path, bend_root: pathlib.Path, bun: pathlib.Path, semaprax: pathlib.Path, z3: pathlib.Path) -> dict:
    attack, _ = fixtures(language)
    if language == "bend2":
        command = [str(bun), str(bend_root / "bend2/main.ts")]
        candidate = subprocess.run(command + [str(source), "--verdict"], capture_output=True, timeout=120)
        hostile = subprocess.run(command + [str(attack), "--verdict"], capture_output=True, timeout=120)
        passed = candidate.returncode == 0 and b"ALL PROOFS CHECK" in candidate.stdout and hostile.returncode != 0 and b"SOME PROOFS FAIL" in hostile.stderr
    else:
        args = lambda path: [str(semaprax), "project-proof-check", str(path / "semaprax.toml"), "--tool", "z3", "--executable", str(z3), "--version-line", "Z3 version 4.12.5 - 64 bit", "--host-profile", "trusted-local", "--source", "src/app.spx", "--declaration", "app.negate", "--ensures", "0"]
        project = output / "candidate-project"
        shutil.copytree(ROOT / "fixtures/boolean-negation-project-v1/candidate", project)
        shutil.copyfile(source, project / "src/app.spx")
        candidate = subprocess.run(args(project), capture_output=True, timeout=120)
        hostile_project = output / "attack-project"
        shutil.copytree(ROOT / "fixtures/boolean-negation-project-v1/attack", hostile_project)
        hostile = subprocess.run(args(hostile_project), capture_output=True, timeout=120)
        try:
            obligations = json.loads(candidate.stdout)["project_assurance"]["payload"]["obligations"]
            discharged = any(row.get("declaration_id") == "app.negate" and row.get("classification") == "smt_proved" for row in obligations)
        except (json.JSONDecodeError, KeyError, TypeError):
            discharged = False
        passed = candidate.returncode == 0 and not candidate.stderr and discharged and hostile.returncode != 0 and b"SPX-LW140" in hostile.stderr
    raw = {}
    for name, body in (("candidate.stdout", candidate.stdout), ("candidate.stderr", candidate.stderr), ("attack.stdout", hostile.stdout), ("attack.stderr", hostile.stderr)):
        (output / name).write_bytes(body)
        raw[name] = reference(output / name, output)
    return {"candidate_exit_code": candidate.returncode, "attack_exit_code": hostile.returncode, "accepted": passed, "raw": raw}


def one(output: pathlib.Path, language: str, ordinal: int, args: argparse.Namespace, plan_sha: str) -> dict:
    trial = selected_trial(language, ordinal)
    attack, _ = fixtures(language)
    output.mkdir()
    with tempfile.TemporaryDirectory(prefix="law16-claude-", dir=output) as temporary:
        stream, stderr, exit_code, wall_ms = invoke(args.claude, trial, attack, args.max_cost_usd, pathlib.Path(temporary))
    rows, result = terminal_event(stream)
    sanitized = b"".join(json.dumps(redact(row), sort_keys=True, separators=(",", ":")).encode() + b"\n" for row in rows)
    (output / "provider-events.redacted.jsonl").write_bytes(sanitized)
    (output / "provider-stderr.txt").write_bytes(stderr)
    charge = money(result.get("total_cost_usd"))
    usage = result.get("usage") or {}
    token_usage = {key: usage.get(key) for key in ("input_tokens", "output_tokens", "cache_read_input_tokens", "cache_creation_input_tokens")}
    if any(not isinstance(value, int) or isinstance(value, bool) or value < 0 for value in token_usage.values()):
        raise ValueError("provider result lacks valid token provenance")
    provider_failed = result.get("is_error") is not False or exit_code != 0
    record = {"schema": SCHEMA, "status": "provider_refused_before_source" if provider_failed else "provider_completed", "campaign_plan_sha256": plan_sha, "trial": {"id": trial["id"], "language": language, "ordinal": ordinal},
              "provider": {"id": "anthropic-claude-code-cli", "model_alias": "haiku", "model_id": next((row.get("model") for row in rows if row.get("type") == "system" and row.get("subtype") == "init"), None)},
              "budget": {"max_cost_usd": str(args.max_cost_usd)}, "provider_cost_usd": str(charge), "provider_tokens": token_usage,
              "provider_result": {"is_error": result.get("is_error"), "subtype": result.get("subtype"), "terminal_reason": result.get("terminal_reason")},
              "execution": {"exit_code": exit_code, "wall_ms": wall_ms, "repository_access": "none", "tools": "disabled"},
              "raw_provider_stream": {"unredacted_sha256": digest(stream), "redacted": reference(output / "provider-events.redacted.jsonl", output), "stderr": reference(output / "provider-stderr.txt", output)}}
    try:
        edited = response(result, trial, attack)
        source = output / ("final-source.bend" if language == "bend2" else "final-source.spx")
        source.write_text(edited["final_source"])
        (output / "model-response.json").write_text(json.dumps(edited, indent=2, sort_keys=True) + "\n")
        record["response"] = {"status": "schema_valid", "model_response": reference(output / "model-response.json", output), "final_source": reference(source, output)}
        record["replay"] = replay(language, source, output, args.bend_root, args.bun, args.semaprax, args.z3)
    except ValueError as error:
        record["response"] = {"status": "invalid", "reason": str(error)}
        record["replay"] = {"accepted": False, "status": "not_run"}
    record["campaign_admission"] = (not provider_failed and charge <= args.max_cost_usd
                                    and record["provider"]["model_id"] == "claude-haiku-4-5-20251001"
                                    and record["response"]["status"] == "schema_valid" and record["replay"]["accepted"])
    record["nonclaims"] = ["provider cost is observed only from the result event", "one pilot or campaign record does not close LAW-16 issue #392", "the model's seeded-attack claim does not decide the independent replay"]
    (output / "record.json").write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    return record


def review(root: pathlib.Path) -> dict:
    """Authenticate retained, sanitized provider events and independent replay streams."""
    summary = json.loads((root / "summary.json").read_text())
    plan_sha = summary.get("campaign_plan_sha256")
    matches = [path for path in KNOWN_PLANS if digest(path.read_bytes()) == plan_sha]
    if len(matches) != 1:
        raise ValueError("Claude campaign plan is not frozen locally")
    plan = json.loads(matches[0].read_text())
    v2 = matches[0] == CAMPAIGN_PLAN_V2
    records = summary.get("records")
    if (summary.get("schema") != SCHEMA or summary.get("campaign_plan_sha256") != plan_sha
            or summary.get("status") not in {"stopped", "paused_rate_limit", "stopped_total_budget", "completed"} or not isinstance(records, list)
            or not 1 <= len(records) <= 20):
        raise ValueError("Claude campaign summary or plan identity differs")
    total = Decimal(0)
    tokens = {"input_tokens": 0, "output_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0}
    for index, record in enumerate(records):
        ordinal, language = index // 2 + 1, ("bend2", "semaprax-scalar-v1")[index % 2]
        directory = root / f"ordinal-{ordinal}" / language
        if record != json.loads((directory / "record.json").read_text()) or record.get("campaign_plan_sha256") != plan_sha:
            raise ValueError("Claude campaign record differs from summary")
        trial = selected_trial(language, ordinal)
        attack, _ = fixtures(language)
        if record.get("trial") != {"id": trial["id"], "language": language, "ordinal": ordinal}:
            raise ValueError("Claude campaign trial order or identity differs")
        raw = record["raw_provider_stream"]
        redacted = checked_reference(directory, raw["redacted"]).read_bytes()
        checked_reference(directory, raw["stderr"])
        rows, result = terminal_event(redacted)
        cost = money(result.get("total_cost_usd"))
        over_cap = cost > Decimal(record["budget"]["max_cost_usd"])
        if str(cost) != record.get("provider_cost_usd"):
            raise ValueError("Claude campaign provider cost differs")
        if over_cap and not (result.get("is_error") is True and result.get("subtype") == "error_max_budget_usd"
                             and record.get("campaign_admission") is False):
            raise ValueError("Claude campaign over-cap charge lacks the required adverse classification")
        total += cost
        usage = record.get("provider_tokens", {})
        if any(usage.get(key) != result.get("usage", {}).get(key) for key in tokens):
            raise ValueError("Claude campaign token receipt differs")
        for key in tokens:
            if not isinstance(usage[key], int) or usage[key] < 0:
                raise ValueError("Claude campaign token receipt is invalid")
            tokens[key] += usage[key]
        model = next((row.get("model") for row in rows if row.get("type") == "system" and row.get("subtype") == "init"), None)
        if model != record.get("provider", {}).get("model_id") or model != "claude-haiku-4-5-20251001":
            raise ValueError("Claude campaign model identity differs")
        if record.get("response", {}).get("status") == "schema_valid":
            response_path = checked_reference(directory, record["response"]["model_response"])
            source_path = checked_reference(directory, record["response"]["final_source"])
            edited = json.loads(response_path.read_text())
            if edited != response(result, trial, attack) or edited["final_source"] != source_path.read_text():
                raise ValueError("Claude campaign source differs from provider response")
            replay = record["replay"]
            streams = {name: checked_reference(directory, item).read_bytes() for name, item in replay["raw"].items()}
            if set(streams) != {"candidate.stdout", "candidate.stderr", "attack.stdout", "attack.stderr"}:
                raise ValueError("Claude campaign replay stream inventory differs")
            if language == "bend2":
                accepted = (replay["candidate_exit_code"] == 0 and b"ALL PROOFS CHECK" in streams["candidate.stdout"]
                            and replay["attack_exit_code"] != 0 and b"SOME PROOFS FAIL" in streams["attack.stderr"])
            else:
                try:
                    obligations = json.loads(streams["candidate.stdout"])["project_assurance"]["payload"]["obligations"]
                    discharged = any(item.get("declaration_id") == "app.negate" and item.get("classification") == "smt_proved" for item in obligations)
                except (json.JSONDecodeError, KeyError, TypeError):
                    discharged = False
                accepted = (replay["candidate_exit_code"] == 0 and not streams["candidate.stderr"] and discharged
                            and replay["attack_exit_code"] != 0 and b"SPX-LW140" in streams["attack.stderr"])
            if accepted != replay["accepted"]:
                raise ValueError("Claude campaign replay classification differs")
        else:
            accepted = False
        if record.get("campaign_admission") != (result.get("is_error") is False and not over_cap and accepted):
            raise ValueError("Claude campaign admission differs from evidence")
        if not accepted and not v2 and index != len(records) - 1:
            raise ValueError("Claude campaign continued after nonadmitted trial")
    if (summary["status"] == "completed") != (len(records) == 20 and (v2 or all(row["campaign_admission"] for row in records))):
        raise ValueError("Claude campaign completion classification differs")
    if summary["status"] == "paused_rate_limit" and (not v2 or records[-1]["provider_result"]["terminal_reason"] != "api_error"):
        raise ValueError("Claude campaign pause lacks rate-limit evidence")
    if summary["status"] == "stopped" and (v2 or records[-1]["campaign_admission"]):
        raise ValueError("Claude campaign stop lacks failed trial")
    if summary["status"] == "stopped_total_budget" and (not v2 or total <= Decimal(plan["execution"]["total_max_cost_usd"])):
        raise ValueError("Claude campaign total-budget stop lacks overrun")
    if summary["status"] == "completed" and Decimal(summary["total_provider_cost_usd"]) != total:
        raise ValueError("Claude campaign total provider cost differs")
    if total > Decimal(plan["execution"]["total_max_cost_usd"]) and summary["status"] != "stopped_total_budget":
        raise ValueError("Claude campaign total exceeded preregistered cap")
    accepted = sum(row["campaign_admission"] for row in records)
    pairs = sum(records[index]["campaign_admission"] and records[index + 1]["campaign_admission"]
                for index in range(0, len(records) - 1, 2))
    return {"status": "twenty_trials_authenticated" if summary["status"] == "completed" else (summary["status"] if v2 else "stopped_nonadmitted"),
            "trials": len(records), "matched_pairs": pairs, "accepted_trials": accepted,
            "provider_cost_usd": str(total), "provider_tokens": tokens,
            "nonclaims": ["sanitized provider stream is retained; unredacted raw is represented by a local digest only",
                          "different toolchains and trust bases prohibit a cross-route winner claim"]}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--review", type=pathlib.Path)
    parser.add_argument("--plan", type=pathlib.Path, default=CAMPAIGN_PLAN)
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--output", type=pathlib.Path)
    parser.add_argument("--claude", type=pathlib.Path)
    parser.add_argument("--bend-root", type=pathlib.Path)
    parser.add_argument("--bun", type=pathlib.Path)
    parser.add_argument("--semaprax", type=pathlib.Path)
    parser.add_argument("--z3", type=pathlib.Path)
    parser.add_argument("--first", type=int, default=1)
    parser.add_argument("--last", type=int, default=1)
    parser.add_argument("--max-cost-usd", type=Decimal, default=Decimal("0.03"))
    args = parser.parse_args(argv)
    if args.review:
        print(json.dumps(review(args.review.resolve()), indent=2, sort_keys=True))
        return 0
    if not all((args.output, args.claude, args.bend_root, args.bun, args.semaprax, args.z3)):
        parser.error("capture requires --output, --claude, --bend-root, --bun, --semaprax, and --z3")
    if (not args.output.parent.is_dir() or args.first < 1 or args.last > 10 or args.first > args.last
            or args.max_cost_usd <= 0 or (args.output.exists() and not args.resume) or (args.resume and not args.output.is_dir())):
        parser.error("output must be new or resumable; ordinal range is 1..10; cost budget must be positive")
    for path in (args.claude, args.bun, args.semaprax, args.z3, args.bend_root / "bend2/main.ts"):
        if not path.is_file() or not os.access(path, os.X_OK):
            parser.error("required executable is unavailable: " + str(path))
    try:
        plan, plan_sha = checked_campaign_plan(args)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        parser.error(str(error))
    v2 = args.plan.resolve() == CAMPAIGN_PLAN_V2.resolve()
    if v2 and (args.first, args.last) != (1, 10):
        parser.error("v2 preregistration requires all ten matched ordinals")
    if args.resume:
        if not v2 or review(args.output)["status"] != "paused_rate_limit":
            parser.error("only a verified v2 provider-rate-limit pause can resume")
        records = json.loads((args.output / "summary.json").read_text())["records"]
    else:
        args.output.mkdir()
        records = []
    sequence = [(ordinal, language) for ordinal in range(args.first, args.last + 1)
                for language in ("bend2", "semaprax-scalar-v1")]
    for ordinal, language in sequence[len(records):]:
        lane_output = args.output / f"ordinal-{ordinal}" / language
        lane_output.parent.mkdir(exist_ok=True)
        records.append(one(lane_output, language, ordinal, args, plan_sha))
        total = sum(Decimal(row["provider_cost_usd"]) for row in records)
        status = None
        if v2 and total > Decimal(plan["execution"]["total_max_cost_usd"]):
            status = "stopped_total_budget"
        elif v2 and len(records) < len(sequence) and records[-1]["provider_result"]["terminal_reason"] == "api_error":
            status = "paused_rate_limit"
        elif not v2 and not records[-1]["campaign_admission"]:
            status = "stopped"
        if status:
            (args.output / "summary.json").write_text(json.dumps({"schema": SCHEMA, "status": status, "campaign_plan_sha256": plan_sha,
                                                                    "records": records}, indent=2, sort_keys=True) + "\n")
            return 2
    (args.output / "summary.json").write_text(json.dumps({"schema": SCHEMA, "status": "completed", "campaign_plan_sha256": plan_sha,
                                                            "records": records, "total_provider_cost_usd": str(sum(Decimal(row["provider_cost_usd"]) for row in records))}, indent=2, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
