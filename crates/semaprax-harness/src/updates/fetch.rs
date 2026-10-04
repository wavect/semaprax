//! Transport boundary of the resolver. The harness has no remote HTTP client
//! (`host/http.rs` is loopback-only), so remote reads go through a pluggable
//! [`Fetcher`]: the host's own GitHub CLI ([`super::gh`]) or a local fixture
//! ([`super::fixture`]). A fetcher only reads; it never writes or executes.

use super::d;
use crate::diag::HarnessResult;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub draft: bool,
    pub prerelease: bool,
}

/// What a tag name points at: a commit directly, or an annotated tag object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagRef {
    pub object_sha: String,
    pub annotated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeEntry {
    pub path: String,
    /// Git mode string (`100644`, `100755`, `120000` symlink, `160000` submodule).
    pub mode: String,
    pub is_blob: bool,
    pub sha: String,
    pub size: u64,
}

#[derive(Clone, Debug, Default)]
pub struct Tree {
    pub entries: Vec<TreeEntry>,
    pub truncated: bool,
}

pub trait Fetcher {
    /// Newest-first published releases (bounded by the implementation).
    fn releases(&self, repo: &str) -> HarnessResult<Vec<Release>>;
    fn tag_ref(&self, repo: &str, tag: &str) -> HarnessResult<Option<TagRef>>;
    /// Underlying commit of an annotated tag object.
    fn peel_tag(&self, repo: &str, object_sha: &str) -> HarnessResult<String>;
    /// Resolve a branch name to a commit. The only call that takes a mutable
    /// name; every later read is by commit sha.
    fn branch_head(&self, repo: &str, branch: &str) -> HarnessResult<String>;
    /// Recursive tree of one immutable commit.
    fn tree(&self, repo: &str, commit: &str) -> HarnessResult<Tree>;
    /// Raw blob bytes by git blob sha, at most `max` bytes.
    fn blob(&self, repo: &str, sha: &str, max: u64) -> HarnessResult<Vec<u8>>;
    /// Commits or digests the origin has withdrawn (empty when unknown).
    fn revoked(&self, _repo: &str) -> HarnessResult<Vec<String>> {
        Ok(Vec::new())
    }
}

/// `https://github.com/<owner>/<repo>` -> `(owner, repo)`. Anything else is
/// refused: v1 admits exactly one origin host and no redirects off it.
pub fn parse_repo(url: &str) -> HarnessResult<(String, String)> {
    let bad = || {
        d(
            "SPX-HPU016",
            format!("`{url}` is not an https://github.com/<owner>/<repo> origin"),
        )
    };
    let rest = url.strip_prefix("https://github.com/").ok_or_else(bad)?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, name) = rest.split_once('/').ok_or_else(bad)?;
    let ok = |s: &str| {
        !s.is_empty()
            && s != "."
            && s != ".."
            && s.len() <= 100
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    if !ok(owner) || !ok(name) {
        return Err(bad());
    }
    Ok((owner.to_string(), name.to_string()))
}

pub fn is_sha(s: &str) -> bool {
    s.len() == 40 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}
