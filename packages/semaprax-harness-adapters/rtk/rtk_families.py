"""Command-family allowlist for the RTK command.view adapter (pinned rtk 0.51.0).

Every family here was measured against the real binary (see RESEARCH.md and
test/). Anything not matched is bypassed; there is no prefix or fuzzy matching.
"""

import os
import re

PINNED_VERSION = "0.51.0"
SHELL_TOKENS = {"|", "||", "&&", ";", "&", ">", ">>", "<", "<<", "2>&1", "2>"}
# Flags that change output into something another tool may parse exactly.
MACHINE = re.compile(
    r"^(--json\b|--format\b|--message-format\b|--porcelain\b|--name-only$|--name-status$|--numstat$|"
    r"--shortstat$|--raw$|--pretty\b|--output\b|--null$|--exit-code$|-z$|-0$|--junit|"
    r"--report|-print0$|-exec$|-delete$|-ls$|-printf$|--files(-with-matches|-without-match)?$|--count$|--only-matching$)"
)
# Families whose pipe filter also needs stderr (compiler/test runners print on both).
MERGES_STDERR = {"cargo-test", "pytest"}


class Bypass(Exception):
    def __init__(self, reason):
        super().__init__(reason)
        self.reason = reason


def _bare(argv0):
    return os.path.basename(argv0)


def _paths_after(args):
    """Optional `--` then plain paths; a leading-dash token is not a path."""
    rest = args
    if rest and rest[0] == "--":
        rest = rest[1:]
    if any(a.startswith("-") for a in rest):
        raise Bypass("unverified-flag")
    return rest


def classify(argv):
    """Return {family, filter, wrapper} for an allowlisted argv, else raise Bypass(reason).

    family: stable name. filter: `rtk pipe -f` filter (None when RTK offers no
    stdin filter for the family). wrapper: whether `rtk <argv...>` is a tested
    wrapper mapping.
    """
    if not isinstance(argv, list) or not argv or not all(isinstance(a, str) and a and "\0" not in a for a in argv):
        raise Bypass("malformed-argv")
    name = _bare(argv[0])
    if name == "rtk":
        raise Bypass("already-wrapped")
    if "=" in argv[0] or any(a in SHELL_TOKENS for a in argv):
        raise Bypass("shell-syntax")
    if name.startswith("semaprax"):
        raise Bypass("authoritative-envelope")
    args = argv[1:]
    if any(MACHINE.match(a) for a in args):
        raise Bypass("machine-output-flag")

    if name == "git":
        if not args or args[0].startswith("-"):
            raise Bypass("unverified-flag")
        sub, rest = args[0], args[1:]
        if sub == "status" and not rest:
            return {"family": "git-status", "filter": None, "wrapper": True}  # pipe filter expects porcelain; ~0% on human output
        if sub == "diff":
            i = 0
            while i < len(rest) and rest[i] in ("--cached", "--staged"):
                i += 1
            _paths_after(rest[i:])
            return {"family": "git-diff", "filter": "git-diff", "wrapper": True}
        if sub == "log":
            if rest == [] or (len(rest) == 1 and re.fullmatch(r"-\d+|--max-count=\d+", rest[0])) or (
                len(rest) == 2 and rest[0] == "-n" and rest[1].isdigit()
            ):
                return {"family": "git-log", "filter": "git-log", "wrapper": True}
            raise Bypass("unverified-flag")
        raise Bypass("unsupported-command")
    if name == "rg":
        if _has_line_numbers(args):
            return {"family": "rg", "filter": "rg", "wrapper": True}
        raise Bypass("unparsable-output-shape")
    if name == "grep":
        if _has_line_numbers(args) and any(re.fullmatch(r"-[A-Za-z]*r[A-Za-z]*|--recursive", a) for a in args):
            return {"family": "grep", "filter": "grep", "wrapper": False}
        raise Bypass("unparsable-output-shape")
    if name == "find":
        _check_find(args)
        return {"family": "find", "filter": "find", "wrapper": False}  # rtk find swallows errors: exit 0 where find exits 1
    if name == "ls":
        if all(re.fullmatch(r"-[laAhR1]+", a) or not a.startswith("-") for a in args):
            return {"family": "ls", "filter": None, "wrapper": True}
        raise Bypass("unverified-flag")
    if name == "cargo":
        if len(args) >= 1 and args[0] == "test" and not any("format" in a or "json" in a for a in args):
            return {"family": "cargo-test", "filter": "cargo-test", "wrapper": True}
        raise Bypass("unsupported-command")
    if name in ("pytest", "py.test"):
        return {"family": "pytest", "filter": "pytest", "wrapper": True}
    raise Bypass("unsupported-command")


def _has_line_numbers(args):
    # -c/-l/-L/-o/-q/-Z change the record shape; refuse them even inside a bundle like -rnl.
    for a in args:
        if re.fullmatch(r"-[A-Za-z]+", a) and re.search(r"[cLloqZ]", a[1:]):
            return False
    for a in args:
        if a == "--line-number" or re.fullmatch(r"-[A-Za-z]*n[A-Za-z]*", a):
            return True
    return False


def _check_find(args):
    value_flags = {"-name", "-iname", "-type", "-path", "-maxdepth", "-mindepth"}
    i = 0
    while i < len(args) and not args[i].startswith("-"):
        i += 1
    while i < len(args):
        if args[i] not in value_flags or i + 1 >= len(args):
            raise Bypass("unverified-flag")
        i += 2
