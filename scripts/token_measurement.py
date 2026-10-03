"""Offline exact-text token measurement shared by compact-projection tools.

The module deliberately accepts bytes, validates their UTF-8 boundary, and
uses only locally cached tiktoken assets.  It has no model-name lookup,
network fallback, billing, or telemetry surface.
"""

from __future__ import annotations

import hashlib
import json
import socket
from typing import Any


SUPPORTED_ENCODINGS = ("cl100k_base", "o200k_base")


class TokenizerUnavailable(RuntimeError):
    """The requested local tokenizer package or asset is unavailable."""


def sha256(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data).hexdigest()


def tokenizer_fingerprint(encoding: Any) -> str:
    ranks = getattr(encoding, "_mergeable_ranks", None)
    specials = getattr(encoding, "_special_tokens", None)
    pat_str = getattr(encoding, "_pat_str", None)
    if ranks is None or specials is None or pat_str is None:
        raise TokenizerUnavailable(f"encoding {encoding.name} does not expose stable rank tables")
    rows = [[key.hex(), int(value)] for key, value in sorted(ranks.items())]
    special_rows = [[key, int(value)] for key, value in sorted(specials.items())]
    payload = json.dumps([pat_str, rows, special_rows], separators=(",", ":"), ensure_ascii=True).encode()
    return sha256(payload)


def load_tokenizer(name: str) -> tuple[Any, dict[str, Any]]:
    """Load one admitted tokenizer while refusing every socket connection."""
    if name not in SUPPORTED_ENCODINGS:
        raise TokenizerUnavailable(
            f"unsupported measurement tokenizer `{name}`; supported: {', '.join(SUPPORTED_ENCODINGS)}"
        )
    original_socket = socket.socket

    class OfflineSocket(original_socket):
        def connect(self, address: Any) -> None:
            raise RuntimeError("network access is disabled for tokenizer loading")

        def connect_ex(self, address: Any) -> int:
            raise RuntimeError("network access is disabled for tokenizer loading")

    socket.socket = OfflineSocket
    try:
        try:
            import tiktoken
        except ImportError as error:
            raise TokenizerUnavailable("tiktoken is required and must already be installed") from error
        try:
            encoding = tiktoken.get_encoding(name)
        except Exception as error:
            raise TokenizerUnavailable(f"cached tokenizer asset unavailable for {name}: {error}") from error
        return encoding, {
            "name": name,
            "package_version": getattr(tiktoken, "__version__", "unknown"),
            "vocabulary_size": encoding.n_vocab,
            "vocabulary_fingerprint": tokenizer_fingerprint(encoding),
        }
    finally:
        socket.socket = original_socket


def measure_utf8(data: bytes, tokenizer_name: str) -> tuple[int, dict[str, Any]]:
    """Count exactly the supplied UTF-8 bytes with special tokens disabled."""
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ValueError("token measurement input must be UTF-8") from error
    encoding, metadata = load_tokenizer(tokenizer_name)
    return len(encoding.encode(text, disallowed_special=())), metadata


def tokenize_all(payloads: dict[str, bytes]) -> dict[str, dict[str, Any]]:
    """Preserve the benchmark's v1 output shape for both supported encodings."""
    result: dict[str, dict[str, Any]] = {}
    for name in SUPPORTED_ENCODINGS:
        encoding, metadata = load_tokenizer(name)
        tokens = {}
        for label, data in payloads.items():
            try:
                text = data.decode("utf-8")
            except UnicodeDecodeError as error:
                raise ValueError(f"payload `{label}` must be UTF-8 for token measurement") from error
            tokens[label] = len(encoding.encode(text, disallowed_special=()))
        result[name] = {
            "version": metadata["package_version"],
            "encoding": name,
            "vocab_size": metadata["vocabulary_size"],
            "vocab_fingerprint": metadata["vocabulary_fingerprint"],
            "tokens": tokens,
        }
    return result
