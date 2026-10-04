#!/usr/bin/env python3
"""Graphify context.repository/v1 adapter for the SEMAPRAX harness host.

Standard library only (plus the shared wire helper). Builds a local code-only
graph with the pinned Graphify under the host cache dir and answers from
graph.json directly. See README.md for the trust and coverage rules.
"""

import glob
import hashlib
import json
import os
import re
import subprocess
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "sdk", "python"))
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

PROVIDER_ID = "com.graphify-labs/graphify-context"
ADAPTER_VERSION = "0.1.0"
TESTED_VERSIONS = ("0.9.25",)
KIND = "context.repository"
OPERATIONS = ["orient", "search", "skeleton", "references"]
CONFIDENCES = {"EXTRACTED", "INFERRED", "AMBIGUOUS"}
STRUCTURAL_RELATIONS = {"contains", "method"}
SKIP_DIRS = {
    "venv", "node_modules", "__pycache__", "dist", "build", "target", "out", "site-packages",
    "graphify-out", "coverage", "storybook-static",
}
DOC_EXT = {".md", ".markdown", ".txt", ".rst", ".adoc", ".pdf", ".docx", ".xlsx", ".html", ".htm"}
MEDIA_EXT = {".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".mp3", ".mp4", ".wav", ".mov", ".webm"}
MANIFESTS = {"Cargo.toml", "package.json", "pyproject.toml", "go.mod", "pom.xml", "Gemfile"}
LANGS = {
    ".py": "python", ".ts": "typescript", ".tsx": "typescript", ".js": "javascript", ".jsx": "javascript",
    ".mjs": "javascript", ".cjs": "javascript", ".go": "go", ".rs": "rust", ".java": "java",
    ".c": "c", ".h": "c", ".cpp": "cpp", ".hpp": "cpp", ".rb": "ruby", ".sh": "shell", ".json": "json",
    ".kt": "kotlin", ".swift": "swift", ".cs": "csharp", ".php": "php", ".lua": "lua", ".spx": "semaprax",
}
# Environment is an allowlist, not a denylist: nothing else reaches graphify.
ENV_ALLOW = ("LANG", "LC_ALL", "LC_CTYPE", "TMPDIR")
BUILD_TIMEOUT_S = 110
MAX_SKIPPED = 200


def env_var(name):
    value = os.environ.get(name)
    if not value:
        raise AdapterError("unavailable", "SPX-HPG005", f"host did not provide {name}")
    return value


def child_env(cache_dir):
    home = os.path.join(cache_dir, "home")
    os.makedirs(home, exist_ok=True)
    env = {k: os.environ[k] for k in ENV_ALLOW if k in os.environ}
    env.update(PATH="/usr/bin:/bin", HOME=home, PYTHONDONTWRITEBYTECODE="1", GRAPHIFY_NO_TIPS="1")
    return env


def verify_identity(upstream):
    """Identity comes from the installed distribution metadata, not the name."""
    real = os.path.realpath(upstream)
    root = os.path.dirname(os.path.dirname(real))
    metas = glob.glob(os.path.join(root, "lib", "python*", "site-packages", "graphifyy-*.dist-info", "METADATA"))
    if len(metas) != 1:
        raise AdapterError("unsupported", "SPX-HPG001", "no unique graphifyy distribution metadata beside the upstream executable")
    fields = {}
    with open(metas[0], encoding="utf-8") as fh:
        for line in fh:
            if not line.strip():
                break
            key, _, value = line.partition(":")
            fields.setdefault(key.strip(), value.strip())
    repo_ok = "github.com/Graphify-Labs/graphify" in open(metas[0], encoding="utf-8").read()
    if fields.get("Name") != "graphifyy" or not repo_ok:
        raise AdapterError("unsupported", "SPX-HPG001", "upstream distribution is not graphifyy from Graphify-Labs/graphify")
    version = fields.get("Version")
    if version not in TESTED_VERSIONS:
        raise AdapterError("unsupported", "SPX-HPG001", f"graphifyy {version} is not a tested version {list(TESTED_VERSIONS)}")
    return version


def walk_files(root):
    """Sorted relative paths of non-hidden, non-generated files."""
    found = []
    for base, dirs, files in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in SKIP_DIRS and not d.startswith("."))
        for name in files:
            if not name.startswith("."):
                found.append(os.path.relpath(os.path.join(base, name), root).replace(os.sep, "/"))
    return sorted(found)


def file_sha(path):
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def source_digest(root, files):
    h = hashlib.sha256(b"semaprax.graphify-source-set.v1\0")
    for rel in files:
        try:
            h.update(f"{rel}\0{file_sha(os.path.join(root, rel))}\n".encode())
        except OSError:
            h.update(f"{rel}\0unreadable\n".encode())
    return h.hexdigest()


def skip_reason(rel):
    name, ext = os.path.basename(rel), os.path.splitext(rel)[1].lower()
    if ext == ".spx":
        return "semaprax source: not extracted by graphify; compiler-owned facts come from the native context"
    if ext == ".spatch":
        return "semaprax semantic patch: not extracted by graphify"
    if name in MANIFESTS:
        return "package manifest: not indexed in code-only mode"
    if ext in DOC_EXT or ext in MEDIA_EXT:
        return "document or media: skipped by --code-only (no model-backed ingestion)"
    return "not indexed by graphify (unsupported language, ignored or excluded)"


class Graph:
    """Validated view of a pinned-schema graphify graph.json."""

    def __init__(self, raw):
        if not isinstance(raw, dict) or not isinstance(raw.get("nodes"), list) or not isinstance(raw.get("links"), list):
            raise AdapterError("unsupported", "SPX-HPG003", "graph.json lacks the 0.9.25 'nodes'/'links' lists")
        self.nodes, self.edges, self.by_id = [], [], {}
        for n in raw["nodes"]:
            if not isinstance(n, dict) or not all(isinstance(n.get(k), str) for k in ("id", "label", "file_type", "source_file")):
                raise AdapterError("unsupported", "SPX-HPG003", "graph node violates the pinned schema")
            src = n["source_file"]
            if not src:  # unresolved/external stub (e.g. a std type): no project location
                n["_stub"] = True
                self.by_id[n["id"]] = n
                continue
            if src.startswith("/") or ".." in src.split("/"):
                raise AdapterError("unsupported", "SPX-HPG003", "graph node path is not project-relative")
            loc = n.get("source_location", "L1")
            m = re.fullmatch(r"L(\d+)", loc) if isinstance(loc, str) else None
            if not m:
                raise AdapterError("unsupported", "SPX-HPG003", f"unrecognised source_location {loc!r}")
            n["_line"] = max(1, int(m.group(1)))
            self.nodes.append(n)
            self.by_id[n["id"]] = n
        for e in raw["links"]:
            if not isinstance(e, dict) or e.get("confidence") not in CONFIDENCES or not all(
                isinstance(e.get(k), str) for k in ("source", "target", "relation", "source_file")
            ):
                raise AdapterError("unsupported", "SPX-HPG003", "graph edge violates the pinned schema (keys or confidence)")
            loc = e.get("source_location", "L1")
            m = re.fullmatch(r"L(\d+)", loc) if isinstance(loc, str) else None
            e["_line"] = int(m.group(1)) if m else 1
            if e["source"] in self.by_id and e["target"] in self.by_id:
                self.edges.append(e)
        self.degree = {}
        for e in self.edges:
            for k in ("source", "target"):
                self.degree[e[k]] = self.degree.get(e[k], 0) + 1
        self.files = sorted({n["source_file"] for n in self.nodes})


class Index:
    def __init__(self, root, cache_dir):
        self.root, self.cache_dir = root, cache_dir
        self.out = os.path.join(cache_dir, "graphify-index")
        self.graph_path = os.path.join(self.out, "graphify-out", "graph.json")
        self.meta_path = os.path.join(self.out, "adapter-meta.json")
        self.identity = None
        self.state = None  # dict: digest, graph, coverage, errors
        self._lines = {}

    # -- upstream -----------------------------------------------------
    def check_identity(self):
        if self.identity is None:
            upstream = env_var("SEMAPRAX_HARNESS_UPSTREAM")
            if not os.path.isabs(upstream) or not os.path.isfile(upstream):
                raise AdapterError("unavailable", "SPX-HPG005", "SEMAPRAX_HARNESS_UPSTREAM must be an absolute executable path")
            self.upstream, self.identity = upstream, verify_identity(upstream)
        return self.identity

    def build(self, digest, files):
        os.makedirs(self.out, exist_ok=True)
        cmd = [self.upstream, "extract", self.root, "--code-only", "--out", self.out]
        try:
            proc = subprocess.run(cmd, cwd=self.cache_dir, env=child_env(self.cache_dir), stdin=subprocess.DEVNULL,
                                  capture_output=True, text=True, timeout=BUILD_TIMEOUT_S)
        except subprocess.TimeoutExpired:
            raise AdapterError("failed", "SPX-HPG004", "graphify extraction timed out")
        log = (proc.stdout + proc.stderr).splitlines()
        if proc.returncode != 0 or not os.path.isfile(self.graph_path):
            raise AdapterError("failed", "SPX-HPG004", f"graphify extract exited {proc.returncode}: {' | '.join(log[-3:])[:300]}")
        errors = [ln.strip()[:300] for ln in log if re.search(r"error|failed|traceback|permission", ln, re.I)]
        with open(self.meta_path, "w", encoding="utf-8") as fh:
            json.dump({"schema": "semaprax.graphify-adapter-meta.v1", "upstream": self.identity, "root": self.root,
                       "digest": digest, "errors": errors}, fh, sort_keys=True)
        return self.load(digest, files, errors)

    def load(self, digest, files, errors):
        with open(self.graph_path, encoding="utf-8") as fh:
            graph = Graph(json.load(fh))
        manifest = os.path.join(self.out, "graphify-out", "manifest.json")
        indexed = set(graph.files)
        if os.path.isfile(manifest):
            with open(manifest, encoding="utf-8") as fh:
                indexed |= {k for k in json.load(fh) if isinstance(k, str)}
        indexed &= set(files)
        skipped = [{"path": f, "reason": skip_reason(f)} for f in files if f not in indexed]
        self.state = {"digest": digest, "graph": graph, "indexed": sorted(indexed), "skipped": skipped, "errors": errors}
        self._lines = {}
        return self.state

    def disk_meta(self):
        try:
            with open(self.meta_path, encoding="utf-8") as fh:
                meta = json.load(fh)
            ok = meta.get("upstream") == self.identity and meta.get("root") == self.root and os.path.isfile(self.graph_path)
            return meta if ok else None
        except (OSError, ValueError):
            return None

    def ensure(self, refresh):
        """Return (state, stale_flag)."""
        self.check_identity()
        files = walk_files(self.root)
        digest = source_digest(self.root, files)
        if self.state is None:
            meta = self.disk_meta()
            if meta and meta.get("digest") == digest:
                return self.load(digest, files, meta.get("errors", [])), False
            return self.build(digest, files), False
        if self.state["digest"] != digest:
            if refresh == "rebuild":
                return self.build(digest, files), False
            return self.state, True
        return self.state, False

    # -- source lines --------------------------------------------------
    def line_digest(self, rel, line):
        lines = self._lines.get(rel)
        if lines is None:
            try:
                with open(os.path.join(self.root, rel), "rb") as fh:
                    lines = fh.read().split(b"\n")
            except OSError:
                lines = []
            self._lines[rel] = lines
        text = lines[line - 1] if 0 < line <= len(lines) else b""
        return hashlib.sha256(text).hexdigest()


def language(rel):
    return LANGS.get(os.path.splitext(rel)[1].lower(), "unknown")


def item(index, rel, line, provenance, rank, text):
    return {"path": rel, "span": {"start_line": line, "end_line": line}, "digest": index.line_digest(rel, line),
            "provenance": provenance, "language": language(rel), "rank": rank, "text": text}


def coverage(state, exhaustive):
    skipped = state["skipped"][:MAX_SKIPPED]
    complete = not state["skipped"] and not state["errors"]
    return {"complete": complete, "indexed_files": len(state["indexed"]), "skipped": skipped,
            "exhaustive": bool(exhaustive and complete), "extraction_errors": state["errors"]}


def node_text(n):
    return f"{n['label']} ({n['file_type']})"


def clean(label):
    return label.strip().lstrip(".").removesuffix("()").lower()


def finish(request, index, state, items, exhaustive, diags=()):
    cov = coverage(state, exhaustive)
    budget = request.get("budget", {}).get("max_result_bytes", 65536)
    limit = int(budget * 0.8)
    diags = list(diags)
    while items and len(json.dumps({"items": items, "coverage": cov})) > limit:
        items = items[: max(0, len(items) // 2)]
        cov = dict(cov, complete=False, exhaustive=False)
        diags.append({"code": "SPX-HPG007", "message": "result truncated to fit the byte budget"})
    if len(state["skipped"]) > MAX_SKIPPED:
        diags.append({"code": "SPX-HPG008", "message": f"skipped list truncated to {MAX_SKIPPED} of {len(state['skipped'])}"})
    status = "complete" if cov["complete"] else "partial"
    return status, {"items": items, "coverage": cov}, diags


def make_handlers(index):
    def prelude(request, op):
        payload = request.get("payload") or {}
        state, stale = index.ensure(payload.get("refresh", "stale"))
        if stale:
            cov = coverage(state, False)
            cov.update(complete=False, exhaustive=False)
            return None, payload, ("stale", {"items": [], "coverage": cov}, [
                {"code": "SPX-HPG002", "message": "source files changed since the graph was built; resend with payload.refresh=rebuild"}])
        return state, payload, None

    def limit_of(payload):
        v = payload.get("limit", 20)
        return v if isinstance(v, int) and 0 < v <= 200 else 20

    def orient(request):
        state, payload, early = prelude(request, "orient")
        if early:
            return early
        g = state["graph"]
        hubs = sorted((n for n in g.nodes if n["file_type"] == "code" and g.degree.get(n["id"])),
                      key=lambda n: (-g.degree[n["id"]], n["source_file"], n["_line"], n["id"]))
        items = [item(index, n["source_file"], n["_line"], "structural", i + 1,
                      f"{node_text(n)} degree={g.degree[n['id']]}") for i, n in enumerate(hubs[: limit_of(payload)])]
        return finish(request, index, state, items, False)

    def search(request):
        state, payload, early = prelude(request, "search")
        if early:
            return early
        q = payload.get("query")
        if not isinstance(q, str) or not q.strip():
            raise AdapterError("refused", "SPX-HPG006", "search requires payload.query")
        terms = [t for t in re.split(r"\W+", q.lower()) if t]
        g, scored = state["graph"], []
        for n in g.nodes:
            label, path = clean(n["label"]), n["source_file"].lower()
            score = 0
            for t in terms:
                score += 100 if label == t else 40 if label.startswith(t) else 20 if t in label else 5 if t in path else 0
            if score:
                scored.append((-score, -g.degree.get(n["id"], 0), n["source_file"], n["_line"], n["id"], n))
        scored.sort(key=lambda s: s[:5])
        items = [item(index, n["source_file"], n["_line"], "structural", i + 1, node_text(n))
                 for i, (*_, n) in enumerate(scored[: limit_of(payload)])]
        return finish(request, index, state, items, False)

    def skeleton(request):
        state, payload, early = prelude(request, "skeleton")
        if early:
            return early
        rel = payload.get("path")
        if not isinstance(rel, str) or rel.startswith("/") or ".." in rel.split("/"):
            raise AdapterError("refused", "SPX-HPG006", "skeleton requires a project-relative payload.path")
        if rel not in state["indexed"]:
            cov = coverage(state, False)
            reason = next((s["reason"] for s in state["skipped"] if s["path"] == rel), "file not found in project")
            return "unsupported", {"items": [], "coverage": cov}, [{"code": "SPX-HPG009", "message": f"{rel}: {reason}"}]
        nodes = sorted((n for n in state["graph"].nodes if n["source_file"] == rel), key=lambda n: (n["_line"], n["id"]))
        items = [item(index, rel, n["_line"], "structural", i + 1, node_text(n)) for i, n in enumerate(nodes)]
        st, out, diags = finish(request, index, state, items, True)
        # skeleton exhaustiveness is per file: the file itself is indexed
        out["coverage"]["exhaustive"] = len(out["items"]) == len(nodes)
        return st, out, diags

    def references(request):
        state, payload, early = prelude(request, "references")
        if early:
            return early
        sym = payload.get("symbol")
        if not isinstance(sym, str) or not sym.strip():
            raise AdapterError("refused", "SPX-HPG006", "references requires payload.symbol")
        g = state["graph"]
        targets = {n["id"] for n in list(g.nodes) + [x for x in g.by_id.values() if x.get("_stub")] if n["id"] == sym or clean(n["label"]) == clean(sym)}
        edges = [e for e in g.edges if e["target"] in targets and e["relation"] not in STRUCTURAL_RELATIONS]
        edges.sort(key=lambda e: (e["source_file"], e["_line"], e["source"], e["relation"]))
        items = []
        for i, e in enumerate(edges[: limit_of(payload)]):
            src, dst = g.by_id[e["source"]], g.by_id[e["target"]]
            # a stub target was matched by name only, so it is never structural
            prov = "structural" if e["confidence"] == "EXTRACTED" and not dst.get("_stub") else "inferred"
            items.append(item(index, e["source_file"], e["_line"], prov, i + 1,
                              f"{src['label']} -{e['relation']}-> {dst['label']} [{e['confidence']}{', unresolved target' if dst.get('_stub') else ''}]"))
        diags = []
        if not targets:
            diags.append({"code": "SPX-HPG010", "message": "symbol not found in the graph; this is not evidence it is unused"})
        elif not items:
            diags.append({"code": "SPX-HPG010", "message": "no recorded edges to the symbol; graphify edges are name-resolved, so absence is not proof of no callers"})
        return finish(request, index, state, items, bool(targets), diags)

    return {(KIND, "orient"): orient, (KIND, "search"): search, (KIND, "skeleton"): skeleton, (KIND, "references"): references}


def main():
    root = os.environ.get("SEMAPRAX_HARNESS_PROJECT_ROOT", "")
    cache = os.environ.get("SEMAPRAX_HARNESS_CACHE_DIR", "")
    index = Index(os.path.realpath(root) if root else "", cache)
    handlers = make_handlers(index)

    def guarded(fn):
        def run(request):
            if not index.root or not index.cache_dir:
                raise AdapterError("unavailable", "SPX-HPG005", "host did not provide project root and cache dir")
            return fn(request)
        return run

    handlers = {k: guarded(v) for k, v in handlers.items()}
    accepted = [{"kind": KIND, "version": 1, "operations": OPERATIONS}]
    prov = {"provider_id": PROVIDER_ID, "adapter_version": ADAPTER_VERSION, "upstream_version": TESTED_VERSIONS[0]}
    serve(accepted, handlers, prov)


if __name__ == "__main__":
    main()
