import assert from 'node:assert/strict';
import test from 'node:test';
import {
  COMMAND_INPUT_IO_ERROR,
  createStdinStreamProvider,
  StdinStreamGuardError,
  STDIN_STREAM_CHUNK_BYTES,
} from '../../stdin_stream_provider.mjs';

function scriptedReader(bytes, sizes) {
  let offset = 0;
  let call = 0;
  const buffers = [];
  const readInto = buffer => {
    buffers.push(buffer);
    const requested = sizes[call] ?? buffer.length;
    call += 1;
    const length = Math.min(requested, buffer.length, bytes.length - offset);
    buffer.set(bytes.subarray(offset, offset + length), 0);
    offset += length;
    return { length, eof: length === 0 };
  };
  return { readInto, buffers, callCount: () => call };
}

function collect(provider, reader) {
  const bytes = [];
  const viewLengths = [];
  for (;;) {
    const result = provider.next(reader);
    assert.equal(result.reader, reader);
    if (result.kind === 'eof') return { bytes, viewLengths, eof: result };
    assert.equal(result.kind, 'chunk');
    assert.ok(Object.isFrozen(result));
    assert.ok(Object.isFrozen(result.chunk));
    assert.equal('buffer' in result.chunk, false);
    const length = result.chunk.byteLength;
    viewLengths.push(length);
    for (let index = 0; index < length; index += 1) {
      bytes.push(result.chunk.byteAt(index));
    }
  }
}

function expectGuard(callback) {
  assert.throws(callback, error => error instanceof StdinStreamGuardError
    && error.code === 'ERR_STDIN_STREAM_GUARD');
}

test('short and full reads reuse the reader and one private 4096-byte buffer', () => {
  const expected = Uint8Array.from({ length: 4_111 }, (_, index) => index % 251);
  const source = scriptedReader(expected, [13, 4096, 2]);
  const provider = createStdinStreamProvider(source.readInto);
  const reader = provider.open();
  const result = collect(provider, reader);

  assert.deepEqual(Uint8Array.from(result.bytes), expected);
  assert.deepEqual(result.viewLengths, [13, 4096, 2]);
  assert.equal(source.callCount(), 4); // three data reads and exact-zero EOF
  assert.ok(source.buffers.every(buffer => buffer === source.buffers[0]));
  assert.equal(source.buffers[0].byteLength, STDIN_STREAM_CHUNK_BYTES);
  assert.equal(provider.next(reader), result.eof); // EOF is latched, no further host call
  assert.equal(source.callCount(), 4);
  provider.drop(reader);
});

test('empty input starts at EOF and repeated next does not call the provider again', () => {
  let calls = 0;
  const provider = createStdinStreamProvider(buffer => {
    calls += 1;
    return { length: 0, eof: true };
  });
  const reader = provider.open();
  const eof = provider.next(reader);
  assert.deepEqual(eof, { kind: 'eof', reader });
  assert.equal(provider.next(reader), eof);
  assert.equal(calls, 1);
  provider.drop(reader);
});

test('checked read errors publish no chunk and map to command input code 3', () => {
  let calls = 0;
  const provider = createStdinStreamProvider(buffer => {
    calls += 1;
    buffer.fill(0xa5); // a failing provider may have touched the reusable buffer
    throw new Error('underlying read failed');
  });
  const reader = provider.open();
  const failure = provider.next(reader);
  assert.deepEqual(failure, {
    kind: 'io-error', reader, ...COMMAND_INPUT_IO_ERROR,
  });
  assert.equal(failure.code, 3);
  assert.equal(failure.domain, 'semaprax.command-input.v1');
  assert.equal('chunk' in failure, false);
  assert.equal(provider.next(reader), failure);
  assert.equal(calls, 1);
  provider.drop(reader);
  expectGuard(() => provider.open()); // open remains consumed after settlement
});

test('65,537 whitespace bytes stream through a constant reusable buffer', () => {
  const input = new Uint8Array(65_537).fill(0x20);
  const sizes = [17, 4011, 3, 4096, 1001];
  const source = scriptedReader(input, sizes);
  const provider = createStdinStreamProvider(source.readInto);
  const reader = provider.open();
  const { bytes } = collect(provider, reader);

  assert.equal(bytes.length, 65_537);
  assert.ok(bytes.every(byte => byte === 0x20));
  assert.ok(source.callCount() > 16);
  assert.ok(source.buffers.every(buffer => buffer === source.buffers[0]));
  assert.equal(source.buffers[0].byteLength, 4096);
  provider.drop(reader);
});

test('forged and cross-invocation reader tokens poison the receiving provider', () => {
  const first = createStdinStreamProvider(() => ({ length: 0, eof: true }));
  const second = createStdinStreamProvider(() => ({ length: 0, eof: true }));
  const reader = first.open();
  expectGuard(() => second.next(reader));
  expectGuard(() => second.open());
  expectGuard(() => first.next(Object.freeze({})));
});

test('opening twice and settling twice are rejected', () => {
  const provider = createStdinStreamProvider(() => ({ length: 0, eof: true }));
  const reader = provider.open();
  expectGuard(() => provider.open());

  const settling = createStdinStreamProvider(() => ({ length: 0, eof: true }));
  const settlingReader = settling.open();
  settling.drop(settlingReader);
  expectGuard(() => settling.drop(settlingReader));
});

test('advancing expires old chunk views before reusing the buffer', () => {
  const bytes = Uint8Array.of(0x41, 0x42);
  const source = scriptedReader(bytes, [1, 1]);
  const provider = createStdinStreamProvider(source.readInto);
  const reader = provider.open();
  const first = provider.next(reader).chunk;
  assert.equal(first.byteAt(0), 0x41);
  expectGuard(() => first.byteAt.call(Object.create(first), 0));
  // The hostile receiver poisons this provider, so test stale generations on
  // an independent invocation.
  const fresh = createStdinStreamProvider(scriptedReader(bytes, [1, 1]).readInto);
  const freshReader = fresh.open();
  const oldView = fresh.next(freshReader).chunk;
  const second = fresh.next(freshReader).chunk;
  assert.equal(second.byteAt(0), 0x42);
  expectGuard(() => oldView.byteAt(0));
});

test('reentrant provider access is a guard failure, not a checked IO error', () => {
  let provider;
  let reader;
  provider = createStdinStreamProvider(() => {
    expectGuard(() => provider.next(reader));
    return { length: 0, eof: true };
  });
  reader = provider.open();
  expectGuard(() => provider.next(reader));
});

test('malformed read records poison the guard instead of masquerading as EOF', () => {
  for (const invalid of [
    { length: 0, eof: false },
    { length: 1, eof: true },
    { length: STDIN_STREAM_CHUNK_BYTES + 1, eof: false },
    { length: 0, eof: true, extra: true },
    new Proxy({}, { ownKeys() { throw new Error('malformed provider record'); } }),
  ]) {
    const provider = createStdinStreamProvider(() => invalid);
    const reader = provider.open();
    expectGuard(() => provider.next(reader));
  }
});
