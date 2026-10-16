use super::*;
use crate::bounded_output::identity_reservation::IdentityReservation;
use crate::source_verify::funding::{SourceFunding, SourceStop};
#[test]
fn specialization_abort_moves_the_exact_six_field_diagnostic_prefix() {
    let program = crate::parse(
        r#"module source.prefix;
@id("source.prefix.identity") fn identity<T>(value:T)->T {value}
@id("source.prefix.main") fn main()->i64 {0}"#,
        "prefix.spx",
    )
    .unwrap();
    let cells = crate::source_verify::function_cells(&program);
    let functions = cells
        .iter()
        .map(|(_, function)| (function.name(), function))
        .collect();
    let types = TypeTable::new(&program);
    let imports = HashMap::new();
    let prefix = vec![
        Diagnostic::error("SPX-T900", "already selected", program.functions[0].span)
            .at_path("prefix.spx")
            .with_help("preserve exact prefix help"),
    ];
    let expected = prefix.clone();
    let bytes = IdentityReservation::setup_bytes().unwrap() + SourceFunding::setup_bytes();
    let (abort, refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut owner = IdentityReservation::acquire().unwrap();
        let mut funding = SourceFunding::funded(&mut owner).unwrap();
        let mut diagnostics = prefix;
        let stop = check_function_bodies(
            &program,
            &cells,
            &functions,
            &imports,
            &types,
            &mut diagnostics,
            None,
            &mut funding,
        )
        .unwrap_err();
        assert!(matches!(stop, SourceStop::Reservation(_)));
        funding.abort(diagnostics, stop)
    });
    assert!(refused);
    assert_eq!(abort.prefix(), expected);
    // The refusal precedes the specialized view and variable-scope formation;
    // no moved bindings, later body, or diagnostic dedup pass runs afterward.
}
