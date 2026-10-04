#!/usr/bin/env python3
# Fake named tokenizer speaking the harness_tokenize.py line protocol:
# one token per whitespace-separated word. Test double for plumbing only.
import json, sys
print(json.dumps({"name": "fake-words", "fingerprint": "sha256:fake"}), flush=True)
for line in sys.stdin:
    print(json.dumps({"tokens": len(json.loads(line)["text"].split())}), flush=True)
