#!/usr/bin/env python3
"""Check handbook navigation and explicitly marked compiler-backed examples.

Python 3.10+. No third-party packages. Shell fences are never executed.
--structure-only deliberately skips compiler execution. Other runs require
an installed compiler, format temporary copies, and compare exact run output.
"""
from __future__ import annotations

import argparse
from collections import Counter
from dataclasses import dataclass
import json
import os
from pathlib import Path, PurePosixPath
import posixpath
import re
import shutil
import signal
import subprocess
import sys
import tempfile
from urllib.parse import unquote, urlsplit


class HandbookError(RuntimeError):
    """An invalid document, unsupported marker, or failed smoke check."""


@dataclass(frozen=True)
class Block:
    page: str
    line: int
    language: str
    source: str
    kind: str | None
    metadata: dict


MARKER = re.compile(r"^<!-- handbook-(smoke|project-file):\s*(.*?)\s*-->$")
FENCE = re.compile(r"^\s*(`{3,}|~{3,})(.*)$")
LINK = re.compile(r"\]\(([^\n)]+)\)")
HTML_LINK = re.compile(r"\b(?:src|href)=[\"']([^\"']+)[\"']", re.IGNORECASE)
MAX_OUTPUT = 2 * 1024 * 1024


def scan(page: str, source: str) -> tuple[str, list[Block]]:
    """Return prose and code blocks, checking closed fences and marker syntax."""
    prose: list[str] = []
    blocks: list[Block] = []
    pending: tuple[str, dict] | None = None
    opened: tuple[str, int, str, int] | None = None
    lines: list[str] = []
    attached: tuple[str, dict] | None = None
    for number, line in enumerate(source.splitlines(), 1):
        fence = FENCE.match(line)
        if opened is not None:
            char, width, language, start = opened
            if fence and fence[1][0] == char and len(fence[1]) >= width and not fence[2].strip():
                kind, metadata = attached or (None, {})
                blocks.append(Block(page, start, language, "\n".join(lines) + "\n", kind, metadata))
                opened, attached, lines = None, None, []
            else:
                lines.append(line)
            continue
        marker = MARKER.match(line.strip())
        if marker:
            if pending:
                raise HandbookError(f"{page}:{number}: consecutive example markers")
            try:
                metadata = json.loads(marker[2])
            except json.JSONDecodeError as error:
                raise HandbookError(f"{page}:{number}: invalid marker JSON: {error.msg}") from error
            if not isinstance(metadata, dict):
                raise HandbookError(f"{page}:{number}: marker must contain a JSON object")
            pending = marker[1], metadata
        elif fence:
            opened = fence[1][0], len(fence[1]), fence[2].strip(), number + 1
            attached, pending = pending, None
        else:
            if pending and line.strip():
                raise HandbookError(f"{page}:{number}: example marker must directly precede its code fence")
            prose.append(line)
    if opened:
        raise HandbookError(f"{page}:{opened[3]}: unclosed code fence")
    if pending:
        raise HandbookError(f"{page}: example marker has no code block")
    return "\n".join(prose), blocks


def targets(prose: str) -> list[str]:
    return [m[1].strip() for m in LINK.finditer(prose)] + HTML_LINK.findall(prose)


def local_target(page: str, raw: str) -> str | None:
    # Optional Markdown titles follow the destination. Angle-bracket URLs
    # can contain spaces; normal repository paths use percent-encoding.
    if not raw.strip():
        return None
    destination = raw[1:raw.index(">")] if raw.startswith("<") and ">" in raw else raw.split()[0]
    url = urlsplit(destination)
    if url.scheme or url.netloc or not url.path:
        return None
    path = unquote(url.path)
    resolved = posixpath.normpath(posixpath.join(posixpath.dirname(page), path))
    if path.startswith("/") or resolved == ".." or resolved.startswith("../"):
        raise HandbookError(f"{page}: local link leaves the repository: {raw}")
    return resolved


def validate_documents(
    documents: dict[str, str], existing_paths: set[str], chapters: set[str]
) -> list[Block]:
    """Pure structural check. The CLI derives both inventories from disk."""
    summary = "handbook/SUMMARY.md"
    if summary not in documents:
        raise HandbookError("handbook/SUMMARY.md is missing")
    blocks: list[Block] = []
    navigation: Counter[str] = Counter()
    for page, source in sorted(documents.items()):
        if not source.startswith("# "):
            raise HandbookError(f"{page}: expected an H1 title on the first line")
        prose, extracted = scan(page, source)
        blocks.extend(extracted)
        for raw in targets(prose):
            target = local_target(page, raw)
            if target is None:
                continue
            if target not in existing_paths:
                raise HandbookError(f"{page}: missing local target {raw}")
            if page == summary and target.endswith(".md"):
                navigation[target] += 1
    for chapter in sorted(chapters):
        if navigation[chapter] != 1:
            raise HandbookError(f"{chapter}: expected exactly one SUMMARY entry, found {navigation[chapter]}")
    for target in navigation:
        if target not in chapters:
            raise HandbookError(f"SUMMARY entry is not a handbook chapter: {target}")
    collect_examples(blocks)  # Validate marker shape even in structural-only runs.
    return blocks


def collect_examples(blocks: list[Block]) -> tuple[list[Block], dict[str, list[Block]]]:
    standalone: list[Block] = []
    projects: dict[str, list[Block]] = {}
    for block in blocks:
        data = block.metadata
        label = f"{block.page}:{block.line}"
        if block.kind is None:
            continue
        if block.kind == "smoke":
            if block.language != "semaprax" or set(data) != {"stdout"} or not isinstance(data["stdout"], str):
                raise HandbookError(f"{label}: smoke requires a semaprax block and a stdout string")
            standalone.append(block)
            continue
        if block.kind != "project-file" or set(data) - {"group", "path", "stdout", "test"}:
            raise HandbookError(f"{label}: unsupported project marker fields")
        group, name = data.get("group"), data.get("path")
        if not isinstance(group, str) or not re.fullmatch(r"[a-z0-9_-]+", group):
            raise HandbookError(f"{label}: invalid project group")
        if not isinstance(name, str) or not name or "\\" in name:
            raise HandbookError(f"{label}: invalid project path")
        path = PurePosixPath(name)
        if path.is_absolute() or ".." in path.parts or str(path) != name:
            raise HandbookError(f"{label}: project path must be normalized and relative")
        expected_language = "toml" if name == "semaprax.toml" else "semaprax"
        if (name != "semaprax.toml" and path.suffix != ".spx") or block.language != expected_language:
            raise HandbookError(f"{label}: project file extension and code language disagree")
        if name == "semaprax.toml":
            if not isinstance(data.get("stdout"), str) or type(data.get("test")) is not bool:
                raise HandbookError(f"{label}: project manifest must declare stdout and test")
        elif "stdout" in data or "test" in data:
            raise HandbookError(f"{label}: project execution settings belong on its manifest")
        if any(other.metadata["path"] == name for other in projects.get(group, [])):
            raise HandbookError(f"{label}: duplicate project file {group}/{name}")
        projects.setdefault(group, []).append(block)
    for group, files in projects.items():
        if not any(block.metadata["path"] == "semaprax.toml" for block in files):
            raise HandbookError(f"project group {group}: missing semaprax.toml")
    return standalone, projects


def invoke(compiler: Path, arguments: list[str], cwd: Path, timeout: float) -> str:
    """Invoke only the selected compiler; bound time and retained output."""
    with tempfile.TemporaryFile() as out, tempfile.TemporaryFile() as err:
        process = subprocess.Popen(
            [str(compiler), *arguments], cwd=cwd, stdin=subprocess.DEVNULL,
            stdout=out, stderr=err, start_new_session=os.name == "posix",
        )
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired as error:
            if os.name == "posix":
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
            else:
                process.kill()
            process.wait()
            raise HandbookError(f"compiler command {arguments[0]} timed out") from error
        for stream in (out, err):
            if stream.tell() > MAX_OUTPUT:
                raise HandbookError(f"compiler command {arguments[0]} exceeded the output limit")
            stream.seek(0)
        stdout = out.read().decode("utf-8", "strict")
        stderr = err.read().decode("utf-8", "replace")
        if process.returncode:
            detail = (stderr or stdout)[:6000]
            raise HandbookError(f"compiler command {arguments[0]} failed ({process.returncode}):\n{detail}")
        return stdout


def run_examples(blocks: list[Block], compiler: Path, timeout: float) -> tuple[int, int]:
    standalone, projects = collect_examples(blocks)
    if not standalone or not projects:
        raise HandbookError("expected both standalone and project smoke subjects; refusing an empty gate")
    # macOS commonly exposes its temporary directory through /var -> /private/var.
    # The compiler's held-path admission rejects symlink ancestors, so create
    # the fixture under the resolved directory.
    with tempfile.TemporaryDirectory(
        prefix="semaprax-handbook-", dir=Path(tempfile.gettempdir()).resolve()
    ) as temporary:
        root = Path(temporary)
        for index, block in enumerate(standalone):
            path = root / f"example-{index}.spx"
            path.write_text(block.source, encoding="utf-8")
            invoke(compiler, ["fmt", str(path)], root, timeout)
            invoke(compiler, ["check", str(path)], root, timeout)
            actual = invoke(compiler, ["run", str(path), "--max-steps", "100000", "--max-bytes", "65536"], root, timeout)
            if actual != block.metadata["stdout"]:
                raise HandbookError(f"{block.page}:{block.line}: expected {block.metadata['stdout']!r}, got {actual!r}")
        for group, files in sorted(projects.items()):
            directory = root / group
            directory.mkdir()
            for block in files:
                path = directory / block.metadata["path"]
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(block.source, encoding="utf-8")
            manifest = next(block for block in files if block.metadata["path"] == "semaprax.toml")
            invoke(compiler, ["fmt", "semaprax.toml"], directory, timeout)
            invoke(compiler, ["check", "semaprax.toml"], directory, timeout)
            if manifest.metadata["test"]:
                invoke(compiler, ["test", "semaprax.toml"], directory, timeout)
            actual = invoke(compiler, ["run", "semaprax.toml"], directory, timeout)
            if actual != manifest.metadata["stdout"]:
                raise HandbookError(f"project {group}: expected {manifest.metadata['stdout']!r}, got {actual!r}")
    return len(standalone), len(projects)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--structure-only", action="store_true")
    mode.add_argument("--compiler", type=Path, help="explicit path to an installed Semaprax executable")
    parser.add_argument("--timeout", type=float, default=30.0, help="seconds allowed per compiler command")
    args = parser.parse_args()
    if not 0 < args.timeout <= 300:
        parser.error("--timeout must be positive and at most 300 seconds")
    root = Path(__file__).resolve().parents[1]
    files = sorted((root / "handbook").rglob("*.md"))
    documents = {
        path.relative_to(root).as_posix(): path.read_text(encoding="utf-8")
        for path in files if "assets" not in path.relative_to(root / "handbook").parts
    }
    chapters = set(documents) - {"handbook/SUMMARY.md"}
    paths = set(documents)
    # Only inspect destinations named by the handbook; avoid walking target/
    # or another potentially large/unrelated part of the repository.
    for page, text in documents.items():
        prose, _ = scan(page, text)
        for target in targets(prose):
            destination = local_target(page, target)
            if destination is not None:
                resolved = (root / destination).resolve()
                if not resolved.is_relative_to(root):
                    raise HandbookError(f"{page}: linked symlink leaves the repository: {target}")
                if resolved.exists():
                    paths.add(destination)
    blocks = validate_documents(documents, paths, chapters)
    if args.structure_only:
        print(f"handbook: {len(chapters)} chapters; structure passed; compiler execution SKIPPED")
        return 0
    selected = str(args.compiler) if args.compiler is not None else shutil.which("semaprax")
    if not selected:
        raise HandbookError("Semaprax is unavailable; provide --compiler or explicitly request --structure-only")
    compiler = Path(selected).expanduser().resolve()
    if not compiler.is_file() or not os.access(compiler, os.X_OK):
        raise HandbookError(f"not an executable compiler: {compiler}")
    standalone, projects = run_examples(blocks, compiler, args.timeout)
    print(f"handbook: structure passed; {standalone} standalone and {projects} project smoke checks passed")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (HandbookError, OSError, UnicodeError) as error:
        print(f"handbook check failed: {error}", file=sys.stderr)
        raise SystemExit(1)
