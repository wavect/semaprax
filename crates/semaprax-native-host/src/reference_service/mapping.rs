//! The invocation/state mapping from checked decisions to CRUD/job effects.
//!
//! [`bind`] joins three things that must never be confused: decoded
//! configuration intent (origins and secret *references* only), the
//! operator-retained authenticated revision (decision selection authority),
//! and host-created grants (held directories, resolved secret bytes, the
//! deployment identity, password policy). Configuration intent alone binds
//! nothing: without grants there is no [`BoundHost`], and fixture-mode
//! intent is refused even with full grants, because fixture mode has no
//! host runner (it stays on `semaprax run` / `semaprax test`).
//!
//! [`handle`] maps one parsed HTTP exchange to a response, invoking the
//! scaffold's checked decisions as admission gates and committing accepted
//! mutations as canonical snapshots through the durable store. A refused
//! decision, a failed commit, or an unavailable delivery never mutates
//! state; every accepted mutation answers with its new content digest so
//! the operator can retain it for restart.

use hmac::{Hmac, KeyInit, Mac};
use semaprax::authentication::password::{PasswordHasherHost, PasswordPolicy};
use semaprax::authentication::{OsAuthEntropy, SecretBytes};
use semaprax::outbound_host_adapter::CheckpointCommit;
use semaprax::project::service_host_adapter_request::{
    ServiceDatabaseAdapter, ServiceHostAdapterRequestV1,
};
use semaprax_native_rust_interop_platform::HeldDirectory;
use sha2::Sha256;

use crate::outbound_delivery_store::{
    OutboundCheckpointKind, OutboundCheckpointSyncMode, OutboundDeliveryStore,
};

use super::decisions::DecisionEngine;
use super::delivery::{self, DeliveryRefusal, ProviderHttpsAdapter};
use super::json::{self, JsonValue};
use super::secrets::HeldServiceSecrets;
use super::serve::HttpExchange;
use super::state::{
    Account, Job, JobState, ServiceState, Session, Task, TaskStatus, WebhookSettlement,
    MAX_STATE_BYTES,
};

type HmacSha256 = Hmac<Sha256>;

/// The host-selected Argon2id floor for stored passwords: the minimum
/// memory/iterations the password host admits, parallelism fixed at one.
/// Production deployments select their own policy; this reference floor is
/// documented, not negotiated from configuration.
pub const PASSWORD_MEMORY_KIB: u32 = 19_456;
pub const PASSWORD_ITERATIONS: u32 = 2;
pub const PASSWORD_PARALLELISM: u32 = 1;

const MAX_BODY_BYTES: usize = 64 * 1024;
const MAX_NAME_BYTES: usize = 64;
const MAX_TITLE_BYTES: usize = 256;
const MAX_KEY_BYTES: usize = 128;
const MAX_DESC_BYTES: usize = 256;
const MIN_PASSWORD_BYTES: usize = 8;
const MAX_PASSWORD_BYTES: usize = 256;
const SESSION_ID_BYTES: usize = 16;

/// Stable refusal categories for binding intent to grants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BindRefusal {
    /// Fixture-mode intent declares no capabilities and has no host runner.
    FixtureMode,
    /// The decoded requirements are not exactly the four host capabilities.
    IncompleteRequirements,
    /// The deployment binding is not a valid outbound identity.
    InvalidDeployment,
    /// The decoded telemetry origin is not a usable collector target.
    InvalidTelemetryOrigin,
    /// The password host policy is not admitted.
    InvalidPasswordPolicy,
    /// The operator-retained starting digest is absent or not exactly valid.
    UnknownState,
}

/// Host-created grants. Every constructor input is operator-held authority;
/// decoded configuration cannot construct this value.
pub struct HostGrants<'directory> {
    state_directory: &'directory HeldDirectory,
    outbound_directory: &'directory HeldDirectory,
    secrets: HeldServiceSecrets,
    deployment_binding: String,
    sync_mode: OutboundCheckpointSyncMode,
}

impl<'directory> HostGrants<'directory> {
    /// Assemble operator-held grants. The two directories may be the same
    /// held directory; the deployment binding names this deployment for
    /// outbound-delivery identities and must match the outbound identity
    /// grammar.
    pub fn from_trusted_host(
        state_directory: &'directory HeldDirectory,
        outbound_directory: &'directory HeldDirectory,
        secrets: HeldServiceSecrets,
        deployment_binding: String,
        sync_mode: OutboundCheckpointSyncMode,
    ) -> Result<Self, BindRefusal> {
        if !valid_identity(&deployment_binding) {
            return Err(BindRefusal::InvalidDeployment);
        }
        Ok(Self {
            state_directory,
            outbound_directory,
            secrets,
            deployment_binding,
            sync_mode,
        })
    }
}

/// How the operator starts state: empty genesis, or one exact digest the
/// operator retained from an earlier accepted mutation.
pub enum InitialState {
    Genesis,
    Digest(String),
}

/// One bound reference host: decoded intent joined to host grants.
pub struct BoundHost<'revision, 'directory> {
    decisions: DecisionEngine<'revision>,
    state_store: OutboundDeliveryStore<'directory>,
    outbound_store: OutboundDeliveryStore<'directory>,
    adapter: ProviderHttpsAdapter,
    secrets: HeldServiceSecrets,
    deployment_binding: String,
    telemetry_origin: String,
    password_hasher: PasswordHasherHost,
}

/// Bind decoded host-mode intent to host grants and load the starting
/// state. Fixture-mode intent is refused: it has no runner here.
pub fn bind<'revision, 'directory>(
    intent: &ServiceHostAdapterRequestV1,
    decisions: DecisionEngine<'revision>,
    grants: HostGrants<'directory>,
    initial: InitialState,
) -> Result<(BoundHost<'revision, 'directory>, CommittedState), BindRefusal> {
    if intent.requirements().len() != 4 {
        if intent.requirements().is_empty() {
            return Err(BindRefusal::FixtureMode);
        }
        return Err(BindRefusal::IncompleteRequirements);
    }
    // The reference deployment binds the decoded database requirement to
    // the durable snapshot store: no SQLite or PostgreSQL wire protocol is
    // implemented, and the DSN value stays held but unconnected. Matching
    // the adapter here keeps that substitution explicit instead of silent.
    match intent.database().map(|database| database.adapter()) {
        Some(ServiceDatabaseAdapter::Sqlite) | Some(ServiceDatabaseAdapter::Postgresql) => {}
        None => return Err(BindRefusal::IncompleteRequirements),
    }
    // The decoded TLS listen origin is intent only: this host binds
    // loopback plaintext from an explicit operator grant (see `serve`).
    if intent.http().is_none() || intent.secrets().is_none() {
        return Err(BindRefusal::IncompleteRequirements);
    }
    let telemetry_origin = intent
        .telemetry()
        .map(|telemetry| telemetry.endpoint_origin().to_owned())
        .ok_or(BindRefusal::IncompleteRequirements)?;
    semaprax::outbound_host_adapter::TelemetryCollectorTarget::for_trusted_host(
        telemetry_origin.clone(),
    )
    .map_err(|_| BindRefusal::InvalidTelemetryOrigin)?;
    let password_hasher = PasswordHasherHost::new(
        PasswordPolicy::new(
            PASSWORD_MEMORY_KIB,
            PASSWORD_ITERATIONS,
            PASSWORD_PARALLELISM,
        )
        .map_err(|_| BindRefusal::InvalidPasswordPolicy)?,
    )
    .map_err(|_| BindRefusal::InvalidPasswordPolicy)?;
    let state_store =
        OutboundDeliveryStore::with_sync_mode(grants.state_directory, grants.sync_mode);
    let outbound_store =
        OutboundDeliveryStore::with_sync_mode(grants.outbound_directory, grants.sync_mode);
    let committed = match initial {
        InitialState::Genesis => {
            let state = ServiceState::empty();
            let rendered = state.render();
            CommittedState {
                digest: ServiceState::digest(&rendered),
                state,
            }
        }
        InitialState::Digest(digest) => {
            let bytes = state_store
                .load(OutboundCheckpointKind::ServiceState, &digest)
                .map_err(|_| BindRefusal::UnknownState)?;
            let state = ServiceState::decode(&bytes).map_err(|_| BindRefusal::UnknownState)?;
            // The store addresses content: the loaded bytes must digest back
            // to the requested address, or the operator's reference is stale.
            if ServiceState::digest(&state.render()) != digest {
                return Err(BindRefusal::UnknownState);
            }
            CommittedState { digest, state }
        }
    };
    Ok((
        BoundHost {
            decisions,
            state_store,
            outbound_store,
            adapter: ProviderHttpsAdapter::new(),
            secrets: grants.secrets,
            deployment_binding: grants.deployment_binding,
            telemetry_origin,
            password_hasher,
        },
        committed,
    ))
}

/// One committed snapshot plus its content digest.
pub struct CommittedState {
    pub state: ServiceState,
    pub digest: String,
}

impl CommittedState {
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// One mapped response: an HTTP status plus a canonical JSON body.
pub struct PendingResponse {
    pub status: u16,
    pub body: String,
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

/// Map one parsed exchange to a response, committing accepted mutations.
pub fn handle(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    exchange: &HttpExchange,
) -> PendingResponse {
    // Every exchange passes the scaffold's own request-line admission
    // decision before the host's closed route table is consulted.
    match host
        .decisions
        .request_is_admitted(exchange.method.as_bytes(), exchange.target.as_bytes())
    {
        Ok(true) => {}
        Ok(false) => return error(400, "request_not_admitted", None),
        Err(_) => return error(500, "decision_failed", None),
    }
    if exchange.method == "GET" && exchange.target == "/v1/health" {
        return json(
            200,
            JsonValue::Object(vec![
                ("seq".to_owned(), JsonValue::Int(committed.state.seq)),
                ("state".to_owned(), JsonValue::Str(committed.digest.clone())),
            ]),
        );
    }
    if exchange.method == "POST" && exchange.target == "/v1/register" {
        return register(host, committed, exchange);
    }
    if exchange.method == "POST" && exchange.target == "/v1/login" {
        return login(host, committed, exchange);
    }
    let Some(authenticated) = authenticate(host, committed, exchange) else {
        // Registration, login, and health are the only unauthenticated
        // routes; everything else needs a usable session first.
        if exchange.method == "POST" && exchange.target == "/v1/logout" {
            return error(401, "unauthorized", None);
        }
        if route_needs_auth(&exchange.method, &exchange.target) {
            return error(401, "unauthorized", None);
        }
        return error(404, "unknown_route", None);
    };
    if exchange.method == "POST" && exchange.target == "/v1/logout" {
        return logout(host, committed, &authenticated);
    }
    if exchange.method == "POST" && exchange.target == "/v1/tasks" {
        return create_task(host, committed, exchange, &authenticated);
    }
    if let Some(id) = task_member(&exchange.target) {
        match exchange.method.as_str() {
            "GET" => return get_task(host, committed, id, &authenticated),
            "PATCH" => return update_task(host, committed, exchange, id, &authenticated),
            "DELETE" => return delete_task(host, committed, id, &authenticated),
            _ => return error(404, "unknown_route", None),
        }
    }
    if exchange.method == "POST" && exchange.target == "/v1/jobs/enqueue" {
        return enqueue_job(host, committed, exchange, &authenticated);
    }
    if let Some(id) = job_member(&exchange.target, "/complete") {
        if exchange.method == "POST" {
            return complete_job(host, committed, id, &authenticated);
        }
        return error(404, "unknown_route", None);
    }
    if let Some(id) = job_member(&exchange.target, "") {
        if exchange.method == "GET" {
            return get_job(host, committed, id, &authenticated);
        }
        return error(404, "unknown_route", None);
    }
    error(404, "unknown_route", None)
}

struct Authenticated {
    account: i64,
    session: String,
}

fn route_needs_auth(method: &str, target: &str) -> bool {
    (method == "POST" && target == "/v1/logout")
        || (method == "POST" && target == "/v1/tasks")
        || task_member(target).is_some()
        || (method == "POST" && target == "/v1/jobs/enqueue")
        || job_member(target, "/complete").is_some()
        || job_member(target, "").is_some()
}

fn task_member(target: &str) -> Option<i64> {
    member_id(target, "/v1/tasks/")
}

fn job_member(target: &str, suffix: &str) -> Option<i64> {
    let rest = target.strip_prefix("/v1/jobs/")?;
    if suffix.is_empty() {
        if rest.contains('/') {
            return None;
        }
        parse_id(rest)
    } else {
        let id = rest.strip_suffix(suffix)?;
        if id.contains('/') {
            return None;
        }
        parse_id(id)
    }
}

fn member_id(target: &str, prefix: &str) -> Option<i64> {
    let rest = target.strip_prefix(prefix)?;
    if rest.is_empty() || rest.contains('/') {
        return None;
    }
    parse_id(rest)
}

fn parse_id(text: &str) -> Option<i64> {
    if text.is_empty() || text.len() > 18 {
        return None;
    }
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok().filter(|id| *id > 0)
}

fn authenticate(
    host: &BoundHost<'_, '_>,
    committed: &CommittedState,
    exchange: &HttpExchange,
) -> Option<Authenticated> {
    let value = exchange
        .headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
        .map(|(_, value)| value.as_str())?;
    let token = value.strip_prefix("Bearer ")?;
    let (id, tag) = token.split_once('.')?;
    if id.len() != SESSION_ID_BYTES * 2
        || tag.len() != 64
        || !id.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !tag.bytes().all(|byte| byte.is_ascii_hexdigit())
        || id.bytes().any(|byte| byte.is_ascii_uppercase())
        || tag.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return None;
    }
    let id_bytes = unhex(id)?;
    let mut mac = HmacSha256::new_from_slice(host.secrets.session_key()).ok()?;
    mac.update(&id_bytes);
    mac.verify_slice(&unhex(tag)?).ok()?;
    let session = committed.state.session_by_id(id)?;
    if session.retired {
        return None;
    }
    Some(Authenticated {
        account: session.account,
        session: id.to_owned(),
    })
}

fn unhex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(text.len() / 2);
    for pair in text.as_bytes().chunks(2) {
        bytes.push(u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?);
    }
    Some(bytes)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn parse_body(exchange: &HttpExchange, keys: &[&str]) -> Option<Vec<(String, JsonValue)>> {
    if exchange.body.len() > MAX_BODY_BYTES {
        return None;
    }
    let value = json::parse(&exchange.body, MAX_BODY_BYTES).ok()?;
    value.closed(keys).map(|members| members.to_vec())
}

fn body_str<'a>(body: &'a [(String, JsonValue)], key: &str) -> Option<&'a str> {
    body.iter()
        .find(|(candidate, _)| candidate == key)
        .and_then(|(_, value)| value.as_str())
}

fn error(status: u16, code: &str, state: Option<&str>) -> PendingResponse {
    let mut members = vec![("error".to_owned(), JsonValue::Str(code.to_owned()))];
    if let Some(digest) = state {
        members.push(("state".to_owned(), JsonValue::Str(digest.to_owned())));
    }
    json(status, JsonValue::Object(members))
}

fn json(status: u16, value: JsonValue) -> PendingResponse {
    PendingResponse {
        status,
        body: json::render(&value),
    }
}

fn commit(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    mut state: ServiceState,
) -> Result<(), PendingResponse> {
    state.seq = committed.state.seq.saturating_add(1);
    let rendered = state.render();
    if rendered.len() > MAX_STATE_BYTES {
        return Err(error(500, "state_too_large", Some(&committed.digest)));
    }
    let digest = ServiceState::digest(&rendered);
    match host.state_store.commit_service_state(&digest, &rendered) {
        CheckpointCommit::Committed => {
            committed.state = state;
            committed.digest = digest;
            Ok(())
        }
        CheckpointCommit::Uncertain | CheckpointCommit::NotCommitted => {
            // The commit may or may not have landed; in-memory state does
            // not advance, so a client retry replays byte-identical content
            // to `Committed` instead of forking.
            Err(error(503, "store_uncertain", Some(&committed.digest)))
        }
    }
}

fn register(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    exchange: &HttpExchange,
) -> PendingResponse {
    let Some(body) = parse_body(exchange, &["password", "username"]) else {
        return error(400, "malformed_body", None);
    };
    let (Some(username), Some(password)) =
        (body_str(&body, "username"), body_str(&body, "password"))
    else {
        return error(400, "malformed_body", None);
    };
    if username.is_empty() || username.len() > MAX_NAME_BYTES {
        return error(400, "invalid_username", None);
    }
    match host.decisions.identifier_is_valid(username.as_bytes()) {
        Ok(true) => {}
        Ok(false) => return error(400, "invalid_username", None),
        Err(_) => return error(500, "decision_failed", None),
    }
    if password.len() < MIN_PASSWORD_BYTES || password.len() > MAX_PASSWORD_BYTES {
        return error(400, "invalid_password", None);
    }
    if committed.state.account_by_name(username).is_some() {
        return error(409, "username_taken", Some(&committed.digest));
    }
    let account_id = committed
        .state
        .accounts
        .iter()
        .map(|account| account.id)
        .max()
        .unwrap_or(0)
        + 1;
    let prehash = peppered(password.as_bytes(), host.secrets.password_pepper());
    let secret = match SecretBytes::try_from_bytes(&prehash) {
        Ok(secret) => secret,
        Err(_) => return error(500, "decision_failed", None),
    };
    let hash = match host.password_hasher.hash(&secret, &mut OsAuthEntropy) {
        Ok(hash) => hash,
        Err(_) => return error(500, "decision_failed", None),
    };
    let mut state = committed.state.clone();
    let seq = committed.state.seq.saturating_add(1);
    state.accounts.push(Account {
        id: account_id,
        name: username.to_owned(),
        password_phc: hash.expose_for_storage().to_owned(),
        created_seq: seq,
    });
    if let Err(response) = commit(host, committed, state) {
        return response;
    }
    json(
        201,
        JsonValue::Object(vec![
            ("account_id".to_owned(), JsonValue::Int(account_id)),
            ("state".to_owned(), JsonValue::Str(committed.digest.clone())),
        ]),
    )
}

fn peppered(password: &[u8], pepper: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(pepper).expect("HMAC accepts the held pepper");
    mac.update(password);
    mac.finalize().into_bytes().into()
}

fn login(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    exchange: &HttpExchange,
) -> PendingResponse {
    let Some(body) = parse_body(exchange, &["password", "username"]) else {
        return error(400, "malformed_body", None);
    };
    let (Some(username), Some(password)) =
        (body_str(&body, "username"), body_str(&body, "password"))
    else {
        return error(400, "malformed_body", None);
    };
    let verified = committed
        .state
        .account_by_name(username)
        .filter(|_| password.len() >= MIN_PASSWORD_BYTES && password.len() <= MAX_PASSWORD_BYTES)
        .and_then(|account| {
            let prehash = peppered(password.as_bytes(), host.secrets.password_pepper());
            let secret = SecretBytes::try_from_bytes(&prehash).ok()?;
            let stored = semaprax::authentication::password::StoredPasswordHash::parse_for_storage(
                &account.password_phc,
            )
            .ok()?;
            host.password_hasher.verify(&secret, &stored).ok()?;
            Some(account.id)
        });
    // Unknown users, bad passwords, and corrupt records share one refusal:
    // the password host is deliberately indistinct here.
    let Some(account_id) = verified else {
        return error(401, "invalid_credentials", None);
    };
    let mut id_bytes = [0_u8; SESSION_ID_BYTES];
    if getrandom::fill(&mut id_bytes).is_err() {
        return error(500, "decision_failed", None);
    }
    let id = hex(&id_bytes);
    let mut mac = HmacSha256::new_from_slice(host.secrets.session_key())
        .expect("HMAC accepts the held session key");
    mac.update(&id_bytes);
    let token = format!("{id}.{}", hex(mac.finalize().into_bytes().as_slice()));
    let mut state = committed.state.clone();
    state.sessions.push(Session {
        id,
        account: account_id,
        retired: false,
    });
    if let Err(response) = commit(host, committed, state) {
        return response;
    }
    json(
        200,
        JsonValue::Object(vec![
            ("state".to_owned(), JsonValue::Str(committed.digest.clone())),
            ("token".to_owned(), JsonValue::Str(token)),
        ]),
    )
}

fn logout(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    authenticated: &Authenticated,
) -> PendingResponse {
    let mut state = committed.state.clone();
    let Some(session) = state
        .sessions
        .iter_mut()
        .find(|session| session.id == authenticated.session)
    else {
        return error(401, "unauthorized", None);
    };
    session.retired = true;
    if let Err(response) = commit(host, committed, state) {
        return response;
    }
    json(
        200,
        JsonValue::Object(vec![(
            "state".to_owned(),
            JsonValue::Str(committed.digest.clone()),
        )]),
    )
}

fn create_task(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    exchange: &HttpExchange,
    authenticated: &Authenticated,
) -> PendingResponse {
    let Some(body) = parse_body(exchange, &["title"]) else {
        return error(400, "malformed_body", None);
    };
    let Some(title) = body_str(&body, "title") else {
        return error(400, "malformed_body", None);
    };
    // Titles are free text, not identifiers: host bounds apply, while the
    // identifier-grammar decision guards usernames at registration.
    if title.is_empty() || title.len() > MAX_TITLE_BYTES {
        return error(400, "invalid_title", None);
    }
    let id = committed
        .state
        .tasks
        .iter()
        .map(|task| task.id)
        .max()
        .unwrap_or(0)
        + 1;
    let mut state = committed.state.clone();
    state.tasks.push(Task {
        id,
        owner: authenticated.account,
        title: title.to_owned(),
        status: TaskStatus::Open,
    });
    if let Err(response) = commit(host, committed, state) {
        return response;
    }
    json(
        201,
        JsonValue::Object(vec![
            ("id".to_owned(), JsonValue::Int(id)),
            ("state".to_owned(), JsonValue::Str(committed.digest.clone())),
        ]),
    )
}

fn authorize_row(
    host: &BoundHost<'_, '_>,
    owner: i64,
    authenticated: &Authenticated,
) -> Result<bool, PendingResponse> {
    // Host usability is presence plus non-retirement (the session passed
    // `authenticate`); the scaffold's tick-based session decision is not in
    // the invocable vocabulary, so it keeps its fixture-mode coverage.
    host.decisions
        .task_owner_authorized(owner, authenticated.account, true)
        .map_err(|_| error(500, "decision_failed", None))
}

fn task_json(task: &Task) -> JsonValue {
    JsonValue::Object(vec![
        ("id".to_owned(), JsonValue::Int(task.id)),
        ("owner".to_owned(), JsonValue::Int(task.owner)),
        (
            "status".to_owned(),
            JsonValue::Str(
                match task.status {
                    TaskStatus::Open => "open",
                    TaskStatus::Done => "done",
                }
                .to_owned(),
            ),
        ),
        ("title".to_owned(), JsonValue::Str(task.title.clone())),
    ])
}

fn get_task(
    host: &mut BoundHost<'_, '_>,
    committed: &CommittedState,
    id: i64,
    authenticated: &Authenticated,
) -> PendingResponse {
    let Some(task) = committed.state.task_by_id(id) else {
        return error(404, "unknown_task", None);
    };
    match authorize_row(host, task.owner, authenticated) {
        Ok(true) => json(200, task_json(task)),
        Ok(false) => error(403, "forbidden", None),
        Err(response) => response,
    }
}

fn update_task(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    exchange: &HttpExchange,
    id: i64,
    authenticated: &Authenticated,
) -> PendingResponse {
    let Some(body) = parse_body(exchange, &["status"]) else {
        return error(400, "malformed_body", None);
    };
    let status = match body_str(&body, "status") {
        Some("open") => TaskStatus::Open,
        Some("done") => TaskStatus::Done,
        _ => return error(400, "malformed_body", None),
    };
    let Some(task) = committed.state.task_by_id(id).cloned() else {
        return error(404, "unknown_task", None);
    };
    match authorize_row(host, task.owner, authenticated) {
        Ok(true) => {}
        Ok(false) => return error(403, "forbidden", None),
        Err(response) => return response,
    }
    let mut state = committed.state.clone();
    state
        .tasks
        .iter_mut()
        .find(|candidate| candidate.id == id)
        .expect("task present")
        .status = status;
    if let Err(response) = commit(host, committed, state) {
        return response;
    }
    json(
        200,
        JsonValue::Object(vec![(
            "state".to_owned(),
            JsonValue::Str(committed.digest.clone()),
        )]),
    )
}

fn delete_task(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    id: i64,
    authenticated: &Authenticated,
) -> PendingResponse {
    let Some(task) = committed.state.task_by_id(id).cloned() else {
        return error(404, "unknown_task", None);
    };
    match authorize_row(host, task.owner, authenticated) {
        Ok(true) => {}
        Ok(false) => return error(403, "forbidden", None),
        Err(response) => return response,
    }
    let mut state = committed.state.clone();
    state.tasks.retain(|candidate| candidate.id != id);
    if let Err(response) = commit(host, committed, state) {
        return response;
    }
    json(
        200,
        JsonValue::Object(vec![(
            "state".to_owned(),
            JsonValue::Str(committed.digest.clone()),
        )]),
    )
}

fn enqueue_job(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    exchange: &HttpExchange,
    authenticated: &Authenticated,
) -> PendingResponse {
    let Some(body) = parse_body(exchange, &["desc", "key"]) else {
        return error(400, "malformed_body", None);
    };
    let (Some(key), Some(desc)) = (body_str(&body, "key"), body_str(&body, "desc")) else {
        return error(400, "malformed_body", None);
    };
    if key.is_empty() || key.len() > MAX_KEY_BYTES || desc.is_empty() || desc.len() > MAX_DESC_BYTES
    {
        return error(400, "invalid_job", None);
    }
    let existing = committed.state.job_by_key(key);
    // The host mirrors `task_service.core.enqueue_outcome`'s documented
    // truth table (`std.jobs.idempotency.enqueue_outcome`: fresh 0,
    // duplicate 1, conflicting reuse 2) instead of invoking it: the
    // decision's closure reaches the contract-bearing
    // `std.bytes.byte_to_i64`, so invocation is refused (`SPX-F102`) while
    // the fixture scenario keeps covering the checked decision itself.
    let outcome = match existing {
        None => 0,
        Some(job) if job.desc.as_bytes() == desc.as_bytes() => 1,
        Some(_) => 2,
    };
    match outcome {
        0 => {
            if existing.is_some() {
                return error(500, "decision_failed", None);
            }
            let id = committed
                .state
                .jobs
                .iter()
                .map(|job| job.id)
                .max()
                .unwrap_or(0)
                + 1;
            let mut state = committed.state.clone();
            state.jobs.push(Job {
                id,
                owner: authenticated.account,
                key: key.to_owned(),
                desc: desc.to_owned(),
                state: JobState::Pending,
                webhook: WebhookSettlement::None,
            });
            if let Err(response) = commit(host, committed, state) {
                return response;
            }
            json(
                200,
                JsonValue::Object(vec![
                    ("id".to_owned(), JsonValue::Int(id)),
                    ("outcome".to_owned(), JsonValue::Str("created".to_owned())),
                    ("state".to_owned(), JsonValue::Str(committed.digest.clone())),
                ]),
            )
        }
        1 => {
            let Some(job) = existing else {
                return error(500, "decision_failed", None);
            };
            json(
                200,
                JsonValue::Object(vec![
                    ("id".to_owned(), JsonValue::Int(job.id)),
                    ("outcome".to_owned(), JsonValue::Str("duplicate".to_owned())),
                    ("state".to_owned(), JsonValue::Str(committed.digest.clone())),
                ]),
            )
        }
        _ => error(409, "key_conflict", Some(&committed.digest)),
    }
}

fn job_json(job: &Job) -> JsonValue {
    JsonValue::Object(vec![
        ("desc".to_owned(), JsonValue::Str(job.desc.clone())),
        ("id".to_owned(), JsonValue::Int(job.id)),
        ("key".to_owned(), JsonValue::Str(job.key.clone())),
        ("owner".to_owned(), JsonValue::Int(job.owner)),
        (
            "state".to_owned(),
            JsonValue::Str(
                match job.state {
                    JobState::Pending => "pending",
                    JobState::Completed => "completed",
                }
                .to_owned(),
            ),
        ),
        (
            "webhook".to_owned(),
            JsonValue::Str(webhook_json(&job.webhook)),
        ),
    ])
}

fn webhook_json(settlement: &WebhookSettlement) -> String {
    match settlement {
        WebhookSettlement::None => "none".to_owned(),
        WebhookSettlement::Failed => "failed".to_owned(),
        WebhookSettlement::Uncertain => "uncertain".to_owned(),
        WebhookSettlement::Delivered { evidence_digest } => {
            format!("delivered:{evidence_digest}")
        }
    }
}

fn get_job(
    host: &mut BoundHost<'_, '_>,
    committed: &CommittedState,
    id: i64,
    authenticated: &Authenticated,
) -> PendingResponse {
    let Some(job) = committed.state.job_by_id(id) else {
        return error(404, "unknown_job", None);
    };
    match authorize_row(host, job.owner, authenticated) {
        Ok(true) => json(200, job_json(job)),
        Ok(false) => error(403, "forbidden", None),
        Err(response) => response,
    }
}

fn complete_job(
    host: &mut BoundHost<'_, '_>,
    committed: &mut CommittedState,
    id: i64,
    authenticated: &Authenticated,
) -> PendingResponse {
    let Some(job) = committed.state.job_by_id(id).cloned() else {
        return error(404, "unknown_job", None);
    };
    match authorize_row(host, job.owner, authenticated) {
        Ok(true) => {}
        Ok(false) => return error(403, "forbidden", None),
        Err(response) => return response,
    }
    if job.state != JobState::Pending {
        return error(409, "already_completed", Some(&committed.digest));
    }
    // The durable delivery attempt precedes the state commit, and its
    // identity is stable per job: a crash between the two leaves a pending
    // job whose durable marker already exists, so the retry settles
    // `Uncertain` instead of redispatching. Either way the job completes
    // exactly once in state.
    let settlement = match delivery::deliver_completion_webhook(
        &mut host.outbound_store,
        &host.deployment_binding,
        &host.telemetry_origin,
        job.id,
        job.owner,
        &job.desc,
        host.secrets.webhook_key(),
        &mut host.adapter,
    ) {
        Ok(settlement) => settlement,
        Err(DeliveryRefusal::InvalidRequest | DeliveryRefusal::InvalidPolicy) => {
            return error(500, "decision_failed", None)
        }
        Err(DeliveryRefusal::StoreUnavailable) => {
            return error(503, "delivery_unavailable", Some(&committed.digest))
        }
    };
    let mut state = committed.state.clone();
    let stored = state
        .jobs
        .iter_mut()
        .find(|candidate| candidate.id == id)
        .expect("job present");
    stored.state = JobState::Completed;
    stored.webhook = settlement;
    if let Err(response) = commit(host, committed, state) {
        return response;
    }
    let job = committed.state.job_by_id(id).expect("job present");
    json(
        200,
        JsonValue::Object(vec![
            ("state".to_owned(), JsonValue::Str(committed.digest.clone())),
            (
                "webhook".to_owned(),
                JsonValue::Str(webhook_json(&job.webhook)),
            ),
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference_service::test_support::TempDir;
    use semaprax::project::with_authenticated_project;
    use semaprax_native_rust_interop_platform as platform;
    use std::ffi::OsStr;

    const DEPLOYMENT: &str = "reference-service-test-v1";

    struct Fixture {
        _state: TempDir,
        _outbound: TempDir,
        _secrets: TempDir,
        host: BoundHost<'static, 'static>,
        committed: CommittedState,
    }

    // The revision, directories, and secrets outlive the test body through
    // intentional leaks: this keeps the fixture's lifetimes simple without
    // changing any production signature for tests.
    fn fixture() -> Fixture {
        // The project loader rejects `.`/`..` components, so the fixture
        // path is canonicalized before loading.
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("task-service-project")
            .join("semaprax.toml")
            .canonicalize()
            .expect("canonicalize task-service-project fixture");
        let revision: &'static semaprax::project::ProjectRevision = Box::leak(Box::new(
            with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
                .expect("load task-service-project fixture"),
        ));
        let (state_dir, state_held) = TempDir::hold("reference-mapping-state");
        let (outbound_dir, outbound_held) = TempDir::hold("reference-mapping-outbound");
        let (secrets_dir, secrets_held) = TempDir::hold("reference-mapping-secrets");
        let state_held: &'static HeldDirectory = Box::leak(Box::new(state_held));
        let outbound_held: &'static HeldDirectory = Box::leak(Box::new(outbound_held));
        write_secret(&secrets_held, "auth.pepper", &[1_u8; 32]);
        write_secret(&secrets_held, "auth.session", &[2_u8; 32]);
        write_secret(&secrets_held, "webhook.signing", &[3_u8; 32]);
        write_secret(&secrets_held, "db.primary", b"held-but-unconnected");
        let intent = decode_host_intent();
        let secrets = super::super::secrets::resolve(
            &secrets_held,
            intent.secrets().unwrap(),
            intent.database().unwrap().dsn_secret_reference(),
        )
        .unwrap();
        let decisions =
            DecisionEngine::bind(revision, super::super::decisions::DECISION_MAX_STEPS).unwrap();
        let grants = HostGrants::from_trusted_host(
            state_held,
            outbound_held,
            secrets,
            DEPLOYMENT.to_owned(),
            OutboundCheckpointSyncMode::FileOnly,
        )
        .unwrap();
        let (host, committed) = bind(&intent, decisions, grants, InitialState::Genesis).unwrap();
        Fixture {
            _state: state_dir,
            _outbound: outbound_dir,
            _secrets: secrets_dir,
            host,
            committed,
        }
    }

    fn write_secret(directory: &HeldDirectory, name: &str, bytes: &[u8]) {
        let _ = platform::write_file_new(directory, OsStr::new(name), bytes, 0o600).unwrap();
    }

    fn decode_host_intent() -> ServiceHostAdapterRequestV1 {
        let text = r#"{"capabilities":["semaprax.service.database.connect.v1","semaprax.service.http.serve-tls.v1","semaprax.service.secrets.resolve.v1","semaprax.service.telemetry.emit.v1"],"database":{"adapter":"sqlite","dsn_secret_ref":"db.primary","migration_table":"semaprax_migrations"},"http":{"adapter":"native","listen_origin":"https://service.example","tls_profile":"modern"},"mode":"host","schema":"semaprax.service-host-adapter-request.v1","secrets":{"password_pepper_ref":"auth.pepper","session_signing_key_ref":"auth.session","webhook_signing_key_ref":"webhook.signing"},"telemetry":{"adapter":"otlp","endpoint_origin":"https://127.0.0.1:9"}}"#;
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(b'\n');
        semaprax::project::service_host_adapter_request::decode(&bytes).unwrap()
    }

    fn exchange(method: &str, target: &str, body: &str, token: Option<&str>) -> HttpExchange {
        let mut headers = vec![("content-length".to_owned(), body.len().to_string())];
        if let Some(token) = token {
            headers.push(("authorization".to_owned(), format!("Bearer {token}")));
        }
        HttpExchange {
            method: method.to_owned(),
            target: target.to_owned(),
            headers,
            body: body.as_bytes().to_vec(),
        }
    }

    fn field(body: &str, key: &str) -> JsonValue {
        json::parse(body.as_bytes(), 64 * 1024)
            .unwrap()
            .get(key)
            .unwrap()
            .clone()
    }

    #[test]
    fn register_login_crud_logout_round_trip() {
        // The fixture stays whole: destructuring it would drop the
        // directory guards and delete the held directories mid-test.
        let mut fixture = fixture();
        let health = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("GET", "/v1/health", "", None),
        );
        assert_eq!(health.status, 200);

        let registered = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/register",
                r#"{"username":"alice","password":"correct horse 7"}"#,
                None,
            ),
        );
        assert_eq!(registered.status, 201, "{}", registered.body);
        let account_id = field(&registered.body, "account_id").as_i64().unwrap();
        assert_eq!(account_id, 1);

        let duplicate = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/register",
                r#"{"username":"alice","password":"another secret 8"}"#,
                None,
            ),
        );
        assert_eq!(duplicate.status, 409);

        let bad_name = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/register",
                r#"{"username":"1alice","password":"correct horse 7"}"#,
                None,
            ),
        );
        assert_eq!(bad_name.status, 400);

        let denied = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/login",
                r#"{"username":"alice","password":"wrong password 0"}"#,
                None,
            ),
        );
        assert_eq!(denied.status, 401);

        let logged_in = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/login",
                r#"{"username":"alice","password":"correct horse 7"}"#,
                None,
            ),
        );
        assert_eq!(logged_in.status, 200, "{}", logged_in.body);
        let token = field(&logged_in.body, "token").as_str().unwrap().to_owned();

        let created = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/tasks",
                r#"{"title":"write the report"}"#,
                Some(&token),
            ),
        );
        assert_eq!(created.status, 201, "{}", created.body);

        let fetched = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("GET", "/v1/tasks/1", "", Some(&token)),
        );
        assert_eq!(fetched.status, 200);
        assert_eq!(
            field(&fetched.body, "title").as_str().unwrap(),
            "write the report"
        );

        let updated = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("PATCH", "/v1/tasks/1", r#"{"status":"done"}"#, Some(&token)),
        );
        assert_eq!(updated.status, 200);

        let deleted = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("DELETE", "/v1/tasks/1", "", Some(&token)),
        );
        assert_eq!(deleted.status, 200);
        let gone = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("GET", "/v1/tasks/1", "", Some(&token)),
        );
        assert_eq!(gone.status, 404);

        let logged_out = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("POST", "/v1/logout", "", Some(&token)),
        );
        assert_eq!(logged_out.status, 200);
        let retired = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("GET", "/v1/tasks/1", "", Some(&token)),
        );
        assert_eq!(retired.status, 401);
    }

    #[test]
    fn fixture_intent_and_bad_deployment_refuse_binding() {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("task-service-project")
            .join("semaprax.toml")
            .canonicalize()
            .expect("canonicalize task-service-project fixture");
        let revision =
            with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
                .expect("load task-service-project fixture");
        let decisions =
            DecisionEngine::bind(&revision, super::super::decisions::DECISION_MAX_STEPS).unwrap();
        let (_temp, directory) = TempDir::hold("reference-mapping-bind");
        let (_secrets_temp, secrets_dir) = TempDir::hold("reference-mapping-bind-secrets");
        write_secret(&secrets_dir, "auth.pepper", &[1_u8; 32]);
        write_secret(&secrets_dir, "auth.session", &[2_u8; 32]);
        write_secret(&secrets_dir, "webhook.signing", &[3_u8; 32]);
        write_secret(&secrets_dir, "db.primary", b"held-but-unconnected");
        let host_intent = decode_host_intent();
        let secrets = super::super::secrets::resolve(
            &secrets_dir,
            host_intent.secrets().unwrap(),
            host_intent.database().unwrap().dsn_secret_reference(),
        )
        .unwrap();
        // Full grants plus fixture-mode intent still refuse: configuration
        // intent never mints a runner.
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("task-service-project")
            .join("service-host-adapter-request.json");
        let fixture_bytes = std::fs::read(&fixture_path).unwrap();
        let fixture_intent =
            semaprax::project::service_host_adapter_request::decode(&fixture_bytes).unwrap();
        let grants = HostGrants::from_trusted_host(
            &directory,
            &directory,
            secrets,
            DEPLOYMENT.to_owned(),
            OutboundCheckpointSyncMode::FileOnly,
        )
        .unwrap();
        // `grants` is moved by the first bind; rebuild the secrets side for
        // the second attempt below.
        let secrets = super::super::secrets::resolve(
            &secrets_dir,
            host_intent.secrets().unwrap(),
            host_intent.database().unwrap().dsn_secret_reference(),
        )
        .unwrap();
        assert_eq!(
            bind(&fixture_intent, decisions, grants, InitialState::Genesis)
                .err()
                .unwrap(),
            BindRefusal::FixtureMode
        );
        // A deployment binding outside the outbound identity grammar is not
        // a grant at all.
        assert_eq!(
            HostGrants::from_trusted_host(
                &directory,
                &directory,
                secrets,
                "not a valid identity!".to_owned(),
                OutboundCheckpointSyncMode::FileOnly,
            )
            .err()
            .unwrap(),
            BindRefusal::InvalidDeployment
        );
    }

    #[test]
    fn job_enqueue_is_idempotent_and_completion_settles_once() {
        // The fixture stays whole: destructuring it would drop the
        // directory guards and delete the held directories mid-test.
        let mut fixture = fixture();
        let registered = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/register",
                r#"{"username":"bob","password":"correct horse 7"}"#,
                None,
            ),
        );
        assert_eq!(registered.status, 201, "{}", registered.body);
        let logged_in = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/login",
                r#"{"username":"bob","password":"correct horse 7"}"#,
                None,
            ),
        );
        assert_eq!(logged_in.status, 200);
        let token = field(&logged_in.body, "token").as_str().unwrap().to_owned();

        let enqueued = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/jobs/enqueue",
                r#"{"key":"job-1","desc":"task-1"}"#,
                Some(&token),
            ),
        );
        assert_eq!(enqueued.status, 200, "{}", enqueued.body);
        assert_eq!(
            field(&enqueued.body, "outcome").as_str().unwrap(),
            "created"
        );

        let duplicate = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/jobs/enqueue",
                r#"{"key":"job-1","desc":"task-1"}"#,
                Some(&token),
            ),
        );
        assert_eq!(duplicate.status, 200);
        assert_eq!(
            field(&duplicate.body, "outcome").as_str().unwrap(),
            "duplicate"
        );
        assert_eq!(
            field(&duplicate.body, "state").as_str(),
            field(&enqueued.body, "state").as_str()
        );

        let conflict = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange(
                "POST",
                "/v1/jobs/enqueue",
                r#"{"key":"job-1","desc":"other"}"#,
                Some(&token),
            ),
        );
        assert_eq!(conflict.status, 409);

        // No peer listens on 127.0.0.1:9, so the durable attempt fails
        // closed and settles `Uncertain` without redispatch; the job still
        // completes exactly once in state.
        let completed = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("POST", "/v1/jobs/1/complete", "", Some(&token)),
        );
        assert_eq!(completed.status, 200, "{}", completed.body);
        assert_eq!(
            field(&completed.body, "webhook").as_str().unwrap(),
            "uncertain"
        );

        let again = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("POST", "/v1/jobs/1/complete", "", Some(&token)),
        );
        assert_eq!(again.status, 409);

        let queried = handle(
            &mut fixture.host,
            &mut fixture.committed,
            &exchange("GET", "/v1/jobs/1", "", Some(&token)),
        );
        assert_eq!(queried.status, 200);
        assert_eq!(field(&queried.body, "state").as_str().unwrap(), "completed");
    }
}
