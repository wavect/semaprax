use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    bounded_graph_with_limit, bounded_native_with_limit, bounded_wasm_with_limit,
    capability_manifest, capability_manifest_json, MAX_GRAPH_BYTES, MAX_NATIVE_C11_BYTES,
    MAX_WASM_CORE_BYTES,
};
use crate::{codegen, graph, hir, parse, wasm};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn fixture(label: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let directory = std::env::temp_dir().join(format!(
        "semaprax-target-unit-{}-{label}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source = directory.join("module.spx");
    let patch = directory.join("change.spatch");
    let source_text = "module target.final_check;\n@id(\"target.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    std::fs::write(&source, source_text).unwrap();
    std::fs::write(
        &patch,
        format!(
            "base {}\nrename target.helper to renamed\n",
            graph::revision(&parse(source_text, &source).unwrap())
        ),
    )
    .unwrap();
    (directory, source, patch)
}

#[test]
fn capability_manifest_is_typed_and_canonical() {
    let source = r#"module target.capabilities;
permit { platform.token.release }
@id("platform.token") resource Token {
    @id("platform.token.drop") drop import "platform.token.finalize";
}
@id("platform.token.host") interface TokenHost
    permits { platform.token.release }
{
    @id("platform.token.finalize")
    import fn finalize(token: own Token) -> unit
        effects { platform.token.release }
        failure infallible
        consumes token always;
}
@id("generic.touch") fn touch<T>(value:T)->T { value }
@id("app.main") fn main() -> i64 uses { platform.token.release } { let value=touch<i64>(1);value }
"#;
    let program = parse(source, "capabilities.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let json = capability_manifest_json(&capability_manifest(&resolved).unwrap());
    assert_eq!(
        capability_manifest(&resolved)
            .unwrap()
            .iter()
            .map(|fact| fact.kind)
            .collect::<Vec<_>>(),
        [
            "function_effect",
            "import_effect",
            "import_required_authority",
            "interface_permit",
            "module_permit",
        ]
    );
    for kind in [
        "module_permit",
        "function_effect",
        "interface_permit",
        "import_effect",
        "import_required_authority",
    ] {
        assert!(json.contains(kind));
    }
    let mut synthetic_template_effect = resolved.clone();
    synthetic_template_effect.function_templates[0].effects =
        vec!["platform.token.release".to_owned()];
    synthetic_template_effect.function_instances[0]
        .function
        .effects = vec!["platform.token.release".to_owned()];
    let synthetic = capability_manifest(&synthetic_template_effect).unwrap();
    assert!(synthetic
        .iter()
        .any(|fact| fact.kind == "function_template_effect"));
    let mut mismatch = synthetic_template_effect.clone();
    mismatch.function_instances[0].function.effects.clear();
    assert_eq!(
        capability_manifest(&mismatch).unwrap_err()[0].code,
        "SPX-G141"
    );

    let changed = parse(
        "module target.capabilities;\n@id(\"app.main\") fn main() -> i64 { 1 }\n",
        "capabilities-changed.spx",
    )
    .unwrap();
    let changed = hir::resolve(&changed).unwrap();
    assert_ne!(
        capability_manifest(&resolved).unwrap(),
        capability_manifest(&changed).unwrap()
    );
}

#[test]
fn production_emitters_fail_closed_at_tiny_caps_and_preserve_bytes() {
    let source = "module target.sinks;\n@id(\"app.main\") fn main()->i64 { 1 + 2 }\n";
    let program = parse(source, "sinks.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let revision = graph::revision(&program);

    let graph = graph::to_hir_json(&resolved, &revision).unwrap();
    assert_eq!(
        bounded_graph_with_limit(&resolved, &revision, MAX_GRAPH_BYTES).unwrap(),
        graph
    );
    assert_eq!(
        bounded_graph_with_limit(&resolved, &revision, 1).unwrap_err()[0].code,
        "SPX-G140"
    );

    let native = codegen::emit_resolved_c_with_source(&program, &resolved).unwrap();
    assert_eq!(
        bounded_native_with_limit(&program, &resolved, MAX_NATIVE_C11_BYTES).unwrap(),
        native
    );
    assert_eq!(
        bounded_native_with_limit(&program, &resolved, 1).unwrap_err()[0].code,
        "SPX-G140"
    );

    let wasm = wasm::emit_resolved_module(&resolved).unwrap();
    assert_eq!(
        bounded_wasm_with_limit(&resolved, MAX_WASM_CORE_BYTES).unwrap(),
        wasm
    );
    assert_eq!(
        bounded_wasm_with_limit(&resolved, 1).unwrap_err()[0].code,
        "SPX-G140"
    );
}

#[test]
fn target_route_translates_review_diagnostic_namespaces() {
    let translated = super::map_review_diagnostics(vec![
        crate::diagnostic::Diagnostic::io("SPX-G120", "review bound"),
        crate::diagnostic::Diagnostic::io("SPX-G121", "review invariant"),
    ]);
    assert_eq!(translated[0].code, "SPX-G140");
    assert_eq!(translated[1].code, "SPX-G141");
}

#[test]
fn target_preview_rejects_changed_bytes_and_same_byte_identity_at_final_check() {
    let (directory, source, patch) = fixture("changed");
    let error = super::preview_with_hook(&source, &patch, |path| {
        std::fs::write(path, std::fs::read_to_string(path)?.replace("{1}", "{2}"))
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    std::fs::remove_dir_all(directory).unwrap();

    let (directory, source, patch) = fixture("identity");
    let backup = source.with_extension("original.spx");
    let original = std::fs::read(&source).unwrap();
    let error = super::preview_with_hook(&source, &patch, |path| {
        std::fs::rename(path, &backup)?;
        std::fs::write(path, &original)
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(std::fs::read(&source).unwrap(), original);
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    std::fs::remove_dir_all(directory).unwrap();

    let (directory, source, patch) = fixture("growth");
    let oversized = vec![b'x'; crate::review::MAX_SOURCE_BYTES + 1];
    let error = super::preview_with_hook(&source, &patch, |path| std::fs::write(path, &oversized))
        .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(
        std::fs::metadata(&source).unwrap().len(),
        (crate::review::MAX_SOURCE_BYTES + 1) as u64
    );
    std::fs::remove_dir_all(directory).unwrap();
}
