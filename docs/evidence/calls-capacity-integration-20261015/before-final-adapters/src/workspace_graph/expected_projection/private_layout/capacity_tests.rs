//! Fresh old/current directory derivations; requested backing, not RSS.
use super::*;
use crate::bounded_output::{active_remaining, with_limit_usage_refusal};
use crate::test_allocation_observer::{Metrics, measure};
use crate::workspace_graph::{MAX_BUILDER_BYTES, index_authored};
const PROVIDER: &str = include_str!("function_provider.spx");
const CALLER: &str = include_str!("function_caller.spx");
const SINGLE_CALLER: &str = "module function_layout.app; use function @id(\"make\") from function_layout.provider as make; @id(\"app.main\") fn main()->i64 {0}";
fn programs(caller: &str, provider: &str) -> Vec<Program> {
    vec![
        crate::parse(caller, "caller.spx").unwrap(),
        crate::parse(provider, "provider.spx").unwrap(),
    ]
}
fn padded(provider: &str) -> String {
    let mut source = provider.to_owned();
    for n in 0..256 {
        use std::fmt::Write;
        writeln!(
            source,
            "@id(\"unused.{n}\") record Unused{n} {{@id(\"unused.{n}.n\") n:i64,}}"
        )
        .unwrap();
    }
    source
}
fn fixtures() -> Vec<(&'static str, Vec<Program>)> {
    let empty = "module function_layout.app; @id(\"app.main\") fn main()->i64 {0}";
    let one = "module function_layout.provider; @id(\"payload\") record Payload {@id(\"payload.text\") text:string,} @id(\"make\") fn make()->Payload {Payload {text:\"ok\"}}";
    let at = PROVIDER.find("@id(\"report\")").unwrap();
    let detail = PROVIDER[..at].replace("function_layout.provider", "function_layout.detail");
    let bridge = format!(
        "module function_layout.provider; use type @id(\"config\") from function_layout.detail as Config; use type @id(\"row\") from function_layout.detail as Row; {}",
        &PROVIDER[at..]
    );
    let imported = SINGLE_CALLER.replace(
        "@id(\"app.main\")",
        "use type @id(\"config\") from function_layout.detail as VisibleConfig; @id(\"app.main\")",
    );
    let mut imports = programs(&imported, &bridge);
    imports.push(crate::parse(&detail, "detail.spx").unwrap());
    vec![
        ("none", programs(empty, PROVIDER)),
        ("one", programs(SINGLE_CALLER, &padded(one))),
        ("sparse", programs(SINGLE_CALLER, &padded(PROVIDER))),
        ("dense", programs(CALLER, PROVIDER)),
        ("duplicates", programs(CALLER, &padded(PROVIDER))),
        ("imported_descendant", imports),
    ]
}
fn census<'a>(
    caller: &Program,
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    programs: &'a [Program],
) -> Vec<(&'a str, &'a TypeDeclaration)> {
    let mut frames = Vec::with_capacity(DEPTH);
    let mut remaining = WORK;
    let mut events = Vec::new();
    visit(
        caller,
        authored,
        programs,
        &mut frames,
        &mut remaining,
        |module, declaration, _| {
            events.push((module, declaration));
            Ok(())
        },
    )
    .unwrap();
    events
}
fn same_layout(a: &LayoutClosure<'_>, b: &LayoutClosure<'_>) {
    assert_eq!(a.entries.len(), b.entries.len());
    for (a, b) in a.entries.iter().zip(&b.entries) {
        assert_eq!(a.module, b.module);
        assert!(std::ptr::eq(a.declaration, b.declaration));
        assert_eq!(a.declaration.stable_id, b.declaration.stable_id);
        assert_eq!(a.alias, b.alias);
    }
}
fn same_diagnostics(a: &[Diagnostic], b: &[Diagnostic]) {
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        assert_eq!(a.code, b.code);
        assert_eq!(a.severity, b.severity);
        assert_eq!(a.message, b.message);
        assert_eq!(a.path, b.path);
        assert_eq!(a.span, b.span);
        assert_eq!(a.help, b.help);
    }
}
#[derive(Clone, Copy, Debug)]
struct Observation {
    ok: bool,
    capacity: usize,
    work_left: usize,
    reserved: usize,
    overflowed: bool,
    metrics: Metrics,
}
#[allow(clippy::too_many_arguments)]
fn observe<'a>(
    programs: &'a [Program],
    authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
    old: bool,
    bytes: usize,
    work: usize,
    expected: Option<&Result<LayoutClosure<'a>, Vec<Diagnostic>>>,
) -> Observation {
    let ((result, metrics), overflowed, reserved, _) = with_limit_usage_refusal(bytes, || {
        measure(|| {
            let mut remaining = work;
            let answer = if old {
                LayoutClosure::derive_with_work_before_capacity(
                    &programs[0],
                    authored,
                    programs,
                    &mut remaining,
                    None,
                )
            } else {
                LayoutClosure::derive_with_work(
                    &programs[0],
                    authored,
                    programs,
                    &mut remaining,
                    None,
                )
            };
            if let Some(expected) = expected {
                match (&answer, expected) {
                    (Ok(a), Ok(b)) => same_layout(a, b),
                    (Err(a), Err(b)) => same_diagnostics(a, b),
                    _ => panic!("complete cold layout verdict differs"),
                }
            }
            let result = (
                answer.is_ok(),
                answer.as_ref().map_or(0, |r| r.entries.capacity()),
                remaining,
            );
            drop(answer);
            result
        })
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    Observation {
        ok: result.0,
        capacity: result.1,
        work_left: result.2,
        reserved,
        overflowed,
        metrics,
    }
}
#[test]
fn callback_occurrences_bound_exact_full_identity_inserts_on_sparse_dense_duplicate_and_imported_closures()
 {
    for (name, parsed) in fixtures() {
        let authored = index_authored(&parsed).unwrap();
        if name == "none" {
            continue;
        }
        let first = census(&parsed[0], &authored, &parsed);
        let second = census(&parsed[0], &authored, &parsed);
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(&second) {
            assert_eq!(a.0, b.0);
            assert!(std::ptr::eq(a.1, b.1));
            assert_eq!(a.1.stable_id, b.1.stable_id);
        }
        let unique = first
            .iter()
            .map(|(_, d)| d.stable_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut work = WORK;
        let old = LayoutClosure::derive_with_work_before_capacity(
            &parsed[0], &authored, &parsed, &mut work, None,
        )
        .unwrap();
        let current = LayoutClosure::derive(&parsed[0], &authored, &parsed).unwrap();
        same_layout(&old, &current);
        let mut old_synthetic = parsed[0].clone();
        let mut current_synthetic = parsed[0].clone();
        old.append_types(&mut old_synthetic, &parsed[0], &parsed)
            .unwrap();
        current
            .append_types(&mut current_synthetic, &parsed[0], &parsed)
            .unwrap();
        assert_eq!(
            crate::format::canonical(&old_synthetic),
            crate::format::canonical(&current_synthetic)
        );
        assert_eq!(current.entries.len(), unique.len());
        assert!(current.entries.len() <= first.len());
        let total = parsed
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
            .unwrap();
        assert_eq!(old.entries.capacity(), total);
        assert_eq!(current.entries.capacity(), total.min(first.len()));
        if name == "duplicates" {
            assert!(first.len() > unique.len());
            assert_eq!(current.entries.capacity(), first.len());
        }
        if name == "dense" {
            assert!(first.len() >= total);
            assert_eq!(old.entries.capacity(), current.entries.capacity());
        }
        if name == "imported_descendant" {
            assert!(
                current
                    .entries
                    .iter()
                    .all(|e| e.declaration.stable_id != "config")
            );
            // Config is explicitly imported, so entry census excludes it, while
            // ownedness must still descend into its actual held declaration.
            let target = authored.get("make").unwrap();
            let ty = &target.function.unwrap().return_type;
            assert_eq!(
                current
                    .signature_owner(&parsed[0], target, ty, &authored, &parsed)
                    .unwrap(),
                Some(true)
            );
        }
    }
}
#[test]
fn actual_cold_reservation_allocation_peak_and_full_drop_are_compared_without_baseline_padding() {
    for (name, parsed) in fixtures() {
        let authored = index_authored(&parsed).unwrap();
        let mut work = WORK;
        let expected = LayoutClosure::derive_with_work_before_capacity(
            &parsed[0], &authored, &parsed, &mut work, None,
        );
        let old = observe(
            &parsed,
            &authored,
            true,
            MAX_BUILDER_BYTES,
            WORK,
            Some(&expected),
        );
        let current = observe(
            &parsed,
            &authored,
            false,
            MAX_BUILDER_BYTES,
            WORK,
            Some(&expected),
        );
        assert!(old.ok && current.ok && !old.overflowed && !current.overflowed);
        assert_eq!(old.work_left, current.work_left);
        let removed = (old.capacity - current.capacity) * std::mem::size_of::<Entry<'_>>();
        assert_eq!(old.reserved - current.reserved, removed);
        assert_eq!(old.metrics.allocated - current.metrics.allocated, removed);
        assert_eq!(old.metrics.freed - current.metrics.freed, removed);
        assert_eq!(old.metrics.allocations, current.metrics.allocations);
        assert_eq!(old.metrics.reallocations, current.metrics.reallocations);
        assert!(current.metrics.highwater <= old.metrics.highwater);
        if matches!(
            name,
            "one" | "sparse" | "duplicates" | "imported_descendant"
        ) {
            assert!(removed > 0);
        }
        // Logging is outside both the allocation observer and ACTIVE owner.
        eprintln!("#834 {name} old={old:?} current={current:?} removed_entry_bytes={removed}");
    }
}
#[test]
fn physical_and_work_exact_short_zero_frontiers_preserve_old_order_and_honest_new_acceptance() {
    let parsed = programs(SINGLE_CALLER, &padded(PROVIDER));
    let authored = index_authored(&parsed).unwrap();
    let old = observe(&parsed, &authored, true, MAX_BUILDER_BYTES, WORK, None);
    let current = observe(&parsed, &authored, false, MAX_BUILDER_BYTES, WORK, None);
    let removed = (old.capacity - current.capacity) * std::mem::size_of::<Entry<'_>>();
    assert_eq!(old.reserved - current.reserved, removed);
    assert!(removed > 0);
    for (prior, baseline) in [(true, old), (false, current)] {
        for allowance in [baseline.reserved, baseline.reserved - 1, 0] {
            let result = observe(&parsed, &authored, prior, allowance, WORK, None);
            assert_eq!(result.ok, allowance == baseline.reserved);
            assert_eq!(result.overflowed, allowance != baseline.reserved);
            assert!(result.reserved <= allowance);
        }
    }
    let same_limit_old = observe(&parsed, &authored, true, current.reserved, WORK, None);
    let same_limit_current = observe(&parsed, &authored, false, current.reserved, WORK, None);
    assert!(!same_limit_old.ok && same_limit_old.overflowed && same_limit_current.ok);
    let work = WORK - old.work_left;
    for allowance in [work, work - 1, 0] {
        let mut old_work = allowance;
        let expected = LayoutClosure::derive_with_work_before_capacity(
            &parsed[0],
            &authored,
            &parsed,
            &mut old_work,
            None,
        );
        let a = observe(
            &parsed,
            &authored,
            true,
            MAX_BUILDER_BYTES,
            allowance,
            Some(&expected),
        );
        let b = observe(
            &parsed,
            &authored,
            false,
            MAX_BUILDER_BYTES,
            allowance,
            Some(&expected),
        );
        assert_eq!(a.ok, b.ok);
        assert_eq!(a.work_left, b.work_left);
        assert_eq!(a.work_left, old_work);
    }
}
#[test]
fn malformed_complete_receivers_keep_all_six_diagnostic_fields_and_priority() {
    for attack in 0..3 {
        let mut parsed = programs(SINGLE_CALLER, &padded(PROVIDER));
        if attack == 0 || attack == 2 {
            parsed[0].module_uses[0].target_module = "foreign.module".into();
        }
        if attack == 1 || attack == 2 {
            let bad = "\n@id(\"unreachable\") fn unreachable(value:_spx_layout_18446744073709551615)->i64{0}\n";
            let caller = crate::format::canonical(&parsed[0]) + bad;
            parsed[0] = crate::parse(&caller, "caller.spx").unwrap();
        }
        let authored = index_authored(&parsed).unwrap();
        let mut work = WORK;
        let expected = LayoutClosure::derive_with_work_before_capacity(
            &parsed[0], &authored, &parsed, &mut work, None,
        );
        assert!(expected.is_err());
        let old = observe(
            &parsed,
            &authored,
            true,
            MAX_BUILDER_BYTES,
            WORK,
            Some(&expected),
        );
        let current = observe(
            &parsed,
            &authored,
            false,
            MAX_BUILDER_BYTES,
            WORK,
            Some(&expected),
        );
        assert!(!old.ok && !current.ok);
        assert_eq!(old.work_left, current.work_left);
    }
}

mod owner_controls;
mod source;
