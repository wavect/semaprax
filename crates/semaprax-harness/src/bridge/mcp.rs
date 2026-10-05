//! MCP stdio server (`bridge <project> --mcp`) exposing the default skills as
//! MCP tools and prompts. It is a thin transport over `SkillsBridge`; it carries
//! no skill text of its own. Claude Code connects to it from a project `.mcp.json`
//! (<https://code.claude.com/docs/en/mcp>). Separate from the authority-free
//! compiler MCP facade, which is untouched.

use super::frame::{read_frame, FrameError};
use super::hostskills::{self, check_host_support};
use super::negotiate::DEPTH_VAR;
use super::skills_bridge::SkillsBridge;
use crate::cli::Environment;
use crate::diag::HarnessDiagnostic;
use crate::json::{canonical, parse_frame, JsonLimits};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const FRAME_LIMIT: usize = 1 << 20;
pub const SERVER_NAME: &str = "semaprax-skills";
/// MCP protocol revisions this server answers, newest first.
pub const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub struct McpOptions {
    pub session: Option<String>,
    pub host_skills_dir: Option<PathBuf>,
    pub log: Option<PathBuf>,
}

fn err(id: &Value, code: i64, d: &HarnessDiagnostic) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": d.message, "data": {"code": d.code}}})
}

fn tools() -> Value {
    let name =
        json!({"type": "string", "description": "official skill id or alias (ponytail, caveman)"});
    let force = json!({"type": "boolean", "description": "deliver even when the host already has the skill installed"});
    let scope = json!({"type": "string", "enum": ["session", "project", "user"]});
    json!([
        {"name": "skills_list", "description": "List the default official skills with availability, mode and pinned revision.",
         "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}},
        {"name": "skills_load", "description": "Load a skill's instructions (framed, revision-locked for this session).",
         "inputSchema": {"type": "object", "properties": {"name": name, "force": force}, "required": ["name"], "additionalProperties": false}},
        {"name": "skills_use", "description": "Turn a skill on in a mode (for example ponytail lite|full|ultra, caveman on) and load its instructions.",
         "inputSchema": {"type": "object", "properties": {"name": name, "mode": {"type": "string"}, "scope": scope, "force": force}, "required": ["name"], "additionalProperties": false}},
        {"name": "skills_off", "description": "Turn a skill off (final for the session) or `all`.",
         "inputSchema": {"type": "object", "properties": {"name": name, "scope": scope}, "required": ["name"], "additionalProperties": false}},
        {"name": "skills_status", "description": "Active mode, pinned revision, host ownership, delivery observations and pending updates.",
         "inputSchema": {"type": "object", "properties": {"updates": {"type": "boolean"}}, "additionalProperties": false}},
        {"name": "skills_resource", "description": "Load one bounded resource file of a skill (for example LICENSE).",
         "inputSchema": {"type": "object", "properties": {"name": name, "path": {"type": "string"}}, "required": ["name", "path"], "additionalProperties": false}}
    ])
}

fn method_of(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "skills_list" => "bridge/skills/list",
        "skills_load" => "bridge/skills/load",
        "skills_use" => "bridge/skills/use",
        "skills_off" => "bridge/skills/off",
        "skills_status" => "bridge/skills/status",
        "skills_resource" => "bridge/skills/resource",
        _ => return None,
    })
}

/// Model-facing text of one result: instructions when delivered, else compact JSON.
fn render(tool: &str, v: &Value) -> String {
    if let Some(t) = v["delivery"]["text"].as_str() {
        return t.to_string();
    }
    if let Some(t) = v["text"].as_str() {
        return t.to_string();
    }
    if tool == "skills_list" {
        let rows: Vec<Value> = v["skills"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|k| {
                        json!({"id": k["id"], "aliases": k["aliases"], "version": k["version"],
                "revision": k["bundle_digest"], "state": k["state"], "features": k["features"]})
                    })
                    .collect()
            })
            .unwrap_or_default();
        return canonical(&json!({"skills": rows, "host_owned": v["host_owned"]}));
    }
    if v["delivery"]["state"] == "host-owned" {
        return v["delivery"]["reason"]
            .as_str()
            .unwrap_or("host-owned")
            .to_string();
    }
    canonical(v)
}

pub fn serve<R: BufRead, W: Write>(
    reader: R,
    mut out: W,
    env: &Environment,
    project: &Path,
    opts: McpOptions,
) -> std::io::Result<()> {
    let limits = JsonLimits::frame(FRAME_LIMIT);
    let mut sb: Option<SkillsBridge> = None;
    let nested = env.vars.get(DEPTH_VAR).is_some_and(|d| d != "0")
        || env
            .vars
            .contains_key(crate::command_view::lineage::LINEAGE_VAR);
    let mut reader = reader;
    loop {
        let line = match read_frame(&mut reader, FRAME_LIMIT) {
            Ok(Some(l)) => l,
            Ok(None) => break,
            Err(e) => {
                if let FrameError::TooLarge { .. } = e {
                    let d = HarnessDiagnostic::new(
                        "SPX-HPA002",
                        format!("frame exceeds the {FRAME_LIMIT}-byte limit; session closed"),
                    );
                    let _ = writeln!(out, "{}", canonical(&err(&Value::Null, -32700, &d)));
                    let _ = out.flush();
                }
                return Err(e.into_io());
            }
        };
        if line.trim().is_empty() {
            continue;
        }
        let v = match parse_frame(line.as_bytes(), &limits) {
            Ok(v) => v,
            Err(d) => {
                writeln!(out, "{}", canonical(&err(&Value::Null, -32700, &d)))?;
                out.flush()?;
                continue;
            }
        };
        let id = v.get("id").cloned();
        let Some(method) = v.get("method").and_then(Value::as_str) else {
            continue;
        };
        let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
        let reply: Result<Value, (i64, HarnessDiagnostic)> = (|| match method {
            "initialize" => {
                if nested {
                    return Err((-32000, HarnessDiagnostic::new("SPX-HPN002", "refused: invoked from Semaprax's own bridge/MCP; calling back would recurse")));
                }
                let ci = &params["clientInfo"];
                let (cn, cv) = (
                    ci["name"].as_str().unwrap_or("mcp-client"),
                    ci["version"].as_str().unwrap_or("0"),
                );
                check_host_support(cn, cv).map_err(|d| (-32602, d))?;
                let mut b =
                    SkillsBridge::new(env, project, opts.session.as_deref(), &format!("{cn}@{cv}"));
                let set = crate::skills::official::embedded_cached();
                if let Some(dir) = &opts.host_skills_dir {
                    b.host_skills = hostskills::scan_dir(set, &env.cwd.join(dir));
                }
                b.log = opts.log.clone();
                let owned = b.ownership();
                let asked = params["protocolVersion"].as_str().unwrap_or("");
                let pv = PROTOCOLS
                    .iter()
                    .find(|p| **p == asked)
                    .unwrap_or(&PROTOCOLS[0]);
                b.observe(
                    json!({"event": "skills.session", "protocol": pv, "client": cn,
                    "client_version": cv, "host_owned": owned["host_owned"]}),
                );
                sb = Some(b);
                Ok(json!({"protocolVersion": pv,
                    "capabilities": {"tools": {"listChanged": false}, "prompts": {"listChanged": false}},
                    "serverInfo": {"name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "Default official skills (Ponytail, Caveman) from the Semaprax catalog. Call skills_list, then skills_use or skills_load; skills_off stops a skill. Model choice stays with the host.",
                    "_semaprax": {"skill_injection": owned}}))
            }
            "ping" => Ok(json!({})),
            m if m.starts_with("notifications/") => Ok(Value::Null),
            _ => {
                let b = sb.as_mut().ok_or_else(|| {
                    (
                        -32002,
                        HarnessDiagnostic::new("SPX-HPN004", "send `initialize` first"),
                    )
                })?;
                match method {
                    "tools/list" => Ok(json!({"tools": tools()})),
                    "tools/call" => {
                        let tool = params["name"].as_str().unwrap_or("");
                        let Some(m) = method_of(tool) else {
                            return Err((
                                -32602,
                                HarnessDiagnostic::new(
                                    "SPX-HPN005",
                                    format!("unknown tool `{tool}`"),
                                ),
                            ));
                        };
                        let args = params
                            .get("arguments")
                            .cloned()
                            .unwrap_or_else(|| json!({}));
                        Ok(match b.handle(m, &args) {
                            Ok(r) => {
                                json!({"content": [{"type": "text", "text": render(tool, &r)}], "isError": false})
                            }
                            Err(d) => {
                                json!({"content": [{"type": "text", "text": d.to_string()}], "isError": true})
                            }
                        })
                    }
                    "prompts/list" => {
                        let set = crate::skills::official::embedded_cached();
                        let prompts: Vec<Value> = set.embedded_skills().map(|k| json!({"name": k.id,
                            "description": format!("Use the official {} skill ({}), revision-locked by Semaprax", k.id, k.version),
                            "arguments": [{"name": "mode", "description": "optional mode", "required": false}]})).collect();
                        Ok(json!({"prompts": prompts}))
                    }
                    "prompts/get" => {
                        let name = params["name"].as_str().unwrap_or("");
                        let mut a = json!({"name": name});
                        if let Some(m) = params["arguments"]["mode"].as_str() {
                            a["mode"] = json!(m);
                        }
                        let r = b.handle("bridge/skills/use", &a).map_err(|d| (-32602, d))?;
                        Ok(json!({"description": format!("{name} via Semaprax"),
                            "messages": [{"role": "user", "content": {"type": "text", "text": render("skills_use", &r)}}]}))
                    }
                    other => Err((
                        -32601,
                        HarnessDiagnostic::new("SPX-HPN004", format!("unknown method `{other}`")),
                    )),
                }
            }
        })();
        if let Some(i) = id {
            let frame = match reply {
                Ok(r) => json!({"jsonrpc": "2.0", "id": i, "result": r}),
                Err((c, d)) => err(&i, c, &d),
            };
            writeln!(out, "{}", canonical(&frame))?;
            out.flush()?;
        }
    }
    Ok(())
}
