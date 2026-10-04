//! [`Fetcher`] backed by the host's already-authenticated GitHub CLI.
//!
//! The executable is an explicit absolute path (never found on `PATH`), runs
//! with a cleared environment that forwards only a short allow-list from the
//! explicit [`Environment`], reads no stdin, and is bounded in output and
//! wall-clock time. Only `gh api` GET reads against `github.com` are issued.

use super::d;
use super::fetch::{is_sha, parse_repo, Fetcher, Release, TagRef, Tree, TreeEntry};
use crate::cli::Environment;
use crate::diag::HarnessResult;
use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const FORWARD: &[&str] = &[
    "HOME",
    "PATH",
    "TMPDIR",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_HOST",
    "XDG_CONFIG_HOME",
];
/// JSON responses (listings, trees) are small; blobs get their own bound.
const JSON_MAX: u64 = 8 << 20;

pub struct GitHubCliFetcher {
    gh: PathBuf,
    env: Vec<(String, String)>,
    timeout: Duration,
}

impl GitHubCliFetcher {
    pub fn new(gh: PathBuf, env: &Environment, timeout: Duration) -> HarnessResult<Self> {
        if !gh.is_absolute() || !gh.is_file() {
            return Err(d(
                "SPX-HPU015",
                format!(
                    "`{}` is not an absolute path to an existing gh executable",
                    gh.display()
                ),
            ));
        }
        let env = FORWARD
            .iter()
            .filter_map(|k| env.vars.get(*k).map(|v| (k.to_string(), v.clone())))
            .collect();
        Ok(Self { gh, env, timeout })
    }

    fn run(&self, args: &[&str], max: u64) -> HarnessResult<Vec<u8>> {
        let fail = |m: String| d("SPX-HPU009", format!("gh: {m}"));
        let mut cmd = Command::new(&self.gh);
        cmd.arg("api")
            .args(args)
            .env_clear()
            .envs(self.env.iter().cloned())
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .map_err(|e| fail(format!("cannot start: {e}")))?;
        let mut out = child.stdout.take().unwrap();
        let mut err = child.stderr.take().unwrap();
        let reader = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = (&mut out).take(max + 1).read_to_end(&mut b);
            b
        });
        let ereader = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = (&mut err).take(2048).read_to_end(&mut b);
            b
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if start.elapsed() > self.timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(fail(format!(
                        "timed out after {} ms",
                        self.timeout.as_millis()
                    )));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => return Err(fail(e.to_string())),
            }
        };
        let body = reader.join().unwrap_or_default();
        let stderr = String::from_utf8_lossy(&ereader.join().unwrap_or_default()).to_string();
        if body.len() as u64 > max {
            return Err(d("SPX-HPU005", format!("gh response exceeds {max} bytes")));
        }
        if !status.success() {
            return Err(fail(super::bounded(stderr.trim())));
        }
        Ok(body)
    }

    fn json(&self, path: &str, extra: &[&str]) -> HarnessResult<Value> {
        let mut args: Vec<&str> = extra.to_vec();
        args.push(path);
        let b = self.run(&args, JSON_MAX)?;
        serde_json::from_slice(&b).map_err(|e| d("SPX-HPU009", format!("gh: bad JSON: {e}")))
    }
}

fn base(url: &str) -> HarnessResult<String> {
    let (o, r) = parse_repo(url)?;
    Ok(format!("repos/{o}/{r}"))
}

fn arg_ok(s: &str) -> HarnessResult<()> {
    if s.is_empty()
        || s.starts_with('-')
        || s.contains("..")
        || !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/' | b'+'))
    {
        return Err(d("SPX-HPU001", format!("`{s}` is not a safe ref name")));
    }
    Ok(())
}

impl Fetcher for GitHubCliFetcher {
    fn releases(&self, url: &str) -> HarnessResult<Vec<Release>> {
        let v = self.json(&format!("{}/releases?per_page=100", base(url)?), &[])?;
        Ok(v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| {
                Some(Release {
                    tag: r["tag_name"].as_str()?.to_string(),
                    draft: r["draft"].as_bool().unwrap_or(false),
                    prerelease: r["prerelease"].as_bool().unwrap_or(false),
                })
            })
            .collect())
    }

    fn tag_ref(&self, url: &str, tag: &str) -> HarnessResult<Option<TagRef>> {
        arg_ok(tag)?;
        match self.json(&format!("{}/git/ref/tags/{tag}", base(url)?), &[]) {
            Ok(v) => Ok(Some(TagRef {
                object_sha: v["object"]["sha"].as_str().unwrap_or("").to_string(),
                annotated: v["object"]["type"] == "tag",
            })),
            Err(e) if e.message.contains("Not Found") || e.message.contains("404") => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn peel_tag(&self, url: &str, sha: &str) -> HarnessResult<String> {
        let mut cur = sha.to_string();
        for _ in 0..4 {
            if !is_sha(&cur) {
                break;
            }
            let v = self.json(&format!("{}/git/tags/{cur}", base(url)?), &[])?;
            let next = v["object"]["sha"].as_str().unwrap_or("").to_string();
            if v["object"]["type"] == "commit" {
                return Ok(next);
            }
            cur = next;
        }
        Err(d(
            "SPX-HPU002",
            format!("tag object {sha} does not peel to a commit"),
        ))
    }

    fn branch_head(&self, url: &str, branch: &str) -> HarnessResult<String> {
        arg_ok(branch)?;
        let v = self.json(&format!("{}/git/ref/heads/{branch}", base(url)?), &[])?;
        v["object"]["sha"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| d("SPX-HPU002", format!("branch `{branch}` did not resolve")))
    }

    fn tree(&self, url: &str, commit: &str) -> HarnessResult<Tree> {
        if !is_sha(commit) {
            return Err(d("SPX-HPU002", format!("`{commit}` is not a commit sha")));
        }
        let v = self.json(
            &format!("{}/git/trees/{commit}?recursive=1", base(url)?),
            &[],
        )?;
        let entries = v["tree"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|e| TreeEntry {
                path: e["path"].as_str().unwrap_or("").to_string(),
                mode: e["mode"].as_str().unwrap_or("").to_string(),
                is_blob: e["type"] == "blob",
                sha: e["sha"].as_str().unwrap_or("").to_string(),
                size: e["size"].as_u64().unwrap_or(0),
            })
            .collect();
        Ok(Tree {
            entries,
            truncated: v["truncated"].as_bool().unwrap_or(false),
        })
    }

    fn blob(&self, url: &str, sha: &str, max: u64) -> HarnessResult<Vec<u8>> {
        if !is_sha(sha) {
            return Err(d("SPX-HPU005", format!("`{sha}` is not a blob sha")));
        }
        let path = format!("{}/git/blobs/{sha}", base(url)?);
        self.run(
            &["-H", "Accept: application/vnd.github.raw+json", &path],
            max,
        )
    }
}
