//! Arms: what differs between compared configurations. An arm is data
//! (`apptasks/arms.json`): a skill instruction or fixed text, a context source
//! (the full tree or a named retrieval tool), and a command view (raw, a named
//! wrapper, or stripped). Core code names no product: tools are `HARNESS_*`
//! variables and their command lines come from the arm set. Skill text is what
//! the harness's own default-skill selection renders (byte-exact upstream files
//! in the host frame); retrieval and views run the real executables.
//! Everything else (task, protocol, grader, model settings) is identical across arms.

use super::task::{run_cmd, tail_bytes, Files, Task, Tools};
use crate::skills::defaults::{DefaultSkills, TaskInput};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Core,
    Ablation,
    NegativeControl,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SkillSpec {
    None,
    /// Rendered by the host's default-skill selection from this instruction.
    Instruction {
        instruction: String,
        ids: Vec<String>,
    },
    /// A fixed text (baseline instruction or negative control).
    Text {
        id: String,
        text: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContextSpec {
    Native,
    Retrieval { tool: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewSpec {
    Raw,
    /// The same argv run once under a wrapper executable (`tool` is a `HARNESS_*` variable).
    Wrap {
        env: String,
        args: Vec<String>,
    },
    /// Negative control: failure lines removed, the run reported as OK.
    Stripped,
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub id: String,
    pub role: Role,
    pub skill: SkillSpec,
    pub context: ContextSpec,
    pub view: ViewSpec,
    /// Task classes this arm can change; `None` = all.
    pub classes: Option<Vec<String>>,
    pub label: String,
    /// Lossy transformation the harness applies to the model's answer before it is
    /// applied (negative control only): `keep_first_file`.
    pub answer_filter: Option<String>,
}

impl Arm {
    pub fn applies_to(&self, class: &str) -> bool {
        self.classes
            .as_ref()
            .is_none_or(|c| c.iter().any(|x| x == class))
    }

    pub fn retrieval_tool(&self) -> Option<&str> {
        match &self.context {
            ContextSpec::Retrieval { tool } => Some(tool),
            ContextSpec::Native => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RetrievalTool {
    pub id: String,
    /// `HARNESS_*` variable of the executable.
    pub env: String,
    /// `HARNESS_*` variable of an interpreter that runs it (scripts), if any.
    pub runner: Option<String>,
    /// Variables whose parent directories are put on the child's PATH.
    pub path_env: Vec<String>,
    pub build: Vec<String>,
    pub query: Vec<String>,
    /// Drop everything before the first occurrence (tool preamble and nags).
    pub preamble_marker: Option<String>,
    /// `line-prefix-l` (`path:L12` lines) or `src-equals` (`src=path` tokens).
    pub refs: String,
    pub index_dir: String,
    pub extra_env: Vec<(String, String)>,
}

pub struct ArmSet {
    pub arms: Vec<Arm>,
    pub retrieval: BTreeMap<String, RetrievalTool>,
    pub untested: Vec<(String, String)>,
}

fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(String::from))
        .collect()
}

impl ArmSet {
    pub fn load(dir: &Path) -> Result<ArmSet, String> {
        let v: Value = serde_json::from_slice(
            &std::fs::read(dir.join("arms.json")).map_err(|e| format!("arms.json: {e}"))?,
        )
        .map_err(|e| format!("arms.json: {e}"))?;
        let mut arms = vec![];
        for a in v["arms"].as_array().ok_or("arms.json: no arms")? {
            let id = a["id"].as_str().ok_or("arm without id")?.to_string();
            let sk = &a["skill"];
            let skill = match sk["kind"].as_str().unwrap_or("none") {
                "none" => SkillSpec::None,
                "instruction" => SkillSpec::Instruction {
                    instruction: sk["instruction"].as_str().unwrap_or("").into(),
                    ids: strs(&sk["ids"]),
                },
                "text" => SkillSpec::Text {
                    id: sk["id"].as_str().unwrap_or(&id).into(),
                    text: sk["text"].as_str().unwrap_or("").into(),
                },
                o => return Err(format!("arm {id}: unknown skill kind {o}")),
            };
            let context = match a["context"]["kind"].as_str().unwrap_or("native") {
                "native" => ContextSpec::Native,
                "retrieval" => ContextSpec::Retrieval {
                    tool: a["context"]["tool"]
                        .as_str()
                        .ok_or("retrieval arm without tool")?
                        .into(),
                },
                o => return Err(format!("arm {id}: unknown context kind {o}")),
            };
            let view = match a["view"]["kind"].as_str().unwrap_or("raw") {
                "raw" => ViewSpec::Raw,
                "stripped" => ViewSpec::Stripped,
                "wrap" => ViewSpec::Wrap {
                    env: a["view"]["env"]
                        .as_str()
                        .ok_or("wrap view without env")?
                        .into(),
                    args: strs(&a["view"]["args"]),
                },
                o => return Err(format!("arm {id}: unknown view kind {o}")),
            };
            let role = match a["role"].as_str().unwrap_or("core") {
                "core" => Role::Core,
                "ablation" => Role::Ablation,
                "negative_control" => Role::NegativeControl,
                o => return Err(format!("arm {id}: unknown role {o}")),
            };
            arms.push(Arm {
                role,
                skill,
                context,
                view,
                classes: a["classes"].is_array().then(|| strs(&a["classes"])),
                label: a["label"].as_str().unwrap_or("").into(),
                answer_filter: a["answer_filter"].as_str().map(String::from),
                id,
            });
        }
        let mut retrieval = BTreeMap::new();
        for (id, t) in v["retrieval_tools"].as_object().into_iter().flatten() {
            retrieval.insert(
                id.clone(),
                RetrievalTool {
                    id: id.clone(),
                    env: t["env"].as_str().unwrap_or("").into(),
                    runner: t["runner"].as_str().map(String::from),
                    path_env: strs(&t["path_env"]),
                    build: strs(&t["build"]),
                    query: strs(&t["query"]),
                    preamble_marker: t["preamble_marker"].as_str().map(String::from),
                    refs: t["refs"].as_str().unwrap_or("line-prefix-l").into(),
                    index_dir: t["index_dir"].as_str().unwrap_or("").into(),
                    extra_env: t["extra_env"]
                        .as_object()
                        .into_iter()
                        .flatten()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").into()))
                        .collect(),
                },
            );
        }
        let untested = v["untested"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, w)| (k.clone(), w.as_str().unwrap_or("").to_string()))
            .collect();
        for a in &arms {
            if let Some(t) = a.retrieval_tool() {
                if !retrieval.contains_key(t) {
                    return Err(format!("arm {} names unknown retrieval tool {t}", a.id));
                }
            }
        }
        Ok(ArmSet {
            arms,
            retrieval,
            untested,
        })
    }

    pub fn arm(&self, id: &str) -> Option<&Arm> {
        self.arms.iter().find(|a| a.id == id)
    }
}

pub struct SkillBlock {
    pub text: String,
    pub ids: Vec<String>,
    pub delivered: bool,
    pub note: Option<String>,
}

impl Clone for SkillBlock {
    fn clone(&self) -> Self {
        SkillBlock {
            text: self.text.clone(),
            ids: self.ids.clone(),
            delivered: self.delivered,
            note: self.note.clone(),
        }
    }
}

/// The exact skill text a model sees for an arm (empty for none).
pub fn skill_block(spec: &SkillSpec, home: &Path) -> SkillBlock {
    let (instruction, ids) = match spec {
        SkillSpec::None => {
            return SkillBlock {
                text: String::new(),
                ids: vec![],
                delivered: false,
                note: None,
            }
        }
        SkillSpec::Text { id, text } => {
            return SkillBlock {
                text: format!("{text}\n\n"),
                ids: vec![id.clone()],
                delivered: true,
                note: None,
            }
        }
        SkillSpec::Instruction { instruction, ids } => (instruction.as_str(), ids.clone()),
    };
    let sel =
        DefaultSkills::embedded(Some(home.to_path_buf()), "hn17", "bench").and_then(|mut d| {
            d.select_for_task(
                &TaskInput {
                    family: "coding",
                    instruction: Some(instruction),
                },
                64 * 1024,
            )
        });
    match sel {
        Ok(s) => {
            let mut got: Vec<String> = s
                .reports
                .iter()
                .filter(|r| r.loaded)
                .map(|r| r.id.clone())
                .collect();
            let mut want = ids;
            want.sort();
            got.sort();
            SkillBlock {
                text: format!("{}\n\n", s.text),
                delivered: want == got,
                ids: got,
                note: s.diagnostics.first().map(|d| d.message.clone()),
            }
        }
        Err(e) => SkillBlock {
            text: String::new(),
            ids: vec![],
            delivered: false,
            note: Some(e.message),
        },
    }
}

/// Every arm's skill block, rendered once into a private scratch home (state
/// files of concurrent trials would otherwise race, and a `stop` in one arm is
/// final for its session). One fresh sub-home per arm; keyed by arm id.
pub fn skill_blocks(set: &ArmSet, home: &Path) -> BTreeMap<String, SkillBlock> {
    set.arms
        .iter()
        .map(|a| {
            (
                a.id.clone(),
                skill_block(&a.skill, &home.join(a.id.replace('+', "_"))),
            )
        })
        .collect()
}

// ---- context ----

#[derive(Clone, Debug, Default)]
pub struct ContextPack {
    pub text: String,
    pub files_in_full: Vec<String>,
    pub build_ms: u64,
    pub retrieval_ms: u64,
    pub index_bytes: u64,
    pub identity: String,
    pub unavailable: Option<String>,
}

pub const MAX_OPENED_FILES: usize = 3;

fn file_block(path: &str, text: &str) -> String {
    format!(
        "--- {path} ---\n{text}{}",
        if text.ends_with('\n') { "" } else { "\n" }
    )
}

fn tree_list(files: &Files) -> String {
    files
        .iter()
        .map(|(k, v)| format!("{k} ({} bytes)", v.len()))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn native_pack(files: &Files) -> ContextPack {
    let text: String = files
        .iter()
        .map(|(k, v)| file_block(k, v))
        .collect::<Vec<_>>()
        .join("\n");
    ContextPack {
        text,
        files_in_full: files.keys().cloned().collect(),
        identity: "full project tree".into(),
        ..ContextPack::default()
    }
}

fn named_in_request(files: &Files, request: &str) -> Vec<String> {
    files
        .keys()
        .filter(|k| request.contains(k.as_str()))
        .cloned()
        .collect()
}

fn assemble(
    task: &Task,
    files: &Files,
    request: &str,
    tool: &str,
    pack: &str,
    referenced: &[String],
) -> (String, Vec<String>) {
    let mut open: Vec<String> = named_in_request(files, request);
    let named = open.len();
    for r in referenced {
        if open.len() >= named + MAX_OPENED_FILES {
            break;
        }
        if files.contains_key(r) && !open.contains(r) && !task.is_protected(r) {
            open.push(r.clone());
        }
    }
    let opened: String = open
        .iter()
        .map(|p| file_block(p, &files[p]))
        .collect::<Vec<_>>()
        .join("\n");
    (format!("Project files:\n{}\n\nRetrieval result ({tool}):\n{pack}\n\nFiles opened in full:\n{opened}", tree_list(files)), open)
}

fn tool_path(tools: &Tools, t: &RetrievalTool) -> String {
    let mut p = vec!["/usr/bin".to_string(), "/bin".to_string()];
    for v in &t.path_env {
        if let Some(d) = tools.get(v).and_then(|n| n.parent()) {
            p.insert(0, d.display().to_string());
        }
    }
    p.join(":")
}

/// Build the retrieval index of one task with the named tool (cold), then ask
/// the task query (retrieval). Both are measured; the pack is reused (warm).
pub fn retrieval_pack(
    t: &RetrievalTool,
    task: &Task,
    files: &Files,
    request: &str,
    tools: &Tools,
    work: &Path,
    slot: &str,
) -> ContextPack {
    let unavailable = |why: String| ContextPack {
        unavailable: Some(why),
        identity: t.id.clone(),
        ..ContextPack::default()
    };
    let Some(exe) = tools.get(&t.env) else {
        return unavailable(format!("{} not provided", t.env));
    };
    let dir = work
        .join("idx")
        .join(format!("{}-{}-{slot}", task.id, t.id));
    if super::task::prepare_sandbox(&dir, files).is_err() {
        return unavailable("cannot stage the project".into());
    }
    let home = work.join("home");
    let path = tool_path(tools, t);
    let mut envs: Vec<(&str, &str)> = vec![("PATH", path.as_str())];
    envs.extend(t.extra_env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
    let argv = |args: &[String]| -> Result<Vec<String>, String> {
        let mut v: Vec<String> = vec![];
        if let Some(r) = &t.runner {
            v.push(
                tools
                    .get(r)
                    .ok_or_else(|| format!("{r} not provided"))?
                    .display()
                    .to_string(),
            );
        }
        v.push(exe.display().to_string());
        v.extend(args.iter().cloned());
        Ok(v)
    };
    let query: Vec<String> = t
        .query
        .iter()
        .map(|a| a.replace("{query}", &task.query))
        .collect();
    let (b, q) = match (argv(&t.build), argv(&query)) {
        (Ok(b), Ok(q)) => (b, q),
        (Err(e), _) | (_, Err(e)) => return unavailable(e),
    };
    let build = run_cmd(&b, &dir, &home, &envs, Duration::from_secs(180));
    if build.code != Some(0) {
        return unavailable(format!(
            "{} build failed: {}",
            t.id,
            tail_bytes(&build.combined(), 200)
        ));
    }
    let ask = run_cmd(&q, &dir, &home, &envs, Duration::from_secs(120));
    if ask.code != Some(0) {
        return unavailable(format!(
            "{} query failed: {}",
            t.id,
            tail_bytes(&ask.combined(), 200)
        ));
    }
    let out = clean_tool_output(t, &ask.stdout);
    let referenced = referenced_files(t, &out);
    let (text, open) = assemble(task, files, request, &t.id, &out, &referenced);
    let pack = ContextPack {
        text,
        files_in_full: open,
        build_ms: build.ms,
        retrieval_ms: ask.ms,
        index_bytes: super::super::measure::dir_bytes(&dir.join(&t.index_dir)),
        identity: format!("{} ({})", t.id, exe.display()),
        unavailable: None,
    };
    let _ = std::fs::remove_dir_all(&dir);
    pack
}

/// Drop the tool's own preamble (marketing, telemetry notes); keep only the result.
pub fn clean_tool_output(t: &RetrievalTool, raw: &str) -> String {
    match t.preamble_marker.as_deref().and_then(|m| raw.find(m)) {
        Some(i) => raw[i..].to_string(),
        None => raw.to_string(),
    }
}

/// Files a retrieval result points at, in first-mention order.
pub fn referenced_files(t: &RetrievalTool, out: &str) -> Vec<String> {
    let mut seen: Vec<String> = vec![];
    let mut push = |p: &str| {
        let p = p.trim_matches(|c: char| c == ',' || c == ']' || c == ')' || c == '`');
        if p.contains('.') && !p.contains(' ') && !seen.iter().any(|s| s == p) {
            seen.push(p.to_string());
        }
    };
    for line in out.lines() {
        if t.refs == "src-equals" {
            if let Some(i) = line.find("src=") {
                push(line[i + 4..].split_whitespace().next().unwrap_or(""));
            }
        } else if let Some((p, rest)) = line.trim().split_once(":L") {
            if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                push(p);
            }
        }
    }
    seen
}

// ---- views ----

pub fn strip_failures(raw: &str) -> String {
    let mut keep = vec![];
    let mut skipping = false;
    for l in raw.lines() {
        let t = l.trim_start();
        let bad = t.starts_with("Traceback")
            || t.starts_with("FAIL")
            || t.starts_with("ERROR")
            || t.contains("AssertionError")
            || t.contains("Error:")
            || t.starts_with("✖")
            || t.starts_with("not ok")
            || t.starts_with("at ")
            || t.contains("... FAIL")
            || t.contains("failing tests");
        if bad {
            skipping = true;
            continue;
        }
        if skipping && (l.starts_with(' ') || l.is_empty() || t.starts_with("File ")) {
            continue;
        }
        skipping = false;
        keep.push(l);
    }
    let mut s = keep.join("\n");
    s.push_str("\nOK\n");
    s
}

/// The command a view runs: the same argv, run once, shown differently.
pub fn view_argv(view: &ViewSpec, argv: &[String], tools: &Tools) -> Result<Vec<String>, String> {
    match view {
        ViewSpec::Raw | ViewSpec::Stripped => Ok(argv.to_vec()),
        ViewSpec::Wrap { env, args } => {
            let exe = tools
                .get(env)
                .ok_or_else(|| format!("{env} not provided"))?;
            let mut v = vec![exe.display().to_string()];
            v.extend(args.iter().cloned());
            v.extend(argv.iter().cloned());
            Ok(v)
        }
    }
}

pub fn render_view(view: &ViewSpec, raw: &str) -> String {
    match view {
        ViewSpec::Stripped => strip_failures(raw),
        _ => raw.to_string(),
    }
}

// ---- prompt ----

pub const PROTOCOL: &str = "Reply with the complete new contents of every file you create or change, each in exactly this block format, and nothing else is read:\n=== FILE: relative/path ===\n<entire file content>\n=== END FILE ===\nTests, graders and semaprax.toml are read-only: edits to them are discarded.";

pub fn build_prompt(
    skill: &str,
    context: &str,
    request: &str,
    command_view: Option<&str>,
) -> String {
    let mut p = String::new();
    p.push_str(skill);
    p.push_str(PROTOCOL);
    p.push_str("\n\n## Project\n");
    p.push_str(context);
    p.push_str("\n\n## Task\n");
    p.push_str(request);
    if let Some(v) = command_view {
        p.push_str("\n\n## Output of the failing run\n");
        p.push_str(v);
    }
    p.push('\n');
    p
}

pub fn retry_prompt(first: &str, answer: &str, feedback: &str) -> String {
    format!(
        "{first}\n## Your previous answer (applied, then graded)\n{}\n\n## Grader result: FAILED\n{feedback}\n\nReply again, in the same block format, with the complete contents of every file that still needs changes.\n",
        tail_bytes(answer, 6000)
    )
}
