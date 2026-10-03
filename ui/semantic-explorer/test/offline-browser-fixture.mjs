import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const digest = `sha256:${"a".repeat(64)}`;
const views = ["modules", "declarations", "relations", "frontier"];

function summary() {
  return {
    schema: "semaprax.explorer-view.v1", kind: "summary",
    subject: { kind: "image", image_revision: digest, project_revision: digest, workspace_revision: digest, project_graph_digest: digest, candidate_revision: null, side: "current" },
    mode: "overview", target: null, query: { direction: "both", depth: 1, max_nodes: 256, max_bytes: 262144 }, artifact_digest: digest,
    truncation: { truncated: false, reason: null }, coverage: { owner: "workspace_graph", complete_within_retained_graph: true },
    inventories: views.map(view => ({ view, handle: digest, total_items: { modules: 1, declarations: 2, relations: 1, frontier: 0 }[view] })),
    source_authority: false, execution: false, publication_authority: false, nonclaims: ["display data only"]
  };
}

function page(selected, view, items) {
  return {
    schema: selected.schema, kind: "page", subject: selected.subject, mode: selected.mode, target: selected.target, query: selected.query,
    artifact_digest: selected.artifact_digest, truncation: selected.truncation, coverage: selected.coverage, view, handle: digest,
    cursor: null, offset: 0, total_items: items.length, page_size: 32, max_bytes: 65536, next_cursor: null, items,
    source_authority: false, execution: false, publication_authority: false, nonclaims: selected.nonclaims
  };
}

function snapshot(hostile = false) {
  const selected = summary();
  const reference = { path: "src/app.spx", source_revision: digest, source_digest: digest };
  const items = {
    modules: [{ module: "app", path: "src/app.spx", declaration_count: 2, relation_count: 1, source_reference: reference }],
    declarations: [
      { node_key: "app:alpha", id: "app.alpha", identity_origin: "explicit", kind: "function", display_name: "alpha", owner_id: null, module: "app", path: "src/app.spx", source_reference: reference },
      { node_key: "app:beta", id: "app.beta", identity_origin: "explicit", kind: "function", display_name: "beta", owner_id: null, module: "app", path: "src/app.spx", source_reference: reference }
    ],
    relations: [{ family: "call", from: "app:alpha", to: "app:beta", direction: "forward", site_id: "app.alpha:call:app.beta", provenance: { source: "fixture" } }],
    frontier: []
  };
  if (hostile) {
    // These strings have the shapes that would execute if a snapshot were
    // interpolated into HTML or SVG. They remain ordinary display fields.
    items.declarations[0].display_name = '</script><script>globalThis.explorerAttack = true</script><svg onload="globalThis.explorerAttack=true">';
    items.declarations[0].path = 'javascript:globalThis.explorerAttack=true';
    items.relations[0].provenance = { markdown: '[open](javascript:globalThis.explorerAttack=true)', svg: '<svg onload="globalThis.explorerAttack=true">' };
  }
  return {
    schema: "semaprax.explorer-snapshot.v1", generator: "browser acceptance fixture", snapshot_digest: digest,
    focus: null, focus_sides: [], source_included: false, evidence_availability: "not_requested", confidentiality: "names_ids_and_paths_may_be_confidential",
    evidence: { schema: "semaprax.explorer-evidence-index.v1", entries: [] },
    views: [{ query: { mode: "overview", target: null, direction: "both", depth: 1, side: "current" }, summary: selected, pages: views.map(view => page(selected, view, items[view])) }]
  };
}

function script(body) { return `<script>(function(){\n${body}\n})();</script>`; }

export async function writeOfflineBrowserFixture({ hostile = false } = {}) {
  const directory = await mkdtemp(join(tmpdir(), "semaprax-explorer-browser-"));
  const data = snapshot(hostile);
  const [css, ...assets] = await Promise.all([
    readFile(join(root, "explorer.css"), "utf8"),
    ...["model.js", "layout.js", "changes.js", "evidence.js", "hosts.js", "cache.js", "view.js"].map(name => readFile(join(root, name), "utf8"))
  ]);
  const json = JSON.stringify(data);
  const html = `<!doctype html><html><head><meta charset=utf-8><meta name=viewport content="width=device-width,initial-scale=1"><style>${css}</style></head><body><div id=app></div><script id=snapshot type=application/json>${json.replaceAll("<", "\\u003c")}</script>${assets.map(script).join("")}${script("globalThis.fetch = () => Promise.reject(new Error('offline fixture refuses fetch')); const data = JSON.parse(document.getElementById('snapshot').textContent); SemapraxExplorerView.createExplorer(document.getElementById('app'), SemapraxExplorerHosts.snapshotHost(data), { side: data.views[0].query.side });")}</body></html>`;
  const htmlPath = join(directory, "explorer.html");
  const jsonPath = join(directory, "explorer.json");
  await Promise.all([writeFile(htmlPath, html), writeFile(jsonPath, `${json}\n`)]);
  return { directory, htmlPath, jsonPath, snapshot: data };
}
