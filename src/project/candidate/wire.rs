use std::fmt::Write as _;
use std::io::{self, Write};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::{
    capacity, invalid, stale, ProjectRevision, MAX_PROJECT_CANDIDATE_BYTES,
    MAX_SEMANTIC_CHANGE_BYTES,
};
use crate::diagnostic::Diagnostic;

pub(super) fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    {
        return Err(invalid(
            "candidate revision must be a canonical SHA-256 digest",
        ));
    }
    Ok(())
}

/// Check caller-owned typed data before cloning or recursive serialization.
pub(super) fn validate_value(value: &Value) -> Result<(), Vec<Diagnostic>> {
    let mut stack = vec![(value, 0usize)];
    let mut nodes = 0usize;
    let mut bytes = 0usize;
    while let Some((value, depth)) = stack.pop() {
        nodes += 1;
        if nodes > 8192 || depth > 64 {
            return Err(capacity(
                "semantic intention exceeds its node or depth limit",
            ));
        }
        match value {
            Value::Array(items) => {
                if items.len() > 8192usize.saturating_sub(nodes + stack.len()) {
                    return Err(capacity("semantic intention array exceeds its node limit"));
                }
                stack.extend(items.iter().map(|item| (item, depth + 1)));
            }
            Value::Object(items) => {
                if items.len() > 8192usize.saturating_sub(nodes + stack.len()) {
                    return Err(capacity("semantic intention object exceeds its node limit"));
                }
                for (key, item) in items {
                    bytes = bytes
                        .checked_add(key.len())
                        .ok_or_else(|| capacity("semantic intention size overflow"))?;
                    stack.push((item, depth + 1));
                }
            }
            Value::String(text) => {
                bytes = bytes
                    .checked_add(text.len())
                    .ok_or_else(|| capacity("semantic intention size overflow"))?
            }
            _ => {}
        }
        if bytes > MAX_SEMANTIC_CHANGE_BYTES {
            return Err(capacity(
                "semantic intention strings exceed the input limit",
            ));
        }
    }
    Ok(())
}

pub(super) fn digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

pub(super) fn render(mut value: Value, limit: usize) -> Result<String, Vec<Diagnostic>> {
    struct Sink {
        bytes: Vec<u8>,
        limit: usize,
    }
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(io::Error::other("candidate output limit"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    value.sort_all_objects();
    let mut sink = Sink {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut sink, &value)
        .map_err(|_| capacity("candidate JSON exceeds its output limit"))?;
    sink.write_all(b"\n")
        .map_err(|_| capacity("candidate JSON exceeds its output limit"))?;
    String::from_utf8(sink.bytes).map_err(|_| invalid("candidate JSON is not UTF-8"))
}

pub(super) fn target_facts(revision: &ProjectRevision) -> Result<Value, Vec<Diagnostic>> {
    let mut targets = Vec::new();
    for (role, program) in [
        ("entry", revision.entry_program()),
        ("test", revision.test_program()),
    ] {
        let (native, overflow) = crate::bounded_output::with_limit(16 * 1024 * 1024, || {
            crate::codegen::emit_hir_c(program)
        });
        if overflow {
            return Err(capacity(
                "native target projection exceeds candidate admission budget",
            ));
        }
        targets.push(match native {
            Ok(source) => {
                if source.len() > 16 * 1024 * 1024 { return Err(capacity("native target projection exceeds candidate byte budget")); }
                json!({"role": role, "lane": "native_c11", "admitted": true, "bytes": source.len(), "digest": digest(b"semaprax.candidate.native-c11.v1\0", source.as_bytes()), "validation": "compiler_emission_not_native_execution"})
            }
            Err(error) => json!({"role": role, "lane": "native_c11", "admitted": false, "diagnostic": error.code}),
        });
        let (wasm, overflow) = crate::bounded_output::with_limit(16 * 1024 * 1024, || {
            crate::wasm::emit_resolved_module(program)
        });
        if overflow {
            return Err(capacity(
                "Wasm target projection exceeds candidate admission budget",
            ));
        }
        targets.push(match wasm {
            Ok(bytes) => {
                if bytes.len() > 16 * 1024 * 1024 { return Err(capacity("Wasm target projection exceeds candidate byte budget")); }
                wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all()).validate_all(&bytes)
                    .map_err(|_| invalid("candidate compiler-emitted Wasm failed structural validation"))?;
                json!({"role": role, "lane": "wasm_core", "admitted": true, "bytes": bytes.len(), "digest": digest(b"semaprax.candidate.wasm-core.v1\0", &bytes), "validation": "wasmparser_structural_not_execution", "validator": "0.258.0"})
            }
            Err(error) => json!({"role": role, "lane": "wasm_core", "admitted": false, "diagnostic": error.code}),
        });
    }
    Ok(json!(targets))
}

pub(super) fn preserve_targets(base: &Value, candidate: &Value) -> Result<(), Vec<Diagnostic>> {
    let base = base
        .as_array()
        .ok_or_else(|| invalid("invalid retained target inventory"))?;
    let candidate = candidate
        .as_array()
        .ok_or_else(|| invalid("invalid candidate target inventory"))?;
    if base.len() != candidate.len() {
        return Err(stale("candidate target inventory changed"));
    }
    for (before, after) in base.iter().zip(candidate) {
        if before["role"] != after["role"]
            || before["lane"] != after["lane"]
            || (before["admitted"] == true && after["admitted"] != true)
        {
            return Err(invalid(
                "candidate invalidated a previously admitted core target",
            ));
        }
    }
    Ok(())
}

/// A deterministic single-hunk unified diff, retaining exact changed lines.
pub(super) fn source_diff(
    path: &str,
    before: &str,
    after: &str,
) -> Result<String, Vec<Diagnostic>> {
    let before = before.lines().collect::<Vec<_>>();
    let after = after.lines().collect::<Vec<_>>();
    let prefix = before
        .iter()
        .zip(&after)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = before[prefix..]
        .iter()
        .rev()
        .zip(after[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_count = before.len() - prefix - suffix;
    let new_count = after.len() - prefix - suffix;
    let (diff, overflow) = crate::bounded_output::with_limit(MAX_PROJECT_CANDIDATE_BYTES, || {
        let mut out = crate::bounded_output::CappedString::new();
        let old_start = prefix + usize::from(old_count != 0);
        let new_start = prefix + usize::from(new_count != 0);
        write!(
            out,
            "--- a/{path}\n+++ b/{path}\n@@ -{old_start},{old_count} +{new_start},{new_count} @@\n"
        )
        .expect("bounded string write");
        for line in &before[prefix..before.len() - suffix] {
            writeln!(out, "-{line}").expect("bounded string write");
        }
        for line in &after[prefix..after.len() - suffix] {
            writeln!(out, "+{line}").expect("bounded string write");
        }
        out.into_string()
    });
    if overflow {
        return Err(capacity("candidate source diff exceeds its bound"));
    }
    Ok(diff)
}
