//! Authentic-source complete production entry: checked and authenticated
//! modules only, no forged caller arrays. The saved complete entry and the
//! current one each run on their own cumulative owner through the unchanged
//! session and final Drop; irrelevant providers and long legal IDs must leave
//! the graph's own work unchanged relative to the saved lookup and reserve no
//! member backing beyond the actual selected closure.
use super::*;

type Flow = for<'a> fn(
    &'a WorkspaceResolvedModule,
    &'a [WorkspaceResolvedModule],
    &BTreeMap<String, WorkspaceDeclarationFact>,
    &mut SelectedReplayPreparation,
) -> Result<Closure<'a>, Refused>;
/// The old algorithm on the current enlarged carrier: its totals include
/// `oracle::carrier_delta` over the committed 296 carrier.
fn saved<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
) -> Result<Closure<'a>, Refused> {
    // The complete production entry authenticates current membership before
    // either algorithm. Execute and charge that shared prelude here too, on
    // the same owner; the full saved receiver below remains untouched.
    let anchor = module.functions.first().ok_or(Refused::MISMATCH)?;
    super::super::super::current_module_receipt(module, modules, resources, anchor)?;
    oracle::authenticate(module, modules, facts, resources).map(|(closure, _)| closure)
}
const SAVED: Flow = saved;
const CURRENT: Flow = authenticate;

struct Outcome {
    work: usize,
    bytes: usize,
    helpers: Vec<*const ResolvedFunction>,
    lexical: Vec<*const ResolvedFunction>,
    count: usize,
    walks: usize,
    edges: usize,
}
/// One owner from preparation through session validation to final Drop.
fn entry(
    build: &WorkspaceGraphBuild,
    flow: Flow,
    resources: impl FnOnce(&ResolvedFunction) -> SelectedReplayPreparation,
    work: usize,
) -> Result<Outcome, Vec<Diagnostic>> {
    let module = caller(build);
    let anchor = &module.functions[0];
    let mut resources = resources(anchor);
    let context = flow(
        module,
        &build.hir.modules,
        &build.hir.declarations,
        &mut resources,
    )
    .map_err(legacy)?;
    let mut session =
        SelectedReplaySession::from_preparation(&context, anchor, resources).map_err(|_| fail())?;
    for helper in &context.helpers {
        session
            .validate_scalar(helper.function)
            .map_err(|_| fail())?;
    }
    if context.lexical.is_empty() {
        return Err(fail());
    }
    for function in &context.lexical {
        session.validate(function).map_err(|_| fail())?;
    }
    let (remaining, bytes) = session.metrics();
    drop(session);
    let outcome = Outcome {
        work: work.saturating_sub(remaining),
        bytes,
        helpers: context
            .helpers
            .iter()
            .map(|m| m.function as *const ResolvedFunction)
            .collect(),
        lexical: context
            .lexical
            .iter()
            .map(|f| *f as *const ResolvedFunction)
            .collect(),
        count: context.count,
        walks: context.walks,
        edges: context.edges,
    };
    drop(context);
    Ok(outcome)
}
fn limited(
    build: &WorkspaceGraphBuild,
    flow: Flow,
    work: usize,
    bytes: usize,
) -> Result<Outcome, Vec<Diagnostic>> {
    entry(
        build,
        flow,
        |anchor| SelectedReplayPreparation::with_limits(anchor, work, bytes).unwrap(),
        work,
    )
}
/// The unchanged production physical default and production work ceiling.
fn default_cap(build: &WorkspaceGraphBuild, flow: Flow) -> Result<Outcome, Vec<Diagnostic>> {
    entry(
        build,
        flow,
        |anchor| {
            SelectedReplayPreparation::new(
                anchor,
                crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES),
            )
            .unwrap()
        },
        0,
    )
}
/// Saved and current complete entries agree on verdict, exact selected
/// pointers in ordinal order, lexical members and edge count; the current
/// entry walks each selected body once where the saved one walked twice.
fn parity(build: &WorkspaceGraphBuild) -> Result<(Outcome, Outcome), Vec<&'static str>> {
    let old = limited(build, SAVED, 32_000_000, 1 << 26);
    let new = limited(build, CURRENT, 32_000_000, 1 << 26);
    match (old, new) {
        (Ok(old), Ok(new)) => {
            assert_eq!(new.helpers, old.helpers, "exact selected pointers");
            assert_eq!(new.lexical, old.lexical);
            assert_eq!(new.count, old.count);
            assert_eq!(new.edges, old.edges, "edge multiplicity");
            assert_eq!(old.walks, 2 * old.count);
            assert_eq!(new.walks, new.count, "one discovery walk per body");
            Ok((old, new))
        }
        (Err(old), Err(new)) => {
            let codes = |e: &[Diagnostic]| e.iter().map(|d| d.code).collect::<Vec<_>>();
            assert_eq!(codes(&new), codes(&old));
            Err(codes(&new))
        }
        (old, new) => panic!(
            "complete entry verdict split: saved {} current {}",
            old.is_ok(),
            new.is_ok()
        ),
    }
}

/// 116-byte identities: below every stable-ID bound the toolchain applies.
fn long(tail: &str) -> String {
    format!("{}.{tail}", "q".repeat(116 - 1 - tail.len()))
}
fn tiny(noise: usize) -> WorkspaceGraphBuild {
    let leaf_id = long("leaf");
    let caller_source = format!(
        "module scalar.app;\nuse function @id(\"{leaf_id}\") from scalar.leaf as step;\n@id(\"{}\") fn main()->i64 {{let label=str\"é\\u{{0}}\";let outcome=step(41);let mut tick=0usize;while tick<1usize {{tick=tick+1usize;return outcome;}} 0}}\n",
        long("main")
    );
    // Same-provider siblings are captured but never called.
    let mut leaf_source =
        format!("module scalar.leaf;\n@id(\"{leaf_id}\") fn step(n:i64)->i64 {{n+1}}\n");
    for j in 0..noise {
        leaf_source.push_str(&format!(
            "@id(\"{}\") fn sibling{j}(n:i64)->i64 {{n+{j}}}\n",
            long(&format!("leaf.s{j:02}"))
        ));
    }
    let mut owned = vec![
        ("caller.spx".to_owned(), caller_source),
        ("leaf.spx".to_owned(), leaf_source),
    ];
    for k in 0..noise {
        let mut source = format!("module scalar.noise{k};\n");
        for j in 0..4 {
            source.push_str(&format!(
                "@id(\"{}\") fn f{j}(n:i64)->i64 {{n+{j}}}\n",
                long(&format!("noise{k:02}.f{j}"))
            ));
        }
        owned.push((format!("noise{k}.spx"), source));
    }
    let sources = owned
        .iter()
        .map(|(p, s)| (p.as_str(), s.as_str()))
        .collect::<Vec<_>>();
    let build = build_sources(&sources).unwrap();
    for k in 0..noise {
        let name = format!("scalar.noise{k}");
        let module = build.hir.modules.iter().find(|m| m.module == name);
        assert_eq!(
            module.map(|m| m.functions.len()),
            Some(4),
            "{name} retained"
        );
    }
    build
}

#[test]
fn tiny_closures_with_many_irrelevant_providers_and_long_legal_ids_keep_saved_costs() {
    let quiet = tiny(0);
    let noisy = tiny(24);
    let mut costs = Vec::new();
    for build in [&quiet, &noisy] {
        assert_eq!(schema(build).unwrap(), "semaprax.graph.v77");
        let (old, new) = parity(build).ok().unwrap();
        assert_eq!(new.count, 2);
        assert!(old.work > 0 && new.work > 0);
        // The unchanged production defaults admit both complete entries.
        let old_default = default_cap(build, SAVED).ok().unwrap();
        let new_default = default_cap(build, CURRENT).ok().unwrap();
        assert_eq!(new_default.helpers, old_default.helpers);
        costs.push((old, new));
    }
    // Irrelevant providers reach only the unchanged fact, provider-module and
    // inventory scans plus the whole-input member bound, so they add exactly
    // the same work to both entries. The saved entry also reserved one member
    // slot per workspace function; the current members, directory, edge
    // ranges and frontier stay sized to the actual selected closure, so the
    // current backing delta is exactly the saved one less those slots.
    let delta = |noisy: usize, quiet: usize| noisy as i128 - quiet as i128;
    let (quiet_old, quiet_new) = &costs[0];
    let (noisy_old, noisy_new) = &costs[1];
    assert_eq!(
        delta(noisy_new.work, quiet_new.work),
        delta(noisy_old.work, quiet_old.work),
        "irrelevant providers add no current-only work"
    );
    let functions = |build: &WorkspaceGraphBuild| {
        build
            .hir
            .modules
            .iter()
            .map(|m| m.functions.len())
            .sum::<usize>()
    };
    let unreached = delta(functions(&noisy), functions(&quiet));
    assert_eq!(unreached, 24 * 5, "24 noise modules of 4 and 24 siblings");
    assert_eq!(
        delta(noisy_new.bytes, quiet_new.bytes),
        delta(noisy_old.bytes, quiet_old.bytes)
            - unreached * std::mem::size_of::<Member<'_>>() as i128,
        "irrelevant providers reserve no current member slots"
    );
}

/// Each published provider ID sorts before every earlier one, the saved
/// directory's worst insertion order. The current arena keeps call order as
/// ordinal order with a balanced tree, and its retained call-target lookup
/// agrees with the saved first-match scan on every workspace and absent ID.
#[test]
fn descending_external_publications_keep_order_balance_and_saved_lookup() {
    const K: usize = 24;
    let imports = (0..K)
        .map(|k| format!("use function @id(\"d{k:02}.step\") from scalar.d{k:02} as d{k:02};\n"))
        .collect::<String>();
    let sum = (0..K)
        .rev()
        .map(|k| format!("d{k:02}(0)"))
        .collect::<Vec<_>>()
        .join("+");
    let caller_source = format!(
        "module scalar.app;\n{imports}@id(\"app.main\") fn main()->i64 {{let label=str\"é\\u{{0}}\";let outcome={sum};let mut tick=0usize;while tick<1usize {{tick=tick+1usize;return outcome;}} 0}}\n"
    );
    let leaves = (0..K)
        .map(|k| {
            (
                format!("d{k:02}.spx"),
                format!(
                    "module scalar.d{k:02};\n@id(\"d{k:02}.step\") fn step(n:i64)->i64 {{n+{k}}}\n"
                ),
            )
        })
        .collect::<Vec<_>>();
    let mut sources = vec![("caller.spx", caller_source.as_str())];
    sources.extend(leaves.iter().map(|(p, s)| (p.as_str(), s.as_str())));
    let build = build_sources(&sources).unwrap();
    let (_, new) = parity(&build).ok().unwrap();
    assert_eq!(new.count, K + 1);
    let published = (0..K)
        .rev()
        .map(|k| ptr(&build, &format!("scalar.d{k:02}"), &format!("d{k:02}.step")))
        .collect::<Vec<_>>();
    assert_eq!(new.helpers, published, "discovery order, not key order");
    assert!(default_cap(&build, CURRENT).is_ok());
    assert!(default_cap(&build, SAVED).is_ok());
    let shape = super::parity(&build, "scalar.app").ok().unwrap();
    assert_eq!(shape.edges, (1..=K).map(|to| (0, to)).collect::<Vec<_>>());

    let module = caller(&build);
    let anchor = &module.functions[0];
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
    let context = authenticate(
        module,
        &build.hir.modules,
        &build.hir.declarations,
        &mut resources,
    )
    .unwrap();
    let Lookup::Directory(directory) = &context.lookup else {
        panic!("current entry retains its discovery directory");
    };
    let (arena, sorted, height) = directory.audit();
    let ordinals = module
        .functions
        .iter()
        .chain(context.helpers.iter().map(|m| m.function))
        .map(|f| f as *const ResolvedFunction)
        .collect::<Vec<_>>();
    assert_eq!(arena, ordinals, "arena slot i is ordinal i");
    // app.main, then d00.step (published last) through d23.step (first).
    assert_eq!(
        sorted,
        std::iter::once(0).chain((1..=K).rev()).collect::<Vec<_>>()
    );
    assert!(height <= avl_bound(K + 1), "height {height}");
    let helpers = context
        .helpers
        .iter()
        .map(|m| m.function)
        .collect::<Vec<_>>();
    let mut ids = build
        .hir
        .modules
        .iter()
        .flat_map(|m| m.functions.iter().map(|f| f.id.as_str().to_owned()))
        .collect::<Vec<_>>();
    ids.extend(["", "a", "app.main0", "d", "d00.step\0", "zz"].map(String::from));
    // The saved complete entry builds no directory: its session resolves by
    // the saved linear scan, an independent reference for the current one.
    let mut saved_resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
    let saved_context = saved(
        module,
        &build.hir.modules,
        &build.hir.declarations,
        &mut saved_resources,
    )
    .unwrap();
    assert!(matches!(saved_context.lookup, Lookup::Linear));
    for name in &ids {
        let id = hir::DeclarationId::new(name.as_str());
        let actual = context
            .resolve_call_target(&id, None)
            .map(|f| f as *const ResolvedFunction);
        let reference = oracle::linear(
            crate::hir::items::ItemSource::view(&module.functions),
            &helpers,
            name,
        )
        .map(|f| f as *const ResolvedFunction);
        let entry = saved_context
            .resolve_call_target(&id, None)
            .map(|f| f as *const ResolvedFunction);
        assert_eq!(actual, reference, "{name:?}");
        assert_eq!(entry, reference, "{name:?}");
    }
    drop(saved_context);
    drop(context);

    let (wide, allocation) = measured(&build, 32_000_000, 1 << 26);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    let (exact, allocation) = measured(&build, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert_eq!(allocation.live, 0);
    assert!(measured(&build, work - 1, bytes).0.is_err());
    assert!(measured(&build, work, bytes - 1).0.is_err());
}

#[test]
fn noisy_tiny_entry_is_exact_one_short_and_drops_all_backing() {
    let build = tiny(24);
    let (wide, allocation) = measured(&build, 32_000_000, 1 << 26);
    let (work, bytes) = wide.unwrap();
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    let (exact, allocation) = measured(&build, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes);
    assert_eq!(allocation.live, 0);
    assert!(measured(&build, work - 1, bytes).0.is_err());
    assert!(measured(&build, work, bytes - 1).0.is_err());
    let current = limited(&build, CURRENT, work, bytes).ok().unwrap();
    assert_eq!((current.work, current.bytes), (work, bytes));
    // The saved entry's own exact boundary is equally sharp on its owner.
    let old = limited(&build, SAVED, 32_000_000, 1 << 26).ok().unwrap();
    assert!(limited(&build, SAVED, old.work, old.bytes).is_ok());
    assert!(limited(&build, SAVED, old.work - 1, old.bytes).is_err());
    assert!(limited(&build, SAVED, old.work, old.bytes - 1).is_err());
}

/// Saved totals are the old algorithm on the current carrier, which embeds
/// the call-target lookup. Against the independently restated 296 layout the
/// carrier grew by exactly `carrier_delta`; the ledger charges that once at
/// each owner reservation, with no refund, and every saved owner drops clean.
#[test]
fn saved_entry_totals_charge_the_enlarged_carrier_once_without_refund() {
    let delta = oracle::carrier_delta();
    assert!(
        delta > 0,
        "the current carrier embeds the call-target lookup"
    );
    assert!(
        delta <= std::mem::size_of::<Lookup<'_>>(),
        "only the lookup was added"
    );

    // Identical owners differing only in the carrier they reserve open one
    // session each on the same real context; its ledger shows the exact sizes.
    let noisy = tiny(24);
    let module = caller(&noisy);
    let anchor = &module.functions[0];
    let mut wide = SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
    let context = authenticate(
        module,
        &noisy.hir.modules,
        &noisy.hir.declarations,
        &mut wide,
    )
    .unwrap();
    drop(wide);
    let carrier = |current: Option<bool>| {
        let mut owner =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
        match current {
            Some(true) => owner
                .reserve_array::<Closure<'_>>(anchor, 1, "current carrier")
                .unwrap(),
            Some(false) => owner
                .reserve_array::<oracle::SavedClosureLayout<'_>>(anchor, 1, "296 carrier")
                .unwrap(),
            None => {}
        }
        let session = SelectedReplaySession::from_preparation(&context, anchor, owner).unwrap();
        session.metrics().1
    };
    let base = carrier(None);
    assert_eq!(
        carrier(Some(true)) - base,
        std::mem::size_of::<Closure<'_>>()
    );
    assert_eq!(
        carrier(Some(false)) - base,
        std::mem::size_of::<oracle::SavedClosureLayout<'_>>()
    );
    assert_eq!(carrier(Some(true)) - carrier(Some(false)), delta);
    drop(context);

    // The saved total admits exactly with the delta; the 296-shaped total
    // that omits it refuses, as does the current total less the delta.
    let local = build_sources(&[("caller.spx", LOCAL_HELPER_CALLER), ("leaf.spx", LEAF)]).unwrap();
    for (name, build) in [("quiet", tiny(0)), ("noisy", noisy), ("local", local)] {
        let old = limited(&build, SAVED, 32_000_000, 1 << 26).ok().unwrap();
        let (exact, allocation) = crate::test_allocation_observer::measure(|| {
            limited(&build, SAVED, old.work, old.bytes).map(|o| (o.work, o.bytes))
        });
        assert_eq!(exact.ok(), Some((old.work, old.bytes)), "{name}");
        assert!(!allocation.invalid, "{name}");
        assert_eq!(
            allocation.live, 0,
            "{name}: saved owner and carrier dropped"
        );
        assert!(
            limited(&build, SAVED, old.work, old.bytes - delta).is_err(),
            "{name}"
        );
        let new = limited(&build, CURRENT, 32_000_000, 1 << 26).ok().unwrap();
        assert!(
            limited(&build, CURRENT, new.work, new.bytes - delta).is_err(),
            "{name}"
        );
    }
}

#[test]
fn authentic_chain_diamond_repeated_alias_and_wide_entries_match_the_saved_entry() {
    let wide_imports = (0..12)
        .map(|k| format!("use function @id(\"w{k}.step\") from scalar.w{k} as w{k};\n"))
        .collect::<String>();
    let sum = (0..12)
        .map(|k| format!("w{k}(0)"))
        .collect::<Vec<_>>()
        .join("+");
    let wide_caller = format!(
        "module scalar.app;\n{wide_imports}@id(\"app.main\") fn main()->i64 {{let label=str\"é\\u{{0}}\";let outcome={sum};let mut tick=0usize;while tick<1usize {{tick=tick+1usize;return outcome;}} 0}}\n"
    );
    let wide_leaves = (0..12)
        .map(|k| {
            (
                format!("w{k}.spx"),
                format!("module scalar.w{k};\n@id(\"w{k}.step\") fn step(n:i64)->i64 {{n+{k}}}\n"),
            )
        })
        .collect::<Vec<_>>();
    let mut sources = vec![("caller.spx", wide_caller.as_str())];
    sources.extend(wide_leaves.iter().map(|(p, s)| (p.as_str(), s.as_str())));
    let wide = build_sources(&sources).unwrap();
    for (name, build, count) in [
        ("chain", build(CALLER, PROVIDER, LEAF).unwrap(), 3),
        ("diamond", diamond("leaf_step(n)"), 4),
        ("repeated", repeated(), 3),
        ("aliases", aliases(), 3),
        ("wide", wide, 13),
    ] {
        let (old, new) = parity(&build).ok().unwrap_or_else(|| panic!("{name}"));
        assert_eq!(new.count, count, "{name}");
        assert!(default_cap(&build, CURRENT).is_ok(), "{name} default cap");
        assert!(
            default_cap(&build, SAVED).is_ok(),
            "{name} saved default cap"
        );
        assert!(old.work > 0, "{name}");
    }
}

/// Consumer-local helper as authenticated source, not a replaced array: the
/// admitted `static.writer` split (lexical `run()` called by `main`) carrying
/// the admitted two-module continue/return body that calls the provider.
const LOCAL_HELPER_CALLER: &str = r#"module scalar.app;
use function @id("leaf.step") from scalar.leaf as step;
@id("app.run") fn run()->i64 {
 let label=str"é\u{0}";
 let mut i=0;
 while i<2 { i=step(i); if i==1 {continue;}else{0}; 0 }
 return i;
}
@id("app.main") fn main()->i64 {run()}
"#;
#[test]
fn admitted_local_helper_entry_keeps_saved_identity_exact_work_and_drop() {
    let build = build_sources(&[("caller.spx", LOCAL_HELPER_CALLER), ("leaf.spx", LEAF)])
        .expect("authenticated local-helper fixture is admitted");
    assert_eq!(schema(&build).unwrap(), "semaprax.graph.v77");
    let module = caller(&build);
    let run = ptr(&build, "scalar.app", "app.run");
    let main = ptr(&build, "scalar.app", "app.main");
    let leaf = ptr(&build, "scalar.leaf", "leaf.step");
    assert_eq!(
        module
            .functions
            .iter()
            .map(|f| f as *const ResolvedFunction)
            .collect::<Vec<_>>(),
        [run, main]
    );
    let (old, new) = parity(&build).unwrap_or_else(|_| panic!("saved and current entries admit"));
    // Ordinals: run 0, main 1, leaf 2; run -> leaf and main -> run (local).
    assert_eq!(new.helpers, [leaf]);
    assert_eq!(new.count, 3);
    assert_eq!(new.edges, 2);
    assert!(
        new.lexical.contains(&run),
        "lexical local helper is replayed"
    );
    assert_eq!(
        (old.helpers, old.lexical),
        (new.helpers.clone(), new.lexical.clone())
    );
    // Saved-oracle graph agreement on the same authentic source.
    let shape = super::parity(&build, "scalar.app").ok().unwrap();
    assert_eq!(shape.functions, [run, main, leaf]);
    assert_eq!(shape.edges, [(0, 2), (1, 0)]);
    assert_eq!(shape.order, [1, 0, 2]);
    // Unchanged production defaults admit both complete entries.
    let old_default = default_cap(&build, SAVED).unwrap_or_else(|_| panic!("saved default cap"));
    let new_default =
        default_cap(&build, CURRENT).unwrap_or_else(|_| panic!("current default cap"));
    assert_eq!(old_default.helpers, new_default.helpers);
    assert_eq!(old_default.lexical, new_default.lexical);
    // Exact and one-short work/backing on each entry's own owner, and no
    // live backing after the current entry's final Drop.
    let (wide, allocation) = measured(&build, 32_000_000, 1 << 26);
    let (work, bytes) = wide.unwrap();
    assert_eq!((work, bytes), (new.work, new.bytes));
    assert!(!allocation.invalid && allocation.allocated <= bytes && allocation.highwater <= bytes);
    assert_eq!(allocation.live, 0);
    let (exact, allocation) = measured(&build, work, bytes);
    assert_eq!(exact.unwrap(), (work, bytes));
    assert_eq!(allocation.live, 0);
    assert!(measured(&build, work - 1, bytes).0.is_err());
    assert!(measured(&build, work, bytes - 1).0.is_err());
    assert!(limited(&build, SAVED, old.work, old.bytes).is_ok());
    assert!(limited(&build, SAVED, old.work - 1, old.bytes).is_err());
    assert!(limited(&build, SAVED, old.work, old.bytes - 1).is_err());
}

#[test]
fn stale_later_providers_keep_saved_entry_verdicts_and_sticky_owners() {
    for side_name in ["a", "b"] {
        let mut build = diamond("leaf_step(n)");
        stale(&mut build, side_name);
        assert_eq!(parity(&build).err().unwrap(), ["SPX-G410"], "{side_name}");
        for flow in [SAVED, CURRENT] {
            let module = caller(&build);
            let anchor = &module.functions[0];
            let mut resources =
                SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
            assert!(flow(
                module,
                &build.hir.modules,
                &build.hir.declarations,
                &mut resources
            )
            .is_err());
            let remaining = resources.remaining();
            assert_eq!(
                resources.charge(anchor, 0, "retry").unwrap_err().code,
                "SPX-H006"
            );
            assert_eq!(resources.remaining(), remaining);
        }
    }
}

/// The real source-authenticated receiving route consumes the scan it already
/// performed. This is the same full session, not a probe outside its owner.
#[test]
fn prepared_receiving_reuses_one_exact_current_scan_through_session_and_drop() {
    const WORK: usize = 32_000_000;
    for noise in [0, 24] {
        let build = tiny(noise);
        let module = caller(&build);
        let anchor = &module.functions[0];
        let direct = limited(&build, CURRENT, WORK, 1 << 26).unwrap();
        let (received, observed) = crate::test_allocation_observer::measure(|| {
            let preparation =
                SelectedReplayPreparation::with_limits(anchor, WORK, 1 << 26).unwrap();
            crate::workspace_graph::source_schema::static_text::primitive::with_prepared_test_context(
                module,
                &build.hir.modules,
                &build.hir.declarations,
                preparation,
                |context, preparation| {
                    let mut session = SelectedReplaySession::from_preparation(
                        context, anchor, preparation,
                    ).unwrap();
                    assert_eq!(context.function_count(), 2);
                    session.validate_scalar(context.function_at(1).unwrap()).unwrap();
                    session.validate(context.function_at(0).unwrap()).unwrap();
                    let metrics = session.metrics();
                    drop(session);
                    metrics
                },
            ).unwrap()
        });
        assert_eq!(
            WORK - received.0,
            direct.work,
            "one complete membership scan"
        );
        assert_eq!(
            received.1, direct.bytes,
            "same caller-owned backing through Drop"
        );
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}

#[test]
fn current_scan_receipt_rejects_detached_equal_clone_and_namespace_transplants() {
    let build = tiny(0);
    let module = caller(&build);
    let anchor = &module.functions[0];
    // Independently resolve the exact same fixture. Workspace modules retain
    // their real owners and deliberately do not expose a production Clone.
    let detached_build = tiny(0);
    let detached = caller(&detached_build);
    assert!(detached.functions.iter().eq(module.functions.iter()));
    assert!(!std::ptr::eq(detached, module));
    let mut duplicate_build = tiny(0);
    let mut duplicate_source = tiny(0);
    let duplicate_index = duplicate_source
        .hir
        .modules
        .iter()
        .position(|candidate| candidate.module == module.module)
        .unwrap();
    duplicate_build
        .hir
        .modules
        .push(duplicate_source.hir.modules.remove(duplicate_index));
    let duplicate = caller(&duplicate_build);
    for (subject, modules) in [
        (detached, build.hir.modules.as_slice()),
        (duplicate, duplicate_build.hir.modules.as_slice()),
        (module, &[][..]),
    ] {
        let anchor = &subject.functions[0];
        let mut preparation =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
        assert!(authenticate(subject, modules, &build.hir.declarations, &mut preparation).is_err());
        assert!(preparation
            .try_charge(anchor, 0, "sticky current-module refusal")
            .is_err());
    }
    let mut paid = SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
    let current =
        super::super::super::current_module_receipt(module, &build.hir.modules, &mut paid, anchor)
            .unwrap();
    let mut restarted =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 26).unwrap();
    assert!(authenticate_current(current, &build.hir.declarations, &mut restarted).is_err());
    assert!(restarted
        .try_charge(anchor, 0, "forbidden new membership owner")
        .is_err());
    // Equal resolved copies are valid only as a separately authenticated
    // complete immutable input. No receipt from `build` grants authority.
    let standalone = tiny(0);
    let subject = caller(&standalone);
    let mut preparation =
        SelectedReplayPreparation::with_limits(&subject.functions[0], 32_000_000, 1 << 26).unwrap();
    let context = authenticate(
        subject,
        &standalone.hir.modules,
        &standalone.hir.declarations,
        &mut preparation,
    )
    .unwrap();
    assert!(std::ptr::eq(context.local.module, subject));
    assert!(!std::ptr::eq(context.local.module, module));
    drop(context);
    drop(preparation);
}
