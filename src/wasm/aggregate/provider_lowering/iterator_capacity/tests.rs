use super::*;

fn demand(body: &str) -> Result<u32, Diagnostic> {
    let text = format!(
        r#"module provider.capacity;
@id("provider.make") fn make() -> Vec<Bytes> {{ vec_with_capacity<Bytes>(0usize) }}
@id("provider.main") fn main() -> i64 {{ {body} }}"#
    );
    let ast = crate::check(&text, "provider-capacity.spx").unwrap();
    let hir = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&hir).unwrap();
    prove(
        &hir,
        &FunctionExecutionId::Monomorphic(hir.entrypoint.clone()),
    )
}

#[test]
fn table_proof_counts_repeated_calls_and_refuses_excess_rows() {
    let body = |rows| {
        let mut text = (0..rows)
            .map(|i| format!("let row{i}=make();"))
            .collect::<String>();
        text.push('0');
        text
    };
    assert_eq!(demand(&body(32)).unwrap(), 32);
    assert!(demand(&body(33)).is_err());
}

#[test]
fn table_proof_requires_literal_capacity_and_preserves_nonallocating_loops() {
    assert_eq!(
        demand("let row=vec_with_capacity<Bytes>(32usize);0").unwrap(),
        1
    );
    assert!(demand("let row=vec_with_capacity<Bytes>(33usize);0").is_err());
    assert!(demand("let capacity=1usize;let row=vec_with_capacity<Bytes>(capacity);0").is_err());
    // A legal nonallocating loop leaves the constructor demand unchanged.
    assert_eq!(demand("let row=make(); while false {0} 0").unwrap(), 1);
}
