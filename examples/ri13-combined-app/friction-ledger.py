#!/usr/bin/env python3
"""Emit a deterministic RI-13 authored-code and comparison-route disclosure."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EXAMPLES = ROOT.parent
APPLICATION_SOURCES = {
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
COMPARISON_HARNESSES = {
    "m1": {
        "paths": ["ri13-m1-regex-url/consumer/src/bin/measure.rs"],
        "handwritten_adapter_symbols": ["HandwrittenRegex", "HandwrittenUrl"],
    },
    "m2": {
        "paths": ["ri13-m2-record-iterator/src/bin/measure.rs"],
        "handwritten_adapter_symbols": ["HandwrittenAdapter", "HandwrittenRecordAdapter"],
    },
    "m3": {
        "paths": ["ri13-m3-local-http/src/bin/measure.rs"],
        "handwritten_adapter_symbols": ["Route::Handwritten"],
    },
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
GENERATED_CODE_LOCATIONS = {
    "m1": ["ri13-m1-regex-url/generated/"],
    "m2": [
        "ri13-m2-record-iterator/generated/",
        "ri13-m2-record-iterator/src/generated.rs",
        "ri13-m2-record-iterator/src/semaprax_native_rust_interop_ffi.rs",
    ],
    "m3": ["ri13-m3-local-http/src/generated.rs"],
    "linked": ["ri13-combined-app/linked/generated/"],
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


def inspect_comparison(relative):
    """Disclose instrumentation and handwritten comparison code without calling it app ABI."""
    path = EXAMPLES / relative
    text = path.read_text()
    return {
        "path": relative,
        "sha256": digest(path),
        "authored_noncomment_lines": lines(text),
        "escape_hatch_tokens": {token: text.count(token) for token in ESCAPE_HATCHES if token in text},
    }


def ledger():
    application = {
        milestone: [inspect(path) for path in paths]
        for milestone, paths in APPLICATION_SOURCES.items()
    }
    comparisons = {}
    for milestone, specification in COMPARISON_HARNESSES.items():
        entries = [inspect_comparison(path) for path in specification["paths"]]
        combined = "\n".join((EXAMPLES / path).read_text() for path in specification["paths"])
        symbols = specification["handwritten_adapter_symbols"]
        missing = [symbol for symbol in symbols if symbol not in combined]
        if missing:
            raise ValueError(f"{milestone} comparison route lost handwritten adapter disclosure: {missing}")
        comparisons[milestone] = {
            "comparison_rust_files": entries,
            "handwritten_adapter_symbols": symbols,
            "purpose": "direct-Rust and handwritten-adapter benchmark comparison; not a generated application ABI",
        }
    host = {}
    for milestone, fragments in HOST_CONFIGURATION.items():
        text = "\n".join((EXAMPLES / row["path"]).read_text() for row in application[milestone])
        missing = [fragment for fragment in fragments if fragment not in text]
        if missing:
            raise ValueError(f"{milestone} caller-owned host disclosure changed: {missing}")
        host[milestone] = {"status": "disclosed", "fragments": list(fragments), "count": len(fragments)}
    return {
        "schema": "semaprax.ri13.developer-friction-ledger.v2",
        "milestones": {
            name: {
                "authored_rust_files": entries,
                "authored_noncomment_lines": sum(entry["authored_noncomment_lines"] for entry in entries),
                "handwritten_abi_or_unsafe_escape_hatches": {"count": 0, "status": "refused"},
            }
            for name, entries in application.items()
        },
        "comparison_harnesses": comparisons,
        "caller_owned_host_configuration": host,
        "generated_code": {
            "status": "prepared_then_inventory_required",
            "locations": GENERATED_CODE_LOCATIONS,
            "measurement_receipt_field": "generated_code_inventory",
            "reason": "generated files are derived only by authenticated prepare; the measurement receipt records their SHA-256 values and byte counts after creation",
        },
        "limitations": [
            "line counts are a reproducible disclosure, not a usability or maintenance-quality score",
            "comparison harnesses include direct and handwritten routes plus allocator instrumentation; their escape-hatch token counts are disclosed separately from application ABI escape hatches",
            "the ledger does not count Cargo or dependency source, generated artifacts, or compiler internals",
            "caller-owned host configuration remains explicit and does not grant generated code ambient authority",
        ],
    }


def main():
    print(json.dumps(ledger(), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
