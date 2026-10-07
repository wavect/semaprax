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
    let error = build_owned(vec![app, test_module()])
        .expect("workspace graph must build")
        .linked_scalar_program_with_roots(
            "app.main",
            &[],
            crate::project::ProjectProfile::UsefulDataV1,
            false,
        )
        .expect_err("Useful Data does not retain authored record declarations");

    assert_eq!(error[0].code, "SPX-H006");
    assert_eq!(
        error[0].message,
        "workspace function `app.main` uses authored type `app.item`, which is outside the Useful Data linker profile"
    );
    assert_eq!(error[0].span, Some(expected_span));
}
