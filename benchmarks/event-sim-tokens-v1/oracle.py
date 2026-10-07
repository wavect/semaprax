#!/usr/bin/env python3
"""Small reference oracle for the frozen ShiftSim contract."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

IDENTIFIER = re.compile(r"[A-Za-z0-9_-]{1,16}\Z")
REQUEST_KEYS = {"servers", "patients"}
PATIENT_KEYS = {"id", "arrival", "service", "priority", "deadline"}


class InvalidRequest(ValueError):
    pass


def _integer(value: object, low: int, high: int, field: str) -> int:
    if type(value) is not int or not low <= value <= high:
        raise InvalidRequest(f"{field} must be an integer from {low} through {high}")
    return value


def _identifier(value: object, field: str) -> str:
    if not isinstance(value, str) or IDENTIFIER.fullmatch(value) is None:
        raise InvalidRequest(f"{field} must be a 1 to 16 character ASCII identifier")
    return value


def validate(request: object) -> tuple[list[str], list[dict[str, object]]]:
    if not isinstance(request, dict) or set(request) != REQUEST_KEYS:
        raise InvalidRequest("request must contain exactly servers and patients")
    servers_value = request["servers"]
    patients_value = request["patients"]
    if not isinstance(servers_value, list) or len(servers_value) > 8:
        raise InvalidRequest("servers must be an array of at most 8 identifiers")
    if not isinstance(patients_value, list) or len(patients_value) > 256:
        raise InvalidRequest("patients must be an array of at most 256 objects")
    servers = [_identifier(item, "server id") for item in servers_value]
    if len(set(servers)) != len(servers):
        raise InvalidRequest("server identifiers must be unique")
    patients = []
    seen = set()
    for value in patients_value:
        if not isinstance(value, dict) or set(value) != PATIENT_KEYS:
            raise InvalidRequest("each patient must contain exactly the five specified fields")
        patient = {
            "id": _identifier(value["id"], "patient id"),
            "arrival": _integer(value["arrival"], 0, 1_000_000, "arrival"),
            "service": _integer(value["service"], 1, 100_000, "service"),
            "priority": _integer(value["priority"], 0, 9, "priority"),
            "deadline": _integer(value["deadline"], 0, 1_000_000, "deadline"),
        }
        if patient["id"] in seen:
            raise InvalidRequest("patient identifiers must be unique")
        seen.add(patient["id"])
        patients.append(patient)
    if patients and not servers:
        raise InvalidRequest("a non-empty patient array requires at least one server")
    return servers, patients


def simulate(request: object) -> dict[str, object]:
    servers, patients = validate(request)
    pending = sorted(patients, key=lambda p: (p["arrival"], p["id"]))
    waiting: list[dict[str, object]] = []
    active: dict[str, tuple[int, dict[str, object]]] = {}
    visits: list[dict[str, object]] = []
    peak_queue = 0
    cursor = 0
    now = 0

    while cursor < len(pending) or waiting or active:
        next_times = []
        if cursor < len(pending):
            next_times.append(pending[cursor]["arrival"])
        next_times.extend(finish for finish, _ in active.values())
        now = min(next_times)

        for server in sorted(tuple(active)):
            finish, _ = active[server]
            if finish == now:
                del active[server]

        while cursor < len(pending) and pending[cursor]["arrival"] == now:
            waiting.append(pending[cursor])
            cursor += 1
        peak_queue = max(peak_queue, len(waiting))

        free_servers = sorted(set(servers) - set(active))
        while waiting and free_servers:
            waiting.sort(key=lambda p: (p["priority"], p["arrival"], p["id"]))
            patient = waiting.pop(0)
            server = free_servers.pop(0)
            finish = now + patient["service"]
            visits.append({
                "id": patient["id"],
                "server": server,
                "arrival": patient["arrival"],
                "start": now,
                "finish": finish,
                "wait": now - patient["arrival"],
                "late": finish > patient["deadline"],
            })
            active[server] = (finish, patient)

    makespan = max((visit["finish"] for visit in visits), default=0)
    busy_time = sum(patient["service"] for patient in patients)
    total_wait = sum(visit["wait"] for visit in visits)
    return {
        "assignments": visits,
        "metrics": {
            "patients": len(patients),
            "total_wait": total_wait,
            "max_wait": max((visit["wait"] for visit in visits), default=0),
            "late": sum(visit["late"] for visit in visits),
            "busy_time": busy_time,
            "makespan": makespan,
            "peak_queue": peak_queue,
            "utilization_ppm": (
                busy_time * 1_000_000 // (len(servers) * makespan)
                if patients and makespan else 0
            ),
        },
    }


def main() -> int:
    try:
        request = json.load(sys.stdin)
        result = simulate(request)
    except (json.JSONDecodeError, InvalidRequest) as error:
        print(f"invalid request: {error}", file=sys.stderr)
        return 2
    sys.stdout.write(json.dumps(result, ensure_ascii=False, separators=(",", ":")) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
