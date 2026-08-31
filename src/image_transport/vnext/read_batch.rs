//! Host-invoked parallel image reads; no registry, source handles or Git host
//! enter worker threads. The ordinary NDJSON server remains sequential.
use super::*;

const MAX_FRAMES: usize = 16;
const MAX_WORKERS: usize = 4;

pub(super) enum Read {
    Immediate(Option<Vec<u8>>),
    Query {
        id: RequestId,
        method: &'static Method,
        params: Map<String, Value>,
    },
}

enum DetachedRead<'a> {
    Immediate(&'a Option<Vec<u8>>),
    Discovery {
        id: &'a RequestId,
        payload: Result<Value, Vec<Diagnostic>>,
    },
    Query {
        id: &'a RequestId,
        method: &'static Method,
        params: &'a Map<String, Value>,
        subjects: Result<candidates::reads::ReadSubjects, Vec<Diagnostic>>,
    },
}

impl VNextSession {
    /// The host chooses concurrency; frames cannot request worker count or
    /// widen the closed immutable-read subset. Results retain input order.
    /// No result escapes until all workers join and live source rechecks pass.
    pub fn handle_read_batch(
        &mut self,
        frames: &[&[u8]],
        workers: usize,
    ) -> Result<Vec<Option<Vec<u8>>>, Vec<Diagnostic>> {
        self.package_attachment_closed = true;
        if frames.is_empty() || frames.len() > MAX_FRAMES || !(1..=MAX_WORKERS).contains(&workers) {
            return Err(failure(
                "SPX-G294",
                "parallel read batch exceeds its frame or worker bounds",
            ));
        }
        if self.terminal {
            return Err(failure(
                "SPX-G294",
                "parallel reads require an open v5 session",
            ));
        }
        if frames.iter().any(|frame| !frame.is_empty()) {
            self.started = true;
        }
        // Check every length before copying or parsing any request. Unlike the
        // stream loop, this host API reports a batch error with no partial rows.
        if frames.iter().any(|frame| frame.len() > MAX_REQUEST_BYTES) {
            return Err(failure(
                "SPX-G294",
                "parallel read frame exceeds its byte bound",
            ));
        }
        let available = session_methods(
            &self.policy,
            self.commit.is_some(),
            self.package_graph.is_some(),
            self.read_batch_workers.is_some(),
        );
        let reads = frames
            .iter()
            .map(|frame| prepare_read(frame, &available, &self.image))
            .collect::<Vec<_>>();
        if reads.iter().all(|read| matches!(read, Read::Immediate(_))) {
            return Ok(reads
                .into_iter()
                .map(|read| match read {
                    Read::Immediate(response) => response,
                    Read::Query { .. } => unreachable!("read inventory checked"),
                })
                .collect());
        }
        let context = ReadContext {
            image: &self.image,
            package_graph: self.package_graph.as_deref(),
            registry: &self.registry,
            policy: &self.policy,
            commit_enabled: self.commit.is_some(),
            available: &available,
        };
        self.snapshot
            .with_authenticated_request(|_| execute(&reads, workers, context))
    }

    /// Discover the host batch API's fixed read subset without doing source
    /// work. This does not add a JSON-RPC batching method or an authority grant.
    pub fn parallel_read_methods(&self) -> Vec<&'static str> {
        session_methods(
            &self.policy,
            self.commit.is_some(),
            self.package_graph.is_some(),
            self.read_batch_workers.is_some(),
        )
        .into_iter()
        .filter(|method| parallel_read(method.operation))
        .map(|method| method.name)
        .collect()
    }
}

/// Coordinator-only inputs. Destructure before spawning workers so neither the
/// registry nor descriptive host policy enters any worker closure.
pub(super) struct ReadContext<'a> {
    pub(super) image: &'a ProjectSemanticImage,
    pub(super) package_graph: Option<&'a crate::package_semantic_graph::PackageSemanticGraph>,
    pub(super) registry: &'a candidates::Registry,
    pub(super) policy: &'a VNextPolicy,
    pub(super) commit_enabled: bool,
    pub(super) available: &'a [&'static Method],
}

/// Caller must hold the ordinary before/after source-authentication boundary.
/// This seam performs no authentication itself and returns no registry mutation.
pub(super) fn execute(
    reads: &[Read],
    workers: usize,
    context: ReadContext<'_>,
) -> Result<Vec<Option<Vec<u8>>>, Vec<Diagnostic>> {
    let ReadContext {
        image,
        package_graph,
        registry,
        policy,
        commit_enabled,
        available,
    } = context;
    let test_enabled = policy.test_policy.is_some();
    // Lookup failures remain semantic query results, including all-unknown
    // batches. Cheap discovery is prepared serially under authentication.
    let detached = reads
        .iter()
        .map(|read| match read {
            Read::Immediate(response) => DetachedRead::Immediate(response),
            Read::Query { id, method, params } => match method.operation {
                Operation::Capabilities
                | Operation::Schemas
                | Operation::Instructions
                | Operation::Client
                | Operation::Catalog => DetachedRead::Discovery {
                    id,
                    payload: discovery::payload(method, params, available, policy, commit_enabled),
                },
                _ => DetachedRead::Query {
                    id,
                    method,
                    params,
                    subjects: registry.detach_read(method.operation, params),
                },
            },
        })
        .collect::<Vec<_>>();
    parallel_map(&detached, workers, &|read| match read {
        DetachedRead::Immediate(response) => (*response).clone(),
        DetachedRead::Discovery { id, payload } => Some(match payload {
            Ok(payload) => response(id, image, payload.clone()),
            Err(errors) => error_response(id, errors),
        }),
        DetachedRead::Query {
            id,
            method,
            params,
            subjects,
        } => {
            let payload = subjects
                .as_ref()
                .map_err(Clone::clone)
                .and_then(|subjects| match method.operation {
                    Operation::Candidate(candidates::Action::Diagnostic(action)) => {
                        candidates::diagnostics::read_payload(action, params, image, subjects)
                    }
                    Operation::Candidate(action) => {
                        candidates::reads::payload(action, params, image, subjects, test_enabled)
                    }
                    operation if retained_reads::supports(operation) => {
                        retained_reads::prepare(operation, params, image, subjects)
                    }
                    Operation::VNext(Action::Dependencies) => dependencies::prepare(params, image),
                    Operation::VNext(
                        action @ (Action::PackageSummary | Action::PackageConsumers),
                    ) => package_graph::prepare(action, params, image, package_graph),
                    Operation::VNext(Action::AnalysisCoverage) => {
                        analysis_coverage::prepare(params, image)
                    }
                    Operation::VNext(
                        action @ (Action::FunctionInstances | Action::FunctionInstanceFacet),
                    ) => function_instances::prepare(action, params, image),
                    Operation::VNext(Action::CleanupDependencies) => {
                        cleanup_dependencies::prepare(params, image)
                    }
                    Operation::VNext(
                        action @ (Action::DependencySummary | Action::DependencyPage),
                    ) => dependencies::prepare_navigation(action, params, image),
                    _ => dispatch(method, params, image),
                });
            Some(match payload {
                Ok(payload) => response(id, image, payload),
                Err(errors) => error_response(id, &errors),
            })
        }
    })
}

pub(super) fn parallel_read(operation: Operation) -> bool {
    if retained_reads::supports(operation) {
        return true;
    }
    if let Operation::Candidate(action) = operation {
        return candidates::reads::supports(action);
    }
    matches!(
        operation,
        Operation::Capabilities
            | Operation::Schemas
            | Operation::Instructions
            | Operation::Client
            | Operation::Catalog
            | Operation::Open
            | Operation::Status
            | Operation::Symbol
            | Operation::Context
            | Operation::Impact
            | Operation::FunctionSummary
            | Operation::Facet
            | Operation::VNext(Action::Dependencies)
            | Operation::VNext(Action::AnalysisCoverage)
            | Operation::VNext(Action::FunctionInstances | Action::FunctionInstanceFacet)
            | Operation::VNext(Action::PackageSummary | Action::PackageConsumers)
            | Operation::VNext(Action::CleanupDependencies)
            | Operation::VNext(Action::DependencySummary | Action::DependencyPage)
    )
}

pub(super) fn prepare_read(
    frame: &[u8],
    available: &[&'static Method],
    image: &ProjectSemanticImage,
) -> Read {
    if frame.is_empty() {
        return Read::Immediate(None);
    }
    let request = match codec::decode_request(frame) {
        Ok(request) => request,
        Err(error) => {
            return Read::Immediate((!error.suppress_response).then(|| {
                codec::bounded_error_response(
                    error.response_id.as_ref(),
                    error.code,
                    &error.message,
                    MAX_RESPONSE_BYTES,
                )
            }))
        }
    };
    let RequestKind::Call(id) = request.kind else {
        return Read::Immediate(None);
    };
    let Some(method) = available
        .iter()
        .copied()
        .find(|method| method.name == request.method && parallel_read(method.operation))
    else {
        return Read::Immediate(Some(codec::bounded_error_response(
            Some(&id),
            -32601,
            "method is unavailable in the host parallel immutable-read subset",
            MAX_RESPONSE_BYTES,
        )));
    };
    let params = request.params.unwrap_or_default();
    if let Err(message) = validate_parameters(method, &params) {
        return Read::Immediate(Some(codec::bounded_error_response(
            Some(&id),
            codec::INVALID_PARAMS,
            &message,
            MAX_RESPONSE_BYTES,
        )));
    }
    if params
        .get("image_revision")
        .is_some_and(|expected| expected.as_str() != Some(image.image_digest()))
    {
        return Read::Immediate(Some(error_response(
            &id,
            &failure("SPX-G282", "v5 expected image revision is stale"),
        )));
    }
    Read::Query { id, method, params }
}

// Scoped workers borrow only immutable inputs. Every successfully spawned
// worker is explicitly joined even after another spawn or worker fails.
fn parallel_map<T: Sync, R: Send>(
    items: &[T],
    workers: usize,
    operation: &(impl Fn(&T) -> R + Sync),
) -> Result<Vec<R>, Vec<Diagnostic>> {
    std::thread::scope(|scope| {
        let count = workers.min(items.len());
        let mut handles = Vec::with_capacity(count);
        let mut failed = false;
        for worker in 0..count {
            match std::thread::Builder::new()
                .name(format!("spx-image-read-{worker}"))
                .spawn_scoped(scope, move || {
                    (worker..items.len())
                        .step_by(count)
                        .map(|index| (index, operation(&items[index])))
                        .collect::<Vec<_>>()
                }) {
                Ok(handle) => handles.push(handle),
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        let mut rows = Vec::with_capacity(items.len());
        for handle in handles {
            match handle.join() {
                Ok(mut result) => rows.append(&mut result),
                Err(_) => failed = true,
            }
        }
        if failed {
            return Err(failure(
                "SPX-G295",
                "parallel read worker failed; no batch results are released",
            ));
        }
        rows.sort_by_key(|(index, _)| *index);
        Ok(rows.into_iter().map(|(_, value)| value).collect())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn dependency_query_extends_only_the_immutable_batch_subset() {
        assert!(parallel_read(Operation::VNext(Action::Dependencies)));
        assert!(parallel_read(Operation::VNext(Action::FunctionInstances)));
        assert!(parallel_read(Operation::VNext(
            Action::FunctionInstanceFacet
        )));
        assert!(parallel_read(Operation::VNext(Action::CleanupDependencies)));
        assert!(parallel_read(Operation::VNext(Action::DependencySummary)));
        assert!(parallel_read(Operation::VNext(Action::DependencyPage)));
        for action in [
            Action::CandidateCleanupDependencies,
            Action::DraftRecoveryExport,
            Action::DraftArchiveExport,
            Action::InterfaceDelta,
            Action::ContractDelta,
            Action::OwnershipDelta,
            Action::SourceReview,
            Action::CandidateMergePreview,
            Action::HoleSummary,
            Action::HolePage,
            Action::HoleFillSuggestions,
            Action::DraftExpressionCatalog,
            Action::SymbolDiagnostics,
            Action::ContractExpressionCatalog,
            Action::Targets,
        ] {
            assert!(parallel_read(Operation::VNext(action)));
        }
        for action in [
            Action::ReadBatch,
            Action::Build,
            Action::ArtifactDelta,
            Action::Commit,
            Action::DraftRecoveryRestore,
            Action::DraftArchiveRestore,
            Action::DraftRebase,
            Action::DraftMerge,
            Action::Refresh,
            Action::RefreshPreview,
            Action::ContractHoleOpen,
        ] {
            assert!(!parallel_read(Operation::VNext(action)));
        }
    }

    #[test]
    fn scoped_workers_overlap_and_restore_input_order() {
        let arrivals = (std::sync::Mutex::new(0), std::sync::Condvar::new());
        let active = AtomicUsize::new(0);
        let high = AtomicUsize::new(0);
        let rows = parallel_map(&(0..12).collect::<Vec<_>>(), 4, &|index| {
            if *index < 4 {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                high.fetch_max(current, Ordering::SeqCst);
                let mut count = arrivals.0.lock().unwrap();
                *count += 1;
                arrivals.1.notify_all();
                let (count, _) = arrivals
                    .1
                    .wait_timeout_while(count, std::time::Duration::from_secs(5), |count| {
                        *count < 4
                    })
                    .unwrap();
                assert_eq!(
                    *count, 4,
                    "all read workers must start before the finite deadline"
                );
                drop(count);
                active.fetch_sub(1, Ordering::SeqCst);
            }
            index * 3
        })
        .unwrap();
        assert_eq!(rows, (0..12).map(|value| value * 3).collect::<Vec<_>>());
        assert_eq!(high.load(Ordering::SeqCst), 4);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn worker_panic_discards_results_and_joins_other_workers() {
        let completed = AtomicUsize::new(0);
        let result = parallel_map(&[0, 1, 2, 3], 4, &|value| {
            if *value == 1 {
                panic!("synthetic pure read failure");
            }
            completed.fetch_add(1, Ordering::SeqCst);
            *value
        });
        assert_eq!(result.unwrap_err()[0].code, "SPX-G295");
        assert_eq!(completed.load(Ordering::SeqCst), 3);
    }
}
