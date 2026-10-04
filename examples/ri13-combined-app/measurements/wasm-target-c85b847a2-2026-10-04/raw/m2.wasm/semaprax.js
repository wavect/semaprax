const SPX_MIN = -(1n << 63n);
const SPX_MAX = (1n << 63n) - 1n;
const SPX_POISON_I64 = 0x5a5a5a5a5a5a5a5an;
const SPX_POISON_HANDLE = 0x5a5a5a5a;
const SPX_MAX_RUNTIME_TAG = 0x7ff;
const SPX_MAX_SLOT = 0x3ff;
const SPX_MAX_GENERATION = 0x3ff;
const SPX_MAX_DYNAMIC_STATUS = 0x7ffffffe;
const SPX_EXHAUSTED_STATUS = 0x7fffffff;
const SPX_OWNED_EXPORTS = Object.freeze({});
export const wasmSha256 = "4707bbe39a5773fc894ca02e7198cdb1ce6b97f3fec15a4e8bbc219b5657f0bb";
const SPX_WASM_SHA256 = wasmSha256;
class SpxSemanticFailure extends RangeError {
  constructor(domainId, code, message) { super(message); this.domainId = domainId; this.code = code; }
}
export function semanticStatus(error) {
  return error instanceof SpxSemanticFailure
    ? Object.freeze({ schema: "semaprax.status.v1", domain_id: error.domainId, code: error.code })
    : null;
}
const SPX_RUNTIME_TAG_ALLOCATOR_KEY = Symbol.for("semaprax.wasm-owned.runtime-tags.v1");
const spxLocalRuntimeTags = new Set();

function runtimeTagAllocator() {
  const installed = globalThis[SPX_RUNTIME_TAG_ALLOCATOR_KEY];
  if (installed !== undefined) {
    if (typeof installed !== "object" || installed === null || typeof installed.take !== "function") {
      throw new Error("SEMAPRAX runtime-tag allocator global is invalid");
    }
    return installed;
  }
  let next = 1;
  const allocator = Object.freeze({
    take() {
      if (next > SPX_MAX_RUNTIME_TAG) {
        throw new Error("SEMAPRAX owned runtime instance identity space exhausted");
      }
      return next++;
    },
  });
  Object.defineProperty(globalThis, SPX_RUNTIME_TAG_ALLOCATOR_KEY, {
    value: allocator,
    configurable: false,
    enumerable: false,
    writable: false,
  });
  return allocator;
}

async function authenticatedWasmBytes(bytes) {
  let source;
  if (bytes instanceof ArrayBuffer) {
    source = new Uint8Array(bytes);
  } else if (ArrayBuffer.isView(bytes)) {
    source = new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  } else {
    throw new TypeError("SEMAPRAX instantiateBytes requires an ArrayBuffer or typed-array view");
  }
  const ownedCopy = new Uint8Array(source);
  if (globalThis.crypto === undefined || globalThis.crypto.subtle === undefined) {
    throw new Error("SEMAPRAX Web Crypto SHA-256 support is required");
  }
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", ownedCopy));
  const actual = Array.from(digest, byte => byte.toString(16).padStart(2, "0")).join("");
  if (actual !== SPX_WASM_SHA256) {
    throw new Error("SEMAPRAX WebAssembly artifact authentication failed");
  }
  return ownedCopy;
}

function checked(value, operation) {
  if (value < SPX_MIN || value > SPX_MAX) {
    throw new SpxSemanticFailure("semaprax.arithmetic.v1", ({ "addition overflow": 1, "subtraction overflow": 2, "multiplication overflow": 3, "negation overflow": 8 })[operation], `SEMAPRAX checked arithmetic failure: ${operation}`);
  }
  return value;
}

function createByteDataRuntime(options = {}) {
  const FIXED_MEMORY_BYTES = 131072;
  const OWNED_UTF8_LITERAL_BASE = 196608;
  const OWNED_UTF8_MEMORY_BYTES = 262144;
  const entries = new Map();
  const maxLiveEntries = boundedLimit(options.maxOwnedByteEntries, 16, "owned-byte-entry");
  const encoder = new TextEncoder();
  let nextToken = 1;
  let instance = null;
  const decode = carrier => {
    if (typeof carrier !== "bigint") throw new TypeError("SEMAPRAX byte carrier is not i64");
    const word = BigInt.asUintN(64, carrier);
    const length = Number(word & 0xffffffffn);
    const root = Number((word >> 32n) & 0xffffffffn);
    // Plain fixed-memory roots are invocation inputs and retain the 64 KiB
    // external-root limit. Owned handles and their checked range descriptors
    // may carry the larger internal `Bytes` result bound.
    const maximum = (root & 0xc0000000) === 0 ? 65536 : 131072;
    if (length > maximum) throw new Error("SEMAPRAX byte carrier length invariant");
    return { carrier: word, length, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff };
  };
  const memory = () => {
    const candidate = instance?.exports.__spx_byte_memory;
    if (!(candidate instanceof WebAssembly.Memory)
        || (candidate.buffer.byteLength !== FIXED_MEMORY_BYTES
            && candidate.buffer.byteLength !== OWNED_UTF8_MEMORY_BYTES)) {
      throw new Error("SEMAPRAX fixed byte memory invariant");
    }
    return new Uint8Array(candidate.buffer);
  };
  const resolve = decoded => {
    if (!decoded.tagged || decoded.token === 0) throw new Error("SEMAPRAX owned Bytes token invariant");
    const entry = entries.get(decoded.token);
    if (!(entry instanceof Uint8Array) || entry.byteLength !== decoded.length) {
      throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
    }
    return entry;
  };
  const fixed = decoded => {
    const bytes = memory();
    if (decoded.root <= FIXED_MEMORY_BYTES - decoded.length) {
      return bytes.slice(decoded.root, decoded.root + decoded.length);
    }
    // Aggregate owned String literals are compiler-authored data in the fourth
    // fixed page. Only the exact 256 KiB String profile can address this
    // declared table; all ordinary borrowed Byte roots remain in pages 0..1.
    if (bytes.byteLength === OWNED_UTF8_MEMORY_BYTES
        && decoded.root >= OWNED_UTF8_LITERAL_BASE
        && decoded.root < OWNED_UTF8_MEMORY_BYTES
        && decoded.root <= OWNED_UTF8_MEMORY_BYTES - decoded.length) {
      return bytes.slice(decoded.root, decoded.root + decoded.length);
    }
    throw new Error("SEMAPRAX fixed byte range invariant");
  };
  const read = decoded => {
    if (decoded.tagged) return resolve(decoded);
    if ((decoded.root & 0xc0000000) === 0x40000000) {
      // The guest validates the descriptor against its private binding globals
      // immediately before this synchronous import. The adapter independently
      // replays the carrier-to-memory identity, shape, and extent checks before
      // it reads either guest memory or an authenticated owned entry.
      const pointer = (decoded.root & 0xffff) * 8;
      if (pointer > FIXED_MEMORY_BYTES - 32) throw new Error("SEMAPRAX byte range descriptor bounds invariant");
      const bytes = memory();
      const descriptor = new DataView(bytes.buffer, bytes.byteOffset + pointer, 32);
      const identity = descriptor.getUint32(0, true);
      const self = descriptor.getUint32(4, true);
      const carrierIdentity = (decoded.root >>> 16) & 0x1fff;
      if (identity === 0 || identity !== carrierIdentity || self !== pointer) {
        throw new Error("SEMAPRAX byte range descriptor identity invariant");
      }
      const ultimate = decode(descriptor.getBigInt64(8, true));
      const offset = descriptor.getBigUint64(16, true);
      const length = descriptor.getBigUint64(24, true);
      if (length !== BigInt(decoded.length)) throw new Error("SEMAPRAX byte range descriptor length invariant");
      if ((ultimate.root & 0xc0000000) === 0x40000000) {
        throw new Error("SEMAPRAX nested byte range descriptor invariant");
      }
      if (offset > BigInt(ultimate.length) || length > BigInt(ultimate.length) - offset) {
        throw new Error("SEMAPRAX byte range descriptor extent invariant");
      }
      const root = ultimate.tagged ? resolve(ultimate) : fixed(ultimate);
      const start = Number(offset);
      return root.slice(start, start + Number(length));
    }
    return fixed(decoded);
  };
  const validUtf8 = bytes => {
    for (let index = 0; index < bytes.length;) {
      const first = bytes[index++];
      let extra, minimum, scalar;
      if (first < 128) continue;
      if (first >= 194 && first <= 223) { extra = 1; minimum = 128; scalar = first & 31; }
      else if (first >= 224 && first <= 239) { extra = 2; minimum = 2048; scalar = first & 15; }
      else if (first >= 240 && first <= 244) { extra = 3; minimum = 65536; scalar = first & 7; }
      else return false;
      if (index + extra > bytes.length) return false;
      for (let count = 0; count < extra; count++) {
        const byte = bytes[index++];
        if ((byte & 192) !== 128) return false;
        scalar = (scalar << 6) | (byte & 63);
      }
      if (scalar < minimum || scalar > 1114111 || (scalar >= 55296 && scalar <= 57343)) return false;
    }
    return true;
  };
  const stringBytes = carrier => {
    const bytes = read(decode(carrier));
    if (!validUtf8(bytes)) throw new Error("SEMAPRAX String UTF-8 invariant");
    return bytes;
  };
  const allocate = bytes => {
    if (!(bytes instanceof Uint8Array) || bytes.byteLength > 131072) {
      throw new Error("SEMAPRAX owned Bytes length invariant");
    }
    if (entries.size >= maxLiveEntries) throw new Error("SEMAPRAX owned Bytes live entry limit exceeded");
    if (nextToken > 0x7fffffff) throw new Error("SEMAPRAX owned Bytes token space exhausted");
    const token = nextToken++;
    const owned = new Uint8Array(bytes);
    entries.set(token, owned);
    const root = 0x80000000n | BigInt(token);
    return BigInt.asIntN(64, (root << 32n) | BigInt(owned.byteLength));
  };
  const textNumber = (value, unsigned) => {
    if (typeof value !== "bigint") throw new TypeError("SEMAPRAX numeric String input is not i64");
    if (BigInt.asIntN(64, value) !== value) throw new Error("SEMAPRAX numeric String input range invariant");
    return allocate(encoder.encode((unsigned ? BigInt.asUintN(64, value) : value).toString()));
  };
  const beginsWith = (value, prefix) => {
    if (prefix.byteLength > value.byteLength) return 0;
    for (let index = 0; index < prefix.byteLength; index++) {
      if (value[index] !== prefix[index]) return 0;
    }
    return 1;
  };
  const contains = (value, needle) => {
    if (needle.byteLength === 0) return 1;
    if (needle.byteLength > value.byteLength) return 0;
    for (let start = 0; start <= value.byteLength - needle.byteLength; start++) {
      let index = 0;
      while (index < needle.byteLength && value[start + index] === needle[index]) index++;
      if (index === needle.byteLength) return 1;
    }
    return 0;
  };
  const byteImports = Object.freeze({
    spx_bytes_copy: carrier => allocate(read(decode(carrier))),
    spx_bytes_zeroed: count => {
      if (typeof count !== "bigint" || count < 0n || count > 131072n) {
        throw new Error("SEMAPRAX owned byte buffer capacity invariant");
      }
      return allocate(new Uint8Array(Number(count)));
    },
    spx_bytes_set: (carrier, index, value) => {
      const decoded = decode(carrier);
      const bytes = resolve(decoded);
      if (typeof index !== "bigint" || index < 0n || index >= BigInt(bytes.byteLength)
          || !Number.isInteger(value) || value < 0 || value > 255) {
        throw new Error("SEMAPRAX owned byte buffer element invariant");
      }
      bytes[Number(index)] = value;
      return BigInt.asIntN(64, decoded.carrier);
    },
    spx_bytes_set5: (carrier, index, first, second, third, fourth, fifth) => {
      const decoded = decode(carrier);
      const bytes = resolve(decoded);
      if (typeof index !== "bigint" || index < 0n
          || index > BigInt(bytes.byteLength)
          || BigInt(bytes.byteLength) - index < 5n
          || ![first, second, third, fourth, fifth].every(
            value => Number.isInteger(value) && value >= 0 && value <= 255
          )) {
        throw new Error("SEMAPRAX owned byte buffer five-byte interval invariant");
      }
      const slot = Number(index);
      bytes[slot] = first;
      bytes[slot + 1] = second;
      bytes[slot + 2] = third;
      bytes[slot + 3] = fourth;
      bytes[slot + 4] = fifth;
      return BigInt.asIntN(64, decoded.carrier);
    },
    spx_bytes_set1_or5: (carrier, index, one, sourceCarrier, selector) => {
      const decoded = decode(carrier);
      const bytes = resolve(decoded);
      const source = read(decode(sourceCarrier));
      if (typeof index !== "bigint" || index < 0n || typeof selector !== "bigint"
          || !Number.isInteger(one) || one < 0 || one > 255) {
        throw new Error("SEMAPRAX owned byte buffer one-or-five invariant");
      }
      const selectorBits = BigInt.asUintN(64, selector);
      const wide = (selectorBits & (1n << 63n)) !== 0n;
      const sourceStart = selectorBits & ((1n << 63n) - 1n);
      const width = wide ? 5n : 1n;
      if (index > BigInt(bytes.byteLength) || BigInt(bytes.byteLength) - index < width) {
        throw new Error("SEMAPRAX owned byte buffer one-or-five interval invariant");
      }
      const slot = Number(index);
      if (!wide) {
        bytes[slot] = one;
      } else {
        for (let offset = 0; offset < 5; offset += 1) {
          const sourceIndex = sourceStart + BigInt(offset);
          bytes[slot + offset] = sourceIndex < BigInt(source.byteLength)
            ? source[Number(sourceIndex)] : 0;
        }
      }
      return BigInt.asIntN(64, decoded.carrier);
    },
    spx_bytes_set1_or6_or48: (carrier, index, one, sourceCarrier, selector) => {
      const decoded = decode(carrier);
      const bytes = resolve(decoded);
      const source = read(decode(sourceCarrier));
      if (typeof index !== "bigint" || index < 0n || typeof selector !== "bigint"
          || !Number.isInteger(one) || one < 0 || one > 255) {
        throw new Error("SEMAPRAX owned byte buffer one-or-six-or-forty-eight invariant");
      }
      const selectorBits = BigInt.asUintN(64, selector);
      const copy = (selectorBits & (1n << 63n)) !== 0n;
      const wide48 = (selectorBits & (1n << 62n)) !== 0n;
      const sourceStart = selectorBits & ((1n << 62n) - 1n);
      const width = copy ? (wide48 ? 48n : 6n) : 1n;
      if (index > BigInt(bytes.byteLength) || BigInt(bytes.byteLength) - index < width) {
        throw new Error("SEMAPRAX owned byte buffer one-or-six-or-forty-eight interval invariant");
      }
      const slot = Number(index);
      if (!copy) {
        bytes[slot] = one;
      } else {
        for (let offset = 0; offset < Number(width); offset += 1) {
          const sourceIndex = sourceStart + BigInt(offset);
          bytes[slot + offset] = sourceIndex < BigInt(source.byteLength)
            ? source[Number(sourceIndex)] : 0;
        }
      }
      return BigInt.asIntN(64, decoded.carrier);
    },
    spx_bytes_get: (carrier, index) => {
      const bytes = read(decode(carrier));
      const unsigned = BigInt.asUintN(64, index);
      return unsigned >= BigInt(bytes.byteLength) ? -1 : bytes[Number(unsigned)];
    },
    spx_bytes_drop: carrier => {
      const decoded = decode(carrier);
      resolve(decoded);
      entries.delete(decoded.token);
    },
    spx_bytes_as_slice: carrier => {
      const decoded = decode(carrier);
      if (decoded.tagged) resolve(decoded); else read(decoded);
      return BigInt.asIntN(64, decoded.carrier);
    },
    // String import inputs are authenticated carrier values. Every operation
    // validates UTF-8 before inspection or publication; concat leaves its
    // committed inputs intact for the canonical emitted drop transitions.
    spx_string_concat_v1: (left, right) => {
      const leftBytes = stringBytes(left);
      const rightBytes = stringBytes(right);
      if (leftBytes.byteLength > 65536 - rightBytes.byteLength) {
        throw new Error("SEMAPRAX String concatenation capacity invariant");
      }
      const joined = new Uint8Array(leftBytes.byteLength + rightBytes.byteLength);
      joined.set(leftBytes, 0);
      joined.set(rightBytes, leftBytes.byteLength);
      return allocate(joined);
    },
    spx_string_from_char_v1: scalar => {
      if (!Number.isInteger(scalar) || scalar < 0 || scalar > 1114111 || (scalar >= 55296 && scalar <= 57343)) {
        throw new Error("SEMAPRAX String character scalar invariant");
      }
      return allocate(encoder.encode(String.fromCodePoint(scalar)));
    },
    spx_string_len_chars_v1: carrier => {
      const bytes = stringBytes(carrier);
      let count = 0;
      for (const byte of bytes) if ((byte & 192) !== 128) count++;
      return BigInt(count);
    },
    spx_string_from_i64_v1: value => textNumber(value, false),
    spx_string_from_usize_v1: value => textNumber(value, true),
    spx_string_starts_with_v1: (value, prefix) => beginsWith(stringBytes(value), stringBytes(prefix)),
    spx_string_contains_v1: (value, needle) => contains(stringBytes(value), stringBytes(needle)),
  });
  return Object.freeze({
    imports: byteImports,
    bind(wasmInstance) {
      if (instance !== null) throw new Error("SEMAPRAX byte runtime already bound");
      instance = wasmInstance;
    },
  });
}

const imports = {
  env: {
    spx_add: (a, b) => checked(a + b, "addition overflow"),
    spx_sub: (a, b) => checked(a - b, "subtraction overflow"),
    spx_mul: (a, b) => checked(a * b, "multiplication overflow"),
    spx_div: (a, b) => {
      if (b === 0n) throw new SpxSemanticFailure("semaprax.arithmetic.v1", 4, "SEMAPRAX checked arithmetic failure: invalid division");
      if (a === SPX_MIN && b === -1n) throw new SpxSemanticFailure("semaprax.arithmetic.v1", 5, "SEMAPRAX checked arithmetic failure: invalid division");
      return a / b;
    },
    spx_rem: (a, b) => {
      if (b === 0n) throw new SpxSemanticFailure("semaprax.arithmetic.v1", 6, "SEMAPRAX checked arithmetic failure: invalid remainder");
      if (a === SPX_MIN && b === -1n) throw new SpxSemanticFailure("semaprax.arithmetic.v1", 7, "SEMAPRAX checked arithmetic failure: invalid remainder");
      return a % b;
    },
    spx_neg: value => checked(-value, "negation overflow"),
    spx_contract_fail: code => {
      if (code === 9) throw new SpxSemanticFailure("semaprax.contract.v1", 1, "SEMAPRAX contract failure");
      if (code === 10) throw new SpxSemanticFailure("semaprax.contract.v1", 2, "SEMAPRAX contract failure");
      // Call-depth admission (issue #293 P2-2), shared with the legacy
      // scalar-core Wasm emitter's own `spx_contract_fail` calls: the same
      // wire value `aggregate::call_admission::CALL_DEPTH_STATUS` (18)
      // normalizes to the identical status the native C11 backend's
      // `spx_rt_call_depth_failure` reports.
      if (code === 18) throw new SpxSemanticFailure("semaprax.runtime.v1", 1, "SEMAPRAX call-depth admission failure");
      if (code === 11) throw new SpxSemanticFailure("semaprax.byte-range.v1", 1, "SEMAPRAX byte range failure");
      if (code === 12) throw new SpxSemanticFailure("semaprax.byte-range.v1", 2, "SEMAPRAX byte range failure");
      if (code === 16) throw new SpxSemanticFailure("semaprax.byte-buffer.v1", 1, "SEMAPRAX owned byte buffer failure");
      throw new SpxSemanticFailure("semaprax.contract.v1", code, "SEMAPRAX contract failure");
    },
  },
};

function boundedLimit(value, maximum, name) {
  if (value === undefined) return maximum;
  if (!Number.isSafeInteger(value) || value < 1 || value > maximum) {
    throw new RangeError(`invalid SEMAPRAX ${name} limit`);
  }
  return value;
}

function createOwnedRuntime(options = {}) {
  const maxSlot = boundedLimit(options.maxOwnedSlots, SPX_MAX_SLOT, "owned-slot");
  const maxDynamicStatus = boundedLimit(options.maxStatusTokens, SPX_MAX_DYNAMIC_STATUS, "status-token");
  const runtimeTag = runtimeTagAllocator().take();
  if (!Number.isInteger(runtimeTag) || runtimeTag < 1 || runtimeTag > SPX_MAX_RUNTIME_TAG
      || spxLocalRuntimeTags.has(runtimeTag)) {
    throw new Error("SEMAPRAX runtime-tag allocator returned an invalid or repeated identity");
  }
  spxLocalRuntimeTags.add(runtimeTag);
  const context = ((runtimeTag << 20) | 0x5350) | 0;
  const slots = new Map();
  const generations = new Map();
  const freeSlots = [];
  const statuses = new Map();
  statuses.set(SPX_EXHAUSTED_STATUS, Object.freeze({
    schema: "semaprax.status.v1",
    domain_id: "semaprax.wasm-adapter.v1",
    code: 5,
    class: "adapter",
    retryable: false,
  }));
  const events = [];
  const adoptionTickets = new WeakMap();
  let nextSlot = 1;
  let nextStatus = 1;
  let staging = null;
  let activeResult = null;
  let activeStatus = null;
  let semanticInvocation = null;
  let instance = null;

  const recordStatus = (domain, code, classification) => {
    if (nextStatus > maxDynamicStatus) return SPX_EXHAUSTED_STATUS;
    const token = nextStatus++;
    statuses.set(token, Object.freeze({
      schema: "semaprax.status.v1",
      domain_id: domain,
      code,
      class: classification,
      retryable: false,
    }));
    return token;
  };
  const fillStatus = (status, domain, code, classification) => {
    status.domain_id = domain;
    status.code = code;
    status.class = classification;
    Object.freeze(status);
  };
  const adapterFailure = code => {
    if (staging !== null) {
      fillStatus(staging.status, "semaprax.wasm-adapter.v1", code, "adapter");
      staging.retainStatus = true;
      return staging.statusToken;
    }
    if (activeStatus !== null) {
      fillStatus(activeStatus.status, "semaprax.wasm-adapter.v1", code, "adapter");
      const token = activeStatus.token;
      activeStatus = null;
      return token;
    }
    return recordStatus("semaprax.wasm-adapter.v1", code, "adapter");
  };
  const requireContext = candidate => candidate === context;
  const reserveSlot = (value, state) => {
    let slot;
    let generation;
    while (freeSlots.length > 0) {
      slot = freeSlots.pop();
      generation = (generations.get(slot) ?? 0) + 1;
      if (generation <= SPX_MAX_GENERATION) break;
      slot = undefined;
    }
    if (slot === undefined) {
      if (nextSlot > maxSlot) throw new Error("SEMAPRAX owned handle table exhausted");
      slot = nextSlot++;
      generation = 1;
    }
    generations.set(slot, generation);
    const handle = ((runtimeTag << 20) | (generation << 10) | slot) | 0;
    if (handle === 0 || slots.has(handle)) throw new Error("SEMAPRAX handle allocation invariant");
    const entry = { slot, generation, value, state };
    slots.set(handle, entry);
    return { handle, entry };
  };
  const allocate = value => reserveSlot(value, "owned").handle;
  const release = (handle, expected) => {
    const entry = slots.get(handle);
    if (!entry || entry.state !== expected) throw new Error("SEMAPRAX owned runtime invariant");
    slots.delete(handle);
    freeSlots.push(entry.slot);
    return entry;
  };

  const ownedImports = {
    spx_owned_begin: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging !== null || activeStatus !== null || activeResult !== null) return adapterFailure(2);
      if (nextStatus > maxDynamicStatus) return SPX_EXHAUSTED_STATUS;
      const statusToken = nextStatus++;
      const status = {
        schema: "semaprax.status.v1",
        domain_id: null,
        code: 0,
        class: null,
        retryable: false,
      };
      statuses.set(statusToken, status);
      staging = { handles: [], result: null, statusToken, status, retainStatus: false };
      return 0;
    },
    spx_owned_stage: (candidate, handle) => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null) return adapterFailure(2);
      const entry = slots.get(handle);
      if (!entry || entry.state !== "owned") return adapterFailure(3);
      if (staging.handles.includes(handle)) return adapterFailure(4);
      staging.handles.push(handle);
      return 0;
    },
    spx_owned_abort: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned abort context invariant");
      if (staging !== null && staging.result !== null) release(staging.result, "reserved");
      if (staging !== null && !staging.retainStatus) statuses.delete(staging.statusToken);
      staging = null;
    },
    spx_owned_reserve_result: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null || staging.result !== null) return adapterFailure(2);
      try {
        staging.result = reserveSlot(undefined, "reserved").handle;
      } catch (error) {
        if (error instanceof Error && error.message === "SEMAPRAX owned handle table exhausted") {
          return adapterFailure(5);
        }
        throw error;
      }
      return 0;
    },
    spx_owned_commit: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null) return adapterFailure(2);
      for (const handle of staging.handles) {
        const entry = slots.get(handle);
        if (!entry || entry.state !== "owned") return adapterFailure(3);
      }
      for (const handle of staging.handles) slots.get(handle).state = "inflight";
      activeResult = staging.result;
      activeStatus = { token: staging.statusToken, status: staging.status };
      events.push(Object.freeze({ kind: "commit", handles: Object.freeze([...staging.handles]) }));
      staging = null;
      return 0;
    },
    spx_owned_drop: (candidate, handle) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned drop context invariant");
      release(handle, "inflight");
      events.push(Object.freeze({ kind: "drop", handle }));
    },
    spx_owned_cancel_result: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned cancel context invariant");
      if (activeResult === null) throw new Error("SEMAPRAX result reservation invariant");
      release(activeResult, "reserved");
      activeResult = null;
    },
    spx_owned_publish: (candidate, handle) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned publish context invariant");
      const entry = release(handle, "inflight");
      if (activeResult === null) throw new Error("SEMAPRAX result publication reservation invariant");
      const published = activeResult;
      const reserved = slots.get(published);
      if (!reserved || reserved.state !== "reserved") throw new Error("SEMAPRAX reserved result invariant");
      reserved.value = entry.value;
      reserved.state = "owned";
      activeResult = null;
      events.push(Object.freeze({ kind: "publish", from: handle, to: published }));
      return published;
    },
    spx_status_record: (candidate, classification, code) => {
      if (!requireContext(candidate)) return adapterFailure(1);
      const target = staging ?? activeStatus;
      if (target === null) throw new Error("SEMAPRAX status reservation invariant");
      let domain;
      let statusClass;
      if (classification === 1 || classification === 2) {
        domain = "semaprax.contract.v1";
        statusClass = "contract";
      } else if (classification === 3) {
        domain = "semaprax.arithmetic.v1";
        statusClass = "arithmetic";
      } else if (classification === 4) {
        domain = "semaprax.wasm-adapter.v1";
        statusClass = "adapter";
      } else {
        throw new Error("SEMAPRAX compiler status classification invariant");
      }
      fillStatus(target.status, domain, code, statusClass);
      events.push(Object.freeze({ kind: "status", domain_id: domain, code, class: statusClass }));
      const token = target.token ?? target.statusToken;
      if (staging !== null) staging.retainStatus = true;
      else activeStatus = null;
      return token;
    },
    spx_owned_success: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned success context invariant");
      if (activeStatus === null || activeResult !== null) throw new Error("SEMAPRAX success reservation invariant");
      statuses.delete(activeStatus.token);
      activeStatus = null;
    },
    spx_semantic_event: (candidate, functionOrdinal, eventOrdinal) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX semantic event context invariant");
      if (semanticInvocation === null) throw new Error("SEMAPRAX semantic event outside invocation");
      const contract = semanticInvocation.contract;
      if (functionOrdinal !== contract.function_ordinal
          || !contract.valid_ordinals.includes(eventOrdinal)
          || eventOrdinal === 0) {
        throw new Error("SEMAPRAX semantic event dictionary invariant");
      }
      semanticInvocation.ordinals.push(eventOrdinal);
    },
  };

  const facade = Object.freeze({
    prepareTrustedAdoption(value) {
      const ticket = Object.freeze(Object.create(null));
      adoptionTickets.set(ticket, { consumed: false, value });
      return ticket;
    },
    adopt(ticket) {
      const adoption = adoptionTickets.get(ticket);
      if (adoption === undefined || adoption.consumed) {
        throw new TypeError("SEMAPRAX adoption ticket is invalid or already consumed");
      }
      const handle = allocate(adoption.value);
      adoption.consumed = true;
      adoption.value = undefined;
      return handle;
    },
    dispose(handle) {
      if (!Number.isInteger(handle) || handle === 0) {
        throw new TypeError("SEMAPRAX owned handle is invalid");
      }
      release(handle, "owned");
      events.push(Object.freeze({ kind: "drop", handle }));
    },
    invoke(exportName, args, resultKind) {
      if (instance === null) throw new Error("SEMAPRAX owned runtime is not bound");
      if (typeof exportName !== "string") {
        throw new TypeError("SEMAPRAX owned export name must be a string");
      }
      if (!Object.hasOwn(SPX_OWNED_EXPORTS, exportName)) {
        throw new TypeError(`unknown SEMAPRAX owned export: ${exportName}`);
      }
      const contract = SPX_OWNED_EXPORTS[exportName];
      if (resultKind !== contract.result) {
        throw new TypeError(`SEMAPRAX owned export ${exportName} requires result kind ${contract.result}`);
      }
      if (!Array.isArray(args) || args.length !== contract.parameters.length) {
        throw new TypeError(`SEMAPRAX owned export ${exportName} argument count mismatch`);
      }
      const canonicalArgs = [];
      for (let index = 0; index < contract.parameters.length; index += 1) {
        const kind = contract.parameters[index];
        const value = args[index];
        const valid = kind === "i64" ? typeof value === "bigint" && value >= SPX_MIN && value <= SPX_MAX
          : kind === "bool" ? Number.isInteger(value) && (value === 0 || value === 1)
          : kind === "resource" ? Number.isInteger(value) && value >= 1 && value <= 0x7fffffff
          : false;
        if (!valid) throw new TypeError(`SEMAPRAX owned export ${exportName} argument ${index} kind mismatch`);
        canonicalArgs.push(value);
      }
      const fn = instance.exports[exportName];
      if (typeof fn !== "function") throw new Error(`missing SEMAPRAX owned export: ${exportName}`);
      const memory = instance.exports.memory;
      if (!(memory instanceof WebAssembly.Memory)) throw new Error("SEMAPRAX owned memory export is absent");
      const view = new DataView(memory.buffer);
      if (resultKind === "i64") view.setBigInt64(0, SPX_POISON_I64, true);
      else view.setInt32(0, SPX_POISON_HANDLE, true);
      const callArgs = [context];
      for (let index = 0; index < canonicalArgs.length; index += 1) {
        callArgs.push(canonicalArgs[index]);
      }
      callArgs.push(0);
      semanticInvocation = { contract, ordinals: [] };
      let statusToken;
      try {
        statusToken = Reflect.apply(fn, undefined, callArgs);
      } catch (error) {
        semanticInvocation = null;
        throw error;
      }
      const semantic = Object.freeze({
        schema: contract.dictionary_schema,
        function: contract.function,
        dictionary_fingerprint: contract.dictionary_fingerprint,
        ordinals: Object.freeze([...semanticInvocation.ordinals]),
      });
      semanticInvocation = null;
      if (statusToken !== 0) {
        const preserved = resultKind === "i64"
          ? view.getBigInt64(0, true) === SPX_POISON_I64
          : view.getInt32(0, true) === SPX_POISON_HANDLE;
        if (!preserved) throw new Error("SEMAPRAX failure published a poisoned result slot");
        const status = statuses.get(statusToken);
        if (!status) throw new Error("SEMAPRAX returned an unknown status token");
        return Object.freeze({ ok: false, published: false, statusToken, status, semantic });
      }
      const value = resultKind === "i64" ? view.getBigInt64(0, true) : view.getInt32(0, true);
      return Object.freeze({ ok: true, published: true, value, semantic });
    },
    resolveStatus(token) {
      return statuses.get(token) ?? null;
    },
    trace() {
      return events.map(event => ({ ...event, handles: event.handles ? [...event.handles] : undefined }));
    },
    liveHandleCount() {
      return slots.size;
    },
  });

  return Object.freeze({
    linkImports: Object.freeze({ env: Object.freeze(ownedImports) }),
    bind(wasmInstance) {
      if (instance !== null) throw new Error("SEMAPRAX owned runtime already bound");
      instance = wasmInstance;
    },
    facade,
  });
}

export async function instantiateBytes(bytes, options = {}) {
  const authenticatedBytes = await authenticatedWasmBytes(bytes);
  const byteRuntime = createByteDataRuntime(options);
  if (Object.keys(SPX_OWNED_EXPORTS).length === 0) {
    const linkedImports = { env: { ...imports.env, ...byteRuntime.imports } };
    const result = await WebAssembly.instantiate(authenticatedBytes, linkedImports);
    byteRuntime.bind(result.instance);
    return Object.freeze(result);
  }
  const runtime = createOwnedRuntime(options);
  const linkedImports = { env: { ...imports.env, ...byteRuntime.imports, ...runtime.linkImports.env } };
  const result = await WebAssembly.instantiate(authenticatedBytes, linkedImports);
  byteRuntime.bind(result.instance);
  runtime.bind(result.instance);
  return Object.freeze({ ...result, owned: runtime.facade });
}

export async function instantiate(url = new URL("./app.wasm", import.meta.url)) {
  const response = await fetch(url);
  return instantiateBytes(await response.arrayBuffer());
}
