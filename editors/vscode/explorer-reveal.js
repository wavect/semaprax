'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const { SourceIndex } = require('./positions');

const DOMAIN = Buffer.from('semaprax.semantic-review.source-digest.v1\0', 'utf8');
const MAX_SOURCE_BYTES = 8 * 1024 * 1024;
const DIGEST = /^sha256:[0-9a-f]{64}$/;
const plain = value => value && typeof value === 'object' && !Array.isArray(value) && (Object.getPrototypeOf(value) === Object.prototype || Object.getPrototypeOf(value) === null);

function sourceDigest(bytes) {
  if (!Buffer.isBuffer(bytes)) throw new TypeError('source digest requires bytes');
  const length = Buffer.alloc(8); length.writeBigUInt64LE(BigInt(bytes.length));
  return `sha256:${crypto.createHash('sha256').update(DOMAIN).update(length).update(bytes).digest('hex')}`;
}
function reference(value) {
  if (!plain(value) || Object.keys(value).length !== 4 || typeof value.path !== 'string' || !value.path ||
      path.isAbsolute(value.path) || path.win32.isAbsolute(value.path) || value.path.includes('\0') ||
      !DIGEST.test(value.source_revision) || !DIGEST.test(value.source_digest) || !plain(value.span) ||
      Object.keys(value.span).length !== 4) throw new Error('Invalid explorer source reference');
  for (const name of ['start', 'end', 'line', 'column']) if (!Number.isSafeInteger(value.span[name]) || value.span[name] < 0) throw new Error('Invalid explorer source span');
  if (value.span.end < value.span.start || value.span.line < 1 || value.span.column < 1) throw new Error('Invalid explorer source span');
  return value;
}
function contained(root, file) { return file.startsWith(root + path.sep); }
function openDocument(vscode, file) {
  return vscode.workspace.textDocuments.find(doc => doc.uri.scheme === 'file' && doc.uri.fsPath === file);
}
function checkedSource(vscode, manifestRoot, raw, fileSystem = fs) {
  const selected = reference(raw);
  if (typeof manifestRoot !== 'string' || !path.isAbsolute(manifestRoot)) throw new Error('Explorer manifest root is unavailable');
  const root = fileSystem.realpathSync(manifestRoot);
  const candidate = path.resolve(root, selected.path);
  if (!contained(root, candidate)) throw new Error('Explorer source path escapes the manifest root');
  const file = fileSystem.realpathSync(candidate);
  if (!contained(root, file)) throw new Error('Explorer source path escapes through a symlink');
  const before = openDocument(vscode, file);
  if (before?.isDirty) throw new Error('Save the destination buffer before revealing source');
  const bytes = fileSystem.readFileSync(file);
  if (!Buffer.isBuffer(bytes) || bytes.length > MAX_SOURCE_BYTES || sourceDigest(bytes) !== selected.source_digest) throw new Error('Explorer source bytes no longer match the retained revision');
  const range = new SourceIndex(bytes).range(selected.span.start, selected.span.end);
  if (!range) throw new Error('Explorer source span is not valid UTF-8 source location');
  return { file, bytes, range };
}
async function revealCurrentSource(vscode, manifestRoot, raw, live = () => true, fileSystem = fs) {
  if (!live()) throw new Error('Explorer session is stale');
  const selected = checkedSource(vscode, manifestRoot, raw, fileSystem);
  const doc = await vscode.workspace.openTextDocument(vscode.Uri.file(selected.file));
  if (!live()) throw new Error('Explorer session became stale while opening source');
  const after = openDocument(vscode, selected.file);
  if (after?.isDirty) throw new Error('Save the destination buffer before revealing source');
  if (typeof doc.getText !== 'function' || !Buffer.from(doc.getText(), 'utf8').equals(selected.bytes)) throw new Error('Explorer editor buffer does not match the retained source');
  const bytes = fileSystem.readFileSync(selected.file);
  if (!Buffer.isBuffer(bytes) || !bytes.equals(selected.bytes) || sourceDigest(bytes) !== raw.source_digest) throw new Error('Explorer source changed while opening');
  const editor = await vscode.window.showTextDocument(doc);
  const target = new vscode.Range(selected.range.startLine, selected.range.startColumn, selected.range.endLine, selected.range.endColumn);
  editor.selection = new vscode.Selection(target.start, target.end);
  editor.revealRange(target, vscode.TextEditorRevealType.InCenter);
  return selected.file;
}

module.exports = { sourceDigest, reference, checkedSource, revealCurrentSource };
