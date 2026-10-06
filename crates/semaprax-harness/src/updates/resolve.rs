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

/// One strict numeric component: ASCII digits only, no leading zero, fits `u64`.
fn component(x: &str) -> Option<u64> {
    let ok =
        !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit()) && (x == "0" || !x.starts_with('0'));
    if ok {
        x.parse().ok()
    } else {
        None
    }
}

/// `[v]X[.Y[.Z]]`, fully consumed. Absent trailing components are `None`;
/// malformed, empty or extra components are errors, never zero.
fn partial(s: &str) -> Option<[Option<u64>; 3]> {
    let s = s.strip_prefix('v').unwrap_or(s);
    let mut out = [None; 3];
    let mut parts = s.split('.');
    for slot in out.iter_mut() {
        match parts.next() {
            Some(x) => *slot = Some(component(x)?),
            None => break,
        }
    }
    if out[0].is_none() || parts.next().is_some() {
        return None;
    }
    Some(out)
}

fn comparators(req: &str) -> HarnessResult<Vec<Cmp>> {
    let bad = |c: &str| {
        d(
            "SPX-HPU001",
            format!(
                "bad range comparator `{}` (expected [^|~|>=|<=|>|<|=]X[.Y[.Z]] with numeric components)",
                super::bounded(c)
            ),
        )
    };
    let mut out = Vec::new();
    for c in req.split(',').map(str::trim) {
        let (op, rest) = ["^", "~", ">=", "<=", ">", "<", "="]
            .iter()
            .find_map(|op| c.strip_prefix(op).map(|r| (*op, r.trim())))
            .unwrap_or(("=", c));
        let [a, b, cc] = partial(rest).ok_or_else(|| bad(c))?;
        let (maj, min, pat) = (a.unwrap_or(0), b.unwrap_or(0), cc.unwrap_or(0));
        let v = Ver(maj, min, pat);
        // Exclusive upper bound; `None` when it would overflow `u64`.
        let upper = match op {
            "^" => match (a, b, cc) {
                _ if maj > 0 => maj.checked_add(1).map(|m| Ver(m, 0, 0)),
                (_, None, _) => Some(Ver(1, 0, 0)),
                _ if min > 0 => min.checked_add(1).map(|m| Ver(0, m, 0)),
                (_, _, None) => Some(Ver(0, 1, 0)),
                _ => pat.checked_add(1).map(|p| Ver(0, 0, p)),
            },
            "~" => match b {
                Some(_) => min.checked_add(1).map(|m| Ver(maj, m, 0)),
                None => maj.checked_add(1).map(|m| Ver(m, 0, 0)),
            },
            _ => None,
        };
        if matches!(op, "^" | "~") {
            out.push((">=", v));
            out.push(("<", upper.ok_or_else(|| bad(c))?));
            continue;
        }
        match op {
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
            cands.sort_by_key(|a| std::cmp::Reverse(a.0));
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

    fn ok(req: &str, v: (u64, u64, u64)) -> bool {
        satisfies(Ver(v.0, v.1, v.2), &comparators(req).unwrap())
    }

    #[test]
    fn caret_and_tilde_bounds_follow_the_leftmost_nonzero_component() {
        assert!(ok("^0.0.1", (0, 0, 1)));
        assert!(!ok("^0.0.1", (0, 0, 2)) && !ok("^0.0.1", (0, 0, 9)) && !ok("^0.0.1", (0, 0, 0)));
        assert!(ok("^0.2.3", (0, 2, 9)) && !ok("^0.2.3", (0, 3, 0)) && !ok("^0.2.3", (0, 2, 2)));
        assert!(ok("^1.2.3", (1, 9, 9)) && !ok("^1.2.3", (2, 0, 0)));
        assert!(ok("^0.0", (0, 0, 7)) && !ok("^0.0", (0, 1, 0)));
        assert!(ok("^0", (0, 9, 9)) && !ok("^0", (1, 0, 0)));
        assert!(ok("^0.0.0", (0, 0, 0)) && !ok("^0.0.0", (0, 0, 1)));
        assert!(ok("~1.2.3", (1, 2, 9)) && !ok("~1.2.3", (1, 3, 0)));
        assert!(ok("~1.2", (1, 2, 0)) && !ok("~1.2", (1, 3, 0)));
        assert!(ok("~1", (1, 9, 0)) && !ok("~1", (2, 0, 0)));
        assert!(ok(">=v1.2, <v2", (1, 5, 0)) && !ok(">=v1.2, <v2", (2, 0, 0)));
    }

    #[test]
    fn malformed_ranges_are_refused_not_reinterpreted() {
        for bad in [
            ">=1.2.3.4",
            ">=1.bad.3",
            ">=1.2.bad",
            ">=",
            "",
            ">=1.",
            ">=1..3",
            ">=.1",
            ">=+1",
            ">=1.2.3-rc1",
            ">=1.2.3+b",
            ">=01.2.3",
            ">=1,",
            ">=1,,<2",
            "^",
            "~x",
            "^18446744073709551616",
            "^18446744073709551615",
            "~1.18446744073709551615",
            "^0.0.18446744073709551615",
            "^0.18446744073709551615.1",
        ] {
            let e = Channel::parse(&format!("range:{bad}")).unwrap_err();
            assert_eq!(e.code, "SPX-HPU001", "{bad}");
            assert!(e.message.chars().count() < 300);
        }
        // The largest values whose bound does not overflow still parse.
        assert!(comparators(">=18446744073709551615.0.0").is_ok());
        assert!(comparators("^0.0.18446744073709551614").is_ok());
    }
}
