//! Explicit-development watcher adapter for authenticated Project reloads.
//!
//! File events are hints. This module has no native watcher thread: a selected
//! client injects portable events or calls `poll`. Every dirty generation goes
//! through ordinary authenticated Project admission before it reaches HR-01.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use super::{
    with_authenticated_project, HotReloadFailure, HotReloadPlan, HotReloadSession,
    HotReloadSourceAgentHandoffStatus, PreparedProjectInterpreterOptions, ProjectRevision,
};
use crate::diagnostic::Diagnostic;

const MANIFEST_FILE: &str = "semaprax.toml";
const MAX_WATCHED_INPUTS: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HotReloadWatchEvent {
    Create(PathBuf),
    Modify(PathBuf),
    Remove(PathBuf),
    Rename { from: PathBuf, to: PathBuf },
    Overflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotReloadWatchState {
    Watching,
    Stopped,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotReloadWatcherUpdate {
    Idle,
    Unchanged,
    CandidateAdmitted,
    CandidateRejected,
    Stopped,
}

/// A bounded external stop request for one watcher.
///
/// Requesting a stop is non-blocking. The watcher observes it at the next
/// record, poll, admission, or activation boundary and releases its pending
/// candidate before reporting [`HotReloadWatcherUpdate::Stopped`].
#[derive(Clone, Debug)]
pub struct HotReloadWatchControl {
    stop_requested: Arc<AtomicBool>,
}

impl HotReloadWatchControl {
    pub fn request_stop(&self) {
        self.stop_requested.store(true, Ordering::Release);
    }
}

#[derive(Debug)]
pub struct HotReloadWatcherFailure {
    pub diagnostics: Vec<Diagnostic>,
}

impl HotReloadWatcherFailure {
    fn diagnostics(diagnostics: Vec<Diagnostic>) -> Self {
        Self { diagnostics }
    }

    fn stopped() -> Self {
        Self::diagnostics(vec![Diagnostic::io(
            "SPX-HR401",
            "hot reload watcher is stopped",
        )])
    }
}

/// Bounded client-driven watching for one explicitly selected Project.
///
/// The selected implementation is bounded polling. It examines exact paths
/// returned by Project admission and never walks a project root. Native editor
/// notification clients use `record` and share the same deterministic coalescer.
pub struct HotReloadWatcher {
    manifest_path: PathBuf,
    root: PathBuf,
    session: HotReloadSession,
    inputs: BTreeSet<PathBuf>,
    fingerprints: BTreeMap<PathBuf, InputFingerprint>,
    event_generation: u64,
    dirty_generation: Option<u64>,
    rescan_required: bool,
    state: HotReloadWatchState,
    last_diagnostics: Vec<Diagnostic>,
    pending_candidate_revision: Option<String>,
    stop_requested: Arc<AtomicBool>,
    #[cfg(test)]
    after_admission: Option<Box<dyn FnMut()>>,
}

impl HotReloadWatcher {
    pub fn start(
        manifest_path: &Path,
        options: PreparedProjectInterpreterOptions,
    ) -> Result<Self, HotReloadWatcherFailure> {
        let (root, paths, active) = with_authenticated_project(manifest_path, |snapshot| {
            Ok((
                snapshot.root().to_path_buf(),
                snapshot.authoritative_input_paths(),
                snapshot.retain_revision(),
            ))
        })
        .map_err(HotReloadWatcherFailure::diagnostics)?;
        let inputs = bounded_inputs(paths).map_err(HotReloadWatcherFailure::diagnostics)?;
        let session = HotReloadSession::new(active, options)
            .map_err(|failure| HotReloadWatcherFailure::diagnostics(failure.diagnostics))?;
        Ok(Self {
            manifest_path: root.join(MANIFEST_FILE),
            root,
            session,
            fingerprints: fingerprints(&inputs),
            inputs,
            event_generation: 0,
            dirty_generation: None,
            rescan_required: false,
            state: HotReloadWatchState::Watching,
            last_diagnostics: Vec::new(),
            pending_candidate_revision: None,
            stop_requested: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            after_admission: None,
        })
    }

    pub fn state(&self) -> HotReloadWatchState {
        self.state
    }
    pub fn session(&self) -> &HotReloadSession {
        &self.session
    }
    pub fn last_diagnostics(&self) -> &[Diagnostic] {
        &self.last_diagnostics
    }
    pub fn control(&self) -> HotReloadWatchControl {
        HotReloadWatchControl {
            stop_requested: Arc::clone(&self.stop_requested),
        }
    }

    /// Coalesce one untrusted hint. Paths outside the exact inventory are
    /// ignored, except while a manifest change needs fresh admission: a newly
    /// named input cannot be in the older inventory yet.
    pub fn record(&mut self, event: HotReloadWatchEvent) -> HotReloadWatcherUpdate {
        if self.observe_stop_request() {
            return HotReloadWatcherUpdate::Stopped;
        }
        if self.state != HotReloadWatchState::Watching {
            return HotReloadWatcherUpdate::Stopped;
        }
        let relevant = match event {
            HotReloadWatchEvent::Overflow => {
                self.rescan_required = true;
                true
            }
            HotReloadWatchEvent::Create(path)
            | HotReloadWatchEvent::Modify(path)
            | HotReloadWatchEvent::Remove(path) => self.relevant(&path),
            HotReloadWatchEvent::Rename { from, to } => self.relevant(&from) || self.relevant(&to),
        };
        if relevant {
            if !self.mark_dirty() {
                return HotReloadWatcherUpdate::CandidateRejected;
            }
        }
        HotReloadWatcherUpdate::Idle
    }

    /// Do one bounded metadata scan then at most two authenticated admissions.
    pub fn poll(&mut self) -> HotReloadWatcherUpdate {
        if self.observe_stop_request() {
            return HotReloadWatcherUpdate::Stopped;
        }
        if self.state != HotReloadWatchState::Watching {
            return HotReloadWatcherUpdate::Stopped;
        }
        if self.fingerprints != fingerprints(&self.inputs) && self.dirty_generation.is_none() {
            self.rescan_required = true;
            if !self.mark_dirty() {
                return HotReloadWatcherUpdate::CandidateRejected;
            }
        }
        match self.dirty_generation.take() {
            Some(generation) => self.admit(generation),
            None => HotReloadWatcherUpdate::Idle,
        }
    }

    /// The caller selects activation explicitly. This reauthenticates current
    /// disk inputs before delegating to HR-01, so a historical candidate cannot
    /// activate after a later save.
    pub fn activate(&mut self, plan: HotReloadPlan) -> Result<(), HotReloadWatcherFailure> {
        self.confirm_activation_candidate()?;
        self.session.activate(plan).map_err(watcher_failure)?;
        self.pending_candidate_revision = None;
        Ok(())
    }

    /// Runs the authenticated source-Agent owner at the same activation
    /// boundary as an interpreter replacement. The watcher retains the
    /// compiler-checked predecessor until the owner has replayed the opaque
    /// handoff row; it never supplies checkpoint, provider, or journal
    /// authority itself.
    pub fn activate_source_agent(
        &mut self,
        plan: HotReloadPlan,
        activate: impl FnOnce(&mut HotReloadSession, HotReloadPlan) -> Result<(), ()>,
    ) -> Result<(), HotReloadWatcherFailure> {
        self.confirm_activation_candidate()?;
        let expected_generation = self.session.generation().checked_add(1).ok_or_else(|| {
            HotReloadWatcherFailure::diagnostics(vec![Diagnostic::io(
                "SPX-HR401",
                "source-Agent activation generation is exhausted",
            )])
        })?;
        let expected_revision = self.pending_candidate_revision.clone().ok_or_else(|| {
            HotReloadWatcherFailure::diagnostics(vec![Diagnostic::io(
                "SPX-HR401",
                "source-Agent activation has no pending candidate",
            )])
        })?;
        if plan.source_agent_handoffs().is_empty() {
            return Err(HotReloadWatcherFailure::diagnostics(vec![Diagnostic::io(
                "SPX-HR401",
                "source-Agent activation requires a compiler-derived handoff",
            )]));
        }
        if activate(&mut self.session, plan).is_err() {
            return Err(self
                .source_agent_activation_failure("source-Agent handoff owner refused activation"));
        }
        if self.session.generation() != expected_generation
            || self.session.active_project_revision() != expected_revision
            || self.session.source_agent_handoff_status()
                != HotReloadSourceAgentHandoffStatus::Activated
        {
            self.session.refuse_source_agent_handoff(true);
            return Err(self.source_agent_activation_failure(
                "source-Agent handoff owner did not acknowledge the exact candidate",
            ));
        }
        self.pending_candidate_revision = None;
        Ok(())
    }

    fn source_agent_activation_failure(
        &mut self,
        ordinary: &'static str,
    ) -> HotReloadWatcherFailure {
        if self.session.terminal() {
            self.pending_candidate_revision = None;
            self.state = HotReloadWatchState::Failed;
            HotReloadWatcherFailure::diagnostics(vec![Diagnostic::io(
                "SPX-HR401",
                "source-Agent activation acknowledgement is uncertain",
            )])
        } else {
            HotReloadWatcherFailure::diagnostics(vec![Diagnostic::io("SPX-HR401", ordinary)])
        }
    }

    fn confirm_activation_candidate(&mut self) -> Result<(), HotReloadWatcherFailure> {
        if self.observe_stop_request() {
            return Err(HotReloadWatcherFailure::stopped());
        }
        if self.state != HotReloadWatchState::Watching {
            return Err(HotReloadWatcherFailure::stopped());
        }
        let current = match admitted_revision(&self.manifest_path) {
            Ok(current) => current,
            Err(diagnostics) => {
                self.last_diagnostics = diagnostics.clone();
                self.rescan_required = true;
                self.mark_dirty();
                return Err(HotReloadWatcherFailure::diagnostics(diagnostics));
            }
        };
        if self.pending_candidate_revision.as_deref() != Some(current.project_revision()) {
            self.rescan_required = true;
            self.mark_dirty();
            return Err(HotReloadWatcherFailure::diagnostics(vec![Diagnostic::io(
                "SPX-HR401",
                "Project inputs changed after reload candidate admission",
            )]));
        }
        Ok(())
    }

    pub fn stop(&mut self) {
        self.stop_requested.store(true, Ordering::Release);
        self.dirty_generation = None;
        self.pending_candidate_revision = None;
        self.state = HotReloadWatchState::Stopped;
    }

    fn relevant(&self, path: &Path) -> bool {
        self.inputs.contains(path) || (self.rescan_required && strictly_beneath(&self.root, path))
    }

    fn observe_stop_request(&mut self) -> bool {
        if self.stop_requested.load(Ordering::Acquire) {
            self.dirty_generation = None;
            self.pending_candidate_revision = None;
            self.state = HotReloadWatchState::Stopped;
            true
        } else {
            false
        }
    }

    /// Reserve the sole dirty slot. Saturating an event identity would make a
    /// later event indistinguishable from an earlier one, so exhaustion is a
    /// terminal local refusal rather than a silently coalesced update.
    fn mark_dirty(&mut self) -> bool {
        let Some(next) = self.event_generation.checked_add(1) else {
            self.dirty_generation = None;
            self.state = HotReloadWatchState::Failed;
            self.last_diagnostics = vec![Diagnostic::io(
                "SPX-HR401",
                "hot reload watcher event generation is exhausted",
            )];
            return false;
        };
        self.event_generation = next;
        self.dirty_generation = Some(next);
        true
    }

    fn admit(&mut self, generation: u64) -> HotReloadWatcherUpdate {
        let admitted = with_authenticated_project(&self.manifest_path, |snapshot| {
            Ok((
                snapshot.authoritative_input_paths(),
                snapshot.retain_revision(),
            ))
        });
        let (paths, candidate) = match admitted {
            Ok(value) => value,
            Err(diagnostics) => {
                self.last_diagnostics = diagnostics;
                self.rescan_required = true;
                return HotReloadWatcherUpdate::CandidateRejected;
            }
        };
        #[cfg(test)]
        if let Some(hook) = self.after_admission.as_mut() {
            hook();
        }
        if self.observe_stop_request() {
            return HotReloadWatcherUpdate::Stopped;
        }
        // The loader's held-file recheck establishes the candidate at the end
        // of the first admission. A second ordinary admission closes the
        // interval before it reaches HR-01: a save that lands there is queued
        // as a newer generation instead of submitting the older candidate.
        let current = admitted_revision(&self.manifest_path);
        let current = match current {
            Ok(current) => current,
            Err(diagnostics) => {
                self.last_diagnostics = diagnostics;
                self.rescan_required = true;
                return HotReloadWatcherUpdate::CandidateRejected;
            }
        };
        if current.project_revision() != candidate.project_revision() {
            self.rescan_required = true;
            if !self.mark_dirty() {
                return HotReloadWatcherUpdate::CandidateRejected;
            }
            return HotReloadWatcherUpdate::Idle;
        }
        if self
            .dirty_generation
            .is_some_and(|newer| newer > generation)
        {
            return HotReloadWatcherUpdate::Idle;
        }
        let inputs = match bounded_inputs(paths) {
            Ok(inputs) => inputs,
            Err(diagnostics) => {
                self.last_diagnostics = diagnostics;
                self.rescan_required = true;
                return HotReloadWatcherUpdate::CandidateRejected;
            }
        };
        self.inputs = inputs;
        self.fingerprints = fingerprints(&self.inputs);
        self.rescan_required = false;
        if candidate.project_revision() == self.session.active_project_revision() {
            self.pending_candidate_revision = None;
            self.last_diagnostics.clear();
            return HotReloadWatcherUpdate::Unchanged;
        }
        match self.session.admit_candidate(candidate.clone()) {
            Ok(()) => {
                self.pending_candidate_revision = Some(candidate.project_revision().to_owned());
                self.last_diagnostics.clear();
                HotReloadWatcherUpdate::CandidateAdmitted
            }
            Err(HotReloadFailure { diagnostics, .. }) => {
                self.last_diagnostics = diagnostics;
                HotReloadWatcherUpdate::CandidateRejected
            }
        }
    }
}

fn strictly_beneath(root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    !relative
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
}

fn watcher_failure(failure: HotReloadFailure) -> HotReloadWatcherFailure {
    HotReloadWatcherFailure::diagnostics(failure.diagnostics)
}

fn admitted_revision(manifest_path: &Path) -> Result<Arc<ProjectRevision>, Vec<Diagnostic>> {
    with_authenticated_project(manifest_path, |snapshot| Ok(snapshot.retain_revision()))
}

fn bounded_inputs(paths: Vec<PathBuf>) -> Result<BTreeSet<PathBuf>, Vec<Diagnostic>> {
    let paths = paths.into_iter().collect::<BTreeSet<_>>();
    if paths.is_empty() || paths.len() > MAX_WATCHED_INPUTS {
        return Err(vec![Diagnostic::io(
            "SPX-HR401",
            "hot reload watcher input inventory is outside its bound",
        )]);
    }
    Ok(paths)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InputFingerprint {
    Present {
        bytes: u64,
        modified: Option<std::time::SystemTime>,
    },
    Missing,
}

fn fingerprints(paths: &BTreeSet<PathBuf>) -> BTreeMap<PathBuf, InputFingerprint> {
    paths
        .iter()
        .map(|path| {
            let value = match fs::symlink_metadata(path) {
                Ok(metadata) => InputFingerprint::Present {
                    bytes: metadata.len(),
                    modified: metadata.modified().ok(),
                },
                Err(_) => InputFingerprint::Missing,
            };
            (path.clone(), value)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::HotReloadDecision;
    use std::sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Mutex, MutexGuard,
    };
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    fn test_guard() -> MutexGuard<'static, ()> {
        TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
                "semaprax-hot-reload-watcher-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("src")).unwrap();
            let original =
                Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
            for relative in [
                "semaprax.toml",
                "src/app.spx",
                "src/core.spx",
                "src/tests.spx",
            ] {
                fs::copy(original.join(relative), root.join(relative)).unwrap();
            }
            Self(root)
        }
        fn manifest(&self) -> PathBuf {
            self.0.join(MANIFEST_FILE)
        }
        fn rewrite(&self, old: &str, new: &str) {
            let path = self.0.join("src/app.spx");
            let source = fs::read_to_string(&path).unwrap();
            let changed = source.replacen(old, new, 1);
            assert_ne!(source, changed);
            fs::write(
                &path,
                crate::format::canonical(
                    &crate::parse(&changed, Path::new("src/app.spx")).unwrap(),
                ),
            )
            .unwrap();
        }
        fn rewrite_raw(&self, path: &Path, old: &str, new: &str) {
            let source = fs::read_to_string(path).unwrap();
            let changed = source.replacen(old, new, 1);
            assert_ne!(source, changed);
            fs::write(path, changed).unwrap();
        }
        fn formatted_source(path: &Path, old: &str, new: &str) -> String {
            let source = fs::read_to_string(path).unwrap();
            let changed = source.replacen(old, new, 1);
            assert_ne!(source, changed);
            crate::format::canonical(&crate::parse(&changed, path).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn real_directory_burst_atomic_save_overflow_and_stop_are_coalesced() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        fixture.rewrite("multiply(6, 7)", "multiply(6, 8)");
        let app = fixture.0.join("src/app.spx");
        for _ in 0..3 {
            watcher.record(HotReloadWatchEvent::Modify(app.clone()));
        }
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        assert_eq!(
            watcher.session().plan().unwrap().decision(),
            HotReloadDecision::EligibleCodeReplacement
        );
        let temporary = fixture.0.join("src/app.atomic-save");
        fs::rename(&app, &temporary).unwrap();
        watcher.record(HotReloadWatchEvent::Rename {
            from: app.clone(),
            to: temporary.clone(),
        });
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateRejected);
        fs::rename(&temporary, &app).unwrap();
        watcher.record(HotReloadWatchEvent::Overflow);
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        watcher.stop();
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::Stopped);
    }
    #[test]
    fn newer_c_supersedes_b_before_explicit_activation() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let app = fixture.0.join("src/app.spx");
        fixture.rewrite("multiply(6, 7)", "multiply(6, 8)");
        watcher.record(HotReloadWatchEvent::Modify(app.clone()));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        let b = watcher.session().plan().unwrap();
        fixture.rewrite("multiply(6, 8)", "multiply(6, 9)");
        watcher.record(HotReloadWatchEvent::Modify(app));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        assert!(watcher.activate(b).is_err());
    }

    #[test]
    fn injected_source_agent_owner_cannot_claim_an_ordinary_reload_plan() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let app = fixture.0.join("src/app.spx");
        fixture.rewrite("multiply(6, 7)", "multiply(6, 8)");
        watcher.record(HotReloadWatchEvent::Modify(app));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        let plan = watcher.session().plan().unwrap();
        let invoked = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&invoked);
        assert!(watcher
            .activate_source_agent(plan, move |_, _| {
                observed.store(true, Ordering::Release);
                Ok(())
            })
            .is_err());
        assert!(
            !invoked.load(Ordering::Acquire),
            "an injected host cannot convert a code-only plan into Agent authority"
        );
    }

    #[test]
    fn atomic_save_and_delete_recreate_rescan_the_exact_admitted_input() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let app = fixture.0.join("src/app.spx");
        let staged = fixture.0.join("src/app.spx.save");
        fs::write(
            &staged,
            Fixture::formatted_source(&app, "multiply(6, 7)", "multiply(6, 8)"),
        )
        .unwrap();
        fs::rename(&staged, &app).unwrap();
        watcher.record(HotReloadWatchEvent::Rename {
            from: staged,
            to: app.clone(),
        });
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);

        let recreated = fixture.0.join("src/app.spx.recreated");
        fs::rename(&app, &recreated).unwrap();
        watcher.record(HotReloadWatchEvent::Remove(app.clone()));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateRejected);
        fs::rename(&recreated, &app).unwrap();
        watcher.record(HotReloadWatchEvent::Create(app));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
    }

    #[test]
    fn controlled_b_to_c_save_between_admission_and_commit_never_submits_b() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let app = fixture.0.join("src/app.spx");
        fixture.rewrite("multiply(6, 7)", "multiply(6, 8)");
        let c = Fixture::formatted_source(&app, "multiply(6, 8)", "multiply(6, 9)");
        let mut c = Some(c);
        let app_for_hook = app.clone();
        watcher.after_admission = Some(Box::new(move || {
            if let Some(source) = c.take() {
                fs::write(&app_for_hook, source).unwrap();
            }
        }));
        watcher.record(HotReloadWatchEvent::Modify(app));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::Idle);
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        assert_eq!(
            watcher.session().plan().unwrap().decision(),
            HotReloadDecision::EligibleCodeReplacement
        );
    }

    #[test]
    fn pre_activation_edit_rejects_historical_plan_and_queues_current_revision() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let app = fixture.0.join("src/app.spx");
        fixture.rewrite("multiply(6, 7)", "multiply(6, 8)");
        watcher.record(HotReloadWatchEvent::Modify(app.clone()));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        let b = watcher.session().plan().unwrap();
        fixture.rewrite("multiply(6, 8)", "multiply(6, 9)");
        assert!(watcher.activate(b).is_err());
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
    }

    #[test]
    fn external_stop_during_admission_clears_pending_work_and_releases_the_fixture() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let active = watcher.session().active_project_revision().to_owned();
        let control = watcher.control();
        watcher.after_admission = Some(Box::new(move || control.request_stop()));
        fixture.rewrite("multiply(6, 7)", "multiply(6, 8)");
        watcher.record(HotReloadWatchEvent::Modify(fixture.0.join("src/app.spx")));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::Stopped);
        assert_eq!(watcher.state(), HotReloadWatchState::Stopped);
        assert_eq!(watcher.session().active_project_revision(), active);
        drop(watcher);
        assert!(fs::remove_dir_all(&fixture.0).is_ok());
    }

    #[test]
    fn invalid_c_rejects_after_b_without_replacing_active_a_or_admitting_stale_b() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let active = watcher.session().active_project_revision().to_owned();
        let app = fixture.0.join("src/app.spx");
        fixture.rewrite("multiply(6, 7)", "multiply(6, 8)");
        watcher.record(HotReloadWatchEvent::Modify(app.clone()));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        let b = watcher.session().plan().unwrap();

        // C is deliberately malformed. The Project admission owner supplies
        // the diagnostic; the watcher must retain the complete active A.
        fixture.rewrite_raw(&app, "multiply(6, 8)", "multiply(6, )");
        watcher.record(HotReloadWatchEvent::Modify(app));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateRejected);
        assert_eq!(watcher.session().active_project_revision(), active);
        assert!(!watcher.last_diagnostics().is_empty());
        let failure = watcher.activate(b).unwrap_err();
        assert!(!failure.diagnostics.is_empty());
        assert_eq!(watcher.session().active_project_revision(), active);
    }

    #[test]
    fn manifest_membership_failure_is_reauthenticated_and_reported() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let manifest = fixture.manifest();
        fixture.rewrite_raw(
            &manifest,
            "\"src/tests.spx\"]",
            "\"src/tests.spx\", \"src/missing.spx\"]",
        );
        watcher.record(HotReloadWatchEvent::Modify(manifest));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateRejected);
        assert!(!watcher.last_diagnostics().is_empty());
        assert_eq!(watcher.state(), HotReloadWatchState::Watching);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_input_is_rejected_by_project_admission() {
        let _guard = test_guard();
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let app = fixture.0.join("src/app.spx");
        let retained = fixture.0.join("src/app-retained.spx");
        fs::rename(&app, &retained).unwrap();
        symlink(&retained, &app).unwrap();
        watcher.record(HotReloadWatchEvent::Modify(app));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateRejected);
        assert!(!watcher.last_diagnostics().is_empty());
        assert_eq!(watcher.state(), HotReloadWatchState::Watching);
    }

    #[test]
    fn event_generation_exhaustion_is_explicit_and_terminal() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        watcher.event_generation = u64::MAX;
        assert_eq!(
            watcher.record(HotReloadWatchEvent::Modify(fixture.0.join("src/app.spx"))),
            HotReloadWatcherUpdate::CandidateRejected
        );
        assert_eq!(watcher.state(), HotReloadWatchState::Failed);
        assert_eq!(watcher.last_diagnostics()[0].code, "SPX-HR401");
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::Stopped);
    }

    #[test]
    fn watcher_input_inventory_rejects_empty_and_first_over_bound() {
        let _guard = test_guard();
        let empty = bounded_inputs(Vec::new()).unwrap_err();
        assert_eq!(empty[0].code, "SPX-HR401");
        let over_bound = (0..=MAX_WATCHED_INPUTS)
            .map(|index| PathBuf::from(format!("/watcher-input-{index}")))
            .collect();
        let diagnostics = bounded_inputs(over_bound).unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-HR401");
    }

    #[test]
    fn same_byte_write_is_an_unchanged_revision_no_op() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let app = fixture.0.join("src/app.spx");
        let same = fs::read(&app).unwrap();
        fs::write(&app, same).unwrap();
        watcher.record(HotReloadWatchEvent::Modify(app));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::Unchanged);
        assert!(watcher.session().plan().is_err());
    }

    #[test]
    fn valid_repair_after_invalid_c_admits_once() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let app = fixture.0.join("src/app.spx");
        fixture.rewrite_raw(&app, "multiply(6, 7)", "multiply(6, )");
        watcher.record(HotReloadWatchEvent::Modify(app.clone()));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateRejected);
        let admissions = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&admissions);
        watcher.after_admission = Some(Box::new(move || {
            observed.fetch_add(1, Ordering::Relaxed);
        }));
        fixture.rewrite_raw(&app, "multiply(6, )", "multiply(6, 9)");
        watcher.record(HotReloadWatchEvent::Modify(app));
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::CandidateAdmitted);
        assert_eq!(admissions.load(Ordering::Relaxed), 1);
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::Idle);
        assert_eq!(admissions.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn derived_outputs_and_lexically_escaping_hints_do_not_start_a_rescan_loop() {
        let _guard = test_guard();
        let fixture = Fixture::new();
        let mut watcher = HotReloadWatcher::start(
            &fixture.manifest(),
            PreparedProjectInterpreterOptions::default(),
        )
        .unwrap();
        let derived = fixture.0.join("target/generated/app.spx");
        assert_eq!(
            watcher.record(HotReloadWatchEvent::Modify(derived)),
            HotReloadWatcherUpdate::Idle
        );
        assert_eq!(watcher.poll(), HotReloadWatcherUpdate::Idle);
        watcher.rescan_required = true;
        let escaping = fixture.0.join("src/../../outside.spx");
        assert_eq!(
            watcher.record(HotReloadWatchEvent::Create(escaping)),
            HotReloadWatcherUpdate::Idle
        );
        assert_eq!(watcher.dirty_generation, None);
    }
}
