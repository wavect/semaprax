'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { sourceDigest, checkedSource } = require('../explorer-reveal');

function vscode(documents = []) { return { workspace: { textDocuments: documents } }; }
function fixture() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'spx-explorer-reveal-'));
  fs.mkdirSync(path.join(root, 'src'));
  const file = path.join(root, 'src', 'main.spx');
  const bytes = Buffer.from('module app.main;\n😀value\n', 'utf8');
  fs.writeFileSync(file, bytes);
  const start = bytes.indexOf(Buffer.from('😀'));
  return { root, file, bytes, reference: { path: 'src/main.spx', source_revision: 'sha256:' + 'a'.repeat(64), source_digest: sourceDigest(bytes), span: { start, end: start + 4, line: 2, column: 1 } } };
}
test('current explorer source reveal authenticates exact saved bytes before UTF-16 mapping', () => {
  const value = fixture();
  try {
    const found = checkedSource(vscode(), value.root, value.reference);
    assert.equal(found.file, fs.realpathSync(value.file));
    assert.deepEqual(found.range, { startLine: 1, startColumn: 0, endLine: 1, endColumn: 2 });
    fs.writeFileSync(value.file, Buffer.from('changed'));
    assert.throws(() => checkedSource(vscode(), value.root, value.reference), /no longer match/);
  } finally { fs.rmSync(value.root, { recursive: true, force: true }); }
});
test('current explorer source reveal rejects dirty buffers, traversal, and symlink escapes', () => {
  const value = fixture();
  const outside = fs.mkdtempSync(path.join(os.tmpdir(), 'spx-explorer-outside-'));
  try {
    assert.throws(() => checkedSource(vscode([{ uri: { scheme: 'file', fsPath: fs.realpathSync(value.file) }, isDirty: true }]), value.root, value.reference), /destination buffer/);
    assert.throws(() => checkedSource(vscode(), value.root, { ...value.reference, path: '../outside.spx' }), /escapes/);
    fs.writeFileSync(path.join(outside, 'outside.spx'), value.bytes);
    fs.symlinkSync(path.join(outside, 'outside.spx'), path.join(value.root, 'src', 'escape.spx'));
    assert.throws(() => checkedSource(vscode(), value.root, { ...value.reference, path: 'src/escape.spx' }), /symlink/);
  } finally { fs.rmSync(value.root, { recursive: true, force: true }); fs.rmSync(outside, { recursive: true, force: true }); }
});
