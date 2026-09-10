//! Named cross-backend selector for the bundled `std.data.toml` package, whose
//! quoted-key and value cursors are allocation-free offset computations over
//! borrowed views.

#[test]
fn toml_cursors_execute_on_all_three_backends() {
    super::run_examples_and_conformance(
        super::packages()
            .into_iter()
            .filter(|p| p.module == "std.data.toml")
            .collect(),
    );
}
