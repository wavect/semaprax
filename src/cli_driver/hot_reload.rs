//! Bounded stdio control adapter for an explicitly selected development session.
//!
//! This is deliberately a thin owner of the stdio vocabulary.  Project
//! admission, watching, compatibility and replacement remain in `project`.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use semaprax::project::{
    HotReloadPlan, HotReloadWatcher, HotReloadWatcherUpdate, PreparedProjectInterpreterOptions,
};
use serde::de::{self, MapAccess, Visitor};
use serde::Deserialize;
use serde_json::{json, Value};

const SCHEMA: &str = "semaprax.hot-reload-control.v1";
const MAX_FRAME_BYTES: usize = 4096;
const MAX_RESPONSES: usize = 64;
const MAX_RESPONSE_BYTES: usize = 8192;

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
                    "start" | "status" | "plan" | "activate" | "stop"
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
    let [manifest, format] = args else {
        eprintln!("dev requires exactly <semaprax.toml> --jsonl");
        return Err(2);
    };
    if format != "--jsonl" || manifest.is_empty() || manifest.starts_with('-') {
        eprintln!("dev requires exactly <semaprax.toml> --jsonl");
        return Err(2);
    }
    let stdin = io::stdin();
    let mut output = io::stdout().lock();
    run_jsonl(PathBuf::from(manifest), stdin.lock(), &mut output)
}

fn run_jsonl(
    manifest: PathBuf,
    mut input: impl BufRead,
    output: &mut impl Write,
) -> Result<(), u8> {
    let mut watcher: Option<HotReloadWatcher> = None;
    let mut retained_plan: Option<HotReloadPlan> = None;
    let mut previous_id = None;
    for _ in 0..MAX_RESPONSES {
        let frame = match read_frame(&mut input) {
            Ok(frame) => frame,
            Err(message) => {
                write_error(output, 0, message)?;
                return Err(2);
            }
        };
        let Some(frame) = frame else {
            if let Some(watcher) = watcher.as_mut() {
                watcher.stop();
            }
            return Ok(());
        };
        let request: Request = match serde_json::from_slice(&frame) {
            Ok(request) => request,
            Err(_) => {
                write_error(output, 0, "malformed control frame")?;
                continue;
            }
        };
        if previous_id.is_some_and(|last| request.id <= last) {
            write_error(output, request.id, "request id is stale")?;
            continue;
        }
        previous_id = Some(request.id);
        match request.op.as_str() {
            "start" => {
                if watcher.is_some() {
                    write_error(output, request.id, "session already started")?;
                    continue;
                }
                match HotReloadWatcher::start(
                    &manifest,
                    PreparedProjectInterpreterOptions::default(),
                ) {
                    Ok(value) => {
                        watcher = Some(value);
                        write_status(
                            output,
                            request.id,
                            watcher.as_ref().unwrap(),
                            "started",
                            None,
                        )?;
                    }
                    Err(_) => write_error(output, request.id, "session startup rejected")?,
                }
            }
            "status" => match watcher.as_ref() {
                Some(value) => write_status(output, request.id, value, "status", None)?,
                None => write_error(output, request.id, "session is not started")?,
            },
            "plan" => match watcher.as_mut() {
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
                            retained_plan = Some(plan);
                            write_status(output, request.id, value, event, Some(rendered))?;
                        }
                        Err(_) => write_status(output, request.id, value, event, None)?,
                    }
                }
                None => write_error(output, request.id, "session is not started")?,
            },
            "activate" => match (watcher.as_mut(), retained_plan.take()) {
                (Some(value), Some(plan)) => match value.activate(plan) {
                    Ok(()) => write_status(output, request.id, value, "activated", None)?,
                    Err(_) => write_status(output, request.id, value, "activation_rejected", None)?,
                },
                (Some(_), None) => write_error(output, request.id, "no retained activation plan")?,
                (None, _) => write_error(output, request.id, "session is not started")?,
            },
            "stop" => {
                if let Some(value) = watcher.as_mut() {
                    value.stop();
                }
                write_json(
                    output,
                    json!({"schema": SCHEMA, "id": request.id, "event": "stopped"}),
                )?;
                return Ok(());
            }
            _ => unreachable!("request parser closes the operation set"),
        }
    }
    if let Some(watcher) = watcher.as_mut() {
        watcher.stop();
    }
    write_error(output, 0, "control request limit exceeded")
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
        value["plan"] = plan;
    }
    write_json(output, value)
}

fn write_error(output: &mut impl Write, id: u64, message: &str) -> Result<(), u8> {
    write_json(
        output,
        json!({"schema": SCHEMA, "id": id, "event": "rejected", "message": message}),
    )
}

fn write_json(output: &mut impl Write, value: Value) -> Result<(), u8> {
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
