// Hostile and lifecycle regressions for the private shared-owner host.
// Authored for `shared_owner::tests`; each case uses a fresh host.
import { createPrivateOrdinaryByteHost, MAX_BACKING_EVENTS } from "./host.mjs";
import {
  MAX_BACKINGS,
  MAX_LEASE_GENERATION,
  PARTITION_STATUS_CODES,
  PARTITION_REFUSALS,
  carrierOf,
  decodeOwnedCarrier,
} from "./registry.mjs";

const check = (condition, message) => { if (!condition) throw new Error(`FAILED: ${message}`); };
const throws = (body, pattern, message) => {
  try { body(); } catch (error) {
    check(pattern.test(String(error.message)), `${message}: unexpected ${error.message}`);
    return;
  }
  throw new Error(`FAILED: ${message}: no rejection`);
};
const same = (a, b) => a.length === b.length && a.every((value, index) => value === b[index]);
const fresh = options => {
  const host = createPrivateOrdinaryByteHost(options);
  const pages = options?.memoryPages ?? 2;
  const memory = new WebAssembly.Memory({ initial: pages, maximum: pages });
  host.bind({ exports: { memory } });
  const receipt = host.facade.beginInvocation();
  return { host, env: host.imports, facade: host.facade, memory, receipt };
};
// NUL, Unicode (U+00E9, U+1F600) and non-UTF-8 bytes in one 12-byte owner.
const PAYLOAD = [0x00, 0x41, 0xff, 0xc3, 0xa9, 0x42, 0xf0, 0x9f, 0x98, 0x80, 0x00, 0xfe];
const filled = env => {
  let carrier = env.spx_bytes_zeroed(BigInt(PAYLOAD.length));
  for (let index = 0; index < PAYLOAD.length; index += 1) {
    carrier = env.spx_bytes_set(carrier, BigInt(index), PAYLOAD[index]);
  }
  return carrier;
};
const partition = (facade, carrier, count, width) => {
  const result = facade.partitionForWitness(carrier, BigInt(count), BigInt(width));
  check(result.ok, `partition ${count}x${width} refused: ${result.refused}`);
  facade.audit();
  return [...result.children];
};
const cases = [];
const test = (name, body) => cases.push([name, body]);

test("status domain codes follow refusal precedence", () => {
  check(same(PARTITION_REFUSALS, ["width_zero", "extent_overflow", "extent_mismatch", "metadata_capacity"]),
    "refusal precedence");
  check(PARTITION_REFUSALS.every((symbol, index) => PARTITION_STATUS_CODES[symbol] === index + 1), "codes 1..4");
});

test("exact order, NUL, Unicode and non-UTF-8 preserved without a charge", () => {
  const { env, facade, receipt } = fresh();
  const children = partition(facade, filled(env), 4, 3);
  const joined = children.flatMap(child => [...facade.peek(child)]);
  check(same(joined, PAYLOAD), "concatenated children reproduce the owner");
  const stats = facade.stats();
  check(stats.events === 1 && stats.payload === PAYLOAD.length, "partition charged no backing event or payload");
  check(stats.entries === 0 && stats.liveBackings === 1 && stats.liveLeases === 4, "inventory after commit");
  let sum = 0;
  for (const child of children) for (let i = 0n; i < 3n; i += 1n) sum += env.spx_bytes_get(child, i);
  check(sum === PAYLOAD.reduce((a, b) => a + b, 0), "byte sum through ordinary get");
  check(env.spx_bytes_get(children[0], 3n) === -1, "get past the lease extent is out of range");
  for (const child of children) env.spx_bytes_drop(child);
  check(facade.stats().releasedBackings === 1 && facade.stats().liveBackings === 0, "final backing released once");
  facade.endInvocation(receipt);
});

test("refusals precede transfer and keep the whole owner", () => {
  const { env, facade, receipt } = fresh({ maxLeaseSlots: 4 });
  const owner = filled(env);
  for (const [count, width, symbol] of [
    [4n, 0n, "width_zero"],
    [0n, 0n, "width_zero"],
    [1n << 63n, 4n, "extent_overflow"],
    [5n, 3n, "extent_mismatch"],
    [12n, 1n, "metadata_capacity"],
  ]) {
    const result = facade.partitionForWitness(owner, count, width);
    check(!result.ok && result.refused === symbol, `refusal ${symbol}`);
    check(same([...facade.peek(owner)], PAYLOAD), `owner unchanged after ${symbol}`);
    check(facade.stats().reservedSlots === 0 && facade.stats().entries === 1, `no reservation after ${symbol}`);
  }
  // Exact slot bound succeeds; one more is refused above.
  const children = partition(facade, owner, 4, 3);
  throws(() => env.spx_bytes_get(owner, 0n), /stale or malformed/, "consumed whole owner is stale");
  for (const child of children) env.spx_bytes_drop(child);
  facade.endInvocation(receipt);
});

test("Vec carrier capacity bounds child metadata exactly", () => {
  const { env, facade, receipt } = fresh();
  const over = env.spx_bytes_zeroed(8193n);
  const refused = facade.partitionForWitness(over, 8193n, 1n);
  check(!refused.ok && refused.refused === "metadata_capacity", "8193 children refused");
  env.spx_bytes_drop(over);
  const exact = env.spx_bytes_zeroed(8192n);
  const children = partition(facade, exact, 8192, 1);
  check(facade.stats().peakLiveLeases === 8192 && facade.stats().freeSlots === 0, "exact 8192 children");
  const vec = facade.vecForWitness(children);
  env.spx_vec_drop_v2(vec);
  check(facade.stats().liveLeases === 0 && facade.stats().releasedBackings === 1, "Vec drop settles all children once");
  check(facade.stats().freeSlots === 8192, "every slot returned to the fixed table");
  facade.endInvocation(receipt);
});

test("partition table charge retires with the last child", () => {
  const { env, facade, memory, receipt } = fresh();
  const [first, firstStatus] = env.spx_bytes_partition(env.spx_bytes_zeroed(8192n), 8192n, 1n);
  check(firstStatus === 0, "exact table fits");
  const view = new DataView(memory.buffer);
  check(env.spx_iter_bytes_into_v2(first, 0) === 0, "iterator takes the Vec");
  const iterator = view.getBigUint64(0, true);
  check(env.spx_iter_bytes_next_v2(iterator, 0n, 16) === 0, "iterator transfers one child");
  const last = view.getBigUint64(24, true);
  env.spx_iter_bytes_drop_v2(view.getBigUint64(32, true), 1n);
  const next = env.spx_bytes_zeroed(1n);
  const [refused, refusedStatus] = env.spx_bytes_partition(next, 1n, 1n);
  check(refused === 0n && refusedStatus === PARTITION_STATUS_CODES.metadata_capacity, "live child pins the backing table");
  facade.audit();
  env.spx_bytes_drop(last);
  const [second, secondStatus] = env.spx_bytes_partition(next, 1n, 1n);
  check(secondStatus === 0, "settled table can fund another partition");
  throws(() => env.spx_bytes_get(last, 0n), /stale|unleased/, "retired child stays stale after reuse");
  env.spx_vec_drop_v2(second);
  check(facade.stats().events === 2 && facade.stats().payload === 8193, "backing charges remain cumulative");
  facade.audit();
  facade.endInvocation(receipt);
});

test("simultaneous partitioned owners share one fixed slot table", () => {
  const { env, facade, receipt } = fresh();
  const left = partition(facade, env.spx_bytes_zeroed(4096n), 4096, 1);
  const right = partition(facade, env.spx_bytes_zeroed(4096n), 2048, 2);
  check(facade.stats().liveBackings === 2 && facade.stats().freeSlots === 2048, "two live backings");
  const third = env.spx_bytes_zeroed(2049n);
  check(facade.partitionForWitness(third, 2049n, 1n).refused === "metadata_capacity", "slot table exhausted");
  check(facade.stats().entries === 1 && facade.stats().reservedSlots === 0, "refused owner stays whole");
  const fits = partition(facade, third, 1, 2049);
  check(facade.stats().liveBackings === 3, "third backing within the remaining slots");
  // Dropping one backing's children releases only that backing.
  for (const child of left) env.spx_bytes_drop(child);
  check(facade.stats().liveBackings === 2 && facade.stats().releasedBackings === 1, "left backing released alone");
  check(facade.peek(right[2047]).length === 2 && facade.peek(fits[0]).length === 2049, "other backings readable");
  env.spx_vec_drop_v2(facade.vecForWitness(right));
  env.spx_bytes_drop(fits[0]);
  check(facade.stats().releasedBackings === 3, "every backing released once");
  facade.endInvocation(receipt);
});

test("the 32-event limit funds every live backing", () => {
  const { env, facade, receipt } = fresh();
  check(MAX_BACKINGS === MAX_BACKING_EVENTS, "backing table equals the event limit");
  const held = [];
  for (let index = 0; index < MAX_BACKING_EVENTS; index += 1) {
    held.push(...partition(facade, env.spx_bytes_zeroed(2n), 2, 1));
  }
  check(facade.stats().liveBackings === MAX_BACKINGS, "32 simultaneous backings");
  throws(() => env.spx_bytes_zeroed(1n), /allocation budget exceeded/, "a 33rd owner is refused by the old limit");
  // A popped child keeps only its own backing after its sibling drops.
  for (let index = 1; index < held.length; index += 2) env.spx_bytes_drop(held[index]);
  check(facade.stats().liveBackings === MAX_BACKINGS && facade.stats().liveLeases === 32, "siblings settled");
  for (let index = 0; index < held.length; index += 2) env.spx_bytes_drop(held[index]);
  check(facade.stats().releasedBackings === MAX_BACKINGS, "each backing released once");
  facade.endInvocation(receipt);
});

test("zero count settles an empty owner without a backing", () => {
  const { env, facade, receipt } = fresh();
  const empty = env.spx_bytes_zeroed(0n);
  const result = facade.partitionForWitness(empty, 0n, (1n << 64n) - 1n);
  check(result.ok && result.children.length === 0, "empty partition with maximum width");
  check(facade.stats().entries === 0 && facade.stats().liveBackings === 0, "empty owner settled");
  const nonempty = env.spx_bytes_zeroed(3n);
  check(facade.partitionForWitness(nonempty, 0n, 3n).refused === "extent_mismatch", "zero count needs empty input");
  env.spx_bytes_drop(nonempty);
  facade.endInvocation(receipt);
});

test("mutation is isolated to one renewed range", () => {
  const { env, facade, receipt } = fresh();
  const [a, b] = partition(facade, filled(env), 2, 6);
  const sibling = [...facade.peek(b)];
  const renewed = env.spx_bytes_set5(a, 0n, 1, 2, 3, 4, 5);
  check(renewed !== a, "store renews the lease generation");
  throws(() => env.spx_bytes_get(a, 0n), /stale|unleased/, "prior generation is stale");
  check(same([...facade.peek(renewed)], [1, 2, 3, 4, 5, PAYLOAD[5]]), "renewed range observable");
  check(same([...facade.peek(b)], sibling), "sibling untouched");
  throws(() => env.spx_bytes_set(renewed, 6n, 9), /element invariant/, "store past the range is refused");
  check(env.spx_bytes_get(renewed, 0n) === 1, "failed store keeps the current generation");
  throws(() => env.spx_bytes_set5(renewed, 2n, 9, 9, 9, 9, 9), /interval invariant/, "wide store crossing into a sibling");
  check(same([...facade.peek(b)], sibling), "sibling unchanged after refused wide store");
  facade.audit();
  for (const child of [renewed, b]) env.spx_bytes_drop(child);
  check(facade.stats().releasedBackings === 1, "backing released once");
  facade.endInvocation(receipt);
});

test("self-sourced choice store reads before it writes", () => {
  const { env, facade, receipt } = fresh();
  const [a, b] = partition(facade, filled(env), 2, 6);
  const expected = [...facade.peek(a)];
  const renewed = env.spx_bytes_set1_or6_or48(a, 0n, 0, a, 1n << 63n);
  check(same([...facade.peek(renewed)], expected), "six-byte self copy observes pre-store bytes");
  env.spx_bytes_drop(renewed);
  env.spx_bytes_drop(b);
  facade.endInvocation(receipt);
});

test("iteration transfers children that survive Vec, sibling and iterator drop", () => {
  const { env, facade, memory, receipt } = fresh();
  const children = partition(facade, filled(env), 4, 3);
  const vec = facade.vecForWitness(children);
  const out = 65536;
  check(env.spx_iter_bytes_into_v2(vec, out) === 0, "into iterator");
  const view = new DataView(memory.buffer, out, 32);
  const iterator = view.getBigUint64(0, true);
  check(env.spx_iter_bytes_next_v2(iterator, 0n, out) === 0 && view.getUint32(0, true) === 1, "next");
  const popped = BigInt.asIntN(64, view.getBigUint64(8, true));
  const successor = view.getBigUint64(16, true);
  check(popped === children[0], "next transfers the original child carrier");
  check(env.spx_iter_bytes_next_v2(iterator, 0n, out) === 2, "stale iterator handle refused");
  env.spx_iter_bytes_drop_v2(successor, 1n);
  check(facade.stats().liveLeases === 1 && facade.stats().liveBackings === 1, "popped child keeps the backing");
  check(same([...facade.peek(popped)], PAYLOAD.slice(0, 3)), "popped child bytes intact");
  throws(() => env.spx_bytes_drop(children[1]), /double or stale/, "abandoned sibling already settled");
  env.spx_bytes_drop(popped);
  check(facade.stats().releasedBackings === 1 && facade.stats().liveBackings === 0, "last child releases the backing");
  facade.endInvocation(receipt);
});

test("Vec set and clear drop replaced children in order", () => {
  const { env, facade, receipt } = fresh();
  const children = partition(facade, filled(env), 3, 4);
  let vec = facade.vecForWitness(children);
  const replacement = env.spx_bytes_zeroed(2n);
  vec = env.spx_vec_set_v2(vec, 9, 1n, replacement);
  check(vec !== 0n && facade.stats().liveLeases === 2, "set drops only the replaced child");
  const spare = env.spx_bytes_zeroed(1n);
  check(env.spx_vec_set_v2(vec, 9, 7n, spare) === 0n, "out-of-range set refused without transfer");
  env.spx_bytes_drop(spare);
  throws(() => env.spx_vec_push_v2(vec, 9, children[1]), /stale|unleased/, "settled child cannot be pushed");
  vec = env.spx_vec_clear_v2(vec, 9);
  const stats = facade.stats();
  check(stats.liveLeases === 0 && stats.releasedBackings === 1 && stats.entries === 0, "clear settles everything once");
  env.spx_vec_drop_v2(vec);
  facade.endInvocation(receipt);
});

test("Box v2 owns a lease and drops or detaches it exactly once", () => {
  const { env, facade, receipt } = fresh();
  const [a, b] = partition(facade, filled(env), 2, 6);
  const boxed = env.spx_box_new_v2(9, a);
  throws(() => env.spx_box_get_v2(boxed, 9), /cannot be copied/, "owned Box payload get");
  throws(() => env.spx_box_into_inner_v2(boxed, 1), /stale or mistyped Box/, "mistyped Box consume");
  const inner = env.spx_box_into_inner_v2(boxed, 9);
  check(inner === a && same([...facade.peek(inner)], PAYLOAD.slice(0, 6)), "into_inner detaches the same lease");
  throws(() => env.spx_box_drop_v2(boxed), /stale Box drop/, "consumed Box drop");
  const dropped = env.spx_box_new_v2(9, b);
  env.spx_box_drop_v2(dropped);
  check(facade.stats().liveLeases === 1 && facade.stats().boxes === 0, "Box drop settled its lease");
  throws(() => env.spx_box_new_v2(9, b), /stale or malformed/, "settled lease cannot be boxed");
  throws(() => env.spx_box_new_v2(9, BigInt.asIntN(64, (16n << 32n) | 2n)), /token invariant/, "borrowed carrier cannot be boxed");
  const scalar = env.spx_box_new_v2(1, -5n);
  check(env.spx_box_get_v2(scalar, 1) === -5n && env.spx_box_into_inner_v2(scalar, 1) === -5n, "scalar Box v2");
  env.spx_bytes_drop(inner);
  check(facade.stats().releasedBackings === 1, "backing released once");
  facade.endInvocation(receipt);
});

test("deep clone is isolated and keeps ordinary charges", () => {
  const { env, facade, receipt } = fresh();
  const [a, b] = partition(facade, filled(env), 2, 6);
  const before = facade.stats();
  const clone = env.spx_bytes_copy(a);
  const after = facade.stats();
  check(after.events === before.events + 1 && after.payload === before.payload + 6, "clone charged one ordinary event");
  check(decodeOwnedCarrier(clone).token !== decodeOwnedCarrier(a).token, "clone owns fresh backing");
  const original = [...facade.peek(a)];
  const mutatedClone = env.spx_bytes_set(clone, 0n, 0x7a);
  check(same([...facade.peek(a)], original), "clone writes do not reach the lease");
  const mutatedLease = env.spx_bytes_set(a, 1n, 0x7b);
  check(facade.peek(mutatedClone)[1] === original[1], "lease writes do not reach the clone");
  env.spx_bytes_drop(mutatedLease);
  env.spx_bytes_drop(b);
  check(facade.stats().releasedBackings === 1, "children settle the backing");
  check(facade.peek(mutatedClone)[0] === 0x7a, "clone survives backing release");
  env.spx_bytes_drop(mutatedClone);
  facade.endInvocation(receipt);
});

test("stale, foreign, forged and child-partition carriers are rejected", () => {
  const first = fresh(), second = fresh();
  const [a, b] = partition(first.facade, filled(first.env), 2, 6);
  const [foreign] = partition(second.facade, second.env.spx_bytes_zeroed(4n), 1, 4);
  const foreignWhole = second.env.spx_bytes_zeroed(6n);
  const token = decodeOwnedCarrier(a).token;
  throws(() => first.env.spx_bytes_get(carrierOf(token, 7), 0n), /extent invariant/, "forged longer extent");
  throws(() => first.env.spx_bytes_get(carrierOf(token, 5), 0n), /extent invariant/, "forged shorter extent");
  // Never-reused tokens: a foreign lease or owner never names a live entry here.
  check(decodeOwnedCarrier(foreign).token !== token, "foreign token is distinct");
  throws(() => first.env.spx_bytes_get(foreign, 0n), /stale or malformed/, "foreign registry carrier");
  throws(() => first.env.spx_bytes_drop(foreign), /double or stale/, "foreign drop");
  throws(() => first.env.spx_bytes_get(foreignWhole, 0n), /stale or malformed/, "foreign whole owner");
  throws(() => first.facade.partitionForWitness(foreignWhole, 1n, 6n), /stale or malformed/, "foreign partition input");
  throws(() => first.facade.partitionForWitness(a, 1n, 6n), /child partition refused/, "child as partition input");
  throws(() => first.env.spx_bytes_drop(BigInt.asIntN(64, (0n << 32n) | 6n)), /borrowed Bytes drop/, "untagged drop");
  first.env.spx_bytes_drop(a);
  throws(() => first.env.spx_bytes_drop(a), /double or stale/, "double child drop");
  throws(() => first.env.spx_bytes_copy(a), /stale|malformed/, "settled child clone");
  first.env.spx_bytes_drop(b);
  check(first.facade.stats().releasedBackings === 1, "first backing released once");
  const [whole] = partition(first.facade, first.env.spx_bytes_zeroed(5n), 1, 5);
  throws(() => first.facade.partitionForWitness(whole, 1n, 5n), /child partition refused/, "zero-offset whole-extent child");
  first.env.spx_bytes_drop(whole);
  second.env.spx_bytes_drop(foreign);
  second.env.spx_bytes_drop(foreignWhole);
  check(first.facade.stats().releasedBackings === 2 && second.facade.stats().releasedBackings === 1, "each backing released once");
  first.facade.endInvocation(first.receipt);
  second.facade.endInvocation(second.receipt);
});

test("invocation receipts settle before fresh reentry", () => {
  const { host, env, facade, receipt } = fresh();
  throws(() => facade.beginInvocation(), /before fresh reentry/, "nested begin refused");
  const [a, b] = partition(facade, filled(env), 2, 6);
  throws(() => facade.endInvocation(Object.freeze(Object.create(null))), /receipt invariant/, "forged receipt");
  throws(() => facade.endInvocation(receipt), /unsettled/, "unsettled end refused");
  check(facade.stats().liveLeases === 2 && facade.stats().invocation !== 0, "refused end keeps the invocation");
  env.spx_bytes_drop(a);
  env.spx_bytes_drop(b);
  const events = facade.stats().events;
  facade.endInvocation(receipt);
  throws(() => env.spx_bytes_zeroed(1n), /no live invocation/, "idle host refuses every import");
  throws(() => facade.endInvocation(receipt), /receipt invariant/, "receipt cannot end twice");
  const next = facade.beginInvocation();
  check(facade.stats().events === 0 && events > 0, "counters reset only at a fresh begin");
  const carrier = env.spx_bytes_zeroed(2n);
  throws(() => facade.abandon(receipt), /receipt invariant/, "old receipt cannot abandon");
  check(facade.abandon(next) === 0 && facade.stats().entries === 0, "abandon releases the whole owner");
  const third = facade.beginInvocation();
  throws(() => env.spx_bytes_get(carrier, 0n), /stale or malformed/, "prior invocation owner is stale");
  check(decodeOwnedCarrier(env.spx_bytes_zeroed(2n)).token > decodeOwnedCarrier(carrier).token, "tokens never rewind");
  facade.abandon(third);
  check(host.payload.own !== undefined, "payload hooks exported");
});

test("range descriptor views narrow the current lease generation", () => {
  const { env, facade, memory, receipt } = fresh();
  const [a, b] = partition(facade, filled(env), 2, 6);
  const pointer = 4096, identity = 7;
  const descriptor = new DataView(memory.buffer, pointer, 32);
  const write = (root, offset, length) => {
    descriptor.setUint32(0, identity, true);
    descriptor.setUint32(4, pointer, true);
    descriptor.setBigInt64(8, root, true);
    descriptor.setBigUint64(16, BigInt(offset), true);
    descriptor.setBigUint64(24, BigInt(length), true);
    return BigInt.asIntN(64, (BigInt(0x40000000 | (identity << 16) | (pointer / 8)) << 32n) | BigInt(length));
  };
  const view = write(b, 2, 3);
  check(env.spx_bytes_get(view, 0n) === PAYLOAD[8], "view reads inside its lease");
  check(env.spx_bytes_get(view, 3n) === -1, "view read past its extent");
  throws(() => env.spx_bytes_get(write(b, 4, 3), 0n), /extent invariant/, "view escaping its lease");
  const renewed = env.spx_bytes_set(b, 0n, 1);
  throws(() => env.spx_bytes_get(write(b, 0, 2), 0n), /stale|unleased/, "view over a prior generation");
  check(env.spx_bytes_get(write(renewed, 0, 2), 0n) === 1, "view over the current generation");
  env.spx_bytes_drop(a);
  env.spx_bytes_drop(renewed);
  facade.endInvocation(receipt);
});

test("String consumers read leases and copy out ordinary owners", () => {
  const { env, facade, memory, receipt } = fresh();
  const [, unicode] = partition(facade, filled(env), 4, 3);
  const out = 65536;
  check(env.spx_string_from_utf8_v1(unicode, out) === 0, "UTF-8 range accepted");
  const text = BigInt.asIntN(64, new DataView(memory.buffer, out, 8).getBigInt64(0, true));
  check(env.spx_string_len_chars_v1(text) === 2n, "two scalars");
  check(env.spx_string_starts_with_v1(text, unicode) === 1, "lease compares with its copy");
  check(env.spx_string_compare_v2(text, unicode) === 0n, "copy equals lease");
  const invalid = partition(facade, env.spx_bytes_zeroed(0n), 0, 1);
  check(invalid.length === 0, "empty");
  facade.abandon(receipt);
});

test("ordinary 32-event and payload controls are unchanged", () => {
  const { env, facade, receipt } = fresh();
  const owner = env.spx_bytes_zeroed(32n);
  const children = partition(facade, owner, 32, 1);
  const kept = [];
  for (let index = 1; index < MAX_BACKING_EVENTS; index += 1) kept.push(env.spx_bytes_copy(children[index]));
  check(facade.stats().events === MAX_BACKING_EVENTS, "exact 32 events with 32 children issued");
  throws(() => env.spx_bytes_copy(children[0]), /allocation budget exceeded/, "+1 event refused");
  throws(() => env.spx_bytes_zeroed(1n), /allocation budget exceeded/, "+1 constructor refused");
  check(facade.stats().liveLeases === 32, "refused clone leaves the source lease");
  facade.abandon(receipt);
  const payload = fresh();
  const big = payload.env.spx_bytes_zeroed(131072n);
  const halves = partition(payload.facade, big, 2, 65536);
  for (let index = 0; index < 14; index += 1) payload.env.spx_bytes_drop(payload.env.spx_bytes_zeroed(131072n));
  check(payload.facade.stats().payload === 131072 * 15, "drops do not refund payload");
  payload.env.spx_bytes_drop(payload.env.spx_bytes_copy(halves[0]));
  payload.env.spx_bytes_drop(payload.env.spx_bytes_copy(halves[1]));
  check(payload.facade.stats().payload === 2097152, "exact 2 MiB payload including deep clones");
  throws(() => payload.env.spx_bytes_zeroed(1n), /allocation budget exceeded/, "+1 payload byte refused");
  payload.facade.abandon(payload.receipt);
});

test("lease witness charges exactly the ordinary allocation", () => {
  const plain = fresh(), witnessed = fresh({ leaseWitness: true });
  for (const { env } of [plain, witnessed]) {
    // Copy borrows its source; settle both independently owned allocations.
    const source = env.spx_bytes_zeroed(5n);
    const copy = env.spx_bytes_copy(source);
    env.spx_bytes_drop(copy);
    env.spx_bytes_drop(source);
    env.spx_bytes_drop(env.spx_bytes_zeroed(0n));
  }
  const a = plain.facade.stats(), b = witnessed.facade.stats();
  check(a.events === 3 && b.events === 3 && a.payload === 10 && b.payload === 10,
    "two nonempty allocations plus the empty owner keep exact ordinary charges");
  check(b.releasedBackings === 2 && a.releasedBackings === 0, "witness allocations were leases");
  plain.facade.endInvocation(plain.receipt);
  witnessed.facade.endInvocation(witnessed.receipt);
});

test("public copy-out plus provider drop never exports the backing", () => {
  const { env, facade, receipt } = fresh();
  const [a, b] = partition(facade, filled(env), 2, 6);
  const before = facade.stats();
  // Inject snapshot allocation failure at the exact copy-out boundary.
  const OriginalBytes = globalThis.Uint8Array;
  try {
    globalThis.Uint8Array = class extends OriginalBytes {
      constructor(size) {
        if (size === 6) throw new Error("injected snapshot allocation failure");
        super(size);
      }
    };
    throws(() => facade.copyOutAndDrop(a), /injected snapshot allocation failure/,
      "failed snapshot preserves the source lease");
  } finally {
    globalThis.Uint8Array = OriginalBytes;
  }
  const refused = facade.stats();
  check(refused.drops === before.drops && refused.liveLeases === before.liveLeases
    && refused.releasedBackings === before.releasedBackings, "failed snapshot records no settlement");
  check(same([...facade.peek(a)], PAYLOAD.slice(0, 6)), "failed snapshot leaves bytes intact");
  const snapshot = facade.copyOutAndDrop(a);
  check(same([...snapshot], PAYLOAD.slice(0, 6)), "copy-out bytes");
  check(snapshot.byteOffset === 0 && snapshot.buffer.byteLength === 6, "snapshot is a fresh exact buffer");
  throws(() => env.spx_bytes_get(a, 0n), /stale|unleased/, "provider drop settled the lease");
  snapshot[0] = 0x55;
  check(facade.peek(b)[0] === PAYLOAD[6], "snapshot writes cannot reach siblings");
  facade.copyOutAndDrop(b);
  check(facade.stats().releasedBackings === 1 && facade.stats().liveBackings === 0, "final provider drop releases once");
  facade.endInvocation(receipt);
});

test("generation exhaustion refuses before writing", () => {
  const { env, facade, receipt } = fresh();
  let [child] = partition(facade, env.spx_bytes_zeroed(1n), 1, 1);
  for (let generation = 1; generation < MAX_LEASE_GENERATION; generation += 1) {
    child = env.spx_bytes_set(child, 0n, generation & 255);
  }
  const last = env.spx_bytes_get(child, 0n);
  throws(() => env.spx_bytes_set(child, 0n, 0), /generation exhausted/, "exhausted generation");
  check(env.spx_bytes_get(child, 0n) === last, "refused store wrote nothing and kept the generation");
  env.spx_bytes_drop(child);
  facade.endInvocation(receipt);
});

test("abandonment after a trap releases remaining administration once", () => {
  const { env, facade, receipt } = fresh();
  const children = partition(facade, filled(env), 3, 4);
  env.spx_bytes_drop(children[0]);
  check(facade.abandon(receipt) === 1, "abandon releases the backing once");
  check(facade.stats().liveLeases === 0 && facade.stats().liveBackings === 0, "nothing hidden stays live");
  throws(() => env.spx_bytes_get(children[1], 0n), /no live invocation/, "idle after abandonment");
  const next = facade.beginInvocation();
  throws(() => env.spx_bytes_get(children[1], 0n), /stale|malformed/, "abandoned child is stale");
  facade.endInvocation(next);
});

test("plain roots preserve external bounds and private memory pages", () => {
  const { host, env, facade, receipt } = fresh();
  const carrier = (root, extent) => (BigInt(root) << 32n) | BigInt(extent);
  check(env.spx_bytes_get(carrier(0, 65536), 65535n) === 0, "exact external extent");
  throws(() => env.spx_bytes_get(carrier(0, 65537), 0n), /length invariant/, "external extent plus one");
  facade.endInvocation(receipt);
  for (const pages of [1, 3, 5]) {
    const invalid = fresh({memoryPages: pages});
    throws(() => invalid.env.spx_bytes_get(carrier(0, 1), 0n), /memory invariant/, "nonprofile memory size");
    invalid.facade.endInvocation(invalid.receipt);
  }
  const literal = fresh({memoryPages: 4});
  new Uint8Array(literal.memory.buffer)[196608] = 42;
  check(literal.env.spx_bytes_get(carrier(196608, 1), 0n) === 42, "fourth page literal");
  throws(() => literal.env.spx_bytes_get(carrier(131072, 1), 0n), /range invariant/, "private third page");
  throws(() => literal.env.spx_bytes_get(carrier(196607, 2), 0n), /range invariant/, "crossing literal boundary");
  throws(() => literal.env.spx_bytes_get(carrier(262143, 2), 0n), /range invariant/, "literal extent overflow");
  literal.facade.endInvocation(literal.receipt);
});

test("simultaneous host Vec and iterator authorities never collide", () => {
  const first = fresh(), second = fresh();
  const a = first.env.spx_vec_with_capacity_v2(9, 1n);
  const b = second.env.spx_vec_with_capacity_v2(9, 1n);
  check(a !== b, "Vec namespace is shared across populated hosts");
  throws(() => first.env.spx_vec_len_v2(b, 9), /stale/, "foreign Vec");
  check(first.env.spx_iter_bytes_into_v2(a, 65536) === 0, "first iterator");
  check(second.env.spx_iter_bytes_into_v2(b, 65536) === 0, "second iterator");
  const ia = new DataView(first.memory.buffer).getBigUint64(65536, true);
  const ib = new DataView(second.memory.buffer).getBigUint64(65536, true);
  check(ia !== ib, "iterator namespace is shared across populated hosts");
  check(first.env.spx_iter_bytes_next_v2(ib, 0n, 65536) === 2, "foreign iterator refused");
  first.env.spx_iter_bytes_drop_v2(ia, 0n);
  second.env.spx_iter_bytes_drop_v2(ib, 0n);
  first.facade.endInvocation(first.receipt);
  second.facade.endInvocation(second.receipt);
});

// A separately evaluated host module cannot adopt the marker or rewind mint
// state. The installed namespace exposes no duck-typed mint authority.
try {
  await import("./host.mjs?duplicate-copy");
  throw new Error("duplicate host module was admitted");
} catch (error) {
  check(/namespace already installed/.test(error.message), "duplicate module refuses before populating a host");
}

let failed = 0;
for (const [name, body] of cases) {
  try { body(); } catch (error) { failed += 1; console.error(`${name}: ${error.stack}`); }
}
if (failed !== 0) process.exit(1);
console.log(`shared-owner-ok ${cases.length}`);
