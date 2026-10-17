use super::super::FunctionIter;
use super::*;
use crate::ast::program_source::function_alias::{FunctionDirectory, FunctionEntry, FunctionView};
use crate::ast::program_source::generated_roots::GeneratedRoots;
use std::path::Path;

#[test]
fn stored_cell_membership_and_effective_name_lookup_keep_first_match() {
    let provider = crate::parse(
        "module provider; fn original()->i64{1} fn second()->i64{2}",
        Path::new("provider.spx"),
    )
    .unwrap();
    let authored =
        crate::parse("module caller; fn main()->i64{0}", Path::new("caller.spx")).unwrap();
    // This is a representation control, not a producer/admission success claim.
    // Direct metadata control: actual producer admission has separate scope tests.
    let directory = FunctionDirectory {
        roots: GeneratedRoots::plain(Vec::new(), Vec::new()),
        entries: vec![
            entry(&provider, &provider.functions[0]),
            entry(&provider, &provider.functions[1]),
        ],
    };
    let foreign = FunctionDirectory {
        roots: GeneratedRoots::plain(Vec::new(), Vec::new()),
        entries: vec![entry(&provider, &provider.functions[0])],
    };
    let source = FunctionSlices::projected(&authored.functions, &directory).unwrap();
    let detached = FunctionSlices::projected(&authored.functions, &foreign).unwrap();
    assert!(std::ptr::eq(
        source.payload_by_name("local_alias").unwrap(),
        &provider.functions[0]
    ));
    assert!(std::ptr::eq(
        source
            .find_named_payload("local_alias", |function| function.name == "second")
            .unwrap(),
        &provider.functions[1]
    ));
    assert!(source.unaliased_payload_by_name("local_alias").is_none());
    assert!(!source.contains_unaliased_payload(&provider.functions[0]));
    assert!(source.contains_unaliased_payload(&authored.functions[0]));
    assert!(source.payload_by_name("original").is_none());
    assert!(source.payload_by_name("missing").is_none());
    assert!(std::ptr::eq(
        source.payload_by_name("main").unwrap(),
        &authored.functions[0]
    ));
    let first = source.get(authored.functions.len()).unwrap();
    let second = source.get(authored.functions.len() + 1).unwrap();
    assert!(source.contains_presentation(&first));
    assert!(source.contains_presentation(&second));
    assert!(!source.contains_presentation(&detached.get(authored.functions.len()).unwrap()));
    assert!(!source.contains_presentation(&FunctionRef::ordinary(&provider.functions[0])));
    assert!(source.contains_presentation(&source.get(0).unwrap()));
    let suffix = source.suffix(authored.functions.len() + 1).unwrap();
    assert!(!suffix.contains_presentation(&first));
    assert!(suffix.contains_presentation(&second));
    assert!(std::ptr::eq(
        suffix.payload_by_name("local_alias").unwrap(),
        &provider.functions[1]
    ));
}

#[test]
fn selected_and_generated_payloads_keep_exact_reference_membership() {
    let authored = crate::parse(
        "module authored; fn skipped()->i64{0} fn selected()->i64{1}",
        Path::new("authored.spx"),
    )
    .unwrap();
    let generated = crate::parse(
        "module generated; fn generated()->i64{2}",
        Path::new("generated.spx"),
    )
    .unwrap();
    let indices = [1];
    let source =
        FunctionSlices::selected(&authored.functions, &indices, &generated.functions).unwrap();
    assert!(source.payload_by_name("skipped").is_none());
    assert!(std::ptr::eq(
        source.payload_by_name("selected").unwrap(),
        &authored.functions[1]
    ));
    assert!(std::ptr::eq(
        source.payload_by_name("generated").unwrap(),
        &generated.functions[0]
    ));
    assert!(!source.contains_presentation(&FunctionRef::ordinary(&authored.functions[0])));
    assert!(source.contains_presentation(&source.get(0).unwrap()));
    assert!(source.contains_presentation(&source.get(1).unwrap()));
    assert!(source.get(usize::MAX).is_none());
    let duplicate = crate::parse(
        "module member; fn same()->i64{1} fn same()->i64{2}",
        "member.spx",
    )
    .unwrap();
    let slices = FunctionSlices::one(&duplicate.functions);
    let second = {
        let short = &duplicate.functions[1];
        slices.unaliased_payload_member(short).unwrap()
    };
    assert!(std::ptr::eq(second, &duplicate.functions[1]));
    let detached = duplicate.functions[1].clone();
    assert!(slices.unaliased_payload_member(&detached).is_none());
}

fn entry<'a>(provider: &'a crate::ast::Program, original: &'a Function) -> FunctionEntry<'a> {
    FunctionEntry::Borrowed(FunctionView {
        _provider: provider,
        original,
        _alias: "local_alias",
    })
}

#[test]
fn bytes_aliases_cannot_enter_raw_proof_assignment_or_transitive_cursor() {
    use crate::ast::program_source::{function_alias::TransientFunctions, ProgramSource};
    use crate::ast::{Expr, ExprKind, Span};
    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/owned_data/byte_helper_renewal.spx"
    ));
    let provider = crate::parse(SOURCE, "byte-provider.spx").unwrap();
    let mut authored = crate::parse(SOURCE, "byte-caller.spx").unwrap();
    authored
        .functions
        .retain(|function| function.name != "leaf");
    let leaf = provider
        .functions
        .iter()
        .find(|function| function.name == "leaf")
        .unwrap();
    // Deliberately create a representation outside the admitted wrapper subset.
    // Even a name-equal borrowed Bytes payload cannot reach the raw proof key.
    let source = TransientFunctions::new(
        &authored,
        FunctionDirectory {
            roots: GeneratedRoots::plain(Vec::new(), Vec::new()),
            entries: vec![FunctionEntry::Borrowed(FunctionView {
                _provider: &provider,
                original: leaf,
                _alias: "leaf",
            })],
        },
    );
    let cell = source
        .function_refs()
        .get(authored.functions.len())
        .unwrap();
    let assignment = Expr {
        kind: ExprKind::Call {
            name: "leaf".to_owned(),
            type_arguments: Vec::new(),
            args: ["output", "index", "value"]
                .into_iter()
                .map(|name| Expr {
                    kind: ExprKind::Var(name.to_owned()),
                    span: Span::default(),
                })
                .collect(),
        },
        span: Span::default(),
    };
    let index = crate::loop_calls::byte_writer::SourceIndex::new(&source);
    let (_, refused, used) = crate::bounded_output::with_limit_usage(0, || {
        assert!(!index.target_ref(&cell));
        assert!(!index.target(leaf));
        assert!(!index.assignment("output", &assignment));
    });
    assert!(!refused);
    assert_eq!(used, 0);
    assert_eq!(index.owned_capacity_bytes(), 0);
    let relay = authored
        .functions
        .iter()
        .find(|function| function.name == "relay")
        .unwrap();
    let (admitted, refused, _) =
        crate::bounded_output::with_limit_usage(usize::MAX, || index.target(relay));
    assert!(
        !admitted,
        "an ordinary caller cannot cross to a borrowed alias in the raw cursor"
    );
    assert!(!refused);
    assert!(index.failure().is_none());
}

#[test]
fn metadata_facade_and_cursor_headers_keep_actual_old_target_layout() {
    use crate::ast::program_source::{decl_slices::DeclIter, DeclSlices};
    use std::mem::{align_of, size_of};
    assert_eq!(
        size_of::<FunctionSlices<'_>>(),
        size_of::<DeclSlices<'_, Function>>()
    );
    assert_eq!(
        align_of::<FunctionSlices<'_>>(),
        align_of::<DeclSlices<'_, Function>>()
    );
    assert_eq!(
        size_of::<FunctionIter<'_>>(),
        size_of::<DeclIter<'_, Function>>()
    );
    assert_eq!(
        align_of::<FunctionIter<'_>>(),
        align_of::<DeclIter<'_, Function>>()
    );
    assert_eq!(
        size_of::<Option<&Function>>(),
        size_of::<Option<&FunctionRef<'_>>>()
    );
    assert_eq!(
        size_of::<crate::loop_calls::byte_writer::SourceIndex<'_>>(),
        size_of::<crate::loop_calls::byte_writer::source_index_original::SourceIndex<'_>>()
    );
    assert_eq!(
        align_of::<crate::loop_calls::byte_writer::SourceIndex<'_>>(),
        align_of::<crate::loop_calls::byte_writer::source_index_original::SourceIndex<'_>>()
    );
    // Report the real target's value-carrier expansion separately from the
    // equal-width borrowed receivers. This is not a measured memory gain.
    eprintln!(
        "847 function payload/view bytes: {} {}; facade bytes: {} {}; iterator bytes: {} {}",
        size_of::<&Function>(),
        size_of::<FunctionRef<'_>>(),
        size_of::<DeclSlices<'_, Function>>(),
        size_of::<FunctionSlices<'_>>(),
        size_of::<DeclIter<'_, Function>>(),
        size_of::<FunctionIter<'_>>()
    );
}
