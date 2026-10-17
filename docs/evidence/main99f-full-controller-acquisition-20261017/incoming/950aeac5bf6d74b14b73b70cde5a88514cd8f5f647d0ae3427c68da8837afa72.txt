//! Funded text-profile controls against the complete saved receivers.
use super::super::{oracle, saved};
use super::*;

pub(crate) const LIMIT: usize = usize::MAX;

pub(crate) fn fixture() -> DeclarationIndex {
    let source = crate::parse(
        r#"module text.classifier;
@id("text.row") record Row { @id("text.row.field") field: i64, }
@id("text.box") record Box<T> { @id("text.box.field") field: T, }
@id("text.seed") fn seed(value: borrow Vec<i64>) -> i64 { 0 }
@id("text.main") fn main() -> i64 { 0 }
"#,
        std::path::Path::new("text-classifier.spx"),
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap().declarations.clone()
}

pub(crate) fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}

fn param(owner: &str) -> ResolvedType {
    ResolvedType::TypeParameter {
        owner: DeclarationId::new(owner),
        index: 0,
    }
}

fn missing_param(owner: &str) -> ResolvedType {
    ResolvedType::TypeParameter {
        owner: DeclarationId::new(owner),
        index: 7,
    }
}

fn add(index: &mut DeclarationIndex, template: &str, id: &str, types: Vec<ResolvedType>) {
    let template = DeclarationId::new(template);
    let id = DeclarationId::new(id);
    let mut declaration = index.declarations[&template].clone();
    declaration.id = id.clone();
    index.declarations.insert(id.clone(), declaration);
    let parameters = index.type_parameters[&template].clone();
    index.type_parameters.insert(id.clone(), parameters);
    let field = index.record_fields[&template][0].clone();
    let fields: Vec<crate::hir::ResolvedFieldDeclaration> = types
        .into_iter()
        .map(|ty| {
            let mut field = field.clone();
            field.ty = ty;
            field
        })
        .collect();
    index.record_fields.insert(id, fields);
}

/// Zero-parameter record with the given field types (text-profile metadata only).
pub(crate) fn add_record(index: &mut DeclarationIndex, id: &str, types: Vec<ResolvedType>) {
    add(index, "text.row", id, types);
}

fn add_generic(index: &mut DeclarationIndex, id: &str, types: Vec<ResolvedType>) {
    add(index, "text.box", id, types);
}

pub(crate) struct Seen {
    pub verdict: Option<bool>,
    pub invalid: bool,
    pub leaked: bool,
    pub allocated: usize,
    pub highwater: usize,
    pub refused: bool,
    pub paid: usize,
    pub receipt: bool,
}

pub(crate) fn observe(limit: usize, run: impl FnOnce() -> Result<bool, Diagnostic>) -> Seen {
    let ((verdict, metrics), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            crate::test_allocation_observer::measure(|| {
                let result = run();
                let verdict = result.as_ref().ok().copied();
                if verdict.is_none() {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                }
                drop(result);
                verdict
            })
        });
    Seen {
        verdict,
        invalid: metrics.invalid,
        leaked: metrics.live != 0 || metrics.allocated != metrics.freed,
        allocated: metrics.allocated,
        highwater: metrics.highwater,
        refused,
        paid,
        receipt: receipt.is_some(),
    }
}

/// Exact, one-short and sticky funding with physical allocation <= paid bytes
/// through every real Drop. Returns the exact paid byte count.
pub(crate) fn check(
    expected: bool,
    run: impl Fn() -> Result<bool, Diagnostic>,
    label: &str,
) -> usize {
    let actual = observe(LIMIT, &run);
    assert_eq!(actual.verdict, Some(expected), "{label}");
    assert!(
        !actual.refused && !actual.receipt && !actual.invalid && !actual.leaked,
        "{label}"
    );
    let paid = actual.paid;
    assert!(
        actual.allocated <= paid && actual.highwater <= paid,
        "{label}: allocated {}, highwater {}, paid {paid}",
        actual.allocated,
        actual.highwater
    );
    let exact = observe(paid, &run);
    assert_eq!(exact.verdict, Some(expected), "{label}");
    assert!(
        !exact.refused && !exact.receipt && !exact.invalid && !exact.leaked,
        "{label}"
    );
    assert_eq!(
        (exact.paid, exact.allocated),
        (paid, actual.allocated),
        "{label}"
    );
    if paid > 0 {
        let short = observe(paid - 1, &run);
        assert!(
            short.verdict.is_none()
                && short.refused
                && short.receipt
                && !short.invalid
                && !short.leaked,
            "{label}"
        );
        assert!(short.paid < paid, "{label}");
        assert!(
            short.allocated <= short.paid && short.highwater <= short.paid,
            "{label}: refused backing was acquired before funding"
        );
    }
    paid
}

fn adm(ty: &ResolvedType, index: &DeclarationIndex) -> bool {
    admitted(ty, index).unwrap()
}

fn parity(index: &DeclarationIndex, ty: &ResolvedType) {
    assert_eq!(adm(ty, index), saved::admitted(ty, index));
    assert_eq!(adm(ty, index), oracle::admitted(ty, index));
    assert_eq!(
        contains_string(ty, index).unwrap(),
        saved::contains_string(ty, index)
    );
    assert_eq!(
        contains_string(ty, index).unwrap(),
        oracle::contains_string(ty, index)
    );
    assert_eq!(
        runtime_admitted(ty, index).unwrap(),
        saved::runtime_admitted(ty, index)
    );
    assert_eq!(
        collection_admitted(ty, index).unwrap(),
        crate::hir::owned_collection_record::saved::admitted(ty, index)
    );
    let verdict =
        |result: Result<(), Diagnostic>| result.map_err(|error| (error.code, error.message));
    assert_eq!(
        verdict(validate_use(ty, index)),
        verdict(oracle::validate_use(ty, index))
    );
    assert_eq!(
        verdict(validate_use(ty, index)),
        verdict(saved::validate_use(ty, index))
    );
}

#[test]
fn funded_text_classifiers_preserve_limits_cycles_and_substitution_errors() {
    let mut index = fixture();
    let id = DeclarationId::new("text.row");
    let field = index.record_fields[&id][0].clone();
    let root = nominal("text.row", vec![]);
    for count in [0, 1, 256, 257, 4096, 4097] {
        let mut fields = vec![field.clone(); count];
        for f in &mut fields {
            f.ty = ResolvedType::I64;
        }
        if let Some(f) = fields.first_mut() {
            f.ty = ResolvedType::String;
        }
        index.record_fields.insert(id.clone(), fields);
        parity(&index, &root);
        assert_eq!(adm(&root, &index), count != 0 && count <= 4096);
    }
    for count in [256, 257] {
        let mut fields = vec![field.clone(); count];
        for f in &mut fields {
            f.ty = ResolvedType::String;
        }
        index.record_fields.insert(id.clone(), fields);
        parity(&index, &root);
        assert_eq!(adm(&root, &index), count == 256);
    }
    for depth in [64, 65] {
        for level in 0..depth {
            let level_id = DeclarationId::new(format!("text.depth.{level}"));
            let mut declaration = index.declarations[&id].clone();
            declaration.id = level_id.clone();
            index.declarations.insert(level_id.clone(), declaration);
            index.type_parameters.insert(level_id.clone(), vec![]);
            let mut f = field.clone();
            f.ty = if level + 1 == depth {
                ResolvedType::String
            } else {
                nominal(&format!("text.depth.{}", level + 1), vec![])
            };
            index.record_fields.insert(level_id, vec![f]);
        }
        let ty = nominal("text.depth.0", vec![]);
        parity(&index, &ty);
        assert_eq!(adm(&ty, &index), depth == 64);
    }
    let mut recursive = field.clone();
    recursive.ty = root.clone();
    let mut text = field.clone();
    text.ty = ResolvedType::String;
    index
        .record_fields
        .insert(id.clone(), vec![recursive, text]);
    parity(&index, &root);
    assert!(contains_string(&root, &index).unwrap());
    assert!(!adm(&root, &index));
    for ty in [
        ResolvedType::String,
        ResolvedType::Bytes,
        root.clone(),
        nominal("missing.α:0:", vec![]),
        nominal("text.box", vec![ResolvedType::String]),
        nominal("text.box", vec![]),
    ] {
        parity(&index, &ty);
    }
    // Repeated siblings are not active cycles; the leave frame retires
    // precisely one ancestor before the next sibling is examined.
    let mut leaf = field.clone();
    leaf.ty = ResolvedType::String;
    index.record_fields.insert(id.clone(), vec![leaf]);
    let mut parent = field.clone();
    parent.ty = root.clone();
    let parent_id = DeclarationId::new("text.parent");
    let mut declaration = index.declarations[&id].clone();
    declaration.id = parent_id.clone();
    index.declarations.insert(parent_id.clone(), declaration);
    index.type_parameters.insert(parent_id.clone(), vec![]);
    index
        .record_fields
        .insert(parent_id, vec![parent.clone(), parent]);
    let siblings = nominal("text.parent", vec![]);
    parity(&index, &siblings);
    assert!(adm(&siblings, &index));
    let invariant_id = DeclarationId::new("text.row#invariant");
    let mut invariant = index.declarations[&id].clone();
    invariant.id = invariant_id.clone();
    index.declarations.insert(invariant_id.clone(), invariant);
    parity(&index, &root);
    assert!(!adm(&root, &index));
    index.declarations.remove(&invariant_id);
    for ty in [
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("foreign"),
            index: 0,
        },
        nominal(
            "text.box",
            vec![ResolvedType::TypeParameter {
                owner: id.clone(),
                index: 0,
            }],
        ),
        ResolvedType::Function {
            parameters: vec![ResolvedType::String],
            result: Box::new(ResolvedType::Bool),
        },
    ] {
        let mut f = field.clone();
        f.ty = ty;
        index.record_fields.insert(id.clone(), vec![f]);
        parity(&index, &root);
    }
}

fn funded_index() -> DeclarationIndex {
    let mut index = fixture();
    index
        .record_fields
        .get_mut(&DeclarationId::new("text.row"))
        .unwrap()[0]
        .ty = ResolvedType::String;
    let boxed = |inner: ResolvedType| nominal("text.box", vec![inner]);
    add_generic(&mut index, "text.wrap", vec![boxed(param("text.wrap"))]);
    add_generic(
        &mut index,
        "text.partial",
        vec![boxed(param("foreign")), param("text.partial")],
    );
    add_generic(
        &mut index,
        "text.absent",
        vec![boxed(missing_param("text.absent"))],
    );
    add_generic(
        &mut index,
        "text.callable",
        vec![ResolvedType::Function {
            parameters: vec![param("text.callable")],
            result: Box::new(boxed(param("text.callable"))),
        }],
    );
    add_record(
        &mut index,
        "text.nest",
        vec![nominal("text.row", vec![]), ResolvedType::I64],
    );
    index
}

fn funded_roots() -> Vec<ResolvedType> {
    let string = || vec![ResolvedType::String];
    vec![
        nominal("text.row", vec![]),
        nominal("text.nest", vec![]),
        nominal("text.box", string()),
        nominal("text.wrap", string()),
        nominal("text.wrap", vec![ResolvedType::I64]),
        nominal("text.wrap", vec![nominal("text.box", string())]),
        nominal("text.partial", string()),
        nominal("text.partial", vec![ResolvedType::I64]),
        nominal("text.absent", string()),
        nominal("text.callable", string()),
        nominal("missing.id", vec![]),
        ResolvedType::String,
        ResolvedType::Bytes,
    ]
}

#[test]
fn funded_text_classifiers_match_saved_receivers_and_pay_exactly() {
    let index = funded_index();
    for ty in funded_roots() {
        parity(&index, &ty);
        let label = ty.identity_key();
        check(
            saved::admitted(&ty, &index),
            || admitted(&ty, &index),
            &label,
        );
        check(
            saved::contains_string(&ty, &index),
            || contains_string(&ty, &index),
            &label,
        );
        check(
            saved::runtime_admitted(&ty, &index),
            || runtime_admitted(&ty, &index),
            &label,
        );
        check(
            crate::hir::owned_collection_record::saved::admitted(&ty, &index),
            || collection_admitted(&ty, &index),
            &label,
        );
    }
    let contains = |ty: ResolvedType| contains_string(&ty, &index).unwrap();
    assert!(contains(nominal("text.wrap", vec![ResolvedType::String])));
    assert!(!contains(nominal("text.wrap", vec![ResolvedType::I64])));
    // A foreign-owner field is skipped, not fatal; the sibling still decides.
    assert!(contains(nominal(
        "text.partial",
        vec![ResolvedType::String]
    )));
    assert!(!contains(nominal("text.partial", vec![ResolvedType::I64])));
    assert!(!contains(nominal(
        "text.absent",
        vec![ResolvedType::String]
    )));
}

#[test]
fn substitution_refusal_is_never_swallowed_as_a_skipped_field() {
    let index = funded_index();
    for ty in [
        nominal(
            "text.wrap",
            vec![nominal("text.box", vec![ResolvedType::String])],
        ),
        nominal("text.callable", vec![ResolvedType::String]),
    ] {
        let expected = saved::contains_string(&ty, &index);
        let paid = check(
            expected,
            || contains_string(&ty, &index),
            &ty.identity_key(),
        );
        assert!(paid > 0);
        for limit in 0..paid {
            let seen = observe(limit, || contains_string(&ty, &index));
            assert!(
                seen.verdict.is_none()
                    && seen.refused
                    && seen.receipt
                    && !seen.invalid
                    && !seen.leaked,
                "limit {limit} of {paid}"
            );
            assert!(
                seen.allocated <= seen.paid && seen.highwater <= seen.paid,
                "limit {limit} of {paid}: backing preceded funding"
            );
        }
    }
}

#[test]
fn funded_text_classifiers_zero_debit_and_sticky_refusal() {
    let index = funded_index();
    for run in [
        &(|| contains_string(&ResolvedType::String, &index))
            as &dyn Fn() -> Result<bool, Diagnostic>,
        &(|| admitted(&ResolvedType::Bytes, &index)),
        &(|| runtime_admitted(&ResolvedType::String, &index)),
    ] {
        let seen = observe(0, run);
        assert_eq!(seen.verdict, Some(false));
        assert!(!seen.refused && !seen.receipt && !seen.invalid && !seen.leaked);
        assert_eq!((seen.paid, seen.allocated), (0, 0));
    }
    let row = nominal("text.row", vec![]);
    let ((), overflow) = crate::bounded_output::with_limit(0, || {
        assert!(contains_string(&row, &index).is_err());
        // The refusal is sticky: even a trivially false request is refused now.
        assert!(contains_string(&ResolvedType::String, &index).is_err());
        assert!(admitted(&row, &index).is_err());
        assert!(runtime_admitted(&row, &index).is_err());
        assert!(collection_admitted(&row, &index).is_err());
        assert!(validate_use(&row, &index).is_err());
        assert!(!crate::bounded_output::reserve_active_required(0));
    });
    assert!(overflow);
    let (_, overflow) = crate::bounded_output::with_limit(LIMIT, || {
        assert!(crate::bounded_output::set_active_floor(1));
        assert!(contains_string(&row, &index).is_ok());
    });
    assert!(!overflow);
}

const ORIGINAL_SELECTION_SOURCE: &str = "module text.saved; @id(\"saved.row\") record Row { @id(\"saved.field\") field: String, } @id(\"saved.main\") fn main() -> i64 { 0 }";

#[test]
fn original_uppercase_string_selection_fixture_keeps_full_unknown_type_refusal() {
    let path = std::path::Path::new("saved.spx");
    let diagnostics = crate::check(ORIGINAL_SELECTION_SOURCE, path).unwrap_err();
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        ["SPX-T001"]
    );
    let source = crate::parse(ORIGINAL_SELECTION_SOURCE, path).unwrap();
    assert_eq!(crate::hir::resolve(&source).unwrap_err(), diagnostics);
}

#[test]
fn full_saved_receiver_sources_remain_byte_identical_and_program_selection_matches() {
    use sha2::{Digest, Sha256};
    for (source, expected) in [
        (
            ci_str!("../saved.rs"),
            "84528240afefb008d37eb14ec040f92017d56e1c404e563e4ec9147d7f445cbb",
        ),
        (
            ci_str!("../../owned_collection_record/saved.rs"),
            "60dc4fbe4f8610edf46d111a9b187e2fc039d81fb50706fc9c77e49461a52114",
        ),
    ] {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            expected
        );
    }
    let admitted = ORIGINAL_SELECTION_SOURCE.replace("field: String", "field: string");
    assert_ne!(admitted, ORIGINAL_SELECTION_SOURCE);
    let source = crate::check(&admitted, std::path::Path::new("saved.spx")).unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    assert_eq!(
        saved::program_uses_strings(&program),
        super::super::program_uses_strings(&program)
    );
}

#[test]
fn borrowed_scalar_record_reduces_full_original_allocation_through_drop() {
    let index = fixture();
    let ty = nominal("text.row", Vec::new());
    let old = observe(LIMIT, || Ok(saved::contains_string(&ty, &index)));
    let new = observe(LIMIT, || contains_string(&ty, &index));
    assert_eq!(old.verdict, new.verdict);
    assert!(!old.invalid && !old.leaked && !new.invalid && !new.leaked);
    assert!(old.allocated > new.allocated);
    assert!(new.allocated <= new.paid && new.highwater <= new.paid);
}

#[test]
fn inline_scalar_children_and_paid_spills_keep_full_saved_receiver_and_drop() {
    let mut index = fixture();
    let id = DeclarationId::new("text.row");
    let field = index.record_fields[&id][0].clone();
    let ty = nominal("text.row", Vec::new());
    for count in [0, 1, 2, 17] {
        index
            .record_fields
            .insert(id.clone(), vec![field.clone(); count]);
        let old = observe(LIMIT, || Ok(saved::contains_string(&ty, &index)));
        let new = observe(LIMIT, || contains_string(&ty, &index));
        assert_eq!(old.verdict, Some(false));
        assert_eq!(old.verdict, new.verdict);
        assert!(!old.invalid && !old.leaked && !new.invalid && !new.leaked);
        assert!(old.allocated > new.allocated);
        if count == 1 {
            // The same visited-node allocation remains. Only the saved initial
            // one-element Cow Vec backing disappears from the full receiver.
            assert_eq!(
                old.allocated,
                new.allocated + std::mem::size_of::<Cow<'_, ResolvedType>>()
            );
        }
        check(
            false,
            || contains_string(&ty, &index),
            "inline scalar/spill backing",
        );
        parity(&index, &ty);
    }
}

#[test]
fn explicit_collect_types_preserve_complete_prior_text_and_early_profile_controls() {
    use sha2::{Digest, Sha256};
    let sources = [
        (ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/owned_text_record/paid/tests.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nominal-collect-test-adapters-504b/original/src__hir__owned_text_record__paid__tests.rs.before.txt")), "8ea7173a52d53b2d5fb4184e7c620cb8c8be536ac277b4e629501cfddd7a1c57"),
        (ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation/function_scope/collection_tests/funded_early.rs")), ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/nominal-collect-test-adapters-504b/original/src__hir__validation__function_scope__collection_tests__funded_early.rs.before.txt")), "304f391011d2bb12acd4d4cd4827f8221d7e330f7bb50f1609865ba5ecf3a264"),
    ];
    let inverse: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/nominal-collect-test-adapters-504b/exact-inverses.json"
    )))
    .unwrap();
    assert_eq!(inverse["files"].as_array().unwrap().len(), sources.len());
    for (index, (current, original, fixed)) in sources.into_iter().enumerate() {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            fixed
        );
        let mut restored = current.to_owned();
        for edit in inverse["files"][index]["splices"]
            .as_array()
            .unwrap()
            .iter()
            .rev()
        {
            let at = edit["start"].as_u64().unwrap() as usize;
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.get(at..at + after.len()), Some(after));
            restored.replace_range(at..at + after.len(), edit["before"].as_str().unwrap());
        }
        assert_eq!(restored, original);
    }
}
