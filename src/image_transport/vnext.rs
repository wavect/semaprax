//! Additive v5 session with an explicit host policy and recoverable source refresh.
use super::*;
use crate::project::{CandidateTestPolicy, ProjectFrontendCache};
use crate::project_transport::codec::RequestId;
use std::path::PathBuf;

mod commit;
mod dependencies;
mod discovery;
mod draft_recovery;
mod projections;
mod read_batch;
mod recovery;
mod review_facets;
mod symbol_diagnostics;
pub use commit::GitCommitHost;

pub const VNEXT_PROTOCOL_SCHEMA: &str = "semaprax.image-agent-protocol.v5";
pub const VNEXT_RESULT_SCHEMA: &str = "semaprax.image-agent-result.v5";

/// Structured final-session diagnostics retained inside `io::Error`.
#[derive(Debug)]
pub struct VNextSessionFailure {
    diagnostics: Vec<Diagnostic>,
}
impl VNextSessionFailure {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}
impl std::fmt::Display for VNextSessionFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index != 0 {
                formatter.write_str("; ")?;
            }
            write!(formatter, "{}: {}", diagnostic.code, diagnostic.message)?;
        }
        Ok(())
    }
}
impl std::error::Error for VNextSessionFailure {}

#[derive(Clone, Copy)]
enum PublicationOutcome {
    Published,
    Uncertain,
}

/// Selected by the host before opening a session; requests cannot change it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VNextPolicy {
    pub candidate_prepare: bool,
    pub diagnostics: bool,
    pub test_policy: Option<CandidateTestPolicy>,
    pub build_enabled: bool,
}

#[derive(Clone, Copy)]
pub(super) enum Action {
    Refresh,
    RefreshPreview,
    Commit,
    Targets,
    Build,
    InterfaceDelta,
    SymbolDiagnostics,
    DraftRecoveryExport,
    DraftRecoveryRestore,
    Dependencies,
    DependencySummary,
    DependencyPage,
}

const REFRESH: Method = Method {
    name: "workspace/refresh",
    operation: Operation::VNext(Action::Refresh),
    parameters: &[
        REVISION,
        Parameter {
            name: "expected_new_project_revision",
            kind: ParameterKind::Digest,
            required: true,
        },
    ],
    query: false,
    payload_schema: "semaprax.image-workspace-refresh.v1",
};
const REFRESH_PREVIEW: Method = Method {
    name: "workspace/refresh-preview",
    operation: Operation::VNext(Action::RefreshPreview),
    parameters: &[REVISION],
    query: true,
    payload_schema: "semaprax.image-workspace-refresh-preview.v1",
};

pub struct VNextSession {
    manifest: PathBuf,
    snapshot: ProjectSnapshot,
    image: Arc<ProjectSemanticImage>,
    registry: candidates::Registry,
    policy: VNextPolicy,
    commit: Option<GitCommitHost>,
    started: bool,
    terminal: bool,
    frontend: Option<ProjectFrontendCache>,
}

impl VNextSession {
    pub fn open(manifest: &Path, policy: VNextPolicy) -> Result<Self, Vec<Diagnostic>> {
        Self::open_inner(manifest, policy, None)
    }

    /// Opt-in invocation-owned AST reuse. Every load still authenticates source
    /// files and performs complete semantic, linking, and profile admission.
    pub fn open_with_frontend_cache(
        manifest: &Path,
        policy: VNextPolicy,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::open_inner(manifest, policy, Some(ProjectFrontendCache::new()))
    }

    /// Opt-in invocation-owned checked-module reuse as well as frontend reuse.
    /// Fresh source authority, invalidation, linking, and profile admission stay
    /// mandatory; requests cannot select or seed this host-created cache.
    pub fn open_with_semantic_cache(
        manifest: &Path,
        policy: VNextPolicy,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::open_inner(
            manifest,
            policy,
            Some(ProjectFrontendCache::new_with_semantic_cache()),
        )
    }

    /// Reuse opaque compiler-created or store-authenticated cache state. Live
    /// source authentication and exact-input admission still occur on opening.
    pub fn open_with_retained_semantic_cache(
        manifest: &Path,
        policy: VNextPolicy,
        cache: ProjectFrontendCache,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !cache.is_semantic_cache_enabled() {
            return Err(failure(
                "SPX-G280",
                "retained semantic cache requires explicit checked-module mode",
            ));
        }
        Self::open_inner(manifest, policy, Some(cache))
    }

    /// Retain a historical compiler cache through the live source boundary.
    /// This host API does not write storage, grant approval or publish source.
    pub fn retained_semantic_cache(&mut self) -> Result<ProjectFrontendCache, Vec<Diagnostic>> {
        if self.terminal {
            return Err(failure(
                "SPX-G280",
                "terminal sessions cannot export a semantic cache",
            ));
        }
        let cache = self
            .frontend
            .as_ref()
            .filter(|cache| cache.is_semantic_cache_enabled())
            .ok_or_else(|| failure("SPX-G280", "semantic cache export requires its host opt-in"))?;
        self.snapshot
            .with_authenticated_request(|_| Ok(cache.fork()))
    }

    fn open_inner(
        manifest: &Path,
        policy: VNextPolicy,
        cache: Option<ProjectFrontendCache>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if !manifest.is_absolute()
            || (!policy.candidate_prepare
                && (policy.diagnostics || policy.test_policy.is_some() || policy.build_enabled))
        {
            return Err(failure("SPX-G280", "v5 requires an absolute host manifest and candidate preparation for diagnostics, tests, or builds"));
        }
        let (mut snapshot, frontend) = if let Some(cache) = cache {
            let (snapshot, cache, _) =
                crate::project::load_snapshot_with_frontend(manifest, cache)?;
            (snapshot, Some(cache))
        } else {
            (crate::project::load_snapshot(manifest)?, None)
        };
        if manifest != snapshot.root().join("semaprax.toml") {
            return Err(failure(
                "SPX-G280",
                "v5 manifest must be its exact authenticated canonical absolute path",
            ));
        }
        let image = snapshot.with_authenticated_request(|snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })?;
        Ok(Self {
            manifest: manifest.to_owned(),
            snapshot,
            image: Arc::new(image),
            registry: candidates::Registry::default(),
            policy,
            commit: None,
            started: false,
            terminal: false,
            frontend,
        })
    }

    /// Attach source-publication authority only before accepting any frame.
    pub fn with_git_commit_host(mut self, host: GitCommitHost) -> Result<Self, Vec<Diagnostic>> {
        if self.started
            || self.commit.is_some()
            || !self.policy.candidate_prepare
            || host.manifest() != self.manifest
        {
            return Err(failure("SPX-G280", "Git publication host must be attached once before requests and match the session manifest"));
        }
        self.commit = Some(host);
        Ok(self)
    }

    /// Out-of-band host approval; deliberately not a protocol method.
    pub fn approve_git_commit(
        &mut self,
        candidate_digest: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        if self.started {
            return Err(failure(
                "SPX-G280",
                "v5 host approvals must precede the first frame",
            ));
        }
        self.commit
            .as_mut()
            .ok_or_else(|| {
                failure(
                    "SPX-G280",
                    "v5 source-publication authority was not selected",
                )
            })?
            .approve(candidate_digest)
    }

    pub fn image_revision(&self) -> &str {
        self.image.image_digest()
    }
    pub fn is_terminal(&self) -> bool {
        self.terminal
    }
    pub fn policy(&self) -> VNextPolicy {
        self.policy
    }

    /// Notifications and invalid frames never perform semantic work or refresh.
    pub fn handle_frame(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        if self.terminal || frame.is_empty() {
            return None;
        }
        self.started = true;
        if frame.len() > MAX_REQUEST_BYTES {
            self.terminal = true;
            return Some(codec::bounded_error_response(
                None,
                codec::PARSE_ERROR,
                "request exceeds configured byte limit",
                MAX_RESPONSE_BYTES,
            ));
        }
        let request = match codec::decode_request(frame) {
            Ok(request) => request,
            Err(error) => {
                return (!error.suppress_response).then(|| {
                    codec::bounded_error_response(
                        error.response_id.as_ref(),
                        error.code,
                        &error.message,
                        MAX_RESPONSE_BYTES,
                    )
                })
            }
        };
        let RequestKind::Call(id) = request.kind else {
            return None;
        };
        let available = methods(&self.policy, self.commit.is_some());
        let Some(method) = available
            .iter()
            .copied()
            .find(|method| method.name == request.method)
        else {
            return Some(codec::bounded_error_response(
                Some(&id),
                -32601,
                "method is unavailable in the host-selected v5 session",
                MAX_RESPONSE_BYTES,
            ));
        };
        let params = request.params.unwrap_or_default();
        if let Err(message) = validate_parameters(method, &params) {
            return Some(codec::bounded_error_response(
                Some(&id),
                codec::INVALID_PARAMS,
                &message,
                MAX_RESPONSE_BYTES,
            ));
        }
        // This check precedes every extension or historical candidate lookup.
        if params
            .get("image_revision")
            .is_some_and(|expected| expected.as_str() != Some(self.image.image_digest()))
        {
            return Some(error_response(
                &id,
                &failure("SPX-G282", "v5 expected image revision is stale"),
            ));
        }
        let response = match method.operation {
            Operation::VNext(Action::Refresh) => self.refresh(&id, &params, false),
            Operation::VNext(Action::RefreshPreview) => self.refresh(&id, &params, true),
            Operation::VNext(Action::Commit) => self.commit_request(&id, method, &params),
            _ => self.ordinary_request(&id, method, &params, &available),
        };
        Some(response)
    }

    fn ordinary_request(
        &mut self,
        id: &RequestId,
        method: &Method,
        params: &Map<String, Value>,
        available: &[&'static Method],
    ) -> Vec<u8> {
        let image = &self.image;
        let registry = &self.registry;
        let policy = &self.policy;
        let commit_enabled = self.commit.is_some();
        let prepared = self.snapshot.with_authenticated_request(|_| {
            let (payload, mutation) = match method.operation {
                Operation::Capabilities
                | Operation::Schemas
                | Operation::Instructions
                | Operation::Client
                | Operation::Catalog => (
                    discovery::payload(method, params, available, policy, commit_enabled)?,
                    candidates::Mutation::None,
                ),
                Operation::VNext(action @ (Action::Targets | Action::Build)) => (
                    projections::prepare(action, params, image, registry)?,
                    candidates::Mutation::None,
                ),
                Operation::VNext(Action::InterfaceDelta) => (
                    review_facets::interface_delta(params, image, registry)?,
                    candidates::Mutation::None,
                ),
                Operation::VNext(Action::SymbolDiagnostics) => (
                    symbol_diagnostics::prepare(params, image, registry)?,
                    candidates::Mutation::None,
                ),
                Operation::VNext(Action::Dependencies) => (
                    dependencies::prepare(params, image)?,
                    candidates::Mutation::None,
                ),
                Operation::VNext(action @ (Action::DependencySummary | Action::DependencyPage)) => {
                    (
                        dependencies::prepare_navigation(action, params, image)?,
                        candidates::Mutation::None,
                    )
                }
                Operation::VNext(
                    action @ (Action::DraftRecoveryExport | Action::DraftRecoveryRestore),
                ) => draft_recovery::prepare(action, params, image, registry)?,
                Operation::Candidate(candidates::Action::Diagnostic(_)) => {
                    candidates::diagnostics::prepare(
                        method,
                        params,
                        image,
                        registry,
                        policy.test_policy.as_ref(),
                    )?
                }
                Operation::Candidate(_) => candidates::prepare(
                    method,
                    params,
                    image,
                    registry,
                    policy.test_policy.as_ref(),
                )?,
                _ => (dispatch(method, params, image)?, candidates::Mutation::None),
            };
            registry.admit(&mutation)?;
            let response = response(id, image, payload);
            let mutation = if codec::is_overflow_response(&response) {
                candidates::Mutation::None
            } else {
                mutation
            };
            Ok((response, mutation))
        });
        match prepared {
            Ok((response, mutation)) => {
                self.registry.commit(mutation);
                response
            }
            Err(errors) => error_response(id, &errors),
        }
    }

    fn refresh(&mut self, id: &RequestId, params: &Map<String, Value>, preview: bool) -> Vec<u8> {
        // Do not touch/revive the old absorbing snapshot. Recovery independently
        // opens the one host-bound manifest and requires the expected new subject.
        let prepared = (|| {
            // Fork only compiler-owned AST/optional checked-module entries;
            // newly held handles and bytes are authenticated independently even
            // for unchanged sources. Preview and failures never adopt this fork.
            let (mut snapshot, frontend, frontend_work) = match &self.frontend {
                Some(cache) => {
                    let (snapshot, cache, work) =
                        crate::project::load_snapshot_with_frontend(&self.manifest, cache.fork())?;
                    (snapshot, Some(cache), Some(work))
                }
                None => (crate::project::load_snapshot(&self.manifest)?, None, None),
            };
            if self.manifest != snapshot.root().join("semaprax.toml")
                || snapshot.manifest().to_canonical_toml()
                    != self.image.revision().manifest().to_canonical_toml()
            {
                return Err(failure(
                    "SPX-G283",
                    "v5 refresh cannot change its host-bound canonical manifest configuration",
                ));
            }
            if !preview
                && snapshot.project_revision() != text(params, "expected_new_project_revision")
            {
                return Err(failure(
                    "SPX-G282",
                    "v5 refresh source revision differs from the caller expectation",
                ));
            }
            let (image, bytes) = snapshot.with_authenticated_request(|fresh| {
                let candidate_image = ProjectSemanticImage::derive(fresh.retain_revision(), fresh.project_revision())?;
                let reused = candidate_image.image_digest() == self.image.image_digest();
                if reused && candidate_image.to_json() != self.image.to_json() {
                    return Err(failure("SPX-G283", "v5 unchanged image identity has inconsistent canonical bytes"));
                }
                let image = if reused { Arc::clone(&self.image) } else { Arc::new(candidate_image) };
                if preview {
                    let mut payload = json!({"schema":"semaprax.image-workspace-refresh-preview.v1",
                        "old_image_revision":self.image.image_digest(),"observed_image_revision":image.image_digest(),
                        "observed_project_revision":image.revision().project_revision(),"workspace_revision":image.revision().workspace_revision(),
                        "manifest_changed":false,"source_authority":false,"current_state_replaced":false,"requires_explicit_refresh":true});
                    if let Some(work) = &frontend_work { payload["frontend_work"] = work.clone(); }
                    let bytes = response(id, &self.image, payload);
                    if codec::is_overflow_response(&bytes) { return Err(failure("SPX-G281", "v5 refresh preview response exceeds its byte bound")); }
                    return Ok((image, bytes));
                }
                let inventory = self.registry.refresh_inventory();
                let mut payload = json!({"schema":"semaprax.image-workspace-refresh.v1",
                    "old_image_revision":self.image.image_digest(),"image_revision":image.image_digest(),
                    "old_project_revision":self.image.revision().project_revision(),"project_revision":image.revision().project_revision(),
                    "workspace_revision":image.revision().workspace_revision(),"image_arc_reused":reused,
                    "retained_candidates":inventory["retained_candidates"],"cleared_drafts":inventory["cleared_drafts"],"cleared_attempts":inventory["cleared_attempts"],
                    "manifest_changed":false,"source_authority":false,"recovery":"explicit_fresh_snapshot",
                    "nonclaims":["no_implicit_candidate_rebase","no_draft_or_attempt_remapping","no_source_publication","no_incremental_build_claim"]});
                if let Some(work) = &frontend_work { payload["frontend_work"] = work.clone(); }
                let bytes = response(id, &image, payload);
                if codec::is_overflow_response(&bytes) { return Err(failure("SPX-G281", "v5 refresh response exceeds its byte bound")); }
                Ok((image, bytes))
            })?;
            Ok((snapshot, image, frontend, bytes))
        })();
        match prepared {
            Ok((snapshot, image, frontend, response)) => {
                if !preview {
                    self.snapshot = snapshot;
                    self.image = image;
                    self.frontend = frontend;
                    self.registry.clear_transients();
                }
                response
            }
            Err(errors) => error_response(id, &errors),
        }
    }

    fn commit_request(
        &mut self,
        id: &RequestId,
        method: &Method,
        params: &Map<String, Value>,
    ) -> Vec<u8> {
        let Some(host) = self.commit.as_mut() else {
            return error_response(
                id,
                &failure("SPX-G280", "v5 source-publication authority is unavailable"),
            );
        };
        let result = match method.name {
            // Immutable outcome/status remain inspectable after an uncertain
            // publication even if held source authentication now fails.
            "source-commit/status" => Ok(host.status()),
            "candidate/commit-report" => host.report(params),
            "candidate/commit" => {
                let candidate = self.snapshot.with_authenticated_request(|snapshot| {
                    let candidate = self.registry.candidate(text(params, "candidate_revision"))?;
                    if candidate.base_revision().project_revision() != snapshot.project_revision() {
                        return Err(failure("SPX-G282", "v5 publication requires a candidate based on the current held source revision"));
                    }
                    Ok(Arc::clone(candidate))
                });
                // The Git authority owns replay, one CAS, and uncertain outcome
                // checks. Generic post-request authentication must not replace
                // a post-publication G267 outcome with an ordinary stale error.
                candidate.and_then(|candidate| host.execute(&candidate, &self.manifest, params))
            }
            _ => Err(failure("SPX-G280", "unknown v5 publication operation")),
        };
        match result {
            Ok(payload) => response(id, &self.image, payload),
            Err(errors) => error_response(id, &errors),
        }
    }

    pub fn finish(&mut self) -> Result<(), Vec<Diagnostic>> {
        let final_check = self.snapshot.with_authenticated_request(|_| Ok(()));
        if self.commit.as_ref().is_some_and(GitCommitHost::is_terminal) {
            let published = self
                .commit
                .as_ref()
                .is_some_and(|host| host.status()["state"] == "published");
            final_check.map_err(|mut errors| {
                let mut classified = failure("SPX-G287", if published {
                    "Git host status remains published; later final source authentication failed; retain the successful publication receipt and do not replay its commit"
                } else {
                    "Git host status remains publication_uncertain; final source authentication also failed; inspect host status and the Git ref before retrying"
                });
                classified.append(&mut errors);
                classified
            })
        } else {
            final_check
        }
    }
}

fn methods(policy: &VNextPolicy, commit_enabled: bool) -> Vec<&'static Method> {
    use candidates::diagnostics::Action as DiagnosticAction;
    let mut methods = candidates::diagnostics::methods(policy.test_policy.is_some())
        .into_iter()
        .filter(|method| match method.operation {
            Operation::Candidate(candidates::Action::Diagnostic(
                DiagnosticAction::ProtocolConformance,
            )) => true,
            Operation::Candidate(candidates::Action::Diagnostic(
                DiagnosticAction::ExpressionHoleOpen
                | DiagnosticAction::InterfaceCatalog
                | DiagnosticAction::Delta
                | DiagnosticAction::DeltaCatalog,
            )) => policy.candidate_prepare,
            Operation::Candidate(candidates::Action::Diagnostic(_)) => policy.diagnostics,
            Operation::Candidate(_) => policy.candidate_prepare,
            _ => true,
        })
        .collect::<Vec<_>>();
    methods.push(&REFRESH);
    methods.push(&REFRESH_PREVIEW);
    methods.push(dependencies::method());
    methods.extend(dependencies::navigation_methods());
    methods.extend(projections::methods(policy.build_enabled));
    methods.extend(review_facets::methods(policy));
    if policy.candidate_prepare {
        methods.extend(draft_recovery::methods());
    }
    if commit_enabled {
        methods.extend(commit::methods());
    }
    methods.sort_by_key(|method| method.name);
    methods
}

fn response(id: &RequestId, image: &ProjectSemanticImage, payload: Value) -> Vec<u8> {
    let mut envelope = json!({"schema":VNEXT_RESULT_SCHEMA,"protocol":VNEXT_PROTOCOL_SCHEMA,
        "image_revision":image.image_digest(),"project_revision":image.revision().project_revision(),"payload":payload});
    envelope.sort_all_objects();
    codec::bounded_success_response(id, &envelope.to_string(), MAX_RESPONSE_BYTES)
}
fn failure(code: &'static str, message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(code, message)]
}
fn error_response(id: &RequestId, errors: &[Diagnostic]) -> Vec<u8> {
    codec::bounded_error_response(Some(id), -32000, &diagnostics(errors), MAX_RESPONSE_BYTES)
}

/// The embedding host supplies both streams and the already configured session.
pub fn serve_vnext<R: BufRead, W: Write>(
    input: R,
    output: W,
    mut session: VNextSession,
) -> io::Result<()> {
    let limits = StdioLimits::new(MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES).expect("fixed v5 limits");
    let mut input = FrameReader::new(input, limits);
    let mut output = FrameWriter::new(output, limits);
    let result = (|| {
        loop {
            let response = match input.read_frame()? {
                Frame::Eof => break,
                Frame::OversizedTerminal => {
                    output.write_response(&codec::bounded_error_response(
                        None,
                        codec::PARSE_ERROR,
                        "request exceeds configured byte limit",
                        MAX_RESPONSE_BYTES,
                    ))?;
                    break;
                }
                Frame::Data(frame) => session.handle_frame(&frame),
            };
            if let Some(response) = response {
                output.write_response(&response)?;
            }
            if session.is_terminal() {
                break;
            }
        }
        Ok(())
    })();
    // Authenticate even after stream failure. Preserve the distinction between
    // an ordinary failed request and an already attempted Git publication.
    let final_check = session.finish();
    let outcome = session
        .commit
        .as_ref()
        .filter(|host| host.is_terminal())
        .map(|host| {
            if host.status()["state"] == "published" {
                PublicationOutcome::Published
            } else {
                PublicationOutcome::Uncertain
            }
        });
    finish_stream(result, final_check, outcome)
}

fn finish_stream(
    stream: io::Result<()>,
    final_check: Result<(), Vec<Diagnostic>>,
    outcome: Option<PublicationOutcome>,
) -> io::Result<()> {
    match (stream, final_check) {
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(diagnostics)) => Err(io::Error::other(VNextSessionFailure { diagnostics })),
        (Err(stream_error), final_check) => {
            let mut diagnostics = match outcome {
                Some(PublicationOutcome::Published) => failure("SPX-G287", "v5 transport I/O failed after Git host status published; publication remains known successful; retain or inspect its receipt and do not replay the commit"),
                Some(PublicationOutcome::Uncertain) => failure("SPX-G287", "v5 transport I/O failed after Git host status publication_uncertain; inspect host status and the Git ref before retrying"),
                None => Vec::new(),
            };
            if let Err(mut authentication_errors) = final_check {
                diagnostics.append(&mut authentication_errors);
            }
            if diagnostics.is_empty() {
                return Err(stream_error);
            }
            diagnostics.push(Diagnostic::io(
                "SPX-G280",
                format!(
                    "v5 transport stream failed with I/O kind {:?}",
                    stream_error.kind()
                ),
            ));
            Err(io::Error::other(VNextSessionFailure { diagnostics }))
        }
    }
}

#[cfg(test)]
mod stream_failure_tests {
    use super::*;
    #[test]
    fn typed_final_failure_preserves_original_codes() {
        let error = finish_stream(
            Ok(()),
            Err(failure(
                "SPX-G287",
                "published outcome with later source drift",
            )),
            Some(PublicationOutcome::Published),
        )
        .unwrap_err();
        let retained = error
            .get_ref()
            .unwrap()
            .downcast_ref::<VNextSessionFailure>()
            .unwrap();
        assert_eq!(retained.diagnostics()[0].code, "SPX-G287");
        assert!(error.to_string().contains("published outcome"));
    }
    #[test]
    fn stream_errors_preserve_outcome_and_final_authentication() {
        for (outcome, text) in [
            (PublicationOutcome::Published, "known successful"),
            (PublicationOutcome::Uncertain, "publication_uncertain"),
        ] {
            let error = finish_stream(
                Err(io::Error::from(io::ErrorKind::BrokenPipe)),
                Err(failure("SPX-G251", "source drift")),
                Some(outcome),
            )
            .unwrap_err();
            let retained = error
                .get_ref()
                .unwrap()
                .downcast_ref::<VNextSessionFailure>()
                .unwrap();
            assert_eq!(retained.diagnostics()[0].code, "SPX-G287");
            assert!(retained.diagnostics()[0].message.contains(text));
            assert!(retained
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-G251"));
        }
        let ordinary = finish_stream(
            Err(io::Error::from(io::ErrorKind::BrokenPipe)),
            Ok(()),
            None,
        )
        .unwrap_err();
        assert_eq!(ordinary.kind(), io::ErrorKind::BrokenPipe);
    }
}
