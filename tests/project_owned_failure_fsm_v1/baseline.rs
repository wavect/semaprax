//! Frozen literal formats inspected at 3983d85, not calls to current renderers.
//! Descriptor bytes/digest are authenticated fixture inputs, NOT historical
//! descriptor evidence. The metadata assertions bind those exact inputs and
//! independently hashed Wasm to the complete old metadata format. Historical
//! descriptor/Wasm preservation remains with the unchanged production paths
//! and existing known-answer gates; this file does not duplicate either emitter.

use sha2::{Digest, Sha256};

const PACKAGE: &str = concat!(
    "{\"name\":\"owned-fsm\",\"version\":\"0.1.0\",\"type\":\"module\",\"sideEffects\":false,",
    "\"exports\":{\".\":{\"types\":\"./semaprax.bindings.d.ts\",\"import\":\"./semaprax.bindings.js\"},",
    "\"./app.wasm\":\"./app.wasm\",\"./manifest\":\"./semaprax.api.json\"},",
    "\"types\":\"./semaprax.bindings.d.ts\",\"files\":[\"app.wasm\",\"semaprax.js\",",
    "\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\"]}\n"
);
const INVENTORY: &str = "[\"app.wasm\",\"semaprax.js\",\"semaprax.bindings.js\",\"semaprax.bindings.d.ts\",\"semaprax.api.json\",\"package.json\"]";
const RECORD: &str = "SpxRecordId636173652e7061636b6574";
const FIELDS: [&str; 4] = [
    "spx_field_id_636173652e7061796c6f6164",
    "spx_field_id_636173652e6b696e64",
    "spx_field_id_636173652e76616c6964",
    "spx_field_id_636173652e73697a65",
];
const TAIL: &str = concat!(
    "export interface SemapraxRuntime { readonly functions: Readonly<SemapraxApi>; call<I extends keyof SemapraxApi>(id: I, ...args: Parameters<SemapraxApi[I]>): ReturnType<SemapraxApi[I]>; readonly wasmSha256: string; }\n",
    "export declare function instantiate(bytes: Uint8Array): Promise<SemapraxRuntime>;\n",
    "export declare const exportIds: readonly (keyof SemapraxApi)[];\nexport default instantiate;\n"
);
const RECORD_TAIL: &str = concat!(
    "export interface SemapraxRuntime { readonly functions: Readonly<SemapraxApi>; call<I extends keyof SemapraxApi>(id:I,...args:Parameters<SemapraxApi[I]>):ReturnType<SemapraxApi[I]>; readonly wasmSha256:string; }\n",
    "export declare function instantiate(bytes:Uint8Array):Promise<SemapraxRuntime>;\n",
    "export declare const exportIds:readonly(keyof SemapraxApi)[];\nexport default instantiate;\n"
);

pub(super) fn assert_artifacts(
    rows: &[(String, Vec<u8>)],
    config: &serde_json::Value,
    descriptor: &[u8],
    descriptor_digest: &str,
) {
    let family = config["family"].as_str().unwrap();
    let record = family == "record";
    let variant = family == "variant";
    let mixed = family == "mixed";
    let utf8 = config["utf8"].as_bool().unwrap();
    let v10 = config["schema"] == "v10";
    let result = if record {
        RECORD
    } else if variant {
        "SemapraxResult<Uint8Array, bigint>"
    } else {
        "Uint8Array"
    };
    let mut types = if record {
        assert_eq!(config["fields"], serde_json::json!(FIELDS));
        format!("export interface {RECORD} {{\n  readonly {}: Uint8Array;\n  readonly {}: bigint;\n  readonly {}: boolean;\n  readonly {}: bigint;\n}}\n", FIELDS[0], FIELDS[1], FIELDS[2], FIELDS[3])
    } else if variant {
        "export type OptionalBytes = Uint8Array | null;\nexport type SemapraxResult<T, E> =\n  | { readonly ok: true; readonly value: T }\n  | { readonly ok: false; readonly error: E };\n".to_owned()
    } else {
        String::new()
    };
    types.push_str(&format!("export interface SemapraxApi {{\n  readonly \"case.before\": (arg0: Uint8Array, arg1: bigint) => {result};\n  readonly \"case.copy\": (arg0: Uint8Array, arg1: bigint) => {result};\n"));
    if variant {
        types.push_str("  readonly \"case.err\": () => SemapraxResult<Uint8Array, bigint>;\n  readonly \"case.none\": () => OptionalBytes;\n");
    }
    if mixed {
        types.push_str("  readonly \"case.flag\": (arg0: boolean) => boolean;\n");
    }
    if utf8 {
        types.push_str("  readonly \"case.text\": () => string;\n");
    }
    types.push_str(&format!(
        "  readonly \"case.utf8\": (arg0: string) => {result};\n}}\n"
    ));
    types.push_str(if record { RECORD_TAIL } else { TAIL });
    assert_eq!(rows[3].1, types.as_bytes(), "complete frozen TypeScript");
    assert_eq!(
        rows[5].1,
        PACKAGE.as_bytes(),
        "complete frozen package JSON"
    );

    let quote = |value: &str| serde_json::to_string(value).unwrap();
    let descriptor = quote(std::str::from_utf8(descriptor).unwrap());
    let digest = quote(descriptor_digest);
    let hash = Sha256::digest(&rows[0].1)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let metadata = if record {
        format!("{{\"schema\":\"semaprax.flat-owned-record-api.v1\",\"descriptor\":{descriptor},\"descriptor_digest\":{digest},\"wasm_sha256\":\"sha256:{hash}\",\"result_carrier\":\"opaque-handle-plus-scalars.v1\",\"settlement\":{{\"copy_before_settle\":true,\"publish_after_settle\":true,\"failure_slot_unchanged\":true}},\"artifacts\":{INVENTORY}}}\n")
    } else {
        // Exact fixture export facts, not introspection of emitted metadata.
        let result = if variant {
            "result-owned-bytes-i64"
        } else {
            "owned-bytes"
        };
        let mut targets = vec![
            (
                "case.before",
                "636173652e6265666f7265",
                "\"borrow-slice-u8\",\"i64\"",
                result,
            ),
            (
                "case.copy",
                "636173652e636f7079",
                "\"borrow-slice-u8\",\"i64\"",
                result,
            ),
        ];
        if variant {
            targets.extend([
                ("case.err", "636173652e657272", "", "result-owned-bytes-i64"),
                ("case.none", "636173652e6e6f6e65", "", "option-owned-bytes"),
            ]);
        }
        if mixed {
            targets.push(("case.flag", "636173652e666c6167", "\"bool\"", "bool"));
        }
        if utf8 {
            targets.push(("case.text", "636173652e74657874", "", "owned-utf8"));
        }
        targets.push(("case.utf8", "636173652e75746638", "\"borrow-str\"", result));
        let targets = targets.iter().map(|(id, hex, params, result)| format!("{{\"stable_id\":\"{id}\",\"wasm_export\":\"spx_owned_v1_{hex}\",\"parameters\":[{params}],\"result\":\"{result}\",\"call\":\"(parameters..., result_out: i32) -> status: i32\"}}")).collect::<Vec<_>>().join(",");
        let schema = if v10 {
            "semaprax.owned-utf8-api.v1"
        } else {
            "semaprax.owned-data-api.v1"
        };
        let utf8_settlement = if v10 {
            ",\"utf8_before_publication\":true"
        } else {
            ""
        };
        format!("{{\"schema\":\"{schema}\",\"package\":\"owned-fsm\",\"version\":\"0.1.0\",\"descriptor\":{descriptor},\"descriptor_digest\":{digest},\"wasm\":{{\"path\":\"app.wasm\",\"sha256\":\"{hash}\"}},\"limits\":{{\"borrowed_input_bytes\":65536,\"owned_output_bytes\":65536}},\"target\":[{targets}],\"settlement\":{{\"copy_before_consume\":true,\"consume_exactly_once\":true,\"require_empty_arena\":true,\"poison_result_memory\":true{utf8_settlement}}},\"artifacts\":{INVENTORY}}}\n")
    };
    assert_eq!(
        rows[4].1,
        metadata.as_bytes(),
        "complete frozen API metadata"
    );
}
