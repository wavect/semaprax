// Compiler-paired private entry. No caller module, imports, issuer or witness.
import { createPrivateOrdinaryByteHost } from "./host.mjs";
const moduleHex = "$MODULE_HEX";
const statuses = new Map($STATUS_TABLE);
export async function install(managedBytes) {
  if (!Number.isSafeInteger(managedBytes) || managedBytes < 1) {
    throw new RangeError("positive finite managed-byte account required");
  }
  const host = createPrivateOrdinaryByteHost({ managedBytes });
  const module = Uint8Array.from(moduleHex.match(/../g), byte => parseInt(byte, 16));
  const min = -(1n << 63n);
  const max = (1n << 63n) - 1n;
  const fail = status => { throw new Error(`status:${status}`); };
  const checked = (value, status) => {
    if (value < min || value > max) fail(status);
    return value;
  };
  const env = {
    spx_add: (a, b) => checked(a + b, $ADD_OVERFLOW),
    spx_sub: (a, b) => checked(a - b, $SUB_OVERFLOW),
    spx_mul: (a, b) => checked(a * b, $MUL_OVERFLOW),
    spx_div: (a, b) => {
      if (b === 0n) fail($DIV_ZERO);
      if (a === min && b === -1n) fail($DIV_OVERFLOW);
      return a / b;
    },
    spx_rem: (a, b) => {
      if (b === 0n) fail($REM_ZERO);
      if (a === min && b === -1n) fail($REM_OVERFLOW);
      return a % b;
    },
    spx_neg: a => checked(-a, $NEG_OVERFLOW),
    spx_contract_fail: fail,
    ...host.imports,
  };
  const { instance } = await WebAssembly.instantiate(module, { env });
  host.bind(instance);
  let poisoned = false;
  const administrative = reason => Object.freeze({ outcome: "administrative_refusal", reason });
  return Object.freeze({
    account: () => Object.freeze({ limit: managedBytes, reserved: host.facade.stats().managedReserved,
      peak: host.facade.stats().managedPeak }),
    invoke() {
      if (poisoned) return administrative("private entry is poisoned");
      const receipt = host.facade.beginInvocation();
      let value;
      let status;
      try {
        try { value = instance.exports.semaprax_main(); }
        catch (error) {
          const match = /^status:([0-9]+)$/.exec(String(error.message));
          if (match === null || !statuses.has(Number(match[1]))) throw error;
          status = statuses.get(Number(match[1]));
        }
        if (status === undefined && typeof value !== "bigint") {
          throw new Error("SEMAPRAX private entry result type invariant");
        }
        const outcome = status !== undefined
          ? Object.freeze({ outcome: "language_failure", status: Object.freeze({ domain: status.domain, code: status.code }) })
          : Object.freeze({ outcome: "returned", result: value.toString() });
        // Construct privately, then settle, then publish. No result escapes cleanup.
        host.facade.endInvocation(receipt);
        return outcome;
      } catch (error) {
        poisoned = true;
        host.facade.abandon(receipt);
        return administrative(String(error.message));
      }
    },
  });
}
