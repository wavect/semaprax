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
            self.event_generation = self.event_generation.saturating_add(1);
            self.dirty_generation = Some(self.event_generation);
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
            self.event_generation = self.event_generation.saturating_add(1);
            self.dirty_generation = Some(self.event_generation);
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
        let current =
            admitted_revision(&self.manifest_path).map_err(HotReloadWatcherFailure::diagnostics)?;
        if self.pending_candidate_revision.as_deref() != Some(current.project_revision()) {
            self.rescan_required = true;
            self.event_generation = self.event_generation.saturating_add(1);
            self.dirty_generation = Some(self.event_generation);
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
                self.state = HotReloadWatchState::Failed;
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
}
