//! Explicit-development watcher adapter for authenticated Project reloads.
//!
//! File events are hints. This module has no native watcher thread: a selected
//! client injects portable events or calls `poll`. Every dirty generation goes
//! through ordinary authenticated Project admission before it reaches HR-01.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{
    with_authenticated_project, HotReloadFailure, HotReloadPlan, HotReloadSession,
    PreparedProjectInterpreterOptions, ProjectRevision,
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
/// notifications use `record` and share the same deterministic coalescer.
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

    /// Coalesce one untrusted hint. Paths outside the exact inventory are
    /// ignored, except while a manifest change needs fresh admission: a newly
    /// named input cannot be in the older inventory yet.
    pub fn record(&mut self, event: HotReloadWatchEvent) -> HotReloadWatcherUpdate {
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

    /// Do one bounded metadata scan then at most one authenticated admission.
    pub fn poll(&mut self) -> HotReloadWatcherUpdate {
        if self.state != HotReloadWatchState::Watching {
            return HotReloadWatcherUpdate::Stopped;
        }
        if self.fingerprints != fingerprints(&self.inputs) {
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
        self.session.activate(plan).map_err(watcher_failure)?;
        self.pending_candidate_revision = None;
        Ok(())
    }

    pub fn stop(&mut self) {
        self.dirty_generation = None;
        self.pending_candidate_revision = None;
        self.state = HotReloadWatchState::Stopped;
    }

    fn relevant(&self, path: &Path) -> bool {
        self.inputs.contains(path) || (self.rescan_required && path.starts_with(&self.root))
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
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
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
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn real_directory_burst_atomic_save_overflow_and_stop_are_coalesced() {
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
    fn invalid_c_rejects_after_b_without_replacing_active_a_or_admitting_stale_b() {
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
        let empty = bounded_inputs(Vec::new()).unwrap_err();
        assert_eq!(empty[0].code, "SPX-HR401");
        let over_bound = (0..=MAX_WATCHED_INPUTS)
            .map(|index| PathBuf::from(format!("/watcher-input-{index}")))
            .collect();
        let diagnostics = bounded_inputs(over_bound).unwrap_err();
        assert_eq!(diagnostics[0].code, "SPX-HR401");
    }
}
