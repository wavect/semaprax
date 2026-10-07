/**
 * Invocation-scoped foundation for a bounded synchronous stdin reader.
 *
 * `createStdinStreamProvider(readInto)` accepts an injected synchronous
 * `readInto(buffer)` function. The function must fill at most the supplied
 * 4096-byte buffer and return exactly `{ length, eof }`: positive lengths are
 * short/full data (`eof: false`); only `{ length: 0, eof: true }` is EOF.
 * Thrown read errors become a latched `{ kind: 'io-error', domain:
 * 'semaprax.command-input.v1', code: 3 }` result. Malformed/foreign/stale
 * carriers, reentrancy, and duplicate settlement are guard errors instead.
 *
 * `open()` creates one opaque reader and its reusable buffer. `next(reader)`
 * returns the same reader plus a generation-bound read-only chunk view, EOF,
 * or the closed I/O failure result. A later `next` or `drop` expires prior
 * views. The private buffer is never returned or copied by this module.
 * This module has no ambient stdin or filesystem access.
 */

export const STDIN_STREAM_CHUNK_BYTES = 4096;
export const COMMAND_INPUT_IO_ERROR = Object.freeze({
  domain: 'semaprax.command-input.v1',
  code: 3,
});

export class StdinStreamGuardError extends Error {
  constructor(message) {
    super(`stdin stream provider guard: ${message}`);
    this.name = 'StdinStreamGuardError';
    this.code = 'ERR_STDIN_STREAM_GUARD';
  }
}

export function createStdinStreamProvider(readInto) {
  if (typeof readInto !== 'function') {
    throw new TypeError('stdin stream provider requires a synchronous readInto function');
  }

  const invocation = {
    opened: false,
    active: false,
    guardError: null,
    stateByReader: new WeakMap(),
    stateByView: new WeakMap(),
  };

  const poison = message => {
    if (invocation.guardError === null) {
      invocation.guardError = new StdinStreamGuardError(message);
    }
    throw invocation.guardError;
  };

  const assertHealthy = () => {
    if (invocation.guardError !== null) throw invocation.guardError;
  };

  const stateForReader = reader => {
    assertHealthy();
    const state = (typeof reader === 'object' && reader !== null)
      ? invocation.stateByReader.get(reader)
      : undefined;
    if (!state) poison('reader token is forged or belongs to another invocation');
    if (state.dropped) poison('reader has already settled');
    return state;
  };

  const currentView = (view, state, generation) => {
    assertHealthy();
    const membership = (typeof view === 'object' && view !== null)
      ? invocation.stateByView.get(view)
      : undefined;
    if (!membership || membership.state !== state || membership.generation !== generation) {
      poison('chunk view is forged or stale');
    }
    if (state.dropped || state.eof || state.ioFailure !== null || state.generation !== generation) {
      poison('chunk view expired before access');
    }
  };

  const validateReadResult = result => {
    if (result === null || typeof result !== 'object') poison('readInto returned a non-record');
    const prototype = Object.getPrototypeOf(result);
    if (prototype !== Object.prototype && prototype !== null) {
      poison('readInto returned a non-plain record');
    }
    const keys = Reflect.ownKeys(result);
    if (keys.length !== 2 || !keys.includes('length') || !keys.includes('eof')) {
      poison('readInto result fields are not closed');
    }
    const lengthDescriptor = Object.getOwnPropertyDescriptor(result, 'length');
    const eofDescriptor = Object.getOwnPropertyDescriptor(result, 'eof');
    if (!lengthDescriptor || !('value' in lengthDescriptor)
        || !eofDescriptor || !('value' in eofDescriptor)) {
      poison('readInto result fields must be data properties');
    }
    const { length, eof } = { length: lengthDescriptor.value, eof: eofDescriptor.value };
    if (!Number.isInteger(length) || length < 0 || length > STDIN_STREAM_CHUNK_BYTES
        || typeof eof !== 'boolean' || (length === 0) !== eof) {
      poison('readInto result does not distinguish data from exact-zero EOF');
    }
    return { length, eof };
  };

  const open = () => {
    assertHealthy();
    if (invocation.opened) poison('only one reader may be opened per invocation');
    // Consume the invocation's one open before allocating; even allocation
    // failure cannot make a second reader admissible.
    invocation.opened = true;
    const reader = Object.freeze(Object.create(null));
    const state = {
      reader,
      buffer: new Uint8Array(STDIN_STREAM_CHUNK_BYTES),
      generation: 0,
      length: 0,
      eof: false,
      eofResult: null,
      ioFailure: null,
      dropped: false,
      active: false,
    };
    invocation.stateByReader.set(reader, state);
    return reader;
  };

  const next = reader => {
    const state = stateForReader(reader);
    if (invocation.active || state.active) poison('reader operation is reentrant');
    if (state.ioFailure !== null) return state.ioFailure;
    if (state.eof) return state.eofResult;
    if (state.generation >= Number.MAX_SAFE_INTEGER) poison('chunk generation exhausted');

    // Invalidate the previous view before the provider can mutate the buffer.
    state.generation += 1;
    state.length = 0;
    invocation.active = true;
    state.active = true;
    let rawResult;
    try {
      rawResult = readInto(state.buffer);
    } catch {
      if (invocation.guardError !== null) throw invocation.guardError;
      state.ioFailure = Object.freeze({
        kind: 'io-error',
        reader,
        ...COMMAND_INPUT_IO_ERROR,
      });
      return state.ioFailure;
    } finally {
      state.active = false;
      invocation.active = false;
    }

    assertHealthy();
    let validated;
    try {
      validated = validateReadResult(rawResult);
    } catch {
      if (invocation.guardError !== null) throw invocation.guardError;
      poison('readInto result could not be authenticated');
    }
    const { length, eof } = validated;
    if (eof) {
      state.eof = true;
      state.eofResult = Object.freeze({ kind: 'eof', reader });
      return state.eofResult;
    }

    state.length = length;
    const generation = state.generation;
    let view;
    view = {};
    Object.defineProperties(view, {
      byteLength: {
        enumerable: true,
        get() {
          if (this !== view) poison('chunk length accessor was detached');
          currentView(view, state, generation);
          return length;
        },
      },
      byteAt: {
        enumerable: true,
        value(index) {
          if (this !== view) poison('chunk byte accessor was detached');
          currentView(view, state, generation);
          if (!Number.isInteger(index) || index < 0 || index >= length) {
            throw new RangeError('chunk byte index is outside the current view');
          }
          return state.buffer[index];
        },
      },
    });
    Object.freeze(view);
    invocation.stateByView.set(view, { state, generation });
    return Object.freeze({ kind: 'chunk', reader, chunk: view });
  };

  const drop = reader => {
    const state = stateForReader(reader);
    if (invocation.active || state.active) poison('reader cannot settle during an operation');
    state.generation += 1;
    state.length = 0;
    state.dropped = true;
  };

  return Object.freeze({ open, next, drop });
}
