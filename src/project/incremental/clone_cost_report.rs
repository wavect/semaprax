//! Deterministic, host-independent measurement of `FrontendPass::lookup`'s
//! real payload on a persistent-semantic-cache warm hit.
//!
//! `lookup` (see `super::lookup`) satisfies a whole-module cache hit with
//! `entry.program.as_ref().clone()`: a full recursive `Program` clone, the
//! same structural walk `Program`'s own iterative `Drop` performs. #85's
//! quiet-host bench found `rebuild-unchanged` at parity with `cold` (no
//! speedup within 1.4% at every tested scale); #131's prior audit named this
//! clone as the confirmed root cause at this exact call site
//! (`src/project/incremental.rs`, `FrontendPass::lookup`).
//!
//! #130/#131 then narrowed the invalidation this module measures: `build` no
//! longer marks an unrelated consumer invalidated merely because a provider
//! it imports from changed (see `build`'s own doc comment for the soundness
//! argument -- parsing one file is a pure function of that file's own bytes,
//! and every cross-module check independently reruns on the current build's
//! `Program` values regardless of cache provenance). The clone this module
//! measures still happens on every remaining cache hit; what changed is only
//! which modules count as hits when a provider, not the module itself, is
//! what changed. This is why `provider_edit_clones_unaffected_consumers_and_reparses_only_the_provider`
//! below now agrees with `local_body_edit_reparses_one_module_and_clones_the_rest`
//! instead of being its opposite -- that reversal, measured in real AST-node
//! counts, is the change #130/#131 shipped.
//!
//! This module does not change `lookup`, `build`, or any admitted result.
//! It replays the exact same cache through an ordinary two-build cold/warm
//! sequence and classifies every module in the second build as either a
//! whole-module clone (a cache hit, per the real build's own
//! `invalidated_sources`) or a reparse, then counts AST nodes on each side.
//! AST node count -- not wall-clock time or byte count -- is the metric: it
//! is the exact substructure `Program::clone`'s derived `Clone` impl
//! recursively visits, so it holds identically on a quiet host or a host
//! running fourteen concurrent `cargo` builds. Source byte count is recorded
//! alongside for comparison, since #85 already showed `modules_parsed == 0`
//! does not by itself imply cheap reuse.
//!
//! #130 asks for cold, warm, edited *and recovered* workflows measured on
//! like-for-like footing. `measure_cold_open` and `measure_recovered_open`
//! (alongside `measure_warm_open`) complete that set on this module's AST-
//! node metric: cold is a brand-new cache's first build (nothing to clone);
//! recovered is a warm cache surviving a refused build (a provider signature
//! change) and then rebuilding the original sources at the same cost as an
//! already-unchanged rebuild -- proving the refused attempt left no residue,
//! per #131's "failed rebuild leaves prior cache/service state intact."
//! `classify_next_build` is the one shared classifier all three call, so
//! cold/warm/recovered can never disagree about what counts as a hit.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::ast::Program;

use super::{ProjectFrontendCache, ProjectFrontendSource, ProjectManifest, Result};

/// One module's contribution to a [`CloneCostReport`]: how many AST nodes
/// and source bytes it carries, on whichever side (`cloned` or `reparsed`)
/// of the report it landed on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ModuleCloneCost {
    pub(crate) path: String,
    pub(crate) ast_node_count: usize,
    pub(crate) source_bytes: usize,
}

/// The classification of every module in a warm build's source set, against
/// the cache state left behind by the build immediately before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CloneCostReport {
    /// Modules the warm build satisfied from cache: `lookup` ran a full
    /// `Program::clone` on each of these.
    pub(crate) cloned: Vec<ModuleCloneCost>,
    /// Modules the warm build reparsed instead: new, changed, or removed
    /// since the baseline build (own-text invalidation only; see `build`'s
    /// doc comment in `src/project/incremental.rs` for why an unaffected
    /// consumer of a changed provider is no longer counted here).
    pub(crate) reparsed: Vec<ModuleCloneCost>,
}

impl CloneCostReport {
    pub(crate) fn total_cloned_nodes(&self) -> usize {
        self.cloned.iter().map(|module| module.ast_node_count).sum()
    }
    pub(crate) fn total_cloned_source_bytes(&self) -> usize {
        self.cloned.iter().map(|module| module.source_bytes).sum()
    }
    pub(crate) fn total_reparsed_nodes(&self) -> usize {
        self.reparsed
            .iter()
            .map(|module| module.ast_node_count)
            .sum()
    }
}

/// Deterministic count of every expression node reachable from `program`'s
/// functions (parameters carry no expressions; bodies, `requires`, and
/// `ensures` do), plus one unit per top-level declaration. This is not a
/// byte-for-byte allocator accounting -- the `String`/`Vec`/`Box` allocations
/// inside each node are not weighed individually -- but every unit counted
/// here is one heap-allocated struct node `#[derive(Clone)]` on `Program`
/// recursively visits, so it is a stable, host-independent proxy for clone
/// (and parse, and iterative-`Drop`) work. Types, interfaces, protocols, and
/// protocol implementations carry no embedded expression bodies in this
/// language's grammar, so they are counted once each as declarations and not
/// walked further. Embedded Agent operation bodies are ordinary Program.functions
/// entries and are counted exactly once by the same walk below.
pub(crate) fn program_ast_node_count(program: &Program) -> usize {
    let mut count = program.types.len()
        + program.interfaces.len()
        + program.protocols.len()
        + program.implementations.len()
        + program.agents.len();
    for function in &program.functions {
        count += 1; // the function declaration itself
        function.body.visit_all_nodes(&mut |_| count += 1);
        for clause in function.requires.iter().chain(function.ensures.iter()) {
            clause.visit_all_nodes(&mut |_| count += 1);
        }
    }
    count
}

/// Snapshot `cache`'s current entries, build `next` against it, and classify
/// every module in `next` as cloned or reparsed using the exact
/// `invalidated_sources` the real build already computes. This function
/// reads that field; it never recomputes invalidation itself, so it cannot
/// disagree with -- or bypass -- the real admission it replays. Shared by
/// every scenario below (#130's cold, warm-unchanged/edited, and recovered
/// workflows) so they all measure through one code path.
fn classify_next_build(
    cache: &mut ProjectFrontendCache,
    manifest: &ProjectManifest,
    next: &[ProjectFrontendSource],
) -> Result<CloneCostReport> {
    // Snapshot what the cache holds before this build: these are the exact
    // `Arc<Program>` values `lookup` clones if their path's source is
    // unchanged and the path is not invalidated. Empty for a cold cache.
    let baseline: BTreeMap<String, (usize, usize)> = cache
        .entries
        .iter()
        .map(|(path, entry)| {
            (
                path.clone(),
                (program_ast_node_count(&entry.program), entry.source.len()),
            )
        })
        .collect();
    let built = cache.build(manifest, next)?;
    let report = super::work_value(&built)?;
    let reset = report["manifest_context_reset"]
        .as_bool()
        .ok_or_else(|| super::invalid("frontend work report is missing manifest_context_reset"))?;
    let invalidated: BTreeSet<String> = report["invalidated_sources"]
        .as_array()
        .ok_or_else(|| super::invalid("frontend work report is missing invalidated_sources"))?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| super::invalid("invalidated_sources entry is not a path string"))
        })
        .collect::<Result<BTreeSet<String>>>()?;
    let mut cloned = Vec::new();
    let mut reparsed = Vec::new();
    for source in next {
        let path = source.path().to_owned();
        let hit = !reset && !invalidated.contains(&path) && baseline.contains_key(&path);
        if hit {
            let (ast_node_count, source_bytes) = baseline[&path];
            cloned.push(ModuleCloneCost {
                path,
                ast_node_count,
                source_bytes,
            });
        } else {
            let (program, _comments) =
                crate::parse_with_comments(source.source(), Path::new(&path))
                    .map_err(|error| vec![error])?;
            reparsed.push(ModuleCloneCost {
                ast_node_count: program_ast_node_count(&program),
                source_bytes: source.source().len(),
                path,
            });
        }
    }
    Ok(CloneCostReport { cloned, reparsed })
}

/// Cold-build `initial`, then warm-build `next` against the same cache, and
/// classify every module in `next` as cloned or reparsed. See
/// `classify_next_build` for the classification contract.
pub(crate) fn measure_warm_open(
    manifest: &ProjectManifest,
    initial: &[ProjectFrontendSource],
    next: &[ProjectFrontendSource],
) -> Result<CloneCostReport> {
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    cache.build(manifest, initial)?;
    classify_next_build(&mut cache, manifest, next)
}

/// #130's "cold" workflow: a brand-new cache's first build of `sources`.
/// There is nothing yet to clone, so every module is necessarily reparsed;
/// this is the explicit baseline the warm scenarios above are implicitly
/// compared against, measured on the identical AST-node footing rather than
/// left as an unstated setup step.
pub(crate) fn measure_cold_open(
    manifest: &ProjectManifest,
    sources: &[ProjectFrontendSource],
) -> Result<CloneCostReport> {
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    classify_next_build(&mut cache, manifest, sources)
}

/// #130's "recovered" workflow, on the same footing as the other scenarios:
/// warm-build `sources`, attempt a build of `failing` that is expected to be
/// refused by real admission, discard that outcome without inspecting it
/// (the point is that the cache must not be poisoned regardless of *why*
/// the attempt failed), then classify a rebuild of the original `sources`.
/// `ProjectFrontendCache::build` only commits `self.context`/`self.entries`/
/// `self.checked` after `build_owned_with_frontend` and the work-report
/// render both succeed (see `build` in `src/project/incremental.rs`), so a
/// refused `failing` build leaves the cache byte-for-byte as it was; this
/// function does not special-case that, it just measures the AST cost of
/// the recovery build and lets a poisoned/partially-evicted cache show up
/// as an unexpected reparse if that guarantee were ever broken.
pub(crate) fn measure_recovered_open(
    manifest: &ProjectManifest,
    sources: &[ProjectFrontendSource],
    failing: &[ProjectFrontendSource],
) -> Result<CloneCostReport> {
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    cache.build(manifest, sources)?;
    if cache.build(manifest, failing).is_ok() {
        return Err(super::invalid(
            "recovered-open fixture's failing build was unexpectedly admitted",
        ));
    }
    classify_next_build(&mut cache, manifest, sources)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calculator_project() -> (ProjectManifest, Vec<ProjectFrontendSource>) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        let manifest =
            ProjectManifest::parse(&std::fs::read_to_string(root.join("semaprax.toml")).unwrap())
                .unwrap();
        let sources = manifest
            .sources()
            .iter()
            .map(|path| {
                ProjectFrontendSource::new(
                    path.as_str(),
                    &std::fs::read_to_string(root.join(path.as_str())).unwrap(),
                )
                .unwrap()
            })
            .collect();
        (manifest, sources)
    }

    fn with_replacement(
        sources: &[ProjectFrontendSource],
        path: &str,
        from: &str,
        to: &str,
    ) -> Vec<ProjectFrontendSource> {
        sources
            .iter()
            .map(|source| {
                if source.path() == path {
                    let changed = source.source().replace(from, to);
                    let canonical =
                        crate::format::canonical(&crate::parse(&changed, path).unwrap_or_else(
                            |error| panic!("fixture edit must stay parseable: {error:?}"),
                        ));
                    ProjectFrontendSource::new(path, &canonical).unwrap()
                } else {
                    ProjectFrontendSource::new(source.path(), source.source()).unwrap()
                }
            })
            .collect()
    }

    /// The measurement is a pure function of its inputs: running it twice on
    /// identical sources produces an identical report, down to per-module
    /// node counts and classification order. This is the "byte-identical
    /// for identical input" invariant, checked both structurally and via a
    /// stable text rendering, so a future change that makes classification
    /// order depend on hash-map iteration would fail this test rather than
    /// only showing up as flaky CI.
    #[test]
    fn measurement_is_deterministic_across_repeated_runs() {
        let (manifest, sources) = calculator_project();
        let first = measure_warm_open(&manifest, &sources, &sources).unwrap();
        let second = measure_warm_open(&manifest, &sources, &sources).unwrap();
        assert_eq!(first, second);
        assert_eq!(format!("{first:?}"), format!("{second:?}"));
    }

    /// Unchanged rebuild: every module in `next` is a whole-module cache
    /// hit, so `reparsed` is empty and `total_cloned_nodes` is exactly the
    /// AST size of the whole retained project -- not a token, a real,
    /// nonzero clone volume, quantifying #85's "warm hits coexist with
    /// substantial total work" finding in units that don't need a quiet
    /// host to be trustworthy.
    #[test]
    fn unchanged_rebuild_clones_every_module_and_reparses_none() {
        let (manifest, sources) = calculator_project();
        let report = measure_warm_open(&manifest, &sources, &sources).unwrap();
        assert!(report.reparsed.is_empty());
        assert_eq!(report.cloned.len(), sources.len());
        assert!(
            report.total_cloned_nodes() > 0,
            "an unchanged warm build must still walk real AST structure per hit"
        );
        eprintln!(
            "unchanged rebuild: {} modules cloned, {} AST nodes, {} source bytes",
            report.cloned.len(),
            report.total_cloned_nodes(),
            report.total_cloned_source_bytes()
        );
    }

    /// #130's "cold" workflow, measured on the same footing as the warm
    /// scenarios around it rather than left as their unstated setup step: a
    /// brand-new cache's first build has nothing to clone, so every module
    /// is reparsed. Cross-checked against `measure_warm_open`'s own
    /// unchanged-rebuild total: cold-open's total parse volume must equal
    /// warm-unchanged's total clone volume, because both walk the identical
    /// admitted `Program` set -- only which side of the report it lands on
    /// differs.
    #[test]
    fn cold_open_parses_every_module_and_clones_none() {
        let (manifest, sources) = calculator_project();
        let report = measure_cold_open(&manifest, &sources).unwrap();
        assert!(report.cloned.is_empty());
        assert_eq!(report.reparsed.len(), sources.len());
        assert!(report.total_reparsed_nodes() > 0);
        let warm_unchanged = measure_warm_open(&manifest, &sources, &sources).unwrap();
        assert_eq!(
            report.total_reparsed_nodes(),
            warm_unchanged.total_cloned_nodes()
        );
        eprintln!(
            "cold open: {} modules parsed, {} AST nodes",
            report.reparsed.len(),
            report.total_reparsed_nodes()
        );
    }

    /// A body-only edit to a module nothing else imports invalidates exactly
    /// that module (matching
    /// `warm_open_after_local_body_edit_reuses_unaffected_modules` in the
    /// cross-process CLI harness); its siblings remain whole-module clone
    /// hits. This is the scenario where the clone genuinely buys something:
    /// most of the project's AST volume is still cloned, not reparsed.
    #[test]
    fn local_body_edit_reparses_one_module_and_clones_the_rest() {
        let (manifest, sources) = calculator_project();
        let edited = with_replacement(&sources, "src/app.spx", "multiply(6, 7)", "multiply(7, 6)");
        let report = measure_warm_open(&manifest, &sources, &edited).unwrap();
        let reparsed_paths: BTreeSet<&str> = report
            .reparsed
            .iter()
            .map(|module| module.path.as_str())
            .collect();
        assert_eq!(reparsed_paths, BTreeSet::from(["src/app.spx"]));
        assert_eq!(report.cloned.len(), sources.len() - 1);
        assert!(report.total_cloned_nodes() > 0);
        assert!(report.total_reparsed_nodes() > 0);
        eprintln!(
            "local body edit: {} modules cloned ({} AST nodes), {} module reparsed ({} AST nodes)",
            report.cloned.len(),
            report.total_cloned_nodes(),
            report.reparsed.len(),
            report.total_reparsed_nodes()
        );
    }

    /// #130/#131: the reverse-import transitive closure that used to seed
    /// `invalidated` in `FrontendPass::build` (see that function's doc
    /// comment) is gone. Editing the shared provider's body now invalidates
    /// only the provider's own AST-cache entry; `src/app.spx` and
    /// `src/tests.spx` import `add` from `src/core.spx` but their own text
    /// is untouched, so their cached `Program` is reused. This is the exact
    /// scenario #130/#131's audit measured as the expensive one (0 cloned,
    /// every module reparsed); it is now the cheap one, symmetric with the
    /// local-body-edit case above instead of its opposite.
    #[test]
    fn provider_edit_clones_unaffected_consumers_and_reparses_only_the_provider() {
        let (manifest, sources) = calculator_project();
        let edited = with_replacement(&sources, "src/core.spx", "left + right", "right + left");
        let report = measure_warm_open(&manifest, &sources, &edited).unwrap();
        let reparsed_paths: BTreeSet<&str> = report
            .reparsed
            .iter()
            .map(|module| module.path.as_str())
            .collect();
        assert_eq!(reparsed_paths, BTreeSet::from(["src/core.spx"]));
        assert_eq!(report.cloned.len(), sources.len() - 1);
        assert!(report.total_cloned_nodes() > 0);
        assert!(report.total_reparsed_nodes() > 0);
        eprintln!(
            "provider edit: {} modules cloned ({} AST nodes), {} module reparsed ({} AST nodes)",
            report.cloned.len(),
            report.total_cloned_nodes(),
            report.reparsed.len(),
            report.total_reparsed_nodes()
        );
    }

    /// #130's "recovered" workflow, on the same footing as the other
    /// scenarios: #131 requires that a failed rebuild leave prior cache
    /// state intact and never label historical facts current. Here a build
    /// with a signature-breaking provider edit is refused (matching
    /// `changed_import_signature_is_rechecked_and_failed_build_does_not_poison_cache`
    /// in `tests/project/frontend_cache.rs`), and the measurement is what a
    /// *subsequent* build of the original, unedited sources costs. A
    /// poisoned or partially evicted cache would show up here as an
    /// unexpected reparse; instead recovery costs exactly what an
    /// already-unchanged rebuild costs.
    #[test]
    fn recovered_open_after_a_failed_build_reclones_the_whole_project() {
        let (manifest, sources) = calculator_project();
        let failing = with_replacement(
            &sources,
            "src/core.spx",
            "fn add(left: i64, right: i64)",
            "fn add(left: i64, right: i64, extra: i64)",
        );
        let report = measure_recovered_open(&manifest, &sources, &failing).unwrap();
        assert!(report.reparsed.is_empty());
        assert_eq!(report.cloned.len(), sources.len());
        assert!(report.total_cloned_nodes() > 0);
        let unchanged = measure_warm_open(&manifest, &sources, &sources).unwrap();
        assert_eq!(report.total_cloned_nodes(), unchanged.total_cloned_nodes());
        eprintln!(
            "recovered open: {} modules cloned, {} AST nodes",
            report.cloned.len(),
            report.total_cloned_nodes()
        );
    }

    /// Fault-injected negative control (the repo's fault-injection testing
    /// standard, and #131's own mandated review checkpoint for a narrowing
    /// like this): a change that actually invalidates a module -- its own
    /// text -- must still be caught even with the reverse-import closure
    /// gone. This mutates the provider's *signature* (not just its body),
    /// which every consumer's exact-generated call site depends on, and
    /// checks the same source through the real, unbypassed admission path
    /// (`measure_warm_open` never recomputes invalidation itself -- see its
    /// doc comment). If this test were made to pass by reintroducing a bug
    /// that stops evicting a changed file's *own* AST-cache entry, the
    /// warm build would keep silently admitting the stale pre-edit `add`,
    /// and this diagnostic assertion would go red.
    #[test]
    fn provider_signature_change_still_invalidates_and_fails_the_same_way_cold_does() {
        let (manifest, sources) = calculator_project();
        let edited = with_replacement(
            &sources,
            "src/core.spx",
            "fn add(left: i64, right: i64)",
            "fn add(left: i64, right: i64, extra: i64)",
        );
        let mut cache = ProjectFrontendCache::new_with_semantic_cache();
        cache.build(&manifest, &sources).unwrap();
        let warm_errors = match cache.build(&manifest, &edited) {
            Ok(build) => panic!(
                "expected the arity mismatch to be rejected, got {:?}",
                build.to_json()
            ),
            Err(errors) => errors,
        };
        let mut cold_cache = ProjectFrontendCache::new_with_semantic_cache();
        let cold_errors = match cold_cache.build(&manifest, &edited) {
            Ok(build) => panic!(
                "expected the arity mismatch to be rejected, got {:?}",
                build.to_json()
            ),
            Err(errors) => errors,
        };
        assert_eq!(format!("{warm_errors:?}"), format!("{cold_errors:?}"));
    }

    /// Negative control: this module is read-only replay of the real
    /// `ProjectFrontendCache::build`, so a genuinely invalid warm-build input
    /// must still be refused by that same admission path, not silently
    /// absorbed into a (wrong) report. A duplicate source path is refused by
    /// `build` itself (`"frontend sources contain duplicate paths"`); this
    /// proves the measurement wrapper propagates that refusal unchanged
    /// rather than swallowing or working around it.
    #[test]
    fn measurement_does_not_bypass_frontend_admission_on_invalid_input() {
        let (manifest, sources) = calculator_project();
        let mut duplicated: Vec<ProjectFrontendSource> = sources
            .iter()
            .map(|source| ProjectFrontendSource::new(source.path(), source.source()).unwrap())
            .collect();
        duplicated
            .push(ProjectFrontendSource::new(sources[0].path(), sources[0].source()).unwrap());
        let result = measure_warm_open(&manifest, &sources, &duplicated);
        let error = match result {
            Ok(report) => panic!("expected duplicate-path input to be refused, got {report:?}"),
            Err(error) => error,
        };
        assert!(
            error
                .iter()
                .any(|diagnostic| diagnostic.message.contains("duplicate paths")),
            "expected the real frontend admission's duplicate-path diagnostic, got {error:?}"
        );
    }
}
