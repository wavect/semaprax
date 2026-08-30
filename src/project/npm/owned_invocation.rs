//! Selected v8-v10 runtime templates. Historical false branches remain in the
//! owning renderers so their byte contracts are not reinterpreted.

use crate::diagnostic::quote_json;

pub(super) fn prelude(digest: &str, capacity: u32) -> String {
    let arena = include_str!("owned_invocation/arena.js")
        .replace("__SPX_CAPACITY__", &capacity.to_string());
    format!(
        "const EXPECTED_WASM_SHA256 = {};\n{}{}{}{}{}",
        quote_json(digest),
        include_str!("owned_data_input_v8.js"),
        arena,
        include_str!("owned_invocation/core.js"),
        include_str!("owned_invocation/call.js"),
        include_str!("owned_invocation/result.js"),
    )
}

pub(super) fn facade(facts: &str, memory_bytes: u32) -> String {
    include_str!("owned_invocation/facade.js")
        .replace("__SPX_MEMORY_BYTES__", &memory_bytes.to_string())
        // Facts contain identities; never rescan their bytes as template tokens.
        .replace("__SPX_FACTS__", facts)
}

#[cfg(test)]
mod tests {
    #[test]
    fn inserted_identity_bytes_are_not_reinterpreted_as_template_markers() {
        let facts = r#"["__SPX_MEMORY_BYTES__",Object.freeze({raw:"__SPX_FACTS__",params:Object.freeze([])})]"#;
        let rendered = super::facade(facts, 131_072);
        assert!(rendered.starts_with(&format!("const FACTS=new Map([{facts}]),")));
        assert!(rendered.contains("createOwnedInvocation(linked,FACTS,131072)"));
    }
}
