//! Bounded stdio control adapter for an explicitly selected development session.
//!
//! This is deliberately a thin owner of the stdio vocabulary.  Project
//! admission, watching, compatibility and replacement remain in `project`.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use semaprax::project::{
    HotReloadPlan, HotReloadWatcher, HotReloadWatcherUpdate, PreparedProjectExecutionOptions,
    PreparedProjectInterpreterOptions, ProjectExecutionCancellation,
    ProjectPreparedExecutionOutcome,
};
use serde::de::{self, MapAccess, Visitor};
use serde::Deserialize;
use serde_json::{json, Value};

const SCHEMA: &str = "semaprax.hot-reload-control.v1";
const MAX_FRAME_BYTES: usize = 4096;
const MAX_RESPONSES: usize = 64;
const MAX_RESPONSE_BYTES: usize = 8192;

#[derive(Clone, Copy)]
enum OutputMode {
    Jsonl,
    Human,
}

#[derive(Clone, Copy)]
enum Lane {
    Interpreter,
    SourceAgentUnsupported,
}

/// Owns the in-process state which must be released before any stdio exit.
///
/// In particular, a slow or disconnected consumer can make response writing
/// fail after `start`. The control adapter has no background work, so dropping
/// the retained plan and explicitly stopping its watcher is the complete,
/// bounded shutdown path for that case as well as EOF and `stop`.
struct ControlSession {
    watcher: Option<HotReloadWatcher>,
    retained_plan: Option<HotReloadPlan>,
}

impl ControlSession {
    fn stop(&mut self) {
        self.retained_plan = None;
        if let Some(watcher) = self.watcher.as_mut() {
            watcher.stop();
        }
    }
}

impl Drop for ControlSession {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug)]
struct Request {
    id: u64,
    op: String,
}

impl<'de> Deserialize<'de> for Request {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RequestVisitor;
        impl<'de> Visitor<'de> for RequestVisitor {
            type Value = Request;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a closed hot-reload control request")
            }
            fn visit_map<M>(self, mut map: M) -> Result<Request, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut schema = None;
                let mut id = None;
                let mut op = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "schema" => {
                            if schema.is_some() {
                                return Err(de::Error::duplicate_field("schema"));
                            }
                            schema = Some(map.next_value::<String>()?);
                        }
                        "id" => {
                            if id.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            id = Some(map.next_value::<u64>()?);
                        }
                        "op" => {
                            if op.is_some() {
                                return Err(de::Error::duplicate_field("op"));
                            }
                            op = Some(map.next_value::<String>()?);
                        }
                        _ => return Err(de::Error::unknown_field(&key, &["schema", "id", "op"])),
                    }
                }
                if schema.as_deref() != Some(SCHEMA) {
                    return Err(de::Error::custom("unsupported control schema"));
                }
                let id = id.ok_or_else(|| de::Error::missing_field("id"))?;
                let op = op.ok_or_else(|| de::Error::missing_field("op"))?;
                if !matches!(
                    op.as_str(),
                    "start" | "status" | "plan" | "activate" | "invoke" | "stop"
                ) {
                    return Err(de::Error::custom("unknown control operation"));
                }
                Ok(Request { id, op })
            }
        }
        deserializer.deserialize_map(RequestVisitor)
    }
}

pub(super) fn run(args: &[String]) -> Result<(), u8> {
    let (manifest, format, lane) = match args {
        [manifest, format] => (manifest, format, Lane::Interpreter),
        [manifest, format, lane] if lane == "--interpreter" => {
            (manifest, format, Lane::Interpreter)
        }
        [manifest, format, lane] if lane == "--source-agent" => {
            (manifest, format, Lane::SourceAgentUnsupported)
        }
        _ => {
            eprintln!(
                "dev requires <semaprax.toml> --jsonl|--human [--interpreter|--source-agent]"
            );
            return Err(2);
        }
    };
    let mode = match format.as_str() {
        "--jsonl" => OutputMode::Jsonl,
        "--human" => OutputMode::Human,
        _ => {
            eprintln!(
                "dev requires <semaprax.toml> --jsonl|--human [--interpreter|--source-agent]"
            );
            return Err(2);
        }
    };
    if manifest.is_empty() || manifest.starts_with('-') {
        eprintln!("dev requires <semaprax.toml> --jsonl|--human [--interpreter|--source-agent]");
        return Err(2);
    }
    let stdin = io::stdin();
    let mut output = io::stdout().lock();
    run_jsonl(
        PathBuf::from(manifest),
        stdin.lock(),
        &mut output,
        mode,
        lane,
    )
}

fn run_jsonl(
    manifest: PathBuf,
    mut input: impl BufRead,
    output: &mut impl Write,
    mode: OutputMode,
    lane: Lane,
) -> Result<(), u8> {
    let mut session = ControlSession {
        watcher: None,
        retained_plan: None,
    };
    let mut previous_id = None;
    for _ in 0..MAX_RESPONSES {
        let frame = match read_frame(&mut input) {
            Ok(frame) => frame,
            Err(message) => {
                write_error(output, mode, 0, message)?;
                return Err(2);
            }
        };
        let Some(frame) = frame else {
            return Ok(());
        };
        let request: Request = match serde_json::from_slice(&frame) {
            Ok(request) => request,
            Err(_) => {
                write_error(output, mode, 0, "malformed control frame")?;
                continue;
            }
        };
        if previous_id.is_some_and(|last| request.id <= last) {
            write_error(output, mode, request.id, "request id is stale")?;
            continue;
        }
        previous_id = Some(request.id);
        match request.op.as_str() {
            "start" => {
                if matches!(lane, Lane::SourceAgentUnsupported) {
                    write_error(output, mode, request.id, "source-Agent development sessions require the authenticated source-live migration adapter")?;
                    continue;
                }
                if session.watcher.is_some() {
                    write_error(output, mode, request.id, "session already started")?;
                    continue;
                }
                match HotReloadWatcher::start(
                    &manifest,
                    PreparedProjectInterpreterOptions::default(),
                ) {
                    Ok(value) => {
                        session.watcher = Some(value);
                        write_status(
                            output,
                            request.id,
                            session.watcher.as_ref().unwrap(),
                            "started",
                            None,
                            mode,
                        )?;
                    }
                    Err(_) => write_error(output, mode, request.id, "session startup rejected")?,
                }
            }
            "status" => match session.watcher.as_ref() {
                Some(value) => write_status(output, request.id, value, "status", None, mode)?,
                None => write_error(output, mode, request.id, "session is not started")?,
            },
            "plan" => match session.watcher.as_mut() {
                Some(value) => {
                    let update = value.poll();
                    let event = match update {
                        HotReloadWatcherUpdate::CandidateAdmitted => "candidate_admitted",
                        HotReloadWatcherUpdate::CandidateRejected => "candidate_rejected",
                        HotReloadWatcherUpdate::Unchanged => "unchanged",
                        HotReloadWatcherUpdate::Idle => "waiting_safe_point",
                        HotReloadWatcherUpdate::Stopped => "stopped",
                    };
                    match value.session().plan() {
                        Ok(plan) => {
                            let rendered: Value =
                                serde_json::from_str(&plan.to_json()).expect("plan JSON is closed");
                            session.retained_plan = Some(plan);
                            write_status(output, request.id, value, event, Some(rendered), mode)?;
                        }
                        Err(_) => write_status(output, request.id, value, event, None, mode)?,
                    }
                }
                None => write_error(output, mode, request.id, "session is not started")?,
            },
            "activate" => match (session.watcher.as_mut(), session.retained_plan.take()) {
                (Some(value), Some(plan)) => match value.activate(plan) {
                    Ok(()) => write_status(output, request.id, value, "activated", None, mode)?,
                    Err(_) => {
                        write_status(output, request.id, value, "activation_rejected", None, mode)?
                    }
                },
                (Some(_), None) => {
                    write_error(output, mode, request.id, "no retained activation plan")?
                }
                (None, _) => write_error(output, mode, request.id, "session is not started")?,
            },
            "invoke" => match session.watcher.as_ref() {
                Some(value) => match value.session().execute_entry(
                    &PreparedProjectExecutionOptions::default(),
                    &ProjectExecutionCancellation::new(),
                ) {
                    Ok(execution) => write_status(
                        output,
                        request.id,
                        value,
                        "invoked",
                        Some(json!({
                            "outcome": outcome_json(execution.outcome()),
                            "steps_used": execution.steps_used(),
                        })),
                        mode,
                    )?,
                    Err(_) => write_status(
                        output,
                        request.id,
                        value,
                        "runtime_terminal_failure",
                        None,
                        mode,
                    )?,
                },
                None => write_error(output, mode, request.id, "session is not started")?,
            },
            "stop" => {
                session.stop();
                write_json(
                    mode,
                    output,
                    json!({"schema": SCHEMA, "id": request.id, "event": "stopped"}),
                )?;
                return Ok(());
            }
            _ => unreachable!("request parser closes the operation set"),
        }
    }
    write_error(output, mode, 0, "control request limit exceeded")
}

fn read_frame(input: &mut impl BufRead) -> Result<Option<Vec<u8>>, &'static str> {
    let mut frame = Vec::new();
    loop {
        let available = input.fill_buf().map_err(|_| "control input failed")?;
        if available.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err("unterminated control frame")
            };
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if frame.len() + take > MAX_FRAME_BYTES {
            return Err("control frame exceeds its bound");
        }
        frame.extend_from_slice(&available[..take]);
        input.consume(take);
        if frame.last() == Some(&b'\n') {
            frame.pop();
            return Ok(Some(frame));
        }
    }
}

fn write_status(
    output: &mut impl Write,
    id: u64,
    watcher: &HotReloadWatcher,
    event: &str,
    plan: Option<Value>,
    mode: OutputMode,
) -> Result<(), u8> {
    let session = watcher.session();
    let mut value = json!({
        "schema": SCHEMA,
        "id": id,
        "event": event,
        "generation": session.generation(),
        "active_project_revision": session.active_project_revision(),
        "terminal_uncertainty": session.terminal(),
        "watch_state": format!("{:?}", watcher.state()).to_lowercase(),
    });
    if let Some(plan) = plan {
        if event == "invoked" {
            value["invocation"] = plan;
        } else {
            value["plan"] = plan;
        }
    }
    write_json(mode, output, value)
}

fn write_error(
    output: &mut impl Write,
    mode: OutputMode,
    id: u64,
    message: &str,
) -> Result<(), u8> {
    write_json(
        mode,
        output,
        json!({"schema": SCHEMA, "id": id, "event": "rejected", "message": message}),
    )
}

fn write_json(mode: OutputMode, output: &mut impl Write, value: Value) -> Result<(), u8> {
    if matches!(mode, OutputMode::Human) {
        let event = value["event"].as_str().unwrap_or("rejected");
        let generation = value["generation"]
            .as_u64()
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_owned());
        let revision = value["active_project_revision"].as_str().unwrap_or("-");
        let message = value["message"].as_str().unwrap_or("");
        let terminal = value["terminal_uncertainty"].as_bool().unwrap_or(false);
        let terminal = if terminal {
            "terminal_uncertainty; "
        } else {
            ""
        };
        let line = format!(
            "SEMAPRAX dev {event}: generation {generation}; active {revision}; {terminal}{message}\n"
        );
        return output
            .write_all(line.as_bytes())
            .and_then(|_| output.flush())
            .map_err(|_| 1);
    }
    let text = value.to_string();
    if text.len() > MAX_RESPONSE_BYTES {
        return Err(1);
    }
    output
        .write_all(text.as_bytes())
        .and_then(|_| output.write_all(b"\n"))
        .map_err(|_| 1)?;
    output.flush().map_err(|_| 1)
}

fn outcome_json(outcome: &ProjectPreparedExecutionOutcome) -> Value {
    match outcome {
        ProjectPreparedExecutionOutcome::Returned(value) => {
            json!({"kind":"returned","value":value})
        }
        ProjectPreparedExecutionOutcome::LanguageFailure(status) => {
            json!({"kind":"language_failure","status":format!("{status:?}")})
        }
        ProjectPreparedExecutionOutcome::FuelExhausted => json!({"kind":"fuel_exhausted"}),
        ProjectPreparedExecutionOutcome::CallDepthExceeded => json!({"kind":"call_depth_exceeded"}),
        ProjectPreparedExecutionOutcome::Cancelled { before_step } => {
            json!({"kind":"cancelled","before_step":before_step})
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-dev-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let original =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for relative in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            fs::copy(original.join(relative), root.join(relative)).unwrap();
        }
        root
    }

    #[test]
    fn jsonl_session_is_explicit_bounded_and_rejects_hostile_frames() {
        let root = fixture();
        let input = concat!(
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":1,\"op\":\"start\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":2,\"op\":\"status\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":2,\"op\":\"status\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":3,\"op\":\"stop\",\"op\":\"stop\"}\n"
        );
        let mut output = Vec::new();
        run_jsonl(
            root.join("semaprax.toml"),
            io::Cursor::new(input),
            &mut output,
            OutputMode::Jsonl,
            Lane::Interpreter,
        )
        .unwrap();
        let rows: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0]["event"], "started");
        assert_eq!(rows[1]["event"], "status");
        assert_eq!(rows[2]["message"], "request id is stale");
        assert_eq!(rows[3]["message"], "malformed control frame");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn jsonl_plan_and_activation_keep_the_session_in_process() {
        let root = fixture();
        let frames = [
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":1,\"op\":\"start\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":2,\"op\":\"plan\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":3,\"op\":\"activate\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":4,\"op\":\"stop\"}\n",
        ];
        let mut output = Vec::new();
        run_jsonl(
            root.join("semaprax.toml"),
            RewriteBeforePlan::new(&root, frames),
            &mut output,
            OutputMode::Jsonl,
            Lane::Interpreter,
        )
        .unwrap();
        let rows: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(rows[1]["event"], "candidate_admitted");
        assert_eq!(rows[1]["plan"]["decision"], "eligible_code_replacement");
        assert_eq!(rows[2]["event"], "activated");
        assert_eq!(rows[2]["generation"], 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn human_mode_renders_saved_session_status_without_json() {
        let root = fixture();
        let input = concat!(
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":1,\"op\":\"start\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":2,\"op\":\"status\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":3,\"op\":\"stop\"}\n"
        );
        let mut output = Vec::new();
        run_jsonl(
            root.join("semaprax.toml"),
            io::Cursor::new(input),
            &mut output,
            OutputMode::Human,
            Lane::Interpreter,
        )
        .unwrap();
        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains("SEMAPRAX dev started"));
        assert!(rendered.contains("SEMAPRAX dev status"));
        assert!(!rendered.contains('{'));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn source_agent_lane_refuses_start_without_creating_a_session() {
        let root = fixture();
        let input = concat!(
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":1,\"op\":\"start\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":2,\"op\":\"status\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":3,\"op\":\"stop\"}\n"
        );
        let mut output = Vec::new();
        run_jsonl(
            root.join("semaprax.toml"),
            io::Cursor::new(input),
            &mut output,
            OutputMode::Jsonl,
            Lane::SourceAgentUnsupported,
        )
        .unwrap();
        let rows = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows[0]["event"], "rejected");
        assert_eq!(
            rows[0]["message"],
            "source-Agent development sessions require the authenticated source-live migration adapter"
        );
        assert_eq!(rows[1]["message"], "session is not started");
        assert_eq!(rows[2]["event"], "stopped");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn human_terminal_uncertainty_matches_the_machine_field() {
        let value = json!({
            "schema": SCHEMA,
            "id": 7,
            "event": "activation_rejected",
            "generation": 3,
            "active_project_revision": "sha256:active",
            "terminal_uncertainty": true,
            "watch_state": "failed",
        });
        let mut output = Vec::new();
        write_json(OutputMode::Human, &mut output, value).unwrap();
        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains("activation_rejected"));
        assert!(rendered.contains("terminal_uncertainty"));
        assert!(rendered.contains("sha256:active"));
    }

    #[test]
    fn slow_consumer_receives_complete_ordered_bounded_frames() {
        let root = fixture();
        let input = concat!(
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":1,\"op\":\"start\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":2,\"op\":\"status\"}\n",
            "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":3,\"op\":\"stop\"}\n"
        );
        let mut output = SlowWriter::default();
        run_jsonl(
            root.join("semaprax.toml"),
            io::Cursor::new(input),
            &mut output,
            OutputMode::Jsonl,
            Lane::Interpreter,
        )
        .unwrap();
        assert!(output.writes > 3, "fixture must force partial writes");
        assert_eq!(output.flushes, 3);
        let rows = String::from_utf8(output.bytes)
            .unwrap()
            .lines()
            .map(|line| {
                assert!(line.len() <= MAX_RESPONSE_BYTES);
                serde_json::from_str::<Value>(line).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.iter()
                .map(|row| row["event"].as_str())
                .collect::<Vec<_>>(),
            vec![Some("started"), Some("status"), Some("stopped")]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disconnected_consumer_exits_through_bounded_session_shutdown() {
        let root = fixture();
        let input = "{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":1,\"op\":\"start\"}\n";
        let mut output = RefusingWriter;
        assert_eq!(
            run_jsonl(
                root.join("semaprax.toml"),
                io::Cursor::new(input),
                &mut output,
                OutputMode::Jsonl,
                Lane::Interpreter,
            )
            .unwrap_err(),
            1
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[derive(Default)]
    struct SlowWriter {
        bytes: Vec<u8>,
        writes: usize,
        flushes: usize,
    }

    impl Write for SlowWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let count = bytes.len().min(3);
            self.bytes.extend_from_slice(&bytes[..count]);
            self.writes += 1;
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            Ok(())
        }
    }

    struct RefusingWriter;

    impl Write for RefusingWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "fixture disconnect",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct RewriteBeforePlan {
        root: PathBuf,
        frames: Vec<Vec<u8>>,
        index: usize,
        current: Vec<u8>,
    }

    impl RewriteBeforePlan {
        fn new(root: &std::path::Path, frames: [&str; 4]) -> Self {
            Self {
                root: root.to_path_buf(),
                frames: frames
                    .into_iter()
                    .map(str::as_bytes)
                    .map(Vec::from)
                    .collect(),
                index: 0,
                current: Vec::new(),
            }
        }
    }

    impl io::Read for RewriteBeforePlan {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let available = self.fill_buf()?;
            let count = available.len().min(buffer.len());
            buffer[..count].copy_from_slice(&available[..count]);
            self.consume(count);
            Ok(count)
        }
    }

    impl BufRead for RewriteBeforePlan {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            if self.current.is_empty() && self.index < self.frames.len() {
                if self.index == 1 {
                    let path = self.root.join("src/app.spx");
                    let source = fs::read_to_string(&path)?;
                    let changed = source.replacen("multiply(6, 7)", "multiply(6, 8)", 1);
                    assert_ne!(source, changed);
                    fs::write(
                        path,
                        semaprax::format::canonical(
                            &semaprax::parse(&changed, std::path::Path::new("src/app.spx"))
                                .unwrap(),
                        ),
                    )?;
                }
                self.current = self.frames[self.index].clone();
                self.index += 1;
            }
            Ok(&self.current)
        }

        fn consume(&mut self, amount: usize) {
            self.current.drain(..amount);
        }
    }
}
