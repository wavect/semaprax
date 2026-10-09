use super::*;

#[test]
fn noncanonical_workspace_source_reports_project_format_repair() {
    let app = canonical_source(
        "app.spx",
        "module app;\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n",
    );
    let library = canonical_source(
        "library.spx",
        "module library;\n@id(\"library.value\")\nfn value() -> i64 { 1 }\n",
    );
    assert!(build_owned(vec![app.clone(), library.clone()]).is_ok());

    let error = build_owned(vec![
        source("app.spx", &format!("\n{}", app.source)),
        library,
    ])
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-G170");
    assert!(error[0]
        .message
        .contains("workspace semantic source `app.spx` is not canonical"));
    assert_eq!(
        error[0].help.as_deref(),
        Some(
            "Run `semaprax fmt <project-directory-or-manifest>` to canonicalize project source, then retry."
        )
    );
}
