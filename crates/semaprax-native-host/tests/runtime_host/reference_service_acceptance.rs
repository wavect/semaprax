//! Acceptance for the runnable reference-service host: a real server
//! process serves login/CRUD/job routes from the scaffold's checked `.spx`
//! decisions with physical persistence, survives kill-and-restart without
//! duplicating settled effects, and refuses fixture-mode intent.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const SERVER: &str = env!("CARGO_BIN_EXE_semaprax-reference-service");
const READY_TIMEOUT: Duration = Duration::from_secs(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Canonical host-mode service configuration (sorted keys plus LF, exactly
/// as `service_config::decode` requires). Origins use the `.invalid` TLD so
/// no real peer can exist; the delivery attempt fails closed by design.
const HOST_CONFIG: &str = "{\"database\":{\"adapter\":\"sqlite\",\"dsn_secret_ref\":\"db.primary\",\"migration_table\":\"semaprax_migrations\"},\"http\":{\"adapter\":\"native\",\"listen_origin\":\"https://service.invalid\",\"tls_profile\":\"modern\"},\"mode\":\"host\",\"schema\":\"semaprax.service-config.v1\",\"secrets\":{\"password_pepper_ref\":\"auth.pepper\",\"session_signing_key_ref\":\"auth.session\",\"webhook_signing_key_ref\":\"webhook.signing\"},\"telemetry\":{\"adapter\":\"otlp\",\"endpoint_origin\":\"https://telemetry.invalid:9\"}}\n";

static NEXT_WORKDIR: AtomicU64 = AtomicU64::new(0);

struct Workdir {
    root: PathBuf,
}

impl Workdir {
    fn create(label: &str) -> Self {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("canonicalize test temp root");
        for _ in 0..32 {
            let nonce = NEXT_WORKDIR.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!(
                "semaprax-reference-acceptance-{label}-{}-{nonce}",
                std::process::id()
            ));
            if std::fs::create_dir(&path).is_ok() {
                for name in ["state", "outbound", "secrets", "bundle"] {
                    std::fs::create_dir(path.join(name)).unwrap();
                }
                return Self { root: path };
            }
        }
        panic!("could not allocate a unique workdir");
    }

    fn write_inputs(&self) {
        std::fs::write(self.root.join("service.config.json"), HOST_CONFIG).unwrap();
        std::fs::write(self.root.join("secrets").join("auth.pepper"), [1_u8; 32]).unwrap();
        std::fs::write(self.root.join("secrets").join("auth.session"), [2_u8; 32]).unwrap();
        std::fs::write(
            self.root.join("secrets").join("webhook.signing"),
            [3_u8; 32],
        )
        .unwrap();
        std::fs::write(
            self.root.join("secrets").join("db.primary"),
            b"held-but-unconnected",
        )
        .unwrap();
    }

    fn example_project(&self) -> PathBuf {
        // The project loader rejects `.`/`..` components, so the fixture
        // path is canonicalized before the server loads it.
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("task-service-project")
            .canonicalize()
            .expect("canonicalize task-service-project fixture")
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn census(&self, name: &str) -> Vec<String> {
        let mut entries: Vec<String> = std::fs::read_dir(self.path(name))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        entries.sort();
        entries
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A server child that is always killed on drop, so a failing test never
/// leaves a listener behind.
struct Server {
    child: Child,
    lines: mpsc::Receiver<String>,
    port: u16,
}

impl Server {
    fn spawn(workdir: &Workdir, extra: &[&str]) -> Self {
        for _ in 0..5 {
            let port = free_port();
            let mut command = Command::new(SERVER);
            command
                .arg("serve")
                .arg("--project")
                .arg(workdir.example_project())
                .arg("--config")
                .arg(workdir.path("service.config.json"))
                .arg("--state-dir")
                .arg(workdir.path("state"))
                .arg("--outbound-dir")
                .arg(workdir.path("outbound"))
                .arg("--secrets-dir")
                .arg(workdir.path("secrets"))
                .arg("--bundle-dir")
                .arg(workdir.path("bundle"))
                .arg("--port")
                .arg(port.to_string())
                .args(extra)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut child = command.spawn().expect("spawn reference server");
            let stdout = child.stdout.take().expect("piped stdout");
            let (sender, lines) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
            let server = Self { child, lines, port };
            match server.wait_for_ready() {
                Ok(()) => return server,
                Err(_) => {
                    let mut server = server;
                    server.kill_and_wait();
                    // A lost port race prints a bind refusal; anything else
                    // is a real failure.
                    let mut stderr = String::new();
                    if let Some(mut pipe) = server.child.stderr.take() {
                        let _ = pipe.read_to_string(&mut stderr);
                    }
                    if !stderr.contains("cannot bind the loopback listener") {
                        panic!("server failed before ready: {stderr}");
                    }
                }
            }
        }
        panic!("could not bind a loopback port after retries");
    }

    fn wait_for_ready(&self) -> Result<(), String> {
        let deadline = Instant::now() + READY_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("server never printed ready".to_owned());
            }
            match self.lines.recv_timeout(remaining) {
                Ok(line) if line.starts_with("ready ") => return Ok(()),
                Ok(_) => continue,
                Err(_) => return Err("server output ended before ready".to_owned()),
            }
        }
    }

    fn kill_and_wait(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.kill_and_wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn http(port: u16, method: &str, target: &str, body: &str, token: Option<&str>) -> (u16, String) {
    let deadline = Instant::now() + REQUEST_TIMEOUT;
    let mut stream = loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(stream) => break stream,
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => panic!("loopback connect failed: {error}"),
        }
    };
    stream.set_read_timeout(Some(REQUEST_TIMEOUT)).unwrap();
    stream.set_write_timeout(Some(REQUEST_TIMEOUT)).unwrap();
    let mut request = format!(
        "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    if let Some(token) = token {
        request.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    request.push_str("\r\n");
    request.push_str(body);
    stream.write_all(request.as_bytes()).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let text = String::from_utf8(response).expect("server speaks UTF-8 JSON");
    let status = text
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .expect("HTTP status line");
    let body = text.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
    (status, body)
}

fn field<'a>(body: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\":\"");
    let start = body
        .find(&needle)
        .unwrap_or_else(|| panic!("{key} missing in {body}"))
        + needle.len();
    let end = body[start..].find('"').unwrap();
    &body[start..start + end]
}

/// Some sandboxes deny loopback sockets outright (`EPERM` on bind). Only
/// that precise denial skips the socket acceptance test; every other
/// failure is real, and CI runs it fully.
fn loopback_denied() -> bool {
    matches!(
        std::net::TcpListener::bind(("127.0.0.1", 0)),
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied
    )
}

#[test]
fn login_crud_job_restart_preserves_state_without_redispatch() {
    if loopback_denied() {
        eprintln!("skipping: sandbox denies loopback bind");
        return;
    }
    let workdir = Workdir::create("restart");
    workdir.write_inputs();

    let server = Server::spawn(&workdir, &[]);
    let port = server.port;

    let (status, body) = http(
        port,
        "POST",
        "/v1/register",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 201, "{body}");

    let (status, body) = http(
        port,
        "POST",
        "/v1/login",
        r#"{"username":"alice","password":"correct horse 7"}"#,
        None,
    );
    assert_eq!(status, 200, "{body}");
    let token = field(&body, "token").to_owned();

    let (status, body) = http(
        port,
        "POST",
        "/v1/tasks",
        r#"{"title":"write the report"}"#,
        Some(&token),
    );
    assert_eq!(status, 201, "{body}");

    let (status, body) = http(
        port,
        "PATCH",
        "/v1/tasks/1",
        r#"{"status":"done"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");

    let (status, body) = http(
        port,
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"job-1","desc":"task-1"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "outcome"), "created");

    // No peer exists at the `.invalid` telemetry origin, so the durable
    // attempt fails closed and settles `Uncertain`; the job still
    // completes exactly once and the durable marker is committed.
    let (status, body) = http(port, "POST", "/v1/jobs/1/complete", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "webhook"), "uncertain");
    let digest = field(&body, "state").to_owned();

    let (status, body) = http(port, "GET", "/v1/jobs/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), "completed");

    let outbound_before = workdir.census("outbound");
    let markers_before: Vec<_> = outbound_before
        .iter()
        .filter(|name| name.ends_with(".marker"))
        .collect();
    assert_eq!(markers_before.len(), 1, "{outbound_before:?}");
    let state_before = workdir.census("state");
    assert!(!state_before.is_empty());

    drop(server);

    // Restart from the operator-retained digest: state survives, and no
    // settled effect is duplicated.
    let server = Server::spawn(&workdir, &["--state", digest.as_str()]);
    let port = server.port;

    let (status, body) = http(port, "GET", "/v1/health", "", None);
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), digest.as_str());

    let (status, body) = http(port, "GET", "/v1/tasks/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "title"), "write the report");
    assert_eq!(field(&body, "status"), "done");

    let (status, body) = http(port, "GET", "/v1/jobs/1", "", Some(&token));
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "state"), "completed");
    assert_eq!(field(&body, "webhook"), "uncertain");

    let (status, _) = http(port, "POST", "/v1/jobs/1/complete", "", Some(&token));
    assert_eq!(status, 409);

    let (status, body) = http(
        port,
        "POST",
        "/v1/jobs/enqueue",
        r#"{"key":"job-1","desc":"task-1"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");
    assert_eq!(field(&body, "outcome"), "duplicate");
    assert_eq!(field(&body, "state"), digest.as_str());

    // No new outbound file: nothing redispatched after the restart.
    assert_eq!(workdir.census("outbound"), outbound_before);
    assert_eq!(workdir.census("state"), state_before);

    // A fresh mutation still commits exactly one new snapshot.
    let (status, body) = http(
        port,
        "PATCH",
        "/v1/tasks/1",
        r#"{"status":"open"}"#,
        Some(&token),
    );
    assert_eq!(status, 200, "{body}");
    assert_ne!(field(&body, "state"), digest.as_str());
    assert_eq!(workdir.census("state").len(), state_before.len() + 1);

    // Row-level authorization survives the restart too: a second account
    // cannot read the first account's row.
    let (status, _) = http(
        port,
        "POST",
        "/v1/register",
        r#"{"username":"bob","password":"another secret 8"}"#,
        None,
    );
    assert_eq!(status, 201);
    let (status, body) = http(
        port,
        "POST",
        "/v1/login",
        r#"{"username":"bob","password":"another secret 8"}"#,
        None,
    );
    assert_eq!(status, 200, "{body}");
    let bob = field(&body, "token").to_owned();
    let (status, _) = http(port, "GET", "/v1/tasks/1", "", Some(&bob));
    assert_eq!(status, 403);

    drop(server);
    // The run bundle was written and verified at startup.
    let manifest =
        std::fs::read_to_string(workdir.path("bundle").join("bundle-manifest.json")).unwrap();
    assert!(
        manifest.contains("semaprax.reference-service.bundle.v1"),
        "{manifest}"
    );
    assert!(manifest.contains("service.config.json"), "{manifest}");
    assert!(
        manifest.contains("service-host-adapter-request.json"),
        "{manifest}"
    );
}

#[test]
fn fixture_configuration_is_refused_without_a_runner() {
    let workdir = Workdir::create("fixture");
    workdir.write_inputs();
    // The checked-in credential-free fixture instance, verbatim.
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("task-service-project")
        .join("service.config.json");
    std::fs::copy(fixture, workdir.path("service.config.json")).unwrap();

    let output = Command::new(SERVER)
        .arg("serve")
        .arg("--project")
        .arg(workdir.example_project())
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--state-dir")
        .arg(workdir.path("state"))
        .arg("--outbound-dir")
        .arg(workdir.path("outbound"))
        .arg("--secrets-dir")
        .arg(workdir.path("secrets"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .arg("--port")
        // The fixture refusal precedes the bind, so no listener is ever
        // opened; a fixed dummy port avoids probing for a free one.
        .arg("9")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run reference server");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("fixture-mode configuration has no host runner"),
        "{stderr}"
    );
    // Nothing was served and no bundle was written.
    assert!(workdir.census("bundle").is_empty());
}

#[test]
fn bundle_command_writes_verifies_and_rejects_tamper() {
    let workdir = Workdir::create("bundle");
    workdir.write_inputs();

    let output = Command::new(SERVER)
        .arg("bundle")
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundle command");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("bundle sha256:"), "{stdout}");
    let first = stdout.trim().to_owned();

    // A byte-identical replay yields the same manifest digest.
    let output = Command::new(SERVER)
        .arg("bundle")
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundle command");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), first);

    // Tampering with a bundled file breaks verification on rewrite.
    std::fs::write(
        workdir.path("bundle").join("service.config.json"),
        HOST_CONFIG.replace("sqlite", "tampered"),
    )
    .unwrap();
    let output = Command::new(SERVER)
        .arg("bundle")
        .arg("--config")
        .arg(workdir.path("service.config.json"))
        .arg("--bundle-dir")
        .arg(workdir.path("bundle"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("run bundle command");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
}
