//! Local simulated upstream: an in-memory [`MemoryFetcher`] (tests, offline
//! demos) and a [`DirectoryFetcher`] that loads the same model from disk.
//! Every call is logged so tests can assert exactly which requests were made.

use super::d;
use super::fetch::{Fetcher, Release, TagRef, Tree, TreeEntry};
use super::sha1::{git_blob_sha, sha1};
use crate::diag::HarnessResult;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub struct FixtureFile {
    pub path: String,
    pub mode: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default)]
pub struct FixtureRepo {
    pub releases: Vec<Release>,
    /// tag -> (object sha, annotated)
    pub tags: BTreeMap<String, (String, bool)>,
    /// annotated tag object sha -> commit
    pub peeled: BTreeMap<String, String>,
    pub branches: BTreeMap<String, String>,
    pub commits: BTreeMap<String, Vec<FixtureFile>>,
    pub revoked: Vec<String>,
}

#[derive(Default)]
struct Inner {
    repos: BTreeMap<String, FixtureRepo>,
    requests: Vec<String>,
    outage: Option<String>,
    truncate: BTreeSet<String>,
    corrupt: BTreeSet<String>,
    tree_truncated: bool,
}

#[derive(Default)]
pub struct MemoryFetcher {
    inner: Mutex<Inner>,
}

/// Deterministic 40-hex id for fixtures.
pub fn fake_sha(seed: &str) -> String {
    super::sha1::hex(&sha1(seed.as_bytes()))
}

impl MemoryFetcher {
    pub fn new() -> Self {
        Self::default()
    }

    fn with<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        f(&mut self.inner.lock().unwrap())
    }

    pub fn repo_mut<R>(&self, repo: &str, f: impl FnOnce(&mut FixtureRepo) -> R) -> R {
        self.with(|i| f(i.repos.entry(repo.to_string()).or_default()))
    }

    /// Add a commit with regular files (`100644`).
    pub fn add_commit(&self, repo: &str, commit: &str, files: &[(&str, &str)]) {
        let files = files
            .iter()
            .map(|(p, c)| FixtureFile {
                path: p.to_string(),
                mode: "100644".into(),
                bytes: c.as_bytes().to_vec(),
            })
            .collect();
        self.repo_mut(repo, |r| {
            r.commits.insert(commit.to_string(), files);
        });
    }

    pub fn add_raw_file(&self, repo: &str, commit: &str, file: FixtureFile) {
        self.repo_mut(repo, |r| {
            r.commits.entry(commit.to_string()).or_default().push(file)
        });
    }

    /// Publish a release whose tag points at `commit` (annotated or lightweight).
    pub fn add_release(&self, repo: &str, tag: &str, commit: &str, annotated: bool) {
        self.repo_mut(repo, |r| {
            r.releases.insert(
                0,
                Release {
                    tag: tag.into(),
                    draft: false,
                    prerelease: false,
                },
            );
            set_tag(r, tag, commit, annotated);
        });
    }

    pub fn add_prerelease(&self, repo: &str, tag: &str, commit: &str) {
        self.repo_mut(repo, |r| {
            r.releases.insert(
                0,
                Release {
                    tag: tag.into(),
                    draft: false,
                    prerelease: true,
                },
            );
            set_tag(r, tag, commit, false);
        });
    }

    /// Re-point an existing tag at another commit (a moved tag).
    pub fn move_tag(&self, repo: &str, tag: &str, commit: &str) {
        self.repo_mut(repo, |r| {
            let annotated = r.tags.get(tag).is_some_and(|t| t.1);
            set_tag(r, tag, commit, annotated)
        });
    }

    pub fn set_branch(&self, repo: &str, branch: &str, commit: &str) {
        self.repo_mut(repo, |r| {
            r.branches.insert(branch.into(), commit.into());
        });
    }

    pub fn revoke(&self, repo: &str, id: &str) {
        self.repo_mut(repo, |r| r.revoked.push(id.into()));
    }

    /// Every request fails with this message until cleared (outage, rate limit).
    pub fn set_outage(&self, msg: Option<&str>) {
        self.with(|i| i.outage = msg.map(String::from));
    }

    /// Serve this blob cut in half (truncated download).
    pub fn truncate_blob(&self, sha: &str) {
        self.with(|i| i.truncate.insert(sha.into()));
    }

    /// Serve different bytes of the same length under this blob sha.
    pub fn corrupt_blob(&self, sha: &str) {
        self.with(|i| i.corrupt.insert(sha.into()));
    }

    pub fn truncate_trees(&self, on: bool) {
        self.with(|i| i.tree_truncated = on);
    }

    pub fn requests(&self) -> Vec<String> {
        self.with(|i| i.requests.clone())
    }

    pub fn request_count(&self) -> usize {
        self.with(|i| i.requests.len())
    }

    pub fn clear_requests(&self) {
        self.with(|i| i.requests.clear());
    }

    fn call<R>(
        &self,
        log: String,
        f: impl FnOnce(&mut Inner) -> HarnessResult<R>,
    ) -> HarnessResult<R> {
        self.with(|i| {
            i.requests.push(log);
            if let Some(m) = &i.outage {
                return Err(d("SPX-HPU009", format!("upstream unavailable: {m}")));
            }
            f(i)
        })
    }
}

fn set_tag(r: &mut FixtureRepo, tag: &str, commit: &str, annotated: bool) {
    if annotated {
        let obj = fake_sha(&format!("tag-object:{tag}:{commit}"));
        r.peeled.insert(obj.clone(), commit.to_string());
        r.tags.insert(tag.into(), (obj, true));
    } else {
        r.tags.insert(tag.into(), (commit.into(), false));
    }
}

fn repo<'a>(i: &'a Inner, url: &str) -> HarnessResult<&'a FixtureRepo> {
    i.repos
        .get(url)
        .ok_or_else(|| d("SPX-HPU009", format!("fixture has no repository `{url}`")))
}

impl Fetcher for MemoryFetcher {
    fn releases(&self, url: &str) -> HarnessResult<Vec<Release>> {
        self.call(format!("releases {url}"), |i| {
            Ok(repo(i, url)?.releases.clone())
        })
    }

    fn tag_ref(&self, url: &str, tag: &str) -> HarnessResult<Option<TagRef>> {
        self.call(format!("tag-ref {url} {tag}"), |i| {
            Ok(repo(i, url)?.tags.get(tag).map(|(o, a)| TagRef {
                object_sha: o.clone(),
                annotated: *a,
            }))
        })
    }

    fn peel_tag(&self, url: &str, sha: &str) -> HarnessResult<String> {
        self.call(format!("peel-tag {url} {sha}"), |i| {
            repo(i, url)?
                .peeled
                .get(sha)
                .cloned()
                .ok_or_else(|| d("SPX-HPU009", format!("unknown tag object {sha}")))
        })
    }

    fn branch_head(&self, url: &str, branch: &str) -> HarnessResult<String> {
        self.call(format!("branch-head {url} {branch}"), |i| {
            repo(i, url)?
                .branches
                .get(branch)
                .cloned()
                .ok_or_else(|| d("SPX-HPU002", format!("branch `{branch}` does not exist")))
        })
    }

    fn tree(&self, url: &str, commit: &str) -> HarnessResult<Tree> {
        self.call(format!("tree {url} {commit}"), |i| {
            let truncated = i.tree_truncated;
            let files = repo(i, url)?
                .commits
                .get(commit)
                .ok_or_else(|| d("SPX-HPU002", format!("commit {commit} does not exist")))?;
            Ok(Tree {
                entries: files
                    .iter()
                    .map(|f| TreeEntry {
                        path: f.path.clone(),
                        mode: f.mode.clone(),
                        is_blob: f.mode != "040000" && f.mode != "160000",
                        sha: git_blob_sha(&f.bytes),
                        size: f.bytes.len() as u64,
                    })
                    .collect(),
                truncated,
            })
        })
    }

    fn blob(&self, url: &str, sha: &str, max: u64) -> HarnessResult<Vec<u8>> {
        self.call(format!("blob {url} {sha}"), |i| {
            let mut bytes = repo(i, url)?
                .commits
                .values()
                .flatten()
                .find(|f| git_blob_sha(&f.bytes) == sha)
                .map(|f| f.bytes.clone())
                .ok_or_else(|| d("SPX-HPU009", format!("blob {sha} not found")))?;
            if i.truncate.contains(sha) {
                bytes.truncate(bytes.len() / 2);
            }
            if i.corrupt.contains(sha) && !bytes.is_empty() {
                bytes[0] ^= 0x01;
            }
            if bytes.len() as u64 > max {
                return Err(d("SPX-HPU005", format!("blob {sha} exceeds {max} bytes")));
            }
            Ok(bytes)
        })
    }

    fn revoked(&self, url: &str) -> HarnessResult<Vec<String>> {
        self.call(format!("revoked {url}"), |i| {
            Ok(repo(i, url)?.revoked.clone())
        })
    }
}

/// Loads a fixture repository from disk:
/// `<dir>/<owner>/<repo>/repo.json` (`{"releases":[{"tag","draft","prerelease"}],
/// "tags":{name:{"commit","annotated"}},"branches":{},"revoked":[]}`) and the
/// files of each commit under `<dir>/<owner>/<repo>/commits/<sha>/`.
pub struct DirectoryFetcher;

impl DirectoryFetcher {
    pub fn load(dir: &Path) -> HarnessResult<MemoryFetcher> {
        let io = |e: std::io::Error| d("SPX-HPU009", format!("fixture {}: {e}", dir.display()));
        let m = MemoryFetcher::new();
        for owner in sorted(dir).map_err(io)? {
            for name in sorted(&owner).map_err(io)? {
                let url = format!(
                    "https://github.com/{}/{}",
                    owner.file_name().unwrap().to_string_lossy(),
                    name.file_name().unwrap().to_string_lossy()
                );
                let doc = std::fs::read(name.join("repo.json")).map_err(io)?;
                let v: serde_json::Value = serde_json::from_slice(&doc)
                    .map_err(|e| d("SPX-HPU009", format!("fixture repo.json: {e}")))?;
                for r in v["releases"].as_array().into_iter().flatten() {
                    m.repo_mut(&url, |x| {
                        x.releases.push(Release {
                            tag: r["tag"].as_str().unwrap_or("").into(),
                            draft: r["draft"].as_bool().unwrap_or(false),
                            prerelease: r["prerelease"].as_bool().unwrap_or(false),
                        })
                    });
                }
                for (tag, t) in v["tags"].as_object().into_iter().flatten() {
                    let c = t["commit"].as_str().unwrap_or("");
                    m.repo_mut(&url, |x| {
                        set_tag(x, tag, c, t["annotated"].as_bool() == Some(true))
                    });
                }
                for (b, c) in v["branches"].as_object().into_iter().flatten() {
                    m.set_branch(&url, b, c.as_str().unwrap_or(""));
                }
                for r in v["revoked"].as_array().into_iter().flatten() {
                    m.revoke(&url, r.as_str().unwrap_or(""));
                }
                let commits = name.join("commits");
                for c in sorted(&commits).unwrap_or_default() {
                    let sha = c.file_name().unwrap().to_string_lossy().to_string();
                    let mut files = Vec::new();
                    collect(&c, &c, &mut files).map_err(io)?;
                    m.repo_mut(&url, |x| {
                        x.commits.insert(sha, files);
                    });
                }
            }
        }
        Ok(m)
    }
}

fn sorted(dir: &Path) -> std::io::Result<Vec<std::path::PathBuf>> {
    let mut v: Vec<_> = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    v.sort();
    Ok(v.into_iter().filter(|p| p.is_dir()).collect())
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<FixtureFile>) -> std::io::Result<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let ft = e.file_type()?;
        if ft.is_dir() {
            collect(root, &e.path(), out)?;
        } else if ft.is_file() {
            out.push(FixtureFile {
                path: e
                    .path()
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                mode: "100644".into(),
                bytes: std::fs::read(e.path())?,
            });
        }
    }
    Ok(())
}
