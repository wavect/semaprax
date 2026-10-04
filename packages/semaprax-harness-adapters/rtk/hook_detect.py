"""Read-only detection of an existing RTK hook from settings text the caller supplies.

The adapter never reads agent configuration itself (no ambient authority); a host
that was granted the file passes its text here. Formats are those written by
rtk 0.51.0 `rtk init` (see RESEARCH.md): Claude/Droid/Antigravity JSON with a
PreToolUse entry whose command is `rtk hook <agent>` or the legacy
`rtk-rewrite.sh`; Vibe TOML names `rtk hook vibe`.
"""
import json
import re

HOOK_COMMAND = re.compile(r"""(^|[\\/\s"'])rtk(\.exe)?\s+hook\s+[a-z]+\b|rtk-rewrite\.(sh|json)|rtk-hook-gemini\.sh""")


def _commands(node):
    if isinstance(node, dict):
        c = node.get("command")
        if isinstance(c, str):
            yield c
        for v in node.values():
            yield from _commands(v)
    elif isinstance(node, list):
        for v in node:
            yield from _commands(v)


def detect_rtk_hook(settings_text):
    """True when the JSON/TOML text registers an RTK rewrite hook."""
    try:
        return any(HOOK_COMMAND.search(c) for c in _commands(json.loads(settings_text)))
    except (ValueError, TypeError):
        return bool(HOOK_COMMAND.search(settings_text or ""))
