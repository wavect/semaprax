use super::*;

#[test]
fn useful_data_profile_rejection_uses_the_function_source_span() {
    let app = source(
        "src/app.spx",
        r#"
module app.main;

@id("app.item")
record Item { @id("app.item.value") value: i64, }

@id("app.main")
fn main() -> i64 { Item { value: 0 }.value }
"#,
    );
    let parsed =
        crate::parse(&app.source, Path::new(&app.path)).expect("canonical workspace source parses");
    let expected_span = parsed
        .functions
        .iter()
        .find(|function| function.stable_id == "app.main")
        .expect("entry function")
        .body
        .span;
    let resolved = crate::hir::resolve(&parsed).expect("source program resolves");
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .expect("resolved entry function")
        .clone();
    let error = super::super::link_useful_data_workspace(
        resolved.module,
        resolved.entrypoint,
        vec![crate::hir::LinkedScalarFunction {
            function,
            origin: crate::hir::IdentityOrigin::Explicit,
        }],
    )
    .expect_err("Useful Data does not retain authored record declarations");

    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "workspace function `app.main` uses authored type `app.item`, which is outside the Useful Data linker profile"
    );
    assert_eq!(error.span, Some(expected_span));
}
