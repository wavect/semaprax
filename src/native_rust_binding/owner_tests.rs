use super::bind_selected_regex_result_signature;
use crate::ast::{ImportResult, ParamMode, Type};
use std::path::Path;

const SOURCE: &str = r#"module regex.binding;
@id("regex.resource") resource Regex { @id("regex.drop") drop import "regex.drop"; }
@id("regex.host") interface Host permits { } {
 @id("regex.drop") import fn drop_regex(regex: own Regex) -> unit effects { } failure infallible consumes regex always;
 @id("regex.new") import rust selected fn regex_new from "regex::Regex::new" effects { } failure infallible;
 @id("regex.match") import rust selected fn regex_match from "regex::Regex::is_match" effects { } failure infallible;
}
@id("regex.main") fn main() -> i64 { 0 }
"#;
const DIGEST: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn bind(id: &str, signature: &str, receiver: &str) -> crate::ast::ImportDeclaration {
    let mut program = crate::parse(SOURCE, Path::new("regex-binding.spx")).unwrap();
    let types = program.types.clone();
    let import = program
        .interfaces
        .iter_mut()
        .flat_map(|interface| &mut interface.imports)
        .find(|import| import.stable_id == id)
        .unwrap();
    assert!(
        bind_selected_regex_result_signature(import, &types, signature, DIGEST, receiver).unwrap()
    );
    import.clone()
}

#[test]
fn selected_regex_result_binding_is_narrow_and_borrowed() {
    let constructor = bind(
        "regex.new",
        "fn new(re: &str) -> core::result::Result<regex::Regex, regex::Error>",
        "none",
    );
    assert_eq!(constructor.params.len(), 1);
    assert_eq!(constructor.params[0].mode, ParamMode::Borrow);
    assert_eq!(constructor.params[0].ty, Type::String);
    assert_eq!(
        constructor.result,
        ImportResult::OwnedResultResourceI64 {
            name: "Regex".into()
        }
    );
    assert_eq!(constructor.selected_receiver, None);

    let matcher = bind(
        "regex.match",
        "fn is_match(&self, text: &str) -> bool",
        "shared",
    );
    assert!(matcher
        .params
        .iter()
        .all(|parameter| parameter.mode == ParamMode::Borrow));
    assert_eq!(
        matcher.params[0].ty,
        Type::Named {
            name: "Regex".into(),
            arguments: Vec::new()
        }
    );
    assert_eq!(matcher.params[1].ty, Type::String);
    assert_eq!(matcher.result, ImportResult::Bool);
    assert_eq!(matcher.selected_receiver.as_deref(), Some("shared"));
}

#[test]
fn selected_regex_result_binding_refuses_other_error_arms() {
    let mut program = crate::parse(SOURCE, Path::new("regex-binding.spx")).unwrap();
    let types = program.types.clone();
    let import = program.interfaces[0]
        .imports
        .iter_mut()
        .find(|import| import.stable_id == "regex.new")
        .unwrap();
    let error = bind_selected_regex_result_signature(
        import,
        &types,
        "fn new(re: &str) -> core::result::Result<regex::Regex, regex::Other>",
        DIGEST,
        "none",
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-B145");
    assert!(error.message.contains("outside the RI-06 profile"));
}
