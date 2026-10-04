#!/usr/bin/env python3
"""Graphify context.repository/v1 adapter for the SEMAPRAX harness host.

Standard library only (plus the shared wire helper). Builds a local code-only
graph with the pinned Graphify under the host cache dir and answers from
graph.json directly. See README.md for the trust and coverage rules.
"""

import ast
import fcntl
import glob
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "sdk", "python"))
from semaprax_harness_adapter import AdapterError, serve  # noqa: E402

PROVIDER_ID = "com.graphify-labs/graphify-context"
ADAPTER_VERSION = "0.1.0"
# Explicit, tested schema profiles. Nothing outside this table is normalized, and the
# extractor version is part of the cache identity, so a graph written by one profile is
# never read as another. `call_binding` records what the upstream version is known to bind.
PROFILES = {
    "0.9.25": {"id": "graphify-0.9.25-node-link", "node_keys": ("id", "label", "file_type", "source_file"),
               "edge_keys": ("source", "target", "relation", "confidence", "source_file"),
               "node_optional_bool": (), "intra_class": False},
    "0.9.75": {"id": "graphify-0.9.75-node-link", "node_keys": ("id", "label", "file_type", "source_file"),
               "edge_keys": ("source", "target", "relation", "confidence", "source_file"),
               "node_optional_bool": ("_callable", "_callable_class"), "intra_class": True},
}
TESTED_VERSIONS = tuple(PROFILES)
META_SCHEMA = "semaprax.graphify-adapter-meta.v2"
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
ADOPT_MODES = ("read-only", "copied-snapshot")
MAX_GRAPH_BYTES = 512 * 1024 * 1024
MAX_REASONS = 8


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


def adoption_config(env=None):
    """Opt-in adoption of a user-owned `graphify-out`. Primary source: the host-validated descriptor config
    (SEMAPRAX_HARNESS_CFG_ADOPT_INDEX / _USER_INDEX); SEMAPRAX_GRAPHIFY_* are aliases. Off by default."""
    env = os.environ if env is None else env
    mode = env.get("SEMAPRAX_HARNESS_CFG_ADOPT_INDEX") or env.get("SEMAPRAX_GRAPHIFY_ADOPT_INDEX")
    if not mode:
        return None
    rel = env.get("SEMAPRAX_HARNESS_CFG_USER_INDEX") or env.get("SEMAPRAX_GRAPHIFY_USER_INDEX") or "graphify-out"
    if mode not in ADOPT_MODES or rel.startswith("/") or "\\" in rel or ".." in rel.split("/"):
        return {"invalid": True, "mode": mode, "rel": rel}
    return {"mode": mode, "rel": rel}


def graphify_file_hash(data, rel):
    """Graphify's own cache key for a file: sha256(content + NUL + lower-cased relative posix path)."""
    return hashlib.sha256(data + b"\0" + rel.lower().encode()).hexdigest()


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


def extraction_errors(log, files):
    """Log lines that name a project file become contract `{path, reason}` entries."""
    known, out = set(files), []
    for ln in log:
        if not re.search(r"error|failed|traceback|permission", ln, re.I):
            continue
        hit = next((f for f in sorted(known, key=len, reverse=True) if f in ln), None)
        if hit:
            out.append({"path": hit, "reason": ln.strip()[:240]})
    return out[:200]


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

    def __init__(self, raw, version="0.9.25"):
        profile = PROFILES.get(version)
        if profile is None:
            raise AdapterError("unsupported", "SPX-HPG003", f"no schema profile for graphifyy {version}")
        self.profile = profile
        if not isinstance(raw, dict) or not isinstance(raw.get("nodes"), list) or not isinstance(raw.get("links"), list):
            raise AdapterError("unsupported", "SPX-HPG003", f"graph.json lacks the {version} 'nodes'/'links' lists")
        self.nodes, self.edges, self.by_id = [], [], {}
        for n in raw["nodes"]:
            if not isinstance(n, dict) or not all(isinstance(n.get(k), str) for k in profile["node_keys"]) or any(
                    k in n and not isinstance(n[k], bool) for k in profile["node_optional_bool"]):
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
                isinstance(e.get(k), str) for k in profile["edge_keys"] if k != "confidence"
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
        self.owner, self.parents, self.defs = {}, {}, {}
        for e in self.edges:
            if e["relation"] == "method":
                self.owner[e["target"]] = e["source"]
                self.defs.setdefault(e["source"], set()).add(clean(self.by_id[e["target"]]["label"]))
            elif e["relation"] == "inherits":
                self.parents.setdefault(e["source"], []).append(e["target"])

    def ancestors(self, cls):
        seen, todo = [], list(self.parents.get(cls, ()))
        while todo:
            c = todo.pop(0)
            if c not in seen:
                seen.append(c)
                todo.extend(self.parents.get(c, ()))
        return seen

    def descendants(self, cls):
        return [c for c in self.parents if cls in self.ancestors(c)]


def classify(index, g, e):
    """Resolution of one edge: (status, note). Graphify `EXTRACTED` is a parse fact, not a binding proof.

    resolved    the target is the only possible binding the graph can justify
    ambiguous   several same-named targets exist, or upstream marked it inferred/ambiguous
    unsupported the target is an unresolved stub the graph cannot locate
    Dynamic dispatch (getattr, callbacks, untyped receivers) leaves no edge at all, so a missing
    edge is never evidence of no caller; see the references diagnostic.
    """
    dst = g.by_id[e["target"]]
    if dst.get("_stub"):
        return "unsupported", "unresolved target"
    if e["confidence"] != "EXTRACTED":
        return "ambiguous", f"upstream confidence {e['confidence']}"
    if e["relation"] != "calls":
        return "resolved", ""
    name = clean(dst["label"])
    method = dst["label"].startswith(".")
    cands = [n for n in g.nodes if n["file_type"] == "code" and clean(n["label"]) == name
             and n["label"].startswith(".") == method and not n.get("_callable_class")]
    if len(cands) <= 1:
        return "resolved", "unique name"
    if method and g.profile["intra_class"]:
        src_cls, dst_cls = g.owner.get(e["source"]), g.owner.get(e["target"])
        site = index.line_text(e["source_file"], e["_line"])
        if src_cls and dst_cls and src_cls == dst_cls:
            if any(name in g.defs.get(d, ()) for d in g.descendants(dst_cls)):
                return "ambiguous", "overridden in a subclass (dynamic dispatch)"
            return "resolved", "same-class binding"
        if src_cls and dst_cls and "super" in site and dst_cls in g.ancestors(src_cls):
            definers = [a for a in g.ancestors(src_cls) if name in g.defs.get(a, ())]
            if len(definers) == 1:
                return "resolved", "super call to the only defining ancestor"
            return "ambiguous", "multiple inheritance: several ancestors define it"
    return "ambiguous", f"{len(cands)} same-named candidates"


class Index:
    def __init__(self, root, cache_dir):
        self.root, self.cache_dir = root, cache_dir
        self.out = self.graph_path = self.meta_path = None  # bound to the extractor version in check_identity
        self.identity = None
        self.state = None  # dict: digest, graph, coverage, errors
        self.adoption_note = None  # why a configured user index was not used (scalar, reported in metadata)
        self._lines = {}
        self._defs = {}

    # -- upstream -----------------------------------------------------
    def check_identity(self):
        if self.identity is None:
            upstream = env_var("SEMAPRAX_HARNESS_UPSTREAM")
            if not os.path.isabs(upstream) or not os.path.isfile(upstream):
                raise AdapterError("unavailable", "SPX-HPG005", "SEMAPRAX_HARNESS_UPSTREAM must be an absolute executable path")
            self.upstream, self.identity = upstream, verify_identity(upstream)
            self.bind(os.path.join(self.cache_dir, "graphify-index", self.identity))
        return self.identity

    def bind(self, out):
        self.out = out
        self.graph_path = os.path.join(out, "graphify-out", "graph.json")
        self.meta_path = os.path.join(out, "adapter-meta.json")

    def locked(self):
        """Exclusive advisory lock so concurrent processes sharing a cache never read a half-built index."""
        os.makedirs(self.cache_dir, exist_ok=True)
        fh = open(os.path.join(self.cache_dir, "graphify-index.lock"), "w")
        fcntl.flock(fh, fcntl.LOCK_EX)
        return fh

    def build(self, digest, files):
        """Extract into a private staging dir, then publish it atomically under the cache lock."""
        stage = f"{self.out}.stage-{os.getpid()}"
        shutil.rmtree(stage, ignore_errors=True)
        os.makedirs(stage)  # parents included: graphify-index/ holds one directory per extractor version
        cmd = [self.upstream, "extract", self.root, "--code-only", "--out", stage]
        try:
            proc = subprocess.run(cmd, cwd=self.cache_dir, env=child_env(self.cache_dir), stdin=subprocess.DEVNULL,
                                  capture_output=True, text=True, timeout=BUILD_TIMEOUT_S)
        except subprocess.TimeoutExpired:
            shutil.rmtree(stage, ignore_errors=True)
            raise AdapterError("failed", "SPX-HPG004", "graphify extraction timed out")
        log = (proc.stdout + proc.stderr).splitlines()
        staged_graph = os.path.join(stage, "graphify-out", "graph.json")
        if proc.returncode != 0 or not os.path.isfile(staged_graph):
            shutil.rmtree(stage, ignore_errors=True)
            raise AdapterError("failed", "SPX-HPG004", f"graphify extract exited {proc.returncode}: {' | '.join(log[-3:])[:300]}")
        errors = extraction_errors(log, files)
        with open(os.path.join(stage, "adapter-meta.json"), "w", encoding="utf-8") as fh:
            json.dump({"schema": META_SCHEMA, "upstream": self.identity, "profile": PROFILES[self.identity]["id"],
                       "root": self.root, "digest": digest, "errors": errors,
                       "graph_sha256": file_sha(staged_graph)}, fh, sort_keys=True)
        lock = self.locked()
        try:
            shutil.rmtree(self.out, ignore_errors=True)
            os.rename(stage, self.out)
            return self.load(digest, files, errors)
        finally:
            lock.close()

    def load(self, digest, files, errors):
        return self.load_from(os.path.join(self.out, "graphify-out"), digest, files, errors)

    def load_from(self, gdir, digest, files, errors):
        with open(os.path.join(gdir, "graph.json"), encoding="utf-8") as fh:
            graph = Graph(json.load(fh), self.identity)
        manifest = os.path.join(gdir, "manifest.json")
        indexed = set(graph.files)
        if os.path.isfile(manifest):
            with open(manifest, encoding="utf-8") as fh:
                indexed |= {k for k in json.load(fh) if isinstance(k, str)}
        indexed &= set(files)
        skipped = [{"path": f, "reason": skip_reason(f)} for f in files if f not in indexed]
        self.state = {"digest": digest, "graph": graph, "indexed": sorted(indexed), "skipped": skipped, "errors": errors}
        self._lines, self._defs = {}, {}
        return self.state

    # -- opt-in adoption of a user-owned graphify-out (HN-10) ------------------
    def verify_user_index(self, ac, files):
        """Never trust a found graph. Returns (reasons, graph_dir). Pure reads of the user directory."""
        reasons = []

        def fail(r):
            if len(reasons) < MAX_REASONS:
                reasons.append(r)
        gdir = os.path.join(self.root, ac["rel"])
        try:
            st = os.lstat(gdir)
        except OSError:
            return ["no user index directory"], None
        if not os.path.isdir(gdir) or os.path.islink(gdir) or not os.path.realpath(gdir).startswith(self.root + os.sep):
            return ["user index is not a plain directory inside the project (symlinks are refused)"], None
        gpath = os.path.join(gdir, "graph.json")
        if not os.path.isfile(gpath) or os.path.getsize(gpath) > MAX_GRAPH_BYTES:
            return ["no readable graph.json"], None
        # Worktree binding: graphify records the root it analysed.
        try:
            with open(os.path.join(gdir, ".graphify_root"), encoding="utf-8") as fh:
                built = os.path.realpath(fh.read().strip())
        except OSError:
            built = None
        if built != self.root:
            fail(f"index was built for {built or 'an unknown root'}, not this worktree")
        # Version binding: the per-version AST cache directory names the extractor that wrote it.
        astdir = os.path.join(gdir, "cache", "ast")
        tags = sorted(os.listdir(astdir)) if os.path.isdir(astdir) else []
        if not any(t.startswith(f"v{self.identity}-") for t in tags):
            fail(f"index was not written by graphifyy {self.identity} (cache versions: {', '.join(tags) or 'none'})")
        try:
            with open(gpath, encoding="utf-8") as fh:
                raw = json.load(fh)
            graph = Graph(raw, self.identity)
        except (AdapterError, ValueError) as e:
            fail(f"graph.json does not satisfy the {self.identity} profile: {getattr(e, 'message', e)}")
            return reasons, None
        # Code-only policy: documents, semantic or inferred extraction belong to another mode.
        if any(n.get("file_type") not in ("code", "rationale") or n.get("_origin", "ast") != "ast" for n in raw["nodes"]):
            fail("index contains non-code or non-AST nodes from a richer extraction mode; local code-only policy refuses it")
        # Source binding: every indexed file must hash to what graphify recorded.
        try:
            with open(os.path.join(gdir, "cache", "stat-index.json"), encoding="utf-8") as fh:
                stat_index = json.load(fh)
        except (OSError, ValueError):
            fail("no readable cache/stat-index.json: source content cannot be bound")
            return reasons, None
        walked = set(files)
        recorded = {}
        for key, entry in stat_index.items():
            if isinstance(entry, dict):
                recorded[key] = entry
        for rel in graph.files:
            if rel not in walked:
                fail(f"indexed path {rel} is excluded, ignored or absent in this working tree")
                continue
            entry = recorded.get(rel)
            if not entry:
                fail(f"indexed path {rel} has no recorded signature")
                continue
            path = os.path.join(self.root, rel)
            try:
                with open(path, "rb") as fh:
                    data = fh.read()
                st = os.stat(path)
            except OSError:
                fail(f"indexed path {rel} is unreadable")
                continue
            hashes = entry.get("hashes")
            if isinstance(hashes, dict) and hashes:
                # Content-bound: graphify's own file key (content + relative path).
                if graphify_file_hash(data, rel) not in set(hashes.values()):
                    fail(f"indexed path {rel} differs from its indexed content")
            elif entry.get("size") != len(data) or entry.get("mtime_ns") != st.st_mtime_ns:
                # Languages graphify does not content-hash are bound by its recorded size and
                # nanosecond mtime; this is weaker than a content digest (documented in the README).
                fail(f"indexed path {rel} changed since it was indexed (size or mtime)")
        exts = {os.path.splitext(r)[1].lower() for r in graph.files}
        known = set(graph.files)
        for rel in files:
            if rel not in known and os.path.splitext(rel)[1].lower() in exts and rel not in recorded:
                fail(f"source file {rel} is not in the index")
        return reasons, gdir

    def adopt(self, ac, digest, files):
        """Try the user's index; (state or None, info string). Never writes the user's directory."""
        if ac.get("invalid"):
            return None, f"incompatible: invalid adoption config (mode {ac['mode']!r}, index {ac['rel']!r})"
        before = self.user_digest(os.path.join(self.root, ac["rel"]))
        reasons, gdir = self.verify_user_index(ac, files)
        if reasons or gdir is None:
            return None, "incompatible: " + "; ".join(reasons)
        if ac["mode"] == "copied-snapshot":
            dest = os.path.join(self.cache_dir, "adopted", before, "graphify-out")
            if not os.path.isfile(os.path.join(dest, "graph.json")):
                stage = dest + f".stage-{os.getpid()}"
                shutil.rmtree(stage, ignore_errors=True)
                os.makedirs(stage)
                for name in ("graph.json", "manifest.json"):
                    if os.path.isfile(os.path.join(gdir, name)):
                        shutil.copyfile(os.path.join(gdir, name), os.path.join(stage, name))
                        os.chmod(os.path.join(stage, name), 0o444)
                os.makedirs(os.path.dirname(dest), exist_ok=True)
                try:
                    os.rename(stage, dest)
                except OSError:
                    shutil.rmtree(stage, ignore_errors=True)
            st = self.load_from(dest, digest, files, [])
            st["action"], st["adoption"], st["served_by"] = "copied-validated-index", "copied-validated-index", "adopted-snapshot"
            return st, None
        st = self.load_from(gdir, digest, files, [])
        if self.user_digest(os.path.join(self.root, ac["rel"])) != before:
            raise AdapterError("failed", "SPX-HPG013", "the user-owned index changed while it was being read; refusing it")
        st["action"], st["adoption"], st["served_by"] = "reused-user-index", "reused-user-index", "user-index"
        st["user_digest"], st["user_rel"] = before, ac["rel"]
        return st, None

    @staticmethod
    def user_digest(gdir):
        """Digest of the user-owned files this adapter reads (graph.json, manifest.json, stat-index.json)."""
        h = hashlib.sha256(b"semaprax.graphify-user-index.v1\0")
        for name in ("graph.json", "manifest.json", os.path.join("cache", "stat-index.json")):
            p = os.path.join(gdir, name)
            try:
                h.update(f"{name}\0{file_sha(p)}\n".encode())
            except OSError:
                h.update(f"{name}\0absent\n".encode())
        return h.hexdigest()

    def disk_meta(self):
        """Reusable only when schema, extractor version, profile, root and graph bytes all match."""
        try:
            with open(self.meta_path, encoding="utf-8") as fh:
                meta = json.load(fh)
            ok = (meta.get("schema") == META_SCHEMA and meta.get("upstream") == self.identity
                  and meta.get("profile") == PROFILES[self.identity]["id"] and meta.get("root") == self.root
                  and os.path.isfile(self.graph_path) and meta.get("graph_sha256") == file_sha(self.graph_path))
            return meta if ok else None
        except (OSError, ValueError, AttributeError):
            return None

    def ensure(self, refresh):
        """Return (state, stale_flag)."""
        self.check_identity()
        files = walk_files(self.root)
        digest = source_digest(self.root, files)
        ac = adoption_config()
        if self.state is None and ac and self.adoption_note is None:
            st, note = self.adopt(ac, digest, files)
            if st is not None:
                return st, False
            self.adoption_note = note
        if self.state is not None and self.state.get("served_by") == "user-index":
            changed = self.user_digest(os.path.join(self.root, self.state["user_rel"])) != self.state["user_digest"]
            if changed or self.state["digest"] != digest:
                # The user's index or tree moved on: this adapter never refreshes it; fall back to the owned cache.
                self.state, self.adoption_note = None, "incompatible: the user index or working tree changed since it was verified"
        if self.state is None:
            lock = self.locked()
            try:
                meta = self.disk_meta()
                if meta and meta.get("digest") == digest and refresh != "rebuild":
                    st = self.load(digest, files, meta.get("errors", []))
                    st["action"] = "reuse"
                    self.note_adoption(st)
                    return st, False
            finally:
                lock.close()
            if refresh == "never":
                raise AdapterError("unavailable", "SPX-HPG011", "no current graph and refresh=never")
            st = self.build(digest, files)
            st["action"] = "build"
            self.note_adoption(st)
            return st, False
        if self.state["digest"] != digest or refresh == "rebuild":
            if refresh in ("rebuild", "auto"):
                st = self.build(digest, files)
                st["action"] = "refresh"
                return st, False
            return self.state, True
        self.state["action"] = "reuse"
        return self.state, False

    def note_adoption(self, st):
        if self.adoption_note:
            st["adoption"], st["served_by"] = self.adoption_note, "owned-cache"

    # -- source lines --------------------------------------------------
    def lines(self, rel):
        lines = self._lines.get(rel)
        if lines is None:
            try:
                with open(os.path.join(self.root, rel), "rb") as fh:
                    lines = fh.read().split(b"\n")
            except OSError:
                lines = []
            self._lines[rel] = lines
        return lines

    def line_text(self, rel, line):
        lines = self.lines(rel)
        return lines[line - 1].decode("utf-8", "replace") if 0 < line <= len(lines) else ""

    def span_digest(self, rel, start, end):
        """Same rule the host applies: sha256 of the lines joined by LF, no trailing terminator."""
        lines = self.lines(rel)
        body = b"\n".join(lines[start - 1:end]) if 0 < start <= end <= len(lines) else b""
        return "sha256:" + hashlib.sha256(body).hexdigest()

    def line_digest(self, rel, line):
        return self.span_digest(rel, line, line)

    def definition_end(self, rel, line):
        """Version-bound source resolver: Python `ast` end_lineno for a def/class that starts at `line`."""
        if not rel.endswith(".py"):
            return None
        table = self._defs.get(rel)
        if table is None:
            table = {}
            try:
                tree = ast.parse(b"\n".join(self.lines(rel)))
                for n in ast.walk(tree):
                    if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)) and n.end_lineno:
                        table[n.lineno] = n.end_lineno
            except (SyntaxError, ValueError):
                table = {}
            self._defs[rel] = table
        return table.get(line)

    def span_for(self, rel, node):
        """(start, end, kind). `definition` only when a resolver proved the range; else `start-line`."""
        line = node["_line"]
        end = self.definition_end(rel, line) if node.get("file_type") == "code" else None
        return (line, end, "definition") if end and end >= line else (line, line, "start-line")


def language(rel):
    return LANGS.get(os.path.splitext(rel)[1].lower(), "unknown")


def item(index, rel, line, provenance, rank, text, end=None, kind="start-line"):
    """The closed item shape has no span-kind member, so the kind leads `text` and is counted in metadata."""
    end = end or line
    return {"path": rel, "span": {"start_line": line, "end_line": end}, "digest": index.span_digest(rel, line, end),
            "provenance": provenance, "language": language(rel), "rank": rank, "text": f"[{kind}] {text}"}


def node_item(index, n, provenance, rank, text):
    start, end, kind = index.span_for(n["source_file"], n)
    return item(index, n["source_file"], start, provenance, rank, text, end, kind)


def coverage(state, exhaustive):
    skipped = state["skipped"][:MAX_SKIPPED]
    complete = not state["skipped"] and not state["errors"]
    return {"complete": complete, "indexed_files": len(state["indexed"]), "skipped": skipped,
            "exhaustive": bool(exhaustive and complete), "extraction_errors": state["errors"]}


def node_text(n):
    return f"{n['label']} ({n['file_type']})"


def clean(label):
    return label.strip().lstrip(".").removesuffix("()").lower()


def finish(request, index, state, items, exhaustive, diags=(), extra=None):
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
    kinds = {}
    for it in items:
        k = it["text"][1:it["text"].index("]")]
        kinds[k] = kinds.get(k, 0) + 1
    meta = {"upstream_version": index.identity, "schema_profile": PROFILES[index.identity]["id"],
            "span_kinds": dict(sorted(kinds.items())), "span_resolver": f"python-ast-{sys.version_info[0]}.{sys.version_info[1]}", "index_files": len(state["indexed"]),
            "source_digest": "sha256:" + state["digest"], "refresh": state.get("action", "reuse")}
    if state.get("adoption"):
        meta["index_adoption"] = state["adoption"][:400]
        meta["served_by"] = state.get("served_by", "owned-cache")
    meta.update(extra or {})
    return status, {"items": items, "coverage": cov, "metadata": meta}, diags


def make_handlers(index):
    def prelude(request, op):
        payload = request.get("payload") or {}
        # Contract `refresh`: auto (default) rebuilds a stale graph, rebuild forces, never reports stale.
        state, stale = index.ensure(payload.get("refresh", "auto"))
        if stale:
            cov = coverage(state, False)
            cov.update(complete=False, exhaustive=False)
            return None, payload, ("stale", {"items": [], "coverage": cov}, [
                {"code": "SPX-HPG002", "message": "source files changed since the graph was built and refresh=never"}])
        return state, payload, None

    def limit_of(payload):
        v = payload.get("max_items", payload.get("limit", 20))
        return v if isinstance(v, int) and 0 < v <= 200 else 20

    def orient(request):
        state, payload, early = prelude(request, "orient")
        if early:
            return early
        g = state["graph"]
        hubs = sorted((n for n in g.nodes if n["file_type"] == "code" and g.degree.get(n["id"])),
                      key=lambda n: (-g.degree[n["id"]], n["source_file"], n["_line"], n["id"]))
        items = [node_item(index, n, "structural", i + 1, f"{node_text(n)} degree={g.degree[n['id']]}")
                 for i, n in enumerate(hubs[: limit_of(payload)])]
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
        items = [node_item(index, n, "structural", i + 1, node_text(n))
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
        items = [node_item(index, n, "structural", i + 1, node_text(n)) for i, n in enumerate(nodes)]
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
        items, counts = [], {"resolved": 0, "ambiguous": 0, "unsupported": 0}
        for i, e in enumerate(edges[: limit_of(payload)]):
            src, dst = g.by_id[e["source"]], g.by_id[e["target"]]
            status, note = classify(index, g, e)
            counts[status] += 1
            # Only a resolved binding is structural; ambiguous and unsupported ones stay inferred.
            prov = "structural" if status == "resolved" else "inferred"
            it = item(index, e["source_file"], e["_line"], prov, i + 1,
                      f"{src['label']} -{e['relation']}-> {dst['label']} [{status}{': ' + note if note else ''}; upstream {e['confidence']}]",
                      kind="call-site")
            # Graphify confidence maps to structural/inferred only, never to compiler certainty.
            it["edges"] = [{"target": dst["label"][:1024], "relation": f"{e['relation']}:{status}"[:64], "provenance": prov}]
            items.append(it)
        diags = []
        if items:
            diags.append({"code": "SPX-HPG012", "message": "dynamic dispatch, untyped receivers and getattr calls leave no edge; this list is not a complete caller set"})
        if not targets:
            diags.append({"code": "SPX-HPG010", "message": "symbol not found in the graph; this is not evidence it is unused"})
        elif not items:
            diags.append({"code": "SPX-HPG010", "message": "no recorded edges to the symbol; graphify edges are name-resolved, so absence is not proof of no callers"})
        # Call resolution is never exhaustive: unrepresented dynamic calls cannot be enumerated.
        return finish(request, index, state, items, False, diags, {"resolution": dict(counts)})

    return {(KIND, "orient"): orient, (KIND, "search"): search, (KIND, "skeleton"): skeleton, (KIND, "references"): references}


def declared_version():
    """Version reported in provenance: the verified install, else the first tested profile."""
    try:
        return verify_identity(os.environ["SEMAPRAX_HARNESS_UPSTREAM"])
    except (AdapterError, KeyError, OSError):
        return TESTED_VERSIONS[0]


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
    prov = {"provider_id": PROVIDER_ID, "adapter_version": ADAPTER_VERSION, "upstream_version": declared_version()}
    serve(accepted, handlers, prov)


if __name__ == "__main__":
    main()
