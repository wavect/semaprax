'use strict';

// Stable layered layout. Cycles collapse only for placement; their original
// directed, site-bearing relations remain intact for inspection.
function layout(nodes, edges, pinned = new Map()) {
  const ordered = [...nodes].sort((a, b) => a.key.localeCompare(b.key));
  const byKey = new Map(ordered.map(node => [node.key, node]));
  const outgoing = new Map(ordered.map(node => [node.key, []]));
  for (const edge of edges) if (byKey.has(edge.from) && byKey.has(edge.to)) outgoing.get(edge.from).push(edge.to);
  for (const targets of outgoing.values()) targets.sort();

  let next = 0;
  const number = new Map(), low = new Map(), stack = [], onStack = new Set(), components = [];
  function visit(key) {
    number.set(key, next); low.set(key, next++); stack.push(key); onStack.add(key);
    for (const target of outgoing.get(key)) {
      if (!number.has(target)) { visit(target); low.set(key, Math.min(low.get(key), low.get(target))); }
      else if (onStack.has(target)) low.set(key, Math.min(low.get(key), number.get(target)));
    }
    if (low.get(key) !== number.get(key)) return;
    const component = [];
    for (;;) {
      const member = stack.pop(); onStack.delete(member); component.push(member);
      if (member === key) break;
    }
    components.push(component.sort());
  }
  for (const node of ordered) if (!number.has(node.key)) visit(node.key);
  components.sort((a, b) => a[0].localeCompare(b[0]));

  const componentOf = new Map();
  components.forEach((members, index) => members.forEach(key => componentOf.set(key, index)));
  const nextComponents = components.map(() => new Set());
  const indegree = components.map(() => 0);
  for (const edge of edges) {
    const from = componentOf.get(edge.from), to = componentOf.get(edge.to);
    if (from === undefined || to === undefined || from === to || nextComponents[from].has(to)) continue;
    nextComponents[from].add(to); indegree[to]++;
  }
  const layer = components.map(() => 0);
  const queue = indegree.map((count, index) => count === 0 ? index : -1).filter(index => index >= 0);
  queue.sort((a, b) => components[a][0].localeCompare(components[b][0]));
  while (queue.length) {
    const from = queue.shift();
    for (const to of [...nextComponents[from]].sort((a, b) => components[a][0].localeCompare(components[b][0]))) {
      layer[to] = Math.max(layer[to], layer[from] + 1);
      if (--indegree[to] === 0) { queue.push(to); queue.sort((a, b) => components[a][0].localeCompare(components[b][0])); }
    }
  }
  const columns = new Map();
  ordered.forEach(node => {
    const index = layer[componentOf.get(node.key)];
    if (!columns.has(index)) columns.set(index, []);
    columns.get(index).push(node.key);
  });
  const positions = new Map();
  for (const [index, members] of [...columns.entries()].sort((a, b) => a[0] - b[0])) {
    members.sort();
    members.forEach((key, row) => {
      const saved = pinned.get(key);
      const safePin = saved && Number.isFinite(saved.x) && Number.isFinite(saved.y) && Math.abs(saved.x) < 100000 && Math.abs(saved.y) < 100000;
      positions.set(key, safePin ? { x: saved.x, y: saved.y } : { x: 36 + index * 294, y: 36 + row * 104 });
    });
  }
  const width = Math.max(360, ...[...positions.values()].map(point => point.x + 256));
  const height = Math.max(240, ...[...positions.values()].map(point => point.y + 80));
  return { positions, width, height, components: components.map(keys => [...keys]) };
}

const semapraxExplorerLayoutApi = { layout };
if (typeof module !== 'undefined' && module.exports) module.exports = semapraxExplorerLayoutApi;
else globalThis.SemapraxExplorerLayout = semapraxExplorerLayoutApi;
