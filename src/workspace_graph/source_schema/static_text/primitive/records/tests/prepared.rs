//! Independent owning controls for source-bound type/signature preparation.
use super::*;
use crate::cleanup_plan::SelectedReplayPreparation;
fn prepare<'a>(
    module: &'a WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    work: usize,
    bytes: usize,
) -> Result<(super::super::super::Context<'a>, usize, usize), Vec<Diagnostic>> {
    let anchor = module.functions.first().unwrap();
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, work, bytes).map_err(|e| vec![e])?;
    resources
        .reserve_array::<super::super::super::Context<'_>>(anchor, 1, "owning prepared context")
        .map_err(|e| vec![e])?;
    let context =
        super::super::super::helpers::local::authenticate(module, facts, &mut resources, anchor)
            .map_err(super::super::super::legacy)?;
    let spent = work - resources.remaining();
    let bytes = resources
        .construct(anchor, |physical, _| Ok(physical.charged()))
        .map_err(super::super::super::legacy)?;
    Ok((context, spent, bytes))
}
#[test]
fn prepared_string_vec_and_borrowed_record_signatures_keep_current_type_pointers_and_canonical_vec()
{
    // Independent byte census: Row vs Vec fails on its first byte (2),
    // one Type import probe (1), exact Row alias (4), and exact rows.row ID (9).
    let parsed = crate::parse("module meter.app; use type @id(\"rows.row\") from static.rows as Row; @id(\"meter.main\") fn main()->i64{0}", "meter.spx").unwrap();
    let named = crate::ast::Type::Named {
        name: "Row".into(),
        arguments: Vec::new(),
    };
    let nominal = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("rows.row"),
        arguments: Vec::new(),
    };
    for cost in [7usize, 16usize] {
        let mut work = cost;
        let answer = if cost == 7 {
            crate::workspace_graph::static_text::primitive::records::source_type_admitted(
                &parsed, &named, &mut work,
            )
        } else {
            crate::workspace_graph::static_text::primitive::records::source_type_matches(
                &parsed, &named, &nominal, &mut work,
            )
        };
        assert!(answer.unwrap());
        assert_eq!(work, 0);
        let mut short = cost - 1;
        let answer = if cost == 7 {
            crate::workspace_graph::static_text::primitive::records::source_type_admitted(
                &parsed, &named, &mut short,
            )
        } else {
            crate::workspace_graph::static_text::primitive::records::source_type_matches(
                &parsed, &named, &nominal, &mut short,
            )
        };
        assert!(answer.is_err());
    }
    let imported = imported_build();
    let consumer = imported
        .hir
        .modules
        .iter()
        .find(|m| m.module == "static.consumer")
        .unwrap();
    let provider = imported
        .hir
        .modules
        .iter()
        .find(|m| m.module == "static.rows")
        .unwrap();
    assert!(provider.static_text.is_none());
    assert!(provider.record_closure.is_some());
    let anchor = consumer.functions.first().unwrap();
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, usize::MAX).unwrap();
    let context = super::super::super::helpers::authenticate(
        consumer,
        &imported.hir.modules,
        &imported.hir.declarations,
        &mut resources,
    )
    .unwrap();
    // The borrowed defining record, then the context-held canonical Vec.
    assert_eq!(context.type_count(), 2);
    assert!(std::ptr::eq(
        context.type_at(0).unwrap(),
        &provider.types[0]
    ));
    assert!(!std::ptr::eq(
        context.type_at(0).unwrap(),
        &provider.types[0].clone()
    ));
    assert_canonical_vec(context.type_at(1).unwrap(), &[consumer, provider]);
    assert!(context.type_at(2).is_none());
    assert_eq!(
        context
            .declarations()
            .record_fields(&provider.types[0].id)
            .unwrap()
            .len(),
        2
    );
    let borrowed = provider
        .functions
        .iter()
        .find(|f| f.id.as_str() == "rows.read")
        .unwrap();
    assert!(provider
        .scalar_helpers
        .as_ref()
        .unwrap()
        .functions
        .iter()
        .any(|f| f.id.as_str() == "rows.read"));
    assert!(context
        .resolve_call_target(&borrowed.id, None)
        .is_some_and(|actual| std::ptr::eq(actual, borrowed)));
    assert!(context.declarations().declaration(&borrowed.id).is_some());
    let source = format!(
        "{}\n@id(\"flat.count\") fn count(values:borrow Vec<i64>)->i64 {{i64_from_usize(vec_len<i64>(values))}}\n@id(\"flat.texts\") fn texts(values:borrow Vec<string>)->i64 {{0}}\n@id(\"flat.rows\") fn rows(values:borrow Vec<Packet>)->i64 {{0}}",
        lexical_source()
    );
    let build = build_source(&source);
    let module = module(&build);
    assert!(module.static_text.as_ref().unwrap().lexical.is_some());
    let (context, _, _) = prepare(module, &build.hir.declarations, 32_000_000, usize::MAX).unwrap();
    // Exact current module pointers first; the canonical Vec follows them.
    assert_eq!(context.type_count(), module.types.len() + 1);
    for (i, current) in module.types.iter().enumerate() {
        assert!(std::ptr::eq(context.type_at(i).unwrap(), current));
        assert!(!std::ptr::eq(context.type_at(i).unwrap(), &current.clone()));
    }
    assert_canonical_vec(context.type_at(module.types.len()).unwrap(), &[module]);
    assert!(context.type_at(module.types.len() + 1).is_none());
    assert!(!module
        .types
        .iter()
        .any(|current| current.id.as_str() == crate::prelude::VEC_ID));
    let id = hir::DeclarationId::new(crate::prelude::VEC_ID);
    let declaration = context.declarations().declaration(&id).unwrap();
    assert_eq!(declaration.kind, hir::DeclarationKind::Record);
    assert_eq!(
        declaration.identity_origin,
        hir::IdentityOrigin::CompilerOwned
    );
    assert_eq!(declaration.name, "Vec");
    assert!(declaration.owner.is_none());
    let [parameter] = context.declarations().type_parameters(&id).unwrap() else {
        panic!("canonical Vec parameter");
    };
    assert_eq!(parameter.name, "T");
    assert_eq!(parameter.index, 0);
    assert_eq!(context.declarations().record_fields(&id).unwrap(), &[]);
    // The receiving classifier accepts exactly this current/index pairing.
    let vector = context.type_at(module.types.len()).unwrap();
    assert_eq!(
        vector.type_parameters.as_slice(),
        context.declarations().type_parameters(&id).unwrap()
    );
    crate::cleanup::authenticate_selected_vector(&context, &id).unwrap();
}

/// The lent Vec is the exact canonical compiler-owned declaration and is held
/// by the context, never borrowed from (or forged into) a retained module.
fn assert_canonical_vec(
    vector: &hir::ResolvedTypeDeclaration,
    modules: &[&WorkspaceResolvedModule],
) {
    assert_eq!(vector.id.as_str(), crate::prelude::VEC_ID);
    assert_eq!(vector.name, "Vec");
    assert_eq!(vector.span, crate::ast::Span::default());
    let [parameter] = vector.type_parameters.as_slice() else {
        panic!("canonical Vec parameter");
    };
    assert_eq!(
        (parameter.name.as_str(), parameter.index, parameter.span),
        ("T", 0, crate::ast::Span::default())
    );
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &vector.kind else {
        panic!("canonical Vec record");
    };
    assert!(fields.is_empty());
    for module in modules {
        assert!(!module
            .types
            .iter()
            .any(|current| std::ptr::eq(current, vector)));
    }
}
#[test]
fn prepared_current_layout_namespace_and_signature_drift_are_independent_and_sticky() {
    for attack in 0..7 {
        let mut imported = imported_build();
        let provider = imported
            .hir
            .modules
            .iter_mut()
            .find(|m| m.module == "static.rows")
            .unwrap();
        match attack {
            0 => provider.record_source_revision = Some([0; 32]),
            1 => provider.record_closure = None,
            2 => {
                provider.record_closure.as_mut().unwrap().records.records[0]
                    .declaration
                    .name = "Forged".into()
            }
            3 => {
                provider.record_closure.as_mut().unwrap().records.records[0].field_explicit[0] =
                    false
            }
            4 => {
                imported.hir.declarations.get_mut("rows.row").unwrap().path =
                    Some("stale.spx".into())
            }
            5 => {
                imported
                    .hir
                    .declarations
                    .get_mut("rows.row.n")
                    .unwrap()
                    .owner = Some("other.row".into())
            }
            6 => {
                provider
                    .scalar_helpers
                    .as_mut()
                    .unwrap()
                    .functions
                    .iter_mut()
                    .find(|f| f.id.as_str() == "rows.read")
                    .unwrap()
                    .params[0]
                    .ownership = hir::OwnershipMode::Own
            }
            _ => unreachable!(),
        }
        let consumer = imported
            .hir
            .modules
            .iter()
            .find(|m| m.module == "static.consumer")
            .unwrap();
        let anchor = consumer.functions.first().unwrap();
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, usize::MAX).unwrap();
        assert!(
            super::super::super::helpers::authenticate(
                consumer,
                &imported.hir.modules,
                &imported.hir.declarations,
                &mut resources
            )
            .is_err(),
            "imported record attack {attack}"
        );
        assert!(resources
            .charge(anchor, 0, "sticky imported record denial")
            .is_err());
    }
    let build = build_lexical();
    let source = module(&build);
    let anchor = source.functions.first().unwrap();
    let inventory = source
        .static_text
        .as_ref()
        .unwrap()
        .lexical
        .as_ref()
        .unwrap();
    for mutation in 0..6 {
        let mut facts = build.hir.declarations.clone();
        let key = if mutation < 3 {
            "flat.Packet"
        } else {
            "flat.Packet.text"
        };
        let fact = facts.get_mut(key).unwrap();
        match mutation {
            0 => fact.origin = hir::IdentityOrigin::CompilerOwned,
            1 => fact.path = Some("wrong.spx".into()),
            2 => fact.module = Some("wrong.module".into()),
            3 => fact.kind = hir::DeclarationKind::Function,
            4 => fact.owner = Some("wrong.owner".into()),
            5 => fact.origin = hir::IdentityOrigin::Automatic,
            _ => unreachable!(),
        }
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, usize::MAX).unwrap();
        let mut index = hir::DeclarationIndex::default();
        assert!(super::super::authenticate_prepared(
            source,
            inventory.records.as_deref(),
            &facts,
            &mut index,
            &mut resources,
            anchor
        )
        .is_err());
        assert!(resources.charge(anchor, 0, "later current record").is_err());
    }
    for mutation in 0..3 {
        let mut stale = build_lexical();
        let source = stale
            .hir
            .modules
            .iter_mut()
            .find(|m| m.module == "static.flat_records")
            .unwrap();
        match mutation {
            0 => source.functions[0].params[0].ownership = hir::OwnershipMode::Borrow,
            1 => source.functions[0].return_type = hir::ResolvedType::Bool,
            2 => source.functions[0].name.push_str("_drift"),
            _ => unreachable!(),
        }
        let anchor = source.functions.first().unwrap();
        let mut resources =
            SelectedReplayPreparation::with_limits(anchor, 32_000_000, usize::MAX).unwrap();
        assert!(super::super::super::helpers::local::authenticate(
            source,
            &stale.hir.declarations,
            &mut resources,
            anchor
        )
        .is_err());
        assert!(resources.charge(anchor, 0, "later signature").is_err());
    }
    let separate = build_lexical();
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, usize::MAX).unwrap();
    super::super::super::current_module(source, &build.hir.modules, &mut resources, anchor)
        .unwrap();
    assert!(super::super::super::current_module(
        source,
        &separate.hir.modules,
        &mut resources,
        anchor
    )
    .is_err());
    assert!(resources.charge(anchor, 0, "detached module").is_err());
    let mut duplicate = build_lexical();
    let another = build_lexical();
    let repeated = another
        .hir
        .modules
        .into_iter()
        .find(|m| m.module == "static.flat_records")
        .unwrap();
    duplicate.hir.modules.push(repeated);
    let repeated = module(&duplicate);
    let repeated_anchor = repeated.functions.first().unwrap();
    let mut resources =
        SelectedReplayPreparation::with_limits(repeated_anchor, 32_000_000, usize::MAX).unwrap();
    assert!(super::super::super::current_module(
        repeated,
        &duplicate.hir.modules,
        &mut resources,
        repeated_anchor
    )
    .is_err());
    let (context, _, _) = prepare(source, &build.hir.declarations, 32_000_000, usize::MAX).unwrap();
    // Attached schema is not source-signature authority; the actual borrowed
    // member universe remains independently reconstructible.
    assert!(std::ptr::eq(
        context.function_at(0).unwrap(),
        &source.functions[0]
    ));
}
#[test]
fn prepared_full_context_authentication_work_is_cumulative_exact_and_one_short() {
    let build = build_lexical();
    // Unlike the authentication-only constructor counters below, this observes
    // the actual held owner, session receivers, independent replay and Drop.
    let measure =
        |work, bytes| crate::test_allocation_observer::measure(|| full_chain(&build, work, bytes));
    let (wide, allocation) = measure(32_000_000, usize::MAX);
    let (full_work, full_bytes) = wide.unwrap();
    assert!(
        !allocation.invalid
            && allocation.allocated <= full_bytes
            && allocation.highwater <= full_bytes
    );
    assert_eq!(allocation.live, 0);
    let (exact, allocation) = measure(full_work, full_bytes);
    assert_eq!(exact.unwrap(), (full_work, full_bytes));
    assert!(
        !allocation.invalid
            && allocation.allocated <= full_bytes
            && allocation.highwater <= full_bytes
    );
    assert_eq!(allocation.live, 0);
    for (work, bytes) in [(full_work - 1, full_bytes), (full_work, full_bytes - 1)] {
        let (short, allocation) = measure(work, bytes);
        assert!(short.is_err());
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
    }
    let source = module(&build);
    let (_, spent, bytes) =
        prepare(source, &build.hir.declarations, 32_000_000, usize::MAX).unwrap();
    assert_eq!(
        prepare(source, &build.hir.declarations, spent, bytes)
            .unwrap()
            .2,
        bytes
    );
    assert!(prepare(source, &build.hir.declarations, spent, bytes - 1).is_err());
    assert!(spent > 0);
    assert_eq!(
        prepare(source, &build.hir.declarations, spent, usize::MAX)
            .unwrap()
            .1,
        spent
    );
    assert!(prepare(source, &build.hir.declarations, spent - 1, usize::MAX).is_err());
    // Two source-authentication invocations cannot obtain fresh work from one
    // held owner; the second refusal remains sticky even for a zero debit.
    let anchor = source.functions.first().unwrap();
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, spent * 2 - 1, usize::MAX).unwrap();
    resources
        .reserve_array::<super::super::super::Context<'_>>(anchor, 1, "owning prepared context")
        .unwrap();
    let first = super::super::super::helpers::local::authenticate(
        source,
        &build.hir.declarations,
        &mut resources,
        anchor,
    )
    .unwrap();
    drop(first);
    assert!(super::super::super::helpers::local::authenticate(
        source,
        &build.hir.declarations,
        &mut resources,
        anchor
    )
    .is_err());
    assert!(resources.charge(anchor, 0, "after refusal").is_err());
}

fn full_chain(
    build: &WorkspaceGraphBuild,
    work: usize,
    bytes: usize,
) -> Result<(usize, usize), ()> {
    let module = module(build);
    let anchor = module.functions.first().unwrap();
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, work, bytes).map_err(|_| ())?;
    super::super::super::current_module(module, &build.hir.modules, &mut resources, anchor)
        .map_err(|_| ())?;
    resources
        .reserve_array::<super::super::super::Context<'_>>(anchor, 1, "retained lexical context")
        .map_err(|_| ())?;
    let context = super::super::super::helpers::local::authenticate(
        module,
        &build.hir.declarations,
        &mut resources,
        anchor,
    )
    .map_err(|_| ())?;
    resources
        .reserve_array::<&hir::ResolvedFunction>(
            anchor,
            module.functions.len(),
            "retained lexical member references",
        )
        .map_err(|_| ())?;
    let mut members = Vec::with_capacity(module.functions.len());
    assert_eq!(members.capacity(), module.functions.len());
    for function in &module.functions {
        if super::super::super::helpers::local::requires(function, &mut resources, anchor)
            .map_err(|_| ())?
        {
            resources
                .charge(anchor, 1, "retained lexical member insertion")
                .map_err(|_| ())?;
            members.push(function);
        }
    }
    assert!(!members.is_empty());
    let mut session =
        crate::cleanup_plan::SelectedReplaySession::from_preparation(&context, anchor, resources)
            .map_err(|_| ())?;
    for function in members {
        session.validate(function).map_err(|_| ())?;
    }
    let (remaining, charged) = session.usage();
    let spent = work.checked_sub(remaining).unwrap();
    drop(session);
    drop(context);
    Ok((spent, charged))
}

fn imported_build() -> WorkspaceGraphBuild {
    let provider = r#"module static.rows;
@id("rows.row") record Row {@id("rows.row.n") n:i64,@id("rows.row.text") text:string,}
@id("rows.weight") fn weight(n:i64)->i64 {n+1}
@id("rows.read") fn read(row:borrow Row)->i64 {row.n}
"#;
    let consumer = r#"module static.consumer;
use type @id("rows.row") from static.rows as Row;
use function @id("rows.weight") from static.rows as weight;
use function @id("rows.read") from static.rows as reader;
@id("consumer.read") fn read(row:borrow Row)->i64 {row.n}
@id("consumer.main") fn main()->i64 {
 let key=str"x";let row=Row{n:41,text:"x"};let mut next=0;
 while next<1 {next=next+1;if str_len_bytes(key)==1 {return weight(reader(row));} else {0} 0}
 read(row)
}
"#;
    let sources = [("rows.spx", provider), ("consumer.spx", consumer)]
        .into_iter()
        .map(|(path, source)| WorkspaceSource {
            path: path.into(),
            source: format::canonical(&crate::parse(source, path).unwrap()),
        })
        .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .unwrap()
        .0
}
