//! Mixed signatures at the public bound, not an execution-order proof.
use super::*;
use semaprax::project::with_authenticated_project;

#[path = "../../support/owned_mixed_arity_product.rs"]
mod product;

#[test]
fn mixed_zero_through_eight_parameters_preserve_exact_order_and_replay() {
    let source = product::source(8);
    let program = resolve(&source);
    let selected = product::selected(8);
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    assert_eq!(descriptor.exports().len(), 9);
    let types = [
        PublicApiParameterType::I64,
        PublicApiParameterType::Bool,
        PublicApiParameterType::BorrowStr,
        PublicApiParameterType::BorrowSliceU8,
        PublicApiParameterType::I64,
        PublicApiParameterType::Bool,
        PublicApiParameterType::BorrowStr,
        PublicApiParameterType::BorrowSliceU8,
    ];
    let canonical = descriptor.canonical_bytes();
    let wire: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
    for (arity, export) in descriptor.exports().iter().enumerate() {
        assert_eq!(export.stable_id().as_str(), format!("mixed.arity{arity}"));
        assert_eq!(export.typescript_name(), format!("mixed.arity{arity}"));
        assert_eq!(
            export.rust_method_name(),
            format!("spx_mixed_dot_arity{arity}")
        );
        assert_eq!(export.result(), PublicApiResultType::OwnedBytes);
        assert_eq!(export.parameters().len(), arity);
        let mut identities = std::collections::BTreeSet::new();
        for (ordinal, parameter) in export.parameters().iter().enumerate() {
            assert_eq!(parameter.ty(), types[ordinal]);
            assert_eq!(parameter.source_name(), format!("p{ordinal}"));
            assert!(!parameter.stable_id().as_str().is_empty());
            assert!(identities.insert(parameter.stable_id()));
            assert_eq!(
                wire["exports"][arity]["parameters"][ordinal]["ordinal"],
                ordinal
            );
        }
    }
    assert_eq!(
        replay_public_api_descriptor(
            &program,
            &selected,
            subject(),
            &canonical,
            &descriptor.digest()
        )
        .unwrap(),
        descriptor
    );
    assert_eq!(product::source(8), source);
    // Explicit minus-one and exact selections from the same checked source.
    for arity in [7, 8] {
        let one =
            derive_public_api_descriptor(&program, &[format!("mixed.arity{arity}")], subject())
                .unwrap();
        assert_eq!(one.exports()[0].parameters().len(), arity);
    }
}

#[test]
fn selected_ninth_parameter_has_the_exact_arity_diagnostic_before_project_callback() {
    let program = resolve(&product::source(9));
    let error = derive_public_api_descriptor(&program, &["mixed.arity9".to_owned()], subject())
        .unwrap_err();
    assert_eq!(error.code, "SPX-J113");
    assert_eq!(
        error.message,
        "selected public API export `mixed.arity9` exceeds the 8-parameter limit"
    );

    let root = std::env::temp_dir().join(format!(
        "semaprax-mixed-arity-rejection-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    let manifest = product::write_project(&root, 9);
    let originals = ["semaprax.toml", "src/app.spx", "src/tests.spx"]
        .map(|name| (name, std::fs::read(root.join(name)).unwrap()));
    let mut called = false;
    let errors = with_authenticated_project(&manifest, |_snapshot| {
        called = true;
        Ok(())
    })
    .unwrap_err();
    assert!(
        !called,
        "inadmissible Project exposed its authenticated callback"
    );
    assert!(
        errors
            .iter()
            .any(|actual| actual.code == error.code && actual.message == error.message),
        "{errors:?}"
    );
    for (name, bytes) in originals {
        assert_eq!(std::fs::read(root.join(name)).unwrap(), bytes);
    }
    let mut names = std::fs::read_dir(&root)
        .unwrap()
        .map(|row| row.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["semaprax.toml", "src"]);
    eprintln!("retained mixed-arity rejection fixture: {}", root.display());
    // Callback exclusion and unchanged inputs are not a physical tool counter.
}
