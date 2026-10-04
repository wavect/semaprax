#!/usr/bin/env python3
"""Line-protocol tokenizer helper for the harness observation module.

Usage: harness_tokenize.py <cl100k_base|o200k_base>

Reuses scripts/token_measurement.py (local tiktoken cache only, sockets
refused while loading). Handshake line: {"name","fingerprint"} or {"error"}.
Then per request line {"text": ...} replies {"tokens": n}. No network, no
billing, no telemetry; text is counted and discarded.
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def main() -> int:
    out = sys.stdout
    try:
        from token_measurement import load_tokenizer

        encoding, meta = load_tokenizer(sys.argv[1] if len(sys.argv) > 1 else "")
    except Exception as error:  # unavailable tokenizer is a reportable state
        out.write(json.dumps({"error": str(error)[:300]}) + "\n")
        out.flush()
        return 3
    out.write(json.dumps({"name": meta["name"], "fingerprint": meta["vocabulary_fingerprint"]}) + "\n")
    out.flush()
    for line in sys.stdin:
        text = json.loads(line)["text"]
        out.write(json.dumps({"tokens": len(encoding.encode(text, disallowed_special=()))}) + "\n")
        out.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
