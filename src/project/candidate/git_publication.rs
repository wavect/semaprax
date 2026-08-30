//! Explicit canonical-source publication through one host-owned Git ref CAS.
//! Git trees are the atomic reader boundary; raw working trees are untouched.
use super::{wire, ProjectCandidate};
use crate::diagnostic::Diagnostic;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

mod process;
pub use process::CandidateGitProcessAuthority;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
pub const PROJECT_CANDIDATE_GIT_PUBLICATION_SCHEMA: &str =
    "semaprax.project-candidate-git-publication.v1";
const MAX_OBJECT: usize = 64 * 1024 * 1024;
const MAX_TREE: usize = 8 * 1024 * 1024;
const MAX_TOTAL: usize = 256 * 1024 * 1024;
const MAX_OBJECTS: usize = 4096;
const MAX_ENTRIES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateGitObjectKind {
    Blob,
    Tree,
    Commit,
}
impl CandidateGitObjectKind {
    fn name(self) -> &'static str {
        match self {
            Self::Blob => "blob",
            Self::Tree => "tree",
            Self::Commit => "commit",
        }
    }
}
pub struct CandidateGitObject {
    pub kind: CandidateGitObjectKind,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CandidateGitRepository {
    pub identity: String,
    pub bare: bool,
    pub sha256: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateGitRefUpdate {
    Updated,
    NotMatched,
}

/// Separate trusted host authority. Providers must bind one local bare SHA256
/// repository with no attached worktrees, enforce bounded local I/O/deadlines,
/// and implement atomic expected-old ref update. Errors after attempting CAS
/// may mean publication happened. These methods never receive evidence as an
/// authority token. The supplied process adapter implements actual Git writes.
pub trait CandidateGitAuthority {
    fn repository(&self) -> io::Result<CandidateGitRepository>;
    fn read_ref(&mut self, reference: &str) -> io::Result<Option<String>>;
    fn read_object(&mut self, oid: &str, max_bytes: usize) -> io::Result<CandidateGitObject>;
    fn write_object(
        &mut self,
        kind: CandidateGitObjectKind,
        bytes: &[u8],
        expected_oid: &str,
    ) -> io::Result<()>;
    fn compare_and_swap_ref(
        &mut self,
        reference: &str,
        expected_old: &str,
        new_commit: &str,
    ) -> io::Result<CandidateGitRefUpdate>;
}

pub struct CandidateGitTarget {
    repository: String,
    reference: String,
    base_commit: String,
    prefix: String,
}
impl CandidateGitTarget {
    pub fn new(
        repository: &str,
        reference: &str,
        base_commit: &str,
        project_prefix: &str,
    ) -> Result<Self> {
        if repository.is_empty()
            || repository.len() > 4096
            || repository.chars().any(char::is_control)
        {
            return Err(invalid(
                "Git repository identity must be bounded nonempty text",
            ));
        }
        valid_ref(reference)?;
        valid_oid(base_commit)?;
        if !project_prefix.is_empty() {
            valid_path(project_prefix)?;
        }
        Ok(Self {
            repository: repository.to_owned(),
            reference: reference.to_owned(),
            base_commit: base_commit.to_owned(),
            prefix: project_prefix.to_owned(),
        })
    }
    fn path(&self, path: &str) -> Result<Vec<Vec<u8>>> {
        valid_path(path)?;
        let path = if self.prefix.is_empty() {
            path.to_owned()
        } else {
            format!("{}/{path}", self.prefix)
        };
        valid_path(&path)?;
        Ok(path
            .split('/')
            .map(|part| part.as_bytes().to_vec())
            .collect())
    }
}
/// Explicit deterministic host identity and UTC timestamp; no ambient Git user,
/// clock, signer, hook, or editor is consulted. Message includes one final LF.
pub struct CandidateGitCommitMetadata {
    name: String,
    email: String,
    seconds: u64,
    message: String,
}
impl CandidateGitCommitMetadata {
    pub fn new(name: &str, email: &str, unix_seconds: u64, message: &str) -> Result<Self> {
        if name.is_empty()
            || name.len() > 256
            || name
                .chars()
                .any(|ch| ch.is_control() || matches!(ch, '<' | '>'))
            || name.trim() != name
            || email.is_empty()
            || email.len() > 256
            || !email.is_ascii()
            || email.bytes().any(|byte| {
                byte.is_ascii_whitespace() || byte.is_ascii_control() || matches!(byte, b'<' | b'>')
            })
            || unix_seconds > i64::MAX as u64
            || message.is_empty()
            || message.len() > 16_384
            || !message.ends_with('\n')
            || message.ends_with("\n\n")
            || message
                .chars()
                .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
        {
            return Err(invalid(
                "Git author, UTC timestamp, or LF-terminated commit message is invalid",
            ));
        }
        Ok(Self {
            name: name.to_owned(),
            email: email.to_owned(),
            seconds: unix_seconds,
            message: message.to_owned(),
        })
    }
    fn commit(&self, tree: &str, parent: &str) -> Vec<u8> {
        format!("tree {tree}\nparent {parent}\nauthor {} <{}> {} +0000\ncommitter {} <{}> {} +0000\n\n{}",self.name,self.email,self.seconds,self.name,self.email,self.seconds,self.message).into_bytes()
    }
}

/// Replay approved semantics from freshly authenticated raw Project sources,
/// authenticate all original Git blobs, build immutable Git objects, and perform
/// exactly one expected-old branch-ref pivot through the separate host authority.
pub fn apply_candidate_git_publication<A: CandidateGitAuthority>(
    candidate: &ProjectCandidate,
    approved_candidate_digest: &str,
    project_manifest: &Path,
    target: &CandidateGitTarget,
    metadata: &CandidateGitCommitMetadata,
    authority: &mut A,
) -> Result<String> {
    candidate.require_candidate(approved_candidate_digest)?;
    let mut snapshot = crate::project::load_snapshot(project_manifest)?;
    if !project_manifest.is_absolute() || project_manifest != snapshot.root().join("semaprax.toml")
    {
        return Err(invalid(
            "Git publication requires the exact authenticated absolute Project manifest path",
        ));
    }
    if snapshot.project_revision() != candidate.base.project_revision()
        || snapshot.manifest().to_canonical_toml() != candidate.base.manifest().to_canonical_toml()
        || candidate.revision.manifest().to_canonical_toml()
            != candidate.base.manifest().to_canonical_toml()
    {
        return Err(stale(
            "Git publication Project sources/manifest differ from the candidate original base",
        ));
    }
    let repository = authority
        .repository()
        .map_err(|_| host("cannot authenticate host Git repository"))?;
    if repository.identity != target.repository || !repository.bare || !repository.sha256 {
        return Err(invalid("Git publication requires the exact host-selected bare SHA256 repository without worktrees"));
    }
    let mut budget = Budget {
        objects: 0,
        bytes: 0,
        entries: 0,
        start: Instant::now(),
    };
    if authority
        .read_ref(&target.reference)
        .map_err(|_| host("cannot read host-selected Git ref"))?
        .as_deref()
        != Some(target.base_commit.as_str())
    {
        return Err(stale(
            "Git branch does not match the host-selected expected commit",
        ));
    }
    let replay = ProjectCandidate::replay(
        snapshot.retain_revision(),
        snapshot.project_revision(),
        &candidate.changes,
        candidate.to_json().as_bytes(),
    )?;
    if replay.candidate_digest() != approved_candidate_digest {
        return Err(stale(
            "Git publication candidate replay disagrees with host approval",
        ));
    }
    snapshot.recheck()?;
    let original = read(
        authority,
        &target.base_commit,
        CandidateGitObjectKind::Commit,
        1024 * 1024,
        &mut budget,
    )?;
    let root = commit_tree(&original)?;
    let manifest = snapshot.manifest().to_canonical_toml();
    let mut changes = vec![Change {
        path: target.path("semaprax.toml")?,
        before: manifest.as_bytes(),
        after: None,
    }];
    let mut changed_paths = Vec::new();
    if candidate.base.sources().len() != replay.revision.sources().len() {
        return Err(invalid(
            "Git publication cannot change Project source inventory",
        ));
    }
    for (before, after) in candidate
        .base
        .sources()
        .iter()
        .zip(replay.revision.sources())
    {
        if before.path() != after.path() {
            return Err(invalid(
                "Git publication cannot change Project source paths",
            ));
        }
        let changed = before.source() != after.source();
        let path = target.path(before.path())?;
        if changed {
            changed_paths.push(
                path.iter()
                    .map(|part| std::str::from_utf8(part).expect("validated UTF8 source path"))
                    .collect::<Vec<_>>()
                    .join("/"),
            );
        }
        changes.push(Change {
            path,
            before: before.source().as_bytes(),
            after: changed.then_some(after.source().as_bytes()),
        });
    }
    if changed_paths.is_empty() {
        return Err(invalid(
            "Git publication requires at least one changed canonical source",
        ));
    }
    let mut objects = Vec::new();
    let mut written_bytes = 0;
    let tree = update_tree(
        authority,
        &root,
        &changes,
        0,
        &mut objects,
        &mut written_bytes,
        &mut budget,
    )?;
    let commit_bytes = metadata.commit(&tree, &target.base_commit);
    let commit = object_oid(CandidateGitObjectKind::Commit, &commit_bytes);
    stage(
        &mut objects,
        &mut written_bytes,
        CandidateGitObjectKind::Commit,
        commit_bytes,
    )?;
    // Render the complete successful receipt before any publication opportunity.
    let receipt = wire::render(
        json!({"schema":PROJECT_CANDIDATE_GIT_PUBLICATION_SCHEMA,"repository":target.repository,"reference":target.reference,"previous_commit":target.base_commit,"published_commit":commit,"tree":tree,"approved_candidate_digest":approved_candidate_digest,"base_project_revision":candidate.base.project_revision(),"candidate_project_revision":replay.revision.project_revision(),"updated_source_paths":changed_paths,"publication":"git_branch_ref_compare_and_swap","git_object_format":"sha256","working_tree_rewritten":false,"project_manifest_changed":false,"managed_active_changed":false,"source_authority":"explicit_host_git_ref_authority","tests":"not_run","nonclaims":["no_atomic_raw_working_tree_rewrite","no_network_push_or_remote_publication","no_signature_or_approval_service","unreachable_objects_may_remain_after_failure"]}),
        1024 * 1024,
    )?;
    snapshot.recheck()?;
    for object in &objects {
        budget.fuel()?;
        authority
            .write_object(object.kind, &object.bytes, &object.oid)
            .map_err(|_| {
                host("Git immutable object write failed; no ref pivot has been requested")
            })?;
    }
    snapshot.recheck()?;
    if authority
        .repository()
        .map_err(|_| host("Git repository recheck failed"))?
        != repository
    {
        return Err(stale(
            "host Git repository identity changed before publication",
        ));
    }
    budget.fuel()?;
    match authority.compare_and_swap_ref(&target.reference, &target.base_commit, &commit) {
        Ok(CandidateGitRefUpdate::NotMatched) => {
            return Err(stale("Git branch changed before its atomic ref update"))
        }
        Err(_) => {
            return Err(uncertain(
                &target.reference,
                &commit,
                "Git ref update returned an uncertain outcome",
            ))
        }
        Ok(CandidateGitRefUpdate::Updated) => {}
    }
    if snapshot.recheck().is_err() {
        return Err(uncertain(
            &target.reference,
            &commit,
            "Project inputs changed after Git publication",
        ));
    }
    if authority.repository().ok().as_ref() != Some(&repository)
        || authority
            .read_ref(&target.reference)
            .ok()
            .flatten()
            .as_deref()
            != Some(commit.as_str())
    {
        return Err(uncertain(
            &target.reference,
            &commit,
            "Git publication occurred but its final repository/ref observation disagreed",
        ));
    }
    Ok(receipt)
}
struct Change<'a> {
    path: Vec<Vec<u8>>,
    before: &'a [u8],
    after: Option<&'a [u8]>,
}
struct Staged {
    kind: CandidateGitObjectKind,
    oid: String,
    bytes: Vec<u8>,
}
struct Budget {
    objects: usize,
    bytes: usize,
    entries: usize,
    start: Instant,
}
impl Budget {
    fn fuel(&self) -> Result<()> {
        if self.objects > MAX_OBJECTS
            || self.bytes > MAX_TOTAL
            || self.entries > MAX_ENTRIES
            || self.start.elapsed() > Duration::from_secs(60)
        {
            Err(capacity(
                "Git publication exceeds its object, tree, byte, or elapsed-work bounds",
            ))
        } else {
            Ok(())
        }
    }
}
fn read<A: CandidateGitAuthority>(
    authority: &mut A,
    oid: &str,
    kind: CandidateGitObjectKind,
    limit: usize,
    budget: &mut Budget,
) -> Result<Vec<u8>> {
    valid_oid(oid)?;
    budget.objects += 1;
    budget.fuel()?;
    let object = authority
        .read_object(oid, limit)
        .map_err(|_| host("cannot read bounded Git object"))?;
    if object.bytes.len() > limit || object.bytes.len() > MAX_OBJECT {
        return Err(capacity("Git object exceeds its byte bound"));
    }
    budget.bytes = budget.bytes.saturating_add(object.bytes.len());
    budget.fuel()?;
    if object.kind != kind || object_oid(kind, &object.bytes) != oid {
        return Err(stale(
            "Git object type or SHA256 content identity disagrees",
        ));
    }
    Ok(object.bytes)
}
fn stage(
    objects: &mut Vec<Staged>,
    total: &mut usize,
    kind: CandidateGitObjectKind,
    bytes: Vec<u8>,
) -> Result<String> {
    *total = total.saturating_add(bytes.len());
    if bytes.len() > MAX_OBJECT || *total > MAX_TOTAL || objects.len() >= MAX_OBJECTS {
        return Err(capacity("Git staged immutable objects exceed their bound"));
    }
    let oid = object_oid(kind, &bytes);
    objects.push(Staged {
        kind,
        oid: oid.clone(),
        bytes,
    });
    Ok(oid)
}
fn object_oid(kind: CandidateGitObjectKind, bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(format!("{} {}\0", kind.name(), bytes.len()).as_bytes());
    hash.update(bytes);
    format!("{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}
fn commit_tree(bytes: &[u8]) -> Result<String> {
    let end = bytes
        .iter()
        .position(|byte| *byte == b'\n')
        .ok_or_else(|| invalid("Git base commit lacks its root tree header"))?;
    let line =
        std::str::from_utf8(&bytes[..end]).map_err(|_| invalid("Git tree header is not UTF8"))?;
    let tree = line
        .strip_prefix("tree ")
        .ok_or_else(|| invalid("Git commit must start with its root tree header"))?;
    valid_oid(tree)?;
    Ok(tree.to_owned())
}
struct Entry {
    mode: &'static str,
    name: Vec<u8>,
    oid: String,
}
fn tree_order(entry: &Entry) -> Vec<u8> {
    let mut key = entry.name.clone();
    key.push(if entry.mode == "40000" { b'/' } else { 0 });
    key
}
fn parse_tree(bytes: &[u8], budget: &mut Budget) -> Result<Vec<Entry>> {
    let mut entries = Vec::new();
    let mut offset = 0;
    let mut names = BTreeSet::new();
    let mut previous = None;
    while offset < bytes.len() {
        budget.entries += 1;
        budget.fuel()?;
        let space = bytes[offset..]
            .iter()
            .position(|byte| *byte == b' ')
            .map(|index| offset + index)
            .ok_or_else(|| invalid("Git tree mode is unterminated"))?;
        let mode = match &bytes[offset..space] {
            b"40000" => "40000",
            b"100644" => "100644",
            b"100755" => "100755",
            b"120000" => "120000",
            b"160000" => "160000",
            _ => return Err(invalid("Git tree mode is not a canonical supported mode")),
        };
        let nul = bytes[space + 1..]
            .iter()
            .position(|byte| *byte == 0)
            .map(|index| space + 1 + index)
            .ok_or_else(|| invalid("Git tree name is unterminated"))?;
        let name = &bytes[space + 1..nul];
        if name.is_empty()
            || name.len() > 4096
            || name == b"."
            || name == b".."
            || name.contains(&b'/')
            || !names.insert(name.to_vec())
        {
            return Err(invalid("Git tree name is invalid or duplicated"));
        }
        let end = nul
            .checked_add(33)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| invalid("Git SHA256 tree entry is truncated"))?;
        let oid = bytes[nul + 1..end]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let entry = Entry {
            mode,
            name: name.to_vec(),
            oid,
        };
        let order = tree_order(&entry);
        if previous
            .as_ref()
            .is_some_and(|prior: &Vec<u8>| prior >= &order)
        {
            return Err(invalid(
                "Git tree entries are not in canonical Git name order",
            ));
        }
        previous = Some(order);
        entries.push(entry);
        offset = end;
    }
    Ok(entries)
}
fn encode_tree(entries: &[Entry]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend_from_slice(entry.mode.as_bytes());
        bytes.push(b' ');
        bytes.extend_from_slice(&entry.name);
        bytes.push(0);
        for offset in (0..64).step_by(2) {
            bytes.push(
                u8::from_str_radix(&entry.oid[offset..offset + 2], 16)
                    .map_err(|_| invalid("Git staged object ID is invalid"))?,
            );
        }
    }
    if bytes.len() > MAX_TREE {
        return Err(capacity("Git result tree exceeds its byte bound"));
    }
    Ok(bytes)
}
fn update_tree<A: CandidateGitAuthority>(
    authority: &mut A,
    oid: &str,
    changes: &[Change<'_>],
    depth: usize,
    objects: &mut Vec<Staged>,
    written: &mut usize,
    budget: &mut Budget,
) -> Result<String> {
    if depth > 64 {
        return Err(capacity("Git project path nesting exceeds its bound"));
    }
    let bytes = read(
        authority,
        oid,
        CandidateGitObjectKind::Tree,
        MAX_TREE,
        budget,
    )?;
    let mut entries = parse_tree(&bytes, budget)?;
    let mut groups = BTreeMap::<&[u8], Vec<&Change<'_>>>::new();
    for change in changes {
        let head = change
            .path
            .first()
            .ok_or_else(|| invalid("empty Git project path"))?;
        groups.entry(head).or_default().push(change);
    }
    for (name, group) in groups {
        let entry = entries
            .iter_mut()
            .find(|entry| entry.name == name)
            .ok_or_else(|| stale("Git base lacks an original Project path"))?;
        if group.iter().any(|change| change.path.len() == 1) {
            if group.len() != 1
                || group[0].path.len() != 1
                || !matches!(entry.mode, "100644" | "100755")
            {
                return Err(invalid(
                    "Git Project sources must be distinct ordinary blob paths",
                ));
            }
            let change = group[0];
            let blob = read(
                authority,
                &entry.oid,
                CandidateGitObjectKind::Blob,
                MAX_OBJECT,
                budget,
            )?;
            if blob != change.before {
                return Err(stale(
                    "Git base blob differs from the authenticated original Project bytes",
                ));
            }
            if let Some(after) = change.after {
                entry.oid = stage(
                    objects,
                    written,
                    CandidateGitObjectKind::Blob,
                    after.to_vec(),
                )?;
            }
        } else {
            if entry.mode != "40000" {
                return Err(invalid("Git Project path traverses a non-tree entry"));
            }
            let children = group
                .iter()
                .map(|change| Change {
                    path: change.path[1..].to_vec(),
                    before: change.before,
                    after: change.after,
                })
                .collect::<Vec<_>>();
            entry.oid = update_tree(
                authority,
                &entry.oid,
                &children,
                depth + 1,
                objects,
                written,
                budget,
            )?;
        }
    }
    let after = encode_tree(&entries)?;
    if after == bytes {
        Ok(oid.to_owned())
    } else {
        stage(objects, written, CandidateGitObjectKind::Tree, after)
    }
}
fn valid_oid(oid: &str) -> Result<()> {
    if oid.len() != 64
        || !oid
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Err(invalid(
            "Git publication requires canonical 64-hex SHA256 object IDs",
        ))
    } else {
        Ok(())
    }
}
fn valid_ref(reference: &str) -> Result<()> {
    let name = reference
        .strip_prefix("refs/heads/")
        .ok_or_else(|| invalid("Git publication requires an explicit refs/heads branch"))?;
    if name.is_empty()
        || reference.len() > 256
        || name.contains("..")
        || name.split('/').any(|part| {
            part.is_empty()
                || part.starts_with('.')
                || part.ends_with('.')
                || part.ends_with(".lock")
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        })
    {
        Err(invalid(
            "Git publication branch name is outside its bounded grammar",
        ))
    } else {
        Ok(())
    }
}
fn valid_path(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 4096
        || path.chars().any(char::is_control)
        || path.contains('\\')
        || path.split('/').count() > 64
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".." | ".git"))
    {
        Err(invalid(
            "Git project prefix/source path is not a bounded relative path",
        ))
    } else {
        Ok(())
    }
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G263", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G264", message)]
}
fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G265", message)]
}
fn host(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G266", message)]
}
fn uncertain(reference: &str, commit: &str, message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G267",format!("{message}; ref {reference}, prepared commit {commit}; publication may have occurred: inspect the ref and do not retry blindly"))]
}
