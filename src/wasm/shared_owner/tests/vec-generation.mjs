import assert from "node:assert/strict";
import { createPrivateOrdinaryByteHost } from "./host.mjs";

const { bind, imports: env, facade } = createPrivateOrdinaryByteHost();
const memory = new WebAssembly.Memory({ initial: 2, maximum: 2 });
bind({ exports: { memory } });
const invocation = facade.beginInvocation();
const bytesTag = 9;
const vec = env.spx_vec_with_capacity_v2(bytesTag, 2n);
const [child] = facade.partitionForWitness(env.spx_bytes_zeroed(1n), 1n, 1n).children;
const current = env.spx_vec_push_v2(vec, bytesTag, child);
const [partitioned, status] = env.spx_bytes_partition(env.spx_bytes_zeroed(2n), 2n, 1n);
assert.equal(status, 0);
const replacement = env.spx_bytes_zeroed(1n);
const generation = /private handle space exhausted/;

assert.equal(env.spx_vec_push_v2(partitioned, bytesTag, replacement), 0n);
assert.equal(env.spx_vec_set_v2(current, bytesTag, 1n, replacement), 0n);
assert.throws(() => env.spx_vec_push_v2(current, bytesTag, replacement), generation);
assert.throws(() => env.spx_vec_reserve_exact_v2(current, bytesTag, 4n), generation);
assert.throws(() => env.spx_vec_set_v2(current, bytesTag, 0n, replacement), generation);
assert.throws(() => env.spx_vec_clear_v2(partitioned, bytesTag), generation);
assert.equal(env.spx_vec_len_v2(current, bytesTag), 1n);
assert.equal(env.spx_vec_capacity_v2(current, bytesTag), 2n);
assert.equal(env.spx_vec_len_v2(partitioned, bytesTag), 2n);
assert.equal(facade.stats().liveLeases, 3);
assert.equal(facade.stats().drops, 0);

env.spx_vec_drop_v2(partitioned);
env.spx_vec_drop_v2(current);
env.spx_bytes_drop(replacement);
assert.equal(facade.stats().releasedBackings, 2);
facade.endInvocation(invocation);
