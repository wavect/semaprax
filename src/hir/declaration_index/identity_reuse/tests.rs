//! Complete saved index oracle; allocation scope excludes source parsing.
use super::*;
use crate::test_allocation_observer::measure;
mod old;

const SOURCE: &str = r#"module identity.reuse;
@id("token.type") resource Token { @id("token.drop") drop trivial; }
@id("row.type") record Row { @id("row.x") x:i64, }
@id("base.type") class Base {
    @id("base.x") x:i64,
    @id("base.read") fn read(self:Base)->i64 {self.x}
}
@id("child.type") class Child : Base {
    @id("child.y") y:i64,
    @id("child.read") fn read(self:Child)->i64 {self.y}
}
@id("choice.type") variant Choice {
    @id("choice.left") Left { @id("choice.left.x") x:i64, },
    @id("choice.right") Right,
}
@id("app.main") fn main()->i64 {0}
"#;
fn compare(program: &Program) {
    let actual = DeclarationIndex::from_verified_with_prelude(program, &[]);
    let old = DeclarationIndex::saved_from_verified_with_prelude(program, &[]);
    match (actual, old) {
        (Ok(actual), Ok(old)) => assert_eq!(actual, old),
        (Err(actual), Err(old)) => {
            assert_eq!(actual.code, old.code);
            assert_eq!(actual.message, old.message);
            assert_eq!(actual.path, old.path);
            assert_eq!(actual.span, old.span);
        }
        _ => panic!("same current source selected different index result"),
    }
}
#[test]
fn inserted_identity_reuse_preserves_full_index_inheritance_and_current_text_guards() {
    let program = crate::check(SOURCE, "identity-reuse.spx").unwrap();
    compare(&program);
    let index = DeclarationIndex::from_verified_with_prelude(&program, &[]).unwrap();
    for (owner, _) in &index.type_parameters {
        if let Some(declaration) = index.declarations.get(owner.as_str()) {
            assert_eq!(owner.as_str().as_ptr(), declaration.id.as_str().as_ptr());
        }
    }
    for owner in index.record_fields.keys().chain(index.variant_cases.keys()) {
        assert_eq!(
            owner.as_str().as_ptr(),
            index.declarations[owner.as_str()].id.as_str().as_ptr()
        );
    }
    assert_eq!(index.class_parents["child.type"].as_str(), "base.type");
    assert_eq!(index.record_fields["child.type"].len(), 2);
    assert_eq!(
        index.declarations["token.drop"]
            .owner
            .as_ref()
            .unwrap()
            .as_str(),
        "token.type"
    );
    // Detached source copies must build their own current carriers, not reuse
    // a previously held index. Text equality still gives the saved result.
    let detached = DeclarationIndex::from_verified_with_prelude(&program.clone(), &[]).unwrap();
    assert_eq!(detached, index);
    assert_ne!(
        detached.declarations["row.type"].id.as_str().as_ptr(),
        index.declarations["row.type"].id.as_str().as_ptr()
    );
    for text in ["", "nul\0identity", "Røw.雪", "choice.type"] {
        let mut changed = program.clone();
        changed.types[1].stable_id = text.to_owned();
        compare(&changed);
        // The reuse helper grants no verifier authority to the changed AST.
        assert_eq!(
            crate::verify::verify(&changed),
            crate::verify::verify(&changed.clone())
        );
    }
    for source in [
        SOURCE.replace("@id(\"child.read\")", "@id(\"base.read\")"),
        SOURCE.replace("Child : Base", "Child : Missing"),
        SOURCE.replace("class Base {", "class Base : Child {"),
    ] {
        let changed = crate::parse(&source, "identity-reuse.spx").unwrap();
        compare(&changed);
        assert!(DeclarationIndex::saved_from_verified_with_prelude(&changed, &[]).is_err());
    }
}

#[test]
fn inserted_identity_reuse_removes_literal_reconstruction_payloads_through_index_drop() {
    let program = crate::check(SOURCE, "identity-reuse.spx").unwrap();
    let (_, old) = measure(|| {
        let mut index = DeclarationIndex::saved_from_verified_with_prelude(&program, &[]).unwrap();
        // Isolate identity reuse under the same current unfunded payload
        // storage policy. The saved constructor above remains complete, and
        // `compare` separately checks its unmodified result against current.
        index.share_legacy_nominal_payloads_unfunded();
        drop(index);
    });
    let (_, new) = measure(|| {
        drop(DeclarationIndex::from_verified_with_prelude(&program, &[]).unwrap());
    });
    // Authored pass census: initial type-parameter owner; resource owner;
    // record/class layout owner; class layout/method owner; variant owner;
    // three inheritance passes plus the one explicit extends link.
    let repeated = [
        ("token.type", 2usize),
        ("row.type", 2),
        ("base.type", 6),
        ("child.type", 7),
        ("choice.type", 2),
        ("base.read", 1),
        ("child.read", 1),
    ];
    assert_eq!(repeated.iter().map(|(_, count)| count).sum::<usize>(), 21);
    let word = std::mem::size_of::<usize>();
    // Independent Arc<str> layout: two atomic counters and text, rounded to
    // machine-word alignment; legacy new additionally clones the source String.
    let removed = repeated
        .iter()
        .map(|(text, count)| count * (text.len() + (2 * word + text.len()).div_ceil(word) * word))
        .sum::<usize>();
    assert_eq!(old.allocations, new.allocations + 42);
    assert_eq!(old.reallocations, new.reallocations);
    assert_eq!(old.allocated, new.allocated + removed);
    for observed in [old, new] {
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[path = "paid.rs"]
mod paid;
#[path = "resolver_held.rs"]
pub(super) mod resolver_held;
