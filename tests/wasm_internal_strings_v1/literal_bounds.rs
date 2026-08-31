//! Compile-only literal-pool boundaries. No engine, native compiler or files.
use semaprax::wasm::internal_strings::{emit_module, InternalStringModule, InternalStringOptions};

const LIMIT: usize = 65_536;

fn source(left: &str, right: Option<&str>) -> String {
    // The test payload alphabet needs no source escaping. Keep byte lengths
    // independent of character counts for the multibyte boundary cases.
    assert!(!left.contains(['"', '\\']));
    assert!(right.is_none_or(|text| !text.contains(['"', '\\'])));
    let mut source = format!(
        "module strings.literal_bounds;\n\
         @id(\"literal.left\") fn left() -> string {{ \"{left}\" }}\n"
    );
    let body = if let Some(right) = right {
        source.push_str(&format!(
            "@id(\"literal.right\") fn right() -> string {{ \"{right}\" }}\n"
        ));
        "string_len(left()) + string_len(right())"
    } else {
        "string_len(left())"
    };
    source.push_str(&format!(
        "@id(\"literal.main\") fn main() -> i64 {{ {body} }}\n"
    ));
    source
}

fn compile(source: &str) -> Result<InternalStringModule, Box<semaprax::diagnostic::Diagnostic>> {
    let program = semaprax::check(source, "literal-bounds.spx").unwrap();
    emit_module(
        &program,
        &["literal.main".to_owned()],
        InternalStringOptions::default(),
    )
    .map_err(Box::new)
}

fn profile_limit(source: &str) {
    let error = compile(source).unwrap_err();
    assert_eq!(error.code, "SPX-W111", "{error:?}");
    assert_eq!(
        error.message,
        "standalone String literal pool exceeds 65536 bytes"
    );
}

#[test]
fn one_literal_exact_and_plus_one_use_utf8_bytes() {
    for exact in ["x".repeat(LIMIT), "é".repeat(LIMIT / 2)] {
        assert_eq!(exact.len(), LIMIT);
        assert!(compile(&source(&exact, None)).is_ok());
        let excess = format!("{exact}x");
        assert_eq!(excess.len(), LIMIT + 1);
        profile_limit(&source(&excess, None));
    }
}

#[test]
fn distinct_literals_share_one_cumulative_selected_pool() {
    let left = "é".repeat(LIMIT / 4);
    let right = "λ".repeat(LIMIT / 4);
    assert_ne!(left, right);
    assert_eq!(left.len() + right.len(), LIMIT);
    assert!(compile(&source(&left, Some(&right))).is_ok());
    let excess = format!("{right}x");
    assert_eq!(left.len() + excess.len(), LIMIT + 1);
    profile_limit(&source(&left, Some(&excess)));
}

#[test]
fn identical_literal_bytes_are_charged_once_across_selected_functions() {
    let exact = "é".repeat(LIMIT / 2);
    assert_eq!(exact.len(), LIMIT);
    assert!(compile(&source(&exact, Some(&exact))).is_ok());
}

#[test]
fn unrelated_oversize_literal_does_not_change_selected_artifacts() {
    let baseline_source = source("selected", None);
    let baseline = compile(&baseline_source).unwrap();
    let augmented = format!(
        "{baseline_source}\n@id(\"unselected.literal\") fn unrelated() -> string {{ \"{}\" }}\n",
        "λ".repeat(LIMIT / 2 + 1)
    );
    let actual = compile(&augmented).unwrap();
    assert_eq!(actual.wasm_bytes(), baseline.wasm_bytes());
    assert_eq!(actual.descriptor(), baseline.descriptor());
    assert_eq!(actual.runtime_source(), baseline.runtime_source());
}

#[test]
fn legacy_v10_literal_pool_preserves_its_existing_diagnostic() {
    use semaprax::project::{derive_public_api_descriptor, PublicApiSubject};
    const FACT: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    for length in [LIMIT, LIMIT + 1] {
        let source = format!(
            "module strings.legacy_literals;\n\
             @id(\"literal.value\") fn value() -> string {{ \"{}\" }}\n\
             @id(\"literal.main\") fn main() -> i64 {{ 0 }}\n",
            "x".repeat(length)
        );
        let parsed = semaprax::check(&source, "legacy-literal-bounds.spx").unwrap();
        let program = semaprax::hir::resolve(&parsed).unwrap();
        let subject = PublicApiSubject {
            project_schema: semaprax::project::PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
            project_revision: FACT,
            workspace_revision: FACT,
            project_graph_digest: FACT,
        };
        let descriptor =
            derive_public_api_descriptor(&program, &["literal.value".to_owned()], subject).unwrap();
        let emitted =
            semaprax::wasm::emit_resolved_module_with_owned_data_exports(&program, &descriptor);
        if length == LIMIT {
            assert!(emitted.is_ok());
        } else {
            let error = emitted.unwrap_err();
            assert_eq!(error.code, "SPX-W110");
            assert_eq!(
                error.message,
                "owned UTF-8 literal table exceeds 65536 bytes"
            );
        }
    }
}
