//! The runnable reference-service host process.
//!
//! `semaprax-reference-service serve` loads the existing service scaffold
//! project, decodes its checked host-mode configuration, joins it to
//! operator-held grants, and serves the login/CRUD/job API on loopback.
//! `semaprax-reference-service bundle` writes and verifies the digest-bound
//! run bundle without serving. Exit codes: `0` success, `2` usage or
//! refusal, `1` unexpected runtime failure.

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn main() {
    eprintln!("error: the reference service host needs a desktop/server platform");
    std::process::exit(2);
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() {
    std::process::exit(real::run(std::env::args().collect()));
}

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
mod real {
    use std::path::PathBuf;

    use semaprax::network_provider::TcpNetworkProvider;
    use semaprax::project::with_authenticated_project;
    use semaprax_native_host::outbound_delivery_store::OutboundCheckpointSyncMode;
    use semaprax_native_host::reference_service::bundle;
    use semaprax_native_host::reference_service::decisions::{DecisionEngine, DECISION_MAX_STEPS};
    use semaprax_native_host::reference_service::mapping::{
        self, BindRefusal, HostGrants, InitialState, DEFAULT_SESSION_ABSOLUTE_SECONDS,
        DEFAULT_SESSION_IDLE_SECONDS, MAX_SESSION_LIFETIME_SECONDS,
    };
    use semaprax_native_host::reference_service::secrets;
    use semaprax_native_host::reference_service::serve;
    use semaprax_native_rust_interop_platform as platform;

    const DEFAULT_DEPLOYMENT: &str = "reference-service-local-v1";
    const MAX_CONFIG_BYTES: usize = 64 * 1024;

    pub(super) fn run(args: Vec<String>) -> i32 {
        if args.len() < 2 {
            return usage();
        }
        match args[1].as_str() {
            "serve" => serve_command(&args[2..]),
            "bundle" => bundle_command(&args[2..]),
            _ => usage(),
        }
    }

    fn usage() -> i32 {
        eprintln!(
            "usage: semaprax-reference-service serve --project <dir> --config <service.config.json> --state-dir <dir> --outbound-dir <dir> --secrets-dir <dir> --bundle-dir <dir> --port <1-65535> [--state <sha256:hex>] [--deployment <id>] [--max-steps <n>] [--session-idle-seconds <n>] [--session-absolute-seconds <n>] [--sync-namespace] [--tls-certificate-secret <ref> --tls-private-key-secret <ref>]"
        );
        eprintln!(
            "       semaprax-reference-service bundle --config <service.config.json> --bundle-dir <dir>"
        );
        2
    }

    struct ServeArgs {
        project: PathBuf,
        config: PathBuf,
        state_dir: PathBuf,
        outbound_dir: PathBuf,
        secrets_dir: PathBuf,
        bundle_dir: PathBuf,
        port: u16,
        state: Option<String>,
        deployment: String,
        max_steps: usize,
        session_idle_seconds: u64,
        session_absolute_seconds: u64,
        sync_mode: OutboundCheckpointSyncMode,
        /// Both-or-neither: naming exactly one of the pair is a usage error.
        /// Naming both requests TLS serving, resolved against `--secrets-dir`
        /// exactly like the three password/session/webhook references; a
        /// missing or invalid held file refuses startup before any listener
        /// binds. Neither given keeps loopback plaintext serving, unchanged.
        tls_certificate_secret: Option<String>,
        tls_private_key_secret: Option<String>,
    }

    fn take_value(args: &[String], index: &mut usize, flag: &str) -> Option<String> {
        *index += 1;
        if *index >= args.len() {
            eprintln!("error: {flag} needs a value");
            return None;
        }
        Some(args[*index].clone())
    }

    fn parse_serve(args: &[String]) -> Option<ServeArgs> {
        let mut project = None;
        let mut config = None;
        let mut state_dir = None;
        let mut outbound_dir = None;
        let mut secrets_dir = None;
        let mut bundle_dir = None;
        let mut port = None;
        let mut state = None;
        let mut deployment = DEFAULT_DEPLOYMENT.to_owned();
        let mut max_steps = DECISION_MAX_STEPS;
        let mut session_idle_seconds = DEFAULT_SESSION_IDLE_SECONDS;
        let mut session_absolute_seconds = DEFAULT_SESSION_ABSOLUTE_SECONDS;
        let mut sync_mode = OutboundCheckpointSyncMode::FileOnly;
        let mut tls_certificate_secret = None;
        let mut tls_private_key_secret = None;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--project" => project = take_value(args, &mut index, "--project"),
                "--config" => config = take_value(args, &mut index, "--config"),
                "--state-dir" => state_dir = take_value(args, &mut index, "--state-dir"),
                "--outbound-dir" => outbound_dir = take_value(args, &mut index, "--outbound-dir"),
                "--secrets-dir" => secrets_dir = take_value(args, &mut index, "--secrets-dir"),
                "--bundle-dir" => bundle_dir = take_value(args, &mut index, "--bundle-dir"),
                "--port" => {
                    let text = take_value(args, &mut index, "--port")?;
                    match text.parse::<u16>().ok().filter(|port| *port != 0) {
                        Some(port_value) => port = Some(port_value),
                        None => {
                            eprintln!("error: --port must be 1-65535");
                            return None;
                        }
                    }
                }
                "--state" => state = take_value(args, &mut index, "--state"),
                "--deployment" => deployment = take_value(args, &mut index, "--deployment")?,
                "--max-steps" => {
                    let text = take_value(args, &mut index, "--max-steps")?;
                    match text.parse::<usize>().ok().filter(|steps| *steps != 0) {
                        Some(steps) => max_steps = steps,
                        None => {
                            eprintln!("error: --max-steps must be a positive integer");
                            return None;
                        }
                    }
                }
                "--session-idle-seconds" => {
                    let text = take_value(args, &mut index, "--session-idle-seconds")?;
                    match text.parse::<u64>() {
                        Ok(seconds) => session_idle_seconds = seconds,
                        Err(_) => {
                            eprintln!(
                                "error: --session-idle-seconds must be a nonnegative integer"
                            );
                            return None;
                        }
                    }
                }
                "--session-absolute-seconds" => {
                    let text = take_value(args, &mut index, "--session-absolute-seconds")?;
                    match text.parse::<u64>() {
                        Ok(seconds) => session_absolute_seconds = seconds,
                        Err(_) => {
                            eprintln!(
                                "error: --session-absolute-seconds must be a nonnegative integer"
                            );
                            return None;
                        }
                    }
                }
                "--sync-namespace" => sync_mode = OutboundCheckpointSyncMode::NamespaceSynced,
                "--tls-certificate-secret" => {
                    tls_certificate_secret =
                        take_value(args, &mut index, "--tls-certificate-secret")
                }
                "--tls-private-key-secret" => {
                    tls_private_key_secret =
                        take_value(args, &mut index, "--tls-private-key-secret")
                }
                flag => {
                    eprintln!("error: unknown flag {flag}");
                    return None;
                }
            }
            index += 1;
        }
        // A missing value already printed its error inside `take_value`.
        if project.is_none()
            || config.is_none()
            || state_dir.is_none()
            || outbound_dir.is_none()
            || secrets_dir.is_none()
            || bundle_dir.is_none()
            || port.is_none()
        {
            eprintln!("error: serve needs --project, --config, --state-dir, --outbound-dir, --secrets-dir, --bundle-dir, and --port");
            return None;
        }
        if let Some(digest) = &state {
            if !valid_digest(digest) {
                eprintln!("error: --state must be sha256: plus 64 lowercase hex digits");
                return None;
            }
        }
        if tls_certificate_secret.is_some() != tls_private_key_secret.is_some() {
            eprintln!(
                "error: --tls-certificate-secret and --tls-private-key-secret must both be given, or neither"
            );
            return None;
        }
        if session_idle_seconds > session_absolute_seconds
            || session_absolute_seconds > MAX_SESSION_LIFETIME_SECONDS
        {
            eprintln!("error: session deadlines must satisfy idle <= absolute <= 604800 seconds");
            return None;
        }
        Some(ServeArgs {
            project: PathBuf::from(project.unwrap()),
            config: PathBuf::from(config.unwrap()),
            state_dir: PathBuf::from(state_dir.unwrap()),
            outbound_dir: PathBuf::from(outbound_dir.unwrap()),
            secrets_dir: PathBuf::from(secrets_dir.unwrap()),
            bundle_dir: PathBuf::from(bundle_dir.unwrap()),
            port: port.unwrap(),
            state,
            deployment,
            max_steps,
            session_idle_seconds,
            session_absolute_seconds,
            sync_mode,
            tls_certificate_secret,
            tls_private_key_secret,
        })
    }

    fn valid_digest(digest: &str) -> bool {
        digest.len() == 7 + 64
            && digest.starts_with("sha256:")
            && digest[7..]
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn read_config(path: &std::path::Path) -> Option<Vec<u8>> {
        match std::fs::read(path) {
            Ok(bytes) if bytes.len() <= MAX_CONFIG_BYTES => Some(bytes),
            Ok(_) => {
                eprintln!("refused: service configuration exceeds its byte bound");
                None
            }
            Err(_) => {
                eprintln!("refused: cannot read service configuration");
                None
            }
        }
    }

    fn serve_command(args: &[String]) -> i32 {
        let Some(options) = parse_serve(args) else {
            return 2;
        };
        let Some(configuration) = read_config(&options.config) else {
            return 2;
        };
        let intent = match semaprax::project::derive_service_host_adapter_request_v1(&configuration)
        {
            Ok(intent) => intent,
            Err(detail) => {
                eprintln!("refused: service configuration is not valid host intent: {detail}");
                return 2;
            }
        };
        if intent.requirements().is_empty() {
            eprintln!("refused: fixture-mode configuration has no host runner; use semaprax run");
            return 2;
        }
        let manifest = if options.project.is_dir() {
            options.project.join("semaprax.toml")
        } else {
            options.project.clone()
        };
        let revision = match with_authenticated_project(&manifest, |snapshot| {
            Ok(snapshot.retain_revision())
        }) {
            Ok(revision) => revision,
            Err(diagnostics) => {
                eprintln!("refused: project load failed");
                for diagnostic in diagnostics.iter().take(5) {
                    eprintln!("refused: {diagnostic}");
                }
                return 2;
            }
        };
        let decisions = match DecisionEngine::bind(&revision, options.max_steps) {
            Ok(decisions) => decisions,
            Err(_) => {
                eprintln!("refused: project carries no unambiguous service decision set");
                return 2;
            }
        };
        let state_directory = match platform::hold_directory(&options.state_dir) {
            Ok(directory) => directory,
            Err(_) => {
                eprintln!("refused: cannot hold the state directory");
                return 2;
            }
        };
        let outbound_directory = match platform::hold_directory(&options.outbound_dir) {
            Ok(directory) => directory,
            Err(_) => {
                eprintln!("refused: cannot hold the outbound directory");
                return 2;
            }
        };
        let secrets_directory = match platform::hold_directory(&options.secrets_dir) {
            Ok(directory) => directory,
            Err(_) => {
                eprintln!("refused: cannot hold the secrets directory");
                return 2;
            }
        };
        let bundle_directory = match platform::hold_directory(&options.bundle_dir) {
            Ok(directory) => directory,
            Err(_) => {
                eprintln!("refused: cannot hold the bundle directory");
                return 2;
            }
        };
        let (Some(secret_refs), Some(database)) = (intent.secrets(), intent.database()) else {
            eprintln!("refused: host intent lacks secret requirements");
            return 2;
        };
        let resolved = secrets::resolve(
            &secrets_directory,
            secret_refs,
            database.dsn_secret_reference(),
        );
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(_) => {
                eprintln!("refused: cannot resolve every named host secret");
                return 2;
            }
        };
        // TLS serving is entirely optional and never implied by configuration
        // intent alone: naming both flags requests it, and only a held
        // certificate/key pair under `--secrets-dir` can satisfy that
        // request. Naming one flag without the other was already refused in
        // `parse_serve`.
        let tls_material = match (
            &options.tls_certificate_secret,
            &options.tls_private_key_secret,
        ) {
            (Some(certificate_reference), Some(private_key_reference)) => {
                match secrets::resolve_tls(
                    &secrets_directory,
                    certificate_reference,
                    private_key_reference,
                ) {
                    Ok(material) => Some(material),
                    Err(_) => {
                        eprintln!(
                            "refused: cannot resolve the held TLS certificate or private key"
                        );
                        return 2;
                    }
                }
            }
            _ => None,
        };
        let grants = match HostGrants::from_trusted_host(
            &state_directory,
            &outbound_directory,
            resolved,
            options.deployment,
            options.sync_mode,
            options.session_idle_seconds,
            options.session_absolute_seconds,
        ) {
            Ok(grants) => grants,
            Err(_) => {
                eprintln!("refused: deployment binding is not a valid outbound identity");
                return 2;
            }
        };
        let initial = match options.state {
            Some(digest) => InitialState::Digest(digest),
            None => InitialState::Genesis,
        };
        let (mut host, mut committed) = match mapping::bind(&intent, decisions, grants, initial) {
            Ok(bound) => bound,
            Err(BindRefusal::FixtureMode) => {
                eprintln!(
                    "refused: fixture-mode configuration has no host runner; use semaprax run"
                );
                return 2;
            }
            Err(BindRefusal::IncompleteRequirements) => {
                eprintln!("refused: host intent requirements are incomplete");
                return 2;
            }
            Err(BindRefusal::InvalidDeployment) => {
                eprintln!("refused: deployment binding is not a valid outbound identity");
                return 2;
            }
            Err(BindRefusal::InvalidSessionPolicy) => {
                eprintln!("refused: session deadlines are not an admitted host policy");
                return 2;
            }
            Err(BindRefusal::InvalidTelemetryOrigin) => {
                eprintln!("refused: telemetry origin is not a usable collector target");
                return 2;
            }
            Err(BindRefusal::InvalidPasswordPolicy) => {
                eprintln!("refused: password host policy is not admitted");
                return 2;
            }
            Err(BindRefusal::UnknownState) => {
                eprintln!("refused: starting state digest is unknown or stale");
                return 2;
            }
        };
        let manifest_digest = match bundle::write_bundle(
            &bundle_directory,
            &[
                (bundle::BUNDLE_CONFIG, configuration.as_slice()),
                (bundle::BUNDLE_REQUEST, intent.canonical_bytes()),
            ],
        ) {
            Ok(digest) => digest,
            Err(_) => {
                eprintln!("refused: cannot write the run bundle");
                return 2;
            }
        };
        if bundle::verify_bundle(&bundle_directory, &manifest_digest).is_err() {
            eprintln!("refused: run bundle failed verification");
            return 2;
        }
        println!("bundle {manifest_digest}");
        let mut provider = match &tls_material {
            Some(material) => {
                let server_config = match semaprax::network_provider::server_tls_config_from_der(
                    material.certificate_der().to_vec(),
                    material.private_key_der().to_vec(),
                ) {
                    Ok(config) => config,
                    Err(_) => {
                        eprintln!(
                            "refused: held TLS certificate/private key is not a usable server policy"
                        );
                        return 2;
                    }
                };
                TcpNetworkProvider::with_server_tls_config(server_config)
            }
            None => TcpNetworkProvider::new(),
        };
        let listener = match serve::listen_loopback(&mut provider, options.port) {
            Ok(listener) => listener,
            Err(_) => {
                eprintln!("refused: cannot bind the loopback listener");
                return 2;
            }
        };
        println!(
            "ready port={} state={} seq={} tls={}",
            options.port,
            committed.digest(),
            committed.state.seq,
            if tls_material.is_some() { "on" } else { "off" }
        );
        use std::io::Write as _;
        let _ = std::io::stdout().flush();
        let mut handler =
            |exchange: &serve::HttpExchange| mapping::handle(&mut host, &mut committed, exchange);
        match &tls_material {
            Some(_) => serve::serve_forever_tls(&mut provider, listener, &mut handler),
            None => serve::serve_forever(&mut provider, listener, &mut handler),
        }
    }

    fn bundle_command(args: &[String]) -> i32 {
        let mut config = None;
        let mut bundle_dir = None;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--config" => config = take_value(args, &mut index, "--config"),
                "--bundle-dir" => bundle_dir = take_value(args, &mut index, "--bundle-dir"),
                flag => {
                    eprintln!("error: unknown flag {flag}");
                    return 2;
                }
            }
            index += 1;
        }
        let (Some(config), Some(bundle_dir)) = (config, bundle_dir) else {
            eprintln!("error: bundle needs --config and --bundle-dir");
            return 2;
        };
        let Some(configuration) = read_config(std::path::Path::new(&config)) else {
            return 2;
        };
        let intent = match semaprax::project::derive_service_host_adapter_request_v1(&configuration)
        {
            Ok(intent) => intent,
            Err(detail) => {
                eprintln!("refused: service configuration is not valid: {detail}");
                return 2;
            }
        };
        let directory = match platform::hold_directory(std::path::Path::new(&bundle_dir)) {
            Ok(directory) => directory,
            Err(_) => {
                eprintln!("refused: cannot hold the bundle directory");
                return 2;
            }
        };
        let manifest_digest = match bundle::write_bundle(
            &directory,
            &[
                (bundle::BUNDLE_CONFIG, configuration.as_slice()),
                (bundle::BUNDLE_REQUEST, intent.canonical_bytes()),
            ],
        ) {
            Ok(digest) => digest,
            Err(_) => {
                eprintln!("refused: cannot write the run bundle");
                return 2;
            }
        };
        if bundle::verify_bundle(&directory, &manifest_digest).is_err() {
            eprintln!("refused: run bundle failed verification");
            return 2;
        }
        println!("bundle {manifest_digest}");
        0
    }
}
