use super::super::primitive::records::preflight;
use super::super::{capture, legacy, pure_records_metered, Inventory};
use super::{pure, Authored, Directory, RecordProof};
use crate::hir::{OwnershipMode, ResolvedType};
use crate::workspace_graph::{index_authored, AuthoredDeclaration, AuthoredKind, WorkspaceSource};
use std::collections::BTreeMap;

#[test]
fn optional_source_hash_work_keeps_exact_prices_and_declines_every_short_budget() {
    let path = "provider.spx";
    let module = "scalar.leaf";
    let source = "é\0";
    // Three length prefixes, the full domain and UTF-8 bytes are all paid.
    let units = 24
        + b"semaprax.retained-primitive-source.v1\0".len()
        + path.len()
        + module.len()
        + source.len();
    let ((), metrics) = crate::test_allocation_observer::measure(|| {
        for available in 0..units {
            let mut work = available;
            assert!(
                !super::super::pay_source_revision_work(path, module, source, &mut work).unwrap()
            );
            assert_eq!(work, available);
        }
        for spare in [0, 1, 1024] {
            let mut work = units + spare;
            assert!(
                super::super::pay_source_revision_work(path, module, source, &mut work).unwrap()
            );
            assert_eq!(work, spare);
        }
    });
    assert_eq!(metrics, crate::test_allocation_observer::Metrics::default());
}

fn source(count: usize) -> String {
    let mut text = String::from("module helpers;");
    for n in (0..count).rev() {
        text.push_str(&format!(
            "@id(\"helpers.f{n:04}\") fn f{n:04}(x:i64)->i64 {{x+1}}"
        ));
    }
    text.push_str("@id(\"helpers.main\") fn main()->i64 {f0000(41)}");
    text
}
fn programs(text: &str) -> Vec<crate::ast::Program> {
    vec![crate::check(text, "helpers.spx").unwrap(), crate::parse(
        "module caller; use function @id(\"helpers.f0000\") from helpers as helper; @id(\"caller.main\") fn main()->i64 {0}",
        std::path::Path::new("caller.spx")).unwrap()]
}
fn equal(a: &Inventory, b: &Inventory) {
    assert_eq!(a.revision, b.revision);
    assert_eq!(a.functions.len(), b.functions.len());
    for (a, b) in a.functions.iter().zip(&b.functions) {
        assert_eq!(
            (&a.id, a.explicit, &a.name, &a.result, a.digest),
            (&b.id, b.explicit, &b.name, &b.result, b.digest)
        );
        assert_eq!(a.params.len(), b.params.len());
        for (a, b) in a.params.iter().zip(&b.params) {
            assert_eq!(
                (&a.id, &a.name, &a.ty, a.ownership),
                (&b.id, &b.name, &b.ty, b.ownership)
            );
        }
    }
}
#[test]
fn old_capture_and_single_classification_keep_exact_inventory_and_digests() {
    for count in [1, 8, 64] {
        let text = source(count);
        let programs = programs(&text);
        let resolved = crate::hir::resolve(&programs[0]).unwrap();
        let authored = index_authored(&programs).unwrap();
        let sources = [WorkspaceSource {
            path: "helpers.spx".into(),
            source: text,
        }];
        let ((new, old), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            (
                capture(
                    &programs[0],
                    &resolved,
                    &programs,
                    &authored,
                    Some(&sources),
                )
                .unwrap()
                .unwrap(),
                legacy::capture(
                    &programs[0],
                    &resolved,
                    &programs,
                    &authored,
                    Some(&sources),
                )
                .unwrap()
                .unwrap(),
            )
        });
        assert!(!refused);
        equal(&new, &old);
        if count == 64 {
            assert!(new.capture_work < old.capture_work);
        }
    }
    // Dense checked bodies and zero-literal forwarding both retain original facts.
    for dense in [true, false] {
        let body = if dense {
            std::iter::repeat_n("x", 32).collect::<Vec<_>>().join("+")
        } else {
            "x".into()
        };
        let text = source(8).replace("x+1", &body);
        let programs = programs(&text);
        let resolved = crate::hir::resolve(&programs[0]).unwrap();
        let authored = index_authored(&programs).unwrap();
        let sources = [WorkspaceSource {
            path: "helpers.spx".into(),
            source: text,
        }];
        let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            let new = capture(
                &programs[0],
                &resolved,
                &programs,
                &authored,
                Some(&sources),
            )
            .unwrap()
            .unwrap();
            let old = legacy::capture(
                &programs[0],
                &resolved,
                &programs,
                &authored,
                Some(&sources),
            )
            .unwrap()
            .unwrap();
            equal(&new, &old);
        });
        assert!(!refused);
    }
}
#[test]
fn wide_directory_classifies_once_with_exact_work_backing_and_allocation_free_drop() {
    let programs = programs(&source(256));
    let resolved = crate::hir::resolve(&programs[0]).unwrap();
    let mut calls = 0;
    let ((units, bytes), refused, used) =
        crate::bounded_output::with_limit_usage(usize::MAX, || {
            let mut work = usize::MAX;
            let directory = Directory::classify(&programs[0], &resolved, &mut work, &mut |f, w| {
                calls += 1;
                pure(f, w)
            })
            .unwrap();
            assert_eq!(directory.count(), programs[0].functions.len());
            for (n, f) in programs[0].functions.iter().enumerate() {
                assert_eq!(directory.selected(n).unwrap().id.as_str(), f.stable_id);
            }
            let bytes = std::mem::size_of::<super::Index<'_>>()
                + resolved.functions.len() * std::mem::size_of::<&crate::hir::ResolvedFunction>()
                + std::mem::size_of::<Directory<'_>>()
                + programs[0].functions.len()
                    * std::mem::size_of::<Option<&crate::hir::ResolvedFunction>>();
            drop(directory);
            (usize::MAX - work, bytes)
        });
    assert!(!refused);
    assert_eq!(calls, programs[0].functions.len());
    assert_eq!(used, bytes);
    let ((), refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let mut work = units;
            drop(Directory::new(&programs[0], &resolved, &mut work).unwrap());
        });
        assert_eq!(observed.allocations, 2);
        assert_eq!(
            observed.allocated,
            bytes - std::mem::size_of::<super::Index<'_>>() - std::mem::size_of::<Directory<'_>>()
        );
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    });
    assert!(!refused);
    let ((), refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = units;
        let directory = Directory::new(&programs[0], &resolved, &mut work).unwrap();
        let ((), observed) = crate::test_allocation_observer::measure(|| drop(directory));
        assert_eq!(observed.allocated, 0);
    });
    assert!(!refused);
    let ((), refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = units;
        drop(Directory::new(&programs[0], &resolved, &mut work).unwrap());
        assert_eq!(work, 0);
    });
    assert!(!refused);
    assert_eq!(used, bytes);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(bytes - 1, || {
        let mut work = units;
        let result = Directory::new(&programs[0], &resolved, &mut work);
        assert_eq!(work, 0);
        result.is_err()
    });
    assert!(short && refused);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = units - 1;
        let result = Directory::new(&programs[0], &resolved, &mut work);
        assert_eq!(work, 0);
        result.is_err()
    });
    assert!(short && !refused);
    // Drop never refunds sticky cumulative backing; a second directory still pays.
    let (short, refused, _) = crate::bounded_output::with_limit_usage(2 * bytes - 1, || {
        let mut work = usize::MAX;
        drop(Directory::new(&programs[0], &resolved, &mut work).unwrap());
        Directory::new(&programs[0], &resolved, &mut work).is_err()
    });
    assert!(short && refused);
    authored_lookup_controls(&programs[0]);
}
/// Seven literal equal-length identities, so every comparison costs 4 + 1 units
/// and a perfect binary search probes the middle once, the quarters twice and
/// every leaf or equal-length miss three times. Costs are literals, not replays.
fn authored_lookup_controls(program: &crate::ast::Program) {
    const IDS: [&str; 7] = ["id.0", "id.1", "id.2", "id.3", "id.4", "id.5", "id.6"];
    const PROBES: [usize; 7] = [3, 2, 3, 1, 3, 2, 3];
    const PROBE: usize = 1 + 4 + 1;
    const BUILD: usize = 2 * 7 + 6 * (4 + 1);
    let map: BTreeMap<&str, AuthoredDeclaration<'_>> = IDS
        .iter()
        .zip(&program.functions)
        .map(|(id, function)| {
            (
                *id,
                AuthoredDeclaration {
                    path: &program.path,
                    module: &program.module,
                    explicit: true,
                    kind: AuthoredKind::Function,
                    function: Some(function),
                    ty: None,
                },
            )
        })
        .collect();
    assert_eq!(map.len(), IDS.len());
    let entries = IDS.len() * std::mem::size_of::<(&str, &AuthoredDeclaration<'_>)>();
    let bytes = std::mem::size_of::<Authored<'_, '_>>() + entries;
    let ((), refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = BUILD;
        let lookup = Authored::new(&map, &mut work).unwrap();
        assert_eq!(work, 0);
        for (n, id) in IDS.iter().enumerate() {
            let cost = PROBES[n] * PROBE;
            let mut work = cost;
            let origin = lookup.find(id, &mut work).unwrap();
            assert_eq!(work, 0);
            // The map's own borrowed declaration and AST function, never a copy.
            assert!(std::ptr::eq(origin, &map[id]));
            assert!(origin
                .function
                .is_some_and(|f| std::ptr::eq(f, &program.functions[n])));
            let mut work = cost - 1;
            assert!(lookup.find(id, &mut work).is_err());
            assert_eq!(work, 0);
        }
        // Equal-length misses above and below every key refuse after exactly
        // three probes; one unit short refuses by exhaustion with work zeroed.
        for missing in ["id.7", "id.-", "id/0", "hd.3"] {
            let mut work = 3 * PROBE + 1;
            assert!(lookup.find(missing, &mut work).is_err());
            assert_eq!(work, 1);
            let mut work = 3 * PROBE - 1;
            assert!(lookup.find(missing, &mut work).is_err());
            assert_eq!(work, 0);
        }
    });
    assert!(!refused);
    assert_eq!(used, bytes);
    let ((), refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let mut work = BUILD;
            drop(Authored::new(&map, &mut work).unwrap());
        });
        assert_eq!(observed.allocations, 1);
        assert_eq!(observed.allocated, entries);
        assert_eq!(observed.live, 0);
        assert!(!observed.invalid);
    });
    assert!(!refused);
    let ((), refused, _) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = BUILD;
        let lookup = Authored::new(&map, &mut work).unwrap();
        let ((), observed) = crate::test_allocation_observer::measure(|| drop(lookup));
        assert_eq!(observed.allocations, 0);
        assert_eq!(observed.allocated, 0);
    });
    assert!(!refused);
    let (short, refused, _) = crate::bounded_output::with_limit_usage(bytes - 1, || {
        let mut work = BUILD;
        let result = Authored::new(&map, &mut work);
        assert_eq!(work, 0);
        result.is_err()
    });
    assert!(short && refused);
    // Every comparison is precharged: short work refuses before any reservation.
    let (short, refused, used) = crate::bounded_output::with_limit_usage(bytes, || {
        let mut work = BUILD - 1;
        let result = Authored::new(&map, &mut work);
        assert_eq!(work, 0);
        result.is_err()
    });
    assert!(short && !refused);
    assert_eq!(used, 0);
    // Drop never refunds sticky cumulative backing.
    let (short, refused, _) = crate::bounded_output::with_limit_usage(2 * bytes - 1, || {
        let mut work = usize::MAX;
        drop(Authored::new(&map, &mut work).unwrap());
        Authored::new(&map, &mut work).is_err()
    });
    assert!(short && refused);
}
fn rows(count: usize) -> String {
    let mut text = String::from(
        "module rows;\n@id(\"rows.Row\") record Row {@id(\"rows.Row.n\") n:i64,@id(\"rows.Row.text\") text:string,}\n",
    );
    for n in 0..count {
        text.push_str(&format!(
            "@id(\"rows.r{n}\") fn r{n}(row:borrow Row)->i64 {{row.n}}\n"
        ));
    }
    text.push_str("@id(\"rows.main\") fn main()->i64 {0}\n");
    text
}
#[test]
fn record_layout_proof_runs_once_per_exact_binding_with_hostile_and_drop_controls() {
    let ast = crate::check(&rows(4), "rows.spx").unwrap();
    let resolved = crate::hir::resolve(&ast).unwrap();
    // The independent original preflight fixes the one-proof cost.
    let mut alone = usize::MAX;
    assert!(preflight(&ast, &resolved, &mut alone).unwrap());
    let once = usize::MAX - alone;
    assert!(once > 0);
    let mut proof = RecordProof::new(&ast, &resolved);
    let mut work = once;
    assert!(proof.admitted(&ast, &resolved, &mut work).unwrap());
    assert_eq!(work, 0);
    assert!(proof.admitted(&ast, &resolved, &mut work).unwrap());
    // A short proof refuses with work zeroed and is never cached as admitted.
    let mut short = RecordProof::new(&ast, &resolved);
    let mut work = once - 1;
    assert!(short.admitted(&ast, &resolved, &mut work).is_err());
    assert_eq!(work, 0);
    let mut work = once;
    assert!(short.admitted(&ast, &resolved, &mut work).unwrap());
    assert_eq!(work, 0);
    // Equal clones and drifted layouts are other bindings, refused even after
    // the bound proof was admitted; the original preflight refuses the drift.
    let cloned = ast.clone();
    let mut drifted = resolved.clone();
    drifted.types[0].name.push('x');
    for (program, current) in [(&cloned, &resolved), (&ast, &drifted)] {
        let mut work = usize::MAX;
        assert!(proof.admitted(program, current, &mut work).is_err());
        assert_eq!(work, 0);
    }
    assert!(preflight(&ast, &drifted, &mut { usize::MAX }).is_err());
    // The proof owns no scratch: no backing, no allocation, allocation-free drop.
    let ((), refused, used) = crate::bounded_output::with_limit_usage(0, || {
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            let mut proof = RecordProof::new(&ast, &resolved);
            let mut work = once;
            assert!(proof.admitted(&ast, &resolved, &mut work).unwrap());
            #[allow(
                clippy::drop_non_drop,
                reason = "End this borrowed proof at the measured lifetime boundary."
            )]
            drop(proof);
        });
        assert_eq!((observed.allocations, observed.live), (0, 0));
        assert!(!observed.invalid);
    });
    assert!(!refused);
    assert_eq!(used, 0);
    // Directory sharing: the former per-function preflight classifier selects the
    // same functions and pays exactly three more proofs for four record helpers.
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let mut old = usize::MAX;
        let previous = Directory::classify(&ast, &resolved, &mut old, &mut |function, work| {
            if crate::hir::borrowed_scalar::signature(function) {
                return pure(function, work);
            }
            if !preflight(&ast, &resolved, work)? {
                return Ok(false);
            }
            pure_records_metered(function, &resolved.types, work, &mut |_| Ok(()))
        })
        .unwrap();
        let mut new = usize::MAX;
        let shared = Directory::new(&ast, &resolved, &mut new).unwrap();
        assert_eq!(shared.count(), ast.functions.len());
        assert_eq!(previous.count(), shared.count());
        for n in 0..ast.functions.len() {
            assert!(std::ptr::eq(
                previous.selected(n).unwrap(),
                shared.selected(n).unwrap()
            ));
        }
        assert_eq!(new - old, 3 * once);
    });
    assert!(!refused);
}
#[test]
fn duplicate_origin_signature_mode_and_body_drift_do_not_gain_capture_authority() {
    let text = source(8);
    let programs = programs(&text);
    let authored = index_authored(&programs).unwrap();
    let resolved = crate::hir::resolve(&programs[0]).unwrap();
    let sources = [WorkspaceSource {
        path: "helpers.spx".into(),
        source: text,
    }];
    for mutation in 0..4 {
        let mut changed = resolved.clone();
        match mutation {
            0 => changed.functions.push(changed.functions[0].clone()),
            1 => changed.functions[0].name.push('x'),
            2 => changed.functions[0].params[0].ownership = OwnershipMode::Borrow,
            3 => changed.functions[0].params[0].ty = ResolvedType::Bool,
            _ => unreachable!(),
        }
        let (bad, _, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
            match capture(&programs[0], &changed, &programs, &authored, Some(&sources)) {
                Err(_) | Ok(None) => true,
                Ok(Some(inventory)) => !inventory
                    .functions
                    .iter()
                    .any(|fact| fact.id == changed.functions[0].id),
            }
        });
        assert!(bad);
    }
    let cloned = programs[0].clone();
    let (bad, _, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        capture(&cloned, &resolved, &programs, &authored, Some(&sources)).is_err()
    });
    assert!(bad);
    let (inventory, _, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        capture(
            &programs[0],
            &resolved,
            &programs,
            &authored,
            Some(&sources),
        )
        .unwrap()
        .unwrap()
    });
    let mut changed = resolved.clone();
    changed.functions[0].body = changed.functions[1].body.clone();
    assert!(!super::super::matches_metered(
        &inventory,
        &inventory.functions[0],
        &changed.functions[0],
        &mut |_| Some(())
    ));
}

#[test]
fn real_utf8_provider_matches_original_capture_without_literal_inventory() {
    let text = format!(
        "{}\n@id(\"helper.entry\") fn main()->i64 {{0}}",
        include_str!("../../../../../std/data-json-utf8/src/utf8.spx")
    );
    let provider = crate::check(&text, "utf8.spx").unwrap();
    let caller = crate::parse("module helper.caller; use function @id(\"std.data.json.utf8.utf8_end\") from std.data.json.utf8 as end; @id(\"helper.caller.main\") fn main()->i64 {0}",std::path::Path::new("caller.spx")).unwrap();
    let programs = [provider, caller];
    let resolved = crate::hir::resolve(&programs[0]).unwrap();
    let authored = index_authored(&programs).unwrap();
    let sources = [WorkspaceSource {
        path: "utf8.spx".into(),
        source: text,
    }];
    let ((), refused, _) = crate::bounded_output::with_limit_usage(usize::MAX, || {
        let new = capture(
            &programs[0],
            &resolved,
            &programs,
            &authored,
            Some(&sources),
        )
        .unwrap()
        .unwrap();
        let old = legacy::capture(
            &programs[0],
            &resolved,
            &programs,
            &authored,
            Some(&sources),
        )
        .unwrap()
        .unwrap();
        equal(&new, &old);
    });
    assert!(!refused);
}

#[test]
fn optional_source_work_selection_preserves_the_complete_prior_capture_body() {
    let current = include_str!("../../scalar_helpers.rs");
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/optional-helper-source-work-v1/scalar_helpers.before.rs.txt"
    ));
    let helper = "fn pay_source_revision_work(\n    path: &str,\n    module: &str,\n    source: &str,\n    work: &mut usize,\n) -> Result<bool, Vec<Diagnostic>> {\n    let units = source\n        .len()\n        .checked_add(path.len())\n        .and_then(|n| n.checked_add(module.len()))\n        .and_then(|n| n.checked_add(24 + b\"semaprax.retained-primitive-source.v1\\0\".len()))\n        .ok_or_else(fail)?;\n    let Some(remaining) = work.checked_sub(units) else {\n        return Ok(false);\n    };\n    *work = remaining;\n    Ok(true)\n}\n";
    let guard = "    // Capture is optional; a source that cannot fit the hash work retains no\n    // proof. A selected receiver still requires and independently replays the\n    // complete inventory. Decline before allocating scratch or hashing bytes.\n    if !pay_source_revision_work(&program.path, &program.module, &source.source, &mut work)? {\n        return Ok(None);\n    }\n";
    let debit = "    debit(\n        &mut work,\n        source\n            .source\n            .len()\n            .checked_add(program.path.len())\n            .and_then(|n| n.checked_add(program.module.len()))\n            .and_then(|n| n.checked_add(24 + b\"semaprax.retained-primitive-source.v1\\0\".len()))\n            .ok_or_else(fail)?,\n    )?;\n";
    let anchor =
        "    let revision = body_digest::source(&program.path, &program.module, &source.source);";
    let mut restored = current.to_owned();
    for addition in [helper, guard] {
        assert_eq!(restored.matches(addition).count(), 1);
        restored = restored.replacen(addition, "", 1);
    }
    assert_eq!(restored.matches(anchor).count(), 1);
    restored = restored.replacen(anchor, &format!("{debit}{anchor}"), 1);
    assert_eq!(restored, original);
}
