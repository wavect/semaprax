export type ScalarStatus = Readonly<{ schema: "semaprax.status.v1"; domain_id: "semaprax.arithmetic.v1"; code: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 }> | Readonly<{ schema: "semaprax.status.v1"; domain_id: "semaprax.contract.v1"; code: 1 | 2 }>;
export type ScalarResult<T> = Readonly<{ ok: true; value: T }> | Readonly<{ ok: false; status: ScalarStatus }>;
export interface ScalarFunctions {
  readonly "callback.advance": (arg0: bigint, arg1: bigint) => ScalarResult<bigint>;
}
export interface ScalarRuntime { readonly functions: Readonly<ScalarFunctions>; call<I extends keyof ScalarFunctions>(id: I, ...args: Parameters<ScalarFunctions[I]>): ReturnType<ScalarFunctions[I]>; }
export declare function instantiateBytes(bytes: ArrayBuffer | ArrayBufferView): Promise<ScalarRuntime>;
export declare function instantiate(url?: URL | string): Promise<ScalarRuntime>;
export declare const exportIds: readonly (keyof ScalarFunctions)[];
