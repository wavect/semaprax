//! Private deterministic WIT/component-boundary evidence.
//!
//! This freezes one scalar result/status interface and its JavaScript adapter;
//! it does not claim Component Model binary emission, resources, or public WIT
//! import/export.

use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"SPXWIT01";

const WIT: &str = "package semaprax:private@0.1.0;\n\ninterface evaluation {\n  record status { domain: string, code: u32, class: u8, retryable: option<bool> }\n  evaluate: func(left: s64, right: s64) -> result<s64, status>;\n}\n\nworld semaprax-private-v1 {\n  export evaluation;\n}\n";

const SCHEMA: &str = "{\"abi\":\"wasm-component-canonical-v1\",\"copy\":{\"status.domain\":\"utf8-copy\"},\"interface\":\"semaprax:private/evaluation@0.1.0\",\"mapping\":{\"status.domain\":\"semaprax.status.v1.domain_id\"},\"result\":{\"err\":\"status\",\"ok\":\"s64\"},\"version\":1}";

const JAVASCRIPT: &str = r#"export function normalizeEvaluation(result) {
  if (result === null || typeof result !== "object") throw new TypeError("SPX-WIT-RESULT");
  const keys = Object.keys(result);
  if (keys.length !== 1 || (keys[0] !== "ok" && keys[0] !== "err")) throw new TypeError("SPX-WIT-TAG");
  if (keys[0] === "ok") {
    if (typeof result.ok !== "bigint") throw new TypeError("SPX-WIT-I64");
    return { ok: result.ok };
  }
  const value = result.err;
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw new TypeError("SPX-WIT-STATUS");
  const statusKeys = Object.keys(value).sort().join(",");
  const domainBytes = typeof value.domain === "string" ? new TextEncoder().encode(value.domain) : null;
  if (statusKeys !== "class,code,domain,retryable" || domainBytes === null ||
      domainBytes.length < 1 || domainBytes.length > 255 || domainBytes.includes(0) ||
      typeof value.code !== "number" || !Number.isInteger(value.code) || value.code <= 0 || value.code > 0xFFFF_FFFF ||
      typeof value.class !== "number" || !Number.isInteger(value.class) || value.class < 1 || value.class > 5 ||
      !(value.retryable === null || typeof value.retryable === "boolean")) throw new TypeError("SPX-WIT-STATUS");
  return { err: { domain: value.domain, code: value.code, class: value.class, retryable: value.retryable } };
}
"#;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateWitBundleV1 {
    pub wit: &'static str,
    pub schema_json: &'static str,
    pub javascript_adapter: &'static str,
    pub digest: [u8; 32],
    bytes: Vec<u8>,
}

#[must_use]
pub fn emit_private_wit_bundle_v1() -> PrivateWitBundleV1 {
    let mut bytes = Vec::with_capacity(20 + WIT.len() + SCHEMA.len() + JAVASCRIPT.len());
    bytes.extend_from_slice(MAGIC);
    for field in [WIT.as_bytes(), SCHEMA.as_bytes(), JAVASCRIPT.as_bytes()] {
        bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
        bytes.extend_from_slice(field);
    }
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    PrivateWitBundleV1 {
        wit: WIT,
        schema_json: SCHEMA,
        javascript_adapter: JAVASCRIPT,
        digest,
        bytes,
    }
}

impl PrivateWitBundleV1 {
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

pub fn verify_private_wit_bundle_v1(candidate: &[u8]) -> Result<(), &'static str> {
    if candidate.len() < 20 || &candidate[..8] != MAGIC {
        return Err("SPX-WIT001");
    }
    let expected = emit_private_wit_bundle_v1();
    if candidate != expected.bytes() {
        return Err("SPX-WIT002");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_is_deterministic_canonical_and_mutation_closed() {
        let first = emit_private_wit_bundle_v1();
        let second = emit_private_wit_bundle_v1();
        assert_eq!(first, second);
        assert_eq!(
            first.digest,
            [
                200, 87, 216, 77, 97, 89, 252, 47, 85, 127, 9, 235, 144, 37, 72, 248, 80, 235, 230,
                207, 131, 97, 230, 191, 65, 61, 23, 241, 160, 238, 48, 192,
            ]
        );
        assert_eq!(verify_private_wit_bundle_v1(first.bytes()), Ok(()));
        assert!(first.wit.contains("result<s64, status>"));
        assert!(!first.wit.contains("resource"));
        for index in 0..first.bytes().len() {
            let mut hostile = first.bytes().to_vec();
            hostile[index] ^= 1;
            assert_eq!(
                verify_private_wit_bundle_v1(&hostile),
                Err(if index < 8 {
                    "SPX-WIT001"
                } else {
                    "SPX-WIT002"
                })
            );
        }
        for end in 0..first.bytes().len() {
            assert!(verify_private_wit_bundle_v1(&first.bytes()[..end]).is_err());
        }
        let mut trailing = first.bytes().to_vec();
        trailing.push(0);
        assert_eq!(verify_private_wit_bundle_v1(&trailing), Err("SPX-WIT002"));
    }

    #[test]
    fn node_executes_exact_javascript_result_adapter() {
        let script=format!("{}\nconst a=normalizeEvaluation({{ok:7n}});const b=normalizeEvaluation({{err:{{domain:'fixture.v1',code:7,class:3,retryable:false}}}});if(a.ok!==7n||b.err.code!==7)process.exit(91);const s=(domain,code=7)=>({{err:{{domain,code,class:3,retryable:null}}}});for(const x of [{{ok:7}},{{}},s('x',0),s('x',0x1_0000_0000),s('',7),s('a'.repeat(256),7),s('a\\0b',7),s('€'.repeat(86),7)]){{let rejected=false;try{{normalizeEvaluation(x)}}catch(_e){{rejected=true}}if(!rejected)process.exit(92)}}const max=normalizeEvaluation(s('a'.repeat(255),0xFFFF_FFFF));if(max.err.code!==0xFFFF_FFFF)process.exit(93)",JAVASCRIPT);
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "--eval", &script])
            .output()
            .expect("Node is required by the existing Wasm quality gate");
        assert!(
            output.status.success(),
            "Node adapter failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
