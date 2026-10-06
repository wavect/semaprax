//! Host-owned strict-law selection for one authenticated Project root.
//!
//! The fixed sidecar is installed only by a quiescent trusted host. Candidate
//! manifests, source files and proof documents cannot choose or erase it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::assurance_manifest::law_set::native_proof::VerifiedLawProof;
use crate::assurance_manifest::law_set::{
    protected::ProtectedLawBaseline,
    strict::{self, RequiredLawEvidence, StrictLawPolicy},
    LawSet,
};
use crate::assurance_manifest::VerifiedProjectProof;
use crate::diagnostic::Diagnostic;
use crate::project_revision_store;

use super::authority::{HeldDirectory, HeldFile};
use super::{ProjectHostAccess, ProjectRevision, ProjectSnapshot};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const HOST_STRICT_LAW_DIRECTORY: &str = ".semaprax-strict-law";
pub const HOST_STRICT_LAW_SCHEMA: &str = "semaprax.host-strict-law-selection.v1";
const MARKER: &str = "SELECTED";
const REVISIONS: &str = "revisions";
const MAX_MARKER_BYTES: usize = 2 * crate::assurance_manifest::law_set::MAX_BYTES;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionDocument {
    schema: String,
    baseline_entry_digest: String,
    baseline_project_revision: String,
    proof_profile: String,
    baseline_law_set: String,
    requirements: BTreeMap<String, RequiredLawEvidence>,
    editable_bodies: Vec<String>,
    policy_digest: String,
    protection_digest: String,
}

fn refused(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-LW150", message)]
}

pub(super) fn selected_route_refused() -> Vec<Diagnostic> {
    refused("Project has a host-selected strict-law policy; generic Project routes are closed")
}

pub(super) fn strict_route_unselected() -> Vec<Diagnostic> {
    refused("Project has no host-selected strict-law policy")
}

fn canonical(document: &SelectionDocument) -> Result<String> {
    let mut text = serde_json::to_string(document)
        .map_err(|_| refused("host strict-law selection cannot be encoded"))?;
    text.push('\n');
    if text.len() > MAX_MARKER_BYTES {
        return Err(refused("host strict-law selection exceeds its byte bound"));
    }
    Ok(text)
}

fn require_private_directory(path: &Path) -> Result<HeldDirectory> {
    let directory = HeldDirectory::open(path.to_owned())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = fs::symlink_metadata(path).map_err(|error| {
            refused(format!("cannot inspect host strict-law directory: {error}"))
        })?;
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.permissions().mode() & 0o7777 != 0o700
        {
            return Err(refused(
                "host strict-law directory requires current-owner 0700 authority",
            ));
        }
    }
    Ok(directory)
}

fn require_private_file(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = fs::symlink_metadata(_path)
            .map_err(|error| refused(format!("cannot inspect host strict-law marker: {error}")))?;
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.permissions().mode() & 0o7777 != 0o600
        {
            return Err(refused(
                "host strict-law marker requires current-owner 0600 authority",
            ));
        }
    }
    Ok(())
}

fn require_inventory(control: &Path) -> Result<()> {
    let expected = BTreeSet::from([MARKER.into(), REVISIONS.into()]);
    let mut observed = BTreeSet::new();
    for entry in fs::read_dir(control)
        .map_err(|error| refused(format!("cannot inspect host strict-law inventory: {error}")))?
    {
        observed.insert(
            entry
                .map_err(|error| refused(format!("cannot inspect host strict-law entry: {error}")))?
                .file_name(),
        );
    }
    if observed != expected {
        return Err(refused(
            "host strict-law inventory is incomplete or contains unexpected entries",
        ));
    }
    Ok(())
}

/// A held host selection whose policy and original Project baseline have been
/// independently replayed. This value grants no execution or publication.
pub(super) struct SelectedStrictLaw {
    root: PathBuf,
    control: HeldDirectory,
    revisions: HeldDirectory,
    marker: HeldFile,
    marker_text: String,
    baseline: ProjectRevision,
    policy: StrictLawPolicy,
    protection: ProtectedLawBaseline,
    proof_profile: String,
}

impl SelectedStrictLaw {
    pub(super) fn open(root: &Path) -> Result<Option<Self>> {
        let control_path = root.join(HOST_STRICT_LAW_DIRECTORY);
        match fs::symlink_metadata(&control_path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(refused(format!(
                    "cannot inspect host strict-law selection: {error}"
                )))
            }
            Ok(_) => {}
        }
        let control = require_private_directory(&control_path)?;
        require_inventory(&control_path)?;
        let revisions_path = control_path.join(REVISIONS);
        let revisions = require_private_directory(&revisions_path)?;
        let marker_path = control_path.join(MARKER);
        require_private_file(&marker_path)?;
        let mut marker = HeldFile::open(marker_path, MAX_MARKER_BYTES)?;
        let marker_text = marker.utf8()?;
        let document: SelectionDocument = serde_json::from_str(&marker_text).map_err(|_| {
            refused("host strict-law selection has unknown, duplicate or malformed fields")
        })?;
        if document.schema != HOST_STRICT_LAW_SCHEMA || canonical(&document)? != marker_text {
            return Err(refused("host strict-law selection is not canonical"));
        }
        let baseline = project_revision_store::load(
            &revisions_path,
            &document.baseline_entry_digest,
            &document.baseline_project_revision,
        )?;
        let laws = LawSet::replay(
            &baseline,
            &document.proof_profile,
            &document.baseline_law_set,
        )?;
        let policy = StrictLawPolicy::new(laws.clone(), document.requirements)?;
        let protection = ProtectedLawBaseline::new(&baseline, laws, document.editable_bodies)?;
        if policy.digest() != document.policy_digest
            || protection.digest() != document.protection_digest
        {
            return Err(refused(
                "host strict-law policy or protected intent differs from its selected digest",
            ));
        }
        Ok(Some(Self {
            root: root.to_owned(),
            control,
            revisions,
            marker,
            marker_text,
            baseline,
            policy,
            protection,
            proof_profile: document.proof_profile,
        }))
    }

    pub(super) fn require(root: &Path) -> Result<Self> {
        Self::open(root)?.ok_or_else(|| refused("Project has no host-selected strict-law policy"))
    }

    pub(super) fn policy(&self) -> &StrictLawPolicy {
        &self.policy
    }
    pub(super) fn protection(&self) -> &ProtectedLawBaseline {
        &self.protection
    }
    pub(super) fn baseline(&self) -> &ProjectRevision {
        &self.baseline
    }
    pub(super) fn proof_profile(&self) -> &str {
        &self.proof_profile
    }

    pub(super) fn recheck(&mut self) -> Result<()> {
        self.control.recheck()?;
        self.revisions.recheck()?;
        self.marker.recheck()?;
        let current = Self::require(&self.root)?;
        if current.marker_text != self.marker_text
            || current.policy.digest() != self.policy.digest()
            || current.protection.digest() != self.protection.digest()
        {
            return Err(refused(
                "host strict-law selection changed during the operation",
            ));
        }
        Ok(())
    }
}

pub(super) fn require_unselected(root: &Path) -> Result<()> {
    if SelectedStrictLaw::open(root)?.is_some() {
        return Err(refused(
            "Project has a host-selected strict-law policy; use an admitted strict route",
        ));
    }
    Ok(())
}

/// Private, invocation-local admission retained across the ordinary Workspace
/// lock and final ACTIVE boundary. Wire evidence cannot construct this value.
pub(crate) struct StrictWorkspacePermit {
    selection: SelectedStrictLaw,
}

impl StrictWorkspacePermit {
    pub(crate) fn after_strict_gate(
        root: &Path,
        policy: &StrictLawPolicy,
        protection: &ProtectedLawBaseline,
    ) -> Result<Self> {
        let selection = SelectedStrictLaw::require(root)?;
        if selection.policy().digest() != policy.digest()
            || selection.protection().digest() != protection.digest()
        {
            return Err(refused(
                "candidate strict-law inputs differ from the host-selected policy or intent",
            ));
        }
        Ok(Self { selection })
    }

    pub(crate) fn recheck(&mut self, root: &Path) -> Result<()> {
        if self.selection.root != root {
            return Err(refused(
                "strict-law permit belongs to a different Project root",
            ));
        }
        self.selection.recheck()
    }
}

pub(crate) fn require_workspace_admission(
    root: &Path,
    permit: Option<&mut StrictWorkspacePermit>,
) -> Result<()> {
    match permit {
        Some(permit) => permit.recheck(root),
        None => require_unselected(root),
    }
}

/// Restricted inspection over a held Project. It exposes no executable
/// ProjectRevision, publication method, or retained ProjectSnapshot.
pub struct ProjectInspection<'a> {
    snapshot: &'a ProjectSnapshot,
}
impl ProjectInspection<'_> {
    pub fn project_revision(&self) -> &str {
        self.snapshot.project_revision()
    }
    pub fn semantic_graph(&self) -> &str {
        self.snapshot.semantic_graph()
    }
    pub fn check(&self) -> Result<()> {
        self.snapshot.check()
    }
    pub fn law_modules(&self) -> &[crate::assurance_manifest::law_set::LawModule] {
        self.snapshot.law_modules()
    }
    pub fn semantic_context(
        &self,
        target_kind: crate::workspace_analysis::WorkspaceAnalysisTargetKind,
        target: &str,
        options: crate::workspace_analysis::WorkspaceContextOptions,
    ) -> Result<String> {
        self.snapshot.semantic_context(target_kind, target, options)
    }
}

/// Ordinary syntax/type/graph inspection remains available on a selected
/// Project while protected execution and publication require strict admission.
pub fn with_authenticated_project_inspection<T>(
    manifest_path: &Path,
    operation: impl FnOnce(&ProjectInspection<'_>) -> Result<T>,
) -> Result<T> {
    super::with_snapshot_operation(
        super::load_snapshot_for_host_access(manifest_path, ProjectHostAccess::Inspection)?,
        |snapshot| operation(&ProjectInspection { snapshot }),
    )
}

/// An admitted invocation-local facade with no revision escape hatch.
pub struct StrictProjectSession<'a> {
    snapshot: &'a mut ProjectSnapshot,
}
impl StrictProjectSession<'_> {
    fn protected<T>(
        &mut self,
        operation: impl FnOnce(&mut ProjectSnapshot) -> Result<T>,
    ) -> Result<T> {
        self.snapshot.recheck()?;
        let result = operation(self.snapshot);
        let final_check = self.snapshot.recheck();
        match (result, final_check) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(errors), Ok(())) => Err(errors),
            (Ok(_), Err(errors)) => Err(errors),
            (Err(mut errors), Err(mut drift)) => {
                errors.append(&mut drift);
                Err(errors)
            }
        }
    }
    pub fn execute_entry(
        &mut self,
        options: &super::ProjectExecutionOptions,
    ) -> Result<super::ProjectExecution> {
        self.protected(|snapshot| snapshot.execute_entry(options))
    }
    pub fn execute_test(
        &mut self,
        options: &super::ProjectExecutionOptions,
    ) -> Result<super::ProjectExecution> {
        self.protected(|snapshot| snapshot.execute_test(options))
    }
    pub fn build_web_inline(&mut self, max_bytes: usize) -> Result<super::ProjectWebBuild> {
        self.protected(|snapshot| snapshot.build_web_inline(max_bytes))
    }
    pub fn build_npm_inline(&mut self, max_bytes: usize) -> Result<super::ProjectNpmBuild> {
        self.protected(|snapshot| snapshot.build_npm_inline(max_bytes))
    }
    pub fn build_web(&mut self, output: &Path) -> Result<()> {
        self.protected(|snapshot| snapshot.build_web(output))
    }
    pub fn build_npm(&mut self, output: &Path) -> Result<()> {
        self.protected(|snapshot| snapshot.build_npm(output))
    }
    pub fn build_oci(&mut self, output: &Path) -> Result<()> {
        self.protected(|snapshot| snapshot.build_oci(output))
    }
    pub fn build_native(&mut self, output: &Path) -> Result<()> {
        self.protected(|snapshot| snapshot.build_native(output))
    }
}

/// Rebuild current native law sources and exact opaque proof evidence against
/// the host-retained baseline before exposing one protected operation.
pub fn with_strict_authenticated_project<T>(
    manifest_path: &Path,
    project_proofs: &[VerifiedProjectProof],
    native_proofs: &[VerifiedLawProof],
    operation: impl FnOnce(&mut StrictProjectSession<'_>) -> Result<T>,
) -> Result<T> {
    let mut snapshot =
        super::load_snapshot_for_host_access(manifest_path, ProjectHostAccess::Strict)?;
    snapshot.recheck()?;
    let selection = snapshot
        .host_policy
        .as_ref()
        .expect("strict load retains selection");
    let current = LawSet::derive(
        &snapshot,
        selection.proof_profile(),
        snapshot.law_modules().to_vec(),
    )?;
    let report = strict::derive_with_native_proofs(
        &snapshot,
        &current,
        selection.policy(),
        project_proofs,
        native_proofs,
    )?;
    strict::require_with_native_proofs(
        &report,
        &snapshot,
        &current,
        selection.policy(),
        project_proofs,
        native_proofs,
    )?;
    super::with_snapshot_operation(snapshot, |snapshot| {
        operation(&mut StrictProjectSession { snapshot })
    })
}

/// Read-only selected-law diagnostic route. It authenticates the host's
/// independently held baseline and rejects specification edits before any
/// caller-supplied proof attempt. The callback may inspect current evidence
/// but receives no publication or source-mutation authority.
pub fn with_selected_law_diagnostics<T>(
    manifest_path: &Path,
    operation: impl FnOnce(&ProjectRevision, &LawSet, &StrictLawPolicy) -> Result<T>,
) -> Result<T> {
    let mut snapshot =
        super::load_snapshot_for_host_access(manifest_path, ProjectHostAccess::Strict)?;
    snapshot.recheck()?;
    let selection = snapshot
        .host_policy
        .as_ref()
        .expect("strict load retains host selection");
    let current = LawSet::derive(
        &snapshot,
        selection.proof_profile(),
        snapshot.law_modules().to_vec(),
    )?;
    selection
        .protection()
        .review(
            selection.baseline(),
            &snapshot,
            &current,
            snapshot.project_revision(),
        )?
        .require(None)?;
    super::with_snapshot_operation(snapshot, |snapshot| {
        let selection = snapshot
            .host_policy
            .as_ref()
            .expect("strict snapshot retains host selection");
        operation(snapshot, &current, selection.policy())
    })
}

/// Install once under explicitly held host filesystem authority. The caller
/// must quiesce concurrent Project and Workspace operations for this root.
/// The marker is created last; interruption before it remains fail-closed.
pub fn install_host_strict_law_policy(
    manifest_path: &Path,
    policy: &StrictLawPolicy,
    editable_bodies: Vec<String>,
) -> Result<()> {
    let mut snapshot: ProjectSnapshot = super::load_snapshot(manifest_path)?;
    let root = snapshot.root().to_owned();
    require_unselected(&root)?;
    let baseline = policy.baseline();
    LawSet::replay(&snapshot, baseline.proof_profile(), baseline.to_json())?;
    if snapshot.law_modules().is_empty()
        || LawSet::derive(
            &snapshot,
            baseline.proof_profile(),
            snapshot.law_modules().to_vec(),
        )?
        .to_json()
            != baseline.to_json()
    {
        return Err(refused("host strict-law installation requires the complete authenticated native law-source inventory"));
    }
    let protection =
        ProtectedLawBaseline::new(&snapshot, baseline.clone(), editable_bodies.clone())?;
    snapshot.recheck()?;
    let control_path = root.join(HOST_STRICT_LAW_DIRECTORY);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&control_path)
            .map_err(|error| {
                refused(format!("cannot create host strict-law directory: {error}"))
            })?;
        let revisions_path = control_path.join(REVISIONS);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&revisions_path)
            .map_err(|error| {
                refused(format!(
                    "cannot create host strict-law revision store: {error}"
                ))
            })?;
        let receipt = project_revision_store::persist(
            &revisions_path,
            &snapshot,
            snapshot.project_revision(),
        )?;
        let document = SelectionDocument {
            schema: HOST_STRICT_LAW_SCHEMA.into(),
            baseline_entry_digest: receipt.entry_digest().into(),
            baseline_project_revision: receipt.project_revision().into(),
            proof_profile: baseline.proof_profile().into(),
            baseline_law_set: baseline.to_json().into(),
            requirements: policy.requirements().clone(),
            editable_bodies,
            policy_digest: policy.digest().into(),
            protection_digest: protection.digest().into(),
        };
        let text = canonical(&document)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(control_path.join(MARKER))
            .map_err(|error| refused(format!("cannot create host strict-law marker: {error}")))?;
        file.write_all(text.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|error| refused(format!("cannot persist host strict-law marker: {error}")))?;
        drop(file);
        SelectedStrictLaw::require(&root)?.recheck()?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (snapshot, policy, protection, editable_bodies, control_path);
        Err(refused(
            "host strict-law installation requires the supported Unix host authority",
        ))
    }
}
