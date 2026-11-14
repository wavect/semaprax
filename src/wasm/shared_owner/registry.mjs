// Private authenticated shared-owner registry for ordinary Core-Wasm Bytes.
//
// OPT #746 consumer closure, docs/OWNED-BYTE-PARTITION-V1.md. One backing
// record owns the original whole-owner allocation; each child lease owns one
// immutable disjoint range of it. The guest still sees the frozen two-word
// owned carrier `((0x80000000 | token) << 32) | extent`: tokens are opaque,
// minted by the enclosing host's never-reset token authority and resolved
// only through this registry's private index. No pointer, offset or header
// ever crosses into the guest, and nothing here probes memory to classify a
// value.
//
// Metadata lives in fixed typed tables sized at creation: a lease-slot table
// bounded by the existing Vec<Bytes> carrier capacity, a backing table bounded
// by the existing per-invocation allocation-event limit, and an open-addressed
// token index at most half full. Preparation reserves exact free entries of
// those tables, so a committed partition never grows any structure. The
// reported table bytes are typed-array payload only, not physical proof: JS
// object headers, the backing reference array and transient views are host
// context overhead outside managed guest storage.
//
// The private checked producer and the host's witness facade both use this
// reservation path. The enclosing host authenticates the whole-owner carrier.

export const SHARED_OWNER_DOMAIN = "semaprax.private.wasm-shared-byte-owner.v1";
// Child metadata is bounded by the existing Vec<Bytes> carrier capacity
// (`vec_ops::MAX_CAPACITY`); the registry never raises it.
export const MAX_LEASE_SLOTS = 8192;
// Every backing was one whole owner charged as one ordinary allocation event
// of the current invocation, and the host settles every owner before a fresh
// invocation. The existing 32-event limit therefore funds at most 32 live
// backings without any additional quota.
export const MAX_BACKINGS = 32;
// The existing individual Bytes extent (`vec_ops::MAX_OWNED_PAYLOAD_BYTES`).
export const MAX_BYTES_EXTENT = 131072;
// A store renews its lease generation. Exhaustion refuses the store before
// any byte is written; tokens are never reissued, so there is no ABA reuse.
export const MAX_LEASE_GENERATION = 0xffff;
export const MAX_TOKEN = 0x7fffffff;
const U64_MAX = (1n << 64n) - 1n;
// Open-addressed token index: a power of two at least twice the slot bound.
const INDEX_CAPACITY = 16384;
const INDEX_SHIFT = 32 - 14;
const FREE = 0;
const RESERVED = 1;
const LIVE = 2;

// Partition refusals of `semaprax.byte-partition.v1`, in precedence order.
// Codes are assigned by the owning contract: 1 WidthZero, 2 ExtentOverflow,
// 3 ExtentMismatch, 4 MetadataCapacity.
export const PARTITION_REFUSALS = Object.freeze([
  "width_zero",
  "extent_overflow",
  "extent_mismatch",
  "metadata_capacity",
]);
export const PARTITION_STATUS_DOMAIN = "semaprax.byte-partition.v1";
export const PARTITION_STATUS_CODES = Object.freeze({
  width_zero: 1,
  extent_overflow: 2,
  extent_mismatch: 3,
  metadata_capacity: 4,
});

export function carrierOf(token, extent) {
  if (!Number.isInteger(token) || token < 1 || token > MAX_TOKEN
      || !Number.isInteger(extent) || extent < 0 || extent > MAX_BYTES_EXTENT) {
    throw new Error("SEMAPRAX shared owner carrier invariant");
  }
  return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(extent));
}

export function decodeOwnedCarrier(carrier) {
  if (typeof carrier !== "bigint") throw new TypeError("SEMAPRAX byte carrier is not i64");
  const word = BigInt.asUintN(64, carrier);
  const extent = Number(word & 0xffffffffn);
  const root = Number((word >> 32n) & 0xffffffffn);
  return { word, extent, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff };
}

// Fixed open-addressed map from a nonzero 31-bit token to a lease slot.
// Linear probing with backward-shift deletion; it never resizes.
function createTokenIndex() {
  const keys = new Uint32Array(INDEX_CAPACITY);
  const values = new Uint16Array(INDEX_CAPACITY);
  const mask = INDEX_CAPACITY - 1;
  const home = key => Math.imul(key, 0x9e3779b1) >>> INDEX_SHIFT;
  const find = key => {
    for (let at = home(key); ; at = (at + 1) & mask) {
      if (keys[at] === 0) return -1;
      if (keys[at] === key) return at;
    }
  };
  return Object.freeze({
    bytes: keys.byteLength + values.byteLength,
    get(key) {
      const at = find(key);
      return at < 0 ? -1 : values[at];
    },
    set(key, value) {
      let at = home(key);
      while (keys[at] !== 0) {
        if (keys[at] === key) throw new Error("SEMAPRAX shared owner token reuse invariant");
        at = (at + 1) & mask;
      }
      keys[at] = key;
      values[at] = value;
    },
    delete(key) {
      let hole = find(key);
      if (hole < 0) throw new Error("SEMAPRAX shared owner token index invariant");
      keys[hole] = 0;
      for (let at = (hole + 1) & mask; keys[at] !== 0; at = (at + 1) & mask) {
        const wanted = home(keys[at]);
        const stays = hole <= at ? wanted > hole && wanted <= at : wanted > hole || wanted <= at;
        if (stays) continue;
        keys[hole] = keys[at];
        values[hole] = values[at];
        keys[at] = 0;
        hole = at;
      }
    },
  });
}

// `tokens` is the enclosing host's single never-reset token authority:
// `remaining()` reports unreserved tokens across all registries. A hold
// protects the promised count from ordinary mints and other preparations.
// `invocation()` is the host's current invocation serial (0 when idle); every
// lease is stamped with it and authenticates only inside that invocation.
export function createSharedByteOwnerRegistry({
  tokens,
  invocation,
  maxLeaseSlots = MAX_LEASE_SLOTS,
  maxBackings = MAX_BACKINGS,
  reserveStorage = () => {},
  releasePayload = () => {},
} = {}) {
  if (tokens === null || typeof tokens !== "object"
      || typeof tokens.mint !== "function" || typeof tokens.remaining !== "function"
      || typeof tokens.hold !== "function" || typeof tokens.release !== "function") {
    throw new TypeError("SEMAPRAX shared owner token authority is required");
  }
  if (typeof invocation !== "function") {
    throw new TypeError("SEMAPRAX shared owner invocation authority is required");
  }
  if (!Number.isSafeInteger(maxLeaseSlots) || maxLeaseSlots < 1 || maxLeaseSlots > MAX_LEASE_SLOTS) {
    throw new RangeError("invalid SEMAPRAX shared owner lease-slot bound");
  }
  if (!Number.isSafeInteger(maxBackings) || maxBackings < 1 || maxBackings > MAX_BACKINGS) {
    throw new RangeError("invalid SEMAPRAX shared owner backing bound");
  }
  // Exact typed-array backing only; reference arrays and VM objects excluded.
  reserveStorage(INDEX_CAPACITY * 6 + maxLeaseSlots * 22 + maxBackings * 11);
  const index = createTokenIndex();
  // Lease slot table.
  const leaseToken = new Uint32Array(maxLeaseSlots);
  const leaseBacking = new Uint8Array(maxLeaseSlots);
  const leaseOffset = new Uint32Array(maxLeaseSlots);
  const leaseExtent = new Uint32Array(maxLeaseSlots);
  const leaseGeneration = new Uint16Array(maxLeaseSlots);
  const leaseInvocation = new Uint32Array(maxLeaseSlots);
  const leaseState = new Uint8Array(maxLeaseSlots);
  const freeSlots = new Uint16Array(maxLeaseSlots);
  for (let slot = 0; slot < maxLeaseSlots; slot += 1) freeSlots[slot] = maxLeaseSlots - 1 - slot;
  let freeTop = maxLeaseSlots;
  // Backing table. `backingBytes` holds only references to the former whole
  // owners' exact buffers; its length never changes.
  const backingBytes = new Array(maxBackings).fill(null);
  const backingExtent = new Uint32Array(maxBackings);
  const backingLive = new Uint16Array(maxBackings);
  const backingInvocation = new Uint32Array(maxBackings);
  const backingState = new Uint8Array(maxBackings);
  const tableBytes = index.bytes + leaseToken.byteLength + leaseBacking.byteLength
    + leaseOffset.byteLength + leaseExtent.byteLength + leaseGeneration.byteLength
    + leaseInvocation.byteLength + leaseState.byteLength + freeSlots.byteLength
    + backingExtent.byteLength + backingLive.byteLength + backingInvocation.byteLength
    + backingState.byteLength;

  // Pending tickets are bounded by the whole-backing inventory. A claimed
  // buffer cannot be prepared again through another whole-array view.
  const tickets = new Map();
  const claimed = new WeakSet();
  let reservedSlots = 0;
  // Match native's backing table lifetime. A retired child frees a lookup
  // slot, while its backing's full table charge remains until the last child.
  let backingTableSlots = 0;
  let reservedTokens = 0;
  let liveLeases = 0;
  let peakLiveLeases = 0;
  let liveBackings = 0;
  let peakLiveBackings = 0;
  let releasedBackings = 0;

  const current = () => {
    const serial = invocation();
    if (!Number.isInteger(serial) || serial < 1) {
      throw new Error("SEMAPRAX shared owner has no live invocation");
    }
    return serial;
  };

  // Authenticates one carrier against the current lease generation and
  // invocation. Returns the lease slot, or -1 when this registry never
  // issued the token or already settled it.
  const authenticate = decoded => {
    if (!decoded.tagged || decoded.token === 0) return -1;
    const slot = index.get(decoded.token);
    if (slot < 0) return -1;
    const backing = leaseBacking[slot];
    if (leaseState[slot] !== LIVE || leaseToken[slot] !== decoded.token
        || backingState[backing] !== LIVE || backingBytes[backing] === null
        || leaseInvocation[slot] !== current() || backingInvocation[backing] !== leaseInvocation[slot]) {
      throw new Error("SEMAPRAX shared owner lease authority invariant");
    }
    if (decoded.extent !== leaseExtent[slot]) {
      throw new Error("SEMAPRAX shared owner lease extent invariant");
    }
    return slot;
  };

  const requireLease = carrier => {
    const slot = authenticate(decodeOwnedCarrier(carrier));
    if (slot < 0) throw new Error("SEMAPRAX stale, foreign or unleased Bytes carrier");
    return slot;
  };

  const viewOf = slot => {
    const offset = leaseOffset[slot];
    return backingBytes[leaseBacking[slot]].subarray(offset, offset + leaseExtent[slot]);
  };

  const freeBacking = () => {
    for (let backing = 0; backing < maxBackings; backing += 1) {
      if (backingState[backing] === FREE) return backing;
    }
    return -1;
  };

  // Validation order: width, checked extent, exact extent, then complete
  // metadata. Only the final success reserves exact table entries; the caller
  // still owns the whole input until commit.
  const prepare = (whole, count, width) => {
    const serial = current();
    if (!(whole instanceof Uint8Array) || whole.byteLength > MAX_BYTES_EXTENT
        || whole.byteOffset !== 0 || whole.byteLength !== whole.buffer.byteLength) {
      throw new Error("SEMAPRAX shared owner whole backing invariant");
    }
    if (claimed.has(whole.buffer)) throw new Error("SEMAPRAX shared owner backing already claimed");
    if (typeof count !== "bigint" || typeof width !== "bigint"
        || count < 0n || count > U64_MAX || width < 0n || width > U64_MAX) {
      throw new TypeError("SEMAPRAX shared owner partition operand invariant");
    }
    if (width === 0n) return { refused: "width_zero" };
    const extent = count * width;
    if (extent > U64_MAX) return { refused: "extent_overflow" };
    if (extent !== BigInt(whole.byteLength)) return { refused: "extent_mismatch" };
    if (count > BigInt(maxLeaseSlots)) return { refused: "metadata_capacity" };
    const children = Number(count);
    const backing = children > 0 ? freeBacking() : -1;
    if (tickets.size >= maxBackings || children > maxLeaseSlots - backingTableSlots - reservedSlots
        || (children > 0 && backing < 0)
        || tokens.remaining() < children) {
      return { refused: "metadata_capacity" };
    }
    const ticket = Object.freeze(Object.create(null));
    const reservation = { serial, whole, children, width: Number(width), backing };
    claimed.add(whole.buffer);
    try {
      tickets.set(ticket, reservation);
      tokens.hold(children);
    }
    catch (error) { tickets.delete(ticket); claimed.delete(whole.buffer); throw error; }
    reservedSlots += children;
    reservedTokens += children;
    if (backing >= 0) backingState[backing] = RESERVED;
    return { ticket };
  };

  const pending = ticket => {
    const prepared = tickets.get(ticket);
    if (prepared === undefined || prepared.serial !== current()) {
      throw new Error("SEMAPRAX shared owner partition ticket invariant");
    }
    return prepared;
  };

  const unreserve = prepared => {
    tokens.release(prepared.children);
    reservedSlots -= prepared.children;
    reservedTokens -= prepared.children;
    if (prepared.backing >= 0) backingState[prepared.backing] = FREE;
  };

  // Releases only the unpublished reservation; the whole owner is untouched.
  const abort = ticket => {
    const prepared = pending(ticket);
    unreserve(prepared);
    tickets.delete(ticket);
    claimed.delete(prepared.whole.buffer);
  };

  // Every table entry and result array is reserved before commit. The caller
  // removes the whole owner only after this successful transfer completes.
  const commit = (ticket, destination = undefined) => {
    const prepared = pending(ticket);
    const { serial, whole, children, width, backing } = prepared;
    // Allocate all result storage while the whole owner and reservation live.
    const carriers = destination === undefined ? new Array(children) : destination;
    if (!Array.isArray(carriers) || carriers.length !== children) {
      throw new Error("SEMAPRAX shared owner result capacity invariant");
    }
    tickets.delete(ticket);
    unreserve(prepared);
    if (children === 0) return carriers;
    backingTableSlots += children;
    backingState[backing] = LIVE;
    backingBytes[backing] = whole;
    backingExtent[backing] = whole.byteLength;
    backingLive[backing] = children;
    backingInvocation[backing] = serial;
    liveBackings += 1;
    if (liveBackings > peakLiveBackings) peakLiveBackings = liveBackings;
    for (let child = 0; child < children; child += 1) {
      const token = tokens.mint();
      if (!Number.isInteger(token) || token < 1 || token > MAX_TOKEN) {
        throw new Error("SEMAPRAX shared owner token authority invariant");
      }
      const slot = freeSlots[--freeTop];
      leaseToken[slot] = token;
      leaseBacking[slot] = backing;
      leaseOffset[slot] = child * width;
      leaseExtent[slot] = width;
      leaseGeneration[slot] = 1;
      leaseInvocation[slot] = serial;
      leaseState[slot] = LIVE;
      index.set(token, slot);
      carriers[child] = carrierOf(token, width);
    }
    liveLeases += children;
    if (liveLeases > peakLiveLeases) peakLiveLeases = liveLeases;
    return carriers;
  };

  // Settles one lease. Returns whether it was the final lease of its backing
  // and therefore that backing's single release.
  const settle = slot => {
    index.delete(leaseToken[slot]);
    leaseState[slot] = FREE;
    leaseToken[slot] = 0;
    freeSlots[freeTop++] = slot;
    liveLeases -= 1;
    const backing = leaseBacking[slot];
    backingLive[backing] -= 1;
    if (backingLive[backing] > 0) return false;
    if (backingState[backing] !== LIVE) throw new Error("SEMAPRAX shared owner backing release invariant");
    const tableSlots = backingExtent[backing] / leaseExtent[slot];
    if (!Number.isInteger(tableSlots) || tableSlots < 1 || tableSlots > backingTableSlots) {
      throw new Error("SEMAPRAX shared owner backing table accounting invariant");
    }
    backingTableSlots -= tableSlots;
    backingState[backing] = FREE;
    releasePayload(backingExtent[backing]);
    backingBytes[backing] = null;
    backingExtent[backing] = 0;
    liveBackings -= 1;
    releasedBackings += 1;
    return true;
  };

  return Object.freeze({
    domain: SHARED_OWNER_DOMAIN,
    // True only for a live lease of this registry in the current invocation;
    // whole owners and foreign tokens are left to the caller's own table.
    owns(carrier) {
      return authenticate(decodeOwnedCarrier(carrier)) >= 0;
    },
    // Bounded view of exactly the authenticated range. Typed-array bounds
    // are the read/write fence: an index outside the range cannot reach a
    // sibling's bytes.
    view(carrier) {
      return viewOf(requireLease(carrier));
    },
    // A renewed store keeps the exact range and advances the generation.
    // `mutate` validates its operands before writing; it runs only after
    // renewal capacity is proven, and its failure leaves the old generation.
    store(carrier, mutate) {
      const slot = requireLease(carrier);
      if (leaseGeneration[slot] >= MAX_LEASE_GENERATION) {
        throw new Error("SEMAPRAX shared owner lease generation exhausted");
      }
      if (tokens.remaining() < 1) throw new Error("SEMAPRAX owned Bytes token space exhausted");
      mutate(viewOf(slot));
      const token = tokens.mint();
      if (!Number.isInteger(token) || token < 1 || token > MAX_TOKEN) {
        throw new Error("SEMAPRAX shared owner token authority invariant");
      }
      index.delete(leaseToken[slot]);
      leaseToken[slot] = token;
      leaseGeneration[slot] += 1;
      index.set(token, slot);
      return carrierOf(token, leaseExtent[slot]);
    },
    // Ordinary drop of one child.
    release(carrier) {
      return settle(requireLease(carrier));
    },
    // Public copy-out plus provider drop: a fresh unshared snapshot, then
    // the private lease settles. No interior view or offset is returned.
    copyOutAndRelease(carrier) {
      const slot = requireLease(carrier);
      const snapshot = new Uint8Array(leaseExtent[slot]);
      snapshot.set(viewOf(slot));
      settle(slot);
      return snapshot;
    },
    prepare,
    commit,
    abort,
    // Context abandonment after a trap: releases remaining administration
    // and invalidates every pending ticket, without synthesizing source
    // finalizer order or publishing a result.
    abandon() {
      let released = 0;
      for (let slot = 0; slot < maxLeaseSlots; slot += 1) {
        if (leaseState[slot] === LIVE && settle(slot)) released += 1;
      }
      for (let backing = 0; backing < maxBackings; backing += 1) {
        if (backingState[backing] === RESERVED) backingState[backing] = FREE;
      }
      for (const prepared of tickets.values()) claimed.delete(prepared.whole.buffer);
      tokens.release(reservedTokens);
      tickets.clear();
      reservedSlots = 0;
      reservedTokens = 0;
      return released;
    },
    settled() {
      return liveLeases === 0 && liveBackings === 0 && tickets.size === 0
        && reservedSlots === 0 && backingTableSlots === 0;
    },
    // Independent inventory audit: every live range lies inside its live
    // backing, live counts agree, and no two live ranges overlap.
    audit() {
      const ranges = new Map();
      let leases = 0;
      for (let slot = 0; slot < maxLeaseSlots; slot += 1) {
        if (leaseState[slot] !== LIVE) continue;
        leases += 1;
        const backing = leaseBacking[slot];
        if (backingState[backing] !== LIVE || index.get(leaseToken[slot]) !== slot
            || leaseOffset[slot] + leaseExtent[slot] > backingExtent[backing]
            || leaseInvocation[slot] !== backingInvocation[backing]) {
          throw new Error("SEMAPRAX shared owner audit: malformed lease");
        }
        if (!ranges.has(backing)) ranges.set(backing, []);
        ranges.get(backing).push([leaseOffset[slot], leaseExtent[slot]]);
      }
      let backings = 0;
      let tableSlots = 0;
      for (let backing = 0; backing < maxBackings; backing += 1) {
        if (backingState[backing] !== LIVE) continue;
        backings += 1;
        const list = ranges.get(backing) ?? [];
        if (list.length !== backingLive[backing] || backingBytes[backing]?.byteLength !== backingExtent[backing]) {
          throw new Error("SEMAPRAX shared owner audit: live count");
        }
        const width = list[0]?.[1];
        const slots = backingExtent[backing] / width;
        if (!Number.isInteger(slots) || slots < list.length || slots > maxLeaseSlots) {
          throw new Error("SEMAPRAX shared owner audit: backing table extent");
        }
        tableSlots += slots;
        list.sort((a, b) => a[0] - b[0]);
        for (let at = 1; at < list.length; at += 1) {
          if (list[at - 1][0] + list[at - 1][1] > list[at][0]) {
            throw new Error("SEMAPRAX shared owner audit: overlapping live ranges");
          }
        }
      }
      let pendingSlots = 0;
      for (const reservation of tickets.values()) {
        if (reservation.serial !== current() || !claimed.has(reservation.whole.buffer)
            || (reservation.backing >= 0 && backingState[reservation.backing] !== RESERVED)) {
          throw new Error("SEMAPRAX shared owner audit: reservation");
        }
        pendingSlots += reservation.children;
      }
      if (leases !== liveLeases || backings !== liveBackings || freeTop + liveLeases !== maxLeaseSlots
          || tableSlots !== backingTableSlots || backingTableSlots + reservedSlots > maxLeaseSlots
          || pendingSlots !== reservedSlots || pendingSlots !== reservedTokens || tickets.size > maxBackings) {
        throw new Error("SEMAPRAX shared owner audit: inventory");
      }
      return true;
    },
    stats() {
      return Object.freeze({
        liveLeases,
        peakLiveLeases,
        liveBackings,
        peakLiveBackings,
        releasedBackings,
        reservedSlots,
        freeSlots: freeTop,
        leaseSlotCapacity: maxLeaseSlots,
        backingCapacity: maxBackings,
        tableBytes,
      });
    },
  });
}
