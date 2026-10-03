//! Bounded, read-only projection from a replayed Rust API index. The index is
//! discovery metadata; this route never prepares a tool or a callable adapter.

use super::*;
use crate::rust_api_index::{
    ItemKind, Receiver, RejectionReason, RustApiIndex, Support, Visibility,
};

const CANDIDATE_SCHEMA: &str = "semaprax.rust-api-candidates.v1";

/// A pure, prefix-bounded discovery projection over prepared index bytes.
/// It does not assert that a source declaration is valid or compile a wrapper.
pub fn prepared_rust_api_candidates_json(
    index_bytes: &[u8],
    prefix: &str,
    requested_max_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    if prefix.len() > crate::rust_api_index::MAX_PATH_BYTES
        || !prefix
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b':')
    {
        return Err(vec![Diagnostic::io("SPX-B141", "Rust API candidate prefix must contain only path characters and fit the index path bound")]);
    }
    let index = RustApiIndex::replay(index_bytes).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-B148",
            format!("prepared Rust API index cannot be replayed: {error:?}"),
        )]
    })?;
    let limit = requested_max_bytes.min(MAX_BYTES);
    let alias = index
        .package()
        .renamed_from
        .as_deref()
        .unwrap_or(&index.package().name);
    let matches = index
        .items()
        .iter()
        .filter(|item| item.visibility == Visibility::Public && item.path.starts_with(prefix))
        .collect::<Vec<_>>();
    let matches_len = matches.len();
    let mut items = Vec::new();
    let candidate = |items: &[Value]| {
        render(json!({
            "authority": {"execution": false, "publication": false, "tool_invocation": false},
            "budget": {"max_bytes": limit, "used_bytes": 0},
            "index": {"digest": index.digest(), "status": "prepared_metadata"},
            "items": items,
            "package": {"cargo_alias": alias, "feature_digest": index.feature_digest(), "name": index.package().name, "source_sha256": index.package().source_sha256, "stable_rustc_version": index.stable_rustc_version(), "target": index.target(), "version": index.package().version},
            "prefix": prefix,
            "schema": CANDIDATE_SCHEMA,
            "truncation": {"omitted_items": matches_len - items.len(), "truncated": items.len() < matches_len},
        }))
    };
    let minimum = candidate(&items);
    if minimum.len() > limit {
        return Err(vec![Diagnostic::io("SPX-G004", format!("Rust API candidate context requires at least {} output bytes but max_bytes is {limit}", minimum.len()))]);
    }
    for item in matches {
        let (support, reason) = match &item.support {
            Support::Supported => ("supported", None),
            Support::Rejected { reason } => ("rejected", Some(reason_name(*reason))),
        };
        items.push(json!({"kind": kind_name(item.kind), "manual_adapter": manual_adapter(&item.support), "ownership": receiver_name(item.receiver), "path": item.path, "reason": reason, "signature": item.signature, "support": support}));
        if candidate(&items).len() > limit {
            items.pop();
            break;
        }
    }
    Ok(candidate(&items))
}

/// Inspect one declared selected import against exact prepared index bytes.
/// Source verification still admits only the expected unbound-index state.
pub fn prepared_selected_rust_import_context_json(
    program: &Program,
    symbol: &str,
    index_bytes: &[u8],
    requested_max_bytes: usize,
) -> Result<Option<String>, Vec<Diagnostic>> {
    if selected_rust_import_context_json(program, symbol, MAX_BYTES)?.is_none() {
        return Ok(None);
    }
    let import = program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
        .find(|import| {
            import.index_selected
                && (import.stable_id == symbol
                    || import.name == symbol
                    || import.rust_path.as_deref() == Some(symbol))
        })
        .expect("unambiguous selected import was checked above");
    let index = RustApiIndex::replay(index_bytes).map_err(|error| {
        vec![Diagnostic::error(
            "SPX-B148",
            format!("prepared Rust API index cannot be replayed: {error:?}"),
            import.span,
        )]
    })?;
    let path = import
        .rust_path
        .as_deref()
        .expect("selected path verified above");
    let path_root = path
        .split("::")
        .next()
        .expect("selected path verified above");
    let alias = index
        .package()
        .renamed_from
        .as_deref()
        .unwrap_or(&index.package().name);
    if path_root != index.package().name.as_str() && path_root != alias {
        return Err(vec![Diagnostic::error(
            "SPX-B143",
            "selected Rust API path disagrees with the prepared package identity",
            import.span,
        )]);
    }
    let item = index
        .items()
        .iter()
        .find(|item| item.path == path)
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "SPX-B141",
                "selected Rust API path is absent from the prepared index",
                import.span,
            )]
        })?;
    let (support, reason) = match &item.support {
        Support::Supported => ("supported", None),
        Support::Rejected { reason } => ("rejected", Some(reason_name(*reason))),
    };
    let limit = requested_max_bytes.min(MAX_BYTES);
    let mut value = json!({
        "authority": {"execution": false, "publication": false, "tool_invocation": false},
        "budget": {"max_bytes": limit, "used_bytes": 0},
        "declared_effects": import.effects,
        "index": {"digest": index.digest(), "status": "prepared_metadata"},
        "module": program.module,
        "package": {
            "cargo_alias": alias,
            "feature_digest": index.feature_digest(),
            "name": index.package().name,
            "source_sha256": index.package().source_sha256,
            "stable_rustc_version": index.stable_rustc_version(),
            "target": index.target(),
            "version": index.package().version,
        },
        "revision": crate::graph::revision(program),
        "schema": SCHEMA,
        "selected_import": {
            "docs": item.docs,
            "id": import.stable_id,
            "kind": kind_name(item.kind),
            "location": {"start": import.span.start, "end": import.span.end},
            "manual_adapter": manual_adapter(&item.support),
            "ownership": receiver_name(item.receiver),
            "path": item.path,
            "reachable_types": item.reachable_types,
            "reason": reason,
            "signature": item.signature,
            "source_name": import.name,
            "support": support,
        },
        "setup": {"instruction": "Validate the generated wrapper with the selected stable rustc and authenticated package bytes before any foreign execution.", "required": true},
        "truncation": {"omitted": [], "truncated": false},
    });
    let full = render(value.clone());
    if full.len() <= limit {
        return Ok(Some(full));
    }
    value["selected_import"]["docs"] = Value::Null;
    value["selected_import"]["reachable_types"] = json!([]);
    value["setup"]["instruction"] = Value::Null;
    value["truncation"] = json!({
        "omitted": ["selected_import.docs", "selected_import.reachable_types", "setup.instruction"],
        "truncated": true
    });
    let reduced = render(value);
    if reduced.len() <= limit {
        Ok(Some(reduced))
    } else {
        Err(vec![Diagnostic::io(
            "SPX-G004",
            format!("prepared Rust API context requires at least {} output bytes but max_bytes is {limit}", reduced.len()),
        )])
    }
}

fn manual_adapter(support: &Support) -> Value {
    match support {
        Support::Supported => Value::Null,
        Support::Rejected { .. } => json!({
            "action": "explicit_scalar_callback",
            "command": "semaprax-native-rust-sdk project --manifest-path <absolute-manifest> --output <fresh-absolute-path>",
            "source_form": "import rust fn ... effects { ... } failure status \"domain\";",
            "scope": "bounded_scalar_only; caller implements generated NativeRustSdkImports; indexed item remains rejected",
        }),
    }
}

fn kind_name(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Function => "function",
        ItemKind::InherentMethod => "inherent_method",
        ItemKind::TraitMethod => "trait_method",
        ItemKind::AssociatedType => "associated_type",
    }
}

fn receiver_name(receiver: Receiver) -> &'static str {
    match receiver {
        Receiver::None => "none",
        Receiver::Shared => "shared",
        Receiver::Mutable => "mutable",
        Receiver::Owned => "owned",
    }
}

fn reason_name(reason: RejectionReason) -> &'static str {
    match reason {
        RejectionReason::Private => "private",
        RejectionReason::SealedTrait => "sealed_trait",
        RejectionReason::OpaqueReturn => "opaque_return",
        RejectionReason::UnsupportedGeneric => "unsupported_generic",
        RejectionReason::UnsupportedSignature => "unsupported_signature",
        RejectionReason::IncompleteTypeClosure => "incomplete_type_closure",
        RejectionReason::ExpansionLimit => "expansion_limit",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;
    use std::path::Path;

    fn source(path: &str, name: &str) -> Program {
        parse(
            &format!(
                "module test.prepared;\n@id(\"rust.host\") interface RustHost permits {{ regex.read }} {{\n@id(\"rust.host.method\") import rust selected fn {name} from \"{path}\" effects {{ regex.read }} failure infallible;\n}}\n@id(\"rust.host.main\") fn main() -> i64 {{ 0 }}\n"
            ),
            Path::new("prepared-rust.spx"),
        )
        .unwrap()
    }

    fn regex_index() -> RustApiIndex {
        RustApiIndex::admit_extractor_output(include_bytes!(
            "../../crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
        ))
        .unwrap()
    }

    #[test]
    fn prepared_regex_context_is_pure_bounded_and_preserves_rejection() {
        let index = regex_index();
        let bytes = index.canonical_json().as_bytes();
        let program = source("regex::Regex::is_match", "is_match");
        let first = prepared_selected_rust_import_context_json(
            &program,
            "regex::Regex::is_match",
            bytes,
            4096,
        )
        .unwrap()
        .unwrap();
        let second =
            prepared_selected_rust_import_context_json(&program, "rust.host.method", bytes, 4096)
                .unwrap()
                .unwrap();
        assert_eq!(first, second);
        let value: Value = serde_json::from_str(&first).unwrap();
        assert_eq!(value["package"]["name"], "regex");
        assert_eq!(value["package"]["cargo_alias"], "regex_alias");
        assert_eq!(value["package"]["version"], "1.13.1");
        assert_eq!(
            value["selected_import"]["signature"],
            "fn is_match(&self, haystack: &str) -> bool"
        );
        assert_eq!(value["selected_import"]["ownership"], "shared");
        assert_eq!(value["selected_import"]["support"], "supported");
        assert!(value["selected_import"]["manual_adapter"].is_null());
        assert_eq!(value["authority"]["tool_invocation"], false);
        assert_eq!(value["budget"]["used_bytes"], first.len());
        assert!(first.len() <= 4096);

        let refused = source("regex::Regex::find", "find");
        let status =
            prepared_selected_rust_import_context_json(&refused, "rust.host.method", bytes, 4096)
                .unwrap()
                .unwrap();
        let value: Value = serde_json::from_str(&status).unwrap();
        assert_eq!(value["selected_import"]["support"], "rejected");
        assert_eq!(
            value["selected_import"]["reason"],
            "incomplete_type_closure"
        );
        assert_eq!(
            value["selected_import"]["manual_adapter"]["action"],
            "explicit_scalar_callback"
        );
        assert!(value["selected_import"]["manual_adapter"]["scope"]
            .as_str()
            .unwrap()
            .contains("indexed item remains rejected"));

        let mut damaged = bytes.to_vec();
        damaged[0] ^= 1;
        let error = prepared_selected_rust_import_context_json(
            &program,
            "rust.host.method",
            &damaged,
            4096,
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-B148");
        assert!(error[0].span.is_some());
    }

    #[test]
    fn every_selected_import_and_candidate_retains_its_index_status() {
        let index = regex_index();
        let bytes = index.canonical_json().as_bytes();
        let replay_digest = RustApiIndex::replay(bytes).unwrap().digest().to_owned();
        let program = parse(
            "module test.prepared_all;\n@id(\"rust.host\") interface RustHost permits { regex.read } {\n@id(\"rust.host.match\") import rust selected fn is_match from \"regex::Regex::is_match\" effects { regex.read } failure infallible;\n@id(\"rust.host.find\") import rust selected fn find from \"regex::Regex::find\" effects { regex.read } failure infallible;\n}\n@id(\"rust.host.main\") fn main() -> i64 { 0 }\n",
            Path::new("prepared-all.spx"),
        )
        .unwrap();
        let expected = [
            ("rust.host.match", "regex::Regex::is_match", "supported"),
            ("rust.host.find", "regex::Regex::find", "rejected"),
        ];
        for (id, path, support) in expected {
            let output = prepared_selected_rust_import_context_json(&program, id, bytes, 4096)
                .unwrap()
                .unwrap();
            assert!(output.len() <= 4096);
            let value: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(value["selected_import"]["id"], id);
            assert_eq!(value["selected_import"]["path"], path);
            assert_eq!(value["selected_import"]["support"], support);
            assert_eq!(value["index"]["digest"], replay_digest);
            assert_eq!(value["authority"]["tool_invocation"], false);
        }
        let candidates = prepared_rust_api_candidates_json(bytes, "regex::Regex::", 4096).unwrap();
        assert!(candidates.len() <= 4096);
        let value: Value = serde_json::from_str(&candidates).unwrap();
        for (_, path, support) in expected {
            let item = value["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["path"] == path)
                .expect("every declared selected import has an indexed candidate");
            assert_eq!(item["support"], support);
            assert_eq!(item["manual_adapter"].is_null(), support == "supported");
        }
    }

    #[test]
    fn oversized_index_docs_truncate_without_changing_supported_or_rejected_status() {
        let mut envelope: Value = serde_json::from_slice(include_bytes!(
            "../../crates/semaprax-rust-api-index/fixtures/regex-1.13.1-index-envelope.json"
        ))
        .unwrap();
        for item in envelope["index"]["items"].as_array_mut().unwrap() {
            if item["path"] == "regex::Regex::is_match" || item["path"] == "regex::Regex::find" {
                item["docs"] = json!("x".repeat(8_000));
            }
        }
        let mut extractor_bytes = serde_json::to_vec(&envelope).unwrap();
        extractor_bytes.push(b'\n');
        let index = RustApiIndex::admit_extractor_output(&extractor_bytes).unwrap();
        let bytes = index.canonical_json().as_bytes();
        for (path, source_name, expected_support) in [
            ("regex::Regex::is_match", "is_match", "supported"),
            ("regex::Regex::find", "find", "rejected"),
        ] {
            let program = source(path, source_name);
            let first = prepared_selected_rust_import_context_json(&program, path, bytes, 4096)
                .unwrap()
                .unwrap();
            let second = prepared_selected_rust_import_context_json(&program, path, bytes, 4096)
                .unwrap()
                .unwrap();
            assert_eq!(first, second);
            assert!(first.len() <= 4096);
            let value: Value = serde_json::from_str(&first).unwrap();
            assert_eq!(value["budget"]["used_bytes"], first.len());
            assert_eq!(value["truncation"]["truncated"], true);
            assert_eq!(value["selected_import"]["docs"], Value::Null);
            assert_eq!(value["selected_import"]["support"], expected_support);
            if expected_support == "rejected" {
                assert_eq!(
                    value["selected_import"]["reason"],
                    "incomplete_type_closure"
                );
            }
        }
    }
}
