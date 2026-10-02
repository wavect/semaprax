'use strict';

// Synthetic renderer fixtures. They carry no claim that the compiler admits
// these inventories; the stress fixture exercises only UI bounds and layout.
function declaration(index, moduleCount) {
  const module = `module-${String(index % moduleCount).padStart(4, '0')}`;
  return { node_key: `@id:fixture.${String(index).padStart(5, '0')}`, id: `@id:fixture.${index}`, display_name: `fixture_${index}`, kind: 'function', module, path: `${module}.spx` };
}
function relation(index, declarations, family = 'call') {
  return { family, from: declarations[index % declarations.length].node_key, to: declarations[(index * 17 + 1) % declarations.length].node_key, direction: 'forward', site_id: `@id:site.${index}`, provenance: {} };
}
function fixture(name, declarationCount, edgeCount) {
  const declarations = Array.from({ length: declarationCount }, (_, index) => declaration(index, Math.max(1, Math.ceil(declarationCount / 30))));
  return Object.freeze({ name, renderer_only: declarationCount === 4096, declarations: Object.freeze(declarations), relations: Object.freeze(Array.from({ length: edgeCount }, (_, index) => relation(index, declarations))) });
}
function pathological(declarations) {
  const star = Array.from({ length: declarations - 1 }, (_, index) => [0, index + 1]);
  const chain = Array.from({ length: declarations - 1 }, (_, index) => [index, index + 1]);
  const duplicateSite = [[0, 1], [0, 1]];
  const disconnected = [[declarations - 2, declarations - 1]];
  return Object.freeze({ star, chain, duplicateSite, disconnected });
}

const PERF_FIXTURES = Object.freeze({
  small: fixture('small', 30, 60),
  medium: fixture('medium', 500, 2000),
  renderer_stress: fixture('renderer_stress', 4096, 65536),
  pathological: pathological(256)
});
const semapraxExplorerPerfFixturesApi = Object.freeze({ PERF_FIXTURES });
if (typeof module !== 'undefined' && module.exports) module.exports = semapraxExplorerPerfFixturesApi;
else globalThis.SemapraxExplorerPerfFixtures = semapraxExplorerPerfFixturesApi;
