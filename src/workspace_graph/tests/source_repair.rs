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
    .err()
    .expect("noncanonical Project source must be refused");
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

#[test]
fn unknown_import_help_names_the_exact_provider_without_repairing_identity() {
    let library = canonical_source(
        "lib/core.spx",
        "module lib.core;\n@id(\"lib.answer\") fn answer() -> i64 { 42 }\n",
    );
    let importer = |identity: &str| {
        canonical_source(
            "app/main.spx",
            &format!(
                "module app.main;\nuse function @id({identity:?}) from lib.core as answer;\n@id(\"app.main\") fn main() -> i64 {{ answer() }}\n"
            ),
        )
    };
    let admitted = build_owned(vec![importer("lib.answer"), library.clone()]).unwrap();
    assert!(admitted.edges.iter().any(|edge| edge.kind == "call"
        && edge.caller == "app.main"
        && edge.target == "lib.answer"));

    for identity in [
        "lib.missing".to_owned(),
        "lib.\nmissing".to_owned(),
        format!("lib.{}", "界".repeat(200)),
    ] {
        let app = importer(&identity);
        let parsed = crate::parse(&app.source, Path::new("app/main.spx")).unwrap();
        assert_eq!(format::canonical(&parsed), app.source);
        let error = build_owned(vec![app, library.clone()])
            .err()
            .expect("an unknown identity cannot resolve by provider display name");
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G172");
        assert_eq!(error[0].message, "persistent target identity is unknown");
        assert_eq!(error[0].path.as_deref(), Some("app/main.spx"));
        assert_eq!(error[0].span, Some(parsed.module_uses[0].span));
        let help = error[0].help.as_deref().unwrap();
        assert!(help.contains("provider module \"lib.core\""));
        assert!(help.contains("semaprax query 'lib/core.spx'"));
        assert!(!help.contains("lib.answer")); // No guessed replacement.
        assert!(!help.chars().any(char::is_control));
        assert!(help.len() <= 4096);
        if identity == "lib.missing" {
            assert_eq!(
                help,
                concat!(
                "imported identity \"lib.missing\" was not found in provider module \"lib.core\"; ",
                "run `semaprax query 'lib/core.spx'` to list its declarations, ",
                "then use the exact explicit @id and declaration kind"
            )
            );
        } else if identity.contains('\n') {
            assert!(help.contains("lib.\\nmissing"));
        } else {
            assert!(help.contains('…'));
            assert!(!help.contains(&identity));
        }
    }
}
