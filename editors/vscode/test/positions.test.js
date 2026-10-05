'use strict';
// Byte-offset to UTF-16 position work bounds (REF-17). The shared mapper must
// answer many lookups on one long line without re-decoding the line prefix for
// every lookup, in any request order, while keeping the exact answers of the
// original prefix-decoding implementation. Decoder accounting is test-local:
// it wraps the platform TextDecoder and never reaches the shipped API.
const test = require('node:test');
const assert = require('node:assert/strict');
const { TextDecoder } = require('node:util');
const { SourceIndex } = require('../positions');

// The original mapper, kept verbatim as the parity oracle: decode the bytes
// from the line start to the offset with a fatal decoder on every lookup.
const oracleDecoder = new TextDecoder('utf-8', { fatal: true });
const oracleStarts = new WeakMap();
function oraclePosition(bytes, offset) {
  if (!Number.isSafeInteger(offset) || offset < 0 || offset > bytes.length) return null;
  if (!oracleStarts.has(bytes)) {
    const found = [0];
    for (let index = bytes.indexOf(0x0a); index >= 0; index = bytes.indexOf(0x0a, index + 1)) found.push(index + 1);
    oracleStarts.set(bytes, found);
  }
  const starts = oracleStarts.get(bytes);
  let line = 0;
  for (let high = starts.length - 1; line < high;) {
    const middle = (line + high + 1) >> 1;
    if (starts[middle] <= offset) line = middle; else high = middle - 1;
  }
  const next = line + 1 < starts.length ? starts[line + 1] - 1 : bytes.length;
  const end = next > starts[line] && bytes[next - 1] === 0x0d ? next - 1 : next;
  try { return { line, character: oracleDecoder.decode(bytes.subarray(starts[line], Math.min(offset, end))).length }; } catch { return null; }
}

// Count the bytes handed to every TextDecoder while `body` runs.
function decodedBytes(body) {
  const original = TextDecoder.prototype.decode;
  let total = 0;
  TextDecoder.prototype.decode = function (input, options) {
    if (input) total += input.byteLength;
    return original.call(this, input, options);
  };
  try { body(); } finally { TextDecoder.prototype.decode = original; }
  return total;
}

function shuffled(values, seed) {
  const out = values.slice();
  let state = seed >>> 0;
  for (let index = out.length - 1; index > 0; index--) {
    state = (Math.imul(state, 1103515245) + 12345) >>> 0;
    const other = state % (index + 1);
    [out[index], out[other]] = [out[other], out[index]];
  }
  return out;
}

// The documented bound: each lookup decodes at most one checkpoint stride
// plus one partial scalar, and each line is walked once to lay checkpoints.
function bound(lookups, lineBytes) {
  const stride = SourceIndex.CHECKPOINT_BYTES || 128;
  return lookups * (stride + 4) + lineBytes + 4 * Math.ceil(lineBytes / stride);
}

test('positions on one long line are not a quadratic number of decoded bytes', () => {
  const measured = [];
  for (const size of [1024, 2048, 4096]) {
    const bytes = Buffer.alloc(size, 0x61);
    const offsets = Array.from({ length: size }, (_, index) => index + 1);
    for (const [order, list] of [['ascending', offsets], ['descending', offsets.slice().reverse()], ['shuffled', shuffled(offsets, size)], ['repeated', offsets.concat(offsets)]]) {
      const index = new SourceIndex(bytes);
      const total = decodedBytes(() => {
        for (const offset of list) assert.deepEqual(index.position(offset), { line: 0, character: offset });
      });
      const quadratic = size * (size + 1) / 2;
      assert.ok(total <= bound(list.length, size), `${order} ${size}: ${total} decoded bytes exceeds ${bound(list.length, size)} (prefix decoding is ${quadratic})`);
      measured.push(`${order}/${size}=${total}`);
    }
  }
  assert.ok(Number.isSafeInteger(SourceIndex.CHECKPOINT_BYTES) && SourceIndex.CHECKPOINT_BYTES > 0, 'the checkpoint stride is a documented constant');
  process.stdout.write(`# decoded bytes ${measured.join(' ')}\n`);
});

test('many short lines keep a one-off lookup as cheap as decoding that line prefix', () => {
  const text = Array.from({ length: 2000 }, (_, line) => `let v${line} = ${line};`).join('\n');
  const bytes = Buffer.from(text, 'utf8');
  const index = new SourceIndex(bytes);
  const offsets = shuffled(Array.from({ length: bytes.length + 1 }, (_, offset) => offset), 7);
  const expected = offsets.map(offset => oraclePosition(bytes, offset));
  const actual = [];
  const total = decodedBytes(() => { for (const offset of offsets) actual.push(index.position(offset)); });
  assert.deepEqual(actual, expected);
  // Every line is shorter than the stride, so no lookup decodes more than its
  // own line and nothing beyond the original prefix decoding is spent.
  // ASCII source: a position's character count is its decoded prefix bytes.
  const prefix = expected.reduce((sum, position) => sum + position.character, 0);
  assert.ok(total <= prefix, `${total} decoded bytes for short lines exceeds the prefix decoding work ${prefix}`);
});

test('every offset agrees with prefix decoding across Unicode, CRLF, blank lines, malformed bytes and EOF', () => {
  const long = (unit, count) => unit.repeat(count);
  const samples = [
    Buffer.from(''),
    Buffer.from('\n\n\r\n', 'utf8'),
    Buffer.from('\u{1F600} true\nsecond line', 'utf8'),
    Buffer.from('one\r\ntwo\r\n', 'utf8'),
    Buffer.from('éx\tà́b', 'utf8'),
    Buffer.from(long('ab\u{1F600}é\t', 200) + '\r\n' + long('中', 300) + '\n\n' + long('x', 1000), 'utf8'),
    Buffer.from('﻿bom line\n﻿second\n' + long('﻿z', 100), 'utf8'),
    Buffer.concat([Buffer.from(long('a', 300)), Buffer.from([0xff]), Buffer.from(long('b', 300))]),
    Buffer.concat([Buffer.from(long('a', 255)), Buffer.from([0xf0, 0x9f, 0x98]), Buffer.from(long('c', 400) + '\nnext')]),
    Buffer.concat([Buffer.from(long('é', 200)), Buffer.from([0xed, 0xa0, 0x80]), Buffer.from(long('d', 200))]),
    Buffer.concat([Buffer.from(long('q', 129)), Buffer.from([0xc0, 0xaf]), Buffer.from(long('r', 129))]),
    Buffer.concat([Buffer.from(long('s', 700)), Buffer.from([0xe2, 0x82])])
  ];
  for (const bytes of samples) {
    const offsets = Array.from({ length: bytes.length + 3 }, (_, offset) => offset - 1);
    for (const order of [offsets, offsets.slice().reverse(), shuffled(offsets, bytes.length + 1)]) {
      const index = new SourceIndex(bytes);
      for (const offset of order) assert.deepEqual(index.position(offset), oraclePosition(bytes, offset), `offset ${offset} of ${bytes.length}`);
    }
    const index = new SourceIndex(bytes);
    for (const [start, end] of shuffled(offsets.flatMap(start => [[start, start + 1], [start, start + 5]]), 3)) {
      const from = oraclePosition(bytes, start), to = oraclePosition(bytes, end);
      const expected = !from || !to || to.line < from.line || (to.line === from.line && to.character < from.character)
        ? null : { startLine: from.line, startColumn: from.character, endLine: to.line, endColumn: to.character };
      assert.deepEqual(index.range(start, end), expected, `range ${start}..${end}`);
    }
  }
});

test('a malformed prefix stays rejected after checkpoints were laid beyond it', () => {
  const bytes = Buffer.concat([Buffer.from('a'.repeat(10)), Buffer.from([0xff]), Buffer.from('b'.repeat(2000))]);
  const index = new SourceIndex(bytes);
  assert.equal(index.position(2011), null);
  assert.equal(index.position(1500), null);
  assert.deepEqual(index.position(10), { line: 0, character: 10 });
  assert.equal(index.position(11), null);
});

test('the conversion state is bound to one saved snapshot', () => {
  const first = new SourceIndex('x'.repeat(600) + 'y');
  const second = new SourceIndex('\u{1F600}'.repeat(150) + 'y');
  assert.deepEqual(first.position(600), { line: 0, character: 600 });
  assert.deepEqual(second.position(600), { line: 0, character: 300 });
  assert.deepEqual(first.position(600), { line: 0, character: 600 });
});
