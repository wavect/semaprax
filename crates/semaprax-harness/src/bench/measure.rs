//! Measurement primitives: injected clock, local resource accounting through
//! `/usr/bin/time`, disk bytes and small honest statistics.
//!
//! Wall-clock enters only through [`Clock`], so a test can inject a fixed
//! clock and compare outputs byte-for-byte.

use std::path::Path;
use std::time::Instant;

pub trait Clock {
    /// Milliseconds since an arbitrary origin.
    fn now_ms(&self) -> u64;
}

pub struct SystemClock(Instant);

impl SystemClock {
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }
}

/// Advances one millisecond per reading: deterministic latencies.
pub struct TickClock(std::cell::Cell<u64>);

impl TickClock {
    pub fn new() -> Self {
        Self(std::cell::Cell::new(0))
    }
}

impl Default for TickClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for TickClock {
    fn now_ms(&self) -> u64 {
        let v = self.0.get() + 1;
        self.0.set(v);
        v
    }
}

/// Child-process resources of one cell (`None` = not measured).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rusage {
    pub user_ms: Option<u64>,
    pub sys_ms: Option<u64>,
    pub max_rss_kb: Option<u64>,
}

/// Parse `/usr/bin/time -l` (BSD/macOS, RSS in bytes) or `-v` (GNU, RSS in KiB)
/// text. Unknown shapes stay `None`; nothing is guessed.
pub fn parse_time_output(stderr: &str) -> Rusage {
    let mut r = Rusage::default();
    let secs = |s: &str| {
        s.trim()
            .parse::<f64>()
            .ok()
            .map(|f| (f * 1000.0).round() as u64)
    };
    for line in stderr.lines() {
        let t = line.trim();
        // BSD one-liner: `0.12 real 0.05 user 0.02 sys`.
        if t.ends_with(" sys") && t.contains(" user ") {
            let p: Vec<&str> = t.split_whitespace().collect();
            if p.len() >= 6 {
                r.user_ms = secs(p[2]);
                r.sys_ms = secs(p[4]);
            }
        } else if let Some(v) = t.strip_suffix(" maximum resident set size") {
            r.max_rss_kb = v.trim().parse::<u64>().ok().map(|b| b / 1024);
        } else if let Some(v) = t.strip_prefix("User time (seconds):") {
            r.user_ms = secs(v);
        } else if let Some(v) = t.strip_prefix("System time (seconds):") {
            r.sys_ms = secs(v);
        } else if let Some(v) = t.strip_prefix("Maximum resident set size (kbytes):") {
            r.max_rss_kb = v.trim().parse().ok();
        }
    }
    r
}

/// Total regular-file bytes below `dir` (0 when absent).
pub fn dir_bytes(dir: &Path) -> u64 {
    let mut total = 0;
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            match e.file_type() {
                Ok(t) if t.is_dir() => total += dir_bytes(&e.path()),
                Ok(t) if t.is_file() => total += e.metadata().map_or(0, |m| m.len()),
                _ => {}
            }
        }
    }
    total
}

/// Nearest-rank percentile of `xs` (`p` in 0..=100); `None` when empty.
pub fn percentile(xs: &[u64], p: u32) -> Option<u64> {
    if xs.is_empty() {
        return None;
    }
    let mut s = xs.to_vec();
    s.sort_unstable();
    let rank = ((p as f64 / 100.0) * s.len() as f64).ceil().max(1.0) as usize;
    Some(s[rank.min(s.len()) - 1])
}

pub fn mean(xs: &[f64]) -> Option<f64> {
    (!xs.is_empty()).then(|| xs.iter().sum::<f64>() / xs.len() as f64)
}

/// Sample standard deviation (`None` below two samples).
pub fn stdev(xs: &[f64]) -> Option<f64> {
    if xs.len() < 2 {
        return None;
    }
    let m = mean(xs)?;
    Some((xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() - 1) as f64).sqrt())
}

/// Wilson 95% score interval for `k` successes in `n` trials. With n == 0 the
/// interval is the whole unit range: no data, no information.
pub fn wilson95(k: u64, n: u64) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let z = 1.959_963_984_540_054_f64;
    let (n, p) = (n as f64, k as f64 / n as f64);
    let d = 1.0 + z * z / n;
    let centre = p + z * z / (2.0 * n);
    let margin = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt();
    (
        ((centre - margin) / d).max(0.0),
        ((centre + margin) / d).min(1.0),
    )
}

/// Round to 4 decimals so JSON output is stable.
pub fn r4(x: f64) -> f64 {
    (x * 10_000.0).round() / 10_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_and_wilson() {
        assert_eq!(percentile(&[5, 1, 3, 2, 4], 50), Some(3));
        assert_eq!(percentile(&[5, 1, 3, 2, 4], 95), Some(5));
        assert_eq!(percentile(&[], 50), None);
        let (lo, hi) = wilson95(10, 10);
        assert!(lo > 0.69 && hi > 0.999, "{lo} {hi}");
        assert_eq!(wilson95(0, 0), (0.0, 1.0));
    }

    #[test]
    fn time_output_parses_bsd_and_gnu() {
        let bsd = "        0.31 real         0.20 user         0.05 sys\n  12582912  maximum resident set size\n";
        assert_eq!(
            parse_time_output(bsd),
            Rusage {
                user_ms: Some(200),
                sys_ms: Some(50),
                max_rss_kb: Some(12288)
            }
        );
        let gnu = "\tUser time (seconds): 0.10\n\tSystem time (seconds): 0.02\n\tMaximum resident set size (kbytes): 4096\n";
        assert_eq!(
            parse_time_output(gnu),
            Rusage {
                user_ms: Some(100),
                sys_ms: Some(20),
                max_rss_kb: Some(4096)
            }
        );
        assert_eq!(parse_time_output("nothing"), Rusage::default());
    }
}
