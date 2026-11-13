// Private strict Core-Wasm host for ordinary Bytes, Vec<Bytes>, Bytes
// iterators and Box<Bytes> with lease-aware consumers (OPT #746 consumer
// closure).
//
// Every import keeps the frozen ordinary `env` name, signature and two-word
// carrier. A carrier resolves either to an ordinary whole owner in the fixed
// whole-owner table or to a live lease in the shared-owner registry; nothing
// else is readable, writable or droppable. This is not the public browser
// adapter: public routes keep their own token tables and refuse lease
// carriers as stale. Ordinary admission is closed. The private source-checked
// generated module alone declares the partition producer import. The
// witness facade exists only for the owning regressions until the
// cross-backend join admits `bytes_partition`.
//
// `payload` exports the same read/allocate/drop hooks to a joined owning host
// (record/leaf Vec, iterator and sort hosts), so their payload consumers use
// this table and quota instead of a host-local map.
import {
  MAX_BACKINGS,
  MAX_BYTES_EXTENT,
  MAX_TOKEN,
  PARTITION_STATUS_CODES,
  carrierOf,
  createSharedByteOwnerRegistry,
  decodeOwnedCarrier,
} from "./registry.mjs";

// Existing source-derived limits, mirrored as defense in depth. They are not
// raised, and a partition charges neither: it issues leases over a backing
// that its constructor already paid for.
export const MAX_BACKING_EVENTS = 32;
export const MAX_PAYLOAD_BYTES = 2097152;
export const MAX_VEC_CAPACITY = 8192n;
const OWNED_BYTES_TAG = 9;
const FIXED_MEMORY_BYTES = 131072;
// Each whole owner was charged as one allocation event of the current
// invocation, and every owner settles before a fresh invocation, so the
// existing event limit funds this fixed table exactly.
const WHOLE_SLOTS = MAX_BACKING_EVENTS;
if (MAX_BACKINGS !== MAX_BACKING_EVENTS) throw new Error("SEMAPRAX shared owner funding invariant");

// Ordinary consumers a joined owning host takes from this host. Vec<Bytes>
// and Bytes iterator authorities stay with whichever host owns the Vec table.
export const PAYLOAD_CONSUMERS = Object.freeze([
  "spx_bytes_zeroed",
  "spx_bytes_copy",
  "spx_bytes_get",
  "spx_bytes_as_slice",
  "spx_bytes_drop",
  "spx_bytes_set",
  "spx_bytes_set5",
  "spx_bytes_set1_or5",
  "spx_bytes_set1_or6_or48",
  "spx_string_from_utf8_v1",
  "spx_string_from_str_v2",
  "spx_string_len_chars_v1",
  "spx_string_starts_with_v1",
  "spx_string_contains_v1",
  "spx_string_compare_v2",
  "spx_box_new_v2",
  "spx_box_get_v2",
  "spx_box_into_inner_v2",
  "spx_box_drop_v2",
]);

// One never-reset token space and one never-reset handle space per evaluated
// module record, shared by every host it creates. Neither is installed on a
// global, so no other code can replace, rewind or observe them. A second
// evaluated copy refuses at the realm installation marker. Tokens and
// handles are therefore never reused across registries, hosts, instances or
// invocations of this module record; each table still only resolves what its
// own issuer recorded.
// Trusted first installation. Copies may not adopt a duck-typed namespace
// or start a second counter that could collide with live carriers. The marker
// contains no mint/lookup authority; all counters remain module-private.
const REALM_KEY = Symbol.for("semaprax.private.wasm-shared-byte-owner.realm.v1");
if (Object.prototype.hasOwnProperty.call(globalThis, REALM_KEY)) {
  throw new Error("SEMAPRAX shared owner namespace already installed");
}
Object.defineProperty(globalThis, REALM_KEY, {
  value: Object.freeze({}), writable: false, configurable: false, enumerable: false,
});
let nextToken = 1;
const TOKENS = Object.freeze({
  remaining: () => MAX_TOKEN - nextToken + 1,
  mint: () => {
    if (nextToken > MAX_TOKEN) throw new Error("SEMAPRAX owned Bytes token space exhausted");
    return nextToken++;
  },
});
let nextHandleSerial = 1n;
const nextHandle = () => {
  if (nextHandleSerial >= 1n << 63n) throw new Error("SEMAPRAX private handle space exhausted");
  return nextHandleSerial++;
};

export function createPrivateOrdinaryByteHost(options = {}) {
  let instance = null;
  // Invocation lifecycle: a serial that is never reset, and the receipt of
  // the current invocation (null while idle).
  let serial = 0;
  let current = null;
  const invocation = () => (current === null ? 0 : current.serial);
  const live = () => {
    if (current === null) throw new Error("SEMAPRAX private ordinary host has no live invocation");
    return current.serial;
  };
  // Optional private-entry account. Ordinary callers retain their old limits.
  // This funds requested typed-array metadata and live payload backing only.
  const managed = options.managedBytes !== undefined;
  if (managed && (!Number.isSafeInteger(options.managedBytes) || options.managedBytes < 1)) {
    throw new RangeError("SEMAPRAX managed-byte limit must be a positive safe integer");
  }
  let managedReserved = 0;
  let managedPeak = 0;
  const reserve = bytes => {
    if (!managed) return;
    if (bytes > options.managedBytes - managedReserved) {
      throw new Error("SEMAPRAX private managed-byte account exhausted");
    }
    managedReserved += bytes;
    managedPeak = Math.max(managedPeak, managedReserved);
  };
  const releasePayload = bytes => {
    if (!managed) return;
    if (bytes > managedReserved) throw new Error("SEMAPRAX managed-byte release invariant");
    managedReserved -= bytes;
  };
  reserve(WHOLE_SLOTS * 8);
  const shared = createSharedByteOwnerRegistry({
    tokens: TOKENS,
    invocation,
    maxLeaseSlots: options.maxLeaseSlots,
    reserveStorage: reserve,
    releasePayload,
  });
  // Witness-only: every nonempty owner this host allocates is returned as a
  // one-child lease of its own freshly charged backing. The charge is the
  // ordinary one; the guest cannot select this, and no installer may set it.
  const leaseWitness = options.leaseWitness === true;
  // Fixed whole-owner table.
  const wholeToken = new Uint32Array(WHOLE_SLOTS);
  const wholeInvocation = new Uint32Array(WHOLE_SLOTS);
  const wholeBytes = new Array(WHOLE_SLOTS).fill(null);
  let liveWhole = 0;
  // Vec, iterator and Box authorities mirror the existing ordinary routes:
  // they are host context overhead outside managed guest Bytes storage and
  // are not counted by the byte quota.
  const vectors = new Map();
  const iterators = new Map();
  const boxes = new Map();
  let events = 0;
  let payload = 0;
  let copies = 0;
  let drops = 0;

  const memory = () => {
    const candidate = instance?.exports.__spx_byte_memory ?? instance?.exports.memory;
    if (!(candidate instanceof WebAssembly.Memory)
        || (candidate.buffer.byteLength !== FIXED_MEMORY_BYTES
            && candidate.buffer.byteLength !== 262144)) {
      throw new Error("SEMAPRAX fixed byte memory invariant");
    }
    return candidate;
  };
  const wholeSlot = token => {
    for (let slot = 0; slot < WHOLE_SLOTS; slot += 1) if (wholeToken[slot] === token) return slot;
    return -1;
  };
  // Ordinary allocation-producing event: charged before the backing exists.
  const charge = length => {
    if (events >= MAX_BACKING_EVENTS || payload > MAX_PAYLOAD_BYTES - length) {
      throw new Error("SEMAPRAX ordinary Bytes allocation budget exceeded");
    }
    events += 1;
    payload += length;
  };
  // Resolves one whole owner of this host's current invocation, else -1.
  const whole = decoded => {
    if (!decoded.tagged || decoded.token === 0) return -1;
    const slot = wholeSlot(decoded.token);
    if (slot < 0) return -1;
    if (wholeInvocation[slot] !== live()) throw new Error("SEMAPRAX whole owner invocation invariant");
    return wholeBytes[slot].byteLength === decoded.extent ? slot : -1;
  };
  // Partition core: validation runs before any transfer; refusal leaves the
  // whole owner live and unchanged. A child, even at offset zero covering
  // its whole backing, is never an eligible input.
  const partition = (carrier, count, width) => {
    const decoded = decodeOwnedCarrier(carrier);
    if (!decoded.tagged) throw new Error("SEMAPRAX shared owner partition input invariant");
    const slot = whole(decoded);
    if (slot < 0) {
      if (shared.owns(carrier)) throw new Error("SEMAPRAX shared owner child partition refused");
      throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
    }
    const prepared = shared.prepare(wholeBytes[slot], count, width);
    if (prepared.refused !== undefined) return Object.freeze({ ok: false, refused: prepared.refused });
    let children;
    try { children = new Array(Number(count)); }
    catch (error) { shared.abort(prepared.ticket); throw error; }
    shared.commit(prepared.ticket, children);
    wholeToken[slot] = 0;
    wholeBytes[slot] = null;
    liveWhole -= 1;
    return Object.freeze({ ok: true, children: Object.freeze(children) });
  };
  const own = (bytes, fresh = false) => {
    const serialNow = live();
    const length = fresh ? bytes : (bytes instanceof Uint8Array ? bytes.byteLength : -1);
    if (!Number.isSafeInteger(length) || length < 0 || length > MAX_BYTES_EXTENT) {
      throw new Error("SEMAPRAX owned Bytes length invariant");
    }
    if (TOKENS.remaining() < 2) throw new Error("SEMAPRAX owned Bytes token space exhausted");
    const slot = wholeSlot(0);
    charge(length);
    if (slot < 0) throw new Error("SEMAPRAX whole owner funding invariant");
    // Fresh exact backing: clone and copy never alias a lease or its backing.
    reserve(length);
    const owned = new Uint8Array(length);
    if (!fresh) owned.set(bytes);
    const token = TOKENS.mint();
    wholeToken[slot] = token;
    wholeInvocation[slot] = serialNow;
    wholeBytes[slot] = owned;
    liveWhole += 1;
    copies += 1;
    const carrier = carrierOf(token, owned.byteLength);
    if (!leaseWitness || owned.byteLength === 0) return carrier;
    const result = partition(carrier, 1n, BigInt(owned.byteLength));
    if (!result.ok) throw new Error(`SEMAPRAX lease witness refused: ${result.refused}`);
    return result.children[0];
  };
  // Owned carrier: whole owner, else authenticated lease, else rejection.
  const owned = carrier => {
    live();
    const decoded = decodeOwnedCarrier(carrier);
    if (!decoded.tagged || decoded.token === 0) throw new Error("SEMAPRAX owned Bytes token invariant");
    const slot = whole(decoded);
    if (slot >= 0) return wholeBytes[slot];
    if (shared.owns(carrier)) return shared.view(carrier);
    throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
  };
  // Borrowed read: owned carrier, checked range descriptor, or fixed root.
  const read = carrier => {
    const decoded = decodeOwnedCarrier(carrier);
    if (decoded.tagged) return owned(carrier);
    live();
    const bytes = new Uint8Array(memory().buffer);
    if ((decoded.root & 0xc0000000) === 0x40000000) {
      const pointer = (decoded.root & 0xffff) * 8;
      if (pointer > FIXED_MEMORY_BYTES - 32) throw new Error("SEMAPRAX byte range descriptor bounds invariant");
      const descriptor = new DataView(bytes.buffer, pointer, 32);
      const identity = descriptor.getUint32(0, true);
      if (identity === 0 || identity !== ((decoded.root >>> 16) & 0x1fff)
          || descriptor.getUint32(4, true) !== pointer) {
        throw new Error("SEMAPRAX byte range descriptor identity invariant");
      }
      const ultimate = descriptor.getBigInt64(8, true);
      const root = decodeOwnedCarrier(ultimate);
      const start = descriptor.getBigUint64(16, true);
      const extent = descriptor.getBigUint64(24, true);
      if ((root.root & 0xc0000000) === 0x40000000 || extent !== BigInt(decoded.extent)
          || start > BigInt(root.extent) || extent > BigInt(root.extent) - start) {
        throw new Error("SEMAPRAX byte range descriptor extent invariant");
      }
      // A view over a lease resolves through the lease's current generation
      // and can only narrow its range.
      return read(ultimate).subarray(Number(start), Number(start + extent));
    }
    // Plain external roots retain the ordinary 64 KiB extent. The third
    // page is private machinery; the fourth is compiler-owned literal data.
    if (decoded.extent > 65536) throw new Error("SEMAPRAX byte carrier length invariant");
    if (decoded.root <= FIXED_MEMORY_BYTES - decoded.extent
        || (bytes.byteLength === 262144 && decoded.root >= 196608
            && decoded.root < 262144 && decoded.root <= 262144 - decoded.extent)) {
      return bytes.subarray(decoded.root, decoded.root + decoded.extent);
    }
    throw new Error("SEMAPRAX fixed byte range invariant");
  };
  // Ordinary drop: a whole owner frees its slot; a lease settles once and
  // the last live lease of a backing releases it exactly once.
  const drop = carrier => {
    live();
    const decoded = decodeOwnedCarrier(carrier);
    if (!decoded.tagged) throw new Error("SEMAPRAX borrowed Bytes drop invariant");
    const slot = whole(decoded);
    if (slot >= 0) {
      releasePayload(wholeBytes[slot].byteLength);
      wholeToken[slot] = 0;
      wholeBytes[slot] = null;
      liveWhole -= 1;
    } else if (shared.owns(carrier)) {
      shared.release(carrier);
    } else {
      throw new Error("SEMAPRAX double or stale Bytes drop");
    }
    drops += 1;
  };
  // Stores validate every operand before writing. A whole owner keeps its
  // carrier; a lease renews its generation over the same exact range.
  const store = (carrier, validate, write) => {
    live();
    const decoded = decodeOwnedCarrier(carrier);
    if (!decoded.tagged) throw new Error("SEMAPRAX borrowed Bytes store invariant");
    const slot = whole(decoded);
    if (slot >= 0) {
      validate(wholeBytes[slot]);
      write(wholeBytes[slot]);
      return BigInt.asIntN(64, decoded.word);
    }
    if (!shared.owns(carrier)) throw new Error("SEMAPRAX stale or malformed owned Bytes carrier");
    return shared.store(carrier, view => { validate(view); write(view); });
  };
  const byte = value => Number.isInteger(value) && value >= 0 && value <= 255;
  const interval = (index, width) => target => {
    if (typeof index !== "bigint" || index < 0n || index > BigInt(target.byteLength)
        || BigInt(target.byteLength) - index < BigInt(width)) {
      throw new Error("SEMAPRAX owned byte buffer interval invariant");
    }
  };
  const choice = (carrier, index, one, sourceCarrier, selector, width, extended) => {
    if (typeof selector !== "bigint" || !byte(one)) throw new Error("SEMAPRAX owned byte buffer choice invariant");
    const bits = BigInt.asUintN(64, selector);
    const copy = (bits & (1n << 63n)) !== 0n;
    const wide = extended && (bits & (1n << 62n)) !== 0n;
    const start = bits & ((1n << (extended ? 62n : 63n)) - 1n);
    const count = copy ? (wide ? 48 : width) : 1;
    // The source is read before the destination is written, so a source that
    // is the destination's own range observes its pre-store bytes.
    const input = read(sourceCarrier);
    const values = new Uint8Array(count);
    if (copy) {
      for (let offset = 0; offset < count; offset += 1) {
        const at = start + BigInt(offset);
        values[offset] = at < BigInt(input.byteLength) ? input[Number(at)] : 0;
      }
    } else {
      values[0] = one;
    }
    return store(carrier, interval(index, count), target => target.set(values, Number(index)));
  };

  const vector = (handle, tag) => {
    const value = vectors.get(handle);
    if (value === undefined || tag !== value.tag || tag < 1 || tag > 9) throw new Error("SEMAPRAX stale or mistyped Vec");
    return value;
  };
  // Every successful Vec mutation renews the Vec authority.
  const renewVec = (handle, value) => {
    vectors.delete(handle);
    const renewed = nextHandle();
    vectors.set(renewed, value);
    return renewed;
  };
  const outputView = (out, length) => {
    if (!Number.isInteger(out) || out < 0 || out > memory().buffer.byteLength - length) return null;
    return new DataView(memory().buffer, out, length);
  };
  const iterator = (handle, cursor) => {
    const state = iterators.get(handle);
    if (state === undefined || typeof cursor !== "bigint" || cursor !== state.cursor
        || cursor > BigInt(state.values.length)) return null;
    return state;
  };
  const box = (handle, tag) => {
    const value = boxes.get(handle);
    if (value === undefined || value.tag !== tag) throw new Error("SEMAPRAX stale or mistyped Box");
    return value;
  };
  const utf8 = bytes => {
    for (let index = 0; index < bytes.length;) {
      const first = bytes[index++];
      let extra, minimum, scalar;
      if (first < 128) continue;
      if (first >= 194 && first <= 223) { extra = 1; minimum = 128; scalar = first & 31; }
      else if (first >= 224 && first <= 239) { extra = 2; minimum = 2048; scalar = first & 15; }
      else if (first >= 240 && first <= 244) { extra = 3; minimum = 65536; scalar = first & 7; }
      else return false;
      if (index + extra > bytes.length) return false;
      for (let count = 0; count < extra; count += 1) {
        const next = bytes[index++];
        if ((next & 192) !== 128) return false;
        scalar = (scalar << 6) | (next & 63);
      }
      if (scalar < minimum || scalar > 1114111 || (scalar >= 55296 && scalar <= 57343)) return false;
    }
    return true;
  };
  const text = carrier => {
    const bytes = read(carrier);
    if (!utf8(bytes)) throw new Error("SEMAPRAX String UTF-8 invariant");
    return bytes;
  };
  const decimal = (value, unsigned) => {
    if (typeof value !== "bigint" || BigInt.asIntN(64, value) !== value) {
      throw new Error("SEMAPRAX numeric String input range invariant");
    }
    return own(new TextEncoder().encode((unsigned ? BigInt.asUintN(64, value) : value).toString()));
  };
  const textOutput = offset => {
    if (!Number.isInteger(offset) || offset < 65536 || offset % 8 !== 0 || offset > FIXED_MEMORY_BYTES - 8) {
      throw new Error("SEMAPRAX checked text output slot invariant");
    }
    return new DataView(memory().buffer, offset, 8);
  };

  const producer = {
    // Private generated producer: metadata and Vec storage precede transfer.
    // Allocation exceptions are administrative fail-stop, never a language status.
    spx_bytes_partition: (carrier, count, width) => {
      const decoded = decodeOwnedCarrier(carrier);
      const slot = whole(decoded);
      if (slot < 0) throw new Error("SEMAPRAX partition requires current whole owner");
      const prepared = shared.prepare(wholeBytes[slot], BigInt.asUintN(64, count), BigInt.asUintN(64, width));
      if (prepared.refused !== undefined) {
        return [0n, PARTITION_STATUS_CODES[prepared.refused]];
      }
      let handle;
      let values;
      try {
        values = new Array(Number(count));
        handle = nextHandle();
        vectors.set(handle, { tag: OWNED_BYTES_TAG, capacity: count, values });
      } catch (error) {
        shared.abort(prepared.ticket);
        throw error;
      }
      // commit fills this exact retained Vec array, sharing the original backing.
      shared.commit(prepared.ticket, values);
      wholeToken[slot] = 0;
      wholeBytes[slot] = null;
      liveWhole -= 1;
      return [handle, 0];
    },

  };
  const consumers = {
    spx_bytes_zeroed: count => {
      if (typeof count !== "bigint" || count < 0n || count > BigInt(MAX_BYTES_EXTENT)) {
        throw new Error("SEMAPRAX owned byte buffer capacity invariant");
      }
      return own(Number(count), true);
    },
    // Clone and nonconsuming copy deep-copy only the authenticated range into
    // fresh ordinary backing with the ordinary event/payload charge.
    spx_bytes_copy: carrier => own(read(carrier)),
    spx_bytes_get: (carrier, index) => {
      const bytes = read(carrier);
      const unsigned = BigInt.asUintN(64, index);
      return unsigned >= BigInt(bytes.byteLength) ? -1 : bytes[Number(unsigned)];
    },
    spx_bytes_as_slice: carrier => {
      read(carrier);
      return carrier;
    },
    spx_bytes_drop: drop,
    spx_bytes_set: (carrier, index, value) => store(carrier, target => {
      if (typeof index !== "bigint" || index < 0n || index >= BigInt(target.byteLength) || !byte(value)) {
        throw new Error("SEMAPRAX owned byte buffer element invariant");
      }
    }, target => { target[Number(index)] = value; }),
    spx_bytes_set5: (carrier, index, a, b, c, d, e) => {
      const values = [a, b, c, d, e];
      if (!values.every(byte)) throw new Error("SEMAPRAX owned byte buffer five-byte interval invariant");
      return store(carrier, interval(index, 5), target => target.set(values, Number(index)));
    },
    spx_bytes_set1_or5: (carrier, index, one, source, selector) =>
      choice(carrier, index, one, source, selector, 5, false),
    spx_bytes_set1_or6_or48: (carrier, index, one, source, selector) =>
      choice(carrier, index, one, source, selector, 6, true),

    spx_string_from_utf8_v1: (carrier, offset) => {
      const bytes = read(carrier), output = textOutput(offset);
      if (!utf8(bytes)) return 21;
      output.setBigInt64(0, own(bytes), true);
      return 0;
    },
    // The String runtime imports as one group; each result is a fresh
    // ordinary owner charged like any allocation, never a lease view.
    spx_string_concat_v1: (left, right) => {
      const a = text(left), b = text(right);
      if (a.byteLength > 65536 - b.byteLength) throw new Error("SEMAPRAX String concatenation capacity invariant");
      const joined = new Uint8Array(a.byteLength + b.byteLength);
      joined.set(a, 0);
      joined.set(b, a.byteLength);
      return own(joined);
    },
    spx_string_from_char_v1: scalar => {
      if (!Number.isInteger(scalar) || scalar < 0 || scalar > 1114111 || (scalar >= 55296 && scalar <= 57343)) {
        throw new Error("SEMAPRAX String character scalar invariant");
      }
      return own(new TextEncoder().encode(String.fromCodePoint(scalar)));
    },
    spx_string_from_i64_v1: value => decimal(value, false),
    spx_string_from_usize_v1: value => decimal(value, true),
    spx_string_len_chars_v1: carrier => {
      let count = 0n;
      for (const value of text(carrier)) if ((value & 192) !== 128) count += 1n;
      return count;
    },
    spx_string_from_str_v2: (carrier, offset) => {
      const bytes = read(carrier), output = textOutput(offset);
      if (!utf8(bytes)) return 25;
      output.setBigInt64(0, own(bytes), true);
      return 0;
    },
    spx_string_starts_with_v1: (value, prefix) => {
      const a = text(value), b = text(prefix);
      if (b.byteLength > a.byteLength) return 0;
      for (let index = 0; index < b.byteLength; index += 1) if (a[index] !== b[index]) return 0;
      return 1;
    },
    spx_string_contains_v1: (value, needle) => {
      const a = text(value), b = text(needle);
      if (b.byteLength === 0) return 1;
      outer: for (let start = 0; start + b.byteLength <= a.byteLength; start += 1) {
        for (let index = 0; index < b.byteLength; index += 1) if (a[start + index] !== b[index]) continue outer;
        return 1;
      }
      return 0;
    },
    spx_string_compare_v2: (left, right) => {
      const a = text(left), b = text(right), bound = Math.min(a.byteLength, b.byteLength);
      for (let index = 0; index < bound; index += 1) {
        if (a[index] !== b[index]) return a[index] < b[index] ? -1n : 1n;
      }
      return a.byteLength === b.byteLength ? 0n : a.byteLength < b.byteLength ? -1n : 1n;
    },

    // Box v2 (OWNED-BOUNDED-BOX-V2): tag 9 stores one owned carrier, whole or
    // lease, and transfers it only after the Box authority exists.
    spx_box_new_v2: (tag, bits) => {
      if (!Number.isInteger(tag) || tag < 1 || tag > 9 || typeof bits !== "bigint") {
        throw new Error("SEMAPRAX Box tag invariant");
      }
      if (tag === OWNED_BYTES_TAG) owned(bits);
      const handle = nextHandle();
      boxes.set(handle, { tag, bits });
      return handle;
    },
    spx_box_get_v2: (handle, tag) => {
      const value = box(handle, tag);
      if (tag === OWNED_BYTES_TAG) throw new Error("SEMAPRAX owned Box payload cannot be copied");
      return value.bits;
    },
    // Detaches the same owned carrier; the caller owns it.
    spx_box_into_inner_v2: (handle, tag) => {
      const value = box(handle, tag);
      boxes.delete(handle);
      return value.bits;
    },
    // Recursive drop releases the stored carrier exactly once.
    spx_box_drop_v2: handle => {
      const value = boxes.get(handle);
      if (value === undefined) throw new Error("SEMAPRAX stale Box drop");
      boxes.delete(handle);
      if (value.tag === OWNED_BYTES_TAG) drop(value.bits);
    },

    spx_vec_with_capacity_v2: (tag, capacity) => {
      if (!Number.isInteger(tag) || tag < 1 || tag > 9) throw new Error("SEMAPRAX Vec tag invariant");
      if (typeof capacity !== "bigint" || capacity < 0n || capacity > MAX_VEC_CAPACITY) return 0n;
      const handle = nextHandle();
      vectors.set(handle, { tag, capacity, values: [] });
      return handle;
    },
    spx_vec_push_v2: (handle, tag, item) => {
      const value = vector(handle, tag);
      if (tag === OWNED_BYTES_TAG) owned(item);
      if (BigInt(value.values.length) >= value.capacity) return 0n;
      value.values.push(item);
      return renewVec(handle, value);
    },
    spx_vec_len_v2: (handle, tag) => BigInt(vector(handle, tag).values.length),
    spx_vec_capacity_v2: (handle, tag) => vector(handle, tag).capacity,
    spx_vec_get_v2: (handle, tag, index) => {
      const value = vector(handle, tag);
      if (tag === OWNED_BYTES_TAG) throw new Error("SEMAPRAX owned Vec payload cannot be copied");
      return value.values[Number(index)];
    },
    // Canonical element order; each lease settles independently, so a child
    // already transferred out by iteration is unaffected.
    spx_vec_drop_v2: handle => {
      const raw = vectors.get(handle);
      if (raw === undefined) throw new Error("SEMAPRAX stale Vec drop");
      const value = vector(handle, raw.tag);
      vectors.delete(handle);
      if (value.tag === OWNED_BYTES_TAG) for (const item of value.values) drop(item);
    },
    spx_vec_reserve_exact_v2: (handle, tag, additional) => {
      const value = vector(handle, tag);
      if (typeof additional !== "bigint" || additional < 0n) return 0n;
      const target = BigInt(value.values.length) + additional;
      if (target > MAX_VEC_CAPACITY) return 0n;
      if (target > value.capacity) value.capacity = target;
      return renewVec(handle, value);
    },
    spx_vec_set_v2: (handle, tag, index, item) => {
      const value = vector(handle, tag);
      if (tag === OWNED_BYTES_TAG) owned(item);
      if (typeof index !== "bigint" || index < 0n || index >= BigInt(value.values.length)) return 0n;
      if (tag === OWNED_BYTES_TAG) drop(value.values[Number(index)]);
      value.values[Number(index)] = item;
      return renewVec(handle, value);
    },
    spx_vec_clear_v2: (handle, tag) => {
      const value = vector(handle, tag);
      const old = value.values;
      value.values = [];
      if (tag === OWNED_BYTES_TAG) for (const item of old) drop(item);
      return renewVec(handle, value);
    },

    // Consuming traversal: the Vec authority ends and the iterator owns the
    // untransferred suffix.
    spx_iter_bytes_into_v2: (handle, out) => {
      let value;
      try { value = vector(handle, OWNED_BYTES_TAG); } catch (_) { return 2; }
      const view = outputView(out, 16);
      if (view === null) return 2;
      const successor = nextHandle();
      vectors.delete(handle);
      iterators.set(successor, { values: value.values, cursor: 0n });
      view.setBigUint64(0, successor, true);
      view.setBigUint64(8, 0n, true);
      return 0;
    },
    // Next transfers the original child carrier, never a copy; the popped
    // child survives the iterator, the Vec and every sibling.
    spx_iter_bytes_next_v2: (handle, cursor, out) => {
      const state = iterator(handle, cursor);
      if (state === null) return 2;
      const view = outputView(out, 32);
      if (view === null) return 2;
      if (cursor === BigInt(state.values.length)) {
        iterators.delete(handle);
        for (let index = 0; index < 32; index += 1) view.setUint8(index, 0);
        return 0;
      }
      const item = state.values[Number(cursor)];
      try { owned(item); } catch (_) { return 2; }
      const successor = nextHandle();
      iterators.delete(handle);
      iterators.set(successor, { values: state.values, cursor: cursor + 1n });
      view.setUint32(0, 1, true);
      view.setUint32(4, 0, true);
      view.setBigUint64(8, BigInt.asUintN(64, item), true);
      view.setBigUint64(16, successor, true);
      view.setBigUint64(24, cursor + 1n, true);
      return 0;
    },
    // Abandonment drops only the untransferred suffix in canonical order.
    spx_iter_bytes_drop_v2: (handle, cursor) => {
      const state = iterator(handle, cursor);
      if (state === null) throw new Error("SEMAPRAX stale Iter drop");
      iterators.delete(handle);
      for (let index = Number(cursor); index < state.values.length; index += 1) drop(state.values[index]);
    },
  };
  // Every import requires the live invocation before touching any table.
  const imports = Object.freeze(Object.fromEntries([...Object.entries(consumers), ...Object.entries(producer)].map(([name, body]) => [
    name,
    (...args) => {
      live();
      return body(...args);
    },
  ])));

  const settled = () => liveWhole === 0 && shared.settled()
    && vectors.size === 0 && iterators.size === 0 && boxes.size === 0;
  const requireReceipt = receipt => {
    if (current === null || receipt !== current.receipt) {
      throw new Error("SEMAPRAX private ordinary invocation receipt invariant");
    }
  };
  const release = () => {
    vectors.clear();
    iterators.clear();
    boxes.clear();
    for (const bytes of wholeBytes) if (bytes !== null) releasePayload(bytes.byteLength);
    wholeToken.fill(0);
    wholeBytes.fill(null);
    liveWhole = 0;
    return shared.abandon();
  };

  const facade = Object.freeze({
    // Opens one invocation. A previous invocation must have ended settled or
    // been abandoned by its own receipt; counters reset only here.
    beginInvocation() {
      if (current !== null) throw new Error("SEMAPRAX settle the current invocation before fresh reentry");
      if (!settled()) throw new Error("SEMAPRAX private ordinary host settlement invariant");
      if (serial >= 0xffffffff) throw new Error("SEMAPRAX invocation serial exhausted");
      serial += 1;
      events = 0;
      payload = 0;
      copies = 0;
      drops = 0;
      const receipt = Object.freeze(Object.create(null));
      current = Object.freeze({ receipt, serial });
      return receipt;
    },
    // Ends the invocation only when every owner, lease, backing, reservation
    // and Vec/iterator/Box authority has settled. Refusal keeps it open.
    endInvocation(receipt) {
      requireReceipt(receipt);
      if (!settled()) throw new Error("SEMAPRAX private ordinary invocation is unsettled");
      current = null;
    },
    // Post-trap administration release for the current invocation only; no
    // finalizer order is synthesized and no result is published.
    abandon(receipt) {
      requireReceipt(receipt);
      const released = release();
      current = null;
      return released;
    },
    // Witness-only lease issuance for the owning regressions.
    partitionForWitness(carrier, count, width) {
      live();
      return partition(carrier, count, width);
    },
    // Builds a host-owned Vec<Bytes> over committed children, as the
    // producer's provisional result would after its own capacity check.
    vecForWitness(children) {
      live();
      if (!Array.isArray(children) || BigInt(children.length) > MAX_VEC_CAPACITY) {
        throw new Error("SEMAPRAX witness Vec invariant");
      }
      for (const item of children) owned(item);
      const handle = nextHandle();
      vectors.set(handle, { tag: OWNED_BYTES_TAG, capacity: BigInt(children.length), values: [...children] });
      return handle;
    },
    // Public copy-out plus provider drop for any owned carrier.
    copyOutAndDrop(carrier) {
      live();
      if (managed) throw new Error("SEMAPRAX private managed entry has no copy-out bridge");
      if (shared.owns(carrier)) {
        const snapshot = shared.copyOutAndRelease(carrier);
        drops += 1;
        return snapshot;
      }
      const snapshot = new Uint8Array(owned(carrier));
      drop(carrier);
      return snapshot;
    },
    peek(carrier) {
      if (managed) throw new Error("SEMAPRAX private managed entry has no copy-out bridge");
      return new Uint8Array(read(carrier));
    },
    audit: () => shared.audit(),
    stats() {
      return Object.freeze({
        ...shared.stats(),
        managedReserved,
        managedPeak,
        entries: liveWhole,
        vectors: vectors.size,
        iterators: iterators.size,
        boxes: boxes.size,
        events,
        payload,
        copies,
        drops,
        invocation: invocation(),
        wholeTableBytes: wholeToken.byteLength + wholeInvocation.byteLength,
      });
    },
  });

  // Read/allocate/drop hooks for a joined owning host. Each requires the
  // live invocation and resolves only this host's whole owners and leases.
  const payloadHooks = Object.freeze({
    owned,
    read,
    own,
    clone: carrier => own(read(carrier)),
    drop,
  });

  return Object.freeze({
    imports,
    facade,
    payload: payloadHooks,
    bind(wasmInstance) {
      if (instance !== null) throw new Error("SEMAPRAX private ordinary host already bound");
      instance = wasmInstance;
    },
  });
}
