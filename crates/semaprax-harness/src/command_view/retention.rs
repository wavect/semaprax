//! Private per-project raw retention: a 0700 directory of 0600 files with a
//! TTL and a size bound. Enabled only by an explicit policy. Recovery reads
//! these bytes; nothing here ever executes a command.

use super::executor::Spill;
use super::policy::RetentionPolicy;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::Value;
use std::fs;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

static TMP: AtomicU64 = AtomicU64::new(0);

fn fail(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// `cv-<24 hex>`, derived only from authoritative digests.
pub fn handle_for(argv_digest: &str, stdout: &str, stderr: &str, status: &str) -> String {
    let d = crate::json::sha256_plain(
        format!("{argv_digest}\0{stdout}\0{stderr}\0{status}").as_bytes(),
    );
    format!("cv-{}", &d["sha256:".len()..][..24])
}

pub fn valid_handle(h: &str) -> bool {
    h.strip_prefix("cv-")
        .is_some_and(|x| x.len() == 24 && x.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

pub struct Retention {
    dir: PathBuf,
    policy: RetentionPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamName {
    Stdout,
    Stderr,
}

impl StreamName {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "stdout" => Some(Self::Stdout),
            "stderr" => Some(Self::Stderr),
            _ => None,
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

impl Retention {
    pub fn dir_for(home: &Path, project_id: &str) -> PathBuf {
        home.join("retention")
            .join(project_id.trim_start_matches("sha256:"))
            .join("command-view")
    }

    /// Create (or tighten to) a private directory.
    pub fn open(home: &Path, project_id: &str, policy: &RetentionPolicy) -> HarnessResult<Self> {
        let dir = Self::dir_for(home, project_id);
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .and_then(|_| fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)))
            .map_err(|e| {
                fail(
                    "SPX-HPH031",
                    format!("cannot prepare retention directory: {e}"),
                )
            })?;
        let r = Self {
            dir,
            policy: policy.clone(),
        };
        r.prune(0);
        Ok(r)
    }

    pub fn spill(&self) -> Spill {
        let n = TMP.fetch_add(1, Ordering::SeqCst);
        let base = format!(".tmp-{}-{n}", std::process::id());
        Spill {
            stdout: self.dir.join(format!("{base}.stdout")),
            stderr: self.dir.join(format!("{base}.stderr")),
            max_stream: self.policy.max_stream_bytes,
        }
    }

    pub fn discard(&self, s: &Spill) {
        let _ = fs::remove_file(&s.stdout);
        let _ = fs::remove_file(&s.stderr);
    }

    fn entries(&self) -> Vec<(PathBuf, u64, SystemTime)> {
        fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                Some((e.path(), m.len(), m.modified().ok()?))
            })
            .collect()
    }

    /// Drop expired files, then the oldest until `incoming` more bytes fit.
    fn prune(&self, incoming: u64) {
        let ttl = Duration::from_secs(self.policy.ttl_secs);
        let now = SystemTime::now();
        let mut live = Vec::new();
        for (p, len, t) in self.entries() {
            if now.duration_since(t).unwrap_or_default() > ttl {
                let _ = fs::remove_file(&p);
            } else {
                live.push((p, len, t));
            }
        }
        live.sort_by_key(|(p, _, t)| (*t, p.clone()));
        let mut total: u64 = live.iter().map(|(_, l, _)| l).sum::<u64>() + incoming;
        for (p, len, _) in live {
            if total <= self.policy.max_bytes {
                break;
            }
            if !p
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with(".tmp-"))
            {
                let _ = fs::remove_file(p);
                total -= len;
            }
        }
    }

    /// Publish the spilled streams under `handle`. `Ok(false)`: the result
    /// alone exceeds the size bound, nothing is retained.
    pub fn commit(&self, spill: &Spill, handle: &str, meta: &Value) -> HarnessResult<bool> {
        let size = |p: &Path| fs::metadata(p).map_or(0, |m| m.len());
        let meta_bytes = serde_json::to_vec(meta).unwrap_or_default();
        let total = size(&spill.stdout) + size(&spill.stderr) + meta_bytes.len() as u64;
        if total > self.policy.max_bytes {
            self.discard(spill);
            return Ok(false);
        }
        self.prune(total);
        let io = |e: std::io::Error| fail("SPX-HPH031", format!("cannot retain output: {e}"));
        fs::rename(&spill.stdout, self.dir.join(format!("{handle}.stdout"))).map_err(io)?;
        fs::rename(&spill.stderr, self.dir.join(format!("{handle}.stderr"))).map_err(io)?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(self.dir.join(format!("{handle}.json")))
            .map_err(io)?;
        std::io::Write::write_all(&mut f, &meta_bytes).map_err(io)?;
        Ok(true)
    }

    fn live_path(&self, handle: &str, ext: &str) -> HarnessResult<PathBuf> {
        if !valid_handle(handle) {
            return Err(fail(
                "SPX-HPH040",
                "recovery handle must look like `cv-<24 hex>`",
            ));
        }
        let meta = self.dir.join(format!("{handle}.json"));
        let t = fs::metadata(&meta)
            .and_then(|m| m.modified())
            .map_err(|_| fail("SPX-HPH041", format!("no retained output for `{handle}` in this project (unknown, pruned or expired)")))?;
        if SystemTime::now().duration_since(t).unwrap_or_default()
            > Duration::from_secs(self.policy.ttl_secs)
        {
            self.prune(0);
            return Err(fail(
                "SPX-HPH041",
                format!("retained output for `{handle}` expired"),
            ));
        }
        Ok(self.dir.join(format!("{handle}.{ext}")))
    }

    pub fn meta(&self, handle: &str) -> HarnessResult<Value> {
        let p = self.live_path(handle, "json")?;
        serde_json::from_slice(&fs::read(p).map_err(|e| fail("SPX-HPH041", e.to_string()))?)
            .map_err(|e| fail("SPX-HPH041", e.to_string()))
    }

    /// Bounded read of retained bytes: `(bytes, stream total)`.
    pub fn read(
        &self,
        handle: &str,
        stream: StreamName,
        offset: u64,
        limit: u64,
    ) -> HarnessResult<(Vec<u8>, u64)> {
        use std::io::{Read, Seek, SeekFrom};
        let p = self.live_path(handle, stream.ext())?;
        let mut f = fs::File::open(p).map_err(|e| fail("SPX-HPH041", e.to_string()))?;
        let total = f
            .metadata()
            .map_err(|e| fail("SPX-HPH041", e.to_string()))?
            .len();
        if offset > total {
            return Err(fail(
                "SPX-HPH042",
                format!("offset {offset} is beyond the {total} retained bytes"),
            ));
        }
        f.seek(SeekFrom::Start(offset))
            .map_err(|e| fail("SPX-HPH041", e.to_string()))?;
        let mut buf = Vec::new();
        f.take(limit)
            .read_to_end(&mut buf)
            .map_err(|e| fail("SPX-HPH041", e.to_string()))?;
        Ok((buf, total))
    }

    /// Up to `max` critical lines scanned from a retained stream, for outputs
    /// too large to hold in memory.
    pub fn critical_lines(&self, handle: &str, stream: StreamName, max: usize) -> Vec<String> {
        use std::io::{BufRead, BufReader, Read};
        let Ok(p) = self.live_path(handle, stream.ext()) else {
            return Vec::new();
        };
        let Ok(f) = fs::File::open(p) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut r = BufReader::new(f);
        let mut buf = Vec::new();
        while out.len() < max {
            buf.clear();
            if r.by_ref()
                .take(8192)
                .read_until(b'\n', &mut buf)
                .unwrap_or(0)
                == 0
            {
                break;
            }
            let line = String::from_utf8_lossy(&buf);
            let line = line.trim_end_matches(['\n', '\r']);
            if super::guard::is_critical(line) {
                out.push(line.to_string());
            }
        }
        out
    }
}
