#!/usr/bin/env python3
"""Emit a deterministic RI-13 authored-code and escape-hatch friction ledger."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EXAMPLES = ROOT.parent
SOURCES = {
    "m1": [
        "ri13-m1-regex-url/consumer/src/main.rs",
        "ri13-m1-regex-url/consumer/build.rs",
        "ri13-m1-regex-url/prepare/src/main.rs",
    ],
    "m2": [
        "ri13-m2-record-iterator/src/bin/consumer.rs",
        "ri13-m2-record-iterator/src/bin/prepare.rs",
        "ri13-m2-record-iterator/build.rs",
    ],
    "m3": [
        "ri13-m3-local-http/src/bin/consumer.rs",
        "ri13-m3-local-http/src/bin/prepare.rs",
    ],
    "linked": [
        "ri13-combined-app/linked/src/main.rs",
        "ri13-combined-app/linked/src/bin_prepare.rs",
        "ri13-combined-app/linked/build.rs",
    ],
}
ESCAPE_HATCHES = ("unsafe", 'extern "C"', "#[no_mangle]", "pub extern")
HOST_CONFIGURATION = {
    "m3": (
        "reqwest::Client::builder",
        "reqwest::Url::parse",
        "tokio::runtime::Builder",
    ),
    "linked": ("m3::register", "Ok::<i64, ()>(43)"),
}


def digest(path):
    return "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()


def lines(text):
    return sum(1 for line in text.splitlines() if line.strip() and not line.lstrip().startswith("//"))


def inspect(relative):
    path = EXAMPLES / relative
    text = path.read_text()
    prohibited = [token for token in ESCAPE_HATCHES if token in text]
    if prohibited:
        raise ValueError(f"{relative} contains prohibited handwritten ABI or unsafe escape hatch: {prohibited}")
    return {"path": relative, "sha256": digest(path), "authored_noncomment_lines": lines(text)}


def ledger():
    rows = {milestone: [inspect(path) for path in paths] for milestone, paths in SOURCES.items()}
    host = {}
    for milestone, fragments in HOST_CONFIGURATION.items():
        text = "\n".join((EXAMPLES / row["path"]).read_text() for row in rows[milestone])
        missing = [fragment for fragment in fragments if fragment not in text]
        if missing:
            raise ValueError(f"{milestone} caller-owned host disclosure changed: {missing}")
        host[milestone] = {"status": "disclosed", "fragments": list(fragments), "count": len(fragments)}
    return {
        "schema": "semaprax.ri13.developer-friction-ledger.v1",
        "milestones": {
            name: {
                "authored_rust_files": entries,
                "authored_noncomment_lines": sum(entry["authored_noncomment_lines"] for entry in entries),
                "handwritten_abi_or_unsafe_escape_hatches": {"count": 0, "status": "refused"},
            }
            for name, entries in rows.items()
        },
        "caller_owned_host_configuration": host,
        "generated_artifacts": {
            "status": "excluded_from_authored_count",
            "paths": [
                "ri13-m1-regex-url/consumer/generated/",
                "ri13-m2-record-iterator/src/generated.rs",
                "ri13-m3-local-http/src/generated.rs",
                "ri13-combined-app/linked/generated/",
            ],
            "reason": "these files are derived during an authenticated prepare stage and are not authored application code",
        },
        "limitations": [
            "line counts are a reproducible disclosure, not a usability or maintenance-quality score",
            "the ledger does not count Cargo or dependency source, generated artifacts, or compiler internals",
            "caller-owned host configuration remains explicit and does not grant generated code ambient authority",
        ],
    }


def main():
    print(json.dumps(ledger(), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
