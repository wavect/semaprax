//! Complete immediate-old builder, independently compiled for comparison.
use crate::workspace_graph::*;
pub(super) fn build_owned_inner(
    mut sources: Vec<WorkspaceSource>,
    change_builder_limit: Option<usize>,
    retain_operation_programs: bool,
    mut frontend: Option<&mut crate::project::incremental::FrontendPass>,
) -> Result<(WorkspaceGraphBuild, current_source::SourceFacts), Vec<Diagnostic>> {
    let _diagnostics = diagnostics::invocation_scope();
    if sources.len() < 2 {
        return Err(vec![graph_error(
            "SPX-G170",
            "Workspace Semantic Graph requires 2..32 source files",
        )]);
    }
    if sources.len() > MAX_FILES {
        return Err(vec![limit_error("files", MAX_FILES)]);
    }
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    let mut seen_paths = BTreeSet::new();
    let mut source_bytes = 0usize;
    for source in &sources {
        workspace::validate_logical_path(&source.path).map_err(|_| {
            vec![graph_error(
                "SPX-G170",
                format!(
                    "workspace semantic source path `{}` is not canonical",
                    source.path
                ),
            )]
        })?;
        if !seen_paths.insert(source.path.clone()) {
            return Err(vec![graph_error(
                "SPX-G170",
                format!("duplicate workspace semantic source path `{}`", source.path),
            )]);
        }
        source_bytes = checked_usage(
            source_bytes,
            source.source.len(),
            "total_source_bytes",
            MAX_TOTAL_SOURCE_BYTES,
        )?;
    }
    let mut programs = Vec::with_capacity(sources.len());
    let mut native_law_paths = BTreeSet::new();
    let mut declarations = 0usize;
    let mut callables = 0usize;
    let mut calls = 0usize;
    let mut uses = 0usize;
    let mut canonical_bytes = 0usize;
    for source in &sources {
        let remaining = active_builder_limit().saturating_sub(canonical_bytes);
        if let Some((inert, canonical_len)) = native_law::inert_program(source, remaining)? {
            canonical_bytes = checked_usage(
                canonical_bytes,
                canonical_len,
                "builder_bytes",
                active_builder_limit(),
            )?;
            native_law_paths.insert(source.path.clone());
            programs.push(inert);
            continue;
        }
        let cached = frontend
            .as_deref_mut()
            .and_then(|cache| cache.lookup(&source.path, &source.source));
        let reused = cached.is_some();
        let (mut program, comments) = if let Some(program) = cached {
            (program, None)
        } else {
            let (program, comments) =
                crate::parse_with_comments(&source.source, Path::new(&source.path))
                    .map_err(|error| vec![error])?;
            if let Some(frontend) = frontend.as_deref_mut() {
                frontend.parsed(source.source.len());
            }
            (program, Some(comments))
        };
        if let Some(frontend) = frontend.as_deref() {
            crate::project::indexed_rust::bind_program(&mut program, &frontend.indexed_imports)?;
        }
        // Check source-local conformance before imported declarations become
        // synthetic stubs. A stub must never acquire local implementation
        // authority merely because it has an authenticated imported identity.
        crate::static_protocol::validate(&program).map_err(|error| vec![error])?;
        let remaining = active_builder_limit().saturating_sub(canonical_bytes);
        let canonical_len = if reused {
            source.source.len()
        } else {
            let comments = comments
                .as_ref()
                .expect("cold workspace parse retains its exact comments");
            let (canonical, overflowed) = crate::bounded_output::with_limit(remaining, || {
                format::comments::canonical_comparison(&program, comments, &source.source)
            });
            if overflowed || canonical.bytes.is_none() {
                return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
            }
            if !canonical.matches {
                return Err(vec![graph_error(
                    "SPX-G170",
                    format!(
                        "workspace semantic source `{}` is not canonical",
                        source.path
                    ),
                )
                .with_help(
                    "Run `semaprax fmt <project-directory-or-manifest>` to canonicalize project source, then retry.",
                )]);
            }
            if let Some(frontend) = frontend.as_deref_mut() {
                frontend.canonicalized();
            }
            canonical
                .bytes
                .expect("canonical byte count checked before source equality")
        };
        canonical_bytes = checked_usage(
            canonical_bytes,
            canonical_len,
            "builder_bytes",
            active_builder_limit(),
        )?;
        declarations = checked_usage(
            declarations,
            declaration_count(&program)
                .ok_or_else(|| vec![limit_error("declarations", MAX_DECLARATIONS)])?,
            "declarations",
            MAX_DECLARATIONS,
        )?;
        callables = checked_usage(
            callables,
            program.functions.len(),
            "callables",
            MAX_CALLABLES,
        )?;
        calls = checked_usage(
            calls,
            call_count(&program).ok_or_else(|| vec![limit_error("calls", MAX_CALLS)])?,
            "calls",
            MAX_CALLS,
        )?;
        uses = checked_usage(uses, program.module_uses.len(), "uses", MAX_USES)?;
        programs.push(program);
    }
    crate::static_protocol::validate_workspace(&programs).map_err(|error| vec![error])?;
    // Every module's own source is parsed, canonical-checked and
    // static-protocol validated above. Only after that does a bundled
    // dependency's unreachable surface stop being built into HIR: the
    // builder charges whole translation units, so a standard-library package
    // costs its entire declaration set however little of it a consumer can
    // reach. See `dependency_pruning` for the soundness argument.
    let pruned_dependency_functions = dependency_pruning::prune(&mut programs, &sources);
    let module_paths = index_modules(&programs)?;
    let authored = index_authored(&programs)?;
    validate_synthetic_main_id_collisions(&programs, &authored)?;
    let layouts =
        expected_projection::private_layout::registry::LayoutRegistry::new(&programs, &authored);
    import_validation::validate_uses_with_registry(
        &programs,
        &module_paths,
        &authored,
        Some(&layouts),
    )?;
    let dependency_depths = validate_dependency_dag(&programs)?;
    let retention = if frontend
        .as_deref()
        .is_some_and(|cache| cache.retains_checked_hir())
    {
        expected_projection::cached_exact::Retention::Semantic
    } else {
        expected_projection::cached_exact::Retention::Parsed
    };
    let (mut resolve_builder_bytes, checked_retention_prebound, allow_uncached_peak, initial_mode) =
        expected_projection::cached_exact::initial_with_registry(
            &programs,
            &authored,
            frontend.is_none(),
            retention,
            Some(&layouts),
        )?;
    if let Some(cache) = frontend.as_deref() {
        cache.checked_retention_prebound(checked_usage(
            checked_retention_prebound,
            layouts.receipt(),
            "builder_bytes",
            active_builder_limit(),
        )?)?;
    }
    // Each isolated attempt keeps the parent's monotonic debit.
    let mut retry_allowed = active_builder_limit() == MAX_BUILDER_BYTES
        && crate::bounded_output::active_remaining().is_none();
    let mut checkpoint = if retry_allowed {
        frontend
            .as_deref()
            .and_then(|cache| cache.checkpoint_core_attempt())
    } else {
        None
    };
    retry_allowed &= frontend.is_none() || checkpoint.is_some();
    let mut core_builder_bytes = 0usize;
    let mut fallback_mode = initial_mode;
    let core = loop {
        #[cfg(test)]
        CORE_BUILD_ATTEMPTS.with(|attempts| attempts.set(attempts.get() + 1));
        let compact_layout = (fallback_mode == 6)
            .then(|| {
                expected_projection::uncached_peak::uncached_output_layout_with_registry(
                    &programs,
                    &authored,
                    Some(&layouts),
                )
            })
            .transpose()?;
        layouts.release_scratch();
        let (core, overflowed, consumed, refusal, site) =
            crate::bounded_output::with_limit_usage_refusal_site(active_builder_limit(), || {
                #[cfg(test)]
                let _trace = crate::bounded_output::stage_debits::start(
                    resolve_builder_bytes,
                    layouts.receipt(),
                    fallback_mode,
                );
                constructor_failure::attempt::run(active_builder_limit(), || {
                    constructor_failure::attempt::reserve(layouts.receipt())?;
                    staged!(
                        CorePrebound,
                        constructor_failure::attempt::reserve(resolve_builder_bytes)
                    )?;
                    let custody = custody::SourceCustody::plan(
                        &sources,
                        &programs,
                        &pruned_dependency_functions,
                    )
                    .map_err(|error| match error {
                        custody::CustodyError::Prepaid(_) => {
                            vec![limit_error("builder_bytes", active_builder_limit())]
                        }
                        _ => vec![graph_error(
                            "SPX-G173",
                            "current parsed source custody is inconsistent",
                        )],
                    })?;
                    let core = staged!(
                        ResolvedCore,
                        build_resolved_core_current(
                            &programs,
                            &module_paths,
                            &dependency_depths,
                            &authored,
                            Some(&sources),
                            frontend.as_deref_mut(),
                            &layouts,
                            compact_layout
                                .as_ref()
                                .map(|layout| &layout.order[..programs.len()]),
                        )
                    )?;
                    Ok::<_, constructor_failure::receiving::CoreFailure>((core, custody))
                })
            });
        let (core, paid_refusal) = core.into_parts();
        core_builder_bytes = core_builder_bytes.max(consumed);
        if !overflowed {
            break core;
        }
        // Drop partial trees; retain their debit in the maximum phase receipt.
        drop(core);
        if fallback_mode
            >= if allow_uncached_peak {
                6
            } else {
                expected_projection::cached_exact::MODE
            }
            || !retry_allowed
        {
            return Err(paid_refusal.unwrap_or_else(|| {
                vec![diagnostics::live_builder_refusal_at(
                    active_builder_limit(),
                    refusal,
                    site,
                )]
            }));
        }
        // Mode six replaces an overflowing all-HIR carrier with the bounded
        // sequential filtered-output/index plus one-full-HIR peak.
        if fallback_mode == 5 {
            let (tighter, _) = expected_projection::uncached_output_peak_prebound_with_registry(
                &programs,
                &authored,
                Some(&layouts),
            )?;
            resolve_builder_bytes = tighter;
            fallback_mode = 6;
            continue;
        }
        if let (Some(cache), Some(saved)) = (frontend.as_deref_mut(), checkpoint.take()) {
            cache.rollback_core_attempt(saved);
            checkpoint = cache.checkpoint_core_attempt();
        }
        let (tighter, total) = if allow_uncached_peak {
            expected_projection::next_retention_prebound_with_uncached_peak_with_registry(
                &programs,
                &authored,
                resolve_builder_bytes,
                &mut fallback_mode,
                true,
                Some(&layouts),
            )?
        } else {
            expected_projection::cached_exact::next_with_registry(
                &programs,
                &authored,
                resolve_builder_bytes,
                &mut fallback_mode,
                retention,
                Some(&layouts),
            )?
        };
        if let Some(cache) = frontend.as_deref() {
            cache.checked_retention_prebound(checked_usage(
                total,
                layouts.receipt(),
                "builder_bytes",
                active_builder_limit(),
            )?)?;
        }
        resolve_builder_bytes = tighter;
    };
    drop(layouts);
    let (core, custody) =
        core.map_err(constructor_failure::receiving::CoreFailure::into_legacy_unfunded)?;
    let (mut modules, module_paths, dependency_depths, declaration_facts, expected_edges) = core;
    for module in &mut modules {
        module.native_law = native_law_paths.contains(&module.path);
    }
    let dependency_depth = dependency_depths.values().copied().max().unwrap_or(0);
    let resolved_cross_file_edges = expected_edges.len();
    let graph_builder_bytes = canonical_bytes.max(core_builder_bytes);
    let (change_fingerprints, operation_sidecar, change_builder_bytes, operation_builder_bytes) =
        if let Some(change_builder_limit) = change_builder_limit {
            let (result, overflowed, consumed) =
                crate::bounded_output::with_limit_usage(change_builder_limit, || {
                    if retain_operation_programs {
                        reserve_builder_structure(graph_builder_bytes)?;
                    }
                    let fingerprints = authenticated_declaration_fingerprints(
                        &programs,
                        &sources,
                        &declaration_facts,
                    )?;
                    let sidecar = retain_operation_programs
                        .then(|| build_operation_sidecar(&programs, &sources, &modules, &authored))
                        .transpose()?;
                    Ok::<_, Vec<Diagnostic>>((fingerprints, sidecar))
                });
            if overflowed {
                return Err(vec![limit_error(
                    "change_builder_bytes",
                    change_builder_limit,
                )]);
            }
            let (fingerprints, sidecar) = result?;
            (
                Some(fingerprints),
                sidecar,
                consumed,
                if retain_operation_programs {
                    consumed
                } else {
                    0
                },
            )
        } else {
            (None, None, 0, 0)
        };
    let shared_prelude_ids = prelude_binding::ids_from_facts(&programs, &declaration_facts);
    let mut build = WorkspaceGraphBuild {
        current_sources: None,
        hir: ValidatedWorkspaceHir {
            modules,
            module_paths,
            dependency_depths,
            declarations: declaration_facts,
            shared_prelude_ids,
        },
        edges: expected_edges,
        usage: WorkspaceGraphWorkUsage {
            managed_files: sources.len(),
            total_source_bytes: source_bytes,
            declarations,
            callables,
            call_sites: calls,
            uses,
            resolved_cross_file_edges,
            dependency_depth,
            builder_bytes: graph_builder_bytes,
        },
        change_fingerprints,
        change_builder_bytes,
        operation_sidecar,
        operation_builder_bytes,
    };
    // Restore before any owning cache publication, exactly as the original
    // provider/pruning receiver. Both ordinary and checked Project linking use
    // these actual parsed owners instead of reparsing an equal source later.
    pruned_dependency_functions.restore(&mut programs);
    let custody = custody.complete(sources, programs).map_err(|_| {
        vec![graph_error(
            "SPX-G173",
            "current parsed source custody is inconsistent",
        )]
    })?;
    if let Some(frontend) = frontend {
        frontend.retain_current(&custody, resolve_builder_bytes)?;
    }
    let (sources, owners) = custody.into_parts();
    build.current_sources = Some(owners);
    Ok((build, sources))
}

// The exact original terminal Vec producer, independently callable after a
// real failed attempt. The whole original Err site remains in the body above.
pub(super) fn terminal(
    paid_refusal: Option<Vec<Diagnostic>>,
    refusal: Option<crate::bounded_output::ReservationRefusal>,
    site: Option<crate::bounded_output::ReservationSite>,
) -> Vec<Diagnostic> {
    paid_refusal.unwrap_or_else(|| {
        vec![diagnostics::live_builder_refusal_at(
            active_builder_limit(),
            refusal,
            site,
        )]
    })
}
