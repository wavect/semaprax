//! Channel resolution: a mutable intent (`latest-stable`, a release range, an
//! exact commit or an explicit head) becomes one immutable commit. Annotated
//! tags are peeled to their commit; a head is resolved once and recorded, and
//! every later read is by commit sha.

use super::d;
use super::fetch::{is_sha, Fetcher};
use crate::diag::HarnessResult;
use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Channel {
    LatestStable,
    /// Raw requirement text, comparators separated by commas.
    Range(String),
    Commit(String),
    Head(Option<String>),
}

impl Channel {
    /// `latest-stable`, `range:>=4.0.0,<5.0.0`, `commit:<40 hex>`, `head[:branch]`.
    pub fn parse(s: &str) -> HarnessResult<Channel> {
        let bad = || {
            d(
                "SPX-HPU001",
                format!("unknown channel `{s}` (latest-stable | range:<req> | commit:<sha> | head[:branch])"),
            )
        };
        Ok(match s {
            "latest-stable" => Channel::LatestStable,
            "head" => Channel::Head(None),
            _ => match s.split_once(':').ok_or_else(bad)? {
                ("range", r) => {
                    comparators(r)?;
                    Channel::Range(r.to_string())
                }
                ("commit", c) if is_sha(c) => Channel::Commit(c.to_string()),
                ("head", b) if !b.is_empty() => Channel::Head(Some(b.to_string())),
                _ => return Err(bad()),
            },
        })
    }

    pub fn render(&self) -> String {
        match self {
            Channel::LatestStable => "latest-stable".into(),
            Channel::Range(r) => format!("range:{r}"),
            Channel::Commit(c) => format!("commit:{c}"),
            Channel::Head(None) => "head".into(),
            Channel::Head(Some(b)) => format!("head:{b}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub commit: String,
    pub tag: Option<String>,
    /// The tag, or `commit:<short>` for exact-commit and head channels.
    pub version: String,
    pub head_branch: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ver(u64, u64, u64);

impl Ord for Ver {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.0, self.1, self.2).cmp(&(o.0, o.1, o.2))
    }
}
impl PartialOrd for Ver {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Stable `vX.Y.Z` / `X.Y.Z` only: a pre-release suffix returns `None`.
fn ver(tag: &str) -> Option<Ver> {
    let t = tag.strip_prefix('v').unwrap_or(tag);
    let mut p = t.split('.');
    let mut n = || p.next()?.parse::<u64>().ok();
    let v = Ver(n()?, n()?, n()?);
    p.next().is_none().then_some(v)
}

type Cmp = (&'static str, Ver);

fn comparators(req: &str) -> HarnessResult<Vec<Cmp>> {
    let bad = |c: &str| d("SPX-HPU001", format!("bad range comparator `{c}`"));
    let mut out = Vec::new();
    for c in req.split(',').map(str::trim) {
        let (op, rest) = ["^", "~", ">=", "<=", ">", "<", "="]
            .iter()
            .find_map(|op| c.strip_prefix(op).map(|r| (*op, r.trim())))
            .unwrap_or(("=", c));
        let rest_s = rest.strip_prefix('v').unwrap_or(rest);
        let mut parts = rest_s.split('.');
        let mut num = |_: ()| parts.next().and_then(|x| x.parse::<u64>().ok());
        let (a, b, cc) = (num(()), num(()), num(()));
        let v = Ver(a.ok_or_else(|| bad(c))?, b.unwrap_or(0), cc.unwrap_or(0));
        match op {
            "^" => {
                out.push((">=", v));
                out.push((
                    "<",
                    if v.0 > 0 {
                        Ver(v.0 + 1, 0, 0)
                    } else {
                        Ver(0, v.1 + 1, 0)
                    },
                ));
            }
            "~" => {
                out.push((">=", v));
                out.push(("<", Ver(v.0, v.1 + 1, 0)));
            }
            ">=" => out.push((">=", v)),
            "<=" => out.push(("<=", v)),
            ">" => out.push((">", v)),
            "<" => out.push(("<", v)),
            _ => out.push(("=", v)),
        }
    }
    Ok(out)
}

fn satisfies(v: Ver, cmps: &[Cmp]) -> bool {
    cmps.iter().all(|(op, c)| match *op {
        ">=" => v >= *c,
        "<=" => v <= *c,
        ">" => v > *c,
        "<" => v < *c,
        _ => v == *c,
    })
}

fn short(c: &str) -> &str {
    &c[..c.len().min(8)]
}

/// Current commit of `tag` (peeled), or `None` when the tag no longer exists.
pub fn tag_commit(f: &dyn Fetcher, repo: &str, tag: &str) -> HarnessResult<Option<String>> {
    Ok(match f.tag_ref(repo, tag)? {
        None => None,
        Some(t) if t.annotated => Some(f.peel_tag(repo, &t.object_sha)?),
        Some(t) => Some(t.object_sha),
    })
}

/// Resolve `channel` for `repo` to an immutable commit.
pub fn resolve(
    f: &dyn Fetcher,
    repo: &str,
    channel: &Channel,
    default_branch: &str,
) -> HarnessResult<Resolved> {
    match channel {
        Channel::Commit(c) => Ok(Resolved {
            commit: c.clone(),
            tag: None,
            version: format!("commit:{}", short(c)),
            head_branch: None,
        }),
        Channel::Head(b) => {
            let branch = b.clone().unwrap_or_else(|| default_branch.to_string());
            let commit = f.branch_head(repo, &branch)?;
            if !is_sha(&commit) {
                return Err(d(
                    "SPX-HPU002",
                    format!("branch `{branch}` resolved to `{commit}`"),
                ));
            }
            Ok(Resolved {
                version: format!("commit:{}", short(&commit)),
                commit,
                tag: None,
                head_branch: Some(branch),
            })
        }
        Channel::LatestStable | Channel::Range(_) => {
            let cmps = match channel {
                Channel::Range(r) => comparators(r)?,
                _ => Vec::new(),
            };
            let mut cands: Vec<(Ver, String)> = f
                .releases(repo)?
                .into_iter()
                .filter(|r| !r.draft && !r.prerelease)
                .filter_map(|r| ver(&r.tag).map(|v| (v, r.tag)))
                .filter(|(v, _)| satisfies(*v, &cmps))
                .collect();
            cands.sort_by(|a, b| b.0.cmp(&a.0));
            for (_, tag) in cands {
                let Some(t) = f.tag_ref(repo, &tag)? else {
                    continue;
                };
                let commit = if t.annotated {
                    f.peel_tag(repo, &t.object_sha)?
                } else {
                    t.object_sha
                };
                if !is_sha(&commit) {
                    return Err(d(
                        "SPX-HPU002",
                        format!("tag `{tag}` resolved to `{commit}`"),
                    ));
                }
                return Ok(Resolved {
                    commit,
                    version: tag.clone(),
                    tag: Some(tag),
                    head_branch: None,
                });
            }
            Err(d(
                "SPX-HPU002",
                format!(
                    "no stable release of {repo} satisfies `{}`",
                    channel.render()
                ),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        let c = comparators(">=4.0.0,<5.0.0").unwrap();
        assert!(satisfies(ver("v4.10.3").unwrap(), &c));
        assert!(!satisfies(ver("v5.0.0").unwrap(), &c));
        assert!(ver("v1.0.0-rc1").is_none());
        let c = comparators("^1.2").unwrap();
        assert!(satisfies(Ver(1, 9, 0), &c) && !satisfies(Ver(2, 0, 0), &c));
        assert!(Channel::parse("range:bogus").is_err());
    }
}
