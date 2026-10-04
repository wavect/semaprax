//! Application tasks: pinned multi-file projects, immutable graders and the
//! file-block edit protocol. A grader is a list of commands run in a sandbox
//! after the model's edits were applied and the protected files (visible tests,
//! hidden grader files) were restored; a model can neither edit nor outrun them.

use crate::json::sha256_plain;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Paths no model edit may change, in addition to every test file and grader file.
pub const ALWAYS_PROTECTED: [&str; 1] = ["semaprax.toml"];

#[derive(Clone, Debug)]
pub struct GradeCmd {
    pub cmd: Vec<String>,
    pub expect_stdout: Option<String>,
}

#[derive(Clone, Debug)]
pub struct InitialCommand {
    pub cmd: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Step {
    pub id: String,
    pub request: String,
    pub grade: Vec<GradeCmd>,
    pub max_attempts: u32,
    pub initial_command: Option<InitialCommand>,
    /// Files the reference solution changes (retrieval recall is measured against them).
    pub reference_files: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Task {
    pub id: String,
    pub class: String,
    pub languages: Vec<String>,
    pub query: String,
    pub steps: Vec<Step>,
    pub dir: PathBuf,
    pub project: BTreeMap<String, String>,
}

pub struct TaskSet {
    pub root: PathBuf,
    pub tasks: Vec<Task>,
    pub digest: String,
}

/// Executable identities for graders, views and retrieval (nothing is
/// discovered): `HARNESS_*` variables name the executables, the compiler is separate.
#[derive(Clone, Debug, Default)]
pub struct Tools {
    pub vars: BTreeMap<String, PathBuf>,
    pub compiler: Option<PathBuf>,
}

impl Tools {
    pub fn from_vars(vars: &BTreeMap<String, String>, compiler: Option<PathBuf>) -> Self {
        Tools {
            vars: vars
                .iter()
                .filter(|(k, _)| k.starts_with("HARNESS_"))
                .map(|(k, v)| (k.clone(), PathBuf::from(v)))
                .collect(),
            compiler,
        }
    }

    pub fn get(&self, var: &str) -> Option<&PathBuf> {
        self.vars.get(var)
    }

    pub fn with(mut self, var: &str, path: &str) -> Self {
        self.vars.insert(var.into(), PathBuf::from(path));
        self
    }

    /// Replace `{python}`, `{node}`, `{compiler}`; `Err` names a missing tool.
    pub fn expand(&self, argv: &[String]) -> Result<Vec<String>, String> {
        argv.iter()
            .map(|a| {
                let pick = |name: &str, v: Option<&PathBuf>| {
                    v.map(|p| p.display().to_string())
                        .ok_or_else(|| format!("tool `{name}` not provided"))
                };
                match a.as_str() {
                    "{python}" => pick("python", self.get("HARNESS_PYTHON")),
                    "{node}" => pick("node", self.get("HARNESS_NODE")),
                    "{compiler}" => pick("compiler", self.compiler.as_ref()),
                    x => Ok(x.to_string()),
                }
            })
            .collect()
    }
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut es: Vec<_> = rd.flatten().collect();
        es.sort_by_key(|e| e.file_name());
        for e in es {
            let p = e.path();
            match e.file_type() {
                Ok(t) if t.is_dir() => walk(base, &p, out),
                Ok(t) if t.is_file() => {
                    let rel = p
                        .strip_prefix(base)
                        .unwrap_or(&p)
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.insert(rel, std::fs::read(&p).unwrap_or_default());
                }
                _ => {}
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(root, root, &mut m);
    m
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

impl TaskSet {
    pub fn load(root: &Path) -> Result<TaskSet, String> {
        let tdir = root.join("tasks");
        let mut names: Vec<String> = std::fs::read_dir(&tdir)
            .map_err(|e| format!("read {}: {e}", tdir.display()))?
            .flatten()
            .filter(|e| e.path().join("task.json").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        let mut tasks = vec![];
        for n in &names {
            let dir = tdir.join(n);
            let meta: Value = serde_json::from_slice(
                &std::fs::read(dir.join("task.json")).map_err(|e| format!("{n}: {e}"))?,
            )
            .map_err(|e| format!("{n}: task.json: {e}"))?;
            let mut steps = vec![];
            for s in meta["steps"].as_array().ok_or("task without steps")? {
                let id = s["id"].as_str().ok_or("step without id")?.to_string();
                let grade = s["grade"]
                    .as_array()
                    .ok_or("step without grade")?
                    .iter()
                    .map(|g| GradeCmd {
                        cmd: strs(&g["cmd"]),
                        expect_stdout: g["expect_stdout"].as_str().map(String::from),
                    })
                    .collect();
                let reference_files = read_tree(&dir.join(&id).join("reference"))
                    .into_keys()
                    .collect();
                steps.push(Step {
                    request: s["request"].as_str().unwrap_or_default().to_string(),
                    grade,
                    max_attempts: s["max_attempts"].as_u64().unwrap_or(2) as u32,
                    initial_command: s["initial_command"].is_object().then(|| InitialCommand {
                        cmd: strs(&s["initial_command"]["cmd"]),
                    }),
                    reference_files,
                    id,
                });
            }
            let project = read_tree(&dir.join("project"))
                .into_iter()
                .map(|(k, v)| (k, String::from_utf8_lossy(&v).into_owned()))
                .collect();
            tasks.push(Task {
                id: meta["id"].as_str().unwrap_or(n).to_string(),
                class: meta["class"].as_str().unwrap_or("unclassified").to_string(),
                languages: strs(&meta["languages"]),
                query: meta["query"].as_str().unwrap_or_default().to_string(),
                steps,
                dir,
                project,
            });
        }
        let mut h = Vec::new();
        for (k, v) in read_tree(&tdir) {
            h.extend_from_slice(k.as_bytes());
            h.push(0);
            h.extend_from_slice(&v);
            h.push(0);
        }
        Ok(TaskSet {
            root: root.to_path_buf(),
            tasks,
            digest: sha256_plain(&h),
        })
    }

    pub fn task(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }
}

impl Task {
    fn overlay_files(&self, dir: &str, upto: usize) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        for s in self.steps.iter().take(upto + 1) {
            out.extend(read_tree(&self.dir.join(&s.id).join(dir)));
        }
        out
    }

    /// Hidden grader files accumulated through step `upto` (later steps win).
    pub fn grader_files(&self, upto: usize) -> BTreeMap<String, Vec<u8>> {
        self.overlay_files("grader", upto)
    }

    pub fn reference_files_of(&self, step: usize) -> BTreeMap<String, Vec<u8>> {
        self.overlay_files("reference", step)
            .into_iter()
            .filter(|(k, _)| self.steps[step].reference_files.contains(k))
            .collect()
    }

    /// Every path the model may not change: test files, grader files, fixed names.
    pub fn is_protected(&self, rel: &str) -> bool {
        let graders = self.grader_files(self.steps.len().saturating_sub(1));
        rel.starts_with("tests/")
            || rel.starts_with("test/")
            || rel.starts_with("_grader/")
            || rel.ends_with(".test.js")
            || ALWAYS_PROTECTED.contains(&rel)
            || graders.contains_key(rel)
    }
}

// ---- edit protocol ----

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedAnswer {
    pub edits: Vec<(String, String)>,
    pub unsafe_paths: Vec<String>,
    /// Blocks that never reached their end marker (truncated or malformed): never applied.
    pub unterminated: Vec<String>,
}

pub fn safe_rel(p: &str) -> bool {
    !p.is_empty()
        && p.len() < 200
        && !p.contains('\\')
        && !p.contains('\0')
        && Path::new(p)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

fn header(line: &str) -> Option<String> {
    let t = line.trim();
    let inner = t.strip_prefix("=== FILE:")?.strip_suffix("===")?;
    Some(
        inner
            .trim()
            .trim_matches(|c| c == '`' || c == '"' || c == '\'')
            .to_string(),
    )
}

fn is_end(line: &str) -> bool {
    matches!(line.trim(), "=== END FILE ===" | "=== END ===")
}

fn unfence(mut lines: Vec<&str>) -> String {
    if lines
        .first()
        .is_some_and(|l| l.trim_start().starts_with("```"))
    {
        lines.remove(0);
        while lines.last().is_some_and(|l| l.trim().is_empty()) {
            lines.pop();
        }
        if lines.last().is_some_and(|l| l.trim() == "```") {
            lines.pop();
        }
    }
    let mut s = lines.join("\n");
    s.push('\n');
    s
}

pub fn parse_answer(text: &str) -> ParsedAnswer {
    let mut out = ParsedAnswer::default();
    let mut cur: Option<(String, Vec<&str>)> = None;
    for line in text.lines() {
        if let Some(p) = header(line) {
            if let Some((path, _)) = cur.take() {
                out.unterminated.push(path);
            }
            cur = Some((p, vec![]));
        } else if is_end(line) {
            if let Some((path, body)) = cur.take() {
                if safe_rel(&path) {
                    out.edits.push((path, unfence(body)));
                } else {
                    out.unsafe_paths.push(path);
                }
            }
        } else if let Some((_, body)) = cur.as_mut() {
            body.push(line);
        }
    }
    if let Some((path, _)) = cur {
        out.unterminated.push(path);
    }
    out
}

pub type Files = BTreeMap<String, String>;

pub fn write_file(root: &Path, rel: &str, content: &[u8]) -> std::io::Result<()> {
    let p = root.join(rel);
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, content)
}

/// Split parsed edits into those that may be applied and protected paths refused.
pub fn split_edits(
    task: &Task,
    edits: &[(String, String)],
) -> (Vec<(String, String)>, Vec<String>) {
    let mut ok = vec![];
    let mut refused = vec![];
    for (p, c) in edits {
        if task.is_protected(p) {
            refused.push(p.clone());
        } else {
            ok.push((p.clone(), c.clone()));
        }
    }
    (ok, refused)
}

/// Fresh sandbox holding exactly `files`.
pub fn prepare_sandbox(root: &Path, files: &Files) -> std::io::Result<()> {
    let _ = std::fs::remove_dir_all(root);
    std::fs::create_dir_all(root)?;
    for (k, v) in files {
        write_file(root, k, v.as_bytes())?;
    }
    Ok(())
}

// ---- command execution ----

pub struct Run {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub ms: u64,
    pub timed_out: bool,
    pub spawn_error: Option<String>,
}

impl Run {
    pub fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Run one command with a scrubbed environment (no ambient authority).
pub fn run_cmd(
    argv: &[String],
    cwd: &Path,
    home: &Path,
    extra: &[(&str, &str)],
    timeout: Duration,
) -> Run {
    let t0 = Instant::now();
    let tmp = std::env::temp_dir();
    let tag = format!(
        "{}-{}",
        std::process::id(),
        t0.elapsed().as_nanos() ^ (cwd.as_os_str().len() as u128)
    );
    let (so, se) = (
        tmp.join(format!("hn17-o-{tag}-{:p}", &t0)),
        tmp.join(format!("hn17-e-{tag}-{:p}", &t0)),
    );
    let fail = |e: String| Run {
        code: None,
        stdout: String::new(),
        stderr: String::new(),
        ms: 0,
        timed_out: false,
        spawn_error: Some(e),
    };
    let (Ok(fo), Ok(fe)) = (std::fs::File::create(&so), std::fs::File::create(&se)) else {
        return fail("cannot create capture files".into());
    };
    let _ = std::fs::create_dir_all(home);
    let mut c = Command::new(&argv[0]);
    c.args(&argv[1..])
        .current_dir(cwd)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", home)
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::from(fo))
        .stderr(Stdio::from(fe));
    for (k, v) in extra {
        c.env(k, v);
    }
    let mut child = match c.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = (std::fs::remove_file(&so), std::fs::remove_file(&se));
            return fail(format!("spawn {}: {e}", argv[0]));
        }
    };
    let (mut timed_out, mut code) = (false, None);
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                code = s.code();
                break;
            }
            Ok(None) if t0.elapsed() > timeout => {
                let _ = child.kill();
                let _ = child.wait();
                timed_out = true;
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(15)),
            Err(_) => break,
        }
    }
    let read =
        |p: &Path| String::from_utf8_lossy(&std::fs::read(p).unwrap_or_default()).into_owned();
    let r = Run {
        code,
        stdout: read(&so),
        stderr: read(&se),
        ms: t0.elapsed().as_millis() as u64,
        timed_out,
        spawn_error: None,
    };
    let _ = (std::fs::remove_file(&so), std::fs::remove_file(&se));
    r
}

#[derive(Clone, Debug, Default)]
pub struct Grade {
    pub passed: bool,
    /// Index of the first failing command; `None` when passed or when untested.
    pub failed_cmd: Option<usize>,
    pub output_tail: String,
    pub ms: u64,
    /// A required tool was not provided: the cell is untested, never a failure.
    pub untested: Option<String>,
}

pub fn tail_bytes(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut i = s.len() - max;
    while !s.is_char_boundary(i) {
        i += 1;
    }
    format!("[...]\n{}", &s[i..])
}

/// Restore protected files, then run the grader commands in order; the first
/// failure decides. `work` holds only scratch (a throwaway HOME).
pub fn grade(sandbox: &Path, work: &Path, task: &Task, step: usize, tools: &Tools) -> Grade {
    for (k, v) in task.grader_files(step) {
        let _ = write_file(sandbox, &k, &v);
    }
    // Visible tests are pinned too: restore pristine copies of every protected project file.
    for (k, v) in &task.project {
        if task.is_protected(k) && !task.grader_files(step).contains_key(k) {
            let _ = write_file(sandbox, k, v.as_bytes());
        }
    }
    let t0 = Instant::now();
    for (i, g) in task.steps[step].grade.iter().enumerate() {
        let argv = match tools.expand(&g.cmd) {
            Ok(a) => a,
            Err(e) => {
                return Grade {
                    untested: Some(e),
                    ms: t0.elapsed().as_millis() as u64,
                    ..Grade::default()
                }
            }
        };
        let r = run_cmd(&argv, sandbox, work, &[], Duration::from_secs(120));
        let ok = r.spawn_error.is_none()
            && !r.timed_out
            && r.code == Some(0)
            && g.expect_stdout
                .as_ref()
                .is_none_or(|e| r.stdout.contains(e.as_str()));
        if !ok {
            let mut out = r.combined();
            if let Some(e) = &r.spawn_error {
                out = e.clone();
            } else if r.timed_out {
                out.push_str("\n[timed out]");
            } else if r.code == Some(0) {
                out.push_str("\n[command succeeded but its output lacks the required text]");
            }
            return Grade {
                passed: false,
                failed_cmd: Some(i),
                output_tail: tail_bytes(&out, 3000),
                ms: t0.elapsed().as_millis() as u64,
                untested: None,
            };
        }
    }
    Grade {
        passed: true,
        ms: t0.elapsed().as_millis() as u64,
        ..Grade::default()
    }
}
