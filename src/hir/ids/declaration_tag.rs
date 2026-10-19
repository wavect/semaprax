//! Closed-tag candidate against the complete immediate identity implementation.
use super::{CompilerDeclaration, DeclarationId};
use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{measure, Metrics};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::mem::{align_of, size_of};
#[rustfmt::skip] // The exact old identity prefix is restored byte for byte below.
#[path = "declaration_tag/before.rs"]
pub(super) mod before;

fn all_closed() -> Vec<CompilerDeclaration> {
    use CompilerDeclaration::*;
    let mut ids = vec![
        Vec,
        Box,
        Option,
        OptionNone,
        OptionSome,
        OptionSomeValue,
        List,
        ListStep,
        Iter,
        IterStep,
        Map,
        Set,
        StdinReader,
        ByteGet,
        FunctionInvoke,
        VecField,
        LiteralFormat,
        RowLoanUnbound,
        OnceConstruct,
        OnceInvoke,
        OnceMixedConstruct,
        OnceMixedInvoke,
        OncePairConstruct,
        OncePairInvoke,
        OnceDrop,
        OnceMixedDrop,
        OncePairDrop,
        BytesDrop,
        StringTake,
        StringDrop,
        StringMapDrop,
        CollectionDrop,
        StdinReaderDrop,
        VecDrop,
    ];
    ids.extend(crate::vec_ops::ALL.into_iter().map(VecOperation));
    ids.extend(crate::box_ops::ALL.into_iter().map(BoxOperation));
    ids.extend(
        [
            crate::stdin_stream_ops::PureOp::Chunk,
            crate::stdin_stream_ops::PureOp::Eof,
        ]
        .into_iter()
        .map(StdinInspection),
    );
    ids.extend(crate::map_ops::MapOp::ALL.into_iter().map(MapOperation));
    ids.extend(crate::list_ops::ALL.into_iter().map(ListOperation));
    ids.extend(crate::iterator_ops::ALL.into_iter().map(IteratorOperation));
    ids.extend(
        [
            crate::hir::ResolvedHostCommandOperation::ArgsLen,
            crate::hir::ResolvedHostCommandOperation::ArgUtf8,
            crate::hir::ResolvedHostCommandOperation::StdinRead,
            crate::hir::ResolvedHostCommandOperation::StdinStreamOpen,
            crate::hir::ResolvedHostCommandOperation::StdinStreamNext,
            crate::hir::ResolvedHostCommandOperation::StderrWrite,
            crate::hir::ResolvedHostCommandOperation::StdoutAppend,
            crate::hir::ResolvedHostCommandOperation::StderrAppend,
            crate::hir::ResolvedHostCommandOperation::NetConnect,
            crate::hir::ResolvedHostCommandOperation::NetSend,
            crate::hir::ResolvedHostCommandOperation::NetRecv,
            crate::hir::ResolvedHostCommandOperation::NetStreamStdout,
            crate::hir::ResolvedHostCommandOperation::NetWait,
            crate::hir::ResolvedHostCommandOperation::NetClose,
            crate::hir::ResolvedHostCommandOperation::NetTlsConnect,
            crate::hir::ResolvedHostCommandOperation::NetListen,
            crate::hir::ResolvedHostCommandOperation::NetAccept,
            crate::hir::ResolvedHostCommandOperation::NetCloseListener,
            crate::hir::ResolvedHostCommandOperation::NetTlsAccept,
            crate::hir::ResolvedHostCommandOperation::HttpsGet,
            crate::hir::ResolvedHostCommandOperation::HttpsPost,
            crate::hir::ResolvedHostCommandOperation::FileRead,
            crate::hir::ResolvedHostCommandOperation::FileWriteNew,
            crate::hir::ResolvedHostCommandOperation::FileStat,
            crate::hir::ResolvedHostCommandOperation::FileList,
            crate::hir::ResolvedHostCommandOperation::FileCreateDir,
            crate::hir::ResolvedHostCommandOperation::FileRemove,
            crate::hir::ResolvedHostCommandOperation::FileWriteAtomic,
            crate::hir::ResolvedHostCommandOperation::FileWriteAtomicChecked,
            crate::hir::ResolvedHostCommandOperation::EnvLen,
            crate::hir::ResolvedHostCommandOperation::EnvNameUtf8,
            crate::hir::ResolvedHostCommandOperation::EnvValueUtf8,
            crate::hir::ResolvedHostCommandOperation::ProcessRun,
        ]
        .into_iter()
        .map(Command),
    );
    ids.extend(super::lowering_identity_tests::cases().into_iter().map(|(tag, _)| tag));
    ids
}
fn hash<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut state = DefaultHasher::new();
    value.hash(&mut state);
    state.finish()
}
fn texts() -> Vec<String> {
    let mut corpus = vec![
        "".to_owned(),
        "a".to_owned(),
        "λ.函数.🦀".to_owned(),
        "id\0suffix".to_owned(),
        "e\u{301}".to_owned(),
        "é".to_owned(),
        "segment.".repeat(512),
    ];
    corpus.extend(all_closed().into_iter().map(|id| id.text().to_owned()));
    corpus
}

#[test]
fn all_closed_tags_and_mixed_dynamic_identities_preserve_exact_old_traits_and_wire() {
    for tag in all_closed() {
        let current = DeclarationId::compiler(tag);
        let old = before::DeclarationId::compiler(tag);
        let dynamic = DeclarationId::new(tag.text());
        let old_dynamic = before::DeclarationId::new(tag.text());
        assert!(current.is_compiler_static() && old.is_compiler_static());
        assert!(!dynamic.is_compiler_static() && !old_dynamic.is_compiler_static());
        assert_eq!(current.as_str().as_bytes(), old.as_str().as_bytes());
        assert_eq!(current, dynamic);
        assert_eq!(old, old_dynamic);
        assert_eq!(hash(&current), hash(&old));
        assert_eq!(hash(&current), hash(&dynamic));
        assert_eq!(format!("{current:?}"), format!("{old:?}"));
        assert_eq!(current.to_string(), old.to_string());
        assert_eq!(current.owned_allocation_bytes(), 0);
        assert_eq!(old.owned_allocation_bytes(), 0);
        let wire = crate::cache_codec::encode(&old).unwrap();
        assert_eq!(crate::cache_codec::encode(&current).unwrap(), wire);
        let decoded = crate::cache_codec::decode::<DeclarationId>(&wire).unwrap();
        let old_decoded = crate::cache_codec::decode::<before::DeclarationId>(&wire).unwrap();
        assert!(!decoded.is_compiler_static() && !old_decoded.is_compiler_static());
        assert_eq!(decoded, current);
        assert_eq!(old_decoded, old);
    }
    let corpus = texts();
    let current = corpus
        .iter()
        .map(|s| DeclarationId::new(s.as_str()))
        .collect::<Vec<_>>();
    let old = corpus
        .iter()
        .map(|s| before::DeclarationId::new(s.as_str()))
        .collect::<Vec<_>>();
    for (i, a) in current.iter().enumerate() {
        assert_eq!(a.as_str(), old[i].as_str());
        assert_eq!(hash(a), hash(&old[i]));
        for (j, b) in current.iter().enumerate() {
            assert_eq!(a == b, old[i] == old[j]);
            assert_eq!(a.cmp(b), old[i].cmp(&old[j]));
            assert_eq!(a.partial_cmp(b), old[i].partial_cmp(&old[j]));
        }
    }
    let current_set = current.iter().cloned().collect::<BTreeSet<_>>();
    let old_set = old.iter().cloned().collect::<BTreeSet<_>>();
    assert!(current_set
        .iter()
        .map(DeclarationId::as_str)
        .eq(old_set.iter().map(before::DeclarationId::as_str)));
    let mut current_map = BTreeMap::new();
    let mut old_map = BTreeMap::new();
    for (i, text) in corpus.iter().enumerate() {
        assert_eq!(
            current_map.insert(DeclarationId::new(text.as_str()), i),
            old_map.insert(before::DeclarationId::new(text.as_str()), i)
        );
    }
    for text in &corpus {
        assert_eq!(current_map.get(text.as_str()), old_map.get(text.as_str()));
        assert_eq!(
            current_map.remove(text.as_str()),
            old_map.remove(text.as_str())
        );
    }
}

#[test]
fn static_tag_creation_clone_and_drop_allocate_nothing_and_cannot_reopen_sticky_budget() {
    for tag in all_closed() {
        let (((), metrics), overflowed, used, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(!reserve_active_required(1));
                let current = DeclarationId::compiler(tag);
                let old = before::DeclarationId::compiler(tag);
                let a = current.clone();
                let b = old.clone();
                assert_eq!(a.as_str(), b.as_str());
                drop((a, b, current, old));
                assert!(!reserve_active_required(0));
            })
        });
        assert!(!metrics.invalid);
        assert_eq!(
            (
                metrics.allocations,
                metrics.allocated,
                metrics.freed,
                metrics.live
            ),
            (0, 0, 0, 0)
        );
        assert!(overflowed);
        assert_eq!(used, 0);
        let receipt = first.unwrap();
        assert_eq!(
            (receipt.requested, receipt.remaining, receipt.floor),
            (1, 0, 0)
        );
    }
}
fn paid(
    text: &str,
    old: bool,
    cap: usize,
    floor: usize,
) -> (
    bool,
    usize,
    Option<crate::bounded_output::ReservationRefusal>,
    Metrics,
) {
    let ((success, metrics), refused, used, first) = with_limit_usage_refusal(cap, || {
        assert!(crate::bounded_output::set_active_floor(floor));
        measure(|| {
            let reserve = |bytes| {
                if reserve_active_required(bytes) {
                    Ok(())
                } else {
                    Err(())
                }
            };
            let success = if old {
                let id = before::DeclarationId::paid_borrowed(text, reserve);
                let okay = id.is_ok();
                if let Ok(id) = &id {
                    let copy = id.clone();
                    assert_eq!(id.shared_allocation_key(), copy.shared_allocation_key());
                    assert_eq!(id.as_str(), copy.as_str());
                    drop(copy);
                }
                drop(id);
                okay
            } else {
                let id = DeclarationId::paid_borrowed(text, reserve);
                let okay = id.is_ok();
                if let Ok(id) = &id {
                    let copy = id.clone();
                    assert_eq!(id.shared_allocation_key(), copy.shared_allocation_key());
                    assert_eq!(id.as_str(), copy.as_str());
                    drop(copy);
                }
                drop(id);
                okay
            };
            if !success {
                assert!(!reserve_active_required(0));
            }
            success
        })
    });
    assert_eq!(refused, !success);
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    (success, used, first, metrics)
}
#[test]
fn both_complete_producers_keep_exact_short_zero_floor_sticky_and_full_backing_drop() {
    for text in texts() {
        let bytes = DeclarationId::shared_backing_bytes(text.len()).unwrap();
        assert_eq!(
            Some(bytes),
            before::DeclarationId::shared_backing_bytes(text.len())
        );
        for floor in [0, 7] {
            for cap in [0, bytes + floor - 1, bytes + floor] {
                let current = paid(&text, false, cap, floor.min(cap));
                let old = paid(&text, true, cap, floor.min(cap));
                assert_eq!(current, old);
                assert_eq!(current.0, cap == bytes + floor);
                if current.0 {
                    assert_eq!(current.1, bytes);
                    assert_eq!(current.3.allocations, 1);
                    assert_eq!(current.3.allocated, bytes);
                } else {
                    assert_eq!(current.3.allocations, 0);
                    assert_eq!(current.3.allocated, 0);
                }
            }
        }
    }
}

#[test]
fn decoder_refuses_truncated_foreign_utf8_and_owning_limits_without_changing_wire() {
    for text in texts() {
        let wire = crate::cache_codec::encode(&before::DeclarationId::new(text.as_str())).unwrap();
        assert_eq!(
            crate::cache_codec::encode(&DeclarationId::new(text.as_str())).unwrap(),
            wire
        );
        assert!(crate::cache_codec::decode::<DeclarationId>(&wire[..wire.len() - 1]).is_err());
        assert!(
            crate::cache_codec::decode::<before::DeclarationId>(&wire[..wire.len() - 1]).is_err()
        );
        let backing = DeclarationId::shared_backing_bytes(text.len()).unwrap();
        for limit in [0, backing - 1, backing] {
            let (current, cr, cu, cf) = with_limit_usage_refusal(limit, || {
                crate::cache_codec::decode::<DeclarationId>(&wire).map(|id| id.as_str().to_owned())
            });
            let (old, or, ou, of) = with_limit_usage_refusal(limit, || {
                crate::cache_codec::decode::<before::DeclarationId>(&wire)
                    .map(|id| id.as_str().to_owned())
            });
            assert_eq!(current, old);
            assert_eq!((cr, cu, cf), (or, ou, of));
        }
    }
    let mut wire = crate::cache_codec::encode(&before::DeclarationId::new("x")).unwrap();
    *wire.last_mut().unwrap() = 0xff;
    assert_eq!(
        crate::cache_codec::decode::<DeclarationId>(&wire).unwrap_err(),
        crate::cache_codec::decode::<before::DeclarationId>(&wire).unwrap_err()
    );
}

#[test]
fn actual_target_layout_and_vector_cost_equations_are_reported_without_a_guessed_width() {
    assert_eq!(size_of::<before::DeclarationId>(), size_of::<String>());
    assert!(size_of::<DeclarationId>() <= size_of::<before::DeclarationId>());
    assert!(align_of::<DeclarationId>() <= align_of::<before::DeclarationId>());
    assert!(size_of::<Option<DeclarationId>>() <= size_of::<Option<before::DeclarationId>>());
    eprintln!("OPT891 actual_target_layout old={} current={} old_option={} current_option={} compiler_tag={}",size_of::<before::DeclarationId>(),size_of::<DeclarationId>(),size_of::<Option<before::DeclarationId>>(),size_of::<Option<DeclarationId>>(),size_of::<CompilerDeclaration>());
    // These are compiled Rust object layouts, not a calling-convention or CPU
    // measurement. Only the complete old ID has an old compiled counterpart;
    // no old aggregate layout is reconstructed from a changed ID field width.
    for (role, bytes, alignment) in [
        (
            "old.DeclarationId",
            size_of::<before::DeclarationId>(),
            align_of::<before::DeclarationId>(),
        ),
        (
            "current.DeclarationId",
            size_of::<DeclarationId>(),
            align_of::<DeclarationId>(),
        ),
        (
            "old.Option<DeclarationId>",
            size_of::<Option<before::DeclarationId>>(),
            align_of::<Option<before::DeclarationId>>(),
        ),
        (
            "current.Option<DeclarationId>",
            size_of::<Option<DeclarationId>>(),
            align_of::<Option<DeclarationId>>(),
        ),
        (
            "old.&DeclarationId",
            size_of::<&before::DeclarationId>(),
            align_of::<&before::DeclarationId>(),
        ),
        (
            "current.&DeclarationId",
            size_of::<&DeclarationId>(),
            align_of::<&DeclarationId>(),
        ),
        (
            "old.Option<&DeclarationId>",
            size_of::<Option<&before::DeclarationId>>(),
            align_of::<Option<&before::DeclarationId>>(),
        ),
        (
            "current.Option<&DeclarationId>",
            size_of::<Option<&DeclarationId>>(),
            align_of::<Option<&DeclarationId>>(),
        ),
        (
            "old.Vec<DeclarationId>",
            size_of::<Vec<before::DeclarationId>>(),
            align_of::<Vec<before::DeclarationId>>(),
        ),
        (
            "current.Vec<DeclarationId>",
            size_of::<Vec<DeclarationId>>(),
            align_of::<Vec<DeclarationId>>(),
        ),
        (
            "current.DeclarationText",
            size_of::<super::DeclarationText>(),
            align_of::<super::DeclarationText>(),
        ),
        (
            "current.CompilerDeclaration",
            size_of::<CompilerDeclaration>(),
            align_of::<CompilerDeclaration>(),
        ),
        (
            "current.Arc<str>",
            size_of::<std::sync::Arc<str>>(),
            align_of::<std::sync::Arc<str>>(),
        ),
        ("current.&str", size_of::<&str>(), align_of::<&str>()),
        ("current.String", size_of::<String>(), align_of::<String>()),
        (
            "current.ast.ExprKind",
            size_of::<crate::ast::ExprKind>(),
            align_of::<crate::ast::ExprKind>(),
        ),
        (
            "current.hir.ResolvedExprKind",
            size_of::<crate::hir::ResolvedExprKind>(),
            align_of::<crate::hir::ResolvedExprKind>(),
        ),
        (
            "current.hir.ResolvedExpr",
            size_of::<crate::hir::ResolvedExpr>(),
            align_of::<crate::hir::ResolvedExpr>(),
        ),
        (
            "current.hir.ResolvedType",
            size_of::<crate::hir::ResolvedType>(),
            align_of::<crate::hir::ResolvedType>(),
        ),
        (
            "current.cleanup_plan.CleanupSlot",
            size_of::<crate::cleanup_plan::CleanupSlot>(),
            align_of::<crate::cleanup_plan::CleanupSlot>(),
        ),
        (
            "current.cleanup_plan.CleanupPlace",
            size_of::<crate::cleanup_plan::CleanupPlace>(),
            align_of::<crate::cleanup_plan::CleanupPlace>(),
        ),
    ] {
        eprintln!("OPT891 actual_compiled_layout role={role} bytes={bytes} alignment={alignment}");
    }
    let old = before::DeclarationId::new("shared-vector-key");
    let current = DeclarationId::new("shared-vector-key");
    for count in [0, 1, 64, 1024] {
        let ((), old_metrics) = measure(|| {
            let values = (0..count).map(|_| old.clone()).collect::<Vec<_>>();
            assert!(values.iter().all(|v| v.as_str() == old.as_str()));
            drop(values);
        });
        let ((), current_metrics) = measure(|| {
            let values = (0..count).map(|_| current.clone()).collect::<Vec<_>>();
            assert!(values.iter().all(|v| v.as_str() == current.as_str()));
            drop(values);
        });
        for m in [old_metrics, current_metrics] {
            assert!(!m.invalid && m.live == 0 && m.allocated == m.freed);
            assert_eq!(m.reallocations, 0);
        }
        assert_eq!(
            old_metrics.allocated,
            count * size_of::<before::DeclarationId>()
        );
        assert_eq!(
            current_metrics.allocated,
            count * size_of::<DeclarationId>()
        );
        assert_eq!(
            old_metrics.allocated + count * size_of::<DeclarationId>(),
            current_metrics.allocated + count * size_of::<before::DeclarationId>()
        );
        eprintln!("OPT891 exact_vector_roles count={count} old={old_metrics:?} current={current_metrics:?}");
    }
}

#[test]
fn complete_old_owned_overlap_and_new_backing_have_the_same_physical_frontiers() {
    for text in ["", "λ.函数", "embedded\0id"] {
        let backing = DeclarationId::shared_backing_bytes(text.len()).unwrap();
        for cap in [0, backing - 1, backing] {
            let observe = |old: bool| {
                let input = text.to_owned();
                let (answer, refused, used, first) = with_limit_usage_refusal(cap, || {
                    measure(|| {
                        let reserve = |bytes| {
                            if reserve_active_required(bytes) {
                                Ok(())
                            } else {
                                Err(())
                            }
                        };
                        if old {
                            let id = before::DeclarationId::paid_owned(input, reserve);
                            let okay = id.is_ok();
                            if let Ok(id) = &id {
                                assert_eq!(id.as_str(), text);
                            }
                            drop(id);
                            okay
                        } else {
                            let id = DeclarationId::paid_owned(input, reserve);
                            let okay = id.is_ok();
                            if let Ok(id) = &id {
                                assert_eq!(id.as_str(), text);
                            }
                            drop(id);
                            okay
                        }
                    })
                });
                assert!(!answer.1.invalid);
                assert_eq!(answer.1.allocated, if cap == backing { backing } else { 0 });
                // The input is created before observation and released inside it;
                // compare the entire old/current metrics, including its free.
                (answer, refused, used, first)
            };
            assert_eq!(observe(false), observe(true));
        }
    }
}

// A later exact raw-site census retains this whole immediate source stage.
// Authenticate the complete current reader and complete archive before the old
// identity assertions acquire their historical view. No source bytes are edited.
fn backing_test_source_before_census(current: &str) -> &'static str {
    use sha2::{Digest, Sha256};
    let previous = include_str!("../../../docs/evidence/constructor-census-20261018/before.rs.txt");
    let current = crate::source_reflows_merge::main_e177_final_inputs::value(
        "src/hir/ids/declaration_backing_tests.rs",
        "24214f246c6d7eaaeb41d618e59bdd4959f87294b0054508280a07a003e3089f",
        current,
    );
    assert_eq!(current.len(), 25016);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        "24214f246c6d7eaaeb41d618e59bdd4959f87294b0054508280a07a003e3089f"
    );
    assert_eq!(previous.len(), 24729);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(previous.as_bytes()))
        ),
        "b1cc80cc1307bc5d27a1905ed3a9d767d6d5ce0c9e6903f05e17c0ecafa9437c"
    );
    previous
}

#[test]
fn complete_backing_source_stage_refuses_drift_before_historical_assertions() {
    let current = include_str!("declaration_backing_tests.rs");
    assert_eq!(
        backing_test_source_before_census(current),
        include_str!("../../../docs/evidence/constructor-census-20261018/before.rs.txt")
    );
    for foreign in [
        String::new(),
        format!("{current} "),
        format!("{current}{current}"),
        current[..current.len() - 1].to_owned(),
        format!("X{}", &current[1..]),
        include_str!("../../../docs/evidence/constructor-census-20261018/before.rs.txt").to_owned(),
    ] {
        assert!(std::panic::catch_unwind(|| backing_test_source_before_census(&foreign)).is_err());
    }
}

#[test]
fn complete_original_identity_prefix_codec_and_old_width_control_are_exact() {
    let whole = include_str!(
        "../../../docs/evidence/declaration-tag-identity-20261018/ids.whole-before.rs.txt"
    );
    let prefix = whole
        .split(
            "#[derive(Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]\npub struct FunctionInstanceId",
        )
        .next()
        .unwrap();
    let compiled = include_str!("declaration_tag/before.rs");
    let (compiled,codec)=compiled.split_once("\n// Complete unchanged DeclarationId cache codec; outside the identity prefix.\nuse crate::cache_codec::{Codec, Decoder, Encoder};\n").unwrap();
    let restored=compiled.replacen("use std::fmt;\n", "use std::fmt;\nuse std::fmt::Write as _;\n",1)
        .replacen("use std::sync::Arc;\n\n\nuse crate::hir::ids::compiler::CompilerDeclaration;", "use std::sync::Arc;\n\nuse super::nodes::ResolvedType;\n\nmod compiler;\npub(crate) use compiler::CompilerDeclaration;",1);
    assert_eq!(restored, prefix);
    let current_codec = include_str!("../cache_codec.rs");
    let start = current_codec
        .find("impl Codec for DeclarationId {")
        .unwrap();
    let end = start
        + current_codec[start..]
            .find("\ncodec_tuple!(FunctionInstanceId")
            .unwrap();
    assert_eq!(codec.trim(), current_codec[start..end].trim());
    let old_tests=include_str!("../../../docs/evidence/declaration-tag-identity-20261018/declaration_backing_tests.whole-before.rs.txt");
    let current_tests =
        backing_test_source_before_census(include_str!("declaration_backing_tests.rs"));
    assert_eq!(current_tests.replace("    // Preserve the complete original width/backing control on its real old producer.\n    use super::declaration_tag::before::DeclarationId;\n", ""),old_tests);
}

#[test]
fn closed_variant_corpus_covers_every_named_compiler_tag_without_static_text_admission() {
    let source = include_str!("compiler.rs");
    let body = source
        .split("pub(crate) enum CompilerDeclaration {")
        .nth(1)
        .unwrap()
        .split("\n}")
        .next()
        .unwrap();
    let named = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| line.split(['(', ',']).next().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    let actual = all_closed()
        .into_iter()
        .map(|tag| format!("{tag:?}").split('(').next().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, named);
    let source = include_str!("../ids.rs");
    assert!(source.contains("CompilerStatic(CompilerDeclaration)"));
    assert!(source.contains("DeclarationText::CompilerStatic(id) => id.text()"));
    assert!(!source.contains("CompilerStatic(&'static str)"));
}

pub(super) fn old_source(source: &str, current: &str) -> Result<String, &'static str> {
    use sha2::{Digest, Sha256};
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/declaration-tag-identity-20261018/sources.json"
    ))
    .unwrap();
    let row = proof
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)
        .ok_or("foreign source")?;
    let digest = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
    );
    if current.len() != row["after"]["length"].as_u64().unwrap() as usize
        || digest != row["after"]["sha256"]
    {
        return Err("current source mismatch");
    }
    let mut before = current.to_owned();
    for edit in row["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        if before.matches(after).count() != 1 {
            return Err("inverse multiplicity");
        }
        before = before.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    let digest = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
    );
    if before.len() != row["before"]["length"].as_u64().unwrap() as usize
        || digest != row["before"]["sha256"]
    {
        return Err("whole prior mismatch");
    }
    Ok(before)
}
#[test]
fn complete_source_inverses_keep_all_raw_children_and_refuse_foreign_stages() {
    let ids_before_lowering = super::lowering_identity_tests::prior_source(
        "src/hir/ids.rs", include_str!("../ids.rs"),
    );
    let sources=[
        ("src/hir/ids.rs",ids_before_lowering.as_str()),
        ("src/hir/ids/declaration_backing_tests.rs",backing_test_source_before_census(include_str!("declaration_backing_tests.rs"))),
        ("tests/hir_module_boundaries.rs",include_str!("../../../tests/hir_module_boundaries.rs")),
        ("tests/workspace/semantic_graph/source_inventory.rs",include_str!("../../../tests/workspace/semantic_graph/source_inventory.rs")),
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",include_str!("../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
        ("docs/DECLARATION-IDENTITY-BACKING-V1.md",include_str!("../../../docs/DECLARATION-IDENTITY-BACKING-V1.md")),
    ];
    for (source, actual_current) in sources {
        let proof: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/evidence/declaration-tag-identity-20261018/sources.json"
        ))
        .unwrap();
        let row = proof
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)
            .unwrap();
        let acquired = crate::source_reflows_merge::next_big_batch_inputs::value(
            source,
            row["after"]["sha256"].as_str().unwrap(),
            actual_current,
        );
        let current = acquired.as_str();
        let old = old_source(source, current).unwrap();
        assert_ne!(current, old);
        for foreign in [
            String::new(),
            format!("{current} "),
            format!("{current}{current}"),
            current[..current.len() - 1].to_owned(),
            old,
        ] {
            assert!(old_source(source, &foreign).is_err());
        }
        assert!(old_source("foreign/source", current).is_err());
    }
    for (source, base) in [
        (sources[2].1, "../"),
        (sources[3].1, "../../"),
        (sources[4].1, "../../../../../"),
    ] {
        for child in [
            "src/hir/ids/declaration_tag.rs",
            "src/hir/ids/declaration_tag/before.rs",
        ] {
            assert_eq!(
                source
                    .matches(&format!("include_str!(\"{base}{child}\")"))
                    .count(),
                1
            );
        }
    }
    assert_eq!(
        include_str!("../ids.rs")
            .matches("#[cfg(test)]\n#[path = \"ids/declaration_tag.rs\"]\nmod declaration_tag;")
            .count(),
        1
    );
}
