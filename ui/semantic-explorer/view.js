'use strict';

const semapraxExplorerModel = typeof module !== 'undefined' && module.exports ? require('./model.js') : globalThis.SemapraxExplorerModel;
const semapraxExplorerLayout = typeof module !== 'undefined' && module.exports ? require('./layout.js') : globalThis.SemapraxExplorerLayout;
const SVG = 'http://www.w3.org/2000/svg';

function element(document, tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}
function svg(document, tag, attributes = {}) {
  const node = document.createElementNS(SVG, tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, String(value));
  return node;
}
function clear(node) { while (node.firstChild) node.removeChild(node.firstChild); }
function short(value, length = 30) { return value.length > length ? `${value.slice(0, length - 1)}…` : value; }
function label(family) { return family.replaceAll('_', ' '); }

function createExplorer(root, host, options = {}) {
  if (!root || !root.ownerDocument || !host || typeof host.summary !== 'function' || typeof host.page !== 'function') throw new TypeError('explorer host contract');
  const document = root.ownerDocument;
  const state = {
    generation: 0, summary: null, rows: Object.fromEntries(semapraxExplorerModel.VIEWS.map(view => [view, []])),
    cursors: Object.fromEntries(semapraxExplorerModel.VIEWS.map(view => [view, null])),
    loaded: new Set(), selected: null, selectedModule: null, selectedRelation: null,
    search: '', direction: 'both', depth: 1, families: new Set(semapraxExplorerModel.FAMILIES),
    scale: 1, panX: 0, panY: 0, pins: new Map(), theme: 'system', busy: false
  };

  const shell = element(document, 'div', 'spx-explorer');
  shell.dataset.theme = state.theme;
  const header = element(document, 'header', 'spx-head');
  const titleBlock = element(document, 'div', 'spx-title');
  titleBlock.append(element(document, 'p', 'spx-kicker', 'SEMAPRAX · Project explorer'), element(document, 'h1', '', 'Meaning, mapped.'));
  const identity = element(document, 'p', 'spx-identity', 'Opening project…');
  header.append(titleBlock, identity);
  const controls = element(document, 'div', 'spx-controls');
  const searchInput = element(document, 'input', 'spx-search');
  searchInput.type = 'search'; searchInput.placeholder = 'Search ID, name or path'; searchInput.setAttribute('aria-label', 'Search declarations');
  const backButton = element(document, 'button', 'spx-button', 'Overview'); backButton.type = 'button';
  const directionSelect = element(document, 'select', 'spx-select'); directionSelect.setAttribute('aria-label', 'Relationship direction');
  for (const direction of ['both', 'forward', 'reverse']) { const option = element(document, 'option', '', direction); option.value = direction; directionSelect.append(option); }
  const depthSelect = element(document, 'select', 'spx-select'); depthSelect.setAttribute('aria-label', 'Neighborhood depth');
  for (let depth = 0; depth <= 3; depth++) { const option = element(document, 'option', '', `Depth ${depth}`); option.value = String(depth); if (depth === 1) option.selected = true; depthSelect.append(option); }
  const familyButton = element(document, 'button', 'spx-button', 'Relations'); familyButton.type = 'button'; familyButton.setAttribute('aria-expanded', 'false');
  const fitButton = element(document, 'button', 'spx-button', 'Fit'); fitButton.type = 'button';
  const themeButton = element(document, 'button', 'spx-button', 'Theme'); themeButton.type = 'button';
  controls.append(searchInput, backButton, directionSelect, depthSelect, familyButton, fitButton, themeButton);
  const filters = element(document, 'div', 'spx-filters'); filters.hidden = true;
  for (const family of semapraxExplorerModel.FAMILIES) {
    const wrapper = element(document, 'label', 'spx-filter');
    const input = element(document, 'input'); input.type = 'checkbox'; input.checked = true; input.value = family;
    wrapper.append(input, document.createTextNode(label(family))); filters.append(wrapper);
    input.addEventListener('change', () => { input.checked ? state.families.add(family) : state.families.delete(family); draw(); });
  }
  const main = element(document, 'main', 'spx-main');
  const canvasPanel = element(document, 'section', 'spx-canvas-panel'); canvasPanel.setAttribute('aria-label', 'Semantic graph');
  const graphBar = element(document, 'div', 'spx-graph-bar');
  const breadcrumb = element(document, 'div', 'spx-breadcrumb', 'Project overview');
  const coverage = element(document, 'div', 'spx-coverage', 'Not loaded');
  graphBar.append(breadcrumb, coverage);
  const graphViewport = element(document, 'div', 'spx-graph-viewport');
  const graph = svg(document, 'svg', { role: 'img', 'aria-label': 'Directed semantic relationships', viewBox: '0 0 360 240' });
  graphViewport.append(graph);
  const list = element(document, 'div', 'spx-list'); list.setAttribute('role', 'region'); list.setAttribute('aria-label', 'Accessible graph list');
  const loadMore = element(document, 'button', 'spx-button spx-more', 'Load next page'); loadMore.type = 'button'; loadMore.hidden = true;
  canvasPanel.append(graphBar, graphViewport, list, loadMore);
  const inspector = element(document, 'aside', 'spx-inspector'); inspector.setAttribute('aria-label', 'Selection details');
  main.append(canvasPanel, inspector);
  const status = element(document, 'p', 'spx-status'); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
  shell.append(header, controls, filters, main, status); root.append(shell);

  function visibleRows() {
    const modules = state.rows.modules;
    const declarations = state.rows.declarations;
    const relations = state.rows.relations.filter(row => state.families.has(row.family));
    if (!state.summary || state.summary.mode !== 'overview') {
      const found = semapraxExplorerModel.search(declarations, state.search);
      const nodes = found.map(row => ({ key: row.node_key, title: row.display_name, subtitle: row.kind, row }));
      const known = new Set(nodes.map(node => node.key));
      for (const relation of relations) for (const key of [relation.from, relation.to]) {
        if (known.has(key)) continue;
        known.add(key); nodes.push({ key, title: short(key, 35), subtitle: 'Boundary · not loaded', row: null });
      }
      return { nodes, relations };
    }
    const found = semapraxExplorerModel.search(declarations, state.search);
    const visibleModules = state.search ? new Set(found.map(row => row.module)) : new Set(modules.map(row => row.module));
    if (state.selectedModule) visibleModules.add(state.selectedModule);
    const nodes = modules.filter(row => visibleModules.has(row.module)).map(row => ({ key: row.module, title: row.module, subtitle: `${row.declaration_count} declarations`, row }));
    const owner = new Map(declarations.map(row => [row.node_key, row.module]));
    const aggregated = relations.map(row => ({ ...row, from: owner.get(row.from) || row.from, to: owner.get(row.to) || row.to })).filter(row => visibleModules.has(row.from) && visibleModules.has(row.to));
    return { nodes, relations: aggregated };
  }

  function showInspector() {
    clear(inspector);
    if (state.selectedRelation) {
      const group = state.selectedRelation;
      inspector.append(element(document, 'p', 'spx-kicker', 'Relationship'), element(document, 'h2', '', label(group.family)));
      inspector.append(element(document, 'p', 'spx-detail', `${group.from} → ${group.to}`));
      inspector.append(element(document, 'p', 'spx-muted', `${group.sites.length} original site${group.sites.length === 1 ? '' : 's'}`));
      const sites = element(document, 'ol', 'spx-sites');
      for (const site of group.sites) {
        const entry = element(document, 'li');
        entry.append(element(document, 'strong', '', site.site_id), element(document, 'span', '', JSON.stringify(site.provenance)));
        sites.append(entry);
      }
      inspector.append(sites); return;
    }
    const selected = state.rows.declarations.find(row => row.node_key === state.selected);
    if (selected) {
      inspector.append(element(document, 'p', 'spx-kicker', selected.kind), element(document, 'h2', '', selected.display_name));
      const facts = [['Stable ID', selected.id], ['Module', selected.module], ['Path', selected.path || 'No file path'], ['Identity', selected.identity_origin]];
      const dl = element(document, 'dl', 'spx-facts');
      for (const [name, value] of facts) dl.append(element(document, 'dt', '', name), element(document, 'dd', '', value));
      inspector.append(dl);
      if (selected.source_reference && selected.source_reference.path && typeof host.reveal === 'function') {
        const reveal = element(document, 'button', 'spx-button spx-primary', 'Reveal source'); reveal.type = 'button';
        reveal.addEventListener('click', () => Promise.resolve(host.reveal(selected.source_reference)).catch(showError)); inspector.append(reveal);
      }
      const touching = state.rows.relations.filter(row => row.from === selected.node_key || row.to === selected.node_key);
      inspector.append(element(document, 'h3', '', 'Connections'));
      inspector.append(element(document, 'p', 'spx-muted', touching.length ? `${touching.length} visible relation site${touching.length === 1 ? '' : 's'}` : 'No loaded relation sites. This does not prove absence.'));
      return;
    }
    if (state.selectedModule) {
      const module = state.rows.modules.find(row => row.module === state.selectedModule);
      inspector.append(element(document, 'p', 'spx-kicker', 'Module'), element(document, 'h2', '', state.selectedModule));
      if (module) inspector.append(element(document, 'p', 'spx-muted', `${module.declaration_count} declarations · ${module.relation_count} relations`));
      const entries = element(document, 'div', 'spx-module-declarations');
      for (const row of state.rows.declarations.filter(row => row.module === state.selectedModule)) {
        const button = element(document, 'button', 'spx-row', `${row.display_name} · ${row.kind}`); button.type = 'button';
        button.addEventListener('click', () => selectDeclaration(row.node_key)); entries.append(button);
      }
      inspector.append(entries); return;
    }
    if (state.selected) {
      inspector.append(element(document, 'p', 'spx-kicker', 'Boundary reference'), element(document, 'h2', '', short(state.selected, 60)));
      inspector.append(element(document, 'p', 'spx-muted', 'This endpoint is outside the loaded declaration slice. Its presence does not prove the full relation inventory was analyzed.'));
      return;
    }
    inspector.append(element(document, 'p', 'spx-kicker', 'Navigate'), element(document, 'h2', '', 'Select a module or declaration'));
    inspector.append(element(document, 'p', 'spx-muted', 'The graph shows checked structural relationships. Open a card for its declaration list and source provenance.'));
  }

  function selectNode(node) {
    state.selectedRelation = null;
    if (state.summary.mode === 'overview') { state.selectedModule = node.key; state.selected = null; draw(); }
    else if (node.row) selectDeclaration(node.key);
    else { state.selected = node.key; showInspector(); }
  }

  function draw() {
    if (!state.summary) return;
    identity.textContent = `${state.summary.subject.project_revision} · ${state.summary.subject.side}`;
    breadcrumb.textContent = state.summary.mode === 'overview' ? 'Project overview' : `${state.summary.mode} / ${state.summary.target}`;
    const omitted = state.summary.truncation.truncated || state.summary.coverage.complete_within_query === false || state.summary.coverage.complete_within_query === null;
    const unloaded = semapraxExplorerModel.VIEWS.some(view => state.rows[view].length < state.summary.inventories.find(row => row.view === view).total_items);
    const hidden = state.families.size < semapraxExplorerModel.FAMILIES.length || Boolean(state.search);
    coverage.textContent = [state.summary.truncation.truncated ? 'Compiler truncated' : (omitted ? 'Not analyzed' : 'Compiler inventory'), unloaded ? 'Not loaded pages' : null, hidden ? 'Hidden by filter' : null].filter(Boolean).join(' · ');
    loadMore.hidden = !unloaded;
    const { nodes, relations } = visibleRows();
    const shown = nodes.slice(0, 250);
    const seen = new Set(shown.map(node => node.key));
    const grouped = semapraxExplorerModel.groupRelations(relations).filter(row => seen.has(row.from) && seen.has(row.to)).slice(0, 1000);
    const measured = semapraxExplorerLayout.layout(shown, grouped, state.pins);
    clear(graph); graph.setAttribute('viewBox', `0 0 ${measured.width} ${measured.height}`);
    graph.style.transform = `translate(${state.panX}px, ${state.panY}px) scale(${state.scale})`;
    for (const edge of grouped) {
      const from = measured.positions.get(edge.from), to = measured.positions.get(edge.to);
      if (!from || !to) continue;
      const path = svg(document, 'path', { d: `M ${from.x + 222} ${from.y + 32} C ${from.x + 244} ${from.y + 32}, ${to.x - 20} ${to.y + 32}, ${to.x} ${to.y + 32}`, class: 'spx-edge', 'data-family': edge.family, tabindex: '0', role: 'button', 'aria-label': `${label(edge.family)} from ${edge.from} to ${edge.to}, ${edge.sites.length} sites` });
      path.addEventListener('click', () => { state.selectedRelation = edge; showInspector(); });
      path.addEventListener('keydown', event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); state.selectedRelation = edge; showInspector(); } });
      graph.append(path);
      if (edge.sites.length > 1) {
        const badge = svg(document, 'text', { x: (from.x + to.x + 222) / 2, y: (from.y + to.y) / 2 + 20, class: 'spx-site-count' }); badge.textContent = String(edge.sites.length); graph.append(badge);
      }
    }
    for (const node of shown) {
      const point = measured.positions.get(node.key);
      const group = svg(document, 'g', { class: `spx-node${node.row ? '' : ' is-boundary'}${state.selected === node.key || state.selectedModule === node.key ? ' is-selected' : ''}`, transform: `translate(${point.x} ${point.y})`, tabindex: '0', role: 'button', 'aria-label': `${node.title}, ${node.subtitle}` });
      group.append(svg(document, 'rect', { x: 0, y: 0, width: 222, height: 66, rx: 8 }));
      const text = svg(document, 'text', { x: 16, y: 29, class: 'spx-node-name' }); text.textContent = short(node.title); group.append(text);
      const sub = svg(document, 'text', { x: 16, y: 49, class: 'spx-node-sub' }); sub.textContent = short(node.subtitle, 35); group.append(sub);
      group.addEventListener('click', () => selectNode(node));
      group.addEventListener('keydown', event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); selectNode(node); } });
      graph.append(group);
    }
    clear(list);
    const listHeading = element(document, 'h3', '', state.summary.mode === 'overview' ? 'Modules' : 'Declarations'); list.append(listHeading);
    for (const node of nodes) {
      const button = element(document, 'button', 'spx-row'); button.type = 'button';
      button.append(element(document, 'strong', '', node.title), element(document, 'span', '', node.subtitle));
      button.addEventListener('click', () => selectNode(node)); list.append(button);
    }
    if (nodes.length > 250 || grouped.length > 1000) list.append(element(document, 'p', 'spx-muted', 'Viewport budget reached. Remaining loaded rows are available in this list.'));
    showInspector();
  }

  async function fetchPage(view, cursor, generation) {
    const inventory = state.summary.inventories.find(row => row.view === view);
    const candidate = await host.page({ summary: state.summary, view, handle: inventory.handle, cursor, page_size: 32, max_bytes: 64 * 1024 });
    if (generation !== state.generation) return;
    semapraxExplorerModel.page(candidate, state.summary);
    if (candidate.cursor !== cursor || candidate.offset !== state.rows[view].length) throw new TypeError('explorer nonsequential page');
    state.rows[view].push(...candidate.items);
    state.cursors[view] = candidate.next_cursor;
    state.loaded.add(view);
  }

  async function open(query) {
    const generation = ++state.generation;
    state.busy = true; status.textContent = 'Loading checked project view…';
    try {
      const selected = semapraxExplorerModel.summary(await host.summary(query));
      if (generation !== state.generation) return;
      state.summary = selected;
      state.rows = Object.fromEntries(semapraxExplorerModel.VIEWS.map(view => [view, []]));
      state.cursors = Object.fromEntries(semapraxExplorerModel.VIEWS.map(view => [view, null]));
      state.loaded.clear(); state.selectedRelation = null;
      for (const view of semapraxExplorerModel.VIEWS) await fetchPage(view, null, generation);
      if (generation !== state.generation) return;
      status.textContent = 'Project view ready'; draw();
    } catch (error) { if (generation === state.generation) showError(error); }
    finally { if (generation === state.generation) state.busy = false; }
  }

  function showError(error) {
    status.textContent = `Explorer could not load this view: ${error instanceof Error ? error.message : String(error)}`;
    clear(graph); clear(list); clear(inspector);
    inspector.append(element(document, 'h2', '', 'View unavailable'), element(document, 'p', 'spx-muted', 'Check the project revision and refresh explicitly. No source was changed.'));
  }

  async function selectDeclaration(key) {
    const row = state.rows.declarations.find(candidate => candidate.node_key === key);
    if (!row) { showError(new TypeError('declaration not loaded')); return; }
    state.selected = key; state.selectedModule = null; state.selectedRelation = null;
    await open({ mode: 'context', target: row.id, direction: state.direction, depth: state.depth, side: state.summary.subject.side });
  }

  searchInput.addEventListener('input', () => { state.search = searchInput.value.slice(0, 4096); draw(); });
  backButton.addEventListener('click', () => { state.selected = null; state.selectedModule = null; state.search = ''; searchInput.value = ''; open({ mode: 'overview', side: options.side || 'current' }); });
  directionSelect.addEventListener('change', () => { state.direction = directionSelect.value; if (state.selected && state.summary.mode === 'context') open({ mode: 'context', target: state.summary.target, direction: state.direction, depth: state.depth, side: state.summary.subject.side }); });
  depthSelect.addEventListener('change', () => { state.depth = Number(depthSelect.value); if (state.selected && state.summary.mode === 'context') open({ mode: 'context', target: state.summary.target, direction: state.direction, depth: state.depth, side: state.summary.subject.side }); });
  familyButton.addEventListener('click', () => { filters.hidden = !filters.hidden; familyButton.setAttribute('aria-expanded', String(!filters.hidden)); });
  fitButton.addEventListener('click', () => { state.scale = 1; state.panX = 0; state.panY = 0; draw(); });
  themeButton.addEventListener('click', () => { state.theme = state.theme === 'dark' ? 'light' : 'dark'; shell.dataset.theme = state.theme; });
  loadMore.addEventListener('click', async () => {
    if (state.busy || !state.summary) return;
    state.busy = true;
    try {
      const view = semapraxExplorerModel.VIEWS.find(name => state.rows[name].length < state.summary.inventories.find(row => row.view === name).total_items);
      if (view) await fetchPage(view, state.cursors[view], state.generation);
      draw();
    } catch (error) { showError(error); }
    finally { state.busy = false; }
  });
  graphViewport.addEventListener('wheel', event => {
    if (!event.ctrlKey && !event.metaKey) return;
    event.preventDefault(); state.scale = Math.min(2.5, Math.max(0.45, state.scale * (event.deltaY < 0 ? 1.1 : 0.9))); draw();
  }, { passive: false });
  let drag = null;
  graphViewport.addEventListener('pointerdown', event => { if (event.target === graph) { drag = { x: event.clientX, y: event.clientY, panX: state.panX, panY: state.panY }; graphViewport.setPointerCapture(event.pointerId); } });
  graphViewport.addEventListener('pointermove', event => { if (drag) { state.panX = drag.panX + event.clientX - drag.x; state.panY = drag.panY + event.clientY - drag.y; graph.style.transform = `translate(${state.panX}px, ${state.panY}px) scale(${state.scale})`; } });
  graphViewport.addEventListener('pointerup', () => { drag = null; });

  open({ mode: 'overview', side: options.side || 'current' });
  return { open, selectDeclaration, state, destroy: () => { ++state.generation; shell.remove(); } };
}

const semapraxExplorerViewApi = { createExplorer };
if (typeof module !== 'undefined' && module.exports) module.exports = semapraxExplorerViewApi;
else globalThis.SemapraxExplorerView = semapraxExplorerViewApi;
