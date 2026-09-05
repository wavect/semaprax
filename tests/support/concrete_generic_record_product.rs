//! Shared GEN-05 source facts. Public Project admission remains deliberately absent.

use std::path::Path;

pub const TEMPLATE_MODULE: &str = r#"module generic.product.types;

@id("generic.product.pair")
record Pair<T, U> {
    @id("generic.product.pair.left")
    left: T,
    @id("generic.product.pair.right")
    right: U,
}
"#;

/// Standalone equivalent used to prove that legacy public descriptors reject
/// an otherwise compiler-admitted concrete generic result.
pub fn standalone_source() -> String {
    format!(
        r#"module generic.product.standalone;

{}
@id("generic.product.make")
fn make(input: borrow Slice<u8>) -> Pair<Bytes, bool> {{
    Pair<Bytes, bool> {{
        left: bytes_copy(input),
        right: byte_len(input) > 0usize,
    }}
}}

@id("generic.product.app.main")
fn main() -> i64 {{ 0 }}
"#,
        TEMPLATE_MODULE
            .strip_prefix("module generic.product.types;\n")
            .expect("shared template module prefix")
    )
}

pub fn resolved_standalone() -> semaprax::hir::ResolvedProgram {
    let source = standalone_source();
    let parsed = semaprax::check(&source, Path::new("generic-owned-product.spx"))
        .expect("shared concrete generic product checks");
    semaprax::hir::resolve(&parsed).expect("shared concrete generic product resolves")
}
