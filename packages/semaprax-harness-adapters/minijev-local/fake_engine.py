"""Counting fake engine: deterministic closed-choice scores, no model, no torch, no network.

TEST/DEMO ONLY. Its identity says engine "fake" and the adapter refuses it unless
SEMAPRAX_HARNESS_MINIJEV_ALLOW_FAKE=1. Every method other than `score_closed_choice`
exists only to count (and fail) an illegitimate call: a routing decision must be
exactly one scoring execution and zero generate/decode/baseline calls.
"""

import hashlib
import threading
import time

FORBIDDEN = ("generate", "decode", "run_json", "run_split", "run_labels", "sequence_logprobs", "full_logits_from_hidden")


class FakeEngine:
    def __init__(self, identity=None, delay=0.0, scripted=None, max_prompt_tokens=2048):
        self.identity = dict(identity or {
            "engine": "fake", "model": "fake-counting-engine", "revision": "1" * 40, "tokenizer_sha": "2" * 64,
            "code_commit": "ca612198bfb69f538f029a4615f6d0a18b4f814c", "system_sha": "3" * 64, "letters_sha": "4" * 64,
            "dtype": "none", "device": "none"})
        self.delay, self.scripted, self.max_prompt_tokens = delay, scripted, max_prompt_tokens
        self.counts = {"score": 0, **{n: 0 for n in FORBIDDEN}}
        self.users = []
        self.lock = threading.Lock()
        self.release = threading.Event()   # tests may hold a scoring call open

    def _forbidden(self, name):
        self.counts[name] += 1
        raise AssertionError(f"{name} must never be called for a routing decision")

    def __getattr__(self, name):
        if name in FORBIDDEN:
            return lambda *a, **k: self._forbidden(name)
        raise AttributeError(name)

    def score_closed_choice(self, user, k):
        with self.lock:
            self.counts["score"] += 1
            self.users.append((user, k))
        if self.delay:
            self.release.wait(self.delay)
        if self.scripted is not None:
            return self.scripted(user, k)
        ntok = len(user.split())
        if ntok > self.max_prompt_tokens:
            raise ValueError("too_long")
        h = hashlib.sha256(user.encode()).digest()
        logits = [round(((h[i] / 255.0) * 8.0) - 4.0, 6) for i in range(k)]
        return finish(logits, ntok)


def finish(logits, prompt_tokens):
    """The same derived fields the real engine returns (softmax, argmax-lowest, tie flag)."""
    import math
    m = max(logits)
    es = [math.exp(x - m) for x in logits]
    s = sum(es)
    srt = sorted(logits, reverse=True)
    return {"logits": logits, "p_cand": [round(e / s, 6) for e in es], "pred_pos": logits.index(m),
            "tie": logits.count(m) > 1, "gap": round(srt[0] - srt[1], 6), "prompt_tokens": prompt_tokens}
