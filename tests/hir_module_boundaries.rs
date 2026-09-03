#[test]
fn core_validation_has_no_resolution_cleanup_build_or_physical_authority() {
    let validation = concat!(
        include_str!("../src/hir/validation.rs"),
        include_str!("../src/hir/validation/type_profiles.rs"),
    );
    for forbidden in [
        "struct Resolver",
        "impl Resolver",
        "fn scoped_identity(",
        "fn materialize_function_template",
        "fn specialize_source_function",
        "pub fn resolve(",
        "crate::cleanup::build_inventory",
        "crate::cleanup_plan::build_plan",
        "std::fs",
        "std::process",
        "platform::",
    ] {
        assert!(
            !validation.contains(forbidden),
            "core HIR validation admitted `{forbidden}`"
        );
    }

    let root = include_str!("../src/hir.rs");
    assert!(root.contains("mod validation;"));
    assert!(root.contains("pub(crate) use validation::validate_core;"));

    let format_macro = root.find("macro_rules! format {").unwrap();
    let validation_module = root.find("mod validation;").unwrap();
    assert!(format_macro < validation_module);
    assert!(root[format_macro..validation_module]
        .contains("crate::bounded_output::budgeted_format(format_args!"));
    assert!(validation.contains("format!("));
    for forbidden in ["std::format", "::format!", "use std::format"] {
        assert!(
            !validation.contains(forbidden),
            "core HIR validation bypassed the budgeted format macro with `{forbidden}`"
        );
    }
}
