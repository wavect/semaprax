//! Shared declaration identity backing against an independent owned-String
//! oracle, the actual allocator and the active reservation ledger.
use super::DeclarationId;
use crate::bounded_output::{reserve_active_required, with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

/// The former representation, kept only as the oracle for every trait,
/// projection and cache wire the shared backing must reproduce.
#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Legacy(String);
crate::cache_codec::codec_tuple!(Legacy(0));

const CORPUS: [&str; 9] = [
    "",
    "a",
    "app.main",
    "app.main\0forged",
    "λ.函数.main",
    "owner:é.λ:extra",
    "emoji.🦀.width",
    "e\u{301}.combining",
    "app.mains",
];

fn hash<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn long() -> String {
    "segment.".repeat(512)
}

#[test]
fn every_projection_and_trait_matches_the_owned_string_oracle() {
    let mut texts = CORPUS.map(str::to_owned).to_vec();
    texts.push(long());
    for text in &texts {
        let id = DeclarationId::new(text.clone());
        let legacy = Legacy(text.clone());
        assert_eq!(id.as_str().as_bytes(), text.as_bytes());
        assert_eq!(id.as_str().len(), text.len());
        assert_eq!(id.to_string(), *text);
        // The derived Debug of the former `DeclarationId(String)` tuple.
        assert_eq!(format!("{id:?}"), format!("DeclarationId({text:?})"));
        assert_eq!(format!("{legacy:?}"), format!("Legacy({text:?})"));
        assert_eq!(hash(&id), hash(&legacy));
        assert_eq!(hash(&id), hash(text));
        for other in &texts {
            let other_id = DeclarationId::new(other.as_str());
            assert_eq!(id == other_id, text == other);
            assert_eq!(id.cmp(&other_id), text.cmp(other));
            assert_eq!(id.partial_cmp(&other_id), text.partial_cmp(other));
        }
    }
    let ids = texts
        .iter()
        .rev()
        .map(DeclarationId::new)
        .collect::<BTreeSet<_>>();
    let oracle = texts.iter().rev().cloned().collect::<BTreeSet<_>>();
    assert!(ids
        .iter()
        .map(DeclarationId::as_str)
        .eq(oracle.iter().map(String::as_str)));
}

#[test]
fn cache_wire_is_the_former_owned_string_tuple_in_both_directions() {
    let mut texts = CORPUS.map(str::to_owned).to_vec();
    texts.push(long());
    for text in texts {
        let id = DeclarationId::new(text.as_str());
        let wire = crate::cache_codec::encode(&Legacy(text.clone())).unwrap();
        assert_eq!(crate::cache_codec::encode(&id).unwrap(), wire);
        let decoded = crate::cache_codec::decode::<DeclarationId>(&wire).unwrap();
        assert_eq!(decoded, id);
        assert_eq!(
            decoded.owned_allocation_bytes(),
            id.owned_allocation_bytes()
        );
        let legacy =
            crate::cache_codec::decode::<Legacy>(&crate::cache_codec::encode(&id).unwrap());
        assert_eq!(legacy.unwrap().0, text);
    }
    // Stale or forged wire is still refused by the unchanged String grammar.
    let wire = crate::cache_codec::encode(&Legacy("app.main".to_owned())).unwrap();
    assert!(crate::cache_codec::decode::<DeclarationId>(&wire[..wire.len() - 1]).is_err());
    let mut invalid = wire.clone();
    let last = invalid.len() - 1;
    invalid[last] = 0xff;
    let error = crate::cache_codec::decode::<DeclarationId>(&invalid).unwrap_err();
    assert_eq!(error[0].message, "cache codec string is not UTF-8");
}

#[test]
fn backing_width_is_the_actual_requested_arc_layout() {
    #[cfg(target_pointer_width = "64")]
    assert_eq!(
        std::mem::size_of::<DeclarationId>(),
        std::mem::size_of::<String>()
    );
    let counters = 2 * std::mem::size_of::<usize>();
    let align = std::mem::align_of::<usize>();
    for len in (0..=33).chain([4096]) {
        let expected = (counters + len).div_ceil(align) * align;
        assert_eq!(DeclarationId::shared_backing_bytes(len), Some(expected));
        let text = "x".repeat(len);
        // The String is allocated before observation: only the backing
        // requested by the constructor itself is visible here.
        let (id, observed) = measure(|| DeclarationId::new(text));
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, 1);
        assert_eq!(observed.allocated, expected);
        assert_eq!(observed.live, expected);
        assert_eq!(id.owned_allocation_bytes(), expected);
    }
    assert_eq!(DeclarationId::shared_backing_bytes(usize::MAX), None);
    assert_eq!(
        DeclarationId::shared_backing_bytes(isize::MAX as usize),
        None
    );
}

#[test]
fn legacy_borrowed_text_is_valid_and_explicitly_unfunded_under_exhausted_budget() {
    let text = "λ.函数.main";
    let backing = DeclarationId::shared_backing_bytes(text.len()).unwrap();
    let ((id, observed), overflowed, used) =
        with_limit_usage(0, || measure(|| DeclarationId::new(text)));
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(observed.allocations, 2);
    assert_eq!(observed.allocated, text.len() + backing);
    assert_eq!(observed.highwater, text.len() + backing);
    assert_eq!(observed.freed, text.len());
    assert_eq!(observed.live, backing);
    assert_eq!(id.as_str(), text);
}

#[test]
fn exact_owned_backing_and_one_short_return_no_refused_identity() {
    for text in ["", "app.main", "app.main\0forged", "λ.函数.main"] {
        let backing = DeclarationId::shared_backing_bytes(text.len()).unwrap();
        let owned = text.to_owned();
        let ((accepted, observed), overflowed, used) = with_limit_usage(backing, || {
            measure(|| {
                let id = DeclarationId::paid_owned(owned, |bytes| {
                    if reserve_active_required(bytes) {
                        Ok(())
                    } else {
                        Err(())
                    }
                })
                .unwrap();
                assert_eq!(id.as_str(), text);
                drop(id);
            })
        });
        assert_eq!(accepted, ());
        assert!(!overflowed);
        assert_eq!(used, backing);
        assert_eq!(observed.allocations, 1);
        assert_eq!(observed.allocated, backing);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.freed, backing);
        let owned = text.to_owned();
        let (((refused, observed), retry), overflowed, used, refusal) =
            with_limit_usage_refusal(backing - 1, || {
                let refused = measure(|| {
                    DeclarationId::paid_owned(owned, |bytes| {
                        if reserve_active_required(bytes) {
                            Ok(())
                        } else {
                            Err(())
                        }
                    })
                });
                (refused, reserve_active_required(0))
            });
        assert!(overflowed);
        assert!(!retry);
        assert_eq!(used, 0);
        assert_eq!(refusal.unwrap().requested, backing);
        assert_eq!(refused, Err(()));
        assert_eq!(observed.allocations, 0);
        // Valid empty and NUL-bearing values remain distinct; an Err is not a
        // carrier and cannot reach equality, maps, HIR or encoding.
        assert_eq!(DeclarationId::new(text).as_str(), text);
    }
}

#[test]
fn clones_share_one_backing_without_allocation_charge_or_early_release() {
    let original = DeclarationId::new("app.shared.identity");
    let backing = original.owned_allocation_bytes();
    let ((clones, observed), overflowed, used) = with_limit_usage(0, || {
        measure(|| (0..64).map(|_| original.clone()).collect::<Vec<_>>())
    });
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert!(!observed.invalid);
    // Only the collecting Vec allocates; no clone requests identity backing.
    assert_eq!(observed.allocations, 1);
    assert_eq!(
        observed.allocated,
        64 * std::mem::size_of::<DeclarationId>()
    );
    for clone in &clones {
        assert_eq!(clone, &original);
        assert_eq!(
            clone.shared_allocation_key(),
            original.shared_allocation_key()
        );
        assert_eq!(clone.owned_allocation_bytes(), backing);
    }
    // Dropping clones frees nothing while the original still owns the bytes;
    // the last owner releases exactly the backing.
    let ((), observed) = measure(|| {
        let text = "app.dropped.identity";
        let last = DeclarationId::new(text);
        let expected = last.owned_allocation_bytes();
        let constructed = crate::test_allocation_observer::current_metrics();
        assert_eq!(constructed.allocated, text.len() + expected);
        assert_eq!(constructed.live, expected);
        let copies = [last.clone(), last.clone()];
        drop(copies);
        let shared = crate::test_allocation_observer::current_metrics();
        assert_eq!(shared.allocated, constructed.allocated);
        assert_eq!(shared.freed, constructed.freed);
        drop(last);
        let settled = crate::test_allocation_observer::current_metrics();
        assert_eq!(settled.freed, constructed.freed + expected);
        assert_eq!(settled.live, 0);
    });
    assert_eq!(observed.live, 0);
    drop(clones);
    assert_eq!(original.as_str(), "app.shared.identity");
}

#[test]
fn duplicates_stay_equal_values_with_independent_backings() {
    let first = DeclarationId::new("app.duplicate");
    let second = DeclarationId::new(String::from("app.duplicate"));
    assert_eq!(first, second);
    assert_eq!(first.cmp(&second), std::cmp::Ordering::Equal);
    assert_eq!(hash(&first), hash(&second));
    assert_ne!(
        first.shared_allocation_key(),
        second.shared_allocation_key()
    );
    let set = [first.clone(), second, first.clone()]
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(set.len(), 1);
    // Pointer identity carries no authority: a shared clone and an
    // independent construction answer every comparison identically.
    let other = DeclarationId::new("app.duplicatf");
    assert_eq!(first.clone() < other, first < other);
}

#[test]
fn valid_variants_preserve_empty_nul_unicode_and_have_no_forged_hash_state() {
    let constant = DeclarationId::compiler(super::CompilerDeclaration::Vec);
    let shared = DeclarationId::new(crate::prelude::VEC_ID);
    assert!(constant.is_compiler_static());
    assert!(!shared.is_compiler_static());
    assert_eq!(constant, shared);
    assert_eq!(hash(&constant), hash(&shared));
    assert_eq!(format!("{constant:?}"), format!("{shared:?}"));
    assert_eq!(constant.owned_allocation_bytes(), 0);
    for text in CORPUS {
        let first = DeclarationId::new(text);
        let second = DeclarationId::new(text);
        assert_eq!(first, second);
        assert_eq!(first.cmp(&second), std::cmp::Ordering::Equal);
        assert_eq!(hash(&first), hash(text));
        for other in CORPUS {
            assert_eq!(first == DeclarationId::new(other), text == other);
        }
        let wire = crate::cache_codec::encode(&first).unwrap();
        assert_eq!(
            crate::cache_codec::decode::<DeclarationId>(&wire).unwrap(),
            first
        );
    }
    assert_ne!(DeclarationId::new(""), DeclarationId::new("\0"));
    assert_ne!(
        DeclarationId::new("app.main"),
        DeclarationId::new("app.main\0forged")
    );
}

#[test]
fn refused_result_has_no_wire_and_decoder_keeps_both_cap_boundaries() {
    let text = "codec-refused\0λ";
    let backing = DeclarationId::shared_backing_bytes(text.len()).unwrap();
    let ((result, observed), overflowed, used) = with_limit_usage(0, || {
        measure(|| {
            DeclarationId::paid_borrowed(text, |bytes| {
                if reserve_active_required(bytes) {
                    Ok(())
                } else {
                    Err(())
                }
            })
        })
    });
    assert_eq!(result, Err(()));
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(observed.allocations, 0);
    let wire = crate::cache_codec::encode(&Legacy(text.to_owned())).unwrap();
    let (decoded, overflowed, used) = with_limit_usage(backing, || {
        crate::cache_codec::decode::<DeclarationId>(&wire)
    });
    assert!(!overflowed);
    assert_eq!(used, backing);
    assert_eq!(decoded.unwrap().as_str(), text);
    let (decoded, overflowed, used) = with_limit_usage(backing - 1, || {
        crate::cache_codec::decode::<DeclarationId>(&wire)
    });
    assert!(overflowed);
    assert_eq!(used, 0);
    let error = decoded.unwrap_err();
    assert_eq!(error[0].code, "SPX-G305");
    assert_eq!(
        error[0].message,
        "cache codec declaration identity backing allocation was refused"
    );
    // Decoder-local allocation cap remains independent of the owning payer.
    struct Saturated;
    impl crate::cache_codec::Codec for Saturated {
        fn encode(&self, _: &mut crate::cache_codec::Encoder) -> crate::cache_codec::Result<()> {
            unreachable!("decode-only cap owner")
        }
        fn decode(
            decoder: &mut crate::cache_codec::Decoder<'_>,
        ) -> crate::cache_codec::Result<Self> {
            decoder.allocate(crate::cache_codec::MAX_ALLOCATION - std::mem::size_of::<Self>())?;
            <DeclarationId as crate::cache_codec::Codec>::decode(decoder)?;
            Ok(Self)
        }
    }
    let refused = crate::cache_codec::decode::<Saturated>(&wire)
        .err()
        .unwrap();
    assert_eq!(
        refused[0].message,
        "cache codec exceeds its charged allocation limit"
    );
}

#[test]
fn selected_payer_reserves_exactly_the_backing_without_a_temporary_text() {
    use crate::cleanup_plan::ReplayPhysicalReservation;
    let long = long();
    for text in [
        "",
        "app.main",
        "app.main\0forged",
        "λ.函数.main",
        long.as_str(),
    ] {
        let backing = DeclarationId::shared_backing_bytes(text.len()).unwrap();
        let ((id, physical, observed), overflowed, used) = with_limit_usage(backing, || {
            let mut physical = ReplayPhysicalReservation::new(backing);
            let (id, observed) =
                measure(|| DeclarationId::paid_borrowed(text, |bytes| physical.reserve(bytes)));
            (id, physical, observed)
        });
        // The physical ledger forwards its one debit to the enclosing budget;
        // the constructor adds no second ambient charge of its own.
        assert!(!overflowed);
        assert_eq!(used, backing);
        assert_eq!(physical.charged(), backing);
        let id = id.unwrap();
        assert!(!observed.invalid);
        assert_eq!(observed.allocations, 1);
        assert_eq!(observed.allocated, backing);
        assert_eq!(observed.highwater, backing);
        assert_eq!(observed.live, backing);
        assert_eq!(id.as_str(), text);
        assert_eq!(id, DeclarationId::new(text));
        assert_eq!(id.owned_allocation_bytes(), backing);
    }
}

#[test]
fn selected_payer_one_short_refuses_sticky_before_allocating() {
    use crate::cleanup_plan::{ReplayPhysicalRefusal, ReplayPhysicalReservation};
    let text = "app.selected.identity";
    let backing = DeclarationId::shared_backing_bytes(text.len()).unwrap();
    let ((result, physical, observed), overflowed, used) = with_limit_usage(1 << 20, || {
        let mut physical = ReplayPhysicalReservation::new(backing - 1);
        let (result, observed) =
            measure(|| DeclarationId::paid_borrowed(text, |bytes| physical.reserve(bytes)));
        (result, physical, observed)
    });
    assert_eq!(result.unwrap_err(), ReplayPhysicalRefusal::LocalLimit);
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert!(!observed.invalid);
    assert_eq!(observed.allocations, 0);
    let mut physical = physical;
    assert_eq!(physical.charged(), 0);
    assert_eq!(physical.reserve(0), Err(ReplayPhysicalRefusal::LocalLimit));
    // An exhausted enclosing budget refuses through the same payer.
    let (result, overflowed, used) = with_limit_usage(backing - 1, || {
        let mut physical = ReplayPhysicalReservation::new(backing);
        DeclarationId::paid_borrowed(text, |bytes| physical.reserve(bytes))
    });
    assert_eq!(result.unwrap_err(), ReplayPhysicalRefusal::EnclosingBudget);
    assert!(overflowed);
    assert_eq!(used, 0);
}

#[test]
fn selected_payer_is_the_only_authority_consulted() {
    let text = "app.selected.only";
    let backing = DeclarationId::shared_backing_bytes(text.len()).unwrap();
    // A payer that does not forward is charged once with the exact backing;
    // an exhausted ambient budget is neither consulted nor charged.
    let mut requests = Vec::new();
    let (id, overflowed, used) = with_limit_usage(0, || {
        DeclarationId::paid_borrowed(text, |bytes| {
            requests.push(bytes);
            Ok::<_, ()>(())
        })
    });
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(requests, [backing]);
    assert_eq!(id.unwrap().as_str(), text);
    // A refusing payer constructs nothing, whatever the ambient allowance.
    let ((refused, observed), overflowed, used) = with_limit_usage(1 << 20, || {
        measure(|| DeclarationId::paid_borrowed(text, |_| Err(())))
    });
    assert_eq!(refused, Err(()));
    assert!(!overflowed);
    assert_eq!(used, 0);
    assert_eq!(observed.allocations, 0);
}

fn constructor_sites(
    root: &std::path::Path,
    dir: &std::path::Path,
    needle: &str,
    found: &mut Vec<(String, String, String)>,
) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name != "tests") {
                constructor_sites(root, &path, needle, found);
            }
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy();
        if !name.ends_with(".rs") || name.ends_with("tests.rs") {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&path).unwrap();
        let mut previous = "";
        for line in source.lines() {
            if line.contains(needle) {
                found.push((relative.clone(), previous.to_owned(), line.to_owned()));
            }
            previous = line;
        }
    }
}

/// Only listed paid domains may claim their identity constructions funded.
/// Other legacy new calls remain open and are not covered by this control.
#[test]
fn qualified_identity_domains_have_no_legacy_constructor_or_refusal_carrier() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut sites = Vec::new();
    constructor_sites(&root, &root, "DeclarationId::paid_borrowed(", &mut sites);
    // Exact constructor census includes the later nominal/agent/prelude and
    // lifecycle payers, plus their complete independent historical twins.
    // Filesystem enumeration order grants no semantic meaning.
    sites.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        sites
            .iter()
            .map(|(path, ..)| path.as_str())
            .collect::<Vec<_>>(),
        [
            "cleanup_plan/replay/nested_shape/paid.rs",
            "cleanup_plan/replay/nested_shape/paid.rs",
            "cleanup_plan/replay/nested_shape/paid.rs",
            "cleanup_plan/replay/nested_shape/paid.rs",
            "cleanup_plan/replay/nested_shape/paid/oracle.rs",
            "cleanup_plan/replay/nested_shape/paid/oracle.rs",
            "cleanup_plan/replay/nested_shape/paid/oracle.rs",
            "cleanup_plan/replay/nested_shape/paid/oracle.rs",
            "cleanup_plan/replay/nested_shape/paid_before.rs",
            "cleanup_plan/replay/nested_shape/paid_before.rs",
            "cleanup_plan/replay/nested_shape/paid_before.rs",
            "cleanup_plan/replay/nested_shape/paid_before.rs",
            "hir/cache_codec.rs",
            "hir/declaration_index/identity_payer.rs",
            "hir/declaration_index/nominal_construction.rs",
            "hir/identity_resolution.rs",
            "hir/identity_resolution.rs",
            "hir/identity_resolution/nominal.rs",
            "hir/identity_resolution/nominal.rs",
            "hir/ordinary_identity.rs",
            "hir/prelude_bundle/paid.rs",
        ]
    );
    for path in [
        "byte_ops.rs",
        "hir/cache_codec.rs",
        "hir/resolve_for/bytes.rs",
        "hir/row_loan/producer.rs",
        "cleanup_plan/replay/nested_shape/paid.rs",
        "cleanup_plan/replay/nested_shape/paid/oracle.rs",
        "cleanup_plan/replay/nested_shape/paid_before.rs",
        "hir/declaration_index/identity_payer.rs",
        "hir/declaration_index/nominal_construction.rs",
        "hir/identity_resolution.rs",
        "hir/identity_resolution/nominal.rs",
        "hir/ordinary_identity.rs",
        "hir/prelude_bundle/paid.rs",
        "cleanup/inventory_builder/selected.rs",
        "cleanup_plan/replay/flat_records.rs",
        "hir/declaration_index/primitive_replay/option_u8.rs",
        "hir/declaration_index/primitive_replay/vector.rs",
        "workspace_graph/source_schema/static_text/primitive/records/prepared.rs",
    ] {
        let source = std::fs::read_to_string(root.join(path)).unwrap();
        assert!(
            !source.contains("DeclarationId::new("),
            "legacy allocation remains in qualified {path}"
        );
    }
    let ids = std::fs::read_to_string(root.join("hir/ids.rs")).unwrap();
    let legacy = ids
        .split("pub fn new(value: impl Into<String>)")
        .nth(1)
        .unwrap()
        .split("pub(crate) const fn compiler")
        .next()
        .unwrap();
    assert!(!legacy.contains("bounded_output"));
    assert!(!legacy.contains("reserve_active"));
    assert!(!ids.contains("Refused"));
    assert!(!ids
        .split("pub struct FunctionInstanceId")
        .next()
        .unwrap()
        .contains("invalidate_fingerprint"));
}

#[test]
fn compiler_statics_and_pure_queries_ignore_unrelated_exhausted_budget() {
    let expected = crate::byte_ops::ByteOp::Get.return_type();
    let ((actual, identity, retry), overflowed, used) = with_limit_usage(0, || {
        assert!(!reserve_active_required(1));
        let identity = DeclarationId::compiler(super::CompilerDeclaration::Vec);
        (
            crate::byte_ops::ByteOp::Get.return_type(),
            identity,
            reserve_active_required(0),
        )
    });
    assert!(overflowed);
    assert!(!retry);
    assert_eq!(used, 0);
    assert_eq!(actual, expected);
    assert_eq!(identity.as_str(), crate::prelude::VEC_ID);
    assert_eq!(identity.owned_allocation_bytes(), 0);
    let ((), observed) = measure(|| {
        for _ in 0..64 {
            drop(identity.clone());
        }
    });
    assert_eq!(observed.allocations, 0);
    let legacy = DeclarationId::new(crate::prelude::VEC_ID);
    assert_eq!(identity, legacy);
    assert_eq!(
        crate::cache_codec::encode(&identity).unwrap(),
        crate::cache_codec::encode(&legacy).unwrap()
    );
}

#[test]
fn borrowed_graph_keys_match_old_owned_lookup_without_temporary_payload() {
    let mut actual = [
        DeclarationId::new(crate::prelude::VEC_ID),
        DeclarationId::new(crate::prelude::BOX_ID),
        DeclarationId::new("app.λ\0id"),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut old = actual.clone();
    for key in [
        crate::prelude::VEC_ID,
        "app.λ\0id",
        "foreign",
        crate::prelude::BOX_ID,
    ] {
        let expected = old.remove(&DeclarationId::new(key));
        let ((removed, observed), overflowed, used) =
            with_limit_usage(0, || measure(|| actual.remove(key)));
        assert_eq!(removed, expected);
        assert!(!overflowed);
        assert_eq!(used, 0);
        assert_eq!(observed.allocations, 0);
        assert_eq!(actual, old);
    }
    let graph = include_str!("../../graph.rs");
    let render = graph
        .split("fn render_graph_json(")
        .nth(1)
        .unwrap()
        .split("let functions")
        .next()
        .unwrap();
    assert!(!render.contains("remove(&DeclarationId::new("));
}
